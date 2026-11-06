//! Streaming control at media endpoint 0x08, command 02:8E.
//! Camera endpoint 0x01 uses a separate parameter contract.
use crate::protocol::commands::reply::CameraRawData;
use binrw::{BinRead, BinWrite};

use crate::{
    Error, Result,
    protocol::{
        Query,
        identity::{CameraCommand, Command, Endpoint},
    },
};

pub const COMMAND: Command = Command::Camera(CameraCommand::Parameters);

/// Host-facing controls. Zero is not a stop operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, BinWrite)]
#[repr(u8)]
pub enum MediaStreamingControl {
    #[bw(magic = b"\x01\x01\x1a\x00\x01\x01\x00\x00")]
    Start = 1,
    /// Stops and leaves broadcast mode; not a minimal pause.
    /// Recording and optional service cleanup can also be affected.
    #[bw(magic = b"\x01\x01\x1a\x00\x01\x02\x00\x00")]
    StopAndQuit = 2,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, BinWrite)]
#[bw(magic = b"\x00\x01\x1c\x00")]
pub struct MediaStreamingStateGet;

impl Query for MediaStreamingStateGet {
    type Response = MediaStreamingStateReply;

    fn response_args(&self) {}

    fn receiver(&self) -> Endpoint {
        Endpoint::Media
    }

    fn command(&self) -> Command {
        COMMAND
    }
}

/// A structured success has a LE16 result, count 1, ID 001C, length 9.
/// Rejections are retained raw: their layout is not assumed to match success.
#[derive(Clone, Debug, PartialEq, Eq, BinRead)]
#[br(try_map = Self::from_raw)]
pub enum MediaStreamingStateReply {
    /// Opaque state bytes. No remote-publication predicate is assigned.
    State([u8; 9]),
    Rejected(Vec<u8>),
}

impl MediaStreamingStateReply {
    fn from_raw(raw: CameraRawData) -> Result<Self> {
        let bytes = raw.0;
        let Some(&first) = bytes.first() else {
            return Err(Error::Protocol("missing media running-state result"));
        };
        if first != 0 || bytes.get(1).is_some_and(|byte| *byte != 0) {
            return Ok(Self::Rejected(bytes));
        }
        if bytes.len() != 15 || bytes[..6] != [0, 0, 1, 0x1c, 0, 9] {
            return Err(Error::Protocol(
                "invalid media running-state success layout",
            ));
        }
        let mut state = [0; 9];
        state.copy_from_slice(&bytes[6..]);
        Ok(Self::State(state))
    }
}
