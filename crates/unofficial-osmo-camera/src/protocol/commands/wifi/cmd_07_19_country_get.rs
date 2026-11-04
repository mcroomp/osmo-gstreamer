//! Wi-Fi country-data GET.

use crate::{
    Error, Result,
    protocol::{
        Query, WireDecode, WireEncode, WireReader, WireWriter,
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
#[derive(Clone, Debug, PartialEq, Eq)]
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

impl<Q> WireDecode<Q> for WifiCountryData {
    fn read_from(reader: &mut WireReader<'_>, query: &Q) -> Result<Self> {
        let raw = CameraRawData::read_from(reader, query)?;
        match raw.0.as_slice() {
            [_, _, 0] | [0xff, _, _, 0, ..] => Ok(Self { raw }),
            _ => Err(Error::Protocol("invalid Wi-Fi country reply layout")),
        }
    }
}

impl WireEncode for WifiCountryData {
    fn write_to(&self, writer: &mut WireWriter) -> Result<()> {
        self.raw.write_to(writer)
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
#[derive(Clone, Copy, Debug)]
pub struct WifiCountryGet;

impl WireEncode for WifiCountryGet {
    fn write_to(&self, _writer: &mut WireWriter) -> Result<()> {
        Ok(())
    }
}

impl Query for WifiCountryGet {
    type Response = CameraReadReply<WifiCountryData>;
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
    use crate::protocol::WireDecode;

    #[test]
    fn synthetic_country_formats_preserve_codes_terminators_and_extensions() {
        let query = WifiCountryGet;
        for code in [*b"XX", [0xff, 0xfe], [0, 0]] {
            for prefix in [false, true] {
                let mut bytes = vec![0];
                if prefix {
                    bytes.push(0xff);
                }
                bytes.extend_from_slice(&[code[0], code[1], 0]);
                let reply = CameraReadReply::<WifiCountryData>::decode(&bytes, &query).unwrap();
                let CameraReadReply::Accepted(value) = &reply else {
                    panic!("expected accepted country");
                };
                assert_eq!(value.format_prefix(), prefix.then_some(0xff));
                assert_eq!(value.country_code_raw(), code);
                assert!(value.extension().is_empty());
                assert_eq!(reply.encode().unwrap(), bytes);
                if prefix {
                    bytes.extend_from_slice(&[0xa5, 0x5a]);
                    let reply = CameraReadReply::<WifiCountryData>::decode(&bytes, &query).unwrap();
                    let CameraReadReply::Accepted(value) = &reply else {
                        panic!("expected accepted country");
                    };
                    assert_eq!(value.country_code_raw(), code);
                    assert_eq!(value.extension(), [0xa5, 0x5a]);
                    assert_eq!(reply.encode().unwrap(), bytes);
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
            assert!(CameraReadReply::<WifiCountryData>::decode(bytes, &query).is_err());
        }
        for bytes in [&[0xe0][..], &[0xe0, 0xa5, 0x5a][..]] {
            assert_eq!(
                CameraReadReply::<WifiCountryData>::decode(bytes, &query)
                    .unwrap()
                    .encode()
                    .unwrap(),
                bytes
            );
        }
    }
}
