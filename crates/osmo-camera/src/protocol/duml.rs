use super::{
    ResponseStatus, WORD, WireDecode, WireEncode, WireReader, WireWriter,
    identity::{Command, Endpoint},
    u16_at,
};
use crate::{Error, Result};

pub const DUML_SYNC: u8 = 0x55;
pub const DUML_VERSION: u16 = 0x400;
pub const DUML_LENGTH_MASK: u16 = 0x3ff;
pub const DUML_LENGTH: usize = 1;
pub const DUML_CRC8: usize = 3;
pub const DUML_SENDER: usize = 4;
pub const DUML_RECEIVER: usize = 5;
pub const DUML_ID: usize = 6;
pub const DUML_FLAGS: usize = 8;
pub const DUML_SET: usize = 9;
pub const DUML_COMMAND: usize = 10;
pub const DUML_PAYLOAD: usize = 11;
pub const DUML_MIN: usize = DUML_PAYLOAD + WORD;
pub const CRC8_SEED: u8 = 0x77;
pub const CRC8_POLYNOMIAL: u8 = 0x8c;
pub const CRC16_SEED: u16 = 0x3692;
pub const CRC16_POLYNOMIAL: u16 = 0x8408;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Flags(pub u8);
impl Flags {
    pub const REQUEST: Self = Self(0x40);
    pub const RESPONSE: Self = Self(0x80);
    pub const ACKNOWLEDGMENT: Self = Self(0xc0);
    pub fn is_response(self) -> bool {
        self.0 & Self::RESPONSE.0 != 0
    }
}
impl WireEncode for Flags {
    fn write_to(&self, writer: &mut WireWriter) -> Result<()> {
        writer.write_u8(self.0);
        Ok(())
    }
}
impl WireDecode for Flags {
    fn read_from(reader: &mut WireReader<'_>, _context: &()) -> Result<Self> {
        Ok(Self(reader.read_u8()?))
    }
}

pub fn crc8(bytes: &[u8]) -> u8 {
    let mut crc = CRC8_SEED;
    for byte in bytes {
        crc ^= byte;
        for _ in 0..u8::BITS {
            crc = (crc >> 1) ^ if crc & 1 != 0 { CRC8_POLYNOMIAL } else { 0 };
        }
    }
    crc
}
pub fn crc16(bytes: &[u8]) -> u16 {
    let mut crc = CRC16_SEED;
    for byte in bytes {
        crc ^= u16::from(*byte);
        for _ in 0..u8::BITS {
            crc = (crc >> 1) ^ if crc & 1 != 0 { CRC16_POLYNOMIAL } else { 0 };
        }
    }
    crc
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    pub sender: Endpoint,
    pub receiver: Endpoint,
    pub id: u16,
    pub flags: Flags,
    pub command: Command,
    pub payload: Vec<u8>,
}
impl Message {
    pub fn request(receiver: Endpoint, id: u16, command: Command, payload: &[u8]) -> Self {
        Self {
            sender: Endpoint::App,
            receiver,
            id,
            flags: Flags::REQUEST,
            command,
            payload: payload.to_vec(),
        }
    }
    pub fn encode(&self) -> Result<Vec<u8>> {
        <Self as WireEncode>::encode(self)
    }
    pub fn scan(bytes: &[u8]) -> Vec<Self> {
        let mut result = Vec::new();
        let mut cursor = 0;
        while bytes.len() - cursor >= DUML_MIN {
            let frame = &bytes[cursor..];
            let header = u16_at(frame, DUML_LENGTH);
            let length = usize::from(header & DUML_LENGTH_MASK);
            if frame[0] != DUML_SYNC
                || header & !DUML_LENGTH_MASK != DUML_VERSION
                || length < DUML_MIN
                || length > frame.len()
                || crc8(&frame[..DUML_CRC8]) != frame[DUML_CRC8]
                || crc16(&frame[..length - WORD]) != u16_at(frame, length - WORD)
            {
                cursor += 1;
                continue;
            }
            result.push(Self {
                sender: Endpoint::from(frame[DUML_SENDER]),
                receiver: Endpoint::from(frame[DUML_RECEIVER]),
                id: u16_at(frame, DUML_ID),
                flags: Flags(frame[DUML_FLAGS]),
                command: Command::from_wire(frame[DUML_SET], frame[DUML_COMMAND]),
                payload: frame[DUML_PAYLOAD..length - WORD].to_vec(),
            });
            cursor += length;
        }
        result
    }
    pub fn is_reply_to(&self, request: &Self) -> bool {
        self.sender == request.receiver
            && self.receiver == request.sender
            && self.id == request.id
            && self.command == request.command
            && self.flags.is_response()
    }
    pub fn require_success(&self) -> Result<()> {
        if !self.flags.is_response() {
            return Err(Error::InvalidReply {
                command: self.command,
                reason: "missing response flag",
            });
        }
        let status = self
            .payload
            .first()
            .copied()
            .map(ResponseStatus::from)
            .ok_or(Error::InvalidReply {
                command: self.command,
                reason: "missing status byte",
            })?;
        match status {
            ResponseStatus::Success => Ok(()),
            status => Err(Error::CameraRejected {
                command: self.command,
                status,
            }),
        }
    }
}

