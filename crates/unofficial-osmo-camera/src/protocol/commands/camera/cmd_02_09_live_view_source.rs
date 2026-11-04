//! Camera live-view enable/disable payloads.

use crate::{
    Result,
    protocol::{WireEncode, WireWriter},
};
use num_enum::{IntoPrimitive, TryFromPrimitive};

const SOURCE_PREFIX_LENGTH: usize = 10;
/// Live-view gate, not a camera-source or resolution identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, IntoPrimitive, TryFromPrimitive)]
#[repr(u8)]
pub enum LiveViewGate {
    Enable = 0x03,
    Disable = 0x04,
}

/// Encode the eleven-byte live-view gate variant.
///
/// APP -> Camera | 0x02/0x09
///
/// The first eight bytes are zeroed action arguments, byte 8 is a zero
/// option, byte 9 selects live-view index 0, and byte 10 selects the action.
/// Enable/disable request subscription on/off; enable may be deferred.
/// A zero-status reply is not proof of frame delivery.
/// Support and interaction with teardown remain model-dependent.
/// Encoding does not authorize transmission or implement teardown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LiveViewToggleRequest {
    pub gate: LiveViewGate,
}

impl WireEncode for LiveViewToggleRequest {
    fn write_to(&self, writer: &mut WireWriter) -> Result<()> {
        writer.write_bytes(&[0; SOURCE_PREFIX_LENGTH]);
        writer.write_u8(self.gate.into());
        Ok(())
    }
}

/// Compatibility request for enabling live view.
///
/// APP -> Camera | 0x02/0x09
///
/// ```text
/// request { action_arguments: [0x00; 8], option: u8 = 0,
///           liveview_index: u8 = 0, action: u8 = 0x03 }
/// reply { status: u8, extension: [u8; remaining] | opaque }
/// ```
#[derive(Clone, Copy, Debug)]
pub struct LiveViewSourceRequest;

impl WireEncode for LiveViewSourceRequest {
    fn write_to(&self, writer: &mut WireWriter) -> Result<()> {
        LiveViewToggleRequest {
            gate: LiveViewGate::Enable,
        }
        .write_to(writer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subscription_index_and_actions_preserve_existing_wire_requests() {
        for (gate, action) in [(LiveViewGate::Enable, 3), (LiveViewGate::Disable, 4)] {
            let bytes = LiveViewToggleRequest { gate }.encode().unwrap();
            assert_eq!(&bytes[..10], &[0; 10]);
            assert_eq!(bytes.len(), 11);
            assert_eq!(bytes[10], action);
        }
        assert_eq!(
            LiveViewSourceRequest.encode().unwrap(),
            [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3]
        );
    }
}
