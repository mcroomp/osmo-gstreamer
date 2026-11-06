//! User ISO parameter GET.

use crate::protocol::{
    commands::reply::{camera_data, camera_read_query},
    identity::{CameraCommand, Command},
};

/// Legacy Nano user-ISO labels; numeric ISO meanings are not confirmed here.
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
    /// Return the legacy label's numeric ISO value; Auto has no fixed value.
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
    ///         code: u8 | unchanged raw code, not a physical ISO value
    ///         extension: [u8; remaining] | opaque }
    /// ```
    ///
    /// The legacy Nano labels are a candidate interpretation only. Unknown
    /// codes remain available through `code`, and aliases remain distinct there.
    CameraIsoCode,
    1
);

impl CameraIsoCode {
    pub fn code(&self) -> u8 {
        self.raw_bytes()[0]
    }

    /// Apply the legacy Nano label mapping; the raw code is retained.
    ///
    /// Aliases are not evidence of physical sensitivity or supported settings.
    /// Use `code` when those meanings have not been independently established.
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
    use crate::protocol::commands::reply::CameraReadReply;
    use crate::protocol::{decode, encode};

    #[test]
    fn iso_codes_keep_raw_aliases_and_offer_nano_labels() {
        let _query = CameraUserIsoGet;
        for (code, expected) in [
            (0x00, Some(NanoUserIso::Auto)),
            (0x01, Some(NanoUserIso::Iso100)),
            (0x02, Some(NanoUserIso::Iso100)),
            (0x03, Some(NanoUserIso::Iso100)),
            (0x0C, Some(NanoUserIso::Iso51200)),
            (0x0D, Some(NanoUserIso::Iso51200)),
            (0xFE, None),
        ] {
            let reply = decode::<CameraReadReply<CameraIsoCode>>(&[0, code, 0xAA], ()).unwrap();
            let CameraReadReply::Accepted(value) = reply else {
                panic!("expected accepted ISO code");
            };
            assert_eq!(value.code(), code);
            assert_eq!(value.nano_value(), expected);
            assert_eq!(value.raw_bytes(), [code, 0xAA]);
            assert_eq!(encode(&value).unwrap(), [code, 0xAA]);
        }
        assert_eq!(NanoUserIso::Auto.iso_value(), None);
        assert_eq!(NanoUserIso::Iso6400.iso_value(), Some(6_400));
    }
}
