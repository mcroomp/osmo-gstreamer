//! Sensitive Wi-Fi password GET.

use super::packed_string::{read_packed_string, write_packed_string};
use crate::{
    Result,
    protocol::{WireDecode, WireEncode, WireReader, WireWriter, commands::reply::CameraRawData},
};

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
#[derive(Clone, Copy, Debug)]
pub struct WifiPasswordRequest;

impl WireEncode for WifiPasswordRequest {
    fn write_to(&self, _writer: &mut WireWriter) -> Result<()> {
        Ok(())
    }
}

/// Accepted password body inside `CameraReadReply<WifiPasswordResponse>`.
///
/// Wifi -> APP | 0x07/0x0E | Password reply
///
/// ```text
/// reply { status: u8 | handled by CameraReadReply
///         length: u8, password: [u8; length] | UTF-8, sensitive
///         extension: [u8; remaining] | opaque }
/// ```
#[derive(Clone, PartialEq, Eq)]
pub struct WifiPasswordResponse {
    pub password: String,
    pub extension: CameraRawData,
}

impl std::fmt::Debug for WifiPasswordResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("WifiPasswordResponse { password: [REDACTED], extension: [OPAQUE] }")
    }
}

impl<Q> WireDecode<Q> for WifiPasswordResponse {
    fn read_from(reader: &mut WireReader<'_>, context: &Q) -> Result<Self> {
        Ok(Self {
            password: read_packed_string(reader)?,
            extension: CameraRawData::read_from(reader, context)?,
        })
    }
}

impl WireEncode for WifiPasswordResponse {
    fn write_to(&self, writer: &mut WireWriter) -> Result<()> {
        write_packed_string(&self.password, writer)?;
        self.extension.write_to(writer)
    }
}
