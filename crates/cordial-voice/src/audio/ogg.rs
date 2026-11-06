//! Ogg Opus files (RFC 3533 and RFC 7845): a reader for passthrough and a small writer.
//!
//! The reader gives the Opus packets of a file without a new encode. The writer stores Opus
//! packets in an Ogg file, for example to record received audio.

use std::collections::VecDeque;
use std::fmt;
use std::fs::File;
use std::io::{BufReader, ErrorKind, Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::time::Duration;

use super::{
    AudioError, AudioSource, FRAME_SAMPLES, SourceFrame, duration_to_samples, opus_packet_samples,
    samples_to_duration,
};

const CAPTURE_PATTERN: &[u8; 4] = b"OggS";
const HEADER_LEN: usize = 27;
const FLAG_CONTINUED: u8 = 0x01;
const FLAG_FIRST: u8 = 0x02;
const FLAG_LAST: u8 = 0x04;
/// The largest packet of the packet reader. RFC 7845, section 5.2, permits comment headers
/// up to this size (for example with cover art).
const MAX_PACKET_LEN: usize = 125_829_120;
/// The largest Opus audio packet (RFC 7845, section 6).
const MAX_AUDIO_PACKET_LEN: usize = 61_440;

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

    fn get_mut(&mut self) -> &mut R {
        &mut self.reader
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
fn read_full(reader: &mut (impl Read + ?Sized), buffer: &mut [u8]) -> Result<bool, AudioError> {
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

/// The end of a page. It goes with the last packet that ends on the page.
#[derive(Clone, Copy, Debug)]
struct PageEnd {
    granule: i64,
    last: bool,
}

/// A packet with the end of its page, when it is the last packet that ends on the page.
type Packet = (Vec<u8>, Option<PageEnd>);

/// Reads the packets of the first logical stream of an Ogg file.
///
/// A packet larger than 125,829,120 bytes is a format error.
pub struct OggPacketReader<R> {
    pages: OggPageReader<R>,
    serial: Option<u32>,
    partial: Vec<u8>,
    max_packet_len: usize,
    packets: VecDeque<Packet>,
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
            max_packet_len: MAX_PACKET_LEN,
            packets: VecDeque::new(),
            ended: false,
        }
    }

    /// Sets the largest packet size. A larger packet is a format error.
    fn set_max_packet_len(&mut self, max_packet_len: usize) {
        self.max_packet_len = max_packet_len;
    }

    /// Reads the next packet. Gives `Ok(None)` at the end of the stream.
    ///
    /// # Errors
    ///
    /// Gives an error when a page is not valid.
    pub fn next_packet(&mut self) -> Result<Option<Vec<u8>>, AudioError> {
        Ok(self.next_packet_with_end()?.map(|(packet, _)| packet))
    }

    /// Reads the next packet, with the page end when the packet is the last one that ends on
    /// its page.
    fn next_packet_with_end(&mut self) -> Result<Option<Packet>, AudioError> {
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
            self.add_page(&page)?;
        }
    }

    /// Forgets the read state. Call it after the input moves to the start of a page.
    fn restart(&mut self) {
        self.partial.clear();
        self.packets.clear();
        self.ended = false;
    }

    /// Reads the next page of the stream and drops the packets that end on it. Keeps the
    /// start of a packet that continues on the next page.
    fn skip_page(&mut self) -> Result<(), AudioError> {
        loop {
            let Some(page) = self.pages.next_page()? else {
                self.ended = true;
                return Ok(());
            };
            if self.serial.is_some_and(|serial| serial != page.serial) {
                continue;
            }

            let mut offset = 0;
            let mut tail = 0;
            for value in &page.lacing {
                offset += usize::from(*value);
                if *value < 255 {
                    tail = offset;
                }
            }
            self.partial = page.data[tail..].to_vec();
            self.ended = page.is_last();
            return Ok(());
        }
    }

    fn add_page(&mut self, page: &OggPage) -> Result<(), AudioError> {
        let serial = *self.serial.get_or_insert(page.serial);
        if page.serial != serial {
            return Ok(());
        }
        if !page.is_continued() && !self.partial.is_empty() {
            tracing::debug!("an Ogg packet has no end; dropped");
            self.partial.clear();
        }

        let mut offset = 0;
        let mut ended_here = 0;
        for value in &page.lacing {
            let end = offset + usize::from(*value);
            self.partial.extend_from_slice(&page.data[offset..end]);
            offset = end;
            if self.partial.len() > self.max_packet_len {
                self.partial.clear();
                return Err(AudioError::Format(format!(
                    "an Ogg packet is larger than {} bytes",
                    self.max_packet_len
                )));
            }
            if *value < 255 {
                self.packets
                    .push_back((std::mem::take(&mut self.partial), None));
                ended_here += 1;
            }
        }
        if ended_here > 0
            && let Some((_, page_end)) = self.packets.back_mut()
        {
            *page_end = Some(PageEnd {
                granule: page.granule_position,
                last: page.is_last(),
            });
        }
        if page.is_last() {
            self.ended = true;
        }
        Ok(())
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

    fn encode(channels: u8, pre_skip: u16) -> Vec<u8> {
        let mut packet = b"OpusHead".to_vec();
        packet.push(1);
        packet.push(channels);
        packet.extend_from_slice(&pre_skip.to_le_bytes());
        packet.extend_from_slice(&48_000_u32.to_le_bytes());
        packet.extend_from_slice(&0_i16.to_le_bytes());
        packet.push(0);
        packet
    }
}

/// An input that can seek or not.
trait SeekRead: Read + Seek + Send {}

impl<T: Read + Seek + Send> SeekRead for T {}

enum Input {
    Stream(Box<dyn Read + Send>),
    Seekable(Box<dyn SeekRead>),
}

impl Read for Input {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Self::Stream(reader) => reader.read(buffer),
            Self::Seekable(reader) => reader.read(buffer),
        }
    }
}

