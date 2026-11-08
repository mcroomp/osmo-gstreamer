//! Read-only model metadata sourced from the native capability catalog.

use crate::camera_error;
use crate::typed::{VideoFrameRateCode, VideoResolutionCode};
use pyo3::{prelude::*, types::PyBytes};
#[cfg(feature = "stubgen")]
use pyo3_stub_gen::derive::{gen_stub_pyclass, gen_stub_pymethods};
use unofficial_osmo_camera::protocol::{
    capabilities::{
        CapabilityEvidence, CapabilitySelector, CommandCapability, NANO_10_00_31_22,
        video_formats::{
            NANO_FORMAT_OBSERVATIONS_SOURCE, NANO_PREVIEW_OBSERVATIONS, NANO_RECORDING_MODES,
            NANO_RECORDING_RESOLUTIONS, NANO_SPECIFICATIONS_SOURCE, nano_recording_formats,
        },
    },
    commands::camera::cmd_02_80_camera_state::NanoCaptureMode,
    decode, encode,
    video_formats::{PreviewObservation, RecordingAspect, RecordingFormat},
};

/// Open capture-mode code for explicit metadata context, not mode availability.
#[cfg_attr(feature = "stubgen", gen_stub_pyclass)]
#[pyclass(frozen, eq, module = "osmo_camera._native")]
#[derive(Clone, PartialEq, Eq)]
pub struct CaptureMode {
    inner: NanoCaptureMode,
}

#[cfg_attr(feature = "stubgen", gen_stub_pymethods)]
#[pymethods]
impl CaptureMode {
    #[new]
    fn new(code: u8) -> PyResult<Self> {
        Ok(Self {
            inner: decode(&[code], ()).map_err(camera_error)?,
        })
    }
    #[classattr]
    const VIDEO: CaptureMode = CaptureMode {
        inner: NanoCaptureMode::Video,
    };
    #[classattr]
    const SLOW_MOTION: CaptureMode = CaptureMode {
        inner: NanoCaptureMode::SlowMotion,
    };
    #[classattr]
    const SUPER_NIGHT: CaptureMode = CaptureMode {
        inner: NanoCaptureMode::SuperNight,
    };
    #[classattr]
    const TIMELAPSE: CaptureMode = CaptureMode {
        inner: NanoCaptureMode::Timelapse,
    };
    #[classattr]
    const HYPERLAPSE: CaptureMode = CaptureMode {
        inner: NanoCaptureMode::Hyperlapse,
    };
    #[classattr]
    const PHOTO: CaptureMode = CaptureMode {
        inner: NanoCaptureMode::Photo,
    };
    #[getter]
    fn code(&self) -> PyResult<u8> {
        Ok(encode(&self.inner).map_err(camera_error)?[0])
    }
    #[getter]
    fn name(&self) -> String {
        format!("{:?}", self.inner)
    }
}

/// One published combination, not a safe SET or current availability.
#[cfg_attr(feature = "stubgen", gen_stub_pyclass)]
#[pyclass(frozen, module = "osmo_camera._native")]
pub struct PublishedRecordingFormat {
    mode: NanoCaptureMode,
    inner: RecordingFormat,
}

#[cfg_attr(feature = "stubgen", gen_stub_pymethods)]
#[pymethods]
impl PublishedRecordingFormat {
    #[getter]
    fn mode(&self) -> CaptureMode {
        CaptureMode { inner: self.mode }
    }
    #[getter]
    fn resolution(&self) -> VideoResolutionCode {
        VideoResolutionCode {
            inner: self.inner.resolution.code,
        }
    }
    #[getter]
    fn frame_rate(&self) -> VideoFrameRateCode {
        VideoFrameRateCode {
            inner: self.inner.frame_rate,
        }
    }
    #[getter]
    fn dimensions(&self) -> (u16, u16) {
        self.inner.resolution.dimensions
    }
    #[getter]
    fn aspect(&self) -> String {
        format!("{:?}", self.inner.resolution.aspect)
    }
    #[getter]
    fn nominal_fps(&self) -> u16 {
        self.inner.nominal_fps
    }
    #[getter]
    fn slow_motion_factor(&self) -> Option<u8> {
        self.inner.slow_motion_factor
    }
}

/// Recording aspect metadata, deliberately not mapped to capability wire codes.
#[cfg_attr(feature = "stubgen", gen_stub_pyclass)]
#[pyclass(frozen, module = "osmo_camera._native")]
pub struct PublishedAspect {
    inner: RecordingAspect,
}

#[cfg_attr(feature = "stubgen", gen_stub_pymethods)]
#[pymethods]
impl PublishedAspect {
    #[getter]
    fn name(&self) -> String {
        format!("{:?}", self.inner)
    }
    #[getter]
    fn ratio(&self) -> (u8, u8) {
        match self.inner {
            RecordingAspect::SixteenByNine => (16, 9),
            RecordingAspect::FourByThree => (4, 3),
        }
    }
}

/// Published preview observation, never an output selector or measured stream.
#[cfg_attr(feature = "stubgen", gen_stub_pyclass)]
#[pyclass(frozen, module = "osmo_camera._native")]
pub struct PublishedPreviewObservation {
    inner: PreviewObservation,
}

