//! Fixed-size device identification response payload.

pub const DEVICE_INFO_LENGTH: usize = 62;
pub const DEVICE_NAME_OFFSET: usize = 1;
pub const DEVICE_NAME: &[u8] = b"APP";
pub const DEVICE_UNKNOWN_FIELDS: [(usize, u8); 3] = [(41, 2), (50, 2), (51, 8)];

/// Encodes the fixed-size application device-info body.
///
/// APP -> DeviceInfo | 0x00/0x81 | Reply
///
/// ```text
/// reply {
///     prefix: u8 = 0x00
///     name: [0x41, 0x50, 0x50] | ASCII APP
///     reserved: [0x00; 37]
///     unknown: u8 = 0x02
///     reserved: [0x00; 8]
///     unknown: [0x02, 0x08]
///     reserved: [0x00; 10]
/// }
/// ```
///
/// This is the application reply body, not the camera's device-info layout.
/// UDP registration uses response flags 0x80; BLE acknowledgment of an inbound
/// request uses 0xC0 and reverses the request endpoints. The body is identical.
#[derive(Clone, Copy, Debug, binrw::BinWrite)]
#[bw(map = |_: &Self| device_info_bytes())]
pub struct AppDeviceInfoResponse;

fn device_info_bytes() -> [u8; DEVICE_INFO_LENGTH] {
    let mut bytes = [0; DEVICE_INFO_LENGTH];
    bytes[DEVICE_NAME_OFFSET..DEVICE_NAME_OFFSET + DEVICE_NAME.len()].copy_from_slice(DEVICE_NAME);
    for (offset, value) in DEVICE_UNKNOWN_FIELDS {
        bytes[offset] = value;
    }
    bytes
}
