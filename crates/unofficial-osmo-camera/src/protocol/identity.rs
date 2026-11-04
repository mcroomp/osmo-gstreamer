//! Wire identities used by DUML requests and replies.

use super::{WireDecode, WireEncode, WireReader, WireU8, WireWriter};
use crate::Result;
use num_enum::{FromPrimitive, IntoPrimitive};

#[derive(Clone, Copy, Debug, PartialEq, Eq, FromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum Endpoint {
    Camera = 0x01,
    App = 0x02,
    Gimbal = 0x03,
    Wifi = 0x07,
    Presence = 0x28,
    LiveView = 0x41,
    DeviceInfo = 0x48,
    CameraWake = 0x1c,
    Session = 0xf0,
    #[num_enum(catch_all)]
    Unknown(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, FromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum CommandSet {
    Common = 0x00,
    Camera = 0x02,
    Gimbal = 0x03,
    Wifi = 0x07,
    LiveView = 0x09,
    CameraWake = 0x53,
    #[num_enum(catch_all)]
    Unknown(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, FromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum CommonCommand {
    ModuleVersion = 0x01,
    DeviceInfo = 0x81,
    AppPresence = 0x88,
    SessionKeepalive = 0x2b,
    ActivationStatus = 0x32,
    #[num_enum(catch_all)]
    Unknown(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, FromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum CameraCommand {
    VideoFormatSet = 0x18,
    Playback = 0x0c,
    State = 0x80,
    LiveViewSource = 0x09,
    LiveViewQualitySet = 0x1a,
    LiveViewOutputSet = 0x4c,
    Parameters = 0x8e,
    UserIso = 0x2b,
    UserShutter = 0x29,
    AeLock = 0x69,
    Audio = 0xa0,
    StorageLocation = 0xda,
    Aeb = 0x5f,
    Language = 0x57,
    VideoCaption = 0x63,
    PhotoInterval = 0x4b,
    Watermark = 0xe5,
    Capabilities = 0xe0,
    #[num_enum(catch_all)]
    Unknown(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, FromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum GimbalCommand {
    Initialize = 0xda,
    #[num_enum(catch_all)]
    Unknown(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, FromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum WifiCommand {
    Pair = 0x45,
    Country = 0x19,
    Ssid = 0x07,
    Mac = 0x0c,
    Password = 0x0e,
    GetRole = 0x39,
    PairApproval = 0x46,
    JoinNetwork = 0x47,
    SetRole = 0x48,
    #[num_enum(catch_all)]
    Unknown(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, FromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum LiveViewCommand {
    StartTransmission = 0xa8,
    #[num_enum(catch_all)]
    Unknown(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, FromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum CameraWakeCommand {
    Wake = 0x10,
    #[num_enum(catch_all)]
    Unknown(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Common(CommonCommand),
    Camera(CameraCommand),
    Gimbal(GimbalCommand),
    Wifi(WifiCommand),
    LiveView(LiveViewCommand),
    CameraWake(CameraWakeCommand),
    Unknown { set: u8, id: u8 },
}

impl Command {
    pub fn from_wire(set: u8, id: u8) -> Self {
        match CommandSet::from(set) {
            CommandSet::Common => Self::Common(CommonCommand::from(id)),
            CommandSet::Camera => Self::Camera(CameraCommand::from(id)),
            CommandSet::Gimbal => Self::Gimbal(GimbalCommand::from(id)),
            CommandSet::Wifi => Self::Wifi(WifiCommand::from(id)),
            CommandSet::LiveView => Self::LiveView(LiveViewCommand::from(id)),
            CommandSet::CameraWake => Self::CameraWake(CameraWakeCommand::from(id)),
            CommandSet::Unknown(set) => Self::Unknown { set, id },
        }
    }

    pub fn wire(self) -> (u8, u8) {
        let (set, id) = match self {
            Self::Common(command) => (CommandSet::Common, command.to_wire_u8()),
            Self::Camera(command) => (CommandSet::Camera, command.to_wire_u8()),
            Self::Gimbal(command) => (CommandSet::Gimbal, command.to_wire_u8()),
            Self::Wifi(command) => (CommandSet::Wifi, command.to_wire_u8()),
            Self::LiveView(command) => (CommandSet::LiveView, command.to_wire_u8()),
            Self::CameraWake(command) => (CommandSet::CameraWake, command.to_wire_u8()),
            Self::Unknown { set, id } => (CommandSet::Unknown(set), id),
        };
        (set.to_wire_u8(), id)
    }
}

impl WireEncode for Command {
    fn write_to(&self, writer: &mut WireWriter) -> Result<()> {
        let (set, id) = self.wire();
        writer.write_u8(set);
        writer.write_u8(id);
        Ok(())
    }
}

impl WireDecode for Command {
    fn read_from(reader: &mut WireReader<'_>, _context: &()) -> Result<Self> {
        Ok(Self::from_wire(reader.read_u8()?, reader.read_u8()?))
    }
}
