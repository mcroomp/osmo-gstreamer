//! Targeted camera capability GET payloads and responses.

use crate::{
    Error, Result,
    protocol::{
        Query, ResponseStatus, WireDecode, WireEncode, WireReader, WireWriter,
        commands::reply::CameraRawData,
        identity::{CameraCommand, Command, Endpoint},
    },
};

const CAPABILITY_OPERATION: u8 = 2;
const CAPABILITY_COUNT: u8 = 1;
const EI_RANGE_SELECTOR: u8 = 1;
const RECOMMENDED_EI_SELECTOR: u8 = 2;
const LENS_SELECTOR: u8 = 3;
const ACCESSORY_SELECTOR: u8 = 4;
const STATUS_LENGTH: usize = 1;
const ACCESSORY_HEADER_LENGTH: usize = 4;
const ACCESSORY_OPERATION_OFFSET: usize = 1;
const ACCESSORY_STATUS_OFFSET: usize = 2;
const ACCESSORY_SELECTOR_OFFSET: usize = 3;

/// A targeted camera capability read on command 0x02/0xE0.
///
/// APP -> Camera | 0x02/0xE0
///
/// ```text
/// request {
///     operation: u8 = 0x02
///     count: u8 = 0x01
///     selector: u8 | 0x01 EI range, 0x02 recommended EI, 0x03 lens, 0x04 accessories
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
///     flags: u8
///     extension: [u8; remaining]
/// }
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapabilityQuery {
    /// Read exposure-index range data.
    EiRange,
    /// Read recommended exposure-index data.
    RecommendedEi,
    /// Read lens-information data.
    LensInformation,
    /// Read accessory capability flags.
    Accessories,
}

impl WireEncode for CapabilityQuery {
    fn write_to(&self, writer: &mut WireWriter) -> Result<()> {
        let selector = match self {
            Self::EiRange => EI_RANGE_SELECTOR,
            Self::RecommendedEi => RECOMMENDED_EI_SELECTOR,
            Self::LensInformation => LENS_SELECTOR,
            Self::Accessories => ACCESSORY_SELECTOR,
        };
        writer.write_bytes(&[CAPABILITY_OPERATION, CAPABILITY_COUNT, selector]);
        Ok(())
    }
}

impl Query for CapabilityQuery {
    type Response = CapabilityReply;

    fn receiver(&self) -> Endpoint {
        Endpoint::Camera
    }

    fn command(&self) -> Command {
        Command::Camera(CameraCommand::Capabilities)
    }
}

