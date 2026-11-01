//! Gimbal initialization request.

use crate::{
    Result,
    protocol::{WireEncode, WireWriter},
};

const INITIALIZE_PREFIX: u8 = 5;
const INITIALIZE_UNKNOWN: u32 = u32::MAX;

/// Send the fixed gimbal initialization body.
///
/// APP -> Gimbal | 0x03/0xDA
///
/// ```text
/// request { unknown_prefix: u8 = 0x05, unknown: [0xFF; 4] }
/// reply { status: u8, extension: [u8; remaining] | opaque }
/// ```
#[derive(Clone, Copy, Debug)]
pub struct GimbalInitializeRequest;

impl WireEncode for GimbalInitializeRequest {
    fn write_to(&self, writer: &mut WireWriter) -> Result<()> {
        writer.write_u8(INITIALIZE_PREFIX);
        writer.write_u32_le(INITIALIZE_UNKNOWN);
        Ok(())
    }
}
