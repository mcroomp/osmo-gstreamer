//! Application-presence command payload.

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
#[derive(Clone, Copy, Debug, binrw::BinWrite)]
#[bw(magic = b"\x17\x00\x46\x23\x7cAPP\x00\x00\x00\x00\x00\x02")]
pub struct AppPresenceRequest;
