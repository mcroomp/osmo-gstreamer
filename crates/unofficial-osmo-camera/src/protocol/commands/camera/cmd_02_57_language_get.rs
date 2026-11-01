//! Camera-language GET payload and response.

use crate::protocol::{
    commands::reply::{camera_data, camera_read_query},
    identity::{CameraCommand, Command},
};

camera_data!(
    /// Accepted language code; unknown numeric values are preserved.
    ///
    /// Camera -> APP | 0x02/0x57 | Language reply
    ///
    /// ```text
    /// reply { status: u8 | handled by CameraReadReply
    ///         code: u8, extension: [u8; remaining] | opaque }
    /// ```
    CameraLanguageCode,
    1
);

impl CameraLanguageCode {
    pub fn signed_code(&self) -> i8 {
        self.raw_bytes()[0] as i8
    }
}

camera_read_query!(
    /// Read the camera-language code.
    ///
    /// APP -> Camera | 0x02/0x57
    ///
    /// ```text
    /// request {}
    /// reply { status: u8, code: u8, extension: [u8; remaining] }
    /// ```
    CameraLanguageGet,
    Command::Camera(CameraCommand::Language),
    &[],
    CameraLanguageCode
);