impl WireDecode<CapabilityQuery> for CapabilityReply {
    fn read_from(reader: &mut WireReader<'_>, query: &CapabilityQuery) -> Result<Self> {
        let length = reader.remaining().len();
        let payload = reader.read_bytes(length)?;
        let invalid = |reason| Error::InvalidReply {
            command: query.command(),
            reason,
        };
        let status = payload
            .first()
            .copied()
            .map(ResponseStatus::from)
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
            let status = ResponseStatus::from(payload[ACCESSORY_STATUS_OFFSET]);
            if status != ResponseStatus::Success {
                return Ok(Self::Rejected {
                    status,
                    layer: CapabilityStatusLayer::Accessory,
                    extension: CameraRawData(payload[ACCESSORY_HEADER_LENGTH..].to_vec()),
                });
            }
            return Ok(Self::Accepted(CapabilityValue::Accessories(
                AccessoryCapabilityResponse::decode(&payload[ACCESSORY_HEADER_LENGTH..], &())
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
/// `CapabilityReply` validates the header. Flag meanings and the unknown byte
/// remain uninterpreted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccessoryCapabilityResponse {
    pub raw_flags: u8,
    pub unmapped_byte: u8,
    pub extension: CameraRawData,
}

impl WireDecode for AccessoryCapabilityResponse {
    fn read_from(reader: &mut WireReader<'_>, context: &()) -> Result<Self> {
        Ok(Self {
            unmapped_byte: reader.read_u8()?,
            raw_flags: reader.read_u8()?,
            extension: CameraRawData::read_from(reader, context)?,
        })
    }
}

impl WireEncode for AccessoryCapabilityResponse {
    fn write_to(&self, writer: &mut WireWriter) -> Result<()> {
        writer.write_u8(self.unmapped_byte);
        writer.write_u8(self.raw_flags);
        self.extension.write_to(writer)
    }
}

impl WireEncode for CapabilityReply {
    fn write_to(&self, writer: &mut WireWriter) -> Result<()> {
        match self {
            Self::Accepted(CapabilityValue::Accessories(value)) => {
                ResponseStatus::Success.write_to(writer)?;
                writer.write_u8(CAPABILITY_OPERATION);
                ResponseStatus::Success.write_to(writer)?;
                writer.write_u8(ACCESSORY_SELECTOR);
                value.write_to(writer)
            }
            Self::Accepted(CapabilityValue::Unmapped { data }) => {
                if data.is_empty() {
                    return Err(Error::Protocol("accepted capability reply has no data"));
                }
                ResponseStatus::Success.write_to(writer)?;
                writer.write_bytes(data);
                Ok(())
            }
            Self::Rejected {
                status,
                layer,
                extension,
            } => {
                if *status == ResponseStatus::Success {
                    return Err(Error::Protocol(
                        "rejected capability cannot have success status",
                    ));
                }
                if *layer == CapabilityStatusLayer::Accessory {
                    ResponseStatus::Success.write_to(writer)?;
                    writer.write_u8(CAPABILITY_OPERATION);
                }
                status.write_to(writer)?;
                if *layer == CapabilityStatusLayer::Accessory {
                    writer.write_u8(ACCESSORY_SELECTOR);
                }
                extension.write_to(writer)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{WireDecode, WireU8};

    #[test]
    fn capability_selectors_match_command_payloads() {
        for (query, selector) in [
            (CapabilityQuery::EiRange, 1),
            (CapabilityQuery::RecommendedEi, 2),
            (CapabilityQuery::LensInformation, 3),
            (CapabilityQuery::Accessories, 4),
        ] {
            assert_eq!(query.encode().unwrap(), [2, 1, selector]);
            assert_eq!(query.receiver(), Endpoint::Camera);
            assert_eq!(query.command().wire(), (0x02, 0xe0));
        }
    }

    #[test]
    fn accessory_status_and_unknown_fields_are_preserved() {
        let query = CapabilityQuery::Accessories;
        let payload = [0, 2, 0, 4, 0xa5, 0x87, 0x99];
        let reply = CapabilityReply::decode(&payload, &query).unwrap();
        assert_eq!(
            reply,
            CapabilityReply::Accepted(CapabilityValue::Accessories(AccessoryCapabilityResponse {
                raw_flags: 0x87,
                unmapped_byte: 0xa5,
                extension: CameraRawData(vec![0x99]),
            }))
        );
        assert!(matches!(
            CapabilityReply::decode(&[0, 2, 0xe0, 4, 0, 0], &query).unwrap(),
            CapabilityReply::Rejected {
                status: ResponseStatus::Unknown(0xe0),
                layer: CapabilityStatusLayer::Accessory,
                ..
            }
        ));
        assert_eq!(
            CapabilityReply::decode(&[0xe0, 2, 0], &query).unwrap(),
            CapabilityReply::Rejected {
                status: ResponseStatus::Unknown(0xe0),
                layer: CapabilityStatusLayer::Outer,
                extension: CameraRawData(vec![2, 0]),
            }
        );
    }

    #[test]
    fn unmapped_data_is_retained_and_incomplete_success_is_invalid() {
        let query = CapabilityQuery::EiRange;
        assert_eq!(
            CapabilityReply::decode(b"\x00\xde\xad", &query).unwrap(),
            CapabilityReply::Accepted(CapabilityValue::Unmapped {
                data: vec![0xde, 0xad]
            })
        );
        assert!(CapabilityReply::decode(&[0], &query).is_err());
        let status = ResponseStatus::from_wire_u8(0xe0).unwrap();
        assert_eq!(status.to_wire_u8(), 0xe0);
    }

    #[test]
    fn capability_replies_round_trip_extensions_and_reject_incomplete_headers() {
        let query = CapabilityQuery::Accessories;
        for bytes in [
            &[0, 2, 0, 4, 0xa5, 0xff, 0xab][..],
            &[0, 2, 0xe0, 4, 0xab][..],
            &[0xe0, 0xab][..],
        ] {
            let reply = CapabilityReply::decode(bytes, &query).unwrap();
            assert_eq!(reply.encode().unwrap(), bytes);
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
            assert!(CapabilityReply::decode(bytes, &query).is_err());
        }
        let bytes = [0, 0xab, 0xcd];
        let reply = CapabilityReply::decode(&bytes, &CapabilityQuery::EiRange).unwrap();
        assert_eq!(reply.encode().unwrap(), bytes);
        assert!(
            CapabilityReply::Rejected {
                status: ResponseStatus::Success,
                layer: CapabilityStatusLayer::Outer,
                extension: CameraRawData(vec![]),
            }
            .encode()
            .is_err()
        );
    }
}
