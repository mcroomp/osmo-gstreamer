//! Wi-Fi SSID GET.

use super::packed_string::{read_packed_string, write_packed_string};
use crate::protocol::{
    Query,
    commands::reply::{CameraRawData, CameraReadReply},
    identity::{Command, Endpoint, WifiCommand},
};
use binrw::{BinRead, BinWrite};

pub const MAX_SSID_BYTES: usize = 32;

/// Read the camera's own Wi-Fi SSID, not its joined-router SSID.
///
/// APP -> Wifi | 0x07/0x07 | SSID GET
///
/// ```text
/// request {}
/// reply { status: u8 | reject if nonzero
///         length: u8, ssid: [u8; length] | UTF-8
///         extension: [u8; remaining] | opaque }
/// ```
#[derive(Clone, Copy, Debug, BinWrite)]
#[bw(map = |_: &Self| ())]
pub struct WifiSsidGet;

impl Query for WifiSsidGet {
    type Response = CameraReadReply<WifiSsidResponse>;

    fn response_args(&self) {}

    fn receiver(&self) -> Endpoint {
        Endpoint::Wifi
    }
    fn command(&self) -> Command {
        Command::Wifi(WifiCommand::Ssid)
    }
}

/// Accepted SSID body inside `CameraReadReply<WifiSsidResponse>`.
///
/// Wifi -> APP | 0x07/0x07 | SSID reply
///
/// ```text
/// reply { status: u8 | handled by CameraReadReply
///         length: u8, ssid: [u8; length] | UTF-8
///         extension: [u8; remaining] | opaque }
/// ```
#[derive(Clone, PartialEq, Eq, BinRead, BinWrite)]
pub struct WifiSsidResponse {
    #[br(parse_with = read_packed_string)]
    #[bw(write_with = write_packed_string)]
    pub ssid: String,
    pub extension: CameraRawData,
}

impl std::fmt::Debug for WifiSsidResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("WifiSsidResponse { ssid: [REDACTED], extension: [OPAQUE] }")
    }
}
