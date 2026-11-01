//! Wi-Fi MAC-address GET.

use crate::{
    Result,
    protocol::{
        Query, WireDecode, WireEncode, WireReader, WireWriter,
        commands::reply::CameraReadReply,
        identity::{Command, Endpoint, WifiCommand},
    },
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
#[derive(Clone, Copy, Debug)]
pub struct WifiMacGet;

impl WireEncode for WifiMacGet {
    fn write_to(&self, _writer: &mut WireWriter) -> Result<()> {
        Ok(())
    }
}

impl Query for WifiMacGet {
    type Response = CameraReadReply<WifiMacResponse>;
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
#[derive(Clone, PartialEq, Eq)]
pub struct WifiMacResponse {
    pub address: [u8; MAC_ADDRESS_LENGTH],
}

impl std::fmt::Debug for WifiMacResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("WifiMacResponse { address: [REDACTED] }")
    }
}

impl<Q> WireDecode<Q> for WifiMacResponse {
    fn read_from(reader: &mut WireReader<'_>, _context: &Q) -> Result<Self> {
        let mut address = [0; MAC_ADDRESS_LENGTH];
        address.copy_from_slice(reader.read_bytes(MAC_ADDRESS_LENGTH)?);
        Ok(Self { address })
    }
}

impl WireEncode for WifiMacResponse {
    fn write_to(&self, writer: &mut WireWriter) -> Result<()> {
        writer.write_bytes(&self.address);
        Ok(())
    }
}
