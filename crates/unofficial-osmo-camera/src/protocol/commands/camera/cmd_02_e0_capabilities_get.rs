//! Targeted camera capability GET payloads and responses.
use crate::protocol::binrw_error;
use crate::protocol::decode;
use binrw::{BinRead, BinResult, BinWrite, Endian};
use std::io::{Read, Seek, Write};

use crate::{
    Error, Result,
    protocol::{
        Query, ResponseStatus,
        commands::reply::CameraRawData,
        identity::{CameraCommand, Command, Endpoint},
    },
};

const CAPABILITY_OPERATION: u8 = 2;
const ACCESSORY_SELECTOR: u8 = 4;
const STATUS_LENGTH: usize = 1;
const ACCESSORY_HEADER_LENGTH: usize = 4;
const ACCESSORY_OPERATION_OFFSET: usize = 1;
const ACCESSORY_STATUS_OFFSET: usize = 2;
const ACCESSORY_SELECTOR_OFFSET: usize = 3;
const ACCESSORY_LIVE_STREAMING_MASK: u8 = 0x01;
const ACCESSORY_WIRELESS_MICROPHONE_MASK: u8 = 0x02;
const ACCESSORY_WIFI_SHARE_MASK: u8 = 0x04;

/// A targeted camera capability read on command 0x02/0xE0.
///
/// APP -> Camera | 0x02/0xE0
///
/// ```text
/// request {
///     operation: u8 = 0x02
///     count: u8 = 0x01
///     selector: u8 | 0x01..0x04; names below are compatibility labels
/// }
/// reply {
///     status: u8 | reject if nonzero
///     data: [u8; remaining] | nonempty for selectors 0x01 through 0x03
/// }
/// accessory_reply {
///     status: u8 | reject if nonzero
///     operation: u8 = 0x02
///     accessory_status: u8
///     selector: u8 = 0x04
///     guard | reject if accessory_status is nonzero
///     unknown: u8
///     flags: u8 | raw bits, semantic labels unconfirmed
///     extension: [u8; remaining]
/// }
/// ```
///
/// This is the library's compatibility codec, not a confirmed device support
/// list. The selector names and accessory flag meanings remain unconfirmed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, BinWrite)]
pub enum CapabilityQuery {
    /// Read exposure-index range data.
    #[bw(magic = b"\x02\x01\x01")]
    EiRange,
    /// Read recommended exposure-index data.
    #[bw(magic = b"\x02\x01\x02")]
    RecommendedEi,
    /// Read lens-information data.
    #[bw(magic = b"\x02\x01\x03")]
    LensInformation,
    /// Read accessory capability flags.
    #[bw(magic = b"\x02\x01\x04")]
    Accessories,
}

impl Query for CapabilityQuery {
    type Response = CapabilityReply;

    fn response_args(&self) -> (&Self,) {
        (self,)
    }

    fn receiver(&self) -> Endpoint {
        Endpoint::Camera
    }

    fn command(&self) -> Command {
        Command::Camera(CameraCommand::Capabilities)
    }
}

impl BinRead for CapabilityReply {
    type Args<'a> = (&'a CapabilityQuery,);

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        endian: Endian,
        (query,): Self::Args<'_>,
    ) -> BinResult<Self> {
        let pos = reader.stream_position()?;
        let raw = CameraRawData::read_options(reader, endian, ())?;
        Self::from_payload(&raw.0, query).map_err(|error| binrw_error(pos, error))
    }
}

