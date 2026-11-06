//! The Opus encoder and decoder (libopus). Needs the `opus` feature.

use std::fmt;

use super::{AudioError, CHANNELS, FRAME_LEN, FRAME_SAMPLES, SAMPLE_RATE};

/// The largest Opus packet that the encoder makes.
const MAX_PACKET_LEN: usize = 1275;
/// The largest frame that a decoder gives: 120 ms.
const MAX_DECODED_LEN: usize = 5760 * CHANNELS;

fn codec_error(error: &opus::Error) -> AudioError {
    AudioError::Codec(error.to_string())
}

/// An Opus encoder for 48 kHz stereo audio in 20 ms frames.
pub struct OpusEncoder {
    inner: opus::Encoder,
}

impl fmt::Debug for OpusEncoder {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OpusEncoder")
            .finish_non_exhaustive()
    }
}

impl OpusEncoder {
    /// The default bitrate in bits per second.
    pub const DEFAULT_BITRATE: i32 = 64_000;

    /// Makes an encoder for music and voice at the default bitrate.
    ///
    /// # Errors
    ///
    /// Gives an error when libopus cannot make the encoder.
    pub fn new() -> Result<Self, AudioError> {
        let mut inner = opus::Encoder::new(
            SAMPLE_RATE,
            opus::Channels::Stereo,
            opus::Application::Audio,
        )
        .map_err(|error| codec_error(&error))?;
        inner
            .set_bitrate(opus::Bitrate::Bits(Self::DEFAULT_BITRATE))
            .map_err(|error| codec_error(&error))?;
        Ok(Self { inner })
    }

    /// Sets the bitrate in bits per second.
    ///
    /// # Errors
    ///
    /// Gives an error when libopus does not accept the bitrate.
    pub fn set_bitrate(&mut self, bits_per_second: i32) -> Result<(), AudioError> {
        self.inner
            .set_bitrate(opus::Bitrate::Bits(bits_per_second))
            .map_err(|error| codec_error(&error))
    }

    /// Encodes one 20 ms frame of [`FRAME_LEN`] interleaved samples.
    ///
    /// # Errors
    ///
    /// Gives an error when the frame has a wrong size or libopus fails.
    pub fn encode(&mut self, pcm: &[f32]) -> Result<Vec<u8>, AudioError> {
        if pcm.len() != FRAME_LEN {
            return Err(AudioError::Codec(format!(
                "a frame has {} samples, not {FRAME_LEN}",
                pcm.len()
            )));
        }
        self.inner
            .encode_vec_float(pcm, MAX_PACKET_LEN)
            .map_err(|error| codec_error(&error))
    }
}

/// An Opus decoder that gives 48 kHz stereo audio.
pub struct OpusDecoder {
    inner: opus::Decoder,
}

impl fmt::Debug for OpusDecoder {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OpusDecoder")
            .finish_non_exhaustive()
    }
}

impl OpusDecoder {
    /// Makes a decoder.
    ///
    /// # Errors
    ///
    /// Gives an error when libopus cannot make the decoder.
    pub fn new() -> Result<Self, AudioError> {
        let inner = opus::Decoder::new(SAMPLE_RATE, opus::Channels::Stereo)
            .map_err(|error| codec_error(&error))?;
        Ok(Self { inner })
    }

    /// Decodes a packet to interleaved `i16` samples.
    ///
    /// # Errors
    ///
    /// Gives an error when the packet is not valid.
    pub fn decode(&mut self, packet: &[u8]) -> Result<Vec<i16>, AudioError> {
        let mut output = vec![0_i16; MAX_DECODED_LEN];
        let samples = self
            .inner
            .decode(packet, &mut output, false)
            .map_err(|error| codec_error(&error))?;
        output.truncate(samples * CHANNELS);
        Ok(output)
    }

    /// Decodes a packet to interleaved `f32` samples.
    ///
    /// # Errors
    ///
    /// Gives an error when the packet is not valid.
    pub fn decode_float(&mut self, packet: &[u8]) -> Result<Vec<f32>, AudioError> {
        let mut output = vec![0_f32; MAX_DECODED_LEN];
        let samples = self
            .inner
            .decode_float(packet, &mut output, false)
            .map_err(|error| codec_error(&error))?;
        output.truncate(samples * CHANNELS);
        Ok(output)
    }

    /// Makes 20 ms of audio for a lost packet with packet loss concealment.
    ///
    /// # Errors
    ///
    /// Gives an error when libopus fails.
    pub fn conceal(&mut self) -> Result<Vec<i16>, AudioError> {
        let mut output = vec![0_i16; FRAME_LEN];
        let samples = self
            .inner
            .decode(&[], &mut output, false)
            .map_err(|error| codec_error(&error))?;
        output.truncate(samples.min(FRAME_SAMPLES) * CHANNELS);
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_then_decode() {
        let mut encoder = OpusEncoder::new().expect("encoder");
        let mut decoder = OpusDecoder::new().expect("decoder");
        let tone: Vec<f32> = (0..FRAME_LEN)
            .map(|index| {
                #[allow(clippy::cast_precision_loss)]
                let time = (index / 2) as f32 / 48_000.0;
                (time * 440.0 * std::f32::consts::TAU).sin() * 0.5
            })
            .collect();

        for _ in 0..3 {
            let packet = encoder.encode(&tone).expect("encode");
            assert!(!packet.is_empty());
            assert_eq!(super::super::opus_packet_samples(&packet), Some(960));
            let pcm = decoder.decode(&packet).expect("decode");
            assert_eq!(pcm.len(), FRAME_LEN);
        }
        assert_eq!(decoder.conceal().expect("conceal").len(), FRAME_LEN);
        assert_eq!(
            decoder
                .decode(&super::super::SILENCE_FRAME)
                .expect("decode")
                .len(),
            FRAME_LEN
        );
        assert!(encoder.encode(&tone[..10]).is_err());
    }
}
