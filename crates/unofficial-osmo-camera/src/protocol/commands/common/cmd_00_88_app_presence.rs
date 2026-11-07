//! Application-presence command payload.

use binrw::binwrite;

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
#[binwrite]
#[derive(Clone, Copy, Debug, Default)]
#[bw(magic = b"\x00\x88")]
pub struct AppPresenceRequest {
    #[bw(calc = *b"\x17\x00\x46\x23\x7cAPP\0\0\0\0\0\x02")]
    payload: [u8; 14],
}
