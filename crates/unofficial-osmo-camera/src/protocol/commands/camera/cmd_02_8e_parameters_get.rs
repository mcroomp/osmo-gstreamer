//! Camera parameter GET request and TLV response codecs.

use crate::{
    Error, Result,
    protocol::{
        Query, WireDecode, WireEncode, WireReader, WireWriter,
        commands::reply::CameraReadReply,
        identity::{CameraCommand, Command, Endpoint},
    },
};
use num_enum::{FromPrimitive, IntoPrimitive};

const PARAMETER_GET_OPERATION: u8 = 0;
const ADVANCED_LIVE_REVERSAL_IDS: [CameraParameter; 2] = [
    CameraParameter::Unknown(0x0000),
    CameraParameter::Unknown(0x0001),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, FromPrimitive, IntoPrimitive)]
#[repr(u16)]
pub enum CameraParameter {
    Beauty = 0x0002,
    FpvControl = 0x0003,
    PreviousMode = 0x0007,
    Eis = 0x0008,
    FieldOfView = 0x0009,
    IsoLimit = 0x000f,
    Unknown0014 = 0x0014,
    QuickModeSwitch = 0x000b,
    VoiceControl = 0x000a,
    VoiceLanguage = 0x000e,
    Led = 0x0010,
    ScreenSleep = 0x0011,
    AutoOff = 0x0012,
    SpotMetering = 0x0015,
    CaptureMinimumInterval = 0x0017,
    LoopDuration = 0x0018,
    P2PShare = 0x0021,
    SelfieFollowing = 0x0024,
    VideoBeautify = 0x0025,
    #[num_enum(catch_all)]
    Unknown(u16),
}

/// External field-of-view labels for one-byte parameter `0x0009`.
///
/// These names are source-derived candidates, not a guarantee of camera
/// support or a mapping for the separate `0x02/0x18` format field.
#[derive(Clone, Copy, Debug, PartialEq, Eq, FromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum CameraFieldOfView {
    UltraWide = 0,
    Wide = 1,
    Standard = 2,
    NaturalWide = 5,
    #[num_enum(catch_all)]
    Unknown(u8),
}

/// External stabilization labels for one-byte parameter `0x0008`.
///
/// Names do not establish device support or observed stabilization behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq, FromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum CameraStabilizationMode {
    Off = 0,
    RockSteady = 1,
    HorizonSteady = 2,
    RockSteadyPlus = 3,
    HorizonBalancing = 4,
    #[num_enum(catch_all)]
    Unknown(u8),
}

impl WireEncode for CameraParameter {
    fn write_to(&self, writer: &mut WireWriter) -> Result<()> {
        writer.write_u16_le((*self).into());
        Ok(())
    }
}

impl WireDecode for CameraParameter {
    fn read_from(reader: &mut WireReader<'_>, _context: &()) -> Result<Self> {
        Ok(Self::from(reader.read_u16_le()?))
    }
}

/// One parameter record returned by a camera parameter GET.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CameraParameterValue {
    pub parameter: CameraParameter,
    pub bytes: Vec<u8>,
}

impl CameraParameterValue {
    /// Interpret a one-byte field-of-view parameter; unknown codes are retained.
    pub fn field_of_view(&self) -> Option<CameraFieldOfView> {
        match (self.parameter, self.bytes.as_slice()) {
            (CameraParameter::FieldOfView, [value]) => Some(CameraFieldOfView::from(*value)),
            _ => None,
        }
    }

    /// Interpret a one-byte stabilization parameter; unknown codes are retained.
    pub fn stabilization_mode(&self) -> Option<CameraStabilizationMode> {
        match (self.parameter, self.bytes.as_slice()) {
            (CameraParameter::Eis, [value]) => Some(CameraStabilizationMode::from(*value)),
            _ => None,
        }
    }

    pub fn enabled(&self) -> Option<bool> {
        match (self.parameter, self.bytes.as_slice()) {
            (CameraParameter::VoiceControl | CameraParameter::QuickModeSwitch, [value]) => {
                Some(*value != 0)
            }
            _ => None,
        }
    }
}

/// Accepted TLV records, excluding the outer status byte.
///
/// Camera -> APP | 0x02/0x8E | Parameter GET reply
///
/// ```text
/// reply { status: u8 | handled by CameraReadReply
///         operation: u8 = 0x00, count: u8,
///         records: [{ id: le_u16, length: u8, value: [u8; length] }; count] }
/// ```
///
/// Operation, count, echoed IDs and order must match the request exactly.
/// Trailing records/bytes are rejected. Value widths and unknown bytes are retained.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CameraParameterValues(pub Vec<CameraParameterValue>);

impl WireEncode for CameraParameterValues {
    fn write_to(&self, writer: &mut WireWriter) -> Result<()> {
        let count =
            u8::try_from(self.0.len()).map_err(|_| Error::LimitExceeded("too many camera TLVs"))?;
        writer.write_u8(PARAMETER_GET_OPERATION);
        writer.write_u8(count);
        for value in &self.0 {
            let length = u8::try_from(value.bytes.len())
                .map_err(|_| Error::LimitExceeded("camera TLV exceeds 255 bytes"))?;
            value.parameter.write_to(writer)?;
            writer.write_u8(length);
            writer.write_bytes(&value.bytes);
        }
        Ok(())
    }
}

/// Read a known camera parameter by its 16-bit identifier.
///
/// APP -> Camera | 0x02/0x8E
///
/// ```text
/// request { operation: u8 = 0, count: u8, ids: [le_u16; count] }
/// reply {
///     status: u8
///     operation: u8 = 0
///     count: u8
///     records: [{ id: le_u16, length: u8, value: [u8; length] }]
/// }
/// ```
#[derive(Clone, Debug)]
pub struct CameraParametersGet {
    parameters: Vec<CameraParameter>,
}

