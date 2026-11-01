use pyo3::prelude::*;

#[pyfunction]
fn version() -> &'static str {
    osmo_camera::version()
}

#[pymodule]
fn _native(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(version, module)?)?;
    Ok(())
}
