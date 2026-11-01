//! Low-level requests for bounded protocol operations.
//! Exact known GET tuples need no raw permission; other requests require explicit approval.
//! Receiver-08 RTMP operations are unsupported.

use super::*;
use crate::{
    Error, Result,
    protocol::{
        commands::{
            camera::{
                cmd_02_8e_parameters_get::CameraParameter,
                cmd_02_e0_capabilities_get::CapabilityQuery,
            },
            wifi::cmd_07_39_get_role::WifiRoleGet,
        },
        identity::{CameraCommand, Command, CommonCommand, Endpoint, WifiCommand},
    },
};
use std::time::Duration;

pub const EXPLORATION_MIN_TIMEOUT: Duration = Duration::from_millis(100);
pub const EXPLORATION_MAX_TIMEOUT: Duration = Duration::from_secs(30);
const RTMP_RECEIVER: u8 = 0x08;
const RTMP_SET: u8 = 0x08;
const RTMP_COMMAND_078: u8 = 0x78;
const RTMP_COMMAND_079: u8 = 0x79;

#[derive(Clone)]
pub struct ExplorationRequest {
    pub receiver: Endpoint,
    pub command: Command,
    pub flags: Flags,
    pub payload: Vec<u8>,
    pub timeout: Duration,
}

impl ExplorationRequest {
    pub fn message(&self, id: u16) -> Message {
        Message {
            flags: self.flags,
            ..Message::request(self.receiver, id, self.command, &self.payload)
        }
    }

    pub fn validate(&self, session_approved: bool, request_approved: bool) -> Result<()> {
        if !(EXPLORATION_MIN_TIMEOUT..=EXPLORATION_MAX_TIMEOUT).contains(&self.timeout) {
            return Err(Error::InvalidConfig {
                field: "timeout",
                reason: "must be 0.1..30 seconds",
            });
        }
        self.message(MIN_ID).encode()?;
        let wire = self.command.wire();
        if self.receiver.to_wire_u8() == RTMP_RECEIVER
            && (self.command == Command::Camera(CameraCommand::Parameters)
                || wire == (RTMP_SET, RTMP_COMMAND_078)
                || wire == (RTMP_SET, RTMP_COMMAND_079))
        {
            return Err(Error::Unsupported(
                "receiver-08 RTMP operations are unsupported",
            ));
        }
        if !self.is_known_read() && !(session_approved && request_approved) {
            return Err(Error::InvalidConfig {
                field: "unclassified request",
                reason: "requires allow_unclassified on the session and approve_unclassified on this request",
            });
        }
        Ok(())
    }

    pub fn is_known_read(&self) -> bool {
        if self.flags != Flags::REQUEST {
            return false;
        }
        match (self.receiver, self.command) {
            (
                Endpoint::Wifi,
                Command::Wifi(WifiCommand::Country | WifiCommand::Ssid | WifiCommand::Mac),
            ) => self.payload.is_empty(),
            (Endpoint::Wifi, Command::Wifi(WifiCommand::GetRole)) => WifiRoleGet
                .encode()
                .is_ok_and(|payload| self.payload == payload),
            (Endpoint::Camera, Command::Camera(CameraCommand::Capabilities)) => [
                CapabilityQuery::Accessories,
                CapabilityQuery::EiRange,
                CapabilityQuery::RecommendedEi,
                CapabilityQuery::LensInformation,
            ]
            .iter()
            .any(|query| query.encode().is_ok_and(|payload| self.payload == payload)),
            (Endpoint::Camera, command) => known_camera_read(command, &self.payload),
            _ => false,
        }
    }
}

