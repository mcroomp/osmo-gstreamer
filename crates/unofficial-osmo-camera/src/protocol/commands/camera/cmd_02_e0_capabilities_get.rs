//! Targeted camera capability reads with one query type per selector.
//!
//! Selector names and accessory flag labels are compatibility candidates, not
//! confirmed availability or authorization. The first three replies remain
//! opaque; accessories retain distinct outer and inner rejection layers.

use crate::{
    Error,
    protocol::{
        Query, ResponseStatus,
        commands::reply::{CameraRawData, CameraReadReply, CommandReadReply},
        identity::Endpoint,
    },
};
use binrw::{BinRead, BinWrite, binrw, binwrite};

/// Read exposure-index range data; selector 1 on camera command 02/E0.
#[binwrite]
#[derive(Clone, Copy, Debug, Default)]
#[bw(magic = b"\x02\xe0")]
pub struct EiRangeGet {
    #[bw(calc = <Self as Query>::REQUEST_PAYLOAD)]
    payload: &'static [u8],
}

/// Read recommended exposure-index data; selector 2 on camera command 02/E0.
#[binwrite]
#[derive(Clone, Copy, Debug, Default)]
#[bw(magic = b"\x02\xe0")]
pub struct RecommendedEiGet {
    #[bw(calc = <Self as Query>::REQUEST_PAYLOAD)]
    payload: &'static [u8],
}

/// Read lens-information data; selector 3 on camera command 02/E0.
#[binwrite]
#[derive(Clone, Copy, Debug, Default)]
#[bw(magic = b"\x02\xe0")]
pub struct LensInformationGet {
    #[bw(calc = <Self as Query>::REQUEST_PAYLOAD)]
    payload: &'static [u8],
}

/// Read accessory flags; selector 4 has both outer and inner status bytes.
#[binwrite]
#[derive(Clone, Copy, Debug, Default)]
#[bw(magic = b"\x02\xe0")]
pub struct AccessoriesGet {
    #[bw(calc = <Self as Query>::REQUEST_PAYLOAD)]
    payload: &'static [u8],
}

/// Opaque, nonempty exposure-index range body; no numeric layout is established.
#[binrw]
#[derive(Clone, Debug, PartialEq, Eq)]
#[brw(magic = b"\x02\xe0")]
pub struct EiRangeReply {
    #[br(temp, assert(status == 0, "accepted reply requires success status"))]
    #[bw(calc = 0u8)]
    status: u8,
    #[br(assert(!raw.0.is_empty(), "accepted EI range has no data"))]
    raw: CameraRawData,
}

impl EiRangeReply {
    pub fn raw_bytes(&self) -> &[u8] {
        &self.raw.0
    }
}

/// Opaque, nonempty recommended exposure-index body.
#[binrw]
#[derive(Clone, Debug, PartialEq, Eq)]
#[brw(magic = b"\x02\xe0")]
pub struct RecommendedEiReply {
    #[br(temp, assert(status == 0, "accepted reply requires success status"))]
    #[bw(calc = 0u8)]
    status: u8,
    #[br(assert(!raw.0.is_empty(), "accepted recommended EI has no data"))]
    raw: CameraRawData,
}

impl RecommendedEiReply {
    pub fn raw_bytes(&self) -> &[u8] {
        &self.raw.0
    }
}

/// Opaque, nonempty lens-information body.
#[binrw]
#[derive(Clone, Debug, PartialEq, Eq)]
#[brw(magic = b"\x02\xe0")]
pub struct LensInformationReply {
    #[br(temp, assert(status == 0, "accepted reply requires success status"))]
    #[bw(calc = 0u8)]
    status: u8,
    #[br(assert(!raw.0.is_empty(), "accepted lens information has no data"))]
    raw: CameraRawData,
}

impl LensInformationReply {
    pub fn raw_bytes(&self) -> &[u8] {
        &self.raw.0
    }
}

/// Complete command reply, including command identity and status-bearing data.
#[derive(Clone, Debug, PartialEq, Eq, BinRead, BinWrite)]
#[brw(magic = b"\x02\xe0")]
pub struct AccessoriesReply {
    pub result: CameraReadReply<AccessoriesData>,
}

impl Query for EiRangeGet {
    type Response = CommandReadReply<EiRangeReply>;
    const REQUEST_PAYLOAD: &'static [u8] = &[0x02, 0x01, 0x01];

    fn receiver(&self) -> Endpoint {
        Endpoint::Camera
    }
}

impl Query for RecommendedEiGet {
    type Response = CommandReadReply<RecommendedEiReply>;
    const REQUEST_PAYLOAD: &'static [u8] = &[0x02, 0x01, 0x02];

    fn receiver(&self) -> Endpoint {
        Endpoint::Camera
    }
}

impl Query for LensInformationGet {
    type Response = CommandReadReply<LensInformationReply>;
    const REQUEST_PAYLOAD: &'static [u8] = &[0x02, 0x01, 0x03];

    fn receiver(&self) -> Endpoint {
        Endpoint::Camera
    }
}

impl Query for AccessoriesGet {
    type Response = AccessoriesReply;
    const REQUEST_PAYLOAD: &'static [u8] = &[0x02, 0x01, 0x04];

    fn receiver(&self) -> Endpoint {
        Endpoint::Camera
    }
}

