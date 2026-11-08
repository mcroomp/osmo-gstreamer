//! Lossless frame inspection and bounded, explicitly approved raw requests.

use crate::camera_error;
use pyo3::{prelude::*, types::PyBytes};
#[cfg(feature = "stubgen")]
use pyo3_stub_gen::derive::{gen_stub_pyclass, gen_stub_pymethods};
use std::time::Duration;
use unofficial_osmo_camera::{
    Error,
    protocol::{
        CommandBody, ExplorationRequest, Flags, Message as NativeMessage,
        MessageContext as NativeContext, decode, encode,
        identity::{Command, Endpoint},
    },
};

pub(crate) fn request_timeout(seconds: f64) -> PyResult<Duration> {
    if !seconds.is_finite() || !(0.1..=30.0).contains(&seconds) {
        return Err(camera_error(Error::InvalidConfig {
            field: "timeout",
            reason: "must be finite and 0.1..30 seconds",
        }));
    }
    Ok(Duration::from_secs_f64(seconds))
}

/// A DUML frame with open identities and uninterpreted payload bytes.
///
/// Decoding checks framing integrity, not command payload semantics or support.
/// Its default representation never includes payloads.
#[cfg_attr(feature = "stubgen", gen_stub_pyclass)]
#[pyclass(frozen, module = "osmo_camera._native")]
#[derive(Clone)]
pub struct Message {
    pub(crate) inner: NativeMessage,
}

#[cfg_attr(feature = "stubgen", gen_stub_pymethods)]
#[pymethods]
impl Message {
    #[new]
    fn new(
        sender: u8,
        receiver: u8,
        sequence: u16,
        flags: u8,
        command_set: u8,
        command_id: u8,
        payload: &Bound<'_, PyBytes>,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: NativeMessage {
                sender: decode::<Endpoint>(&[sender], ()).map_err(camera_error)?,
                receiver: decode::<Endpoint>(&[receiver], ()).map_err(camera_error)?,
                id: sequence,
                flags: Flags(flags),
                command: Command::from_wire(command_set, command_id),
                payload: payload.as_bytes().to_vec(),
            },
        })
    }

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
    fn sender(&self) -> PyResult<u8> {
        Ok(encode(&self.inner.sender).map_err(camera_error)?[0])
    }
    #[getter]
    fn receiver(&self) -> PyResult<u8> {
        Ok(encode(&self.inner.receiver).map_err(camera_error)?[0])
    }
    #[getter]
    fn sequence(&self) -> u16 {
        self.inner.id
    }
    #[getter]
    fn flags(&self) -> u8 {
        self.inner.flags.0
    }
    #[getter]
    fn command(&self) -> (u8, u8) {
        self.inner.command.wire()
    }
    #[getter]
    fn payload<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.inner.payload)
    }
}

/// Caller-supplied framing metadata; it grants no transmission permission.
#[cfg_attr(feature = "stubgen", gen_stub_pyclass)]
#[pyclass(frozen, module = "osmo_camera._native")]
pub struct MessageContext {
    pub(crate) inner: NativeContext,
}

#[cfg_attr(feature = "stubgen", gen_stub_pymethods)]
#[pymethods]
impl MessageContext {
    #[new]
    fn new(sender: u8, receiver: u8, sequence: u16, flags: u8) -> PyResult<Self> {
        Ok(Self {
            inner: NativeContext {
                sender: decode(&[sender], ()).map_err(camera_error)?,
                receiver: decode(&[receiver], ()).map_err(camera_error)?,
                id: sequence,
                flags: Flags(flags),
            },
        })
    }

    /// Frame a complete command body, including its two command-identity bytes.
    fn frame(&self, body: &Bound<'_, PyBytes>) -> PyResult<Message> {
        let body = decode::<CommandBody>(body.as_bytes(), ()).map_err(camera_error)?;
        Ok(Message {
            inner: self.inner.frame(&body).map_err(camera_error)?,
        })
    }

    #[getter]
    fn sender(&self) -> PyResult<u8> {
        Ok(encode(&self.inner.sender).map_err(camera_error)?[0])
    }
    #[getter]
    fn receiver(&self) -> PyResult<u8> {
        Ok(encode(&self.inner.receiver).map_err(camera_error)?[0])
    }
    #[getter]
    fn sequence(&self) -> u16 {
        self.inner.id
    }
    #[getter]
    fn flags(&self) -> u8 {
        self.inner.flags.0
    }
}

/// One bounded, app-originated exploration request with opaque payload bytes.
///
/// Construction sends nothing. Known-read matching and two-level write approval
/// use the native Rust policy, including its restrictions on media writes.
#[cfg_attr(feature = "stubgen", gen_stub_pyclass)]
#[pyclass(frozen, module = "osmo_camera._native")]
#[derive(Clone)]
pub struct Request {
    pub(crate) inner: ExplorationRequest,
}

#[cfg_attr(feature = "stubgen", gen_stub_pymethods)]
#[pymethods]
impl Request {
    #[new]
    #[pyo3(signature = (receiver, command_set, command_id, payload, *, flags=None, timeout=3.0))]
    fn new(
        receiver: u8,
        command_set: u8,
        command_id: u8,
        payload: &Bound<'_, PyBytes>,
        flags: Option<u8>,
        timeout: f64,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: ExplorationRequest {
                receiver: decode(&[receiver], ()).map_err(camera_error)?,
                command: Command::from_wire(command_set, command_id),
                flags: flags.map(Flags).unwrap_or(Flags::REQUEST),
                payload: payload.as_bytes().to_vec(),
                timeout: request_timeout(timeout)?,
            },
        })
    }

    pub(crate) fn frame(&self, sequence: u16) -> Message {
        Message {
            inner: self.inner.message(sequence),
        }
    }

    #[pyo3(signature = (*, allow_unclassified=false, approve_unclassified=false))]
    pub(crate) fn validate(
        &self,
        allow_unclassified: bool,
        approve_unclassified: bool,
    ) -> PyResult<()> {
        self.inner
            .validate(allow_unclassified, approve_unclassified)
            .map_err(camera_error)
    }

    #[getter]
    fn is_known_read(&self) -> bool {
        self.inner.is_known_read()
    }
    #[getter]
    fn receiver(&self) -> PyResult<u8> {
        Ok(encode(&self.inner.receiver).map_err(camera_error)?[0])
    }
    #[getter]
    fn command(&self) -> (u8, u8) {
        self.inner.command.wire()
    }
    #[getter]
    fn flags(&self) -> u8 {
        self.inner.flags.0
    }
    #[getter]
    fn timeout(&self) -> f64 {
        self.inner.timeout.as_secs_f64()
    }
    #[getter]
    fn payload<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.inner.payload)
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<Message>()?;
    module.add_class::<MessageContext>()?;
    module.add_class::<Request>()?;
    Ok(())
}
