//! The DAVE protocol frame format.
//!
//! A protocol frame is the media frame, then a footer:
//!
//! | Part | Size |
//! |---|---|
//! | Interleaved media frame (ciphertext and unencrypted ranges) | variable |
//! | Truncated AES-128-GCM tag | 8 bytes |
//! | Truncated synchronization nonce | ULEB128 |
//! | Unencrypted ranges (offset and size pairs) | ULEB128 pairs |
//! | Supplemental size | 1 byte |
//! | Magic marker | `FA FA` |
//!
//! The supplemental size counts all bytes of the footer. The parser follows
//! `InboundFrameProcessor::ParseFrame` in libdave `frame_processors.cpp`.

/// The size of the truncated AES-GCM tag (libdave `common.h`, `kAesGcm128TruncatedTagBytes`).
pub(crate) const TAG_SIZE: usize = 8;

/// The magic marker at the end of each protocol frame (libdave `common.h`, `kMarkerBytes`).
pub(crate) const MAGIC_MARKER: [u8; 2] = [0xFA, 0xFA];

/// The Opus silence frame that the SFU makes. It passes through without encryption.
pub(crate) const OPUS_SILENCE_FRAME: [u8; 3] = [0xF8, 0xFF, 0xFE];

/// The smallest footer: the tag, the supplemental size byte, and the marker.
const MIN_SUPPLEMENTAL_SIZE: usize = TAG_SIZE + 1 + MAGIC_MARKER.len();

/// Gives the number of bytes of the ULEB128 encoding of `value`.
pub(crate) fn uleb128_size(mut value: u64) -> usize {
    let mut size = 1;
    while value >= 0x80 {
        size += 1;
        value >>= 7;
    }
    size
}

/// Appends the ULEB128 encoding of `value` to `out`.
pub(crate) fn write_uleb128(mut value: u64, out: &mut Vec<u8>) {
    while value >= 0x80 {
        // The mask keeps the low 7 bits, so the cast does not lose data.
        out.push(0x80 | (value & 0x7F) as u8);
        value >>= 7;
    }
    // The value is now below 0x80.
    out.push((value & 0x7F) as u8);
}

/// Reads a ULEB128 value from the start of `input`.
///
/// Gives the value and the number of bytes read. Gives `None` when the input ends before the
/// last byte or when the value does not fit in 64 bits. The limits follow libdave `leb128.cpp`.
pub(crate) fn read_uleb128(input: &[u8]) -> Option<(u64, usize)> {
    let mut value: u64 = 0;
    let mut shift = 0;
    for (index, &byte) in input.iter().enumerate() {
        if shift == 63 {
            // The tenth byte can only hold the highest bit.
            if byte > 1 {
                return None;
            }
            value |= u64::from(byte) << shift;
            return Some((value, index + 1));
        }

        value |= u64::from(byte & 0x7F) << shift;
        if byte & 0x80 == 0 {
            return Some((value, index + 1));
        }
        shift += 7;
    }
    None
}

/// A range of the media frame that is not encrypted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct UnencryptedRange {
    /// The offset in the interleaved media frame.
    pub(crate) offset: usize,
    /// The number of bytes.
    pub(crate) size: usize,
}

/// A parsed protocol frame.
#[derive(Debug)]
pub(crate) struct ProtocolFrame<'a> {
    /// The interleaved media frame, without the footer.
    pub(crate) media: &'a [u8],
    /// The truncated tag.
    pub(crate) tag: [u8; TAG_SIZE],
    /// The truncated synchronization nonce.
    pub(crate) nonce: u32,
    /// The unencrypted ranges, in ascending order.
    pub(crate) ranges: Vec<UnencryptedRange>,
}

impl ProtocolFrame<'_> {
    /// Gives the authenticated data (all unencrypted ranges joined) and the ciphertext (all
    /// other bytes joined).
    pub(crate) fn split(&self) -> (Vec<u8>, Vec<u8>) {
        let mut authenticated = Vec::new();
        let mut ciphertext = Vec::with_capacity(self.media.len());
        let mut position = 0;
        for range in &self.ranges {
            ciphertext.extend_from_slice(&self.media[position..range.offset]);
            authenticated.extend_from_slice(&self.media[range.offset..range.offset + range.size]);
            position = range.offset + range.size;
        }
        ciphertext.extend_from_slice(&self.media[position..]);
        (authenticated, ciphertext)
    }

    /// Puts the plaintext back between the unencrypted ranges.
    pub(crate) fn reconstruct(&self, plaintext: &[u8]) -> Vec<u8> {
        let mut frame = Vec::with_capacity(self.media.len());
        let mut position = 0;
        let mut plaintext_position = 0;
        for range in &self.ranges {
            let encrypted_size = range.offset - position;
            frame.extend_from_slice(
                &plaintext[plaintext_position..plaintext_position + encrypted_size],
            );
            frame.extend_from_slice(&self.media[range.offset..range.offset + range.size]);
            plaintext_position += encrypted_size;
            position = range.offset + range.size;
        }
        frame.extend_from_slice(&plaintext[plaintext_position..]);
        frame
    }
}