/// An audio source that gives the Opus packets of an Ogg Opus file without a new encode.
///
/// The packets must be mono or stereo. Packets with a duration that is not a multiple of
/// 20 ms need the `opus` feature: the mixer decodes them and encodes 20 ms frames.
///
/// # Pre-skip and end trim
///
/// The source applies the pre-skip of the `OpusHead` header and the end trim of the last
/// page (RFC 7845, sections 4.2 and 4.4) through [`SourceFrame::Opus`]. Only a decode can
/// drop part of a packet. Thus:
///
/// - When the mixer decodes the track (more tracks, a volume change, or short packets), it
///   drops the pre-skip samples and the samples after the end of the last page.
/// - When the packets go out without a new encode, the pre-skip samples play (the encoder
///   start, usually silence) and the last packet plays in full.
///
/// The end trim applies only to the last packet of the stream.
///
/// # Seek
///
/// A source from [`OggOpusSource::open`], [`OggOpusSource::from_bytes`], or
/// [`OggOpusSource::seekable`] can seek. A source from [`OggOpusSource::new`] cannot seek.
///
/// - The seek reads the page headers from the start of the audio to find the last page
///   before the position. The cost increases with the number of pages before the position.
/// - Packets that end before the position do not play.
/// - Without a decode, the audio starts at the start of the packet that contains the
///   position: at most one packet (usually 20 ms) early.
/// - With a decode, the mixer drops the samples before the position. The decoder starts at
///   the new packet without a pre-roll, so the first frame can have a short artifact.
pub struct OggOpusSource {
    packets: OggPacketReader<Input>,
    head: OpusHead,
    /// The byte offset of the first audio page. `None` when the input cannot seek.
    data_start: Option<u64>,
    /// The granule position at the start of the next packet.
    granule: i64,
    /// A decode drops the samples before this granule position.
    discard_until: i64,
    /// After a seek, packets that end before `discard_until` do not play.
    skip_before: bool,
}

impl fmt::Debug for OggOpusSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OggOpusSource")
            .field("head", &self.head)
            .field("granule", &self.granule)
            .finish_non_exhaustive()
    }
}

