use super::{WireDecode, WireEncode, WireReader, WireU8, WireWriter};
use crate::{Error, Result};
use num_enum::{FromPrimitive, IntoPrimitive, TryFromPrimitive};

pub const UDP_PORT: u16 = 9004;
pub const PAIRING_PORT: u16 = 7001;
pub const UDP_HEADER: usize = 8;
pub const UDP_LENGTH_MARKER: u16 = 0x8000;
pub const UDP_LENGTH_MASK: u16 = 0x3fff;
pub const SESSION_OFFSET: usize = 2;
pub const SEQUENCE_OFFSET: usize = 4;
pub const TYPE_OFFSET: usize = 6;
pub const UDP_CHECKSUM_OFFSET: usize = 7;
pub const WORD: usize = 2;
pub const SEQUENCE_SHIFT: u32 = 3;
pub const SEQUENCE_STEP: u16 = 8;
pub const INDEX_MASK: u16 = 0x1fff;
pub const INDEX_BITS: u16 = 0xfff8;
pub const HALF_CYCLE: u16 = 0x1000;
pub const RETRY_SHIFT: u32 = 1;
pub const RETRY_MASK: u16 = 6;
pub const RETRY_VALUE_MASK: u16 = 3;
pub const RETRY_MARKER: u16 = 1;
pub const CONTROL_CAPACITY: u16 = 20;
pub const VIDEO_CAPACITY: u16 = 100;
pub const WINDOW_LENGTH: usize = 8;
pub const LEFT: usize = 0;
pub const RIGHT: usize = WORD;
pub const FIRST_RETRY: usize = WORD * 2;
pub const SECOND_RETRY: usize = WORD * 3;
pub const ROUTING_LENGTH: usize = WINDOW_LENGTH + 4;
pub const ROUTING_COUNTER: usize = WINDOW_LENGTH;
pub const ROUTING_CHANNEL: usize = 9;
pub const CONTROL_CHANNEL: u8 = 1;
pub const CONTROL_ACK_OFFSET: usize = WINDOW_LENGTH * 2;
pub const ACK_COUNT: usize = CONTROL_ACK_OFFSET + WORD * 2;
pub const ACK_LIST: usize = ACK_COUNT + WORD;
pub const ACK_DISPLACEMENT: usize = ACK_COUNT + WORD;
pub const ACK_BITMAP: usize = ACK_DISPLACEMENT + WORD;
pub const ACK_STATE_BITS: usize = 2;
pub const ACK_STATES_PER_BYTE: usize = 4;
pub const ACK_STATE_MASK: u8 = 3;
pub const ACK_RECEIVED: u8 = 3;
pub const RECEIVE_LIST_HEADER: usize = WORD * 3;
pub const RECEIVE_COMPRESSED_HEADER: usize = WORD * 4;
pub const FEC_OFFSET: usize = UDP_HEADER + WINDOW_LENGTH;
pub const VIDEO_PAYLOAD: usize = FEC_OFFSET + 4;
pub const FEC_ENABLED: u32 = 0x4000;
pub const FEC_COUNT_SHIFT: u32 = 8;
pub const FEC_INDEX_SHIFT: u32 = 15;
pub const FEC_LENGTH_SHIFT: u32 = 21;
pub const FEC_FIELD_MASK: u32 = 63;
pub const SINGLE_PARITY: u32 = 1;
pub const MULTIPLE_PARITY: u32 = 2;
pub const MAX_DATAGRAM: usize = 1472;
pub const MAX_MEDIA: usize = (1 << 21) - 1;
pub const ENVELOPE_HEADER: usize = 12;
pub const ENVELOPE_PREFIX: [u8; 4] = [0, 0, 1, 0xff];
pub const ENVELOPE_LENGTH: usize = 4;
pub const ENVELOPE_EXTENSION: usize = 8;
pub const ENVELOPE_TYPE: usize = 9;
pub const ENVELOPE_CHECKSUM: usize = 11;
pub const EXTENSION_MASK: u8 = 0xc0;
pub const EXTENSION_MARKER: u8 = 0x80;
pub const EXTENSION_LENGTH_MASK: u8 = 63;
pub const METADATA_HEADER: usize = ENVELOPE_HEADER + 4;
pub const PRIVATE_SEI: &[u8] = &[0, 0, 1, 6, 0xf0, 0x19];
pub const PRIVATE_SEI_DATA: usize = 25;
pub const PRIVATE_SEI_END: u8 = 0x80;
pub const TRAILING_AUD: &[u8] = &[0, 0, 0, 1, 9, 0x10];
pub const NAL_FORBIDDEN: u8 = 0x80;
pub const NAL_REFERENCE: u8 = 0x60;
pub const NAL_TYPE_MASK: u8 = 31;
pub const HEVC_TYPE_SHIFT: u32 = 1;
pub const HEVC_TYPE_MASK: u8 = 63;
pub const HEVC_TEMPORAL_MASK: u8 = 7;
pub const SPS_PROBE: usize = 4;
pub const PPS_PROBE: usize = 2;
pub const MIN_PREFIX_ZEROS: usize = 2;
pub const MAX_PREFIX_ZEROS: usize = 3;
pub const PREFIX_END: u8 = 1;
pub const EMULATION_PREVENTION: u8 = 3;
pub const SLICE_TYPE_COUNT: u32 = 5;
pub const B_SLICE: u32 = 1;
pub const MAX_SLICE_TYPE: u32 = 9;
pub const FIRST_ID: u16 = 0xa000;
pub const MIN_ID: u16 = 0x8000;
pub const MAX_ID: u16 = 0xfffe;
pub const INITIAL_SESSION: u16 = 0x1000;
pub const SYN_BASE: usize = 0;
pub const SYN_CAPACITY: usize = 24;
pub const SYN_PARAMETERS: &[u8] = &[
    0, 0, 100, 0, 100, 0, 0xc0, 5, 20, 0, 0, 100, 0, 0, 1, 0x90, 1, 0xc0, 5, 20, 0, 0, 100, 0, 20,
    0, 100, 0, 0xc0, 5, 20, 0, 0, 100, 0, 1, 1, 4, 1, 2,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, FromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum ResponseStatus {
    Success = 0,
    #[num_enum(catch_all)]
    Unknown(u8),
}
impl std::fmt::Display for ResponseStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:02X}", self.to_wire_u8())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
#[num_enum(error_type(name = Error, constructor = MediaType::invalid_wire))]
#[repr(u8)]
pub enum MediaType {
    Video = 0x11,
    Command = 0x12,
    Audio = 0x24,
}
impl MediaType {
    fn invalid_wire(value: u8) -> Error {
        Error::UnsupportedValue {
            field: "media envelope type",
            value,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, FromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum PacketType {
    Handshake = 0,
    StreamWindow = 1,
    Video = 2,
    Download = 3,
    Acknowledgment = 4,
    RoutedCommand = 5,
    ListAcknowledgment = 6,
    #[num_enum(catch_all)]
    Unknown(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
#[num_enum(error_type(name = Error, constructor = RetryPhase::invalid_wire))]
#[repr(u16)]
pub enum RetryPhase {
    Initial = 0,
    First = 1,
    Second = 2,
    Final = 3,
}
impl RetryPhase {
    fn invalid_wire(_value: u16) -> Error {
        Error::Protocol("invalid retry phase")
    }
    pub fn of(sequence: u16) -> Self {
        match (sequence >> RETRY_SHIFT) & RETRY_VALUE_MASK {
            value if value == Self::Initial as u16 => Self::Initial,
            value if value == Self::First as u16 => Self::First,
            value if value == Self::Second as u16 => Self::Second,
            _ => Self::Final,
        }
    }
}
impl WireEncode for RetryPhase {
    fn write_to(&self, writer: &mut WireWriter) -> Result<()> {
        writer.write_u16_le((*self).into());
        Ok(())
    }
}
impl WireDecode for RetryPhase {
    fn read_from(reader: &mut WireReader<'_>, _context: &()) -> Result<Self> {
        Self::try_from(reader.read_u16_le()?)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
#[num_enum(error_type(name = Error, constructor = AckEncoding::invalid_wire))]
#[repr(u8)]
pub enum AckEncoding {
    List = 0,
    Compressed = 1,
}
impl AckEncoding {
    fn invalid_wire(value: u8) -> Error {
        Error::UnsupportedValue {
            field: "negotiated ACK encoding",
            value,
        }
    }
    pub fn parse(value: u8) -> Result<Self> {
        Self::from_wire_u8(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
#[num_enum(error_type(name = Error, constructor = AvcNal::invalid_wire))]
#[repr(u8)]
pub enum AvcNal {
    Slice = 1,
    Idr = 5,
    Sps = 7,
    Pps = 8,
    Aud = 9,
}
impl AvcNal {
    fn invalid_wire(value: u8) -> Error {
        Error::UnsupportedValue {
            field: "AVC NAL type",
            value,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
#[num_enum(error_type(name = Error, constructor = HevcNal::invalid_wire))]
#[repr(u8)]
pub enum HevcNal {
    Vps = 32,
    Sps = 33,
    Pps = 34,
}
impl HevcNal {
    fn invalid_wire(value: u8) -> Error {
        Error::UnsupportedValue {
            field: "HEVC NAL type",
            value,
        }
    }
}

pub fn u16_at(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}
pub fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("checked wire length"),
    )
}
pub fn put16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + WORD].copy_from_slice(&value.to_le_bytes());
}
pub fn distance(earlier: u16, later: u16) -> u16 {
    (later >> SEQUENCE_SHIFT).wrapping_sub(earlier >> SEQUENCE_SHIFT) & INDEX_MASK
}
pub fn next_sequence(sequence: u16) -> u16 {
    sequence.wrapping_add(SEQUENCE_STEP)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UdpDatagram {
    pub kind: PacketType,
    pub session: u16,
    pub sequence: u16,
    pub payload: Vec<u8>,
}

impl WireEncode for UdpDatagram {
    fn write_to(&self, writer: &mut WireWriter) -> Result<()> {
        let length = UDP_HEADER + self.payload.len();
        if length > usize::from(UDP_LENGTH_MASK) {
            return Err(Error::LimitExceeded("UDP payload exceeds wire limit"));
        }

        let start = writer.as_slice().len();
        writer.write_u16_le(UDP_LENGTH_MARKER | length as u16);
        writer.write_u16_le(self.session);
        writer.write_u16_le(self.sequence);
        self.kind.write_to(writer)?;
        let checksum = writer.as_slice()[start..]
            .iter()
            .fold(0, |checksum, byte| checksum ^ byte);
        writer.write_u8(checksum);
        writer.write_bytes(&self.payload);
        Ok(())
    }
}

impl WireDecode<u16> for UdpDatagram {
    fn read_from(reader: &mut WireReader<'_>, expected_session: &u16) -> Result<Self> {
        let remaining = reader.remaining();
        if remaining.len() < UDP_HEADER {
            return Err(Error::Protocol("truncated UDP datagram header"));
        }

        let mut header = WireReader::new(remaining);
        let marker = header.read_u16_le()?;
        let length = usize::from(marker & UDP_LENGTH_MASK);
        if marker & !UDP_LENGTH_MASK != UDP_LENGTH_MARKER
            || length < UDP_HEADER
            || length > remaining.len()
        {
            return Err(Error::Protocol("invalid UDP datagram length marker"));
        }

        let session = header.read_u16_le()?;
        let sequence = header.read_u16_le()?;
        let kind = PacketType::read_from(&mut header, &())?;
        let checksum = header.read_u8()?;
        if remaining[..UDP_CHECKSUM_OFFSET]
            .iter()
            .fold(0, |checksum, byte| checksum ^ byte)
            != checksum
        {
            return Err(Error::Protocol("invalid UDP datagram checksum"));
        }
        if session != *expected_session {
            return Err(Error::Protocol("UDP datagram belongs to another session"));
        }

        reader.read_bytes(UDP_HEADER)?;
        let payload = reader.read_bytes(length - UDP_HEADER)?.to_vec();
        Ok(Self {
            kind,
            session,
            sequence,
            payload,
        })
    }
}

pub fn datagram(kind: PacketType, session: u16, sequence: u16, payload: &[u8]) -> Result<Vec<u8>> {
    UdpDatagram {
        kind,
        session,
        sequence,
        payload: payload.to_vec(),
    }
    .encode()
}
pub fn valid_datagram(bytes: &[u8], session: u16) -> bool {
    <UdpDatagram as WireDecode<u16>>::decode(bytes, &session).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn udp_datagram_encodes_and_decodes_exact_wire_fields() {
        let datagram = UdpDatagram {
            kind: PacketType::Unknown(0xe7),
            session: 0x1234,
            sequence: 0xabcd,
            payload: vec![0x10, 0x20, 0x30],
        };
        let encoded = datagram.encode().unwrap();
        assert_eq!(
            encoded,
            [
                0x0b, 0x80, 0x34, 0x12, 0xcd, 0xab, 0xe7, 0x2c, 0x10, 0x20, 0x30
            ]
        );
        assert_eq!(
            UdpDatagram::decode(&encoded, &datagram.session).unwrap(),
            datagram
        );
        assert!(UdpDatagram::decode(&encoded, &0x9999).is_err());
        assert!(UdpDatagram::decode(&encoded[..encoded.len() - 1], &datagram.session).is_err());
        let mut corrupt = encoded.clone();
        corrupt[UDP_CHECKSUM_OFFSET] ^= 1;
        assert!(UdpDatagram::decode(&corrupt, &datagram.session).is_err());
        assert!(!valid_datagram(&encoded, 0x9999));
    }

    #[test]
    fn strict_status_and_media_values_reject_unclassified_bytes() {
        assert_eq!(
            ResponseStatus::from(u8::MAX),
            ResponseStatus::Unknown(u8::MAX)
        );
        assert!(MediaType::try_from(u8::MAX).is_err());
        assert!(AckEncoding::parse(u8::MAX).is_err());
    }
}
