//! Broadcast-mode entry at media endpoint 0x08, command 02:E1.

use crate::protocol::identity::{CameraCommand, Command};
use binrw::binwrite;

/// Requests broadcast mode, not publication.
///
/// Mode entry can change the Wi-Fi role and interrupt the control connection.
/// A matching success reply does not establish uplink connectivity.
#[binwrite]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[bw(magic = b"\x02\xe1")]
pub struct EnterBroadcastMode {
    #[bw(calc = [26u8])]
    payload: [u8; 1],
}

pub const COMMAND: Command = Command::Camera(CameraCommand::Unknown(0xe1));
