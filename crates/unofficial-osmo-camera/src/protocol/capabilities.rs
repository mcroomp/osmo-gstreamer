//! Model/firmware-scoped command contracts, independent of payload codecs.
//!
//! Coverage is not current-mode availability, device detection or permission.
//! Compatibility contracts are not verified model support. Unknown entries
//! remain unknown; profiles do not inherit another model's capabilities.

use super::{
    Query,
    commands::{
        self,
        camera::{
            cmd_02_8e_parameters as parameters,
            cmd_02_e0_capabilities_get::{
                AccessoriesGet, EiRangeGet, LensInformationGet, RecommendedEiGet,
            },
        },
        media::cmd_02_8e_streaming_control::MediaStreamingStateGet,
        wifi::cmd_07_39_get_role::WifiRoleGet,
    },
    identity::{CameraCommand, Command, CommonCommand, Endpoint, WifiCommand},
};
use crate::{Error, Result};
use parameters::Parameter;

/// Established operation coverage, not authorization.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperationAccess {
    ReadOnly,
    WriteOnly,
    ReadWrite,
}

impl OperationAccess {
    pub fn can_read(self) -> bool {
        matches!(self, Self::ReadOnly | Self::ReadWrite)
    }

    pub fn can_write(self) -> bool {
        matches!(self, Self::WriteOnly | Self::ReadWrite)
    }
}

/// Scope of the implementation's evidence, not an official specification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapabilityEvidence {
    TargetedProfile,
    CompatibilityContract,
}

/// A selector within a command, or the complete bytes of a request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapabilitySelector {
    Parameter(u16),
    Request(&'static [u8]),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommandCapability {
    pub receiver: Endpoint,
    pub command: Command,
    pub selector: CapabilitySelector,
    pub access: OperationAccess,
    pub evidence: CapabilityEvidence,
}

impl CommandCapability {
    const fn parameter<T: Parameter>(access: OperationAccess) -> Self {
        Self {
            receiver: Endpoint::Camera,
            command: Command::Camera(CameraCommand::Parameters),
            selector: CapabilitySelector::Parameter(T::ID),
            access,
            evidence: CapabilityEvidence::TargetedProfile,
        }
    }

    const fn query_read<T: Query>(receiver: Endpoint, command: Command) -> Self {
        Self {
            receiver,
            command,
            selector: CapabilitySelector::Request(T::REQUEST_PAYLOAD),
            access: OperationAccess::ReadOnly,
            evidence: CapabilityEvidence::CompatibilityContract,
        }
    }
}

/// Explicit coverage for one model and firmware scope.
///
/// Callers can supply independent tables for other models. No profile is
/// selected automatically, and custom tables do not change raw approval rules.
#[derive(Clone, Copy, Debug)]
pub struct CameraCapabilities {
    pub model: &'static str,
    pub firmware: &'static str,
    pub commands: &'static [CommandCapability],
}

impl CameraCapabilities {
    pub fn lookup(
        &self,
        receiver: Endpoint,
        command: Command,
        selector: CapabilitySelector,
    ) -> Option<&CommandCapability> {
        self.commands.iter().find(|entry| {
            entry.receiver == receiver && entry.command == command && entry.selector == selector
        })
    }

    /// Find a known read contract using an exact endpoint, command and payload.
    pub fn read_contract(
        &self,
        receiver: Endpoint,
        command: Command,
        payload: &[u8],
    ) -> Option<&CommandCapability> {
        self.commands.iter().find(|entry| {
            entry.receiver == receiver
                && entry.command == command
                && entry.access.can_read()
                && match entry.selector {
                    CapabilitySelector::Parameter(id) => {
                        let [0, 1, low, high] = payload else {
                            return false;
                        };
                        id == u16::from_le_bytes([*low, *high])
                    }
                    CapabilitySelector::Request(expected) => payload == expected,
                }
        })
    }

    /// Require coverage for a typed query, without executing it.
    ///
    /// The returned evidence distinguishes profile coverage from a compatibility
    /// contract. Callers must not interpret either as device authorization.
    pub fn require_query<Q: Query>(&self) -> Result<&CommandCapability> {
        let query = Q::default();
        let payload = query.payload()?;
        self.read_contract(query.receiver(), query.command(), &payload)
            .ok_or(Error::Unsupported(
                "query is not covered by the camera capability profile",
            ))
    }
}

