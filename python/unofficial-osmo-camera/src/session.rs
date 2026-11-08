//! Explicit LAN control ownership, cancellation and teardown through native Rust.

use crate::{
    camera_error,
    raw::{Message, Request, request_timeout},
    typed::{CapabilitySubscribe, CapabilitySubscriptionReply},
};
use pyo3::{exceptions::PyBaseException, prelude::*, types::PyType};
#[cfg(feature = "stubgen")]
use pyo3_stub_gen::derive::{gen_stub_pyclass, gen_stub_pymethods};
use std::path::PathBuf;
use unofficial_osmo_camera::{
    CameraConfig, CameraProfile, ControlSession as NativeSession, Error, PreviewCancellation,
};

/// A cancellation token that may be signaled from another Python thread.
#[cfg_attr(feature = "stubgen", gen_stub_pyclass)]
#[pyclass(frozen, module = "osmo_camera._native")]
pub struct Cancellation {
    inner: PreviewCancellation,
}

#[cfg_attr(feature = "stubgen", gen_stub_pymethods)]
#[pymethods]
impl Cancellation {
    #[new]
    fn new() -> Self {
        Self {
            inner: PreviewCancellation::default(),
        }
    }
    fn cancel(&self) {
        self.inner.cancel();
    }
    #[getter]
    fn is_cancelled(&self) -> bool {
        self.inner.is_cancelled()
    }
}

/// One native LAN owner. Connecting pairs/registers but does not start preview.
///
/// LAN-only by default; explicitly approved BLE-first preparation is optional.
/// There is no automatic reconnect or subscription.
#[cfg_attr(feature = "stubgen", gen_stub_pyclass)]
#[pyclass(module = "osmo_camera._native")]
pub struct ControlSession {
    inner: Option<NativeSession>,
    allow_unclassified: bool,
}

#[cfg_attr(feature = "stubgen", gen_stub_pymethods)]
#[pymethods]
impl ControlSession {
    #[staticmethod]
    #[pyo3(signature = (profile_path, *, approved=false, allow_unclassified=false, cancellation=None, ble_first=false, approve_ble=false))]
    fn connect(
        py: Python<'_>,
        profile_path: PathBuf,
        approved: bool,
        allow_unclassified: bool,
        cancellation: Option<&Cancellation>,
        ble_first: bool,
        approve_ble: bool,
    ) -> PyResult<Self> {
        if !approved {
            return Err(camera_error(Error::InvalidConfig {
                field: "camera registration",
                reason: "connecting pairs/registers the app and requires explicit approval",
            }));
        }
        if ble_first != approve_ble {
            return Err(camera_error(Error::InvalidConfig {
                field: "BLE-first startup",
                reason: "ble_first and approve_ble must both be explicitly enabled",
            }));
        }
        let cancellation = cancellation
            .map(|token| token.inner.clone())
            .unwrap_or_default();
        let inner = py
            .detach(move || {
                if cancellation.is_cancelled() {
                    return Err(Error::Cancelled);
                }
                let profile = CameraProfile::from_file(profile_path)
                    .map_err(|source| Error::io("read camera profile", source))?;
                if ble_first {
                    return profile.connect_control_via_ble(approve_ble, &cancellation);
                }
                let config = CameraConfig {
                    host: profile.camera_ip.ok_or(Error::InvalidConfig {
                        field: "camera profile",
                        reason: "LAN control requires a verified camera_ip",
                    })?,
                    pairing_identity: profile.pairing_identity.ok_or(Error::InvalidConfig {
                        field: "camera profile",
                        reason: "LAN control requires pairing_identity",
                    })?,
                    allow_writes: approved,
                };
                NativeSession::connect(config, &cancellation)
            })
            .map_err(camera_error)?;
        Ok(Self {
            inner: Some(inner),
            allow_unclassified,
        })
    }

    #[pyo3(signature = (request, *, approve_unclassified=false, timeout=3.0))]
    fn subscribe(
        &mut self,
        py: Python<'_>,
        request: &CapabilitySubscribe,
        approve_unclassified: bool,
        timeout: f64,
    ) -> PyResult<CapabilitySubscriptionReply> {
        let outgoing = request.to_raw(timeout)?;
        let reply = self.request_raw(py, &outgoing, approve_unclassified)?;
        request.decode_reply(&reply)
    }

    /// Exchange a bounded exploration request through this same native owner.
    #[pyo3(signature = (request, *, approve_unclassified=false))]
    fn request_raw(
        &mut self,
        py: Python<'_>,
        request: &Request,
        approve_unclassified: bool,
    ) -> PyResult<Message> {
        let request = request.inner.clone();
        request
            .validate(self.allow_unclassified, approve_unclassified)
            .map_err(camera_error)?;
        let allow_unclassified = self.allow_unclassified;
        let session = self
            .inner
            .as_mut()
            .ok_or_else(|| camera_error(Error::InvalidState("control session is closed")))?;
        let reply = py
            .detach(|| session.request_raw(request, allow_unclassified, approve_unclassified))
            .map_err(camera_error)?;
        Ok(Message { inner: reply })
    }

    #[pyo3(signature = (*, timeout=1.0))]
    fn receive(&mut self, py: Python<'_>, timeout: f64) -> PyResult<Option<Message>> {
        let timeout = request_timeout(timeout)?;
        let session = self
            .inner
            .as_mut()
            .ok_or_else(|| camera_error(Error::InvalidState("control session is closed")))?;
        Ok(py
            .detach(|| session.receive(timeout))
            .map_err(camera_error)?
            .map(|inner| Message { inner }))
    }

    #[pyo3(signature = (message, *, approved=false))]
    fn acknowledge_capability_notification(
        &mut self,
        py: Python<'_>,
        message: &Message,
        approved: bool,
    ) -> PyResult<()> {
        let session = self
            .inner
            .as_mut()
            .ok_or_else(|| camera_error(Error::InvalidState("control session is closed")))?;
        py.detach(|| session.acknowledge_capability_notification(&message.inner, approved))
            .map_err(camera_error)
    }

    /// Idempotent explicit teardown. Native cleanup failures remain errors.
    fn close(&mut self, py: Python<'_>) -> PyResult<()> {
        if let Some(mut session) = self.inner.take() {
            py.detach(|| session.close()).map_err(camera_error)?;
        }
        Ok(())
    }

    fn __enter__(slf: PyRef<'_, Self>) -> PyResult<PyRef<'_, Self>> {
        if slf.inner.is_none() {
            return Err(camera_error(Error::InvalidState(
                "control session is closed",
            )));
        }
        Ok(slf)
    }

    fn __exit__(
        &mut self,
        py: Python<'_>,
        _exception_type: Option<Bound<'_, PyType>>,
        _exception: Option<Bound<'_, PyBaseException>>,
        _traceback: Option<Bound<'_, PyAny>>,
    ) -> PyResult<bool> {
        self.close(py)?;
        Ok(false)
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<Cancellation>()?;
    module.add_class::<ControlSession>()?;
    Ok(())
}
