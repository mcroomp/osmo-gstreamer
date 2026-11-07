//! Camera live-view stream-control payloads.

use binrw::{BinRead, BinWrite};

/// Request start or stop of the camera's sender-dependent live-view stream.
///
/// APP -> Camera | 0x02/0x09
///
/// The first ten bytes are zero and byte 10 selects the action. Prefix meanings
/// and device-side completion behavior remain unspecified.
/// A zero-status reply does not confirm an active encoder, delivered frames
/// or completion of every stream operation.
/// Stop is not a complete teardown or rollback guarantee.
/// Encoding does not authorize transmission.
///
/// ```
/// use unofficial_osmo_camera::protocol::commands::camera::cmd_02_09_live_view_control::{
///     LiveViewControlAction, LiveViewControlRequest,
/// };
///
/// assert_eq!(LiveViewControlRequest::start().action, LiveViewControlAction::Start);
/// assert_eq!(LiveViewControlRequest::stop().action, LiveViewControlAction::Stop);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, BinWrite)]
#[bw(magic = b"\x02\x09")]
pub struct LiveViewControlRequest {
    #[bw(pad_before = CONTROL_PREFIX_LENGTH)]
    pub action: LiveViewControlAction,
}

impl LiveViewControlRequest {
    pub const fn new(action: LiveViewControlAction) -> Self {
        Self { action }
    }

    pub const fn start() -> Self {
        Self::new(LiveViewControlAction::Start)
    }

    pub const fn stop() -> Self {
        Self::new(LiveViewControlAction::Stop)
    }
}

const CONTROL_PREFIX_LENGTH: usize = 10;

/// Stream-control actions, not source indices or encoder configuration values.
#[derive(Clone, Copy, Debug, PartialEq, Eq, BinWrite, BinRead)]
#[brw(repr = u8)]
#[repr(u8)]
pub enum LiveViewControlAction {
    Start = 0x03,
    Stop = 0x04,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::CommandPacket;

    #[test]
    fn constructors_and_manual_actions_preserve_exact_stream_control_payloads() {
        for (request, action, code) in [
            (
                LiveViewControlRequest::start(),
                LiveViewControlAction::Start,
                3,
            ),
            (
                LiveViewControlRequest::stop(),
                LiveViewControlAction::Stop,
                4,
            ),
        ] {
            assert_eq!(request, LiveViewControlRequest::new(action));
            let bytes = CommandPacket::payload(&request).unwrap();
            assert_eq!(
                bytes,
                CommandPacket::payload(&LiveViewControlRequest { action }).unwrap()
            );
            assert_eq!(&bytes[..10], &[0; 10]);
            assert_eq!(bytes.len(), 11);
            assert_eq!(bytes[10], code);
            assert_eq!(LiveViewControlRequest::COMMAND_ID, [2, 9]);
        }
        assert_eq!(
            CommandPacket::payload(&LiveViewControlRequest::start()).unwrap(),
            [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3]
        );
    }
}
