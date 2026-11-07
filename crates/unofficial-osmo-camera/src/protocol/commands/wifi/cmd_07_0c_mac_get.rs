//! Wi-Fi MAC-address GET.

use crate::protocol::{Query, commands::reply::CommandReadReply, identity::Endpoint};
use binrw::{BinWrite, binrw};
use std::fmt::{Debug, Formatter, Result as FmtResult};

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
#[derive(Clone, Copy, Debug, BinWrite, Default)]
#[bw(magic = b"\x07\x0c")]
pub struct WifiMacGet;

pub const MAC_ADDRESS_LENGTH: usize = 6;

impl Query for WifiMacGet {
    type Response = CommandReadReply<WifiMacReply>;

    fn receiver(&self) -> Endpoint {
        Endpoint::Wifi
    }
}

/// Accepted six-byte MAC body inside `CameraReadReply<WifiMacReply>`.
///
/// Wifi -> APP | 0x07/0x0C | MAC reply
///
/// ```text
/// reply { status: u8 | handled by CommandReadReply
///         address: [u8; 6] }
/// ```
///
/// The accepted body must be exactly six bytes; no extension layout is defined.
#[binrw]
#[derive(Clone, PartialEq, Eq)]
#[brw(magic = b"\x07\x0c")]
pub struct WifiMacReply {
    #[br(temp, assert(status == 0, "accepted reply requires success status"))]
    #[bw(calc = 0u8)]
    status: u8,
    pub address: [u8; MAC_ADDRESS_LENGTH],
}

impl Debug for WifiMacReply {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str("WifiMacReply { address: [REDACTED] }")
    }
}
