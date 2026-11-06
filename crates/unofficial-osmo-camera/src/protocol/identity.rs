//! Wire identities used by DUML requests and replies.

use binrw::{BinRead, BinWrite};
use std::io::Cursor;

#[derive(Clone, Copy, Debug, PartialEq, Eq, BinRead, BinWrite)]
pub enum Endpoint {
    #[brw(magic = 0x01u8)]
    Camera,
    #[brw(magic = 0x02u8)]
    App,
    #[brw(magic = 0x03u8)]
    Gimbal,
    #[brw(magic = 0x07u8)]
    Wifi,
    /// Media-service endpoint; distinct from camera endpoint 0x01.
    #[brw(magic = 0x08u8)]
    Media,
    #[brw(magic = 0x28u8)]
    Presence,
    #[brw(magic = 0x41u8)]
    LiveView,
    #[brw(magic = 0x48u8)]
    DeviceInfo,
    #[brw(magic = 0x1cu8)]
    CameraWake,
    #[brw(magic = 0xf0u8)]
    Session,
    Unknown(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, BinRead, BinWrite)]
pub enum CommandSet {
    #[brw(magic = 0x00u8)]
    Common,
    #[brw(magic = 0x02u8)]
    Camera,
    #[brw(magic = 0x03u8)]
    Gimbal,
    #[brw(magic = 0x07u8)]
    Wifi,
    #[brw(magic = 0x09u8)]
    LiveView,
    #[brw(magic = 0x53u8)]
    CameraWake,
    Unknown(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, BinRead, BinWrite)]
pub enum CommonCommand {
    #[brw(magic = 0x01u8)]
    ModuleVersion,
    #[brw(magic = 0x81u8)]
    DeviceInfo,
    #[brw(magic = 0x88u8)]
    AppPresence,
    #[brw(magic = 0x2bu8)]
    SessionKeepalive,
    #[brw(magic = 0x32u8)]
    ActivationStatus,
    Unknown(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, BinRead, BinWrite)]
pub enum CameraCommand {
    #[brw(magic = 0x18u8)]
    VideoFormatSet,
    #[brw(magic = 0x0cu8)]
    Playback,
    #[brw(magic = 0x80u8)]
    State,
    #[brw(magic = 0x09u8)]
    LiveViewSource,
    #[brw(magic = 0x1au8)]
    LiveViewQualitySet,
    #[brw(magic = 0x4cu8)]
    LiveViewOutputSet,
    #[brw(magic = 0x8eu8)]
    Parameters,
    #[brw(magic = 0x2bu8)]
    UserIso,
    #[brw(magic = 0x29u8)]
    UserShutter,
    #[brw(magic = 0x69u8)]
    AeLock,
    #[brw(magic = 0xa0u8)]
    Audio,
    #[brw(magic = 0xdau8)]
    StorageLocation,
    #[brw(magic = 0x5fu8)]
    Aeb,
    #[brw(magic = 0x57u8)]
    Language,
    #[brw(magic = 0x63u8)]
    VideoCaption,
    #[brw(magic = 0x4bu8)]
    PhotoInterval,
    #[brw(magic = 0xe5u8)]
    Watermark,
    #[brw(magic = 0xe0u8)]
    Capabilities,
    Unknown(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, BinRead, BinWrite)]
pub enum GimbalCommand {
    #[brw(magic = 0xdau8)]
    Initialize,
    Unknown(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, BinRead, BinWrite)]
pub enum WifiCommand {
    #[brw(magic = 0x45u8)]
    Pair,
    #[brw(magic = 0x19u8)]
    Country,
    #[brw(magic = 0x07u8)]
    Ssid,
    #[brw(magic = 0x0cu8)]
    Mac,
    #[brw(magic = 0x0eu8)]
    Password,
    #[brw(magic = 0x39u8)]
    GetRole,
    #[brw(magic = 0x46u8)]
    PairApproval,
    #[brw(magic = 0x47u8)]
    JoinNetwork,
    #[brw(magic = 0x48u8)]
    SetRole,
    Unknown(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, BinRead, BinWrite)]
pub enum LiveViewCommand {
    #[brw(magic = 0xa8u8)]
    StartTransmission,
    Unknown(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, BinRead, BinWrite)]
pub enum CameraWakeCommand {
    #[brw(magic = 0x10u8)]
    Wake,
    Unknown(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, BinRead, BinWrite)]
pub enum Command {
    #[brw(magic = 0x00u8)]
    Common(CommonCommand),
    #[brw(magic = 0x02u8)]
    Camera(CameraCommand),
    #[brw(magic = 0x03u8)]
    Gimbal(GimbalCommand),
    #[brw(magic = 0x07u8)]
    Wifi(WifiCommand),
    #[brw(magic = 0x09u8)]
    LiveView(LiveViewCommand),
    #[brw(magic = 0x53u8)]
    CameraWake(CameraWakeCommand),
    Unknown {
        set: u8,
        id: u8,
    },
}

impl Command {
    pub fn from_wire(set: u8, id: u8) -> Self {
        Self::read_le(&mut Cursor::new([set, id]))
            .expect("command fallback accepts every two-byte identity")
    }

    pub fn wire(self) -> (u8, u8) {
        let mut cursor = Cursor::new([0u8; 2]);
        self.write_le(&mut cursor)
            .expect("command identity encodes exactly two bytes");
        let [set, id] = cursor.into_inner();
        (set, id)
    }
}
