//! UDP packets: IP discovery, RTP headers, and the packet kinds on the voice socket.

use std::net::IpAddr;

/// The size of an IP discovery packet.
pub const IP_DISCOVERY_LEN: usize = 74;
/// The size of an RTP header without CSRCs and without an extension.
pub const RTP_HEADER_LEN: usize = 12;
/// The first byte of an RTP header that Discord uses: version 2, no padding, no extension.
pub const RTP_VERSION_BYTE: u8 = 0x80;
/// The RTP payload type of Opus on Discord.
pub const RTP_PAYLOAD_TYPE: u8 = 0x78;

const IP_DISCOVERY_REQUEST: u16 = 1;
const IP_DISCOVERY_RESPONSE: u16 = 2;
const IP_DISCOVERY_BODY_LEN: u16 = 70;
const ADDRESS_LEN: usize = 64;
// Community knowledge: Discord keepalive packets start with one of these values.
const KEEPALIVE_MAGIC: [[u8; 4]; 2] = [[0x13, 0x37, 0xCA, 0xFE], [0x13, 0x37, 0xF0, 0x0D]];

/// An error in a UDP packet.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum PacketError {
    /// The packet is too short.
    #[error("the packet has {actual} bytes; at least {expected} are necessary")]
    TooShort {
        /// The necessary size.
        expected: usize,
        /// The size of the packet.
        actual: usize,
    },
    /// The RTP version is not 2.
    #[error("the RTP version is {0}, not 2")]
    BadVersion(u8),
    /// The packet is not an IP discovery response.
    #[error("the packet is not an IP discovery response")]
    NotDiscoveryResponse,
    /// The address in the IP discovery response is not valid.
    #[error("the address in the IP discovery response is not valid")]
    BadAddress,
}

/// Makes an IP discovery request for an SSRC.
#[must_use]
pub fn ip_discovery_request(ssrc: u32) -> [u8; IP_DISCOVERY_LEN] {
    let mut packet = [0_u8; IP_DISCOVERY_LEN];
    packet[0..2].copy_from_slice(&IP_DISCOVERY_REQUEST.to_be_bytes());
    packet[2..4].copy_from_slice(&IP_DISCOVERY_BODY_LEN.to_be_bytes());
    packet[4..8].copy_from_slice(&ssrc.to_be_bytes());
    packet
}

/// An IP discovery response: the external address of the client.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct IpDiscovery {
    /// The SSRC.
    pub ssrc: u32,
    /// The external IP address.
    pub address: String,
    /// The external UDP port.
    pub port: u16,
}

impl IpDiscovery {
    /// Makes a response. The voice server sends it; tests use it.
    #[must_use]
    pub fn new(ssrc: u32, address: String, port: u16) -> Self {
        Self {
            ssrc,
            address,
            port,
        }
    }

    /// Decodes an IP discovery response.
    ///
    /// # Errors
    ///
    /// Gives an error when the packet is not a valid response.
    pub fn decode(packet: &[u8]) -> Result<Self, PacketError> {
        if packet.len() < IP_DISCOVERY_LEN {
            return Err(PacketError::TooShort {
                expected: IP_DISCOVERY_LEN,
                actual: packet.len(),
            });
        }
        let kind = u16::from_be_bytes([packet[0], packet[1]]);
        let length = u16::from_be_bytes([packet[2], packet[3]]);
        if kind != IP_DISCOVERY_RESPONSE || length != IP_DISCOVERY_BODY_LEN {
            return Err(PacketError::NotDiscoveryResponse);
        }

        let ssrc = u32::from_be_bytes([packet[4], packet[5], packet[6], packet[7]]);
        let field = &packet[8..8 + ADDRESS_LEN];
        let end = field
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(ADDRESS_LEN);
        let address = std::str::from_utf8(&field[..end])
            .map_err(|_| PacketError::BadAddress)?
            .to_owned();
        if address.parse::<IpAddr>().is_err() {
            return Err(PacketError::BadAddress);
        }
        let port = u16::from_be_bytes([packet[72], packet[73]]);
        Ok(Self {
            ssrc,
            address,
            port,
        })
    }

    /// Encodes the response.
    ///
    /// # Errors
    ///
    /// Gives an error when the address is longer than 63 bytes.
    pub fn encode(&self) -> Result<[u8; IP_DISCOVERY_LEN], PacketError> {
        let address = self.address.as_bytes();
        if address.len() >= ADDRESS_LEN {
            return Err(PacketError::BadAddress);
        }
        let mut packet = [0_u8; IP_DISCOVERY_LEN];
        packet[0..2].copy_from_slice(&IP_DISCOVERY_RESPONSE.to_be_bytes());
        packet[2..4].copy_from_slice(&IP_DISCOVERY_BODY_LEN.to_be_bytes());
        packet[4..8].copy_from_slice(&self.ssrc.to_be_bytes());
        packet[8..8 + address.len()].copy_from_slice(address);
        packet[72..74].copy_from_slice(&self.port.to_be_bytes());
        Ok(packet)
    }
}

