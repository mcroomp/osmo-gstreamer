//! Auto-exposure lock GET payload and response.

use crate::protocol::{
    Query,
    commands::reply::{CommandReadReply, RemainingBytes},
    identity::Endpoint,
};
use binrw::{BinWrite, binrw};

/// Read the auto-exposure lock value.
///
/// APP -> Camera | 0x02/0x69
///
/// ```text
/// request {}
/// reply { status: u8, code: u8, extension: [u8; remaining] }
/// ```
#[derive(Clone, Copy, Debug, Default, BinWrite)]
#[bw(magic = b"\x02\x69")]
pub struct CameraAeLockGet;

/// Accepted auto-exposure-lock byte, preserving all bits.
///
/// Camera -> APP | 0x02/0x69 | AE lock reply
///
/// ```text
/// reply { status: u8 | handled by CommandReadReply
///         code: u8, extension: [u8; remaining] | opaque }
/// ```
///
/// The Nano reply retains bits 0..5 and clears bits 6..7. Values are raw
/// codes: neither nonzero nor the low two bits establish a lock-state enum.
/// The decoder retains all received bits unchanged.
#[binrw]
#[derive(Clone, Debug, PartialEq, Eq)]
#[brw(magic = b"\x02\x69")]
pub struct CameraAeLockReply {
    #[br(temp, assert(status == 0, "accepted reply requires success status"))]
    #[bw(calc = 0u8)]
    status: u8,
    raw: RemainingBytes<1>,
}

impl CameraAeLockReply {
    pub fn raw_bytes(&self) -> &[u8] {
        self.raw.as_slice()
    }

    /// Test only the low two bits; this is not a confirmed physical lock state.
    pub fn lock_bits_set(&self) -> bool {
        self.raw_bytes()[0] & 3 != 0
    }
}

impl Query for CameraAeLockGet {
    type Response = CommandReadReply<CameraAeLockReply>;

    fn receiver(&self) -> Endpoint {
        Endpoint::Camera
    }
}
