//! Camera wake request.

/// Wake the camera over BLE.
///
/// APP -> CameraWake | 0x53/0x10 | BLE wake
///
/// ```text
/// request { unknown: [0x00; 4] }
/// reply { bytes: [u8; remaining] | unspecified, not awaited }
/// ```
#[derive(Clone, Copy, Debug, binrw::BinWrite)]
#[bw(magic = b"\x00\x00\x00\x00")]
pub struct BleWakeRequest;
