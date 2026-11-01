//! Camera playback-mode requests.

use crate::{
    Result,
    protocol::{WireEncode, WireWriter},
};
use num_enum::{IntoPrimitive, TryFromPrimitive};

const PLAYBACK_PREFIX: [u8; 3] = [1, 1, 0];

#[derive(Clone, Copy, Debug, PartialEq, Eq, IntoPrimitive, TryFromPrimitive)]
#[repr(u8)]
pub enum PlaybackMode {
    Leave = 0,
    Enter = 1,
}

/// Enter or leave camera playback mode.
///
/// APP -> Camera | 0x02/0x0C
///
/// ```text
/// request { unknown: [0x01, 0x01, 0x00], mode: u8 | 0x01 enter, 0x00 leave }
/// reply { status: u8, extension: [u8; remaining] | opaque }
/// ```
#[derive(Clone, Copy, Debug)]
pub struct PlaybackRequest {
    pub mode: PlaybackMode,
}

impl WireEncode for PlaybackRequest {
    fn write_to(&self, writer: &mut WireWriter) -> Result<()> {
        writer.write_bytes(&PLAYBACK_PREFIX);
        writer.write_u8(self.mode.into());
        Ok(())
    }
}
