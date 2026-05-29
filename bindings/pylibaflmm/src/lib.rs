use pyo3::prelude::*;

/// Setup python modules for `libaflmm_qemu`.
///
/// # Errors
/// Returns error if python libaflmm setup failed.
#[pymodule]
#[pyo3(name = "pylibaflmm")]
pub fn python_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    pyo3_log::init();

    let modules = m.py().import("sys")?.getattr("modules")?;

    #[cfg(target_os = "linux")]
    {
        let qemu_module = PyModule::new(m.py(), "qemu")?;
        libaflmm_qemu::python_module(&qemu_module)?;
        m.add_submodule(&qemu_module)?;
        modules.set_item("pylibaflmm.qemu", qemu_module)?;
    }

    let bolts_module = PyModule::new(m.py(), "libaflmm_bolts")?;
    libaflmm_bolts::pybind::python_module(&bolts_module)?;
    m.add_submodule(&bolts_module)?;
    modules.set_item("pylibaflmm.libaflmm_bolts", bolts_module)?;

    Ok(())
}
