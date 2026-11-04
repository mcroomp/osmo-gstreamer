//! Automatic exposure-bracketing GET payload and response.

use crate::protocol::{
    commands::reply::{camera_data, camera_read_query},
    identity::{CameraCommand, Command},
};

camera_data!(
    /// Two unchanged bracketing code bytes, without count/step or enum meanings.
    ///
    /// Camera -> APP | 0x02/0x5F | AEB reply
    ///
    /// ```text
    /// reply { status: u8 | handled by CameraReadReply
    ///         first_code: u8, second_code: u8,
    ///         extension: [u8; remaining] | opaque }
    /// ```
    CameraAebData,
    2
);

impl CameraAebData {
    pub fn codes(&self) -> [u8; 2] {
        [self.raw_bytes()[0], self.raw_bytes()[1]]
    }
}

camera_read_query!(
    /// Read automatic exposure-bracketing fields.
    ///
    /// APP -> Camera | 0x02/0x5F
    ///
    /// ```text
    /// request {}
    /// reply { status: u8, first_code: u8, second_code: u8,
    ///         extension: [u8; remaining] }
    /// ```
    CameraAebGet,
    Command::Camera(CameraCommand::Aeb),
    &[],
    CameraAebData
);
