//! Wi-Fi country-data GET.
use binrw::{BinRead, BinWrite};

use crate::{
    Error, Result,
    protocol::{
        Query,
        commands::reply::{CameraRawData, CameraReadReply},
        identity::{Command, Endpoint, WifiCommand},
    },
};

/// Accepted Nano country data in compact or `FF`-prefixed form.
///
/// After the status byte, compact data is exactly `country[2], 00`.
/// Prefixed data is `FF, country[2], 00`, followed by any opaque extension.
/// Country bytes are not required to be alphabetic or UTF-8. A status-only
/// health acknowledgment is not a typed country reply.
#[derive(Clone, Debug, PartialEq, Eq, BinRead, BinWrite)]
#[br(try_map = Self::from_raw)]
pub struct WifiCountryData {
    raw: CameraRawData,
}

impl WifiCountryData {
    pub fn raw_bytes(&self) -> &[u8] {
        &self.raw.0
    }

    /// Return `Some(0xFF)` for prefixed data, or `None` for compact data.
    pub fn format_prefix(&self) -> Option<u8> {
        (self.raw.0.len() > 3).then_some(0xff)
    }

    /// Return the code bytes without requiring an alphabet or UTF-8.
    pub fn country_code_raw(&self) -> [u8; 2] {
        let start = usize::from(self.format_prefix().is_some());
        [self.raw.0[start], self.raw.0[start + 1]]
    }

    pub fn extension(&self) -> &[u8] {
        let header_length = 3 + usize::from(self.format_prefix().is_some());
        &self.raw.0[header_length..]
    }
}

impl WifiCountryData {
    fn from_raw(raw: CameraRawData) -> Result<Self> {
        match raw.0.as_slice() {
            [_, _, 0] | [0xff, _, _, 0, ..] => Ok(Self { raw }),
            _ => Err(Error::Protocol("invalid Wi-Fi country reply layout")),
        }
    }
}

/// Read a two-byte country code, accepting both Nano reply formats.
///
/// APP -> Wifi | 0x07/0x19 | Country GET
///
/// ```text
/// request {}
/// reply { status: u8 | reject if nonzero
///         data: country_code[2], 0x00
///            or 0xFF, country_code[2], 0x00, extension[remaining] }
/// ```
///
/// A successful health check only checks status; it does not determine the
/// country format, regulatory state, network association or media readiness.
#[derive(Clone, Copy, Debug, BinWrite)]
#[bw(map = |_: &Self| ())]
pub struct WifiCountryGet;

impl Query for WifiCountryGet {
    type Response = CameraReadReply<WifiCountryData>;

    fn response_args(&self) {}

    fn receiver(&self) -> Endpoint {
        Endpoint::Wifi
    }

    fn command(&self) -> Command {
        Command::Wifi(WifiCommand::Country)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{decode, encode};

    #[test]
    fn synthetic_country_formats_preserve_codes_terminators_and_extensions() {
        let _query = WifiCountryGet;
        for code in [*b"XX", [0xff, 0xfe], [0, 0]] {
            for prefix in [false, true] {
                let mut bytes = vec![0];
                if prefix {
                    bytes.push(0xff);
                }
                bytes.extend_from_slice(&[code[0], code[1], 0]);
                let reply = decode::<CameraReadReply<WifiCountryData>>(&bytes, ()).unwrap();
                let CameraReadReply::Accepted(value) = &reply else {
                    panic!("expected accepted country");
                };
                assert_eq!(value.format_prefix(), prefix.then_some(0xff));
                assert_eq!(value.country_code_raw(), code);
                assert!(value.extension().is_empty());
                assert_eq!(encode(&reply).unwrap(), bytes);
                if prefix {
                    bytes.extend_from_slice(&[0xa5, 0x5a]);
                    let reply = decode::<CameraReadReply<WifiCountryData>>(&bytes, ()).unwrap();
                    let CameraReadReply::Accepted(value) = &reply else {
                        panic!("expected accepted country");
                    };
                    assert_eq!(value.country_code_raw(), code);
                    assert_eq!(value.extension(), [0xa5, 0x5a]);
                    assert_eq!(encode(&reply).unwrap(), bytes);
                }
            }
        }
        for bytes in [
            &[][..],
            &[0][..],
            &[0, b'X'][..],
            &[0, b'X', b'X'][..],
            &[0, b'X', b'X', 1][..],
            &[0, 0xff, b'X', b'X', 1][..],
            &[0, 0xa5, b'X', b'X', 0][..],
        ] {
            assert!(decode::<CameraReadReply::<WifiCountryData>>(bytes, ()).is_err());
        }
        for bytes in [&[0xe0][..], &[0xe0, 0xa5, 0x5a][..]] {
            assert_eq!(
                encode(&decode::<CameraReadReply::<WifiCountryData>>(bytes, ()).unwrap()).unwrap(),
                bytes
            );
        }
    }
}
