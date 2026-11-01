//! User ISO parameter GET.

use crate::protocol::{
    commands::reply::{camera_data, camera_read_query},
    identity::{CameraCommand, Command},
};

/// Nano user-ISO labels from an external model-specific table.
///
/// The wire code remains available from [`CameraIsoCode::code`]. These labels
/// do not describe ISO-limit parameter `0x000F` or establish mode support.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NanoUserIso {
    Auto,
    Iso100,
    Iso200,
    Iso400,
    Iso800,
    Iso1600,
    Iso3200,
    Iso6400,
    Iso12800,
    Iso25600,
    Iso51200,
}

impl NanoUserIso {
    /// Return the external table's numeric ISO value; Auto has no fixed value.
    pub fn iso_value(self) -> Option<u32> {
        match self {
            Self::Auto => None,
            Self::Iso100 => Some(100),
            Self::Iso200 => Some(200),
            Self::Iso400 => Some(400),
            Self::Iso800 => Some(800),
            Self::Iso1600 => Some(1_600),
            Self::Iso3200 => Some(3_200),
            Self::Iso6400 => Some(6_400),
            Self::Iso12800 => Some(12_800),
            Self::Iso25600 => Some(25_600),
            Self::Iso51200 => Some(51_200),
        }
    }
}

camera_data!(
    /// One-byte ISO code followed by any uninterpreted extension.
    ///
    /// Camera -> APP | 0x02/0x2B | ISO reply
    ///
    /// ```text
    /// reply { status: u8 | handled by CameraReadReply
    ///         code: u8 | raw code; see `nano_value` for model-specific labels
    ///         extension: [u8; remaining] | opaque }
    /// ```
    ///
    /// The external Nano labels are a candidate interpretation only. Unknown
    /// codes remain available through `code`, and aliases remain distinct there.
    CameraIsoCode,
    1
);

impl CameraIsoCode {
    pub fn code(&self) -> u8 {
        self.raw_bytes()[0]
    }

    /// Interpret codes with the external Nano mapping; the raw code is retained.
    ///
    /// Codes `0x01` and `0x02` both map to ISO 100 in that source, while
    /// `0x0D` maps to ISO 51200. Other models may assign different meanings.
    pub fn nano_value(&self) -> Option<NanoUserIso> {
        match self.code() {
            0x00 => Some(NanoUserIso::Auto),
            0x01..=0x03 => Some(NanoUserIso::Iso100),
            0x04 => Some(NanoUserIso::Iso200),
            0x05 => Some(NanoUserIso::Iso400),
            0x06 => Some(NanoUserIso::Iso800),
            0x07 => Some(NanoUserIso::Iso1600),
            0x08 => Some(NanoUserIso::Iso3200),
            0x09 => Some(NanoUserIso::Iso6400),
            0x0A => Some(NanoUserIso::Iso12800),
            0x0B => Some(NanoUserIso::Iso25600),
            0x0C..=0x0D => Some(NanoUserIso::Iso51200),
            _ => None,
        }
    }
}

camera_read_query!(
    /// Read the configured ISO parameter code.
    ///
    /// APP -> Camera | 0x02/0x2B
    ///
    /// ```text
    /// request {}
    /// reply { status: u8, code: u8, extension: [u8; remaining] }
    /// ```
    CameraUserIsoGet,
    Command::Camera(CameraCommand::UserIso),
    &[],
    CameraIsoCode
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{WireDecode, WireEncode, commands::reply::CameraReadReply};

    #[test]
    fn iso_codes_keep_raw_aliases_and_offer_nano_labels() {
        let query = CameraUserIsoGet;
        for (code, expected) in [
            (0x00, Some(NanoUserIso::Auto)),
            (0x01, Some(NanoUserIso::Iso100)),
            (0x02, Some(NanoUserIso::Iso100)),
            (0x03, Some(NanoUserIso::Iso100)),
            (0x0C, Some(NanoUserIso::Iso51200)),
            (0x0D, Some(NanoUserIso::Iso51200)),
            (0xFE, None),
        ] {
            let reply = CameraReadReply::<CameraIsoCode>::decode(&[0, code, 0xAA], &query).unwrap();
            let CameraReadReply::Accepted(value) = reply else {
                panic!("expected accepted ISO code");
            };
            assert_eq!(value.code(), code);
            assert_eq!(value.nano_value(), expected);
            assert_eq!(value.raw_bytes(), [code, 0xAA]);
            assert_eq!(value.encode().unwrap(), [code, 0xAA]);
        }
        assert_eq!(NanoUserIso::Auto.iso_value(), None);
        assert_eq!(NanoUserIso::Iso6400.iso_value(), Some(6_400));
    }
}
