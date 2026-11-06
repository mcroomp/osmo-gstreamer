//! Wi-Fi MAC-address GET.
use binrw::{BinRead, BinWrite};

use crate::protocol::{
    Query,
    commands::reply::CameraReadReply,
    identity::{Command, Endpoint, WifiCommand},
};

pub const MAC_ADDRESS_LENGTH: usize = 6;

/// Read the Wi-Fi MAC address.
///
/// APP -> Wifi | 0x07/0x0C | MAC GET
///
/// ```text
/// request {}
/// reply { status: u8 | reject if nonzero
///         address: [u8; 6] }
/// ```
///
/// A failed read need not produce a reply; callers must retain bounded
/// timeouts rather than inventing a zero address or treating silence as success.
#[derive(Clone, Copy, Debug, BinWrite)]
#[bw(map = |_: &Self| ())]
pub struct WifiMacGet;

impl Query for WifiMacGet {
    type Response = CameraReadReply<WifiMacResponse>;

    fn response_args(&self) {}

    fn receiver(&self) -> Endpoint {
        Endpoint::Wifi
    }
    fn command(&self) -> Command {
        Command::Wifi(WifiCommand::Mac)
    }
}

/// Accepted six-byte MAC body inside `CameraReadReply<WifiMacResponse>`.
///
/// Wifi -> APP | 0x07/0x0C | MAC reply
///
/// ```text
/// reply { status: u8 | handled by CameraReadReply
///         address: [u8; 6] }
/// ```
///
/// The accepted body must be exactly six bytes; no extension layout is defined.
#[derive(Clone, PartialEq, Eq, BinRead, BinWrite)]
pub struct WifiMacResponse {
    pub address: [u8; MAC_ADDRESS_LENGTH],
}

impl std::fmt::Debug for WifiMacResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("WifiMacResponse { address: [REDACTED] }")
    }
}