impl WireEncode for Message {
    fn write_to(&self, writer: &mut WireWriter) -> Result<()> {
        let length = DUML_MIN + self.payload.len();
        if length > usize::from(DUML_LENGTH_MASK) {
            return Err(Error::LimitExceeded("DUML payload exceeds wire limit"));
        }
        let start = writer.as_slice().len();
        writer.write_u8(DUML_SYNC);
        writer.write_u16_le(DUML_VERSION | length as u16);
        let header_checksum = crc8(&writer.as_slice()[start..start + DUML_CRC8]);
        writer.write_u8(header_checksum);
        self.sender.write_to(writer)?;
        self.receiver.write_to(writer)?;
        writer.write_u16_le(self.id);
        self.flags.write_to(writer)?;
        self.command.write_to(writer)?;
        writer.write_bytes(&self.payload);
        let checksum = crc16(&writer.as_slice()[start..]);
        writer.write_u16_le(checksum);
        Ok(())
    }
}

impl WireDecode for Message {
    fn read_from(reader: &mut WireReader<'_>, _context: &()) -> Result<Self> {
        let remaining = reader.remaining();
        if remaining.len() < DUML_MIN {
            return Err(Error::Protocol("truncated DUML frame"));
        }
        let mut header_reader = WireReader::new(remaining);
        if header_reader.read_u8()? != DUML_SYNC {
            return Err(Error::Protocol("invalid DUML sync byte"));
        }
        let header = header_reader.read_u16_le()?;
        let length = usize::from(header & DUML_LENGTH_MASK);
        if header & !DUML_LENGTH_MASK != DUML_VERSION || length < DUML_MIN {
            return Err(Error::Protocol("invalid DUML frame length"));
        }
        if length > remaining.len() {
            return Err(Error::Protocol("truncated DUML frame"));
        }
        let frame = reader.read_bytes(length)?;
        if crc8(&frame[..DUML_CRC8]) != frame[DUML_CRC8] {
            return Err(Error::Protocol("invalid DUML header checksum"));
        }
        let mut frame_reader = WireReader::new(frame);
        frame_reader.read_u8()?;
        frame_reader.read_u16_le()?;
        frame_reader.read_u8()?;
        let sender = Endpoint::read_from(&mut frame_reader, &())?;
        let receiver = Endpoint::read_from(&mut frame_reader, &())?;
        let id = frame_reader.read_u16_le()?;
        let flags = Flags::read_from(&mut frame_reader, &())?;
        let command = Command::read_from(&mut frame_reader, &())?;
        let payload = frame_reader.read_bytes(length - DUML_MIN)?.to_vec();
        let checksum = frame_reader.read_u16_le()?;
        frame_reader.finish()?;
        if crc16(&frame[..length - WORD]) != checksum {
            return Err(Error::Protocol("invalid DUML frame checksum"));
        }
        Ok(Self {
            sender,
            receiver,
            id,
            flags,
            command,
            payload,
        })
    }
}
