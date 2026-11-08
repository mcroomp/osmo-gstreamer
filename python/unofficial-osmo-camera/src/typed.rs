//! Typed wire contracts with explicit, lossless bridges to raw exploration.

use crate::{
    camera_error,
    raw::{Message, MessageContext, Request, request_timeout},
};
use pyo3::{prelude::*, types::PyBytes};
#[cfg(feature = "stubgen")]
use pyo3_stub_gen::derive::{gen_stub_pyclass, gen_stub_pymethods};
use unofficial_osmo_camera::{
    Error,
    protocol::{
        CommandPacket, Message as NativeMessage, ResponseStatus,
        commands::{
            camera::cmd_02_18_video_format_set::{
                VideoFrameRateCode as NativeFrameRate, VideoResolutionCode as NativeResolution,
            },
            common::cmd_00_99_capability_topics::{
                CapabilityAvailability, CapabilityNotification as NativeNotification,
                CapabilitySubscribe as NativeSubscribe, CapabilityTable as NativeTable,
                CapabilityTopic as NativeTopic, SlowMotionCapability as NativeSlowMotion,
                VideoFormatCapability as NativeVideoFormat,
            },
            reply::{CameraReadReply, RemainingBytes},
        },
        decode, encode,
        identity::Command,
    },
};

/// A supported topic name, not a subscription or a device-support claim.
#[cfg_attr(feature = "stubgen", gen_stub_pyclass)]
#[pyclass(frozen, eq, module = "osmo_camera._native")]
#[derive(Clone, PartialEq, Eq)]
pub struct CapabilityTopic {
    pub(crate) inner: NativeTopic,
}

#[cfg_attr(feature = "stubgen", gen_stub_pymethods)]
#[pymethods]
impl CapabilityTopic {
    #[classattr]
    const VIDEO_FORMATS: CapabilityTopic = CapabilityTopic {
        inner: NativeTopic::VideoFormats,
    };
    #[classattr]
    const SLOW_MOTION_RATIOS: CapabilityTopic = CapabilityTopic {
        inner: NativeTopic::SlowMotionRatios,
    };
    #[classattr]
    const CAPTURE_ASPECTS: CapabilityTopic = CapabilityTopic {
        inner: NativeTopic::CaptureAspects,
    };

    #[getter]
    fn name<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, self.inner.name())
    }
}

/// An offline subscription request. Transmission requires separate approvals.
#[cfg_attr(feature = "stubgen", gen_stub_pyclass)]
#[pyclass(frozen, module = "osmo_camera._native")]
#[derive(Clone)]
pub struct CapabilitySubscribe {
    pub(crate) inner: NativeSubscribe,
}

#[cfg_attr(feature = "stubgen", gen_stub_pymethods)]
#[pymethods]
impl CapabilitySubscribe {
    #[new]
    fn new(topic: &CapabilityTopic, subscription_id: u32) -> Self {
        Self {
            inner: NativeSubscribe::new(topic.inner, subscription_id),
        }
    }

    #[getter]
    fn topic(&self) -> CapabilityTopic {
        CapabilityTopic {
            inner: self.inner.topic,
        }
    }

    #[getter]
    fn subscription_id(&self) -> u32 {
        self.inner.subscription_id
    }

    #[getter]
    fn command(&self) -> (u8, u8) {
        self.inner.command().wire()
    }

