//! Video-caption GET payload and response.

use crate::protocol::{
    Query,
    commands::reply::{CommandReadReply, RemainingBytes},
    identity::Endpoint,
};
use binrw::{BinWrite, binrw};

/// Read the video-caption value.
///
/// APP -> Camera | 0x02/0x63
///
/// ```text
/// request {}
/// reply { status: u8, code: u8, extension: [u8; remaining] }
/// ```
#[derive(Clone, Copy, Debug, Default, BinWrite)]
#[bw(magic = b"\x02\x63")]
pub struct CameraVideoCaptionGet;

/// Accepted video-caption byte, preserving all bits.
///
/// Camera -> APP | 0x02/0x63 | Video caption reply
///
/// ```text
/// reply { status: u8 | handled by CommandReadReply
///         code: u8, extension: [u8; remaining] | opaque }
/// ```
///
/// The Nano reply clears bits 5 and 6; bits 0..4 and 7 remain raw.
/// The decoder preserves every received bit rather than masking it again.
#[binrw]
#[derive(Clone, Debug, PartialEq, Eq)]
#[brw(magic = b"\x02\x63")]
pub struct CameraVideoCaptionReply {
    #[br(temp, assert(status == 0, "accepted reply requires success status"))]
    #[bw(calc = 0u8)]
    status: u8,
    raw: RemainingBytes<1>,
}

impl CameraVideoCaptionReply {
    pub fn raw_bytes(&self) -> &[u8] {
        self.raw.as_slice()
    }

    /// Test bit 7 only; the compatibility name does not confirm overlay behavior.
    pub fn enabled_bit_set(&self) -> bool {
        self.raw_bytes()[0] & 0x80 != 0
    }
}

impl Query for CameraVideoCaptionGet {
    type Response = CommandReadReply<CameraVideoCaptionReply>;

    fn receiver(&self) -> Endpoint {
        Endpoint::Camera
    }
}
