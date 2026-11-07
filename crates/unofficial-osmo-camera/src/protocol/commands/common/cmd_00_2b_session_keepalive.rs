//! BLE session keepalive and setup.

use binrw::binwrite;

/// Keep the BLE session alive.
///
/// APP -> Session | 0x00/0x2B | Keepalive
///
/// ```text
/// request { unknown_prefix: u8 = 0x01, unknown: u8 = 0x01 }
/// reply { bytes: [u8; remaining] | unspecified, not awaited }
/// ```
#[binwrite]
#[derive(Clone, Copy, Debug, Default)]
#[bw(magic = b"\x00\x2b")]
pub struct SessionKeepaliveRequest {
    #[bw(calc = [1u8, 1])]
    payload: [u8; 2],
}

/// Open the BLE keepalive session.
///
/// APP -> Session | 0x00/0x2B | Setup
///
/// ```text
/// request { unknown_prefix: u8 = 0x04, unknown: u8 = 0x00 }
/// reply { bytes: [u8; remaining] | unspecified, not awaited }
/// ```
#[binwrite]
#[derive(Clone, Copy, Debug, Default)]
#[bw(magic = b"\x00\x2b")]
pub struct SessionKeepaliveOpenRequest {
    #[bw(calc = [4u8, 0])]
    payload: [u8; 2],
}