/// Targeted Nano parameter contracts and other implemented compatibility reads.
///
/// Parameter coverage retains the 10.00.31.22 scope. Other reads carry explicit
/// compatibility evidence rather than asserting verified firmware support.
pub const NANO_10_00_31_22: CameraCapabilities = CameraCapabilities {
    model: "Osmo Nano",
    firmware: "10.00.31.22",
    commands: &[
        CommandCapability::parameter::<parameters::AdvanceModeValue>(OperationAccess::ReadWrite),
        CommandCapability::parameter::<parameters::StreamQualityValue>(OperationAccess::ReadWrite),
        CommandCapability::parameter::<parameters::EisValue>(OperationAccess::ReadWrite),
        CommandCapability::parameter::<parameters::FieldOfViewValue>(OperationAccess::ReadWrite),
        CommandCapability::parameter::<parameters::VoiceControlValue>(OperationAccess::ReadWrite),
        CommandCapability::parameter::<parameters::VoiceLanguageValue>(OperationAccess::ReadWrite),
        CommandCapability::parameter::<parameters::IsoLimitValue>(OperationAccess::ReadWrite),
        CommandCapability::parameter::<parameters::LensControlValue>(OperationAccess::ReadOnly),
        CommandCapability::parameter::<parameters::Unknown001fValue>(OperationAccess::ReadWrite),
        CommandCapability::parameter::<parameters::RecordingAudioChannelValue>(
            OperationAccess::ReadWrite,
        ),
        CommandCapability::parameter::<parameters::SelfieFollowingValue>(
            OperationAccess::ReadWrite,
        ),
        CommandCapability::parameter::<parameters::Unknown0028Value>(OperationAccess::ReadWrite),
        CommandCapability::parameter::<parameters::StabilizationPreferenceValue>(
            OperationAccess::ReadWrite,
        ),
        CommandCapability::parameter::<parameters::ImageAdjustmentValue>(
            OperationAccess::ReadWrite,
        ),
        CommandCapability::parameter::<parameters::ShutterLimitValue>(OperationAccess::ReadWrite),
        CommandCapability::parameter::<parameters::VideoQualityEnhancementValue>(
            OperationAccess::ReadWrite,
        ),
        CommandCapability::parameter::<parameters::SelfieMirrorValue>(OperationAccess::ReadWrite),
        CommandCapability::parameter::<parameters::RecordingPlanValue>(OperationAccess::ReadWrite),
        CommandCapability::parameter::<parameters::LiveViewEisValue>(OperationAccess::ReadWrite),
        CommandCapability::parameter::<parameters::LoopDurationValue>(OperationAccess::ReadWrite),
        CommandCapability::parameter::<parameters::CaptureAspectTypeValue>(
            OperationAccess::ReadWrite,
        ),
        CommandCapability::parameter::<parameters::RotationStateValue>(OperationAccess::ReadOnly),
        CommandCapability::parameter::<parameters::ShortVideoParametersValue>(
            OperationAccess::ReadWrite,
        ),
        CommandCapability::parameter::<parameters::StorageOutcomeValue>(OperationAccess::ReadOnly),
        CommandCapability::parameter::<parameters::BeautyEffectValue>(OperationAccess::ReadWrite),
        CommandCapability::parameter::<parameters::AutofocusSensitivityValue>(
            OperationAccess::ReadWrite,
        ),
        CommandCapability::parameter::<parameters::AutofocusStrategyValue>(
            OperationAccess::ReadWrite,
        ),
        CommandCapability::parameter::<parameters::Unknown003cValue>(OperationAccess::ReadWrite),
        CommandCapability::parameter::<parameters::Unknown0019Value>(OperationAccess::WriteOnly),
        CommandCapability::parameter::<parameters::Unknown0026Value>(OperationAccess::WriteOnly),
        CommandCapability::parameter::<parameters::LensUserCalibrationValue>(
            OperationAccess::WriteOnly,
        ),
        CommandCapability::parameter::<parameters::LensUserCalibrationStateValue>(
            OperationAccess::ReadOnly,
        ),
        CommandCapability::parameter::<parameters::LensUserCalibrationFileValue>(
            OperationAccess::WriteOnly,
        ),
        CommandCapability::parameter::<parameters::AddCustomModeValue>(OperationAccess::WriteOnly),
        CommandCapability::parameter::<parameters::StorageSynchronizationValue>(
            OperationAccess::WriteOnly,
        ),
        CommandCapability::parameter::<parameters::TimelapseTrackPreviewValue>(
            OperationAccess::WriteOnly,
        ),
        CommandCapability::parameter::<parameters::TimelapseTrackSetValue>(
            OperationAccess::WriteOnly,
        ),
        CommandCapability::query_read::<MediaStreamingStateGet>(
            Endpoint::Media,
            Command::Camera(CameraCommand::Parameters),
        ),
        CommandCapability::query_read::<commands::wifi::cmd_07_19_country_get::WifiCountryGet>(
            Endpoint::Wifi,
            Command::Wifi(WifiCommand::Country),
        ),
        CommandCapability::query_read::<commands::wifi::cmd_07_07_ssid::WifiSsidGet>(
            Endpoint::Wifi,
            Command::Wifi(WifiCommand::Ssid),
        ),
        CommandCapability::query_read::<commands::wifi::cmd_07_0c_mac_get::WifiMacGet>(
            Endpoint::Wifi,
            Command::Wifi(WifiCommand::Mac),
        ),
        CommandCapability::query_read::<WifiRoleGet>(
            Endpoint::Wifi,
            Command::Wifi(WifiCommand::GetRole),
        ),
        CommandCapability::query_read::<EiRangeGet>(
            Endpoint::Camera,
            Command::Camera(CameraCommand::Capabilities),
        ),
        CommandCapability::query_read::<RecommendedEiGet>(
            Endpoint::Camera,
            Command::Camera(CameraCommand::Capabilities),
        ),
        CommandCapability::query_read::<LensInformationGet>(
            Endpoint::Camera,
            Command::Camera(CameraCommand::Capabilities),
        ),
        CommandCapability::query_read::<AccessoriesGet>(
            Endpoint::Camera,
            Command::Camera(CameraCommand::Capabilities),
        ),
        CommandCapability::query_read::<
            commands::common::cmd_00_01_module_version_get::CameraModuleVersionGet,
        >(
            Endpoint::Camera,
            Command::Common(CommonCommand::ModuleVersion),
        ),
        CommandCapability::query_read::<commands::camera::cmd_02_2b_user_iso_get::CameraUserIsoGet>(
            Endpoint::Camera,
            Command::Camera(CameraCommand::UserIso),
        ),
        CommandCapability::query_read::<
            commands::camera::cmd_02_29_user_shutter_get::CameraUserShutterGet,
        >(
            Endpoint::Camera,
            Command::Camera(CameraCommand::UserShutter),
        ),
        CommandCapability::query_read::<commands::camera::cmd_02_69_ae_lock_get::CameraAeLockGet>(
            Endpoint::Camera,
            Command::Camera(CameraCommand::AeLock),
        ),
        CommandCapability::query_read::<commands::camera::cmd_02_a0_audio_get::CameraAudioGet>(
            Endpoint::Camera,
            Command::Camera(CameraCommand::Audio),
        ),
        CommandCapability::query_read::<commands::camera::cmd_02_5f_aeb_get::CameraAebGet>(
            Endpoint::Camera,
            Command::Camera(CameraCommand::Aeb),
        ),
        CommandCapability::query_read::<commands::camera::cmd_02_57_language_get::CameraLanguageGet>(
            Endpoint::Camera,
            Command::Camera(CameraCommand::Language),
        ),
        CommandCapability::query_read::<
            commands::camera::cmd_02_63_video_caption_get::CameraVideoCaptionGet,
        >(
            Endpoint::Camera,
            Command::Camera(CameraCommand::VideoCaption),
        ),
        CommandCapability::query_read::<
            commands::camera::cmd_02_4b_photo_interval_get::CameraPhotoIntervalGet,
        >(
            Endpoint::Camera,
            Command::Camera(CameraCommand::PhotoInterval),
        ),
        CommandCapability::query_read::<
            commands::camera::cmd_02_da_storage_location_get::CameraStorageLocationGet,
        >(
            Endpoint::Camera,
            Command::Camera(CameraCommand::StorageLocation),
        ),
        CommandCapability::query_read::<
            commands::camera::cmd_02_e5_watermark_get::CameraWatermarkGet,
        >(Endpoint::Camera, Command::Camera(CameraCommand::Watermark)),
    ],
};

