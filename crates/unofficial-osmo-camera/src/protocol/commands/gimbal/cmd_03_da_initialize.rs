//! Gimbal initialization request.

use binrw::binwrite;

/// Send the fixed gimbal initialization body.
///
/// APP -> Gimbal | 0x03/0xDA
///
/// ```text
/// request { unknown_prefix: u8 = 0x05, unknown: [0xFF; 4] }
/// reply { status: u8, extension: [u8; remaining] | opaque }
/// ```
#[binwrite]
#[derive(Clone, Copy, Debug, Default)]
#[bw(magic = b"\x03\xda")]
pub struct GimbalInitializeRequest {
    #[bw(calc = *b"\x05\xff\xff\xff\xff")]
    payload: [u8; 5],
}