impl OggOpusSource {
    /// Makes a source from a reader. Reads the two header packets. The source cannot seek;
    /// use [`OggOpusSource::seekable`] for an input that can seek.
    ///
    /// # Errors
    ///
    /// Gives an error when the input is not an Ogg Opus stream with one or two channels.
    pub fn new(reader: impl Read + Send + 'static) -> Result<Self, AudioError> {
        Self::with_input(Input::Stream(Box::new(reader)))
    }

    /// Makes a source that can seek from a reader, for example a file. Reads the two header
    /// packets.
    ///
    /// # Errors
    ///
    /// Gives an error when the input is not an Ogg Opus stream with one or two channels.
    pub fn seekable(reader: impl Read + Seek + Send + 'static) -> Result<Self, AudioError> {
        Self::with_input(Input::Seekable(Box::new(reader)))
    }

    fn with_input(input: Input) -> Result<Self, AudioError> {
        let mut packets = OggPacketReader::new(input);
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
        packets.set_max_packet_len(MAX_AUDIO_PACKET_LEN);

        // RFC 7845 makes the audio start on a new page after the comment header.
        let data_start = match packets.pages.get_mut() {
            Input::Seekable(reader) => Some(reader.stream_position()?),
            Input::Stream(_) => None,
        };
        Ok(Self {
            packets,
            head,
            data_start,
            granule: 0,
            discard_until: i64::from(head.pre_skip),
            skip_before: false,
        })
    }

    /// Makes a source from the bytes of a file. The source can seek.
    ///
    /// # Errors
    ///
    /// Gives an error when the input is not a valid Ogg Opus stream.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, AudioError> {
        Self::seekable(std::io::Cursor::new(bytes))
    }

    /// Opens a file. The source can seek.
    ///
    /// # Errors
    ///
    /// Gives an error when the file cannot be opened or is not a valid Ogg Opus file.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, AudioError> {
        let file = File::open(path)?;
        Self::seekable(BufReader::new(file))
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
            let Some((packet, page_end)) = self.packets.next_packet_with_end()? else {
                return Ok(None);
            };
            let samples = if packet.is_empty() {
                0
            } else {
                opus_packet_samples(&packet).unwrap_or(960)
            };
            let start = self.granule;
            // The granule position comes from the file and can be near the maximum.
            let end = start.saturating_add(i64::from(samples));
            self.granule = end;

            let mut discard_end = 0;
            if let Some(page_end) = page_end.filter(|page_end| page_end.granule >= 0) {
                if page_end.last {
                    discard_end = clamp_samples(end - page_end.granule, samples);
                }
                // The page tells the exact position. It corrects a count that is wrong, for
                // example after a packet that is not valid.
                self.granule = page_end.granule;
            }
            if packet.is_empty() {
                continue;
            }

            let discard_start = clamp_samples(self.discard_until - start, samples);
            if self.skip_before {
                if discard_start == samples {
                    continue;
                }
                self.skip_before = false;
            }
            return Ok(Some(SourceFrame::Opus {
                packet,
                samples,
                discard_start,
                discard_end: discard_end.min(samples - discard_start),
            }));
        }
    }

    fn seek(&mut self, position: Duration) -> Result<Duration, AudioError> {
        let (Some(data_start), Input::Seekable(reader)) =
            (self.data_start, self.packets.pages.get_mut())
        else {
            return Err(AudioError::Unsupported(
                "an Ogg Opus source from a reader without seek cannot seek".to_owned(),
            ));
        };

        let offset = i64::try_from(duration_to_samples(position)).unwrap_or(i64::MAX);
        let target = i64::from(self.head.pre_skip).saturating_add(offset);
        let page = find_page_before(reader.as_mut(), data_start, self.packets.serial, target)?;
        let (page_offset, granule) = page.unwrap_or((data_start, 0));
        reader.seek(SeekFrom::Start(page_offset))?;

        self.packets.restart();
        if page.is_some() {
            self.packets.skip_page()?;
        }
        self.granule = granule;
        self.discard_until = target;
        self.skip_before = true;
        Ok(samples_to_duration(offset.unsigned_abs()))
    }
}

/// Converts a sample count to a value in `0..=samples`.
fn clamp_samples(count: i64, samples: u32) -> u32 {
    u32::try_from(count.clamp(0, i64::from(samples))).unwrap_or(samples)
}

