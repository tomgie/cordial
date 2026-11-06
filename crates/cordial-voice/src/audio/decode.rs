//! A source that decodes common audio formats with symphonia. Needs the `symphonia` feature.
//!
//! The source converts the audio to stereo at 48 kHz. It uses linear interpolation for the
//! sample rate change, which is good enough for speech and music in a voice call.

use std::collections::VecDeque;
use std::fmt;
use std::fs::File;
use std::path::Path;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{CODEC_TYPE_NULL, Decoder, DecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::{FormatOptions, FormatReader};
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use super::{AudioError, AudioSource, FRAME_LEN, SAMPLE_RATE, SourceFrame};

fn format_error(error: &SymphoniaError) -> AudioError {
    match error {
        SymphoniaError::Unsupported(what) => AudioError::Unsupported((*what).to_owned()),
        other => AudioError::Format(other.to_string()),
    }
}

/// Changes the sample rate of stereo audio with linear interpolation.
#[derive(Debug)]
struct Resampler {
    step: f64,
    position: f64,
    input: Vec<[f32; 2]>,
}

impl Resampler {
    fn new(input_rate: u32) -> Self {
        Self {
            step: f64::from(input_rate) / f64::from(SAMPLE_RATE),
            position: 0.0,
            input: Vec::new(),
        }
    }

    fn push(&mut self, frames: &[[f32; 2]], output: &mut VecDeque<f32>) {
        if (self.step - 1.0).abs() < f64::EPSILON {
            output.extend(frames.iter().flatten());
            return;
        }
        self.input.extend_from_slice(frames);
        while self.position + 1.0 < self.input.len() as f64 {
            // The position is not negative and is less than the input length.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let index = self.position as usize;
            #[allow(clippy::cast_possible_truncation)]
            let fraction = (self.position - index as f64) as f32;
            let [left_a, right_a] = self.input[index];
            let [left_b, right_b] = self.input[index + 1];
            output.push_back(left_a + (left_b - left_a) * fraction);
            output.push_back(right_a + (right_b - right_a) * fraction);
            self.position += self.step;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let consumed = (self.position as usize).min(self.input.len());
        self.input.drain(..consumed);
        self.position -= consumed as f64;
    }
}

/// An audio source that decodes a file with symphonia: MP3, FLAC, WAV, Ogg Vorbis, AAC, and
/// MP4 audio.
pub struct DecodedSource {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn Decoder>,
    track_id: u32,
    resampler: Resampler,
    pending: VecDeque<f32>,
    finished: bool,
}

impl fmt::Debug for DecodedSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DecodedSource")
            .field("track_id", &self.track_id)
            .field("finished", &self.finished)
            .finish_non_exhaustive()
    }
}

