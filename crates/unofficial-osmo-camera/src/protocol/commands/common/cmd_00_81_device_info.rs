//! Fixed-size device identification response payload.

use binrw::binrw;

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
#[binrw]
#[derive(Clone, Copy, Debug, Default)]
#[brw(magic = b"\x00\x81")]
pub struct AppDeviceInfoResponse {
    #[br(temp, assert(prefix == 0))]
    #[bw(calc = 0u8)]
    prefix: u8,
    #[br(temp, assert(name == *b"APP"))]
    #[bw(calc = *b"APP")]
    name: [u8; 3],
    #[br(temp, assert(reserved_before == [0; 37]))]
    #[bw(calc = [0u8; 37])]
    reserved_before: [u8; 37],
    #[br(temp, assert(first_code == 2))]
    #[bw(calc = 2u8)]
    first_code: u8,
    #[br(temp, assert(reserved_middle == [0; 8]))]
    #[bw(calc = [0u8; 8])]
    reserved_middle: [u8; 8],
    #[br(temp, assert(last_codes == [2, 8]))]
    #[bw(calc = [2u8, 8])]
    last_codes: [u8; 2],
    #[br(temp, assert(reserved_after == [0; 10]))]
    #[bw(calc = [0u8; 10])]
    reserved_after: [u8; 10],
}

pub const DEVICE_INFO_LENGTH: usize = 62;
pub const DEVICE_NAME_OFFSET: usize = 1;
pub const DEVICE_NAME: &[u8] = b"APP";
pub const DEVICE_UNKNOWN_FIELDS: [(usize, u8); 3] = [(41, 2), (50, 2), (51, 8)];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{
        CommandBody, Flags, Message, decode, encode,
        identity::{Command, Endpoint},
    };

    #[test]
    fn device_info_body_keeps_payload_offsets_and_command_out_of_the_payload() {
        let response = AppDeviceInfoResponse::default();
        let bytes = encode(&response).unwrap();
        let body = decode::<CommandBody>(&bytes, ()).unwrap();
        assert_eq!(body.command, Command::from_wire(0, 0x81));
        assert_eq!(body.payload.len(), DEVICE_INFO_LENGTH);
        assert_eq!(
            &body.payload[DEVICE_NAME_OFFSET..DEVICE_NAME_OFFSET + DEVICE_NAME.len()],
            DEVICE_NAME,
        );
        for (offset, value) in DEVICE_UNKNOWN_FIELDS {
            assert_eq!(body.payload[offset], value);
        }
        let message = Message::from_command_body(
            Endpoint::DeviceInfo,
            0x1234,
            Flags::ACKNOWLEDGMENT,
            &response,
        )
        .unwrap();
        assert_eq!(message.payload, body.payload);
        assert_eq!(message.flags, Flags::ACKNOWLEDGMENT);
        assert!(decode::<AppDeviceInfoResponse>(&bytes, ()).is_ok());
        for index in [0, 1, 2, 3, 43, 52, 53] {
            let mut changed = bytes.clone();
            changed[index] ^= 1;
            assert!(decode::<AppDeviceInfoResponse>(&changed, ()).is_err());
        }
    }
}