/// Inner accessory reply after the shared outer status has been accepted.
///
/// Operation 2 and selector 4 must match even when the inner status rejects.
/// The enclosing [`CameraReadReply`] preserves outer rejections independently.
#[derive(Clone, Debug, PartialEq, Eq, BinRead, BinWrite)]
pub enum AccessoriesData {
    #[brw(magic = b"\x02\x00\x04")]
    Accepted(AccessoryCapabilityResponse),
    #[brw(magic = 2u8)]
    Rejected {
        #[br(assert(status != ResponseStatus::Success, Error::Protocol("rejected capability cannot have success status")))]
        #[bw(assert(*status != ResponseStatus::Success, Error::Protocol("rejected capability cannot have success status")))]
        status: ResponseStatus,
        #[brw(magic = 4u8)]
        extension: CameraRawData,
    },
}

/// Accepted accessory fields after status/operation/status/selector validation.
///
/// Legacy helpers test bits 0/1/2 only. These are not readiness, availability,
/// enabled-state or authorization guarantees; unknown bits and extensions remain raw.
#[derive(Clone, Debug, PartialEq, Eq, BinRead, BinWrite)]
pub struct AccessoryCapabilityResponse {
    pub unmapped_byte: u8,
    pub raw_flags: u8,
    pub extension: CameraRawData,
}

impl AccessoryCapabilityResponse {
    pub fn supports_live_streaming(&self) -> bool {
        self.raw_flags & 1 != 0
    }
    pub fn supports_wireless_microphone(&self) -> bool {
        self.raw_flags & 2 != 0
    }
    pub fn supports_wifi_share(&self) -> bool {
        self.raw_flags & 4 != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{decode, encode};
    use binrw::Endian;
    use std::io::Cursor;

    #[test]
    fn accessory_native_codecs_preserve_layers_at_nonzero_offsets() {
        for bytes in [
            vec![2, 0, 4, 0xa5, 0x82, 0x99],
            vec![2, 0xe0, 4, 0xaa],
            vec![2, 0xe0, 4],
        ] {
            let mut framed = vec![0xa5; 3];
            framed.extend_from_slice(&bytes);
            let mut reader = Cursor::new(&framed);
            reader.set_position(3);
            let data = AccessoriesData::read_options(&mut reader, Endian::Big, ()).unwrap();
            assert_eq!(reader.position(), framed.len() as u64);
            let mut writer = Cursor::new(vec![0xa5; 3]);
            writer.set_position(3);
            data.write_options(&mut writer, Endian::Big, ()).unwrap();
            assert_eq!(writer.into_inner(), framed);
        }
        let invalid = AccessoriesData::Rejected {
            status: ResponseStatus::Success,
            extension: CameraRawData(vec![0xaa]),
        };
        assert!(encode(&invalid).is_err());
        assert!(
            invalid
                .write_options(&mut Cursor::new(Vec::new()), Endian::Big, ())
                .is_err()
        );
        assert!(matches!(
            decode::<AccessoriesData>(&[1, 0, 4, 0, 0], ()),
            Err(Error::WireCodec { .. })
        ));
    }

    #[test]
    fn accessory_outer_rejection_does_not_require_or_interpret_an_inner_header() {
        let reply = decode::<CameraReadReply<AccessoriesData>>(&[0xe0, 2, 0, 3], ()).unwrap();
        assert_eq!(
            reply,
            CameraReadReply::Rejected {
                status: ResponseStatus::Unknown(0xe0),
                extension: CameraRawData(vec![2, 0, 3]),
            }
        );
        assert_eq!(encode(&reply).unwrap(), [0xe0, 2, 0, 3]);
    }

    #[test]
    fn inner_rejection_still_requires_the_correct_operation_and_selector() {
        let reply = decode::<CameraReadReply<AccessoriesData>>(&[0, 2, 0xe0, 4, 0xaa], ()).unwrap();
        assert_eq!(
            reply,
            CameraReadReply::Accepted(AccessoriesData::Rejected {
                status: ResponseStatus::Unknown(0xe0),
                extension: CameraRawData(vec![0xaa]),
            })
        );
        for bytes in [[0, 1, 0xe0, 4], [0, 2, 0xe0, 3], [0, 2, 0xe0, 5]] {
            assert!(decode::<CameraReadReply<AccessoriesData>>(&bytes, ()).is_err());
        }
        assert_eq!(encode(&reply).unwrap(), [0, 2, 0xe0, 4, 0xaa]);
    }

    #[test]
    fn accessory_acceptance_requires_flags_and_keeps_unmapped_bits() {
        for bytes in [vec![0], vec![0, 2], vec![0, 2, 0, 4], vec![0, 2, 0, 4, 255]] {
            assert!(decode::<CameraReadReply<AccessoriesData>>(&bytes, ()).is_err());
        }
        let CameraReadReply::Accepted(AccessoriesData::Accepted(value)) =
            decode::<CameraReadReply<AccessoriesData>>(&[0, 2, 0, 4, 0xa5, 0x82, 0x99], ())
                .unwrap()
        else {
            panic!("expected accepted accessories");
        };
        assert!(!value.supports_live_streaming());
        assert!(value.supports_wireless_microphone());
        assert!(!value.supports_wifi_share());
        assert_eq!(value.raw_flags, 0x82);
        assert_eq!(value.unmapped_byte, 0xa5);
        assert_eq!(value.extension.0, [0x99]);
    }

    #[test]
    fn opaque_capabilities_require_data_only_after_success() {
        fn check<Q: Query>() {
            let mut bytes = Q::COMMAND_ID.to_vec();
            bytes.push(0);
            assert!(decode::<Q::Response>(&bytes, ()).is_err());
            bytes.push(0xff);
            assert!(decode::<Q::Response>(&bytes, ()).is_ok());
            bytes.truncate(2);
            bytes.push(0xe0);
            assert!(decode::<Q::Response>(&bytes, ()).is_ok());
        }
        check::<EiRangeGet>();
        check::<RecommendedEiGet>();
        check::<LensInformationGet>();
    }
}
