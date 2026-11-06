//! Sensitive Wi-Fi password GET.

use super::packed_string::{read_packed_string, write_packed_string};
use crate::protocol::commands::reply::CameraRawData;
use binrw::{BinRead, BinWrite};

pub const MIN_PASSWORD_LENGTH: usize = 8;
pub const MAX_PASSWORD_BYTES: usize = 63;

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
#[bw(map = |_: &Self| ())]
pub struct WifiPasswordRequest;

/// Accepted password body inside `CameraReadReply<WifiPasswordResponse>`.
///
/// Wifi -> APP | 0x07/0x0E | Password reply
///
/// ```text
/// reply { status: u8 | handled by CameraReadReply
///         length: u8, password: [u8; length] | UTF-8, sensitive
///         extension: [u8; remaining] | opaque }
/// ```
#[derive(Clone, PartialEq, Eq, BinRead, BinWrite)]
pub struct WifiPasswordResponse {
    #[br(parse_with = read_packed_string)]
    #[bw(write_with = write_packed_string)]
    pub password: String,
    pub extension: CameraRawData,
}

impl std::fmt::Debug for WifiPasswordResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("WifiPasswordResponse { password: [REDACTED], extension: [OPAQUE] }")
    }
}