    fn payload<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        Ok(PyBytes::new(
            py,
            &self.inner.payload().map_err(camera_error)?,
        ))
    }

    fn encode<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        Ok(PyBytes::new(
            py,
            &encode(&self.inner).map_err(camera_error)?,
        ))
    }

    fn frame(&self, sequence: u16) -> PyResult<Message> {
        Ok(self.to_raw(1.0)?.frame(sequence))
    }

    /// Convert to an outbound raw request without sending or granting approval.
    #[pyo3(signature = (*, timeout=3.0))]
    pub(crate) fn to_raw(&self, timeout: f64) -> PyResult<Request> {
        Ok(Request {
            inner: self
                .inner
                .exploration_request(request_timeout(timeout)?)
                .map_err(camera_error)?,
        })
    }

    #[pyo3(signature = (*, allow_unclassified=false, approve_unclassified=false, timeout=3.0))]
    fn validate(
        &self,
        allow_unclassified: bool,
        approve_unclassified: bool,
        timeout: f64,
    ) -> PyResult<()> {
        self.to_raw(timeout)?
            .validate(allow_unclassified, approve_unclassified)
    }

    pub(crate) fn decode_reply(&self, reply: &Message) -> PyResult<CapabilitySubscriptionReply> {
        Ok(CapabilitySubscriptionReply {
            inner: self
                .inner
                .decode_reply(&reply.inner)
                .map_err(camera_error)?
                .value,
            raw: reply.inner.clone(),
        })
    }
}

/// Status and opaque extension from a subscription handoff reply.
#[cfg_attr(feature = "stubgen", gen_stub_pyclass)]
#[pyclass(frozen, module = "osmo_camera._native")]
pub struct CapabilitySubscriptionReply {
    inner: CameraReadReply<RemainingBytes>,
    raw: NativeMessage,
}

#[cfg_attr(feature = "stubgen", gen_stub_pymethods)]
#[pymethods]
impl CapabilitySubscriptionReply {
    /// Original validated response, retaining transport metadata and opaque bytes.
    #[getter]
    fn raw(&self) -> Message {
        self.to_raw()
    }

    fn to_raw(&self) -> Message {
        Message {
            inner: self.raw.clone(),
        }
    }

    #[getter]
    fn accepted(&self) -> bool {
        matches!(self.inner, CameraReadReply::Accepted(_))
    }

    #[getter]
    fn status(&self) -> u8 {
        match &self.inner {
            CameraReadReply::Accepted(_) => 0,
            CameraReadReply::Rejected {
                status: ResponseStatus::Unknown(value),
                ..
            } => *value,
            CameraReadReply::Rejected {
                status: ResponseStatus::Success,
                ..
            } => 0,
        }
    }

    #[getter]
    fn extension<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        let bytes = match &self.inner {
            CameraReadReply::Accepted(bytes) => bytes,
            CameraReadReply::Rejected { extension, .. } => extension,
        };
        PyBytes::new(py, bytes.as_slice())
    }

    fn require_success(&self) -> PyResult<()> {
        match &self.inner {
            CameraReadReply::Accepted(_) => Ok(()),
            CameraReadReply::Rejected { status, .. } => Err(camera_error(Error::CameraRejected {
                command: Command::from_wire(
                    NativeSubscribe::COMMAND_ID[0],
                    NativeSubscribe::COMMAND_ID[1],
                ),
                status: *status,
            })),
        }
    }
}

/// Open resolution code with generic dimension metadata, not model availability.
#[cfg_attr(feature = "stubgen", gen_stub_pyclass)]
#[pyclass(frozen, eq, module = "osmo_camera._native")]
#[derive(Clone, PartialEq, Eq)]
pub struct VideoResolutionCode {
    pub(crate) inner: NativeResolution,
}

#[cfg_attr(feature = "stubgen", gen_stub_pymethods)]
#[pymethods]
impl VideoResolutionCode {
    #[new]
    fn new(code: u8) -> Self {
        Self {
            inner: NativeResolution::from(code),
        }
    }
    #[getter]
    fn code(&self) -> u8 {
        self.inner.wire()
    }
    #[getter]
    fn name(&self) -> String {
        format!("{:?}", self.inner)
    }
    #[getter]
    fn dimensions(&self) -> Option<(u16, u16)> {
        self.inner.info().and_then(|info| info.dimensions)
    }
}

/// Open frame-rate code; a label is not a measured cadence.
#[cfg_attr(feature = "stubgen", gen_stub_pyclass)]
#[pyclass(frozen, eq, module = "osmo_camera._native")]
#[derive(Clone, PartialEq, Eq)]
pub struct VideoFrameRateCode {
    pub(crate) inner: NativeFrameRate,
}

