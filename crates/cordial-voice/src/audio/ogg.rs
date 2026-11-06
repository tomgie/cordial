//! Ogg Opus files (RFC 3533 and RFC 7845): a reader for passthrough and a small writer.
//!
//! The reader gives the Opus packets of a file without a new encode. The writer stores Opus
//! packets in an Ogg file, for example to record received audio.

use std::collections::VecDeque;
use std::fmt;
use std::fs::File;
use std::io::{BufReader, ErrorKind, Read, Write};
use std::path::Path;

use super::{AudioError, AudioSource, FRAME_SAMPLES, SourceFrame, opus_packet_samples};

const CAPTURE_PATTERN: &[u8; 4] = b"OggS";
const HEADER_LEN: usize = 27;
const FLAG_CONTINUED: u8 = 0x01;
const FLAG_FIRST: u8 = 0x02;
const FLAG_LAST: u8 = 0x04;

/// The CRC-32 of Ogg: polynomial 0x04C11DB7, initial value 0, no reflection, no final XOR.
const CRC_TABLE: [u32; 256] = crc_table();

const fn crc_table() -> [u32; 256] {
    let mut table = [0_u32; 256];
    let mut index = 0;
    while index < 256 {
        // The index is less than 256: the cast keeps all bits.
        #[allow(clippy::cast_possible_truncation)]
        let mut value = (index as u32) << 24;
        let mut bit = 0;
        while bit < 8 {
            value = if value & 0x8000_0000 != 0 {
                (value << 1) ^ 0x04C1_1DB7
            } else {
                value << 1
            };
            bit += 1;
        }
        table[index] = value;
        index += 1;
    }
    table
}

/// Computes the Ogg CRC-32 of bytes.
#[must_use]
pub fn ogg_crc(bytes: &[u8]) -> u32 {
    bytes.iter().fold(0_u32, |crc, byte| {
        let index = usize::from(crc.to_be_bytes()[0] ^ byte);
        (crc << 8) ^ CRC_TABLE[index]
    })
}

/// One Ogg page.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct OggPage {
    /// The header type flags: 1 continued packet, 2 first page, 4 last page.
    pub header_type: u8,
    /// The granule position: for Opus, the sample count at 48 kHz at the end of the page.
    pub granule_position: i64,
    /// The serial number of the logical stream.
    pub serial: u32,
    /// The sequence number of the page.
    pub sequence: u32,
    /// The lacing values.
    pub lacing: Vec<u8>,
    /// The packet data.
    pub data: Vec<u8>,
}

impl OggPage {
    /// Tells if the page continues a packet of the previous page.
    #[must_use]
    pub fn is_continued(&self) -> bool {
        self.header_type & FLAG_CONTINUED != 0
    }

    /// Tells if the page is the last page of its stream.
    #[must_use]
    pub fn is_last(&self) -> bool {
        self.header_type & FLAG_LAST != 0
    }

    /// Encodes the page with its CRC.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(HEADER_LEN + self.lacing.len() + self.data.len());
        bytes.extend_from_slice(CAPTURE_PATTERN);
        bytes.push(0);
        bytes.push(self.header_type);
        bytes.extend_from_slice(&self.granule_position.to_le_bytes());
        bytes.extend_from_slice(&self.serial.to_le_bytes());
        bytes.extend_from_slice(&self.sequence.to_le_bytes());
        bytes.extend_from_slice(&[0; 4]);
        // A page has at most 255 lacing values; the writer keeps to that limit.
        #[allow(clippy::cast_possible_truncation)]
        bytes.push(self.lacing.len() as u8);
        bytes.extend_from_slice(&self.lacing);
        bytes.extend_from_slice(&self.data);
        let crc = ogg_crc(&bytes);
        bytes[22..26].copy_from_slice(&crc.to_le_bytes());
        bytes
    }
}

/// Reads Ogg pages from a reader.
pub struct OggPageReader<R> {
    reader: R,
}

impl<R> fmt::Debug for OggPageReader<R> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OggPageReader")
            .finish_non_exhaustive()
    }
}

impl<R: Read> OggPageReader<R> {
    /// Makes a page reader.
    pub fn new(reader: R) -> Self {
        Self { reader }
    }

