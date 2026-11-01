//! Storage-location GET payload and response.

use crate::protocol::{
    commands::reply::{camera_data, camera_read_query},
    identity::{CameraCommand, Command},
};

camera_data!(
    /// Accepted storage-location code and uninterpreted prefix.
    ///
    /// Camera -> APP | 0x02/0xDA | Storage location reply
    ///
    /// ```text
    /// reply { status: u8 | handled by CameraReadReply
    ///         unknown: [u8; 5], code: u8,
    ///         extension: [u8; remaining] | opaque }
    /// ```
    CameraStorageData,
    6
);

impl CameraStorageData {
    pub fn code(&self) -> u8 {
        self.raw_bytes()[5] & 3
    }
}

camera_read_query!(
    /// Read the storage-location code.
    ///
    /// APP -> Camera | 0x02/0xDA
    ///
    /// ```text
    /// request { bytes: [0x00; 7] }
    /// reply { status: u8, unknown: [u8; 5], code: u8,
    ///         extension: [u8; remaining] }
    /// ```
    CameraStorageLocationGet,
    Command::Camera(CameraCommand::StorageLocation),
    &[0; 7],
    CameraStorageData
);
