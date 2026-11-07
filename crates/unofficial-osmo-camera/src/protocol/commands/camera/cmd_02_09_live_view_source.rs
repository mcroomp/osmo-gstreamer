//! Camera live-view enable/disable payloads.

use binrw::{BinRead, BinWrite, binwrite};

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
#[derive(Clone, Copy, Debug, PartialEq, Eq, BinWrite)]
#[bw(magic = b"\x02\x09")]
pub struct LiveViewToggleRequest {
    #[bw(pad_before = SOURCE_PREFIX_LENGTH)]
    pub gate: LiveViewGate,
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
#[binwrite]
#[derive(Clone, Copy, Debug, Default)]
#[bw(magic = b"\x02\x09")]
pub struct LiveViewSourceRequest {
    #[bw(calc = [0u8; SOURCE_PREFIX_LENGTH])]
    prefix: [u8; SOURCE_PREFIX_LENGTH],
    #[bw(calc = LiveViewGate::Enable)]
    gate: LiveViewGate,
}

const SOURCE_PREFIX_LENGTH: usize = 10;
/// Live-view gate, not a camera-source or resolution identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, BinWrite, BinRead)]
#[brw(repr = u8)]
#[repr(u8)]
pub enum LiveViewGate {
    Enable = 0x03,
    Disable = 0x04,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::CommandPacket;

    #[test]
    fn subscription_index_and_actions_preserve_existing_wire_requests() {
        for (gate, action) in [(LiveViewGate::Enable, 3), (LiveViewGate::Disable, 4)] {
            let bytes = CommandPacket::payload(&LiveViewToggleRequest { gate }).unwrap();
            assert_eq!(&bytes[..10], &[0; 10]);
            assert_eq!(bytes.len(), 11);
            assert_eq!(bytes[10], action);
        }
        assert_eq!(
            CommandPacket::payload(&LiveViewSourceRequest::default()).unwrap(),
            [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3]
        );
    }
}
