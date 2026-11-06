//! Transport compression of the gateway: `zlib-stream` and `zstd-stream`.
//!
//! Each connection has one decompression context. The context is new for each connection.

use std::fmt;

/// The transport compression of a connection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Compression {
    /// No transport compression.
    None,
    /// `zlib-stream` (feature `zlib`).
    Zlib,
    /// `zstd-stream` (feature `zstd`).
    Zstd,
}

/// The default is `zlib-stream` when the `zlib` feature is on, else no compression.
impl Default for Compression {
    fn default() -> Self {
        if cfg!(feature = "zlib") {
            Self::Zlib
        } else {
            Self::None
        }
    }
}

impl Compression {
    /// Gives the value of the `compress` query parameter.
    #[must_use]
    pub const fn query_value(self) -> Option<&'static str> {
        match self {
            Self::None => None,
            Self::Zlib => Some("zlib-stream"),
            Self::Zstd => Some("zstd-stream"),
        }
    }
}

/// An error of decompression.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum DecompressError {
    /// The compressed data is not valid.
    #[error("the compressed data is not valid: {0}")]
    Corrupt(String),
    /// The decompressed data is not valid UTF-8.
    #[error("the decompressed data is not valid UTF-8")]
    Utf8,
    /// The compression is not compiled in.
    #[error("the compression {0:?} is not compiled in (turn on its feature)")]
    Unsupported(Compression),
}

/// The decompression context of one connection.
pub(crate) enum Decompressor {
    None,
    #[cfg(feature = "zlib")]
    Zlib(ZlibStream),
    #[cfg(feature = "zstd")]
    Zstd(ZstdStream),
}

impl fmt::Debug for Decompressor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::None => "None",
            #[cfg(feature = "zlib")]
            Self::Zlib(_) => "Zlib",
            #[cfg(feature = "zstd")]
            Self::Zstd(_) => "Zstd",
        };
        formatter.write_str(name)
    }
}

impl Decompressor {
    /// Makes a new context for a connection.
    pub(crate) fn new(compression: Compression) -> Result<Self, DecompressError> {
        match compression {
            Compression::None => Ok(Self::None),
            #[cfg(feature = "zlib")]
            Compression::Zlib => Ok(Self::Zlib(ZlibStream::new())),
            #[cfg(feature = "zstd")]
            Compression::Zstd => ZstdStream::new().map(Self::Zstd),
            #[allow(unreachable_patterns)]
            other => Err(DecompressError::Unsupported(other)),
        }
    }

    /// Takes one binary frame. Gives the text of a complete gateway message, or `None` when
    /// more frames are necessary.
    pub(crate) fn push(&mut self, frame: &[u8]) -> Result<Option<String>, DecompressError> {
        match self {
            Self::None => String::from_utf8(frame.to_vec())
                .map(Some)
                .map_err(|_| DecompressError::Utf8),
            #[cfg(feature = "zlib")]
            Self::Zlib(stream) => stream.push(frame),
            #[cfg(feature = "zstd")]
            Self::Zstd(stream) => stream.push(frame).map(Some),
        }
    }
}

/// The `zlib-stream` context: one inflate stream for the whole connection.
#[cfg(feature = "zlib")]
pub(crate) struct ZlibStream {
    inflater: flate2::Decompress,
    buffer: Vec<u8>,
    output: Vec<u8>,
}

#[cfg(feature = "zlib")]
impl ZlibStream {
    const SUFFIX: [u8; 4] = [0x00, 0x00, 0xFF, 0xFF];

    fn new() -> Self {
        Self {
            inflater: flate2::Decompress::new(true),
            buffer: Vec::new(),
            output: Vec::with_capacity(32 * 1024),
        }
    }

    fn push(&mut self, frame: &[u8]) -> Result<Option<String>, DecompressError> {
        self.buffer.extend_from_slice(frame);
        if !self.buffer.ends_with(&Self::SUFFIX) {
            return Ok(None);
        }

        self.output.clear();
        let mut offset = 0;
        loop {
            if self.output.len() == self.output.capacity() {
                self.output.reserve(self.output.capacity().max(4096));
            }
            let before_in = self.inflater.total_in();
            let status = self
                .inflater
                .decompress_vec(
                    &self.buffer[offset..],
                    &mut self.output,
                    flate2::FlushDecompress::Sync,
                )
                .map_err(|error| DecompressError::Corrupt(error.to_string()))?;
            let consumed =
                usize::try_from(self.inflater.total_in() - before_in).unwrap_or(usize::MAX);
            offset += consumed;

            let input_done = offset >= self.buffer.len();
            let output_full = self.output.len() == self.output.capacity();
            if status == flate2::Status::StreamEnd || (input_done && !output_full) {
                break;
            }
            if consumed == 0 && !output_full {
                return Err(DecompressError::Corrupt(
                    "the inflate stream made no progress".to_owned(),
                ));
            }
        }
        self.buffer.clear();

        String::from_utf8(std::mem::take(&mut self.output))
            .map(Some)
            .map_err(|_| DecompressError::Utf8)
    }
}

