//! Wi-Fi SSID GET.

use super::packed_string::{read_packed_string, write_packed_string};
use crate::protocol::{
    Query,
    commands::reply::{CameraRawData, CommandReadReply},
    identity::Endpoint,
};
use binrw::{BinWrite, binrw};
use std::fmt::{Debug, Formatter, Result as FmtResult};

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
#[derive(Clone, Copy, Debug, BinWrite, Default)]
#[bw(magic = b"\x07\x07")]
pub struct WifiSsidGet;

pub const MAX_SSID_BYTES: usize = 32;

impl Query for WifiSsidGet {
    type Response = CommandReadReply<WifiSsidReply>;

    fn receiver(&self) -> Endpoint {
        Endpoint::Wifi
    }
}

/// Accepted SSID body inside `CameraReadReply<WifiSsidReply>`.
///
/// Wifi -> APP | 0x07/0x07 | SSID reply
///
/// ```text
/// reply { status: u8 | handled by CommandReadReply
///         length: u8, ssid: [u8; length] | UTF-8
///         extension: [u8; remaining] | opaque }
/// ```
#[binrw]
#[derive(Clone, PartialEq, Eq)]
#[brw(magic = b"\x07\x07")]
pub struct WifiSsidReply {
    #[br(temp, assert(status == 0, "accepted reply requires success status"))]
    #[bw(calc = 0u8)]
    status: u8,
    #[br(parse_with = read_packed_string)]
    #[bw(write_with = write_packed_string)]
    pub ssid: String,
    pub extension: CameraRawData,
}

impl Debug for WifiSsidReply {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str("WifiSsidReply { ssid: [REDACTED], extension: [OPAQUE] }")
    }
}
