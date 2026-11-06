//! Audio: sources, the mixer, Ogg Opus, and the Opus codec.
//!
//! Discord audio is Opus at 48 kHz, stereo, in frames of 20 ms (960 samples for each
//! channel). PCM in this crate is interleaved stereo at 48 kHz.

use std::time::Duration;

#[cfg(feature = "opus")]
pub mod codec;
#[cfg(feature = "symphonia")]
pub mod decode;
pub mod mixer;
pub mod ogg;
pub mod pcm;
pub mod process;

/// The sample rate.
pub const SAMPLE_RATE: u32 = 48_000;
/// The number of channels.
pub const CHANNELS: usize = 2;
/// The number of samples for each channel in one 20 ms frame.
pub const FRAME_SAMPLES: usize = 960;
/// The number of interleaved samples in one 20 ms frame.
pub const FRAME_LEN: usize = FRAME_SAMPLES * CHANNELS;
/// The duration of one frame.
pub const FRAME_DURATION: Duration = Duration::from_millis(20);
/// The Opus silence frame. The sender sends 5 of them before a pause.
pub const SILENCE_FRAME: [u8; 3] = [0xF8, 0xFF, 0xFE];
/// The number of silence frames before a pause.
pub const SILENCE_FRAMES_BEFORE_PAUSE: u8 = 5;

/// A frame that an audio source gives.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum SourceFrame {
    /// 20 ms of PCM: [`FRAME_LEN`] interleaved stereo samples in -1.0 to 1.0.
    Pcm(Vec<f32>),
    /// An Opus packet that goes out without a new encode.
    ///
    /// The discard values apply only when the mixer decodes the packet. A packet that goes
    /// out without a new encode plays in full, because an Opus packet cannot be cut.
    Opus {
        /// The Opus packet.
        packet: Vec<u8>,
        /// The duration of the packet in samples for each channel at 48 kHz.
        samples: u32,
        /// The number of decoded samples for each channel to drop at the start of the packet,
        /// for example the Ogg Opus pre-skip or the part before a seek position.
        discard_start: u32,
        /// The number of decoded samples for each channel to drop at the end of the packet,
        /// for example the Ogg Opus end trim.
        discard_end: u32,
    },
    /// The source has no audio now, but it is not at its end. The mixer asks again on the
    /// next frame. A source that reads from another thread or a program gives this frame
    /// when its buffer is empty.
    Waiting,
}

impl SourceFrame {
    /// Makes an Opus frame that plays in full. The duration comes from the packet. A packet
    /// that is not valid counts as 20 ms.
    #[must_use]
    pub fn opus(packet: Vec<u8>) -> Self {
        let samples = opus_packet_samples(&packet).unwrap_or(960);
        Self::Opus {
            packet,
            samples,
            discard_start: 0,
            discard_end: 0,
        }
    }
}

/// An error of an audio source or of the codec.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum AudioError {
    /// Reading the input failed.
    #[error("cannot read the audio input: {0}")]
    Io(#[from] std::io::Error),
    /// The input has a format error.
    #[error("the audio input is not valid: {0}")]
    Format(String),
    /// The input has a format that this crate does not support.
    #[error("the audio format is not supported: {0}")]
    Unsupported(String),
    /// The Opus codec failed.
    #[error("the Opus codec failed: {0}")]
    Codec(String),
    /// An external program failed: it cannot start, or it exits with an error.
    #[error("the audio program failed: {0}")]
    Process(String),
}

/// A source of audio frames.
///
/// The send task calls [`AudioSource::next_frame`] once for each 20 ms frame, on the Tokio
/// runtime. Keep the call fast: read from memory or from a file, not from the network. A
/// source that reads from a slow input reads on another thread and gives
/// [`SourceFrame::Waiting`] when it has no audio yet.
pub trait AudioSource: Send + 'static {
    /// Gives the next frame. Gives `Ok(None)` at the end of the audio.
    ///
    /// # Errors
    ///
    /// Gives an error when the input fails. The track ends.
    fn next_frame(&mut self) -> Result<Option<SourceFrame>, AudioError>;

    /// Moves the source to a position from the start of the audio. The mixer calls it
    /// between two frames. Gives the position that the next frame starts at.
    ///
    /// A position after the end is not an error: the source then has no more frames.
    ///
    /// # Errors
    ///
    /// The default implementation gives [`AudioError::Unsupported`]. A source that can seek
    /// gives an error when the input fails.
    fn seek(&mut self, position: Duration) -> Result<Duration, AudioError> {
        let _ = position;
        Err(AudioError::Unsupported(
            "this audio source cannot seek".to_owned(),
        ))
    }
}

