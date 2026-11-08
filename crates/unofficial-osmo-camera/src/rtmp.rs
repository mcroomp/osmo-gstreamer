//! Explicit RTMP controls over an already authorized LAN control owner.
//!
//! Hardware interoperability is unverified. These operations do not provision
//! an uplink, run a receiver, reconnect, or establish that video was delivered.

use crate::{
    ControlSession, Error, Result,
    protocol::{
        CommandPacket, ExplorationRequest, Flags,
        commands::media::{
            cmd_02_8e_streaming_control::{
                MediaStreamingControl, MediaStreamingControlRequest, MediaStreamingStateData,
                MediaStreamingStateGet,
            },
            cmd_02_e1_broadcast_mode::EnterBroadcastMode,
            cmd_08_78_streaming_settings::{MediaStreamingProfile, MediaStreamingSettings},
        },
        identity::Endpoint,
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

pub(crate) fn request<T: CommandPacket>(packet: &T, approved: bool) -> Result<ExplorationRequest> {
    if !approved {
        return Err(Error::InvalidConfig {
            field: "RTMP",
            reason: "RTMP changes require explicit approval for each operation",
        });
    }
    Ok(ExplorationRequest {
        receiver: Endpoint::Media,
        command: packet.command(),
        flags: Flags::REQUEST,
        payload: packet.payload()?,
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
        self.rtmp_command(request(&EnterBroadcastMode::default(), approved)?, false)
    }

    /// Sends settings without issuing a publication-start command.
    /// The caller is responsible for selecting broadcast mode; it is not queried.
    /// The caller owns and authorizes the destination and publishing key.
    pub fn configure_rtmp(&mut self, config: &RtmpConfig, approved: bool) -> Result<()> {
        self.rtmp_command(
            request(
                &MediaStreamingSettings {
                    profile: config.profile.clone(),
                },
                approved,
            )?,
            false,
        )
    }

    /// Requests publication start without verifying mode entry or configuration.
    /// The caller is responsible for those prerequisites.
    /// A success reply is not proof of receiver acceptance or delivered frames.
    pub fn start_rtmp(&mut self, approved: bool) -> Result<()> {
        self.rtmp_command(
            request(
                &MediaStreamingControlRequest {
                    control: MediaStreamingControl::Start,
                },
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
                &MediaStreamingControlRequest {
                    control: MediaStreamingControl::StopAndQuit,
                },
                approved,
            )?,
            true,
        )
    }

    /// Reads the raw structured state. Does not assign a publishing predicate.
    pub fn rtmp_state(&mut self) -> Result<MediaStreamingStateData> {
        Ok(self.execute_query::<MediaStreamingStateGet>()?.result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        fn check<T: crate::protocol::CommandPacket>(body: T, expected: &[u8]) {
            let command = body.command();
            assert!(request(&body, false).is_err());
            let request = request(&body, true).unwrap();
            assert_eq!(request.command, command);
            assert_eq!(request.receiver, Endpoint::Media);
            assert_eq!(request.flags, Flags::REQUEST);
            assert_eq!(request.timeout, Duration::from_secs(10));
            assert_eq!(request.payload, expected);
        }
        check(EnterBroadcastMode::default(), &[0x1a]);
        check(
            MediaStreamingControlRequest {
                control: MediaStreamingControl::Start,
            },
            &[1, 1, 0x1a, 0, 1, 1, 0, 0],
        );
        check(
            MediaStreamingControlRequest {
                control: MediaStreamingControl::StopAndQuit,
            },
            &[1, 1, 0x1a, 0, 1, 2, 0, 0],
        );
    }
}
