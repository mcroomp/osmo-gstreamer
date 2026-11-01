//! Photo-interval GET payload and response.

use crate::protocol::{
    commands::reply::{camera_data, camera_read_query},
    identity::{CameraCommand, Command},
};

camera_data!(
    /// Accepted photo-interval fields; units and unknown bytes remain uninterpreted.
    ///
    /// Camera -> APP | 0x02/0x4B | Photo interval reply
    ///
    /// ```text
    /// reply { status: u8 | handled by CameraReadReply
    ///         unknown: u8, count_code: u8, coarse: le_u16, fine: le_u16,
    ///         extension: [u8; remaining] | opaque }
    /// ```
    CameraIntervalData,
    6
);

impl CameraIntervalData {
    pub fn count_code(&self) -> u8 {
        self.raw_bytes()[1]
    }

    pub fn coarse_field(&self) -> u16 {
        u16::from_le_bytes([self.raw_bytes()[2], self.raw_bytes()[3]])
    }

    pub fn fine_field(&self) -> u16 {
        u16::from_le_bytes([self.raw_bytes()[4], self.raw_bytes()[5]])
    }
}

camera_read_query!(
    /// Read photo-interval fields.
    ///
    /// APP -> Camera | 0x02/0x4B
    ///
    /// ```text
    /// request {}
    /// reply {
    ///     status: u8
    ///     unknown: u8
    ///     count_code: u8
    ///     coarse: le_u16
    ///     fine: le_u16
    ///     extension: [u8; remaining]
    /// }
    /// ```
    CameraPhotoIntervalGet,
    Command::Camera(CameraCommand::PhotoInterval),
    &[],
    CameraIntervalData
);