    /// Reads the next page. Gives `Ok(None)` at the end of the input.
    ///
    /// # Errors
    ///
    /// Gives an error when the input ends inside a page, the page is not valid, or its CRC
    /// is not correct.
    pub fn next_page(&mut self) -> Result<Option<OggPage>, AudioError> {
        let mut header = [0_u8; HEADER_LEN];
        if !read_full(&mut self.reader, &mut header)? {
            return Ok(None);
        }
        if &header[..4] != CAPTURE_PATTERN {
            return Err(AudioError::Format(
                "an Ogg page has no capture pattern".to_owned(),
            ));
        }
        if header[4] != 0 {
            return Err(AudioError::Format(format!(
                "Ogg version {} is not supported",
                header[4]
            )));
        }

        let mut lacing = vec![0_u8; usize::from(header[26])];
        if !read_full(&mut self.reader, &mut lacing)? && !lacing.is_empty() {
            return Err(truncated());
        }
        let data_len: usize = lacing.iter().map(|value| usize::from(*value)).sum();
        let mut data = vec![0_u8; data_len];
        if !read_full(&mut self.reader, &mut data)? && data_len > 0 {
            return Err(truncated());
        }

        let expected_crc = u32::from_le_bytes([header[22], header[23], header[24], header[25]]);
        header[22..26].fill(0);
        let mut crc_input = Vec::with_capacity(HEADER_LEN + lacing.len() + data.len());
        crc_input.extend_from_slice(&header);
        crc_input.extend_from_slice(&lacing);
        crc_input.extend_from_slice(&data);
        if ogg_crc(&crc_input) != expected_crc {
            return Err(AudioError::Format(
                "the CRC of an Ogg page is not correct".to_owned(),
            ));
        }

        Ok(Some(OggPage {
            header_type: header[5],
            granule_position: i64::from_le_bytes(header[6..14].try_into().unwrap_or_default()),
            serial: u32::from_le_bytes([header[14], header[15], header[16], header[17]]),
            sequence: u32::from_le_bytes([header[18], header[19], header[20], header[21]]),
            lacing,
            data,
        }))
    }
}

/// Fills `buffer`. Gives `false` when the input ends before the first byte.
fn read_full(reader: &mut impl Read, buffer: &mut [u8]) -> Result<bool, AudioError> {
    let mut filled = 0;
    while filled < buffer.len() {
        match reader.read(&mut buffer[filled..]) {
            Ok(0) if filled == 0 => return Ok(false),
            Ok(0) => return Err(truncated()),
            Ok(count) => filled += count,
            Err(error) if error.kind() == ErrorKind::Interrupted => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(true)
}

fn truncated() -> AudioError {
    AudioError::Format("the Ogg input ends inside a page".to_owned())
}

/// Reads the packets of the first logical stream of an Ogg file.
pub struct OggPacketReader<R> {
    pages: OggPageReader<R>,
    serial: Option<u32>,
    partial: Vec<u8>,
    packets: VecDeque<Vec<u8>>,
    ended: bool,
}

impl<R> fmt::Debug for OggPacketReader<R> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OggPacketReader")
            .field("serial", &self.serial)
            .field("ended", &self.ended)
            .finish_non_exhaustive()
    }
}

impl<R: Read> OggPacketReader<R> {
    /// Makes a packet reader.
    pub fn new(reader: R) -> Self {
        Self {
            pages: OggPageReader::new(reader),
            serial: None,
            partial: Vec::new(),
            packets: VecDeque::new(),
            ended: false,
        }
    }

    /// Reads the next packet. Gives `Ok(None)` at the end of the stream.
    ///
    /// # Errors
    ///
    /// Gives an error when a page is not valid.
    pub fn next_packet(&mut self) -> Result<Option<Vec<u8>>, AudioError> {
        loop {
            if let Some(packet) = self.packets.pop_front() {
                return Ok(Some(packet));
            }
            if self.ended {
                return Ok(None);
            }
            let Some(page) = self.pages.next_page()? else {
                self.ended = true;
                continue;
            };
            self.add_page(&page);
        }
    }

