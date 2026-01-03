use pyo3::prelude::*;
use pyo3::types::PyModule;

mod errors;
mod repo;
mod types;

pub use crate::repo::{open_repo, PyRepo};
pub use crate::types::PyCommitInfo;

#[pymodule]
fn histgit(_py: Python, m: &Bound<PyModule>) -> PyResult<()> {
    m.add_class::<PyRepo>()?;
    m.add_class::<PyCommitInfo>()?;
    m.add_function(wrap_pyfunction!(open_repo, m)?)?;
    Ok(())
}

#[cfg(all(test, feature = "python-tests"))]
mod tests;
