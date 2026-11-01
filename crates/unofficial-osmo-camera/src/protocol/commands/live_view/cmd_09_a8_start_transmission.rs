//! Live-view transmission start.

use crate::{
    Result,
    protocol::{WireEncode, WireWriter},
};

const TRANSMISSION_PREFIX: [u8; 3] = [0, 4, 2];
const TRANSMISSION_UNKNOWN_LENGTH: usize = 7;

/// Start the fixed live-view transmission variant.
///
/// APP -> LiveView | 0x09/0xA8
///
/// ```text
/// request { unknown_prefix: [0x00, 0x04, 0x02], unknown: [0x00; 7] }
/// reply { status: u8, extension: [u8; remaining] | opaque }
/// ```
#[derive(Clone, Copy, Debug)]
pub struct StartTransmissionRequest;

impl WireEncode for StartTransmissionRequest {
    fn write_to(&self, writer: &mut WireWriter) -> Result<()> {
        writer.write_bytes(&TRANSMISSION_PREFIX);
        writer.write_bytes(&[0; TRANSMISSION_UNKNOWN_LENGTH]);
        Ok(())
    }
}
