//! Camera-language GET payload and response.

use crate::{
    Error,
    protocol::{
        Query,
        commands::reply::{CameraRawData, CommandReadReply},
        identity::Endpoint,
    },
};
use binrw::{BinWrite, binrw};

/// Request the camera-language code; a one-byte rejection needs no data body.
///
/// An accepted language mapping and model support have not been established.
///
/// APP -> Camera | 0x02/0x57
///
/// ```text
/// request {}
/// reply { status: u8, code: u8, extension: [u8; remaining] }
/// ```
#[derive(Clone, Copy, Debug, Default, BinWrite)]
#[bw(magic = b"\x02\x57")]
pub struct CameraLanguageGet;

/// Compatibility decoder for a one-byte language code; support is unconfirmed.
///
/// Camera -> APP | 0x02/0x57 | Language reply
///
/// ```text
/// reply { status: u8 | handled by CommandReadReply
///         code: u8, extension: [u8; remaining] | opaque }
/// ```
#[binrw]
#[derive(Clone, Debug, PartialEq, Eq)]
#[brw(magic = b"\x02\x57")]
pub struct CameraLanguageReply {
    #[br(temp, assert(status == 0, "accepted reply requires success status"))]
    #[bw(calc = 0u8)]
    status: u8,
    #[br(assert(!raw.0.is_empty(), Error::Protocol("truncated CameraLanguageReply")))]
    raw: CameraRawData,
}

impl CameraLanguageReply {
    pub fn raw_bytes(&self) -> &[u8] {
        &self.raw.0
    }

    /// Signed view of the raw byte, without assigning a language or sentinel meaning.
    pub fn signed_code(&self) -> i8 {
        self.raw_bytes()[0] as i8
    }
}

impl Query for CameraLanguageGet {
    type Response = CommandReadReply<CameraLanguageReply>;

    fn receiver(&self) -> Endpoint {
        Endpoint::Camera
    }
}
