use super::binrw_error;
use super::{
    ResponseStatus, WORD,
    identity::{Command, Endpoint},
};
use crate::protocol::{decode, encode};
use crate::{Error, Result};
use binrw::{BinRead, BinResult, BinWrite, Endian};
use std::{
    fmt,
    io::{Cursor, Read, Seek, Write},
};

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
#[derive(Clone, Copy, Debug, PartialEq, Eq, BinRead, BinWrite)]
pub struct Flags(pub u8);
impl Flags {
    pub const REQUEST: Self = Self(0x40);
    pub const RESPONSE: Self = Self(0x80);
    pub const ACKNOWLEDGMENT: Self = Self(0xc0);
    pub fn is_response(self) -> bool {
        self.0 & Self::RESPONSE.0 != 0
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

#[derive(BinRead, BinWrite)]
#[brw(little)]
pub(crate) struct DumlPrefix {
    pub sync: u8,
    pub length: u16,
    pub checksum: u8,
}

#[derive(BinRead, BinWrite)]
#[brw(little)]
struct DumlFields {
    sender: Endpoint,
    receiver: Endpoint,
    id: u16,
    flags: Flags,
    command: Command,
}

pub(crate) struct MessageMetadata<'a>(&'a Message);

impl fmt::Display for MessageMetadata<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = self.0;
        let (set, command) = message.command.wire();
        write!(
            formatter,
            "sender={:#04x} receiver={:#04x} set={set:#04x} command={command:#04x} id={:#06x} flags={:#04x} payload_length={}",
            encode(&message.sender).expect("one-byte enum encodes into memory")[0],
            encode(&message.receiver).expect("one-byte enum encodes into memory")[0],
            message.id,
            message.flags.0,
            message.payload.len()
        )
    }
}