#[cfg_attr(feature = "stubgen", gen_stub_pymethods)]
#[pymethods]
impl VideoFrameRateCode {
    #[new]
    fn new(code: u8) -> PyResult<Self> {
        Ok(Self {
            inner: decode(&[code], ()).map_err(camera_error)?,
        })
    }
    #[getter]
    fn code(&self) -> PyResult<u8> {
        Ok(encode(&self.inner).map_err(camera_error)?[0])
    }
    #[getter]
    fn name(&self) -> String {
        format!("{:?}", self.inner)
    }
}

/// One recording capability row, including its uninterpreted byte.
#[cfg_attr(feature = "stubgen", gen_stub_pyclass)]
#[pyclass(frozen, module = "osmo_camera._native")]
pub struct VideoFormatCapability {
    inner: NativeVideoFormat,
}

#[cfg_attr(feature = "stubgen", gen_stub_pymethods)]
#[pymethods]
impl VideoFormatCapability {
    #[getter]
    fn resolution(&self) -> VideoResolutionCode {
        VideoResolutionCode {
            inner: self.inner.resolution,
        }
    }
    #[getter]
    fn frame_rate(&self) -> VideoFrameRateCode {
        VideoFrameRateCode {
            inner: self.inner.frame_rate,
        }
    }
    #[getter]
    fn opaque(&self) -> u8 {
        self.inner.opaque
    }
}

/// One base frame-rate/factor row; it contains no resolution.
#[cfg_attr(feature = "stubgen", gen_stub_pyclass)]
#[pyclass(frozen, module = "osmo_camera._native")]
pub struct SlowMotionCapability {
    inner: NativeSlowMotion,
}

#[cfg_attr(feature = "stubgen", gen_stub_pymethods)]
#[pymethods]
impl SlowMotionCapability {
    #[getter]
    fn frame_rate(&self) -> VideoFrameRateCode {
        VideoFrameRateCode {
            inner: self.inner.frame_rate,
        }
    }
    #[getter]
    fn factor(&self) -> u8 {
        self.inner.factor
    }
}

/// Lossless availability envelope. Unknown availability cannot become success.
#[cfg_attr(feature = "stubgen", gen_stub_pyclass)]
#[pyclass(frozen, module = "osmo_camera._native")]
pub struct CapabilityTable {
    inner: NativeTable,
}

#[cfg_attr(feature = "stubgen", gen_stub_pymethods)]
#[pymethods]
impl CapabilityTable {
    #[staticmethod]
    fn decode(bytes: &Bound<'_, PyBytes>) -> PyResult<Self> {
        Ok(Self {
            inner: decode(bytes.as_bytes(), ()).map_err(camera_error)?,
        })
    }
    fn encode<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        Ok(PyBytes::new(
            py,
            &encode(&self.inner).map_err(camera_error)?,
        ))
    }
    #[getter]
    fn availability(&self) -> PyResult<u8> {
        Ok(encode(&self.inner.availability).map_err(camera_error)?[0])
    }
    #[getter]
    fn available(&self) -> PyResult<bool> {
        match self.inner.availability {
            CapabilityAvailability::Available => Ok(true),
            CapabilityAvailability::Unavailable => Ok(false),
            CapabilityAvailability::Unknown(value) => Err(camera_error(Error::UnsupportedValue {
                field: "capability availability",
                value,
            })),
        }
    }
    #[getter]
    fn data<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.inner.table)
    }
}

/// Named notification; decoding never subscribes or acknowledges.
#[cfg_attr(feature = "stubgen", gen_stub_pyclass)]
#[pyclass(frozen, module = "osmo_camera._native")]
pub struct CapabilityNotification {
    inner: NativeNotification,
    raw: Option<NativeMessage>,
}

