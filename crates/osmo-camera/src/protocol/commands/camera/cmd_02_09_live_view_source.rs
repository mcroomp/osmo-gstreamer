//! Camera live-view source selection.

use crate::{
    Result,
    protocol::{WireEncode, WireWriter},
};

const SOURCE_PREFIX_LENGTH: usize = 10;
const SOURCE_SELECTOR: u8 = 3;

/// Select the fixed live-view source.
///
/// APP -> Camera | 0x02/0x09
///
/// ```text
/// request { unknown: [0x00; 10], selector: u8 = 0x03 }
/// reply { status: u8, extension: [u8; remaining] | opaque }
/// ```
#[derive(Clone, Copy, Debug)]
pub struct LiveViewSourceRequest;

impl WireEncode for LiveViewSourceRequest {
    fn write_to(&self, writer: &mut WireWriter) -> Result<()> {
        writer.write_bytes(&[0; SOURCE_PREFIX_LENGTH]);
        writer.write_u8(SOURCE_SELECTOR);
        Ok(())
    }
}
