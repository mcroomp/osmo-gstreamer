//! Wi-Fi network join payload.

use crate::{
    Result,
    protocol::{WireEncode, WireWriter},
};

use super::packed_string::write_packed_string;

/// Status and first opaque detail byte qualifying the bounded join retry.
pub const JOIN_RETRY_STATUS: crate::protocol::ResponseStatus =
    crate::protocol::ResponseStatus::Unknown(1);
pub const JOIN_RETRY_DETAIL: u8 = 0xff;

/// Encodes the SSID and password as consecutive length-prefixed UTF-8 strings.
///
/// APP -> Wifi | 0x07/0x47
///
/// ```text
/// request {
///     ssid_length: u8
///     ssid: [u8; ssid_length] | UTF-8
///     password_length: u8
///     password: [u8; password_length] | UTF-8, sensitive
/// }
/// reply { status: u8, extension: [u8; remaining] | opaque }
/// ```
///
/// Replies reuse `CameraReadReply<CameraRawData>`. A nonempty all-zero body
/// indicates acceptance; status 0x01 followed by 0xFF qualifies a temporary
/// refusal. Status-zero replies with nonzero trailing bytes are not accepted
/// by the lifecycle. Other detail bytes remain uninterpreted.
pub struct JoinNetworkRequest<'a> {
    pub ssid: &'a str,
    pub password: &'a str,
}

impl std::fmt::Debug for JoinNetworkRequest<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("JoinNetworkRequest { credentials: [REDACTED] }")
    }
}

impl WireEncode for JoinNetworkRequest<'_> {
    fn write_to(&self, writer: &mut WireWriter) -> Result<()> {
        write_packed_string(self.ssid, writer)?;
        write_packed_string(self.password, writer)
    }
}
