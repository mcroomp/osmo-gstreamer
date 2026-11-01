//! Marker-qualified activation indication.

use crate::{
    Error, Result,
    protocol::{WireDecode, WireEncode, WireReader, WireWriter, commands::reply::CameraRawData},
};
use num_enum::{FromPrimitive, IntoPrimitive};

#[derive(Clone, Copy, Debug, PartialEq, Eq, FromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum ActivationState {
    Unknown00 = 0,
    Unknown02 = 2,
    #[num_enum(catch_all)]
    Other(u8),
}

pub const ACTIVATION_MARKER: u8 = 0x33;
pub const ACTIVATION_MARKER_LENGTH: usize = 2;
pub const ACTIVATION_STATE_OFFSET: usize = 20;
pub const ACTIVATION_UNKNOWN_LENGTH: usize = ACTIVATION_STATE_OFFSET - ACTIVATION_MARKER_LENGTH;

/// Decode the marker-qualified activation state without interpreting unknown bytes.
///
/// Device (sender endpoint unspecified) -> APP | 0x00/0x32 | Activation indication
///
/// ```text
/// notification {
///     marker: [0x33; 2]
///     unknown: [u8; 18]
///     state: u8
///     extension: [u8; remaining] | opaque
/// }
/// ```
///
/// Only this marker-qualified body is defined; unknown states are preserved.
/// The BLE receiver accepts this body by command and marker without imposing
/// additional sender/flags restrictions. Other bodies remain opaque. No
/// outbound activation query is defined.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActivationStatusNotification {
    pub unknown: [u8; ACTIVATION_UNKNOWN_LENGTH],
    pub state: ActivationState,
    pub extension: CameraRawData,
}

impl WireDecode for ActivationStatusNotification {
    fn read_from(reader: &mut WireReader<'_>, context: &()) -> Result<Self> {
        if reader.read_bytes(ACTIVATION_MARKER_LENGTH)?
            != [ACTIVATION_MARKER; ACTIVATION_MARKER_LENGTH]
        {
            return Err(Error::Protocol("unrecognized activation marker"));
        }
        let mut unknown = [0; ACTIVATION_UNKNOWN_LENGTH];
        unknown.copy_from_slice(reader.read_bytes(ACTIVATION_UNKNOWN_LENGTH)?);
        Ok(Self {
            unknown,
            state: ActivationState::read_from(reader, context)?,
            extension: CameraRawData::read_from(reader, context)?,
        })
    }
}

impl WireEncode for ActivationStatusNotification {
    fn write_to(&self, writer: &mut WireWriter) -> Result<()> {
        writer.write_bytes(&[ACTIVATION_MARKER; ACTIVATION_MARKER_LENGTH]);
        writer.write_bytes(&self.unknown);
        self.state.write_to(writer)?;
        self.extension.write_to(writer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_state_and_opaque_bytes_round_trip() {
        let value = ActivationStatusNotification {
            unknown: [0xa5; ACTIVATION_UNKNOWN_LENGTH],
            state: ActivationState::Other(0xff),
            extension: CameraRawData(vec![0xde, 0xad]),
        };
        let mut bytes = value.encode().unwrap();
        assert_eq!(
            ActivationStatusNotification::decode(&bytes, &()).unwrap(),
            value
        );
        for length in 0..=ACTIVATION_STATE_OFFSET {
            assert!(ActivationStatusNotification::decode(&bytes[..length], &()).is_err());
        }
        bytes[0] = 0;
        assert!(ActivationStatusNotification::decode(&bytes, &()).is_err());
    }
}
