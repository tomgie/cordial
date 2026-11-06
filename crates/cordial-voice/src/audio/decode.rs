//! A source that decodes common audio formats with symphonia. Needs the `symphonia` feature.
//!
//! The source converts the audio to stereo at 48 kHz. It uses linear interpolation for the
//! sample rate change, which is good enough for speech and music in a voice call.

use std::collections::VecDeque;
use std::fmt;
use std::fs::File;
use std::path::Path;
use std::time::Duration;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{CODEC_TYPE_NULL, Decoder, DecoderOptions};
use symphonia::core::errors::{Error as SymphoniaError, SeekErrorKind};
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo};
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::units::TimeBase;

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

    fn reset(&mut self) {
        self.position = 0.0;
        self.input.clear();
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
///
/// The source can seek when the format reader of symphonia can seek. The seek is accurate to
/// the sample of the input: the source decodes from the packet before the position and drops
/// the samples before the position.
pub struct DecodedSource {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn Decoder>,
    track_id: u32,
    input_rate: u32,
    time_base: Option<TimeBase>,
    resampler: Resampler,
    pending: VecDeque<f32>,
    /// After a seek, the decoded frames before this timestamp are dropped.
    skip_until: Option<u64>,
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
        let time_base = track.codec_params.time_base;
        let decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &DecoderOptions::default())
            .map_err(|error| format_error(&error))?;

        Ok(Self {
            format,
            decoder,
            track_id,
            input_rate,
            time_base,
            resampler: Resampler::new(input_rate),
            pending: VecDeque::new(),
            skip_until: None,
            finished: false,
        })
    }

    /// Converts a timestamp difference of the track to a number of input frames.
    fn timestamp_to_frames(&self, timestamps: u64) -> u64 {
        let Some(time_base) = self.time_base else {
            return timestamps;
        };
        let time = time_base.calc_time(timestamps);
        let rate = u64::from(self.input_rate);
        // The fraction is in 0.0 to 1.0, so the product is less than the rate.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let fraction = (time.frac * rate as f64).round() as u64;
        time.seconds.saturating_mul(rate).saturating_add(fraction)
    }

    /// Gives the number of frames to drop at the start of a packet after a seek.
    fn frames_to_skip(&mut self, packet_timestamp: u64, frame_count: usize) -> usize {
        let Some(skip_until) = self.skip_until else {
            return 0;
        };
        let skip = self.timestamp_to_frames(skip_until.saturating_sub(packet_timestamp));
        let skip = usize::try_from(skip).unwrap_or(usize::MAX);
        if skip < frame_count {
            self.skip_until = None;
        }
        skip.min(frame_count)
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
        let skip = self.frames_to_skip(packet.ts(), frames.len());
        self.resampler.push(&frames[skip..], &mut self.pending);
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

    fn seek(&mut self, position: Duration) -> Result<Duration, AudioError> {
        let target = SeekTo::Time {
            time: position.into(),
            track_id: Some(self.track_id),
        };
        let result = self.format.seek(SeekMode::Accurate, target);

        self.decoder.reset();
        self.resampler.reset();
        self.pending.clear();
        match result {
            Ok(seeked) => {
                self.skip_until = Some(seeked.required_ts);
                self.finished = false;
                Ok(position)
            }
            Err(SymphoniaError::SeekError(SeekErrorKind::OutOfRange)) => {
                self.skip_until = None;
                self.finished = true;
                Ok(position)
            }
            Err(SymphoniaError::SeekError(SeekErrorKind::Unseekable)) => Err(
                AudioError::Unsupported("the input of the decoder cannot seek".to_owned()),
            ),
            Err(error) => Err(format_error(&error)),
        }
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
    fn seek_in_wav() {
        // Each sample holds its index divided by 4, so the first value shows the position.
        let samples: Vec<i16> = (0..48_000_i32)
            .map(|index| i16::try_from(index / 4).expect("small"))
            .collect();
        let mut source =
            DecodedSource::from_bytes(wav(48_000, &samples), Some("wav")).expect("valid");
        source.next_frame().expect("valid");

        let position = source.seek(Duration::from_millis(250)).expect("seek");
        assert_eq!(position, Duration::from_millis(250));
        let Some(SourceFrame::Pcm(frame)) = source.next_frame().expect("valid") else {
            panic!("PCM expected");
        };
        assert_eq!(crate::audio::pcm::f32_to_i16(frame[0]), 12_000 / 4);
        assert_eq!(crate::audio::pcm::f32_to_i16(frame[2 * 400]), 12_400 / 4);

        source.seek(Duration::ZERO).expect("seek");
        assert_eq!(frames(source).len(), 50);

        let mut source =
            DecodedSource::from_bytes(wav(48_000, &samples), Some("wav")).expect("valid");
        source
            .seek(Duration::from_secs(5))
            .expect("seek after the end");
        assert_eq!(source.next_frame().expect("valid"), None);
    }

    #[test]
    fn unknown_format() {
        assert!(DecodedSource::from_bytes(vec![1, 2, 3, 4], None).is_err());
    }
}