impl DecodedSource {
    /// Opens a file. The file extension helps to find the format.
    ///
    /// # Errors
    ///
    /// Gives an error when the file cannot be opened or its format is not supported.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, AudioError> {
        let path = path.as_ref();
        let file = File::open(path)?;
        let extension = path.extension().and_then(|extension| extension.to_str());
        Self::new(Box::new(file), extension)
    }

    /// Makes a source from the bytes of a file, with an optional file extension as a hint.
    ///
    /// # Errors
    ///
    /// Gives an error when the format is not supported.
    pub fn from_bytes(bytes: Vec<u8>, extension: Option<&str>) -> Result<Self, AudioError> {
        Self::new(Box::new(std::io::Cursor::new(bytes)), extension)
    }

    /// Makes a source from a symphonia media source.
    ///
    /// # Errors
    ///
    /// Gives an error when the format is not supported.
    pub fn new(source: Box<dyn MediaSource>, extension: Option<&str>) -> Result<Self, AudioError> {
        let stream = MediaSourceStream::new(source, Default::default());
        let mut hint = Hint::new();
        if let Some(extension) = extension {
            hint.with_extension(extension);
        }
        let probed = symphonia::default::get_probe()
            .format(
                &hint,
                stream,
                &FormatOptions::default(),
                &MetadataOptions::default(),
            )
            .map_err(|error| format_error(&error))?;
        let format = probed.format;

        let track = format
            .tracks()
            .iter()
            .find(|track| track.codec_params.codec != CODEC_TYPE_NULL)
            .ok_or_else(|| AudioError::Unsupported("the input has no audio track".to_owned()))?;
        let track_id = track.id;
        let input_rate = track.codec_params.sample_rate.unwrap_or(SAMPLE_RATE);
        let decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &DecoderOptions::default())
            .map_err(|error| format_error(&error))?;

        Ok(Self {
            format,
            decoder,
            track_id,
            resampler: Resampler::new(input_rate),
            pending: VecDeque::new(),
            finished: false,
        })
    }

    /// Decodes the next packet into the pending samples. Gives `false` at the end.
    fn decode_next(&mut self) -> Result<bool, AudioError> {
        let packet = match self.format.next_packet() {
            Ok(packet) => packet,
            Err(SymphoniaError::IoError(error))
                if error.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                return Ok(false);
            }
            Err(error) => return Err(format_error(&error)),
        };
        if packet.track_id() != self.track_id {
            return Ok(true);
        }

        let decoded = match self.decoder.decode(&packet) {
            Ok(decoded) => decoded,
            Err(SymphoniaError::DecodeError(error)) => {
                tracing::debug!(error, "a packet cannot be decoded; skipped");
                return Ok(true);
            }
            Err(error) => return Err(format_error(&error)),
        };
        let spec = *decoded.spec();
        let channels = spec.channels.count().max(1);
        let mut buffer = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
        buffer.copy_interleaved_ref(decoded);

        let frames: Vec<[f32; 2]> = buffer
            .samples()
            .chunks_exact(channels)
            .map(|frame| match frame {
                [mono] => [*mono, *mono],
                [left, right, ..] => [*left, *right],
                [] => [0.0, 0.0],
            })
            .collect();
        self.resampler.push(&frames, &mut self.pending);
        Ok(true)
    }
}

impl AudioSource for DecodedSource {
    fn next_frame(&mut self) -> Result<Option<SourceFrame>, AudioError> {
        while !self.finished && self.pending.len() < FRAME_LEN {
            if !self.decode_next()? {
                self.finished = true;
            }
        }
        if self.pending.is_empty() {
            return Ok(None);
        }
        let count = self.pending.len().min(FRAME_LEN);
        let mut frame: Vec<f32> = self.pending.drain(..count).collect();
        frame.resize(FRAME_LEN, 0.0);
        Ok(Some(SourceFrame::Pcm(frame)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Makes a WAV file: 16-bit mono PCM.
    fn wav(rate: u32, samples: &[i16]) -> Vec<u8> {
        let data_len = u32::try_from(samples.len() * 2).expect("small");
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16_u32.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&rate.to_le_bytes());
        bytes.extend_from_slice(&(rate * 2).to_le_bytes());
        bytes.extend_from_slice(&2_u16.to_le_bytes());
        bytes.extend_from_slice(&16_u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_len.to_le_bytes());
        for sample in samples {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        bytes
    }

    fn frames(mut source: DecodedSource) -> Vec<Vec<f32>> {
        let mut frames = Vec::new();
        while let Some(SourceFrame::Pcm(frame)) = source.next_frame().expect("valid") {
            frames.push(frame);
        }
        frames
    }

    #[test]
    fn wav_at_48k_mono() {
        let source =
            DecodedSource::from_bytes(wav(48_000, &[16_384; 1000]), Some("wav")).expect("valid");
        let frames = frames(source);
        assert_eq!(frames.len(), 2);
        assert!((frames[0][0] - 0.5).abs() < 1e-3);
        assert!((frames[0][1] - 0.5).abs() < 1e-3);
        assert!((frames[1][(1000 - 960) * 2 - 1] - 0.5).abs() < 1e-3);
        assert!(frames[1][(1000 - 960) * 2].abs() < 1e-6);
    }

    #[test]
    fn wav_at_24k_is_resampled() {
        let source =
            DecodedSource::from_bytes(wav(24_000, &[8_192; 4800]), Some("wav")).expect("valid");
        let frames = frames(source);
        // 0.2 s at 48 kHz is 10 frames.
        assert!((9..=10).contains(&frames.len()));
        assert!((frames[3][100] - 0.25).abs() < 1e-3);
    }

    #[test]
    fn unknown_format() {
        assert!(DecodedSource::from_bytes(vec![1, 2, 3, 4], None).is_err());
    }
}