/// Parses a protocol frame. Gives `None` when the frame does not pass the protocol frame
/// check (the frame is not a DAVE frame or it is malformed).
pub(crate) fn parse(frame: &[u8]) -> Option<ProtocolFrame<'_>> {
    if frame.len() < MIN_SUPPLEMENTAL_SIZE {
        return None;
    }
    let (body, marker) = frame.split_at(frame.len() - MAGIC_MARKER.len());
    if marker != MAGIC_MARKER {
        return None;
    }

    let (body, supplemental_size) = body.split_at(body.len() - 1);
    let supplemental_size = usize::from(supplemental_size[0]);
    if supplemental_size < MIN_SUPPLEMENTAL_SIZE || supplemental_size > frame.len() {
        return None;
    }

    // The footer without the supplemental size byte and the marker.
    let footer_start = frame.len() - supplemental_size;
    let media = &frame[..footer_start];
    let footer = &body[footer_start..];
    let (tag, rest) = footer.split_at(TAG_SIZE);

    let (nonce, nonce_size) = read_uleb128(rest)?;
    let nonce = u32::try_from(nonce).ok()?;
    let ranges = parse_ranges(&rest[nonce_size..], media.len())?;

    let mut tag_bytes = [0; TAG_SIZE];
    tag_bytes.copy_from_slice(tag);
    Some(ProtocolFrame {
        media,
        tag: tag_bytes,
        nonce,
        ranges,
    })
}

/// Parses and checks the unencrypted ranges. The ranges must be in ascending order, must not
/// overlap, and must stay in the media frame.
fn parse_ranges(mut input: &[u8], media_size: usize) -> Option<Vec<UnencryptedRange>> {
    let mut ranges = Vec::new();
    let mut minimum_offset = 0;
    while !input.is_empty() {
        let (offset, offset_size) = read_uleb128(input)?;
        input = &input[offset_size..];
        let (size, size_size) = read_uleb128(input)?;
        input = &input[size_size..];

        let offset = usize::try_from(offset).ok()?;
        let size = usize::try_from(size).ok()?;
        let end = offset.checked_add(size)?;
        if offset < minimum_offset || end > media_size {
            return None;
        }

        minimum_offset = end;
        ranges.push(UnencryptedRange { offset, size });
    }
    Some(ranges)
}

