//! Raw PCM sources: interleaved stereo at 48 kHz.

use std::fmt;
use std::io::{ErrorKind, Read};

use super::{AudioError, AudioSource, FRAME_LEN, SourceFrame};

enum Input {
    F32(Vec<f32>),
    I16(Vec<i16>),
    Reader(Box<dyn Read + Send>),
}

/// A source of raw PCM: interleaved stereo samples at 48 kHz.
///
/// It needs the `opus` feature to be sent, because the send task encodes it.
pub struct PcmSource {
    input: Input,
    position: usize,
    finished: bool,
}

impl fmt::Debug for PcmSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match &self.input {
            Input::F32(_) => "f32",
            Input::I16(_) => "i16",
            Input::Reader(_) => "reader",
        };
        formatter
            .debug_struct("PcmSource")
            .field("input", &kind)
            .field("position", &self.position)
            .finish_non_exhaustive()
    }
}

impl PcmSource {
    /// Makes a source from `f32` samples in -1.0 to 1.0.
    #[must_use]
    pub fn from_f32(samples: Vec<f32>) -> Self {
        Self::with_input(Input::F32(samples))
    }

    /// Makes a source from `i16` samples.
    #[must_use]
    pub fn from_i16(samples: Vec<i16>) -> Self {
        Self::with_input(Input::I16(samples))
    }

    /// Makes a source that reads signed 16-bit little-endian samples, for example the output
    /// of `ffmpeg -f s16le -ar 48000 -ac 2 -`.
    #[must_use]
    pub fn from_reader(reader: impl Read + Send + 'static) -> Self {
        Self::with_input(Input::Reader(Box::new(reader)))
    }

    fn with_input(input: Input) -> Self {
        Self {
            input,
            position: 0,
            finished: false,
        }
    }

    fn read_frame(reader: &mut dyn Read) -> Result<Option<Vec<f32>>, AudioError> {
        let mut bytes = vec![0_u8; FRAME_LEN * 2];
        let mut filled = 0;
        while filled < bytes.len() {
            match reader.read(&mut bytes[filled..]) {
                Ok(0) => break,
                Ok(count) => filled += count,
                Err(error) if error.kind() == ErrorKind::Interrupted => {}
                Err(error) => return Err(error.into()),
            }
        }
        if filled < 2 {
            return Ok(None);
        }

        let mut frame: Vec<f32> = bytes[..filled - filled % 2]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| i16_to_f32(i16::from_le_bytes(*pair)))
            .collect();
        frame.resize(FRAME_LEN, 0.0);
        Ok(Some(frame))
    }
}

impl AudioSource for PcmSource {
    fn next_frame(&mut self) -> Result<Option<SourceFrame>, AudioError> {
        if self.finished {
            return Ok(None);
        }

        let frame = match &mut self.input {
            Input::F32(samples) => take_frame(samples, self.position, |sample| sample),
            Input::I16(samples) => take_frame(samples, self.position, i16_to_f32),
            Input::Reader(reader) => Self::read_frame(reader.as_mut())?,
        };
        self.position += FRAME_LEN;
        if frame.is_none() {
            self.finished = true;
        }
        Ok(frame.map(SourceFrame::Pcm))
    }
}

fn take_frame<T: Copy>(
    samples: &[T],
    start: usize,
    convert: impl Fn(T) -> f32,
) -> Option<Vec<f32>> {
    let rest = samples.get(start..)?;
    if rest.is_empty() {
        return None;
    }
    let mut frame: Vec<f32> = rest
        .iter()
        .take(FRAME_LEN)
        .map(|sample| convert(*sample))
        .collect();
    frame.resize(FRAME_LEN, 0.0);
    Some(frame)
}

/// Converts an `i16` sample to `f32` in -1.0 to 1.0.
#[must_use]
pub fn i16_to_f32(sample: i16) -> f32 {
    f32::from(sample) / 32_768.0
}

/// Converts an `f32` sample to `i16`, with clipping.
#[must_use]
pub fn f32_to_i16(sample: f32) -> i16 {
    let scaled = (sample * 32_768.0).round().clamp(-32_768.0, 32_767.0);
    // The value is in the range of i16 after the clamp.
    #[allow(clippy::cast_possible_truncation)]
    let value = scaled as i16;
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collect(mut source: PcmSource) -> Vec<Vec<f32>> {
        let mut frames = Vec::new();
        while let Some(frame) = source.next_frame().expect("no error") {
            let SourceFrame::Pcm(samples) = frame else {
                panic!("PCM expected");
            };
            frames.push(samples);
        }
        frames
    }

    #[test]
    fn frames_are_padded() {
        let frames = collect(PcmSource::from_i16(vec![16_384; FRAME_LEN + 10]));
        assert_eq!(frames.len(), 2);
        assert!(frames.iter().all(|frame| frame.len() == FRAME_LEN));
        assert!((frames[0][0] - 0.5).abs() < f32::EPSILON);
        assert!((frames[1][9] - 0.5).abs() < f32::EPSILON);
        assert!(frames[1][10].abs() < f32::EPSILON);
        assert!(collect(PcmSource::from_f32(Vec::new())).is_empty());
    }

    #[test]
    fn reader_input() {
        let mut bytes = Vec::new();
        for _ in 0..FRAME_LEN * 3 / 2 {
            bytes.extend_from_slice(&(-16_384_i16).to_le_bytes());
        }
        let frames = collect(PcmSource::from_reader(std::io::Cursor::new(bytes)));
        assert_eq!(frames.len(), 2);
        assert!((frames[1][0] + 0.5).abs() < f32::EPSILON);
        assert!(frames[1][FRAME_LEN / 2].abs() < f32::EPSILON);
    }

    #[test]
    fn conversions() {
        assert_eq!(f32_to_i16(2.0), i16::MAX);
        assert_eq!(f32_to_i16(-2.0), i16::MIN);
        assert_eq!(f32_to_i16(i16_to_f32(1234)), 1234);
    }
}
