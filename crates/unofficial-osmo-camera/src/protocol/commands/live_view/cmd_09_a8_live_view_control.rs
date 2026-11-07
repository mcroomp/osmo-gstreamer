//! Live-view controller I-frame request used during preview startup.

use binrw::binwrite;

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
/// does not confirm video delivery. This request does not establish encoder
/// startup or configure bitrate.
#[binwrite]
#[derive(Clone, Copy, Debug, Default)]
#[bw(magic = b"\x09\xa8")]
pub struct LiveViewIFrameRequest {
    #[bw(calc = 0u8)]
    liveview_id: u8,
    #[bw(calc = 4u8)]
    action: u8,
    #[bw(calc = 2u8, pad_after = 7)]
    iframe_argument: u8,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::CommandPacket;

    #[test]
    fn iframe_request_preserves_the_exact_ten_byte_control_payload() {
        assert_eq!(
            CommandPacket::payload(&LiveViewIFrameRequest::default()).unwrap(),
            [0, 4, 2, 0, 0, 0, 0, 0, 0, 0]
        );
        assert_eq!(LiveViewIFrameRequest::COMMAND_ID, [9, 0xa8]);
    }
}
