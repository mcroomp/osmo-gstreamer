//! Wi-Fi role-setting request.

use super::cmd_07_39_get_role::WifiRole;
use crate::{
    Result,
    protocol::{WireEncode, WireWriter},
};

/// Select camera hotspot or station mode.
///
/// APP -> Wifi | 0x07/0x48 | Role SET
///
/// ```text
/// request { role: u8 | 0x00 hotspot, 0x01 station }
/// reply { status: u8, extension: [u8; remaining] | opaque }
/// ```
///
/// A hotspot success response can precede the actual transition. It confirms
/// receipt, not that the radio changed mode; this request performs no readback
/// of association or IP configuration. The compatibility encoder exposes only
/// roles 0/1, not every possible device role.
#[derive(Clone, Copy, Debug)]
pub struct WifiRoleSetRequest {
    pub role: WifiRole,
}

impl WireEncode for WifiRoleSetRequest {
    fn write_to(&self, writer: &mut WireWriter) -> Result<()> {
        self.role.write_to(writer)
    }
}