impl Message {
    pub(crate) fn diagnostic_metadata(&self) -> MessageMetadata<'_> {
        MessageMetadata(self)
    }

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
    pub fn scan(bytes: &[u8]) -> Vec<Self> {
        let mut result = Vec::new();
        let mut cursor = 0;
        while bytes.len() - cursor >= DUML_MIN {
            let mut reader = Cursor::new(&bytes[cursor..]);
            match Self::read_options(&mut reader, Endian::Little, ()) {
                Ok(message) => {
                    cursor += DUML_MIN + message.payload.len();
                    result.push(message);
                }
                Err(_) => cursor += 1,
            }
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
            .map(|value| decode::<ResponseStatus>(&[value], ()))
            .transpose()?
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

impl BinWrite for Message {
    type Args<'a> = ();

    fn write_options<W: Write + Seek>(&self, writer: &mut W, _: Endian, (): ()) -> BinResult<()> {
        let pos = writer.stream_position()?;
        if self.payload.len() > usize::from(DUML_LENGTH_MASK) - DUML_MIN {
            return Err(binrw_error(
                pos,
                Error::LimitExceeded("DUML payload exceeds wire limit"),
            ));
        }
        let length = DUML_MIN + self.payload.len();
        let mut frame = Cursor::new(Vec::with_capacity(length));
        (DUML_SYNC, DUML_VERSION | length as u16).write_options(&mut frame, Endian::Little, ())?;
        let header_checksum = crc8(frame.get_ref());
        header_checksum.write_options(&mut frame, Endian::Little, ())?;
        DumlFields {
            sender: self.sender,
            receiver: self.receiver,
            id: self.id,
            flags: self.flags,
            command: self.command,
        }
        .write_options(&mut frame, Endian::Little, ())?;
        frame.write_all(&self.payload)?;
        let checksum = crc16(frame.get_ref());
        checksum.write_options(&mut frame, Endian::Little, ())?;
        writer.write_all(frame.get_ref())?;
        Ok(())
    }
}

impl BinRead for Message {
    type Args<'a> = ();

    fn read_options<R: Read + Seek>(reader: &mut R, _: Endian, (): ()) -> BinResult<Self> {
        let pos = reader.stream_position()?;
        let mut prefix_bytes = [0; DUML_CRC8 + 1];
        reader.read_exact(&mut prefix_bytes)?;
        let prefix = DumlPrefix::read_options(&mut Cursor::new(prefix_bytes), Endian::Little, ())?;
        if prefix.sync != DUML_SYNC {
            return Err(binrw_error(pos, Error::Protocol("invalid DUML sync byte")));
        }
        let header = prefix.length;
        let length = usize::from(header & DUML_LENGTH_MASK);
        if header & !DUML_LENGTH_MASK != DUML_VERSION || length < DUML_MIN {
            return Err(binrw_error(
                pos,
                Error::Protocol("invalid DUML frame length"),
            ));
        }
        if crc8(&prefix_bytes[..DUML_CRC8]) != prefix.checksum {
            return Err(binrw_error(
                pos,
                Error::Protocol("invalid DUML header checksum"),
            ));
        }
        let mut frame = vec![0; length];
        frame[..prefix_bytes.len()].copy_from_slice(&prefix_bytes);
        reader.read_exact(&mut frame[prefix_bytes.len()..])?;
        let fields = DumlFields::read_options(
            &mut Cursor::new(&frame[prefix_bytes.len()..DUML_PAYLOAD]),
            Endian::Little,
            (),
        )?;
        let checksum = u16::read_options(
            &mut Cursor::new(&frame[length - WORD..]),
            Endian::Little,
            (),
        )?;
        if crc16(&frame[..length - WORD]) != checksum {
            return Err(binrw_error(
                pos,
                Error::Protocol("invalid DUML frame checksum"),
            ));
        }
        Ok(Self {
            sender: fields.sender,
            receiver: fields.receiver,
            id: fields.id,
            flags: fields.flags,
            command: fields.command,
            payload: frame[DUML_PAYLOAD..length - WORD].to_vec(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{decode, encode};

    #[test]
    fn maximum_frame_unknown_fields_and_composition_round_trip() {
        let message = Message {
            sender: Endpoint::Unknown(0xe1),
            receiver: Endpoint::Unknown(0xe2),
            id: 0x1234,
            flags: Flags(0xa5),
            command: Command::Unknown {
                set: 0xe3,
                id: 0xe4,
            },
            payload: vec![0x5a; usize::from(DUML_LENGTH_MASK) - DUML_MIN],
        };
        let bytes = encode(&message).unwrap();
        assert_eq!(bytes.len(), usize::from(DUML_LENGTH_MASK));
        assert_eq!(decode::<Message>(&bytes, ()).unwrap(), message);
        for length in 0..DUML_MIN {
            assert!(decode::<Message>(&bytes[..length], ()).is_err());
        }
        assert!(decode::<Message>(&bytes[..bytes.len() - 1], ()).is_err());
        let mut extra = bytes.clone();
        extra.push(0);
        assert!(decode::<Message>(&extra, ()).is_err());
        let mut writer = std::io::Cursor::new(Vec::new());
        writer.get_mut().push(0xaa);
        encode(&message)
            .map(|bytes| writer.get_mut().extend(bytes))
            .unwrap();
        assert_eq!(&writer.get_ref().as_slice()[1..], bytes);
        let mut oversized = message;
        oversized.payload.push(0);
        assert!(matches!(encode(&oversized), Err(Error::LimitExceeded(_))));
    }

    #[test]
    fn scan_resynchronizes_after_corrupt_frames_and_retains_concatenated_frames() {
        let message = Message::request(
            Endpoint::Camera,
            7,
            Command::Unknown {
                set: 0xe1,
                id: 0xe2,
            },
            &[1, 2],
        );
        let bytes = encode(&message).unwrap();
        let mut corrupt = bytes.clone();
        corrupt[DUML_PAYLOAD] ^= 1;
        let mut stream = vec![0x11, 0x22, DUML_SYNC];
        stream.extend(corrupt);
        stream.extend(&bytes);
        stream.extend(&bytes);
        assert_eq!(Message::scan(&stream), [message.clone(), message]);
    }
}

#[cfg(test)]
mod diagnostic_tests {
    use super::*;

    #[test]
    fn metadata_contains_headers_and_length_but_no_payload_bytes() {
        let message = Message::request(
            Endpoint::Media,
            123,
            Command::from_wire(8, 0x78),
            b"rtmp://example.invalid/live/synthetic-private-key",
        );
        assert_eq!(
            message.diagnostic_metadata().to_string(),
            "sender=0x02 receiver=0x08 set=0x08 command=0x78 id=0x007b flags=0x40 payload_length=49"
        );
    }
}