impl CameraParametersGet {
    pub fn single(parameter: CameraParameter) -> Result<Self> {
        if matches!(parameter, CameraParameter::Unknown(_)) {
            return Err(Error::InvalidConfig {
                field: "camera parameter GET",
                reason: "unknown parameters require approved raw exploration",
            });
        }
        Ok(Self {
            parameters: vec![parameter],
        })
    }

    pub fn advanced_live_reversal() -> Self {
        Self {
            parameters: ADVANCED_LIVE_REVERSAL_IDS.to_vec(),
        }
    }

    pub fn quick_mode_switch() -> Self {
        Self {
            parameters: vec![CameraParameter::QuickModeSwitch],
        }
    }
}

impl WireEncode for CameraParametersGet {
    fn write_to(&self, writer: &mut WireWriter) -> Result<()> {
        let count = u8::try_from(self.parameters.len())
            .map_err(|_| Error::LimitExceeded("too many camera parameters"))?;
        writer.write_u8(PARAMETER_GET_OPERATION);
        writer.write_u8(count);
        for parameter in &self.parameters {
            parameter.write_to(writer)?;
        }
        Ok(())
    }
}

impl Query for CameraParametersGet {
    type Response = CameraReadReply<CameraParameterValues>;

    fn receiver(&self) -> Endpoint {
        Endpoint::Camera
    }

    fn command(&self) -> Command {
        Command::Camera(CameraCommand::Parameters)
    }
}

impl WireDecode<CameraParametersGet> for CameraParameterValues {
    fn read_from(reader: &mut WireReader<'_>, query: &CameraParametersGet) -> Result<Self> {
        if reader.read_u8()? != PARAMETER_GET_OPERATION
            || usize::from(reader.read_u8()?) != query.parameters.len()
        {
            return Err(Error::Protocol("camera GET operation/count mismatch"));
        }
        let mut values = Vec::new();
        for expected in &query.parameters {
            let parameter = CameraParameter::read_from(reader, &())?;
            if parameter != *expected {
                return Err(Error::Protocol("camera GET parameter/order mismatch"));
            }
            let length = usize::from(reader.read_u8()?);
            let bytes = reader.read_bytes(length)?.to_vec();
            values.push(CameraParameterValue { parameter, bytes });
        }
        if !reader.remaining().is_empty() {
            return Err(Error::Protocol("trailing camera GET TLVs"));
        }
        Ok(Self(values))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::WireDecode;

    #[test]
    fn parameter_requests_encode_ids_in_order() {
        let one = CameraParametersGet::quick_mode_switch();
        assert_eq!(one.encode().unwrap(), [0, 1, 0x0b, 0]);
        assert_eq!(one.receiver(), Endpoint::Camera);
        assert_eq!(one.command().wire(), (0x02, 0x8e));

        let two = CameraParametersGet::advanced_live_reversal();
        assert_eq!(two.encode().unwrap(), [0, 2, 0, 0, 1, 0]);
    }

    #[test]
    fn unknown_single_parameters_and_malformed_replies_are_rejected() {
        assert!(CameraParametersGet::single(CameraParameter::Unknown(0x1234)).is_err());
        let query = CameraParametersGet::quick_mode_switch();
        for bytes in [
            &[0, 0, 0, 0x0b, 0, 1, 1][..],
            &[0, 0, 2, 0x0b, 0, 1, 1][..],
            &[0, 0, 1, 0x0b, 0, 2, 1][..],
            &[0, 0, 1, 0x0b, 0, 1, 1, 0xff][..],
        ] {
            assert!(CameraReadReply::<CameraParameterValues>::decode(bytes, &query).is_err());
        }
    }

    #[test]
    fn parameter_reply_preserves_rejection_and_exposes_one_byte_flags() {
        let query = CameraParametersGet::quick_mode_switch();
        let reply =
            CameraReadReply::<CameraParameterValues>::decode(&[0, 0, 1, 0x0b, 0, 1, 1], &query)
                .unwrap();
        let CameraReadReply::Accepted(values) = reply else {
            panic!("expected accepted parameter reply");
        };
        assert_eq!(values.0[0].enabled(), Some(true));
        assert_eq!(values.encode().unwrap(), [0, 1, 0x0b, 0, 1, 1]);

        let rejected =
            CameraReadReply::<CameraParameterValues>::decode(b"\xe3opaque", &query).unwrap();
        assert_eq!(rejected.encode().unwrap(), b"\xe3opaque");
    }

    #[test]
    fn fov_and_stabilization_labels_require_their_exact_one_byte_tlv() {
        let value = |parameter, bytes: &[u8]| CameraParameterValue {
            parameter,
            bytes: bytes.to_vec(),
        };
        assert_eq!(
            value(CameraParameter::FieldOfView, &[0x05]).field_of_view(),
            Some(CameraFieldOfView::NaturalWide)
        );
        assert_eq!(
            value(CameraParameter::FieldOfView, &[0xFE]).field_of_view(),
            Some(CameraFieldOfView::Unknown(0xFE))
        );
        assert_eq!(
            value(CameraParameter::Eis, &[0x03]).stabilization_mode(),
            Some(CameraStabilizationMode::RockSteadyPlus)
        );
        assert_eq!(
            value(CameraParameter::Eis, &[0xFE]).stabilization_mode(),
            Some(CameraStabilizationMode::Unknown(0xFE))
        );
        assert_eq!(value(CameraParameter::Eis, &[0]).field_of_view(), None);
        assert_eq!(
            value(CameraParameter::FieldOfView, &[0, 1]).field_of_view(),
            None
        );
    }
}