// Legacy approval contracts are not Nano-profile support.
const LEGACY_PARAMETER_READ_IDS: &[u16] = &[
    parameters::BeautyValue::ID,
    parameters::FpvControlValue::ID,
    parameters::PreviousModeValue::ID,
    parameters::QuickModeSwitchValue::ID,
    parameters::ScreenSleepValue::ID,
    parameters::AutoOffValue::ID,
    parameters::SpotMeteringValue::ID,
    parameters::LedValue::ID,
    parameters::CaptureMinimumIntervalValue::ID,
    parameters::P2PShareValue::ID,
    parameters::VideoBeautifyValue::ID,
];

pub(crate) fn is_known_read(receiver: Endpoint, command: Command, payload: &[u8]) -> bool {
    if NANO_10_00_31_22
        .read_contract(receiver, command, payload)
        .is_some()
    {
        return true;
    }
    if receiver != Endpoint::Camera || command != Command::Camera(CameraCommand::Parameters) {
        return false;
    }
    if payload == [0, 2, 0, 0, 1, 0] {
        return true;
    }
    let [0, 1, low, high] = payload else {
        return false;
    };
    LEGACY_PARAMETER_READ_IDS.contains(&u16::from_le_bytes([*low, *high]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{
        ExplorationRequest, Flags,
        commands::{
            camera::{
                cmd_02_8e_parameters::{EisValue, Get},
                cmd_02_e0_capabilities_get::AccessoriesGet,
            },
            media::cmd_02_8e_streaming_control::MediaStreamingStateGet,
            wifi::cmd_07_39_get_role::WifiRoleGet,
        },
    };
    use std::time::Duration;

    #[test]
    fn model_tables_are_independent_and_missing_coverage_never_inherits_nano() {
        const OTHER: CameraCapabilities = CameraCapabilities {
            model: "Synthetic model",
            firmware: "Synthetic firmware",
            commands: &[CommandCapability {
                receiver: Endpoint::Camera,
                command: Command::Camera(CameraCommand::Parameters),
                selector: CapabilitySelector::Parameter(8),
                access: OperationAccess::WriteOnly,
                evidence: CapabilityEvidence::CompatibilityContract,
            }],
        };
        assert!(NANO_10_00_31_22.require_query::<Get<EisValue>>().is_ok());
        assert!(OTHER.require_query::<Get<EisValue>>().is_err());
        assert!(OTHER.require_query::<AccessoriesGet>().is_err());
        assert_eq!(
            OTHER
                .lookup(
                    Endpoint::Camera,
                    Command::Camera(CameraCommand::Parameters),
                    CapabilitySelector::Parameter(8),
                )
                .unwrap()
                .access,
            OperationAccess::WriteOnly,
        );
    }

    #[test]
    fn other_command_queries_retain_their_compatibility_evidence() {
        for entry in [
            NANO_10_00_31_22.require_query::<AccessoriesGet>().unwrap(),
            NANO_10_00_31_22
                .require_query::<MediaStreamingStateGet>()
                .unwrap(),
            NANO_10_00_31_22.require_query::<WifiRoleGet>().unwrap(),
        ] {
            assert_eq!(entry.evidence, CapabilityEvidence::CompatibilityContract);
        }
        assert_eq!(
            NANO_10_00_31_22
                .require_query::<Get<EisValue>>()
                .unwrap()
                .evidence,
            CapabilityEvidence::TargetedProfile,
        );
    }

    #[test]
    fn exact_request_coverage_preserves_endpoint_payload_and_flag_approval_boundaries() {
        let mut count = 0;
        for entry in NANO_10_00_31_22.commands {
            let payload = match entry.selector {
                CapabilitySelector::Request(payload) => payload,
                CapabilitySelector::Parameter(_) => continue,
            };
            count += 1;
            let request = ExplorationRequest {
                receiver: entry.receiver,
                command: entry.command,
                flags: Flags::REQUEST,
                payload: payload.to_vec(),
                timeout: Duration::from_secs(1),
            };
            request.validate(false, false).unwrap();
            let mut extended = request.clone();
            extended.payload.push(0xff);
            let wrong_endpoint = ExplorationRequest {
                receiver: Endpoint::App,
                ..request.clone()
            };
            let response = ExplorationRequest {
                flags: Flags::RESPONSE,
                ..request
            };
            for changed in [extended, wrong_endpoint, response] {
                assert!(!changed.is_known_read());
                assert!(changed.validate(false, false).is_err());
            }
        }
        assert_eq!(count, 20);
    }

    #[test]
    fn custom_capability_tables_cannot_grant_raw_request_permission() {
        const CUSTOM: CameraCapabilities = CameraCapabilities {
            model: "Synthetic model",
            firmware: "Synthetic firmware",
            commands: &[CommandCapability {
                receiver: Endpoint::Camera,
                command: Command::Camera(CameraCommand::Unknown(0xfe)),
                selector: CapabilitySelector::Request(&[0]),
                access: OperationAccess::ReadOnly,
                evidence: CapabilityEvidence::CompatibilityContract,
            }],
        };
        let entry = &CUSTOM.commands[0];
        assert!(
            CUSTOM
                .read_contract(entry.receiver, entry.command, &[0])
                .is_some()
        );
        let request = ExplorationRequest {
            receiver: entry.receiver,
            command: entry.command,
            flags: Flags::REQUEST,
            payload: vec![0],
            timeout: Duration::from_secs(1),
        };
        assert!(!request.is_known_read());
        assert!(request.validate(false, false).is_err());
        request.validate(true, true).unwrap();
    }
}