/// The `zstd-stream` context: one decompression stream for the whole connection.
#[cfg(feature = "zstd")]
pub(crate) struct ZstdStream {
    context: zstd::stream::raw::Decoder<'static>,
}

#[cfg(feature = "zstd")]
impl ZstdStream {
    fn new() -> Result<Self, DecompressError> {
        zstd::stream::raw::Decoder::new()
            .map(|context| Self { context })
            .map_err(|error| DecompressError::Corrupt(error.to_string()))
    }

    fn push(&mut self, frame: &[u8]) -> Result<String, DecompressError> {
        use zstd::stream::raw::{InBuffer, Operation, OutBuffer};

        let mut input = InBuffer::around(frame);
        let mut output = Vec::with_capacity(frame.len() * 4 + 1024);
        let mut chunk = vec![0_u8; 32 * 1024];
        loop {
            let mut out = OutBuffer::around(&mut chunk[..]);
            self.context
                .run(&mut input, &mut out)
                .map_err(|error| DecompressError::Corrupt(error.to_string()))?;
            let written = out.pos();
            output.extend_from_slice(&chunk[..written]);
            // Each frame is one complete message, but it does not end a zstd frame.
            if input.pos() >= frame.len() && written < chunk.len() {
                break;
            }
        }
        String::from_utf8(output).map_err(|_| DecompressError::Utf8)
    }
}

#[cfg(all(test, feature = "zlib"))]
mod tests {
    use std::io::Write;

    use super::*;

    /// Compresses messages like the gateway: one zlib stream, a sync flush after each message.
    fn compress_stream(messages: &[&str]) -> Vec<Vec<u8>> {
        let mut compressor = flate2::Compress::new(flate2::Compression::default(), true);
        messages
            .iter()
            .map(|message| {
                let mut output = Vec::with_capacity(message.len() + 64);
                compressor
                    .compress_vec(message.as_bytes(), &mut output, flate2::FlushCompress::Sync)
                    .unwrap();
                output
            })
            .collect()
    }

    #[test]
    fn inflates_a_stream_of_messages() {
        let messages = [
            r#"{"op":10,"d":{"heartbeat_interval":41250}}"#,
            r#"{"op":11}"#,
        ];
        let frames = compress_stream(&messages);
        let mut decompressor = Decompressor::new(Compression::Zlib).unwrap();

        for (frame, expected) in frames.iter().zip(messages) {
            assert!(frame.ends_with(&ZlibStream::SUFFIX));
            assert_eq!(decompressor.push(frame).unwrap().as_deref(), Some(expected));
        }
    }

    #[test]
    fn waits_for_the_suffix() {
        let frames = compress_stream(&[r#"{"op":11}"#]);
        let (first, second) = frames[0].split_at(3);
        let mut decompressor = Decompressor::new(Compression::Zlib).unwrap();

        assert_eq!(decompressor.push(first).unwrap(), None);
        assert_eq!(
            decompressor.push(second).unwrap().as_deref(),
            Some(r#"{"op":11}"#)
        );
    }

    #[test]
    fn inflates_large_messages() {
        let large = format!(r#"{{"op":0,"d":"{}"}}"#, "x".repeat(500_000));
        let frames = compress_stream(&[&large]);
        let mut decompressor = Decompressor::new(Compression::Zlib).unwrap();

        assert_eq!(
            decompressor.push(&frames[0]).unwrap().unwrap().len(),
            large.len()
        );
    }

    #[test]
    fn refuses_corrupt_data() {
        let mut decompressor = Decompressor::new(Compression::Zlib).unwrap();
        let mut garbage = Vec::new();
        garbage
            .write_all(&[1, 2, 3, 4, 0x00, 0x00, 0xFF, 0xFF])
            .unwrap();

        assert!(decompressor.push(&garbage).is_err());
    }
}
