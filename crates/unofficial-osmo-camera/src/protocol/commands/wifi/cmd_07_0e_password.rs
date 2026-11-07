//! Sensitive Wi-Fi password GET.

use super::packed_string::{read_packed_string, write_packed_string};
use crate::protocol::commands::reply::CameraRawData;
use binrw::{BinWrite, binrw};
use std::fmt::{Debug, Formatter, Result as FmtResult};

/// Read hotspot credentials only within an explicitly requested credential lifecycle.
///
/// APP -> Wifi | 0x07/0x0E | Password GET
///
/// ```text
/// request {}
/// reply { status: u8 | reject if nonzero
///         length: u8, password: [u8; length] | UTF-8, sensitive
///         extension: [u8; remaining] | opaque }
/// ```
///
/// This is not a general read-only `Query`; the central credential policy applies.
#[derive(Clone, Copy, Debug, BinWrite)]
#[bw(magic = b"\x07\x0e")]
pub struct WifiPasswordGet;

pub const MIN_PASSWORD_LENGTH: usize = 8;
pub const MAX_PASSWORD_BYTES: usize = 63;

/// Accepted password body inside `CameraReadReply<WifiPasswordReply>`.
///
/// Wifi -> APP | 0x07/0x0E | Password reply
///
/// ```text
/// reply { status: u8 | handled by CommandReadReply
///         length: u8, password: [u8; length] | UTF-8, sensitive
///         extension: [u8; remaining] | opaque }
/// ```
#[binrw]
#[derive(Clone, PartialEq, Eq)]
#[brw(magic = b"\x07\x0e")]
pub struct WifiPasswordReply {
    #[br(temp, assert(status == 0, "accepted reply requires success status"))]
    #[bw(calc = 0u8)]
    status: u8,
    #[br(parse_with = read_packed_string)]
    #[bw(write_with = write_packed_string)]
    pub password: String,
    pub extension: CameraRawData,
}

impl Debug for WifiPasswordReply {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str("WifiPasswordReply { password: [REDACTED], extension: [OPAQUE] }")
    }
}
