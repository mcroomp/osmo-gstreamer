//! User shutter GET payload.

use crate::protocol::{
    commands::reply::{CameraRawData, camera_read_query},
    identity::{CameraCommand, Command},
};

const SHUTTER_DECIMAL_MAX: u8 = 9;
const SHUTTER_RECIPROCAL_MASK: u16 = 0x8000;
const SHUTTER_MAGNITUDE_MASK: u16 = 0x7fff;

/// Legacy, model-specific interpretation of raw shutter codes.
///
/// The reciprocal bit, magnitude and seconds units are not established by
/// the byte layout alone. Prefer the raw fields for interoperability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CameraShutterSpeed {
    Reciprocal {
        denominator: u16,
        decimal_digit: u8,
    },
    Seconds {
        whole_seconds: u16,
        decimal_digit: u8,
    },
}

/// Accepted shutter reply body, retaining its unmapped prefix and any tail.
///
/// Camera -> APP | 0x02/0x29 | Shutter GET reply
///
/// ```text
/// reply {
///     status: u8 | handled by CameraReadReply
///     unmapped_prefix: u8
///     encoded_speed: le_u16 | raw code; physical interpretation unspecified
///     decimal_digit: u8 | raw fractional code
///     extension: [u8; remaining] | opaque
/// }
/// ```
///
/// At least four accepted-body bytes are required. The prefix is not
/// interpreted. The empty GET uses this four-data-byte layout, distinct from
/// extended request variants. Shutter interpretation is a legacy compatibility view;
/// the original fields and extension always round-trip unchanged.
#[derive(Clone, Debug, PartialEq, Eq, binrw::BinRead, binrw::BinWrite)]
#[brw(little)]
pub struct CameraUserShutterData {
    pub unmapped_prefix: u8,
    pub encoded_speed: u16,
    pub decimal_digit: u8,
    pub extension: CameraRawData,
}

impl CameraUserShutterData {
    /// Apply the legacy reciprocal/direct interpretation without losing wire bytes.
    ///
    /// A zero magnitude or a decimal byte outside `0..=9` is left uninterpreted.
    /// Returning a value does not independently confirm physical exposure time.
    pub fn speed(&self) -> Option<CameraShutterSpeed> {
        let magnitude = self.encoded_speed & SHUTTER_MAGNITUDE_MASK;
        if magnitude == 0 || self.decimal_digit > SHUTTER_DECIMAL_MAX {
            return None;
        }
        if self.encoded_speed & SHUTTER_RECIPROCAL_MASK != 0 {
            Some(CameraShutterSpeed::Reciprocal {
                denominator: magnitude,
                decimal_digit: self.decimal_digit,
            })
        } else {
            Some(CameraShutterSpeed::Seconds {
                whole_seconds: magnitude,
                decimal_digit: self.decimal_digit,
            })
        }
    }
}

camera_read_query!(
    /// Read the configured shutter body.
    ///
    /// APP -> Camera | 0x02/0x29
    ///
    /// ```text
    /// request {}
    /// reply { status: u8, unmapped_prefix: u8, encoded_speed: le_u16,
    ///         decimal_digit: u8, extension: [u8; remaining] | opaque }
    /// ```
    CameraUserShutterGet,
    Command::Camera(CameraCommand::UserShutter),
    &[],
    CameraUserShutterData
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{decode, encode};

    use crate::protocol::commands::reply::CameraReadReply;

    #[test]
    fn shutter_reply_decodes_candidate_value_and_preserves_wire_body() {
        let _query = CameraUserShutterGet;
        let bytes = [0x00, 0x01, 0x19, 0x80, 0x00, 0xAA];
        let reply = decode::<CameraReadReply<CameraUserShutterData>>(&bytes, ()).unwrap();
        let CameraReadReply::Accepted(value) = reply else {
            panic!("expected accepted shutter reply");
        };
        assert_eq!(
            value.speed(),
            Some(CameraShutterSpeed::Reciprocal {
                denominator: 25,
                decimal_digit: 0,
            })
        );
        assert_eq!(value.unmapped_prefix, 1);
        assert_eq!(value.extension.0, [0xAA]);
        assert_eq!(encode(&value).unwrap(), bytes[1..]);

        let direct =
            decode::<CameraReadReply<CameraUserShutterData>>(&[0, 0, 1, 0, 3], ()).unwrap();
        let CameraReadReply::Accepted(direct) = direct else {
            panic!("expected accepted shutter reply");
        };
        assert_eq!(
            direct.speed(),
            Some(CameraShutterSpeed::Seconds {
                whole_seconds: 1,
                decimal_digit: 3,
            })
        );
    }

    #[test]
    fn shutter_reply_rejects_truncation_but_retains_uninterpretable_values() {
        let _query = CameraUserShutterGet;
        for bytes in [&[0][..], &[0, 1][..], &[0, 1, 2][..], &[0, 1, 2, 3][..]] {
            assert!(decode::<CameraReadReply::<CameraUserShutterData>>(bytes, ()).is_err());
        }

        for body in [[1, 0, 0, 0], [1, 0, 1, 0x0A]] {
            let reply = decode::<CameraReadReply<CameraUserShutterData>>(
                &[&[0][..], &body[..]].concat(),
                (),
            )
            .unwrap();
            let CameraReadReply::Accepted(value) = reply else {
                panic!("expected accepted shutter reply");
            };
            assert_eq!(value.speed(), None);
            assert_eq!(encode(&value).unwrap(), body);
        }

        let rejected = decode::<CameraReadReply<CameraUserShutterData>>(b"\xdfopaque", ()).unwrap();
        assert_eq!(encode(&rejected).unwrap(), b"\xdfopaque");
    }
}
