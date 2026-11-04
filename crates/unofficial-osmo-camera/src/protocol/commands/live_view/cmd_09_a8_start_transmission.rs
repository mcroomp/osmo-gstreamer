//! Fixed live-view control request used during preview startup.

use crate::{
    Result,
    protocol::{WireEncode, WireWriter},
};

const TRANSMISSION_PREFIX: [u8; 3] = [0, 4, 2];
const TRANSMISSION_UNKNOWN_LENGTH: usize = 7;

/// Send the fixed I-frame request used by the preview startup sequence.
///
/// APP -> LiveView | 0x09/0xA8
///
/// ```text
/// request { liveview_id: u8 = 0, action: u8 = 4,
///           iframe_argument: u8 = 2, unused: [0x00; 7] }
/// reply { status: u8, extension: [u8; remaining] | opaque }
/// ```
///
/// This ten-byte control layout selects force-I-frame action 4, not a general
/// encoder-start operation. The raw argument's units/range are unspecified;
/// it is not a codec, resolution or frame-count field. A successful response
/// does not confirm video delivery. The type name is retained for compatibility.
#[derive(Clone, Copy, Debug)]
pub struct StartTransmissionRequest;

impl WireEncode for StartTransmissionRequest {
    fn write_to(&self, writer: &mut WireWriter) -> Result<()> {
        writer.write_bytes(&TRANSMISSION_PREFIX);
        writer.write_bytes(&[0; TRANSMISSION_UNKNOWN_LENGTH]);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compatibility_request_remains_the_exact_ten_byte_iframe_control() {
        assert_eq!(
            StartTransmissionRequest.encode().unwrap(),
            [0, 4, 2, 0, 0, 0, 0, 0, 0, 0]
        );
    }
}
