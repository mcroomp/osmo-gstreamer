//! User shutter GET payload.

use crate::protocol::{
    Query,
    commands::reply::{CommandReadReply, RemainingBytes},
    identity::Endpoint,
};
use binrw::{BinWrite, binrw};

/// Read the configured shutter body.
///
/// APP -> Camera | 0x02/0x29
///
/// ```text
/// request {}
/// reply { status: u8, unmapped_prefix: u8, encoded_speed: le_u16,
///         decimal_digit: u8, extension: [u8; remaining] | opaque }
/// ```
#[derive(Clone, Copy, Debug, Default, BinWrite)]
#[bw(magic = b"\x02\x29")]
pub struct CameraUserShutterGet;

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
///     status: u8 | handled by CommandReadReply
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
#[binrw]
#[derive(Clone, Debug, PartialEq, Eq)]
#[brw(little)]
#[brw(magic = b"\x02\x29")]
pub struct CameraUserShutterReply {
    #[br(temp, assert(status == 0, "accepted reply requires success status"))]
    #[bw(calc = 0u8)]
    status: u8,
    pub unmapped_prefix: u8,
    pub encoded_speed: u16,
    pub decimal_digit: u8,
    pub extension: RemainingBytes,
}

impl CameraUserShutterReply {
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

impl Query for CameraUserShutterGet {
    type Response = CommandReadReply<CameraUserShutterReply>;

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
    fn shutter_reply_decodes_candidate_value_and_preserves_wire_body() {
        let _query = CameraUserShutterGet;
        let bytes = [0x00, 0x01, 0x19, 0x80, 0x00, 0xAA];
        let reply = decode_payload::<CommandReadReply<CameraUserShutterReply>>(&bytes, ()).unwrap();
        let CommandReadReply::Accepted(value) = reply else {
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
        assert_eq!(value.extension.as_slice(), [0xAA]);
        assert_eq!(CommandPacket::payload(&value).unwrap()[1..], bytes[1..]);

        let direct =
            decode_payload::<CommandReadReply<CameraUserShutterReply>>(&[0, 0, 1, 0, 3], ())
                .unwrap();
        let CommandReadReply::Accepted(direct) = direct else {
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
            assert!(decode_payload::<CommandReadReply<CameraUserShutterReply>>(bytes, ()).is_err());
        }

        for body in [[1, 0, 0, 0], [1, 0, 1, 0x0A]] {
            let reply = decode_payload::<CommandReadReply<CameraUserShutterReply>>(
                &[&[0][..], &body[..]].concat(),
                (),
            )
            .unwrap();
            let CommandReadReply::Accepted(value) = reply else {
                panic!("expected accepted shutter reply");
            };
            assert_eq!(value.speed(), None);
            assert_eq!(CommandPacket::payload(&value).unwrap()[1..], body);
        }

        let rejected =
            decode_payload::<CommandReadReply<CameraUserShutterReply>>(b"\xdfopaque", ()).unwrap();
        assert_eq!(CommandPacket::payload(&rejected).unwrap(), b"\xdfopaque");
    }
}
