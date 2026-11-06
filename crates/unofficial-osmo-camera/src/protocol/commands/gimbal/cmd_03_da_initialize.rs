//! Gimbal initialization request.

/// Send the fixed gimbal initialization body.
///
/// APP -> Gimbal | 0x03/0xDA
///
/// ```text
/// request { unknown_prefix: u8 = 0x05, unknown: [0xFF; 4] }
/// reply { status: u8, extension: [u8; remaining] | opaque }
/// ```
#[derive(Clone, Copy, Debug, binrw::BinWrite)]
#[bw(magic = b"\x05\xff\xff\xff\xff")]
pub struct GimbalInitializeRequest;
