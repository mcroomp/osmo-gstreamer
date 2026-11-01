//! Camera wake request.

use crate::{
    Result,
    protocol::{WireEncode, WireWriter},
};

const WAKE_UNKNOWN_LENGTH: usize = 4;

/// Wake the camera over BLE.
///
/// APP -> CameraWake | 0x53/0x10 | BLE wake
///
/// ```text
/// request { unknown: [0x00; 4] }
/// reply { bytes: [u8; remaining] | unspecified, not awaited }
/// ```
#[derive(Clone, Copy, Debug)]
pub struct BleWakeRequest;

impl WireEncode for BleWakeRequest {
    fn write_to(&self, writer: &mut WireWriter) -> Result<()> {
        writer.write_bytes(&[0; WAKE_UNKNOWN_LENGTH]);
        Ok(())
    }
}
