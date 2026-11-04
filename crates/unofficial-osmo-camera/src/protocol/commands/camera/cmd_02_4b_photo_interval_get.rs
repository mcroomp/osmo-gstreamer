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
    ///         first_code: u8, second_code: u8, raw_value: le_u32,
    ///         extension: [u8; remaining] | opaque }
    /// ```
    ///
    /// One Nano variant emits `00 01` followed by a raw LE32 value. Another
    /// supplies all six bytes directly. The low/high halfword views are not
    /// confirmed independent coarse/fine durations, and byte 1 is not a
    /// confirmed photo count. No time units are assigned.
    CameraIntervalData,
    6
);

impl CameraIntervalData {
    /// Compatibility name for the second raw code, not a confirmed count.
    pub fn count_code(&self) -> u8 {
        self.raw_bytes()[1]
    }

    /// Low halfword of the raw LE32 value; no coarse time unit is implied.
    pub fn coarse_field(&self) -> u16 {
        u16::from_le_bytes([self.raw_bytes()[2], self.raw_bytes()[3]])
    }

    /// High halfword of the raw LE32 value; no fine time unit is implied.
    pub fn fine_field(&self) -> u16 {
        u16::from_le_bytes([self.raw_bytes()[4], self.raw_bytes()[5]])
    }

    pub fn raw_value(&self) -> u32 {
        u32::from_le_bytes([
            self.raw_bytes()[2],
            self.raw_bytes()[3],
            self.raw_bytes()[4],
            self.raw_bytes()[5],
        ])
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
    ///     first_code: u8
    ///     second_code: u8
    ///     raw_value: le_u32
    ///     extension: [u8; remaining]
    /// }
    /// ```
    CameraPhotoIntervalGet,
    Command::Camera(CameraCommand::PhotoInterval),
    &[],
    CameraIntervalData
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{WireDecode, WireEncode, commands::reply::CameraReadReply};

    #[test]
    fn both_interval_prefix_variants_preserve_the_same_word_and_tail() {
        for prefix in [[0, 1], [0xfe, 0x80]] {
            let bytes = [0, prefix[0], prefix[1], 0x78, 0x56, 0x34, 0x12, 0xa5];
            let reply =
                CameraReadReply::<CameraIntervalData>::decode(&bytes, &CameraPhotoIntervalGet)
                    .unwrap();
            let CameraReadReply::Accepted(value) = &reply else {
                panic!("expected accepted interval");
            };
            assert_eq!(value.count_code(), prefix[1]);
            assert_eq!(value.raw_value(), 0x12345678);
            assert_eq!(value.coarse_field(), 0x5678);
            assert_eq!(value.fine_field(), 0x1234);
            assert_eq!(reply.encode().unwrap(), bytes);
        }
    }
}