/// The kind of a packet on the voice socket.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum PacketKind {
    /// An RTP packet with audio.
    Rtp,
    /// An RTCP packet (payload type 200 to 206).
    Rtcp,
    /// A keepalive packet.
    Keepalive,
    /// An IP discovery packet.
    IpDiscovery,
    /// A packet of no known kind.
    Unknown,
}

/// Tells the kind of a packet on the voice socket.
#[must_use]
pub fn classify(packet: &[u8]) -> PacketKind {
    if packet.len() == IP_DISCOVERY_LEN && packet[0..4] == [0x00, 0x02, 0x00, 0x46] {
        return PacketKind::IpDiscovery;
    }
    if packet.len() >= 4 && KEEPALIVE_MAGIC.iter().any(|magic| packet[..4] == magic[..]) {
        return PacketKind::Keepalive;
    }
    if packet.len() < RTP_HEADER_LEN || packet[0] >> 6 != 2 {
        return PacketKind::Unknown;
    }
    if (200..=206).contains(&packet[1]) {
        return PacketKind::Rtcp;
    }
    PacketKind::Rtp
}

/// An RTP header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct RtpHeader {
    /// Tells if the payload has padding at the end.
    pub padding: bool,
    /// Tells if a header extension follows the CSRCs.
    pub extension: bool,
    /// The number of CSRCs.
    pub csrc_count: u8,
    /// The marker bit.
    pub marker: bool,
    /// The payload type. Discord uses 120 for Opus.
    pub payload_type: u8,
    /// The sequence number.
    pub sequence: u16,
    /// The timestamp in samples at 48 kHz.
    pub timestamp: u32,
    /// The SSRC of the sender.
    pub ssrc: u32,
}

impl RtpHeader {
    /// Makes the header of a packet that the client sends.
    #[must_use]
    pub const fn new(sequence: u16, timestamp: u32, ssrc: u32) -> Self {
        Self {
            padding: false,
            extension: false,
            csrc_count: 0,
            marker: false,
            payload_type: RTP_PAYLOAD_TYPE,
            sequence,
            timestamp,
            ssrc,
        }
    }

    /// Encodes the fixed 12 bytes of the header. CSRCs and extensions are not encoded.
    #[must_use]
    pub fn encode(&self) -> [u8; RTP_HEADER_LEN] {
        let mut header = [0_u8; RTP_HEADER_LEN];
        header[0] = 0x80
            | (u8::from(self.padding) << 5)
            | (u8::from(self.extension) << 4)
            | (self.csrc_count & 0x0F);
        header[1] = (u8::from(self.marker) << 7) | (self.payload_type & 0x7F);
        header[2..4].copy_from_slice(&self.sequence.to_be_bytes());
        header[4..8].copy_from_slice(&self.timestamp.to_be_bytes());
        header[8..12].copy_from_slice(&self.ssrc.to_be_bytes());
        header
    }

    /// Decodes the fixed 12 bytes of a header.
    ///
    /// # Errors
    ///
    /// Gives an error when the packet is too short or the version is not 2.
    pub fn decode(packet: &[u8]) -> Result<Self, PacketError> {
        if packet.len() < RTP_HEADER_LEN {
            return Err(PacketError::TooShort {
                expected: RTP_HEADER_LEN,
                actual: packet.len(),
            });
        }
        let version = packet[0] >> 6;
        if version != 2 {
            return Err(PacketError::BadVersion(version));
        }
        Ok(Self {
            padding: packet[0] & 0x20 != 0,
            extension: packet[0] & 0x10 != 0,
            csrc_count: packet[0] & 0x0F,
            marker: packet[1] & 0x80 != 0,
            payload_type: packet[1] & 0x7F,
            sequence: u16::from_be_bytes([packet[2], packet[3]]),
            timestamp: u32::from_be_bytes([packet[4], packet[5], packet[6], packet[7]]),
            ssrc: u32::from_be_bytes([packet[8], packet[9], packet[10], packet[11]]),
        })
    }

    /// Gives the size of the unencrypted header in the rtpsize modes: 12 bytes, 4 bytes for
    /// each CSRC, and 4 bytes for the extension preamble.
    #[must_use]
    pub fn unencrypted_len(&self) -> usize {
        RTP_HEADER_LEN + 4 * usize::from(self.csrc_count) + if self.extension { 4 } else { 0 }
    }
}

