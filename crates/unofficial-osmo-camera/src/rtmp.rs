//! Explicit RTMP controls over an already authorized LAN control owner.
//!
//! Hardware interoperability is unverified. These operations do not provision
//! an uplink, run a receiver, reconnect, or establish that video was delivered.

use crate::protocol::encode;
use crate::{
    ControlSession, Error, Result,
    protocol::{
        ExplorationRequest, Flags,
        commands::media::{
            cmd_02_8e_streaming_control::{
                self, MediaStreamingControl, MediaStreamingStateGet, MediaStreamingStateReply,
            },
            cmd_02_e1_broadcast_mode::{self, EnterBroadcastMode},
            cmd_08_78_streaming_settings::{self, MediaStreamingProfile, MediaStreamingSettings},
        },
        identity::{Command, Endpoint},
    },
};
use std::time::Duration;

pub use crate::protocol::commands::media::cmd_08_78_streaming_settings::StreamingResolution;

/// Plain-address RTMP settings. Debug output omits the publishing address/key.
#[derive(Clone, Debug)]
pub struct RtmpConfig {
    profile: MediaStreamingProfile,
}

impl RtmpConfig {
    /// Builds default-FPS settings with unknown fields zero and adaptive bitrate
    /// off. The bitrate must be nonzero; no safe quality/rate range is promised.
    /// Destination length is measured in bytes, not characters.
    pub fn new(
        destination: impl Into<String>,
        resolution: StreamingResolution,
        bitrate_kbps: u16,
    ) -> Result<Self> {
        if bitrate_kbps == 0 {
            return Err(Error::InvalidConfig {
                field: "RTMP bitrate",
                reason: "must be nonzero; a supported operating range is not established",
            });
        }
        let profile = MediaStreamingProfile {
            resolution,
            bitrate_kbps,
            flags: 0,
            fps_selector: 0,
            unknown_6: 0,
            unknown_7: 0,
            orientation_flags: 0,
            destination: destination.into().into_bytes(),
        };
        profile.validate()?;
        Ok(Self { profile })
    }

    /// Requests the adaptive flag; this does not guarantee an adaptive policy.
    pub fn with_adaptive_bitrate(mut self, enabled: bool) -> Self {
        self.profile.flags = if enabled { 0x0100 } else { 0 };
        self
    }
}

pub(crate) fn request<T>(
    command: Command,
    payload: &T,
    approved: bool,
) -> Result<ExplorationRequest>
where
    for<'a> T: binrw::BinWrite<Args<'a> = ()>,
{
    if !approved {
        return Err(Error::InvalidConfig {
            field: "RTMP",
            reason: "RTMP changes require explicit approval for each operation",
        });
    }
    Ok(ExplorationRequest {
        receiver: Endpoint::Media,
        command,
        flags: Flags::REQUEST,
        payload: encode(&payload)?,
        timeout: Duration::from_secs(10),
    })
}

impl ControlSession {
    /// Requests broadcast mode. This is not a request to publish.
    ///
    /// Can change the Wi-Fi role and break this connection. The caller must
    /// arrange network recovery and a usable uplink before configuring/starting.
    /// Failure never retries mode entry or reconnects.
    pub fn enable_rtmp(&mut self, approved: bool) -> Result<()> {
        self.rtmp_command(
            request(
                cmd_02_e1_broadcast_mode::COMMAND,
                &EnterBroadcastMode,
                approved,
            )?,
            false,
        )
    }

    /// Applies settings in broadcast mode, without starting publication.
    /// The caller owns and authorizes the destination and publishing key.
    pub fn configure_rtmp(&mut self, config: &RtmpConfig, approved: bool) -> Result<()> {
        self.rtmp_command(
            request(
                cmd_08_78_streaming_settings::COMMAND,
                &MediaStreamingSettings {
                    profile: config.profile.clone(),
                },
                approved,
            )?,
            false,
        )
    }

    /// Requests start after successful mode entry and configuration.
    /// A success reply is not proof of receiver acceptance or delivered frames.
    pub fn start_rtmp(&mut self, approved: bool) -> Result<()> {
        self.rtmp_command(
            request(
                cmd_02_8e_streaming_control::COMMAND,
                &MediaStreamingControl::Start,
                approved,
            )?,
            false,
        )
    }

    /// Stops and quits broadcast mode, not merely pauses publication.
    ///
    /// Can affect recording and optional service cleanup. Does not promise to
    /// restore the previous Wi-Fi role. Call explicitly before closing when
    /// possible; close/drop does not stop RTMP and never reconnects to do so.
    pub fn stop_rtmp(&mut self, approved: bool) -> Result<()> {
        self.rtmp_command(
            request(
                cmd_02_8e_streaming_control::COMMAND,
                &MediaStreamingControl::StopAndQuit,
                approved,
            )?,
            true,
        )
    }

    /// Reads the raw structured state. Does not assign a publishing predicate.
    pub fn rtmp_state(&mut self) -> Result<MediaStreamingStateReply> {
        self.execute_query(MediaStreamingStateGet)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::encode;

    #[test]
    fn config_defaults_are_explicit_and_credentials_are_redacted() {
        let config = RtmpConfig::new(
            "rtmp://example.invalid/live/synthetic-key",
            StreamingResolution::FullHd1080,
            2000,
        )
        .unwrap()
        .with_adaptive_bitrate(true);
        assert_eq!(config.profile.flags, 0x0100);
        assert_eq!(config.profile.fps_selector, 0);
        assert_eq!(config.profile.orientation_flags, 0);
        let debug = format!("{config:?}");
        assert!(!debug.contains("synthetic-key"));
        assert!(!debug.contains("example.invalid"));
        assert!(
            RtmpConfig::new("rtmp://example.invalid/live", StreamingResolution::Hd720, 0).is_err()
        );
    }

    #[test]
    fn lifecycle_requests_are_approved_bounded_and_media_addressed() {
        let operations: [(Command, Vec<u8>); 3] = [
            (
                cmd_02_e1_broadcast_mode::COMMAND,
                encode(&EnterBroadcastMode).unwrap(),
            ),
            (
                cmd_02_8e_streaming_control::COMMAND,
                encode(&MediaStreamingControl::Start).unwrap(),
            ),
            (
                cmd_02_8e_streaming_control::COMMAND,
                encode(&MediaStreamingControl::StopAndQuit).unwrap(),
            ),
        ];
        for (command, bytes) in operations {
            #[derive(binrw::BinWrite)]
            struct Payload(Vec<u8>);
            let payload = Payload(bytes.clone());
            assert!(request(command, &payload, false).is_err());
            let request = request(command, &payload, true).unwrap();
            assert_eq!(request.receiver, Endpoint::Media);
            assert_eq!(request.flags, Flags::REQUEST);
            assert_eq!(request.timeout, Duration::from_secs(10));
            assert_eq!(request.payload, bytes);
        }
    }
}
