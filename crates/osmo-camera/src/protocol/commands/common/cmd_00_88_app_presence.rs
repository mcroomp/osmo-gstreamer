//! Application-presence command payload.

use crate::{
    Result,
    protocol::{WireEncode, WireWriter},
};

const PRESENCE_PREFIX: [u8; 5] = [0x17, 0, 0x46, 0x23, 0x7c];
const PRESENCE_NAME: &[u8] = b"APP";
const PRESENCE_UNKNOWN_LENGTH: usize = 5;
const PRESENCE_SUFFIX: u8 = 2;

/// Presence body sent to the camera.
///
/// APP -> Presence | 0x00/0x88
///
/// ```text
/// request {
///     bytes: [0x17, 0x00, 0x46, 0x23, 0x7c, 0x41, 0x50, 0x50,
///             0x00, 0x00, 0x00, 0x00, 0x00, 0x02]
/// }
/// reply { bytes: [u8; remaining] | unspecified, not awaited }
/// ```
#[derive(Clone, Copy, Debug)]
pub struct AppPresenceRequest;

impl WireEncode for AppPresenceRequest {
    fn write_to(&self, writer: &mut WireWriter) -> Result<()> {
        writer.write_bytes(&PRESENCE_PREFIX);
        writer.write_bytes(PRESENCE_NAME);
        writer.write_bytes(&[0; PRESENCE_UNKNOWN_LENGTH]);
        writer.write_u8(PRESENCE_SUFFIX);
        Ok(())
    }
}