    fn add_page(&mut self, page: &OggPage) {
        let serial = *self.serial.get_or_insert(page.serial);
        if page.serial != serial {
            return;
        }
        if !page.is_continued() && !self.partial.is_empty() {
            tracing::debug!("an Ogg packet has no end; dropped");
            self.partial.clear();
        }

        let mut offset = 0;
        for value in &page.lacing {
            let end = offset + usize::from(*value);
            self.partial.extend_from_slice(&page.data[offset..end]);
            offset = end;
            if *value < 255 {
                self.packets.push_back(std::mem::take(&mut self.partial));
            }
        }
        if page.is_last() {
            self.ended = true;
        }
    }
}

/// The Opus identification header of an Ogg Opus file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct OpusHead {
    /// The version (1).
    pub version: u8,
    /// The number of channels.
    pub channels: u8,
    /// The number of samples to skip at the start.
    pub pre_skip: u16,
    /// The sample rate of the original input (for information only).
    pub input_sample_rate: u32,
    /// The output gain in Q7.8 dB.
    pub output_gain: i16,
    /// The channel mapping family.
    pub mapping_family: u8,
}

impl OpusHead {
    /// Decodes the identification header.
    ///
    /// # Errors
    ///
    /// Gives an error when the packet is not an `OpusHead` packet.
    pub fn decode(packet: &[u8]) -> Result<Self, AudioError> {
        if packet.len() < 19 || &packet[..8] != b"OpusHead" {
            return Err(AudioError::Format(
                "the first Ogg packet is not an Opus header".to_owned(),
            ));
        }
        Ok(Self {
            version: packet[8],
            channels: packet[9],
            pre_skip: u16::from_le_bytes([packet[10], packet[11]]),
            input_sample_rate: u32::from_le_bytes([packet[12], packet[13], packet[14], packet[15]]),
            output_gain: i16::from_le_bytes([packet[16], packet[17]]),
            mapping_family: packet[18],
        })
    }

    fn encode(channels: u8) -> Vec<u8> {
        let mut packet = b"OpusHead".to_vec();
        packet.push(1);
        packet.push(channels);
        packet.extend_from_slice(&0_u16.to_le_bytes());
        packet.extend_from_slice(&48_000_u32.to_le_bytes());
        packet.extend_from_slice(&0_i16.to_le_bytes());
        packet.push(0);
        packet
    }
}

/// An audio source that gives the Opus packets of an Ogg Opus file without a new encode.
///
/// The packets must be mono or stereo at any frame size of 20 ms or more. Shorter frames
/// play too slowly, because the send task sends one packet each 20 ms. Pre-skip and the end
/// trim are not applied. Volume changes need the `opus` feature.
pub struct OggOpusSource {
    packets: OggPacketReader<Box<dyn Read + Send>>,
    head: OpusHead,
}

impl fmt::Debug for OggOpusSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OggOpusSource")
            .field("head", &self.head)
            .finish_non_exhaustive()
    }
}

impl OggOpusSource {
    /// Makes a source from a reader. Reads the two header packets.
    ///
    /// # Errors
    ///
    /// Gives an error when the input is not an Ogg Opus stream with one or two channels.
    pub fn new(reader: impl Read + Send + 'static) -> Result<Self, AudioError> {
        let mut packets: OggPacketReader<Box<dyn Read + Send>> =
            OggPacketReader::new(Box::new(reader));
        let first = packets
            .next_packet()?
            .ok_or_else(|| AudioError::Format("the Ogg input is empty".to_owned()))?;
        let head = OpusHead::decode(&first)?;
        if head.mapping_family != 0 || !(1..=2).contains(&head.channels) {
            return Err(AudioError::Unsupported(format!(
                "Ogg Opus with {} channels and mapping family {}",
                head.channels, head.mapping_family
            )));
        }

        let tags = packets.next_packet()?.unwrap_or_default();
        if !tags.starts_with(b"OpusTags") {
            return Err(AudioError::Format(
                "the second Ogg packet is not an Opus comment header".to_owned(),
            ));
        }
        Ok(Self { packets, head })
    }

    /// Makes a source from the bytes of a file.
    ///
    /// # Errors
    ///
    /// Gives an error when the input is not a valid Ogg Opus stream.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, AudioError> {
        Self::new(std::io::Cursor::new(bytes))
    }

    /// Opens a file.
    ///
    /// # Errors
    ///
    /// Gives an error when the file cannot be opened or is not a valid Ogg Opus file.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, AudioError> {
        let file = File::open(path)?;
        Self::new(BufReader::new(file))
    }

    /// Gives the identification header.
    #[must_use]
    pub fn head(&self) -> &OpusHead {
        &self.head
    }
}

