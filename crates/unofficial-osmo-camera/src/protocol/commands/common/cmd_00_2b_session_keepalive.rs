//! BLE session keepalive and setup.

/// Keep the BLE session alive.
///
/// APP -> Session | 0x00/0x2B | Keepalive
///
/// ```text
/// request { unknown_prefix: u8 = 0x01, unknown: u8 = 0x01 }
/// reply { bytes: [u8; remaining] | unspecified, not awaited }
/// ```
#[derive(Clone, Copy, Debug, binrw::BinWrite)]
#[bw(magic = b"\x01\x01")]
pub struct SessionKeepaliveRequest;

/// Open the BLE keepalive session.
///
/// APP -> Session | 0x00/0x2B | Setup
///
/// ```text
/// request { unknown_prefix: u8 = 0x04, unknown: u8 = 0x00 }
/// reply { bytes: [u8; remaining] | unspecified, not awaited }
/// ```
#[derive(Clone, Copy, Debug, binrw::BinWrite)]
#[bw(magic = b"\x04\x00")]
pub struct SessionKeepaliveOpenRequest;
