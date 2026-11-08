//! Thin native protocol and explicitly authorized session bindings.

use pyo3::create_exception;
use pyo3::{exceptions::PyRuntimeError, prelude::*};
#[cfg(feature = "stubgen")]
use pyo3_stub_gen::{
    ModuleRef, PyStubType, TypeInfo, define_stub_info_gatherer, derive::gen_stub_pyfunction,
    impl_py_runtime_type, inventory::submit, type_info::PyClassInfo,
};
#[cfg(feature = "stubgen")]
use std::any::TypeId;
use unofficial_osmo_camera::Error;

mod capabilities;
mod raw;
mod session;
mod typed;

macro_rules! camera_exception {
    ($name:ident, $base:ty) => {
        create_exception!(osmo_camera._native, $name, $base);
        #[cfg(feature = "stubgen")]
        impl PyStubType for $name {
            fn type_output() -> TypeInfo {
                TypeInfo::with_module(
                    stringify!($name),
                    ModuleRef::Named("osmo_camera._native".into()),
                )
            }
        }
        #[cfg(feature = "stubgen")]
        impl_py_runtime_type!($name);
        #[cfg(feature = "stubgen")]
        submit! {
            PyClassInfo {
                struct_id: TypeId::of::<$name>,
                pyclass_name: stringify!($name),
                module: Some("osmo_camera._native"),
                bases: &[<$base as PyStubType>::type_output],
                doc: "", getters: &[], setters: &[],
                has_eq: false, has_ord: false, has_hash: false, has_str: false,
                subclass: true,
            }
        }
    };
}

camera_exception!(CameraError, PyRuntimeError);
camera_exception!(ConfigurationError, CameraError);
camera_exception!(ProtocolError, CameraError);
camera_exception!(UnsupportedError, CameraError);
camera_exception!(CancelledError, CameraError);
camera_exception!(CameraTimeoutError, CameraError);
camera_exception!(CameraRejectedError, CameraError);
camera_exception!(OwnershipError, CameraError);

#[cfg(feature = "stubgen")]
define_stub_info_gatherer!(stub_info);

fn camera_error(error: Error) -> PyErr {
    let message = error.to_string();
    match error {
        Error::InvalidConfig { .. } | Error::LimitExceeded(_) => {
            ConfigurationError::new_err(message)
        }
        Error::Protocol(_) | Error::WireCodec { .. } | Error::InvalidReply { .. } => {
            ProtocolError::new_err(message)
        }
        Error::Unsupported(_) | Error::UnsupportedValue { .. } => {
            UnsupportedError::new_err(message)
        }
        Error::Cancelled => CancelledError::new_err(message),
        Error::Timeout { .. } | Error::ReplyTimeout { .. } => CameraTimeoutError::new_err(message),
        Error::CameraRejected { .. } => CameraRejectedError::new_err(message),
        Error::Ownership { .. } => OwnershipError::new_err(message),
        _ => CameraError::new_err(message),
    }
}

#[cfg_attr(feature = "stubgen", gen_stub_pyfunction)]
#[pyfunction]
fn version() -> &'static str {
    unofficial_osmo_camera::version()
}

#[pymodule]
fn _native(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(version, module)?)?;
    raw::register(module)?;
    typed::register(module)?;
    capabilities::register(module)?;
    session::register(module)?;
    module.add("CameraError", module.py().get_type::<CameraError>())?;
    module.add(
        "ConfigurationError",
        module.py().get_type::<ConfigurationError>(),
    )?;
    module.add("ProtocolError", module.py().get_type::<ProtocolError>())?;
    module.add(
        "UnsupportedError",
        module.py().get_type::<UnsupportedError>(),
    )?;
    module.add("CancelledError", module.py().get_type::<CancelledError>())?;
    module.add(
        "CameraTimeoutError",
        module.py().get_type::<CameraTimeoutError>(),
    )?;
    module.add(
        "CameraRejectedError",
        module.py().get_type::<CameraRejectedError>(),
    )?;
    module.add("OwnershipError", module.py().get_type::<OwnershipError>())?;
    Ok(())
}