impl AudioSource for OggOpusSource {
    fn next_frame(&mut self) -> Result<Option<SourceFrame>, AudioError> {
        loop {
            let Some(packet) = self.packets.next_packet()? else {
                return Ok(None);
            };
            if packet.is_empty() {
                continue;
            }
            let samples = opus_packet_samples(&packet).unwrap_or(960);
            return Ok(Some(SourceFrame::Opus { packet, samples }));
        }
    }
}

/// Writes Opus packets to an Ogg Opus file, one packet on each page.
pub struct OggOpusWriter<W: Write> {
    writer: W,
    serial: u32,
    sequence: u32,
    granule: i64,
    pending: Option<Vec<u8>>,
}

impl<W: Write> fmt::Debug for OggOpusWriter<W> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OggOpusWriter")
            .field("serial", &self.serial)
            .field("granule", &self.granule)
            .finish_non_exhaustive()
    }
}

impl<W: Write> OggOpusWriter<W> {
    /// Makes a writer and writes the two header pages.
    ///
    /// # Errors
    ///
    /// Gives an error when the write fails.
    pub fn new(writer: W, channels: u8, serial: u32) -> Result<Self, AudioError> {
        let mut this = Self {
            writer,
            serial,
            sequence: 0,
            granule: 0,
            pending: None,
        };
        this.write_page(&OpusHead::encode(channels), FLAG_FIRST)?;
        let mut tags = b"OpusTags".to_vec();
        let vendor = b"cordial";
        tags.extend_from_slice(&7_u32.to_le_bytes());
        tags.extend_from_slice(vendor);
        tags.extend_from_slice(&0_u32.to_le_bytes());
        this.write_page(&tags, 0)?;
        Ok(this)
    }

    /// Adds a packet. The page goes out when the next packet arrives or at the finish.
    ///
    /// # Errors
    ///
    /// Gives an error when the write fails or the packet is too large for one page.
    pub fn write_packet(&mut self, packet: &[u8]) -> Result<(), AudioError> {
        if packet.len() >= 255 * 255 {
            return Err(AudioError::Format(
                "the Opus packet is too large".to_owned(),
            ));
        }
        if let Some(previous) = self.pending.take() {
            self.write_audio_page(&previous, 0)?;
        }
        self.pending = Some(packet.to_vec());
        Ok(())
    }

    /// Writes the last page and gives the writer back.
    ///
    /// # Errors
    ///
    /// Gives an error when the write fails.
    pub fn finish(mut self) -> Result<W, AudioError> {
        let last = self.pending.take().unwrap_or_default();
        self.write_audio_page(&last, FLAG_LAST)?;
        self.writer.flush()?;
        Ok(self.writer)
    }

    fn write_audio_page(&mut self, packet: &[u8], flags: u8) -> Result<(), AudioError> {
        let samples = opus_packet_samples(packet).unwrap_or(0);
        if packet.is_empty() {
            // An empty last page adds no time.
        } else if samples == 0 {
            self.granule += i64::try_from(FRAME_SAMPLES).unwrap_or(0);
        } else {
            self.granule += i64::from(samples);
        }
        self.write_page(packet, flags)
    }

