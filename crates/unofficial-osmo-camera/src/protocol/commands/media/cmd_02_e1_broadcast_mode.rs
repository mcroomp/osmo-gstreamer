//! Broadcast-mode entry at media endpoint 0x08, command 02:E1.

use crate::protocol::identity::{CameraCommand, Command};

pub const COMMAND: Command = Command::Camera(CameraCommand::Unknown(0xe1));

/// Requests broadcast mode, not publication.
///
/// Mode entry can change the Wi-Fi role and interrupt the control connection.
/// A matching success reply does not establish uplink connectivity.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, binrw::BinWrite)]
#[bw(magic = 0x1au8)]
pub struct EnterBroadcastMode;
