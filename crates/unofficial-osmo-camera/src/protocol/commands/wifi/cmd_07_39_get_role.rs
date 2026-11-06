//! Wi-Fi role query and SET role values.

use crate::protocol::{
    Query,
    commands::reply::{CameraRawData, CameraReadReply},
    identity::{Command, Endpoint, WifiCommand},
};
use binrw::{BinRead, BinWrite};

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
#[derive(Clone, Copy, Debug, BinWrite)]
#[bw(magic = 0u8)]
pub struct WifiRoleGet;

impl Query for WifiRoleGet {
    type Response = CameraReadReply<CameraRawData>;

    fn response_args(&self) {}

    fn receiver(&self) -> Endpoint {
        Endpoint::Wifi
    }
    fn command(&self) -> Command {
        Command::Wifi(WifiCommand::GetRole)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, BinWrite, BinRead)]
#[brw(repr = u8)]
#[repr(u8)]
pub enum WifiRole {
    Hotspot = 0,
    Station = 1,
}
