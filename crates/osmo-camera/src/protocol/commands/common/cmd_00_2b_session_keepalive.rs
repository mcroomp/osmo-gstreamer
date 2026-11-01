//! BLE session keepalive and setup.

use crate::{
    Result,
    protocol::{WireEncode, WireWriter},
};

const KEEPALIVE_PREFIX: u8 = 1;
const KEEPALIVE_UNKNOWN: u8 = 1;
const OPEN_PREFIX: u8 = 4;
const OPEN_UNKNOWN: u8 = 0;

/// Keep the BLE session alive.
///
/// APP -> Session | 0x00/0x2B | Keepalive
///
/// ```text
/// request { unknown_prefix: u8 = 0x01, unknown: u8 = 0x01 }
/// reply { bytes: [u8; remaining] | unspecified, not awaited }
/// ```
#[derive(Clone, Copy, Debug)]
pub struct SessionKeepaliveRequest;

impl WireEncode for SessionKeepaliveRequest {
    fn write_to(&self, writer: &mut WireWriter) -> Result<()> {
        writer.write_u8(KEEPALIVE_PREFIX);
        writer.write_u8(KEEPALIVE_UNKNOWN);
        Ok(())
    }
}

/// Open the BLE keepalive session.
///
/// APP -> Session | 0x00/0x2B | Setup
///
/// ```text
/// request { unknown_prefix: u8 = 0x04, unknown: u8 = 0x00 }
/// reply { bytes: [u8; remaining] | unspecified, not awaited }
/// ```
#[derive(Clone, Copy, Debug)]
pub struct SessionKeepaliveOpenRequest;

impl WireEncode for SessionKeepaliveOpenRequest {
    fn write_to(&self, writer: &mut WireWriter) -> Result<()> {
        writer.write_u8(OPEN_PREFIX);
        writer.write_u8(OPEN_UNKNOWN);
        Ok(())
    }
}
