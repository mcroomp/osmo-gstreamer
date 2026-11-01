//! Wi-Fi country-data GET.

use crate::{
    Result,
    protocol::{
        Query, WireEncode, WireWriter,
        commands::reply::{CameraRawData, CameraReadReply},
        identity::{Command, Endpoint, WifiCommand},
    },
};

/// Read country data without inferring its unresolved fields.
///
/// APP -> Wifi | 0x07/0x19 | Country GET
///
/// ```text
/// request {}
/// reply { status: u8 | reject if nonzero
///         data: [u8; remaining] | opaque }
/// ```
#[derive(Clone, Copy, Debug)]
pub struct WifiCountryGet;

impl WireEncode for WifiCountryGet {
    fn write_to(&self, _writer: &mut WireWriter) -> Result<()> {
        Ok(())
    }
}

impl Query for WifiCountryGet {
    type Response = CameraReadReply<CameraRawData>;
    fn receiver(&self) -> Endpoint {
        Endpoint::Wifi
    }
    fn command(&self) -> Command {
        Command::Wifi(WifiCommand::Country)
    }
}
