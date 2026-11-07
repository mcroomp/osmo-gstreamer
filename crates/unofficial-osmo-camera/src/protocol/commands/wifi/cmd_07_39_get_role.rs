//! Wi-Fi role query and SET role values.

use crate::protocol::{
    Query,
    commands::reply::{CameraRawData, CameraReadReply},
    identity::Endpoint,
};
use binrw::{BinRead, BinWrite, binwrite};

/// Request the Wi-Fi role; the accepted response layout is unspecified.
///
/// APP -> Wifi | 0x07/0x39 | Role GET
///
/// ```text
/// request { operation: u8 = 0x00 }
/// reply { status: u8 | reject if nonzero
///         data: [u8; remaining] | opaque, not decoded as WifiRole }
/// ```
///
/// The accepted-body layout and model support remain unspecified. In
/// particular, do not interpret an arbitrary returned byte as `WifiRole`.
#[binwrite]
#[derive(Clone, Copy, Debug, Default)]
#[bw(magic = b"\x07\x39")]
pub struct WifiRoleGet {
    #[bw(calc = <Self as Query>::REQUEST_PAYLOAD)]
    payload: &'static [u8],
}

/// Complete command reply, including command identity and status-bearing data.
#[derive(Clone, Debug, PartialEq, Eq, BinRead, BinWrite)]
#[brw(magic = b"\x07\x39")]
pub struct WifiRoleReply {
    pub result: CameraReadReply<CameraRawData>,
}

impl Query for WifiRoleGet {
    type Response = WifiRoleReply;
    const REQUEST_PAYLOAD: &'static [u8] = &[0x00];

    fn receiver(&self) -> Endpoint {
        Endpoint::Wifi
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, BinWrite, BinRead)]
#[brw(repr = u8)]
#[repr(u8)]
pub enum WifiRole {
    Hotspot = 0,
    Station = 1,
}
