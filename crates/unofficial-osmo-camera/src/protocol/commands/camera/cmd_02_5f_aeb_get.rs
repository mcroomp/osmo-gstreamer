//! Automatic exposure-bracketing GET payload and response.

use crate::protocol::{
    Query,
    commands::reply::{CommandReadReply, RemainingBytes},
    identity::Endpoint,
};
use binrw::{BinWrite, binrw};

/// Read automatic exposure-bracketing fields.
///
/// APP -> Camera | 0x02/0x5F
///
/// ```text
/// request {}
/// reply { status: u8, first_code: u8, second_code: u8,
///         extension: [u8; remaining] }
/// ```
#[derive(Clone, Copy, Debug, Default, BinWrite)]
#[bw(magic = b"\x02\x5f")]
pub struct CameraAebGet;

/// Two unchanged bracketing code bytes, without count/step or enum meanings.
///
/// Camera -> APP | 0x02/0x5F | AEB reply
///
/// ```text
/// reply { status: u8 | handled by CommandReadReply
///         first_code: u8, second_code: u8,
///         extension: [u8; remaining] | opaque }
/// ```
#[binrw]
#[derive(Clone, Debug, PartialEq, Eq)]
#[brw(magic = b"\x02\x5f")]
pub struct CameraAebReply {
    #[br(temp, assert(status == 0, "accepted reply requires success status"))]
    #[bw(calc = 0u8)]
    status: u8,
    raw: RemainingBytes<2>,
}

impl CameraAebReply {
    pub fn raw_bytes(&self) -> &[u8] {
        self.raw.as_slice()
    }

    pub fn codes(&self) -> [u8; 2] {
        [self.raw_bytes()[0], self.raw_bytes()[1]]
    }
}

impl Query for CameraAebGet {
    type Response = CommandReadReply<CameraAebReply>;

    fn receiver(&self) -> Endpoint {
        Endpoint::Camera
    }
}