/// An RTP packet split into its parts. The body is still encrypted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct RtpPacket<'a> {
    /// The header.
    pub header: RtpHeader,
    /// The unencrypted header: the additional data of the AEAD.
    pub header_bytes: &'a [u8],
    /// The length of the extension body in 32-bit words, when the extension bit is set.
    pub extension_words: Option<u16>,
    /// The bytes after the unencrypted header.
    pub body: &'a [u8],
}

impl<'a> RtpPacket<'a> {
    /// Splits a packet into its parts.
    ///
    /// # Errors
    ///
    /// Gives an error when the packet is shorter than its header.
    pub fn parse(packet: &'a [u8]) -> Result<Self, PacketError> {
        let header = RtpHeader::decode(packet)?;
        let header_len = header.unencrypted_len();
        if packet.len() < header_len {
            return Err(PacketError::TooShort {
                expected: header_len,
                actual: packet.len(),
            });
        }
        let extension_words = header
            .extension
            .then(|| u16::from_be_bytes([packet[header_len - 2], packet[header_len - 1]]));
        Ok(Self {
            header,
            header_bytes: &packet[..header_len],
            extension_words,
            body: &packet[header_len..],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovery_request_layout() {
        let packet = ip_discovery_request(0x0102_0304);
        assert_eq!(packet.len(), 74);
        assert_eq!(
            &packet[..8],
            &[0x00, 0x01, 0x00, 0x46, 0x01, 0x02, 0x03, 0x04]
        );
        assert!(packet[8..].iter().all(|byte| *byte == 0));
    }

    #[test]
    fn discovery_response_round_trip() {
        let response = IpDiscovery::new(7, "203.0.113.9".to_owned(), 50_000);
        let packet = response.encode().expect("short address");
        assert_eq!(&packet[..4], &[0x00, 0x02, 0x00, 0x46]);
        assert_eq!(&packet[8..19], b"203.0.113.9");
        assert_eq!(packet[19], 0);
        assert_eq!(&packet[72..], &[0xC3, 0x50]);
        assert_eq!(classify(&packet), PacketKind::IpDiscovery);
        assert_eq!(IpDiscovery::decode(&packet), Ok(response));

        assert_eq!(
            IpDiscovery::decode(&ip_discovery_request(7)),
            Err(PacketError::NotDiscoveryResponse)
        );
        assert!(IpDiscovery::decode(&packet[..10]).is_err());
    }

    #[test]
    fn rtp_header_round_trip() {
        let header = RtpHeader::new(0xABCD, 0x0102_0304, 0x0A0B_0C0D);
        let bytes = header.encode();
        assert_eq!(
            bytes,
            [
                0x80, 0x78, 0xAB, 0xCD, 0x01, 0x02, 0x03, 0x04, 0x0A, 0x0B, 0x0C, 0x0D
            ]
        );
        assert_eq!(RtpHeader::decode(&bytes), Ok(header));
        assert_eq!(header.unencrypted_len(), 12);
    }

    #[test]
    fn rtp_with_csrc_and_extension() {
        // Version 2, padding, extension, 2 CSRCs; marker with payload type 120.
        let mut packet = vec![0xB2, 0xF8, 0, 1, 0, 0, 0, 2, 0, 0, 0, 3];
        packet.extend_from_slice(&[0, 0, 0, 9, 0, 0, 0, 10]);
        packet.extend_from_slice(&[0xBE, 0xDE, 0x00, 0x02]);
        packet.extend_from_slice(&[0xEE; 5]);
        let parsed = RtpPacket::parse(&packet).expect("valid");
        assert!(parsed.header.padding);
        assert!(parsed.header.extension);
        assert!(parsed.header.marker);
        assert_eq!(parsed.header.csrc_count, 2);
        assert_eq!(parsed.header.payload_type, 120);
        assert_eq!(parsed.header_bytes.len(), 24);
        assert_eq!(parsed.extension_words, Some(2));
        assert_eq!(parsed.body, &[0xEE; 5]);
        assert!(RtpPacket::parse(&packet[..20]).is_err());
    }

    #[test]
    fn classification() {
        let mut rtcp = [0_u8; 16];
        rtcp[0] = 0x81;
        for payload_type in 200..=206 {
            rtcp[1] = payload_type;
            assert_eq!(classify(&rtcp), PacketKind::Rtcp);
        }
        let rtp = RtpHeader::new(1, 2, 3).encode();
        assert_eq!(classify(&rtp), PacketKind::Rtp);
        assert_eq!(
            classify(&[0x13, 0x37, 0xCA, 0xFE, 0, 0, 0, 0]),
            PacketKind::Keepalive
        );
        assert_eq!(classify(&[0x80, 0x78]), PacketKind::Unknown);
        assert_eq!(classify(&[0x00; 20]), PacketKind::Unknown);
    }
}