impl CapabilityReply {
    fn from_payload(payload: &[u8], query: &CapabilityQuery) -> Result<Self> {
        let invalid = |reason| Error::InvalidReply {
            command: query.command(),
            reason,
        };
        let status = payload
            .first()
            .copied()
            .map(|value| decode::<ResponseStatus>(&[value], ()))
            .transpose()?
            .ok_or_else(|| invalid("missing capability status byte"))?;
        if status != ResponseStatus::Success {
            return Ok(Self::Rejected {
                status,
                layer: CapabilityStatusLayer::Outer,
                extension: CameraRawData(payload[STATUS_LENGTH..].to_vec()),
            });
        }
        if query == &CapabilityQuery::Accessories {
            if payload.len() < ACCESSORY_HEADER_LENGTH
                || payload[ACCESSORY_OPERATION_OFFSET] != CAPABILITY_OPERATION
                || payload[ACCESSORY_SELECTOR_OFFSET] != ACCESSORY_SELECTOR
            {
                return Err(invalid(
                    "accessory reply operation or selector does not match",
                ));
            }
            let status = decode::<ResponseStatus>(&[payload[ACCESSORY_STATUS_OFFSET]], ())?;
            if status != ResponseStatus::Success {
                return Ok(Self::Rejected {
                    status,
                    layer: CapabilityStatusLayer::Accessory,
                    extension: CameraRawData(payload[ACCESSORY_HEADER_LENGTH..].to_vec()),
                });
            }
            return Ok(Self::Accepted(CapabilityValue::Accessories(
                decode::<AccessoryCapabilityResponse>(&payload[ACCESSORY_HEADER_LENGTH..], ())
                    .map_err(|_| invalid("truncated accessory flags"))?,
            )));
        }
        if payload.len() == STATUS_LENGTH {
            return Err(invalid("accepted capability reply has no data"));
        }
        Ok(Self::Accepted(CapabilityValue::Unmapped {
            data: payload[STATUS_LENGTH..].to_vec(),
        }))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapabilityStatusLayer {
    Outer,
    Accessory,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Decode targeted capability replies, preserving both rejection layers.
///
/// Camera -> APP | 0x02/0xE0 | Capability reply
///
/// ```text
/// reply { status: u8, data: [u8; remaining] | selectors 0x01..0x03, opaque }
/// accessory_reply {
///     status: u8
///     operation: u8 = 0x02
///     accessory_status: u8
///     selector: u8 = 0x04
///     unknown: u8, flags: u8 | only on acceptance
///     extension: [u8; remaining] | opaque, retained also on rejection
/// }
/// ```
///
/// The request determines the reply variant. Accepted non-accessory data must
/// be nonempty. A rejected outer reply does not require an accessory header.
pub enum CapabilityReply {
    Accepted(CapabilityValue),
    Rejected {
        status: ResponseStatus,
        layer: CapabilityStatusLayer,
        extension: CameraRawData,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CapabilityValue {
    Accessories(AccessoryCapabilityResponse),
    Unmapped { data: Vec<u8> },
}

/// Accepted accessory body after the status/operation/status/selector header.
///
/// Camera -> APP | 0x02/0xE0 | Accessory capability reply
///
/// ```text
/// reply { status: u8 = 0x00, operation: u8 = 0x02,
///         accessory_status: u8 = 0x00, selector: u8 = 0x04,
///         unknown: u8, flags: u8, extension: [u8; remaining] | opaque }
/// ```
///
/// `CapabilityReply` validates the header. The legacy `supports_*` helpers test
/// bits 0/1/2 only; their labels are not confirmed availability, enabled state,
/// readiness or authorization. The unknown byte and other bits are retained.
#[derive(Clone, Debug, PartialEq, Eq, BinRead, BinWrite)]
pub struct AccessoryCapabilityResponse {
    pub unmapped_byte: u8,
    pub raw_flags: u8,
    pub extension: CameraRawData,
}

impl AccessoryCapabilityResponse {
    /// Legacy bit-0 view; no streaming availability or readiness guarantee.
    pub fn supports_live_streaming(&self) -> bool {
        self.raw_flags & ACCESSORY_LIVE_STREAMING_MASK != 0
    }

    /// Legacy bit-1 view; no microphone availability guarantee.
    pub fn supports_wireless_microphone(&self) -> bool {
        self.raw_flags & ACCESSORY_WIRELESS_MICROPHONE_MASK != 0
    }

    /// Legacy bit-2 view; no Wi-Fi-sharing availability guarantee.
    pub fn supports_wifi_share(&self) -> bool {
        self.raw_flags & ACCESSORY_WIFI_SHARE_MASK != 0
    }
}

impl BinWrite for CapabilityReply {
    type Args<'a> = ();

    fn write_options<W: Write + Seek>(
        &self,
        writer: &mut W,
        endian: Endian,
        (): (),
    ) -> BinResult<()> {
        let pos = writer.stream_position()?;
        match self {
            Self::Accepted(CapabilityValue::Accessories(value)) => (
                ResponseStatus::Success,
                CAPABILITY_OPERATION,
                ResponseStatus::Success,
                ACCESSORY_SELECTOR,
                value,
            )
                .write_options(writer, endian, ()),
            Self::Accepted(CapabilityValue::Unmapped { data }) => {
                if data.is_empty() {
                    return Err(binrw_error(
                        pos,
                        Error::Protocol("accepted capability reply has no data"),
                    ));
                }
                ResponseStatus::Success.write_options(writer, endian, ())?;
                writer.write_all(data)?;
                Ok(())
            }
            Self::Rejected {
                status,
                layer,
                extension,
            } => {
                if *status == ResponseStatus::Success {
                    return Err(binrw_error(
                        pos,
                        Error::Protocol("rejected capability cannot have success status"),
                    ));
                }
                if *layer == CapabilityStatusLayer::Accessory {
                    (ResponseStatus::Success, CAPABILITY_OPERATION).write_options(
                        writer,
                        endian,
                        (),
                    )?;
                }
                status.write_options(writer, endian, ())?;
                if *layer == CapabilityStatusLayer::Accessory {
                    ACCESSORY_SELECTOR.write_options(writer, endian, ())?;
                }
                extension.write_options(writer, endian, ())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{decode, encode};

    #[test]
    fn accessory_status_and_unknown_fields_are_preserved() {
        let query = CapabilityQuery::Accessories;
        let payload = [0, 2, 0, 4, 0xa5, 0x87, 0x99];
        let reply = decode::<CapabilityReply>(&payload, (&query,)).unwrap();
        assert_eq!(
            reply,
            CapabilityReply::Accepted(CapabilityValue::Accessories(AccessoryCapabilityResponse {
                raw_flags: 0x87,
                unmapped_byte: 0xa5,
                extension: CameraRawData(vec![0x99]),
            }))
        );
        assert!(matches!(
            decode::<CapabilityReply>(&[0, 2, 0xe0, 4, 0, 0], (&query,)).unwrap(),
            CapabilityReply::Rejected {
                status: ResponseStatus::Unknown(0xe0),
                layer: CapabilityStatusLayer::Accessory,
                ..
            }
        ));
        assert_eq!(
            decode::<CapabilityReply>(&[0xe0, 2, 0], (&query,)).unwrap(),
            CapabilityReply::Rejected {
                status: ResponseStatus::Unknown(0xe0),
                layer: CapabilityStatusLayer::Outer,
                extension: CameraRawData(vec![2, 0]),
            }
        );
    }

    #[test]
    fn synthetic_accessory_support_flags_preserve_all_bits_and_extensions() {
        let mut bytes = [0, 2, 0, 4, 0xa5, 0, 0x5a];
        for flags in [0, 1, 2, 4, 0x80, 0xff] {
            bytes[5] = flags;
            let reply =
                decode::<CapabilityReply>(&bytes, (&CapabilityQuery::Accessories,)).unwrap();
            let CapabilityReply::Accepted(CapabilityValue::Accessories(value)) = &reply else {
                panic!("expected accepted accessories");
            };
            assert_eq!(value.supports_live_streaming(), flags & 1 != 0);
            assert_eq!(value.supports_wireless_microphone(), flags & 2 != 0);
            assert_eq!(value.supports_wifi_share(), flags & 4 != 0);
            assert_eq!(value.raw_flags, flags);
            assert_eq!(value.unmapped_byte, 0xa5);
            assert_eq!(value.extension.0, [0x5a]);
            assert_eq!(encode(&reply).unwrap(), bytes);
        }
    }

    #[test]
    fn unmapped_data_is_retained_and_incomplete_success_is_invalid() {
        let query = CapabilityQuery::EiRange;
        assert_eq!(
            decode::<CapabilityReply>(b"\x00\xde\xad", (&query,)).unwrap(),
            CapabilityReply::Accepted(CapabilityValue::Unmapped {
                data: vec![0xde, 0xad]
            })
        );
        assert!(decode::<CapabilityReply>(&[0], (&query,)).is_err());
        let status = decode::<ResponseStatus>(&[0xe0], ()).unwrap();
        assert_eq!(
            encode(&status).expect("one-byte enum encodes into memory")[0],
            0xe0
        );
    }

    #[test]
    fn capability_replies_round_trip_extensions_and_reject_incomplete_headers() {
        let query = CapabilityQuery::Accessories;
        for bytes in [
            &[0, 2, 0, 4, 0xa5, 0xff, 0xab][..],
            &[0, 2, 0xe0, 4, 0xab][..],
            &[0xe0, 0xab][..],
        ] {
            let reply = decode::<CapabilityReply>(bytes, (&query,)).unwrap();
            assert_eq!(encode(&reply).unwrap(), bytes);
        }
        for bytes in [
            &[][..],
            &[0][..],
            &[0, 2][..],
            &[0, 2, 0][..],
            &[0, 2, 0, 4][..],
            &[0, 2, 0, 4, 0xff][..],
            &[0, 1, 0, 4, 0, 0][..],
            &[0, 2, 0, 3, 0, 0][..],
        ] {
            assert!(decode::<CapabilityReply>(bytes, (&query,)).is_err());
        }
        let bytes = [0, 0xab, 0xcd];
        let reply = decode::<CapabilityReply>(&bytes, (&CapabilityQuery::EiRange,)).unwrap();
        assert_eq!(encode(&reply).unwrap(), bytes);
        assert!(
            encode(&CapabilityReply::Rejected {
                status: ResponseStatus::Success,
                layer: CapabilityStatusLayer::Outer,
                extension: CameraRawData(vec![]),
            })
            .is_err()
        );
    }
}
