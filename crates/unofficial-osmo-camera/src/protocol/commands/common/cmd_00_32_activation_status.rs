//! Marker-qualified activation indication.

use crate::protocol::commands::reply::CameraRawData;
use binrw::{BinRead, BinWrite};

#[derive(Clone, Copy, Debug, PartialEq, Eq, BinRead, BinWrite)]
pub enum ActivationState {
    #[brw(magic = 0u8)]
    Unknown00,
    #[brw(magic = 2u8)]
    Unknown02,
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
#[derive(Clone, Debug, PartialEq, Eq, BinRead, BinWrite)]
#[brw(magic = b"\x33\x33")]
pub struct ActivationStatusNotification {
    pub unknown: [u8; ACTIVATION_UNKNOWN_LENGTH],
    pub state: ActivationState,
    pub extension: CameraRawData,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{decode, encode};

    #[test]
    fn marker_state_and_opaque_bytes_round_trip() {
        let value = ActivationStatusNotification {
            unknown: [0xa5; ACTIVATION_UNKNOWN_LENGTH],
            state: ActivationState::Other(0xff),
            extension: CameraRawData(vec![0xde, 0xad]),
        };
        let mut bytes = encode(&value).unwrap();
        assert_eq!(
            decode::<ActivationStatusNotification>(&bytes, ()).unwrap(),
            value
        );
        for length in 0..=ACTIVATION_STATE_OFFSET {
            assert!(decode::<ActivationStatusNotification>(&bytes[..length], ()).is_err());
        }
        bytes[0] = 0;
        assert!(decode::<ActivationStatusNotification>(&bytes, ()).is_err());
    }
}
