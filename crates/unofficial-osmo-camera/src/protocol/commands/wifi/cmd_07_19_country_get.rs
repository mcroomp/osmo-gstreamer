//! Wi-Fi country-data GET.

use crate::{
    Error,
    protocol::{
        Query,
        commands::reply::{CommandReadReply, RemainingBytes},
        identity::Endpoint,
    },
};
use binrw::{BinWrite, binrw};

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
#[derive(Clone, Copy, Debug, BinWrite, Default)]
#[bw(magic = b"\x07\x19")]
pub struct WifiCountryGet;

/// Accepted Nano country data in compact or `FF`-prefixed form.
///
/// After the status byte, compact data is exactly `country[2], 00`.
/// Prefixed data is `FF, country[2], 00`, followed by any opaque extension.
/// Country bytes are not required to be alphabetic or UTF-8. A status-only
/// health acknowledgment is not a typed country reply.
#[binrw]
#[derive(Clone, Debug, PartialEq, Eq)]
#[brw(magic = b"\x07\x19")]
pub struct WifiCountryReply {
    #[br(temp, assert(status == 0, "accepted reply requires success status"))]
    #[bw(calc = 0u8)]
    status: u8,
    #[br(assert(matches!(raw.as_slice(), [_, _, 0] | [0xff, _, _, 0, ..]), Error::Protocol("invalid Wi-Fi country reply layout")))]
    raw: RemainingBytes,
}

impl WifiCountryReply {
    pub fn raw_bytes(&self) -> &[u8] {
        self.raw.as_slice()
    }

    /// Return `Some(0xFF)` for prefixed data, or `None` for compact data.
    pub fn format_prefix(&self) -> Option<u8> {
        (self.raw.as_slice().len() > 3).then_some(0xff)
    }

    /// Return the code bytes without requiring an alphabet or UTF-8.
    pub fn country_code_raw(&self) -> [u8; 2] {
        let start = usize::from(self.format_prefix().is_some());
        [self.raw.as_slice()[start], self.raw.as_slice()[start + 1]]
    }

    pub fn extension(&self) -> &[u8] {
        let header_length = 3 + usize::from(self.format_prefix().is_some());
        &self.raw.as_slice()[header_length..]
    }
}

impl Query for WifiCountryGet {
    type Response = CommandReadReply<WifiCountryReply>;

    fn receiver(&self) -> Endpoint {
        Endpoint::Wifi
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
    fn synthetic_country_formats_preserve_codes_terminators_and_extensions() {
        let _query = WifiCountryGet;
        for code in [*b"XX", [0xff, 0xfe], [0, 0]] {
            for prefix in [false, true] {
                let mut bytes = vec![0];
                if prefix {
                    bytes.push(0xff);
                }
                bytes.extend_from_slice(&[code[0], code[1], 0]);
                let reply =
                    decode_payload::<CommandReadReply<WifiCountryReply>>(&bytes, ()).unwrap();
                let CommandReadReply::Accepted(value) = &reply else {
                    panic!("expected accepted country");
                };
                assert_eq!(value.format_prefix(), prefix.then_some(0xff));
                assert_eq!(value.country_code_raw(), code);
                assert!(value.extension().is_empty());
                assert_eq!(CommandPacket::payload(&reply).unwrap(), bytes);
                if prefix {
                    bytes.extend_from_slice(&[0xa5, 0x5a]);
                    let reply =
                        decode_payload::<CommandReadReply<WifiCountryReply>>(&bytes, ()).unwrap();
                    let CommandReadReply::Accepted(value) = &reply else {
                        panic!("expected accepted country");
                    };
                    assert_eq!(value.country_code_raw(), code);
                    assert_eq!(value.extension(), [0xa5, 0x5a]);
                    assert_eq!(CommandPacket::payload(&reply).unwrap(), bytes);
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
            assert!(decode_payload::<CommandReadReply<WifiCountryReply>>(bytes, ()).is_err());
        }
        for bytes in [&[0xe0][..], &[0xe0, 0xa5, 0x5a][..]] {
            assert_eq!(
                CommandPacket::payload(
                    &decode_payload::<CommandReadReply<WifiCountryReply>>(bytes, ()).unwrap()
                )
                .unwrap(),
                bytes
            );
        }
    }
}
