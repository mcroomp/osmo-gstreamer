//! Watermark GET payload.

use crate::protocol::commands::reply::CameraRawData;
use crate::protocol::{
    commands::reply::camera_read_query,
    identity::{CameraCommand, Command},
};

camera_read_query!(
    /// Read watermark data.
    ///
    /// APP -> Camera | 0x02/0xE5
    ///
    /// ```text
    /// request { bytes: [0x20, 0, 0, 0, 0, 0, 0] }
    /// reply { status: u8, data: [u8; remaining] }
    /// ```
    CameraWatermarkGet,
    Command::Camera(CameraCommand::Watermark),
    &[0x20, 0, 0, 0, 0, 0, 0],
    CameraRawData
);