/// Finds the last page of the stream that ends at or before the target granule position.
/// Gives its byte offset and its granule position. Reads only the page headers.
fn find_page_before(
    reader: &mut dyn SeekRead,
    start: u64,
    serial: Option<u32>,
    target: i64,
) -> Result<Option<(u64, i64)>, AudioError> {
    reader.seek(SeekFrom::Start(start))?;
    let mut offset = start;
    let mut found = None;
    loop {
        let mut header = [0_u8; HEADER_LEN];
        if !read_full(reader, &mut header)? {
            break;
        }
        if &header[..4] != CAPTURE_PATTERN {
            return Err(AudioError::Format(
                "an Ogg page has no capture pattern".to_owned(),
            ));
        }
        let mut lacing = vec![0_u8; usize::from(header[26])];
        if !read_full(reader, &mut lacing)? && !lacing.is_empty() {
            return Err(truncated());
        }

        let granule = i64::from_le_bytes(header[6..14].try_into().unwrap_or_default());
        let page_serial = u32::from_le_bytes([header[14], header[15], header[16], header[17]]);
        // A granule position of -1 tells that no packet ends on the page.
        if serial.is_none_or(|serial| serial == page_serial) && granule >= 0 {
            if granule > target {
                break;
            }
            found = Some((offset, granule));
        }

        let data_len: u64 = lacing.iter().map(|value| u64::from(*value)).sum();
        reader.seek(SeekFrom::Current(
            i64::try_from(data_len).unwrap_or(i64::MAX),
        ))?;
        offset += (HEADER_LEN + lacing.len()) as u64 + data_len;
    }
    Ok(found)
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
        this.write_page(&OpusHead::encode(channels, 0), FLAG_FIRST)?;
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
pub(crate) mod tests {
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
            Some(SourceFrame::opus(vec![0xFC, 1, 2, 3]))
        );
        assert_eq!(
            source.next_frame().expect("valid"),
            Some(SourceFrame::opus(vec![0xFC; 600]))
        );
        assert_eq!(
            source.next_frame().expect("valid"),
            Some(SourceFrame::opus(vec![0x18, 9]))
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
        file.extend(page(FLAG_FIRST, 1, 0, vec![19], OpusHead::encode(1, 0)));
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

    /// Builds an Ogg Opus file with `per_page` packets on each audio page. Each packet must
    /// be shorter than 255 bytes. The last page has `final_granule` when it is given.
    pub(crate) fn build_file(
        pre_skip: u16,
        packets: &[Vec<u8>],
        per_page: usize,
        final_granule: Option<i64>,
    ) -> Vec<u8> {
        let mut tags = b"OpusTags".to_vec();
        tags.extend_from_slice(&[0; 8]);
        let mut pages = vec![
            (FLAG_FIRST, 0, vec![OpusHead::encode(2, pre_skip)]),
            (0, 0, vec![tags]),
        ];

        let mut granule = 0;
        let chunks: Vec<&[Vec<u8>]> = packets.chunks(per_page).collect();
        for (index, chunk) in chunks.iter().enumerate() {
            granule += chunk
                .iter()
                .map(|packet| i64::from(opus_packet_samples(packet).expect("valid")))
                .sum::<i64>();
            let last = index + 1 == chunks.len();
            let (flags, page_granule) = match final_granule {
                Some(value) if last => (FLAG_LAST, value),
                _ if last => (FLAG_LAST, granule),
                _ => (0, granule),
            };
            pages.push((flags, page_granule, chunk.to_vec()));
        }

        let mut file = Vec::new();
        for (sequence, (flags, granule, packets)) in pages.into_iter().enumerate() {
            let lacing = packets
                .iter()
                .map(|packet| u8::try_from(packet.len()).expect("short packet"))
                .collect();
            file.extend(
                OggPage {
                    header_type: flags,
                    granule_position: granule,
                    serial: 7,
                    sequence: u32::try_from(sequence).expect("small"),
                    lacing,
                    data: packets.concat(),
                }
                .encode(),
            );
        }
        file
    }

    /// Gives the next frame as (packet, discard at the start, discard at the end).
    fn next_opus(source: &mut OggOpusSource) -> Option<(Vec<u8>, u32, u32)> {
        match source.next_frame().expect("valid")? {
            SourceFrame::Opus {
                packet,
                discard_start,
                discard_end,
                ..
            } => Some((packet, discard_start, discard_end)),
            other => panic!("Opus expected, got {other:?}"),
        }
    }

    /// Ten CELT packets of 20 ms. The second byte is the packet index.
    fn numbered_packets() -> Vec<Vec<u8>> {
        (0..10).map(|index| vec![0xFC, index]).collect()
    }

    #[test]
    fn pre_skip_and_end_trim() {
        let packets: Vec<Vec<u8>> = numbered_packets().into_iter().take(5).collect();
        let file = build_file(1000, &packets, 2, Some(4800 - 300));
        let mut source = OggOpusSource::from_bytes(file).expect("valid");
        assert_eq!(source.head().pre_skip, 1000);

        assert_eq!(next_opus(&mut source), Some((vec![0xFC, 0], 960, 0)));
        assert_eq!(next_opus(&mut source), Some((vec![0xFC, 1], 40, 0)));
        assert_eq!(next_opus(&mut source), Some((vec![0xFC, 2], 0, 0)));
        assert_eq!(next_opus(&mut source), Some((vec![0xFC, 3], 0, 0)));
        assert_eq!(next_opus(&mut source), Some((vec![0xFC, 4], 0, 300)));
        assert_eq!(next_opus(&mut source), None);
    }

    #[test]
    fn seek_by_granule_position() {
        let file = build_file(312, &numbered_packets(), 3, None);
        let mut source = OggOpusSource::from_bytes(file.clone()).expect("valid");
        next_opus(&mut source);

        // 50 ms is granule position 312 + 2400 = 2712, inside packet 2 (1920 to 2880).
        let position = source.seek(Duration::from_millis(50)).expect("seek");
        assert_eq!(position, Duration::from_millis(50));
        assert_eq!(next_opus(&mut source), Some((vec![0xFC, 2], 792, 0)));
        assert_eq!(next_opus(&mut source), Some((vec![0xFC, 3], 0, 0)));

        // Packet 7 is the first packet of the third audio page.
        source.seek(Duration::from_millis(140)).expect("seek");
        assert_eq!(next_opus(&mut source), Some((vec![0xFC, 7], 312, 0)));

        source.seek(Duration::ZERO).expect("seek");
        assert_eq!(next_opus(&mut source), Some((vec![0xFC, 0], 312, 0)));
        assert_eq!(next_opus(&mut source), Some((vec![0xFC, 1], 0, 0)));

        source
            .seek(Duration::from_secs(1))
            .expect("seek after the end");
        assert_eq!(next_opus(&mut source), None);
        source.seek(Duration::from_millis(185)).expect("seek");
        assert_eq!(next_opus(&mut source), Some((vec![0xFC, 9], 552, 0)));
        assert_eq!(next_opus(&mut source), None);

        let mut stream = OggOpusSource::new(std::io::Cursor::new(file)).expect("valid");
        assert!(matches!(
            stream.seek(Duration::ZERO),
            Err(AudioError::Unsupported(_))
        ));
    }

    /// A seek to a page whose last packet continues on the next page.
    #[test]
    fn seek_with_a_packet_across_pages() {
        let page = |flags: u8, granule: i64, sequence: u32, lacing: Vec<u8>, data: Vec<u8>| {
            OggPage {
                header_type: flags,
                granule_position: granule,
                serial: 1,
                sequence,
                lacing,
                data,
            }
            .encode()
        };
        let mut tags = b"OpusTags".to_vec();
        tags.extend_from_slice(&[0; 8]);
        let mut file = Vec::new();
        file.extend(page(FLAG_FIRST, 0, 0, vec![19], OpusHead::encode(2, 0)));
        file.extend(page(0, 0, 1, vec![16], tags));
        // Packet 0 (2 bytes), then 255 bytes of packet 1 (300 bytes).
        let mut data = vec![0xFC, 0];
        data.extend_from_slice(&[0xFC; 255]);
        file.extend(page(0, 960, 2, vec![2, 255], data));
        // The last 45 bytes of packet 1, then packet 2.
        let mut data = vec![0xFC; 45];
        data.extend_from_slice(&[0xFC, 2]);
        file.extend(page(FLAG_CONTINUED | FLAG_LAST, 2880, 3, vec![45, 2], data));

        let mut source = OggOpusSource::from_bytes(file).expect("valid");
        source.seek(Duration::from_millis(30)).expect("seek");
        assert_eq!(next_opus(&mut source), Some((vec![0xFC; 300], 480, 0)));
        assert_eq!(next_opus(&mut source), Some((vec![0xFC, 2], 0, 0)));
        assert_eq!(next_opus(&mut source), None);
    }

    fn header_pages() -> Vec<u8> {
        let mut tags = b"OpusTags".to_vec();
        tags.extend_from_slice(&[0; 8]);
        let mut file = OggPage {
            header_type: FLAG_FIRST,
            granule_position: 0,
            serial: 1,
            sequence: 0,
            lacing: vec![19],
            data: OpusHead::encode(2, 0),
        }
        .encode();
        file.extend(
            OggPage {
                header_type: 0,
                granule_position: 0,
                serial: 1,
                sequence: 1,
                lacing: vec![16],
                data: tags,
            }
            .encode(),
        );
        file
    }

    /// A packet that does not end must not fill the memory.
    #[test]
    fn oversized_packet_is_an_error() {
        let mut file = header_pages();
        for sequence in 2..5 {
            file.extend(
                OggPage {
                    header_type: if sequence == 2 { 0 } else { FLAG_CONTINUED },
                    granule_position: -1,
                    serial: 1,
                    sequence,
                    lacing: vec![255; 255],
                    data: vec![0xFC; 255 * 255],
                }
                .encode(),
            );
        }

        let mut source = OggOpusSource::from_bytes(file).expect("the headers are valid");
        assert!(matches!(source.next_frame(), Err(AudioError::Format(_))));
    }

    /// A large comment header (for example with cover art) is valid.
    #[test]
    fn large_comment_header_is_valid() {
        let mut tags = b"OpusTags".to_vec();
        tags.resize(100_000, 0);
        let head = OggPage {
            header_type: FLAG_FIRST,
            granule_position: 0,
            serial: 1,
            sequence: 0,
            lacing: vec![19],
            data: OpusHead::encode(2, 0),
        };
        let mut file = head.encode();
        let mut sequence = 1;
        for (index, chunk) in tags.chunks(255 * 255).enumerate() {
            let mut lacing = vec![255_u8; chunk.len() / 255];
            if chunk.len() < 255 * 255 {
                lacing.push(u8::try_from(chunk.len() % 255).expect("small"));
            }
            let flags = if index == 0 { 0 } else { FLAG_CONTINUED };
            file.extend(
                OggPage {
                    header_type: flags,
                    granule_position: 0,
                    serial: 1,
                    sequence,
                    lacing,
                    data: chunk.to_vec(),
                }
                .encode(),
            );
            sequence += 1;
        }
        file.extend(
            OggPage {
                header_type: FLAG_LAST,
                granule_position: 960,
                serial: 1,
                sequence,
                lacing: vec![2],
                data: vec![0xFC, 0],
            }
            .encode(),
        );

        let mut source = OggOpusSource::from_bytes(file).expect("valid");
        assert_eq!(next_opus(&mut source), Some((vec![0xFC, 0], 0, 0)));
    }

    /// A granule position near the maximum must not overflow the sample count.
    #[test]
    fn granule_near_the_maximum() {
        let mut file = header_pages();
        for (sequence, flags) in [(2, 0), (3, FLAG_LAST)] {
            file.extend(
                OggPage {
                    header_type: flags,
                    granule_position: i64::MAX,
                    serial: 1,
                    sequence,
                    lacing: vec![2],
                    data: vec![0xFC, u8::try_from(sequence).expect("small")],
                }
                .encode(),
            );
        }

        let mut source = OggOpusSource::from_bytes(file).expect("valid");
        assert!(next_opus(&mut source).is_some());
        assert!(next_opus(&mut source).is_some());
        assert_eq!(next_opus(&mut source), None);
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
