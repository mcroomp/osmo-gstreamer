//! Audio GET payload and response.

use crate::protocol::{
    commands::reply::{camera_data, camera_read_query},
    identity::{CameraCommand, Command},
};

camera_data!(
    /// Accepted audio data, excluding the outer status byte.
    ///
    /// Camera -> APP | 0x02/0xA0 | Audio reply
    ///
    /// ```text
    /// reply { status: u8 | handled by CameraReadReply
    ///         data: [u8; 15] | known prefix length, fields partly unmapped
    ///         extension: [u8; remaining] | opaque }
    /// ```
    CameraAudioData,
    15
);

impl CameraAudioData {
    pub fn data_prefix(&self) -> &[u8] {
        &self.raw_bytes()[..15]
    }

    pub fn extension(&self) -> &[u8] {
        &self.raw_bytes()[15..]
    }

    pub fn byte_2_low_bits_set(&self) -> bool {
        self.raw_bytes()[2] & 3 != 0
    }
}

camera_read_query!(
    /// Read audio parameters.
    ///
    /// APP -> Camera | 0x02/0xA0
    ///
    /// ```text
    /// request {}
    /// reply { status: u8, data: [u8; 15], extension: [u8; remaining] }
    /// ```
    CameraAudioGet,
    Command::Camera(CameraCommand::Audio),
    &[],
    CameraAudioData
);