#[cfg_attr(feature = "stubgen", gen_stub_pymethods)]
#[pymethods]
impl PublishedPreviewObservation {
    #[getter]
    fn aspect(&self) -> String {
        format!("{:?}", self.inner.recording_aspect)
    }
    #[getter]
    fn dimensions(&self) -> (u16, u16) {
        self.inner.dimensions
    }
    #[getter]
    fn nominal_fps(&self) -> u16 {
        self.inner.nominal_fps
    }
}

/// Static implementation coverage; neither evidence nor scope grants permission.
#[cfg_attr(feature = "stubgen", gen_stub_pyclass)]
#[pyclass(frozen, module = "osmo_camera._native")]
pub struct CommandContract {
    inner: CommandCapability,
}

#[cfg_attr(feature = "stubgen", gen_stub_pymethods)]
#[pymethods]
impl CommandContract {
    #[getter]
    fn receiver(&self) -> PyResult<u8> {
        Ok(encode(&self.inner.receiver).map_err(camera_error)?[0])
    }
    #[getter]
    fn command(&self) -> (u8, u8) {
        self.inner.command.wire()
    }
    #[getter]
    fn parameter(&self) -> Option<u16> {
        match self.inner.selector {
            CapabilitySelector::Parameter(id) => Some(id),
            CapabilitySelector::Request(_) => None,
        }
    }
    #[getter]
    fn request_payload<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyBytes>> {
        match self.inner.selector {
            CapabilitySelector::Parameter(_) => None,
            CapabilitySelector::Request(bytes) => Some(PyBytes::new(py, bytes)),
        }
    }
    #[getter]
    fn can_read(&self) -> bool {
        self.inner.access.can_read()
    }
    #[getter]
    fn can_write(&self) -> bool {
        self.inner.access.can_write()
    }
    #[getter]
    fn evidence(&self) -> String {
        format!("{:?}", self.inner.evidence)
    }
    #[getter]
    fn firmware(&self) -> Option<&'static str> {
        match self.inner.evidence {
            CapabilityEvidence::TargetedProfile => Some(NANO_10_00_31_22.firmware),
            CapabilityEvidence::CompatibilityContract => None,
        }
    }
    #[getter]
    fn source(&self) -> &'static str {
        "protocol::capabilities::NANO_10_00_31_22"
    }
}

/// Explicitly selected model metadata. It detects no camera and performs no IO.
///
/// Recording/preview sources do not identify firmware. Command coverage retains
/// its own evidence and firmware scope; targeted coverage is not certification.
#[cfg_attr(feature = "stubgen", gen_stub_pyclass)]
#[pyclass(frozen, module = "osmo_camera._native")]
pub struct CapabilityCatalog;

#[cfg_attr(feature = "stubgen", gen_stub_pymethods)]
#[pymethods]
impl CapabilityCatalog {
    #[staticmethod]
    fn nano() -> CapabilityCatalog {
        CapabilityCatalog
    }
    #[getter]
    fn modes(&self) -> Vec<CaptureMode> {
        NANO_RECORDING_MODES
            .into_iter()
            .map(|inner| CaptureMode { inner })
            .collect()
    }
    #[getter]
    fn model(&self) -> &'static str {
        NANO_10_00_31_22.model
    }
    #[getter]
    fn sources(&self) -> Vec<&'static str> {
        vec![NANO_SPECIFICATIONS_SOURCE, NANO_FORMAT_OBSERVATIONS_SOURCE]
    }
    #[getter]
    fn recording_formats(&self) -> PyResult<Vec<PublishedRecordingFormat>> {
        let mut formats = Vec::new();
        for mode in NANO_RECORDING_MODES {
            for inner in nano_recording_formats(mode).map_err(camera_error)? {
                formats.push(PublishedRecordingFormat { mode, inner });
            }
        }
        Ok(formats)
    }
    #[getter]
    fn recording_aspects(&self) -> Vec<PublishedAspect> {
        let mut aspects = Vec::new();
        for resolution in NANO_RECORDING_RESOLUTIONS {
            if !aspects.contains(&resolution.aspect) {
                aspects.push(resolution.aspect);
            }
        }
        aspects
            .into_iter()
            .map(|inner| PublishedAspect { inner })
            .collect()
    }
    #[getter]
    fn preview_observations(&self) -> Vec<PublishedPreviewObservation> {
        NANO_PREVIEW_OBSERVATIONS
            .into_iter()
            .map(|inner| PublishedPreviewObservation { inner })
            .collect()
    }
    #[getter]
    fn command_contracts(&self) -> Vec<CommandContract> {
        NANO_10_00_31_22
            .commands
            .iter()
            .copied()
            .map(|inner| CommandContract { inner })
            .collect()
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PublishedRecordingFormat>()?;
    module.add_class::<PublishedAspect>()?;
    module.add_class::<PublishedPreviewObservation>()?;
    module.add_class::<CommandContract>()?;
    module.add_class::<CaptureMode>()?;
    module.add_class::<CapabilityCatalog>()?;
    Ok(())
}
