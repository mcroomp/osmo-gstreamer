//! Streaming control at media endpoint 0x08, command 02:8E.
//! Camera endpoint 0x01 uses a separate parameter contract.

use crate::{
    Error,
    protocol::{
        Query,
        identity::{CameraCommand, Command, Endpoint},
    },
};
use binrw::{BinRead, BinWrite, binwrite, helpers::until_eof};

/// Requests a streaming control operation at the media endpoint.
#[binwrite]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[bw(magic = b"\x02\x8e")]
pub struct MediaStreamingControlRequest {
    #[bw(calc = [1u8, 1, 0x1a, 0, 1])]
    prefix: [u8; 5],
    pub control: MediaStreamingControl,
    #[bw(calc = [0u8; 2])]
    suffix: [u8; 2],
}

#[binwrite]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[bw(magic = b"\x02\x8e")]
pub struct MediaStreamingStateGet {
    #[bw(calc = <Self as Query>::REQUEST_PAYLOAD)]
    payload: &'static [u8],
}

pub const COMMAND: Command = Command::Camera(CameraCommand::Parameters);

/// Host-facing controls. Zero is not a stop operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, BinWrite)]
#[repr(u8)]
#[bw(repr = u8)]
pub enum MediaStreamingControl {
    Start = 1,
    /// Stops and leaves broadcast mode; not a minimal pause.
    /// Recording and optional service cleanup can also be affected.
    StopAndQuit = 2,
}

/// Complete command reply, including command identity and status-bearing data.
#[derive(Clone, Debug, PartialEq, Eq, BinRead, BinWrite)]
#[brw(magic = b"\x02\x8e")]
pub struct MediaStreamingStateReply {
    pub result: MediaStreamingStateData,
}

impl Query for MediaStreamingStateGet {
    type Response = MediaStreamingStateReply;
    const REQUEST_PAYLOAD: &'static [u8] = &[0x00, 0x01, 0x1c, 0x00];

    fn receiver(&self) -> Endpoint {
        Endpoint::Media
    }
}

/// A structured success has a LE16 result, count 1, ID 001C, length 9.
/// Rejections are retained raw: their layout is not assumed to match success.
#[derive(Clone, Debug, PartialEq, Eq, BinRead, BinWrite)]
#[brw(little)]
pub enum MediaStreamingStateData {
    /// Opaque state bytes. No remote-publication predicate is assigned.
    #[brw(magic = b"\x00\x00\x01\x1c\x00\x09")]
    State([u8; 9]),
    Rejected(
        #[br(parse_with = until_eof)]
        #[brw(assert(!self_0.is_empty(), Error::Protocol("missing media running-state result")))]
        #[brw(assert(self_0[0] != 0 || self_0.get(1).is_some_and(|byte| *byte != 0), Error::Protocol("rejected media state cannot encode success")))]
        Vec<u8>,
    ),
}
