//! Fixed-size device identification response payload.

use crate::{
    Result,
    protocol::{WireEncode, WireWriter},
};

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
#[derive(Clone, Copy, Debug)]
pub struct AppDeviceInfoResponse;

impl WireEncode for AppDeviceInfoResponse {
    fn write_to(&self, writer: &mut WireWriter) -> Result<()> {
        let mut bytes = vec![0; DEVICE_INFO_LENGTH];
        bytes[DEVICE_NAME_OFFSET..DEVICE_NAME_OFFSET + DEVICE_NAME.len()]
            .copy_from_slice(DEVICE_NAME);
        for (offset, value) in DEVICE_UNKNOWN_FIELDS {
            bytes[offset] = value;
        }
        writer.write_bytes(&bytes);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_info_has_a_fixed_size_and_preserved_fields() {
        let encoded = AppDeviceInfoResponse.encode().unwrap();
        assert_eq!(encoded.len(), DEVICE_INFO_LENGTH);
        assert_eq!(
            &encoded[DEVICE_NAME_OFFSET..DEVICE_NAME_OFFSET + DEVICE_NAME.len()],
            DEVICE_NAME
        );
        for (offset, value) in DEVICE_UNKNOWN_FIELDS {
            assert_eq!(encoded[offset], value);
        }
    }
}
