//! Gimbal initialization request.

use binrw::binwrite;

/// Send the fixed compatibility prelude used before preview setup.
///
/// APP -> Gimbal | 0x03/0xDA
///
/// ```text
/// request { unknown_prefix: u8 = 0x05, unknown: [0xFF; 4] }
/// reply { bytes: [u8; remaining] | unspecified, not awaited }
/// ```
///
/// The endpoint label and type name do not establish a physical gimbal or
/// successful initialization. The remote consumer and field meanings remain
/// unresolved; preview startup sends this body without awaiting its reply.
#[binwrite]
#[derive(Clone, Copy, Debug, Default)]
#[bw(magic = b"\x03\xda")]
pub struct GimbalInitializeRequest {
    #[bw(calc = *b"\x05\xff\xff\xff\xff")]
    payload: [u8; 5],
}
