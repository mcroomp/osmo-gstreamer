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
    ///
    /// The Nano reply retains bits 0..5 and clears bits 6..7. Values are raw
    /// codes: neither nonzero nor the low two bits establish a lock-state enum.
    /// The decoder retains all received bits unchanged.
    CameraAeLockValue,
    1
);

impl CameraAeLockValue {
    /// Test only the low two bits; this is not a confirmed physical lock state.
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
