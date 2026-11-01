//! User shutter GET payload.

use crate::{
    Error, Result,
    protocol::{
        WireDecode, WireEncode, WireReader, WireWriter,
        commands::reply::{CameraRawData, camera_read_query},
        identity::{CameraCommand, Command},
    },
};

const SHUTTER_BODY_LENGTH: usize = 4;
const SHUTTER_DECIMAL_MAX: u8 = 9;
const SHUTTER_RECIPROCAL_MASK: u16 = 0x8000;
const SHUTTER_MAGNITUDE_MASK: u16 = 0x7fff;

/// Interpreted shutter value from the external user-shutter layout.
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
///     encoded_speed: le_u16 | bit 15 reciprocal; low 15 bits magnitude
///     decimal_digit: u8
///     extension: [u8; remaining] | opaque
/// }
/// ```
///
/// At least four accepted-body bytes are required. The prefix is not
/// interpreted. Shutter interpretation is an external model-specific view;
/// the original fields and extension always round-trip unchanged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CameraUserShutterData {
    pub unmapped_prefix: u8,
    pub encoded_speed: u16,
    pub decimal_digit: u8,
    pub extension: CameraRawData,
}

impl CameraUserShutterData {
    /// Interpret a valid reciprocal/direct shutter value without losing its wire form.
    ///
    /// A zero magnitude or a decimal byte outside `0..=9` is left uninterpreted.
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

impl<Q> WireDecode<Q> for CameraUserShutterData {
    fn read_from(reader: &mut WireReader<'_>, _context: &Q) -> Result<Self> {
        if reader.remaining().len() < SHUTTER_BODY_LENGTH {
            return Err(Error::Protocol("truncated camera shutter reply"));
        }
        let unmapped_prefix = reader.read_u8()?;
        let encoded_speed = reader.read_u16_le()?;
        let decimal_digit = reader.read_u8()?;
        let extension = CameraRawData(reader.read_bytes(reader.remaining().len())?.to_vec());
        Ok(Self {
            unmapped_prefix,
            encoded_speed,
            decimal_digit,
            extension,
        })
    }
}

impl WireEncode for CameraUserShutterData {
    fn write_to(&self, writer: &mut WireWriter) -> Result<()> {
        writer.write_u8(self.unmapped_prefix);
        writer.write_u16_le(self.encoded_speed);
        writer.write_u8(self.decimal_digit);
        self.extension.write_to(writer)
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
    use crate::protocol::{WireDecode, commands::reply::CameraReadReply};

    #[test]
    fn shutter_reply_decodes_candidate_value_and_preserves_wire_body() {
        let query = CameraUserShutterGet;
        let bytes = [0x00, 0x01, 0x19, 0x80, 0x00, 0xAA];
        let reply = CameraReadReply::<CameraUserShutterData>::decode(&bytes, &query).unwrap();
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
        assert_eq!(value.encode().unwrap(), bytes[1..]);

        let direct =
            CameraReadReply::<CameraUserShutterData>::decode(&[0, 0, 1, 0, 3], &query).unwrap();
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
        let query = CameraUserShutterGet;
        for bytes in [&[0][..], &[0, 1][..], &[0, 1, 2][..], &[0, 1, 2, 3][..]] {
            assert!(CameraReadReply::<CameraUserShutterData>::decode(bytes, &query).is_err());
        }

        for body in [[1, 0, 0, 0], [1, 0, 1, 0x0A]] {
            let reply = CameraReadReply::<CameraUserShutterData>::decode(
                &[&[0][..], &body[..]].concat(),
                &query,
            )
            .unwrap();
            let CameraReadReply::Accepted(value) = reply else {
                panic!("expected accepted shutter reply");
            };
            assert_eq!(value.speed(), None);
            assert_eq!(value.encode().unwrap(), body);
        }

        let rejected =
            CameraReadReply::<CameraUserShutterData>::decode(b"\xdfopaque", &query).unwrap();
        assert_eq!(rejected.encode().unwrap(), b"\xdfopaque");
    }
}