fn known_camera_read(command: Command, payload: &[u8]) -> bool {
    match command {
        Command::Camera(CameraCommand::Parameters) => {
            if payload == [0, 2, 0, 0, 1, 0] {
                return true;
            }
            payload.len() == 4
                && payload[..2] == [0, 1]
                && matches!(
                    CameraParameter::from(u16::from_le_bytes([payload[2], payload[3]])),
                    CameraParameter::Beauty
                        | CameraParameter::FpvControl
                        | CameraParameter::PreviousMode
                        | CameraParameter::Eis
                        | CameraParameter::FieldOfView
                        | CameraParameter::VoiceControl
                        | CameraParameter::QuickModeSwitch
                        | CameraParameter::VoiceLanguage
                        | CameraParameter::IsoLimit
                        | CameraParameter::Led
                        | CameraParameter::ScreenSleep
                        | CameraParameter::AutoOff
                        | CameraParameter::Unknown0014
                        | CameraParameter::SpotMetering
                        | CameraParameter::CaptureMinimumInterval
                        | CameraParameter::LoopDuration
                        | CameraParameter::P2PShare
                        | CameraParameter::SelfieFollowing
                        | CameraParameter::VideoBeautify
                )
        }
        Command::Common(CommonCommand::ModuleVersion)
        | Command::Camera(
            CameraCommand::UserIso
            | CameraCommand::UserShutter
            | CameraCommand::AeLock
            | CameraCommand::Audio
            | CameraCommand::Aeb
            | CameraCommand::Language
            | CameraCommand::VideoCaption
            | CameraCommand::PhotoInterval,
        ) => payload.is_empty(),
        Command::Camera(CameraCommand::StorageLocation) => payload == [0; 7],
        Command::Camera(CameraCommand::Watermark) => payload == [0x20, 0, 0, 0, 0, 0, 0],
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> ExplorationRequest {
        ExplorationRequest {
            receiver: Endpoint::Wifi,
            command: Command::Wifi(WifiCommand::Country),
            flags: Flags::REQUEST,
            payload: vec![],
            timeout: Duration::from_secs(1),
        }
    }

    #[test]
    fn exact_tuple_not_id_determines_read_permission() {
        request().validate(false, false).unwrap();
        for bad in [
            ExplorationRequest {
                payload: vec![1],
                ..request()
            },
            ExplorationRequest {
                receiver: Endpoint::Camera,
                ..request()
            },
            ExplorationRequest {
                flags: Flags::RESPONSE,
                ..request()
            },
        ] {
            assert!(bad.validate(false, false).is_err());
            assert!(bad.validate(true, false).is_err());
            assert!(bad.validate(false, true).is_err());
            bad.validate(true, true).unwrap();
        }
        for query in [
            CapabilityQuery::EiRange,
            CapabilityQuery::RecommendedEi,
            CapabilityQuery::LensInformation,
            CapabilityQuery::Accessories,
        ] {
            let message = Message::request(
                query.receiver(),
                MIN_ID,
                query.command(),
                &query.encode().unwrap(),
            );
            ExplorationRequest {
                receiver: message.receiver,
                command: message.command,
                payload: message.payload,
                ..request()
            }
            .validate(false, false)
            .unwrap();
        }
        let parameters = [0, 1, 0x0b, 0];
        ExplorationRequest {
            receiver: Endpoint::Camera,
            command: Command::Camera(CameraCommand::Parameters),
            payload: parameters.to_vec(),
            ..request()
        }
        .validate(false, false)
        .unwrap();
    }

    #[test]
    fn bounds_and_unsupported_rtmp_requests_cannot_be_approved() {
        for timeout in [
            Duration::ZERO,
            EXPLORATION_MAX_TIMEOUT + Duration::from_millis(1),
        ] {
            assert!(
                ExplorationRequest {
                    timeout,
                    ..request()
                }
                .validate(true, true)
                .is_err()
            );
        }
        for command in [
            Command::Camera(CameraCommand::Parameters),
            Command::from_wire(RTMP_SET, RTMP_COMMAND_078),
            Command::from_wire(RTMP_SET, RTMP_COMMAND_079),
        ] {
            assert!(
                ExplorationRequest {
                    receiver: Endpoint::from(RTMP_RECEIVER),
                    command,
                    ..request()
                }
                .validate(true, true)
                .is_err()
            );
        }
        assert!(
            ExplorationRequest {
                payload: vec![0; usize::from(DUML_LENGTH_MASK)],
                ..request()
            }
            .validate(true, true)
            .is_err()
        );
    }
}
