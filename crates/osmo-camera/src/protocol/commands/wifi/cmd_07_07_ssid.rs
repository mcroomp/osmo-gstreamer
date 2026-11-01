//! Wi-Fi SSID GET.

use super::packed_string::{read_packed_string, write_packed_string};
use crate::{
    Result,
    protocol::{
        Query, WireDecode, WireEncode, WireReader, WireWriter,
        commands::reply::{CameraRawData, CameraReadReply},
        identity::{Command, Endpoint, WifiCommand},
    },
};

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
#[derive(Clone, Copy, Debug)]
pub struct WifiSsidGet;

impl WireEncode for WifiSsidGet {
    fn write_to(&self, _writer: &mut WireWriter) -> Result<()> {
        Ok(())
    }
}

impl Query for WifiSsidGet {
    type Response = CameraReadReply<WifiSsidResponse>;
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
#[derive(Clone, PartialEq, Eq)]
pub struct WifiSsidResponse {
    pub ssid: String,
    pub extension: CameraRawData,
}

impl std::fmt::Debug for WifiSsidResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("WifiSsidResponse { ssid: [REDACTED], extension: [OPAQUE] }")
    }
}

impl<Q> WireDecode<Q> for WifiSsidResponse {
    fn read_from(reader: &mut WireReader<'_>, context: &Q) -> Result<Self> {
        Ok(Self {
            ssid: read_packed_string(reader)?,
            extension: CameraRawData::read_from(reader, context)?,
        })
    }
}

impl WireEncode for WifiSsidResponse {
    fn write_to(&self, writer: &mut WireWriter) -> Result<()> {
        write_packed_string(&self.ssid, writer)?;
        self.extension.write_to(writer)
    }
}
