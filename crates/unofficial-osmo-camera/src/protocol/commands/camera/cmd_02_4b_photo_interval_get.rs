//! Photo-interval GET payload and response.

use crate::protocol::{
    Query,
    commands::reply::{CommandReadReply, RemainingBytes},
    identity::Endpoint,
    u16_at, u32_at,
};
use binrw::{BinWrite, binrw};

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
#[derive(Clone, Copy, Debug, Default, BinWrite)]
#[bw(magic = b"\x02\x4b")]
pub struct CameraPhotoIntervalGet;

/// Accepted photo-interval fields; units and unknown bytes remain uninterpreted.
///
/// Camera -> APP | 0x02/0x4B | Photo interval reply
///
/// ```text
/// reply { status: u8 | handled by CommandReadReply
///         first_code: u8, second_code: u8, raw_value: le_u32,
///         extension: [u8; remaining] | opaque }
/// ```
///
/// The decoder preserves the six-byte prefix without interpreting its origin.
/// The low/high halfword views are not
/// confirmed independent coarse/fine durations, and byte 1 is not a
/// confirmed photo count. No time units are assigned.
#[binrw]
#[derive(Clone, Debug, PartialEq, Eq)]
#[brw(magic = b"\x02\x4b")]
pub struct CameraPhotoIntervalReply {
    #[br(temp, assert(status == 0, "accepted reply requires success status"))]
    #[bw(calc = 0u8)]
    status: u8,
    raw: RemainingBytes<6>,
}

impl CameraPhotoIntervalReply {
    pub fn raw_bytes(&self) -> &[u8] {
        self.raw.as_slice()
    }

    /// Compatibility name for the second raw code, not a confirmed count.
    pub fn count_code(&self) -> u8 {
        self.raw_bytes()[1]
    }

    /// Low halfword of the raw LE32 value; no coarse time unit is implied.
    pub fn coarse_field(&self) -> u16 {
        u16_at(self.raw_bytes(), 2)
    }

    /// High halfword of the raw LE32 value; no fine time unit is implied.
    pub fn fine_field(&self) -> u16 {
        u16_at(self.raw_bytes(), 4)
    }

    pub fn raw_value(&self) -> u32 {
        u32_at(self.raw_bytes(), 2)
    }
}

impl Query for CameraPhotoIntervalGet {
    type Response = CommandReadReply<CameraPhotoIntervalReply>;

    fn receiver(&self) -> Endpoint {
        Endpoint::Camera
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{
        CommandPacket,
        commands::reply::{CommandReadReply, decode_payload},
    };

    #[test]
    fn both_interval_prefix_variants_preserve_the_same_word_and_tail() {
        for prefix in [[0, 1], [0xfe, 0x80]] {
            let bytes = [0, prefix[0], prefix[1], 0x78, 0x56, 0x34, 0x12, 0xa5];
            let reply =
                decode_payload::<CommandReadReply<CameraPhotoIntervalReply>>(&bytes, ()).unwrap();
            let CommandReadReply::Accepted(value) = &reply else {
                panic!("expected accepted interval");
            };
            assert_eq!(value.count_code(), prefix[1]);
            assert_eq!(value.raw_value(), 0x12345678);
            assert_eq!(value.coarse_field(), 0x5678);
            assert_eq!(value.fine_field(), 0x1234);
            assert_eq!(CommandPacket::payload(&reply).unwrap(), bytes);
        }
    }
}