#[cfg_attr(feature = "stubgen", gen_stub_pymethods)]
#[pymethods]
impl CapabilityNotification {
    #[staticmethod]
    fn decode(body: &Bound<'_, PyBytes>) -> PyResult<Self> {
        Ok(Self {
            inner: decode(body.as_bytes(), ()).map_err(camera_error)?,
            raw: None,
        })
    }
    /// Validate known routing and body layout while retaining the original frame.
    #[staticmethod]
    fn from_raw(message: &Message) -> PyResult<Self> {
        Ok(Self {
            inner: NativeNotification::from_message(&message.inner).map_err(camera_error)?,
            raw: Some(message.inner.clone()),
        })
    }

    #[getter]
    fn raw(&self) -> Option<Message> {
        self.raw.as_ref().map(|inner| Message {
            inner: inner.clone(),
        })
    }

    /// Return the original frame, or explicitly reframe with caller-supplied headers.
    #[pyo3(signature = (context=None))]
    fn to_raw(&self, context: Option<&MessageContext>) -> PyResult<Message> {
        if let Some(context) = context {
            return Ok(Message {
                inner: self.inner.frame(context.inner).map_err(camera_error)?,
            });
        }
        self.raw
            .as_ref()
            .map(|inner| Message {
                inner: inner.clone(),
            })
            .ok_or_else(|| {
                camera_error(Error::InvalidConfig {
                    field: "notification framing",
                    reason: "a body-only notification requires an explicit raw MessageContext",
                })
            })
    }
    #[staticmethod]
    fn acknowledgment(message: &Message) -> PyResult<Message> {
        Ok(Message {
            inner: NativeNotification::acknowledgment(&message.inner).map_err(camera_error)?,
        })
    }
    fn encode<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        Ok(PyBytes::new(
            py,
            &encode(&self.inner).map_err(camera_error)?,
        ))
    }
    #[getter]
    fn topic(&self) -> Option<CapabilityTopic> {
        self.inner.topic().map(|inner| CapabilityTopic { inner })
    }
    #[getter]
    fn name<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.inner.name)
    }
    #[getter]
    fn sequence(&self) -> u16 {
        self.inner.sequence
    }
    #[getter]
    fn remaining_length(&self) -> u16 {
        self.inner.remaining_length
    }
    #[getter]
    fn opaque_prefix<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.inner.opaque_prefix)
    }
    #[getter]
    fn opaque_before_data<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.inner.opaque_before_data)
    }
    #[getter]
    fn data<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.inner.data)
    }
    fn table(&self) -> PyResult<CapabilityTable> {
        Ok(CapabilityTable {
            inner: decode(&self.inner.data, ()).map_err(camera_error)?,
        })
    }
    fn video_formats(&self) -> PyResult<Option<Vec<VideoFormatCapability>>> {
        Ok(self
            .inner
            .video_formats()
            .map_err(camera_error)?
            .map(|rows| {
                rows.into_iter()
                    .map(|inner| VideoFormatCapability { inner })
                    .collect()
            }))
    }
    fn slow_motion_ratios(&self) -> PyResult<Option<Vec<SlowMotionCapability>>> {
        Ok(self
            .inner
            .slow_motion_ratios()
            .map_err(camera_error)?
            .map(|rows| {
                rows.into_iter()
                    .map(|inner| SlowMotionCapability { inner })
                    .collect()
            }))
    }
    fn capture_aspects(&self) -> PyResult<Option<Vec<u16>>> {
        // PyO3 converts Vec<u8> to bytes; these codes are exposed as integer rows.
        Ok(self
            .inner
            .capture_aspects()
            .map_err(camera_error)?
            .map(|codes| codes.into_iter().map(u16::from).collect()))
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<CapabilityTopic>()?;
    module.add_class::<CapabilitySubscribe>()?;
    module.add_class::<CapabilitySubscriptionReply>()?;
    module.add_class::<VideoResolutionCode>()?;
    module.add_class::<VideoFrameRateCode>()?;
    module.add_class::<VideoFormatCapability>()?;
    module.add_class::<SlowMotionCapability>()?;
    module.add_class::<CapabilityTable>()?;
    module.add_class::<CapabilityNotification>()?;
    Ok(())
}
