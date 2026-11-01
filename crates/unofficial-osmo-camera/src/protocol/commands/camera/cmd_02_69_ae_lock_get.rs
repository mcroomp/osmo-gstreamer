//! Auto-exposure lock GET payload and response.

use crate::protocol::{
    commands::reply::{camera_data, camera_read_query},
    identity::{CameraCommand, Command},
};

camera_data!(
    /// Accepted auto-exposure-lock byte, preserving all bits.
    ///
    /// Camera -> APP | 0x02/0x69 | AE lock reply
    ///
    /// ```text
    /// reply { status: u8 | handled by CameraReadReply
    ///         code: u8, extension: [u8; remaining] | opaque }
    /// ```
    CameraAeLockValue,
    1
);

impl CameraAeLockValue {
    pub fn lock_bits_set(&self) -> bool {
        self.raw_bytes()[0] & 3 != 0
    }
}

camera_read_query!(
    /// Read the auto-exposure lock value.
    ///
    /// APP -> Camera | 0x02/0x69
    ///
    /// ```text
    /// request {}
    /// reply { status: u8, code: u8, extension: [u8; remaining] }
    /// ```
    CameraAeLockGet,
    Command::Camera(CameraCommand::AeLock),
    &[],
    CameraAeLockValue
);