impl AudioSource for Box<dyn AudioSource> {
    fn next_frame(&mut self) -> Result<Option<SourceFrame>, AudioError> {
        (**self).next_frame()
    }

    fn seek(&mut self, position: Duration) -> Result<Duration, AudioError> {
        (**self).seek(position)
    }
}

/// Converts a duration to a number of samples for each channel at 48 kHz. Rounds to the
/// nearest sample.
#[must_use]
pub fn duration_to_samples(duration: Duration) -> u64 {
    let rate = u64::from(SAMPLE_RATE);
    let fraction = (u64::from(duration.subsec_nanos()) * rate + 500_000_000) / 1_000_000_000;
    duration
        .as_secs()
        .saturating_mul(rate)
        .saturating_add(fraction)
}

/// Converts a number of samples for each channel at 48 kHz to a duration. Rounds down.
#[must_use]
pub fn samples_to_duration(samples: u64) -> Duration {
    let rate = u64::from(SAMPLE_RATE);
    let nanos = (samples % rate) * 1_000_000_000 / rate;
    // The remainder is less than one second, so the value fits in u32.
    Duration::new(samples / rate, u32::try_from(nanos).unwrap_or(0))
}

/// Gives the duration of an Opus packet in samples for each channel at 48 kHz, from its
/// table-of-contents byte (RFC 6716, section 3.1). Gives `None` for a packet that is not
/// valid.
#[must_use]
pub fn opus_packet_samples(packet: &[u8]) -> Option<u32> {
    let toc = *packet.first()?;
    let config = toc >> 3;
    // Frame sizes in samples at 48 kHz.
    let frame_samples: u32 = match config {
        0..=11 => [480, 960, 1920, 2880][usize::from(config % 4)],
        12..=15 => [480, 960][usize::from(config % 2)],
        _ => [120, 240, 480, 960][usize::from(config % 4)],
    };
    let frames: u32 = match toc & 0x03 {
        0 => 1,
        1 | 2 => 2,
        _ => u32::from(*packet.get(1)? & 0x3F),
    };
    let samples = frame_samples * frames;
    // A packet holds at most 120 ms.
    (frames > 0 && samples <= 5760).then_some(samples)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packet_durations() {
        assert_eq!(opus_packet_samples(&SILENCE_FRAME), Some(960));
        // CELT 20 ms, one frame.
        assert_eq!(opus_packet_samples(&[0xFC, 1, 2]), Some(960));
        // SILK 60 ms, one frame.
        assert_eq!(opus_packet_samples(&[0x18]), Some(2880));
        // CELT 10 ms, two frames.
        assert_eq!(opus_packet_samples(&[0xF1]), Some(960));
        // CELT 20 ms, code 3 with 3 frames.
        assert_eq!(opus_packet_samples(&[0xFB, 0x03]), Some(2880));
        // Code 3 with 0 frames, or more than 120 ms.
        assert_eq!(opus_packet_samples(&[0xFB, 0x00]), None);
        assert_eq!(opus_packet_samples(&[0xFB, 0x07]), None);
        assert_eq!(opus_packet_samples(&[]), None);
    }

    #[test]
    fn sample_conversions() {
        assert_eq!(duration_to_samples(Duration::from_millis(20)), 960);
        assert_eq!(duration_to_samples(Duration::from_secs(3)), 144_000);
        assert_eq!(samples_to_duration(960), Duration::from_millis(20));
        assert_eq!(
            samples_to_duration(48_000 * 7 + 24_000),
            Duration::from_millis(7_500)
        );
        for samples in [0, 1, 312, 47_999, 1_000_003] {
            assert_eq!(duration_to_samples(samples_to_duration(samples)), samples);
        }
    }
}
