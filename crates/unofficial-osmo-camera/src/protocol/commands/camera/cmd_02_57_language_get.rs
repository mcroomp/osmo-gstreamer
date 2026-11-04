//! Camera-language GET payload and response.

use crate::protocol::{
    commands::reply::{camera_data, camera_read_query},
    identity::{CameraCommand, Command},
};

camera_data!(
    /// Compatibility decoder for a one-byte language code; support is unconfirmed.
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
    /// Signed view of the raw byte, without assigning a language or sentinel meaning.
    pub fn signed_code(&self) -> i8 {
        self.raw_bytes()[0] as i8
    }
}

camera_read_query!(
    /// Request the camera-language code; a one-byte rejection needs no data body.
    ///
    /// An accepted language mapping and model support have not been established.
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
