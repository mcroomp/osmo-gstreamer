//! Video-caption GET payload and response.

use crate::protocol::{
    commands::reply::{camera_data, camera_read_query},
    identity::{CameraCommand, Command},
};

camera_data!(
    /// Accepted video-caption byte, preserving all bits.
    ///
    /// Camera -> APP | 0x02/0x63 | Video caption reply
    ///
    /// ```text
    /// reply { status: u8 | handled by CameraReadReply
    ///         code: u8, extension: [u8; remaining] | opaque }
    /// ```
    ///
    /// The Nano reply clears bits 5 and 6; bits 0..4 and 7 remain raw.
    /// The decoder preserves every received bit rather than masking it again.
    CameraVideoCaptionValue,
    1
);

impl CameraVideoCaptionValue {
    /// Test bit 7 only; the compatibility name does not confirm overlay behavior.
    pub fn enabled_bit_set(&self) -> bool {
        self.raw_bytes()[0] & 0x80 != 0
    }
}

camera_read_query!(
    /// Read the video-caption value.
    ///
    /// APP -> Camera | 0x02/0x63
    ///
    /// ```text
    /// request {}
    /// reply { status: u8, code: u8, extension: [u8; remaining] }
    /// ```
    CameraVideoCaptionGet,
    Command::Camera(CameraCommand::VideoCaption),
    &[],
    CameraVideoCaptionValue
);
