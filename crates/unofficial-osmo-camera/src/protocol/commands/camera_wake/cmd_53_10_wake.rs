//! Camera wake request.

use binrw::binwrite;

/// Wake the camera over BLE.
///
/// APP -> CameraWake | 0x53/0x10 | BLE wake
///
/// ```text
/// request { unknown: [0x00; 4] }
/// reply { bytes: [u8; remaining] | unspecified, not awaited }
/// ```
#[binwrite]
#[derive(Clone, Copy, Debug, Default)]
#[bw(magic = b"\x53\x10")]
pub struct BleWakeRequest {
    #[bw(calc = [0u8; 4])]
    payload: [u8; 4],
}
