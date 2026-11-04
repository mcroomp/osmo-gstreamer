//! Wi-Fi role query and SET role values.

use crate::{
    Error, Result,
    protocol::{
        Query, WireEncode, WireWriter,
        commands::reply::{CameraRawData, CameraReadReply},
        identity::{Command, Endpoint, WifiCommand},
    },
};
use num_enum::{IntoPrimitive, TryFromPrimitive};

const ROLE_GET_OPERATION: u8 = 0;

/// Request the Wi-Fi role; the accepted response layout is unspecified.
///
/// APP -> Wifi | 0x07/0x39 | Role GET
///
/// ```text
/// request { operation: u8 = 0x00 }
/// reply { status: u8 | reject if nonzero
///         data: [u8; remaining] | opaque, not decoded as WifiRole }
/// ```
///
/// The accepted-body layout and model support remain unspecified. In
/// particular, do not interpret an arbitrary returned byte as `WifiRole`.
#[derive(Clone, Copy, Debug)]
pub struct WifiRoleGet;

impl WireEncode for WifiRoleGet {
    fn write_to(&self, writer: &mut WireWriter) -> Result<()> {
        writer.write_u8(ROLE_GET_OPERATION);
        Ok(())
    }
}

impl Query for WifiRoleGet {
    type Response = CameraReadReply<CameraRawData>;
    fn receiver(&self) -> Endpoint {
        Endpoint::Wifi
    }
    fn command(&self) -> Command {
        Command::Wifi(WifiCommand::GetRole)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
#[num_enum(error_type(name = Error, constructor = WifiRole::invalid_wire))]
#[repr(u8)]
pub enum WifiRole {
    Hotspot = 0,
    Station = 1,
}

impl WifiRole {
    fn invalid_wire(value: u8) -> Error {
        Error::UnsupportedValue {
            field: "Wi-Fi role",
            value,
        }
    }
}