    fn write_page(&mut self, packet: &[u8], flags: u8) -> Result<(), AudioError> {
        let mut lacing = vec![255_u8; packet.len() / 255];
        // The remainder is less than 255.
        #[allow(clippy::cast_possible_truncation)]
        lacing.push((packet.len() % 255) as u8);
        let page = OggPage {
            header_type: flags,
            granule_position: self.granule,
            serial: self.serial,
            sequence: self.sequence,
            lacing,
            data: packet.to_vec(),
        };
        self.sequence += 1;
        self.writer.write_all(&page.encode())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_known_value() {
        // The Ogg CRC (CRC-32/MPEG-2 polynomial, initial value 0) of "123456789".
        assert_eq!(ogg_crc(b"123456789"), 0x89A1_897F);
    }

    fn sample_file() -> Vec<u8> {
        let mut writer = OggOpusWriter::new(Vec::new(), 2, 0x1234).expect("write");
        writer.write_packet(&[0xFC, 1, 2, 3]).expect("write");
        writer.write_packet(&[0xFC; 600]).expect("write");
        writer.write_packet(&[0x18, 9]).expect("write");
        writer.finish().expect("write")
    }

    #[test]
    fn writer_and_reader() {
        let file = sample_file();
        let mut pages = OggPageReader::new(file.as_slice());
        let first = pages.next_page().expect("valid").expect("page");
        assert_eq!(first.header_type, FLAG_FIRST);
        assert_eq!(first.serial, 0x1234);
        assert_eq!(&first.data[..8], b"OpusHead");
        let mut last = first;
        while let Some(page) = pages.next_page().expect("valid") {
            last = page;
        }
        assert!(last.is_last());
        assert_eq!(last.granule_position, 960 + 960 + 2880);

        let mut source = OggOpusSource::from_bytes(file).expect("valid");
        assert_eq!(source.head().channels, 2);
        assert_eq!(
            source.next_frame().expect("valid"),
            Some(SourceFrame::Opus {
                packet: vec![0xFC, 1, 2, 3],
                samples: 960
            })
        );
        assert_eq!(
            source.next_frame().expect("valid"),
            Some(SourceFrame::Opus {
                packet: vec![0xFC; 600],
                samples: 960
            })
        );
        assert_eq!(
            source.next_frame().expect("valid"),
            Some(SourceFrame::Opus {
                packet: vec![0x18, 9],
                samples: 2880
            })
        );
        assert_eq!(source.next_frame().expect("valid"), None);
    }

    /// A packet that spans two pages, a page of another stream between them, and a page with
    /// two packets. The pages are built by hand.
    #[test]
    fn packets_across_pages() {
        let page = |flags: u8, serial: u32, sequence: u32, lacing: Vec<u8>, data: Vec<u8>| {
            OggPage {
                header_type: flags,
                granule_position: 0,
                serial,
                sequence,
                lacing,
                data,
            }
            .encode()
        };
        let mut file = Vec::new();
        file.extend(page(FLAG_FIRST, 1, 0, vec![19], OpusHead::encode(1)));
        file.extend(page(FLAG_FIRST, 2, 0, vec![3], b"abc".to_vec()));
        let mut tags = b"OpusTags".to_vec();
        tags.extend_from_slice(&[0; 8]);
        file.extend(page(0, 1, 1, vec![16], tags));
        // 300 bytes: 255 on this page, 45 on the next page.
        file.extend(page(0, 1, 2, vec![255], vec![0xFC; 255]));
        file.extend(page(0, 2, 1, vec![1], vec![0]));
        let mut data = vec![0xFC; 45];
        data.extend_from_slice(&[0xF8, 0xFF, 0xFE, 0xFC, 7]);
        file.extend(page(FLAG_CONTINUED | FLAG_LAST, 1, 3, vec![45, 3, 2], data));

        let mut source = OggOpusSource::from_bytes(file).expect("valid");
        assert_eq!(source.head().channels, 1);
        let mut packets = Vec::new();
        while let Some(SourceFrame::Opus { packet, .. }) = source.next_frame().expect("valid") {
            packets.push(packet);
        }
        assert_eq!(packets.len(), 3);
        assert_eq!(packets[0], vec![0xFC; 300]);
        assert_eq!(packets[1], vec![0xF8, 0xFF, 0xFE]);
        assert_eq!(packets[2], vec![0xFC, 7]);
    }

    #[test]
    fn bad_input() {
        let mut file = sample_file();
        file[40] ^= 0xFF;
        assert!(matches!(
            OggOpusSource::from_bytes(file),
            Err(AudioError::Format(_))
        ));

        let file = sample_file();
        let truncated = file[..file.len() - 2].to_vec();
        let mut source = OggOpusSource::from_bytes(truncated).expect("headers are complete");
        let mut result = source.next_frame();
        while let Ok(Some(_)) = result {
            result = source.next_frame();
        }
        assert!(result.is_err());

        assert!(OggOpusSource::from_bytes(b"not ogg data at all, no".to_vec()).is_err());
        assert!(OggOpusSource::from_bytes(Vec::new()).is_err());
    }
}