/// Builds a protocol frame for a fully encrypted frame (Opus). The frame has no unencrypted
/// ranges.
pub(crate) fn encode_full(ciphertext: &[u8], tag: &[u8; TAG_SIZE], nonce: u32) -> Vec<u8> {
    let nonce_size = uleb128_size(u64::from(nonce));
    let supplemental_size = MIN_SUPPLEMENTAL_SIZE + nonce_size;
    let mut frame = Vec::with_capacity(ciphertext.len() + supplemental_size);
    frame.extend_from_slice(ciphertext);
    frame.extend_from_slice(tag);
    write_uleb128(u64::from(nonce), &mut frame);

    // The nonce has at most 5 bytes, so the size is at most 16.
    frame.push(u8::try_from(supplemental_size).unwrap_or(u8::MAX));
    frame.extend_from_slice(&MAGIC_MARKER);
    frame
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode(value: u64) -> Vec<u8> {
        let mut out = Vec::new();
        write_uleb128(value, &mut out);
        out
    }

    #[test]
    fn uleb128_known_values() {
        assert_eq!(encode(0), [0x00]);
        assert_eq!(encode(1), [0x01]);
        assert_eq!(encode(0x7F), [0x7F]);
        assert_eq!(encode(0x80), [0x80, 0x01]);
        assert_eq!(encode(300), [0xAC, 0x02]);
        assert_eq!(encode(0x3FFF), [0xFF, 0x7F]);
        assert_eq!(encode(0x4000), [0x80, 0x80, 0x01]);
        assert_eq!(encode(u64::from(u32::MAX)), [0xFF, 0xFF, 0xFF, 0xFF, 0x0F]);
        assert_eq!(
            encode(u64::MAX),
            [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x01]
        );
    }

    #[test]
    fn uleb128_round_trip_and_size() {
        let values = [
            0,
            1,
            127,
            128,
            255,
            16_383,
            16_384,
            0x00FF_FFFF,
            0x0100_0000,
            u64::from(u32::MAX),
            u64::MAX - 1,
            u64::MAX,
        ];
        for value in values {
            let bytes = encode(value);
            assert_eq!(bytes.len(), uleb128_size(value), "size of {value}");
            assert_eq!(
                read_uleb128(&bytes),
                Some((value, bytes.len())),
                "value {value}"
            );
        }
    }

    #[test]
    fn uleb128_rejects_bad_input() {
        // No final byte.
        assert_eq!(read_uleb128(&[]), None);
        assert_eq!(read_uleb128(&[0x80]), None);
        assert_eq!(read_uleb128(&[0xFF, 0xFF]), None);
        // The tenth byte holds more than one bit.
        assert_eq!(
            read_uleb128(&[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x02]),
            None
        );
        // Eleven bytes.
        assert_eq!(
            read_uleb128(&[
                0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x00
            ]),
            None
        );
        // A padded encoding is accepted, as in libdave.
        assert_eq!(read_uleb128(&[0x81, 0x00]), Some((1, 2)));
        // Bytes after the value stay unread.
        assert_eq!(read_uleb128(&[0x05, 0xAA]), Some((5, 1)));
    }

    #[test]
    fn encode_and_parse_round_trip() {
        let ciphertext = [1, 2, 3, 4, 5];
        let tag = [9; TAG_SIZE];
        for nonce in [0, 1, 127, 128, 0x00FF_FFFF, 0x0100_0000, u32::MAX] {
            let frame = encode_full(&ciphertext, &tag, nonce);
            let supplemental = TAG_SIZE + uleb128_size(u64::from(nonce)) + 1 + 2;
            assert_eq!(frame.len(), ciphertext.len() + supplemental);
            assert_eq!(usize::from(frame[frame.len() - 3]), supplemental);
            assert_eq!(&frame[frame.len() - 2..], &MAGIC_MARKER);

            let parsed = parse(&frame).expect("protocol frame");
            assert_eq!(parsed.media, &ciphertext);
            assert_eq!(parsed.tag, tag);
            assert_eq!(parsed.nonce, nonce);
            assert!(parsed.ranges.is_empty());
        }
    }

    #[test]
    fn empty_media_frame() {
        let frame = encode_full(&[], &[0; TAG_SIZE], 1);
        let parsed = parse(&frame).expect("protocol frame");
        assert!(parsed.media.is_empty());
    }

    #[test]
    fn parse_rejects_non_protocol_frames() {
        assert!(parse(&OPUS_SILENCE_FRAME).is_none());
        assert!(parse(&[0; 32]).is_none());

        let frame = encode_full(&[1, 2, 3], &[0; TAG_SIZE], 7);

        // Wrong marker.
        let mut bad = frame.clone();
        let last = bad.len() - 1;
        bad[last] = 0xFB;
        assert!(parse(&bad).is_none());

        // Supplemental size too small or too large.
        let size_index = frame.len() - 3;
        let mut bad = frame.clone();
        bad[size_index] = 10;
        assert!(parse(&bad).is_none());
        let mut bad = frame.clone();
        bad[size_index] = 200;
        assert!(parse(&bad).is_none());

        // Every truncation fails cleanly.
        for length in 0..frame.len() {
            let _ = parse(&frame[..length]);
        }
    }

    #[test]
    fn parse_ranges_and_reconstruct() {
        // Media: 2 unencrypted bytes, 3 encrypted bytes, 1 unencrypted byte, 2 encrypted bytes.
        let media = [0xA0, 0xA1, 1, 2, 3, 0xB0, 4, 5];
        let mut frame = media.to_vec();
        frame.extend_from_slice(&[7; TAG_SIZE]);
        let mut footer = Vec::new();
        write_uleb128(42, &mut footer);
        for value in [0, 2, 5, 1] {
            write_uleb128(value, &mut footer);
        }
        frame.extend_from_slice(&footer);
        frame.push(u8::try_from(TAG_SIZE + footer.len() + 3).unwrap());
        frame.extend_from_slice(&MAGIC_MARKER);

        let parsed = parse(&frame).expect("protocol frame");
        assert_eq!(parsed.nonce, 42);
        assert_eq!(
            parsed.ranges,
            [
                UnencryptedRange { offset: 0, size: 2 },
                UnencryptedRange { offset: 5, size: 1 }
            ]
        );
        let (authenticated, ciphertext) = parsed.split();
        assert_eq!(authenticated, [0xA0, 0xA1, 0xB0]);
        assert_eq!(ciphertext, [1, 2, 3, 4, 5]);
        assert_eq!(parsed.reconstruct(&ciphertext), media);
    }

    #[test]
    fn parse_rejects_bad_ranges() {
        let build = |ranges: &[u64]| {
            let mut frame = vec![0; 8];
            frame.extend_from_slice(&[0; TAG_SIZE]);
            let mut footer = Vec::new();
            write_uleb128(1, &mut footer);
            for &value in ranges {
                write_uleb128(value, &mut footer);
            }
            frame.extend_from_slice(&footer);
            frame.push(u8::try_from(TAG_SIZE + footer.len() + 3).unwrap());
            frame.extend_from_slice(&MAGIC_MARKER);
            frame
        };

        assert!(parse(&build(&[0, 8])).is_some());
        // Past the end of the media.
        assert!(parse(&build(&[4, 5])).is_none());
        // Overlap.
        assert!(parse(&build(&[0, 4, 3, 1])).is_none());
        // Wrong order.
        assert!(parse(&build(&[5, 1, 0, 1])).is_none());
        // A missing size.
        assert!(parse(&build(&[0])).is_none());
        // An overflow.
        assert!(parse(&build(&[1, u64::MAX])).is_none());
    }
}
