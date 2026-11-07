//! Camera playback-mode requests.

use binrw::{BinRead, BinWrite, binwrite};

/// Enter or leave camera playback mode.
///
/// APP -> Camera | 0x02/0x0C
///
/// ```text
/// request { path: u8 = 1, operation: le_u16 = 1,
///           mode_bits: u8 | bit 0: 0x01 enter, 0x00 leave }
/// reply { status: u8, extension: [u8; remaining] | opaque }
/// ```
///
/// The encoder emits a two-byte command prefix and four payload bytes.
/// A zero status acknowledges the mode
/// request, not completion of every ancillary storage/transfer operation.
/// Preview setup separately waits for the playback state notification.
#[binwrite]
#[derive(Clone, Copy, Debug)]
#[bw(little, magic = b"\x02\x0c")]
pub struct PlaybackRequest {
    #[bw(calc = 1u8)]
    path: u8,
    #[bw(calc = 1u16)]
    operation: u16,
    pub mode: PlaybackMode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, BinWrite, BinRead)]
#[brw(repr = u8)]
#[repr(u8)]
pub enum PlaybackMode {
    Leave = 0,
    Enter = 1,
}
