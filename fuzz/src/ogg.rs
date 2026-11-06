//! Ogg Opus files.

use std::io::Cursor;

use cordial_voice::audio::ogg::{OggPacketReader, ogg_crc};
use cordial_voice::{AudioSource, OggOpusSource};

use crate::reader::Reader;

/// The header type flags of an Ogg page that starts a stream.
const FIRST_PAGE: u8 = 2;

/// An Ogg file.
///
/// With selector bit 0 clear, the rest is the file. With the bit set, the body builds a file
/// with valid CRCs: an `OpusHead` page, an `OpusTags` page, and pages from the rest (a header
/// type byte, a lacing count, the lacing values, and the data).
pub fn ogg(data: &[u8]) {
    let mut reader = Reader::new(data);
    let selector = reader.byte();
    let file = if selector & 1 == 0 {
        reader.rest().to_vec()
    } else {
        build_file(reader)
    };

    read_packets(&file);
    read_source(file);
}

fn read_packets(file: &[u8]) {
    let mut packets = OggPacketReader::new(Cursor::new(file));
    while let Ok(Some(_)) = packets.next_packet() {}
}

fn read_source(file: Vec<u8>) {
    let Ok(mut source) = OggOpusSource::from_bytes(file) else {
        return;
    };
    let _ = source.head();
    while let Ok(Some(_)) = source.next_frame() {}
}

fn build_file(mut reader: Reader<'_>) -> Vec<u8> {
    let mut file = Vec::new();
    let mut head = b"OpusHead".to_vec();
    head.extend_from_slice(reader.take(11));
    file.extend(page(FIRST_PAGE, 0, &[lacing_value(head.len())], &head));
    let tags = [b"OpusTags".as_slice(), reader.chunk()].concat();
    file.extend(page(0, 1, &[lacing_value(tags.len())], &tags));

    let mut sequence = 2;
    while !reader.is_empty() {
        let header_type = reader.byte();
        let lacing = reader.chunk();
        let size: usize = lacing.iter().map(|value| usize::from(*value)).sum();
        let mut data = reader.take(size).to_vec();
        data.resize(size, 0);
        file.extend(page(header_type, sequence, lacing, &data));
        sequence += 1;
    }
    file
}

/// Gives the lacing value of a short packet: one value below 255 ends the packet.
fn lacing_value(len: usize) -> u8 {
    u8::try_from(len).unwrap_or(254).min(254)
}

/// Encodes one page of stream 7 with its CRC.
fn page(header_type: u8, sequence: u32, lacing: &[u8], data: &[u8]) -> Vec<u8> {
    let mut bytes = b"OggS".to_vec();
    bytes.push(0);
    bytes.push(header_type);
    bytes.extend_from_slice(&0_i64.to_le_bytes());
    bytes.extend_from_slice(&7_u32.to_le_bytes());
    bytes.extend_from_slice(&sequence.to_le_bytes());
    bytes.extend_from_slice(&[0; 4]);
    bytes.push(u8::try_from(lacing.len()).unwrap_or(u8::MAX));
    bytes.extend_from_slice(&lacing[..lacing.len().min(255)]);
    bytes.extend_from_slice(data);

    let crc = ogg_crc(&bytes);
    bytes[22..26].copy_from_slice(&crc.to_le_bytes());
    bytes
}
