use pyo3::exceptions::PyRuntimeError;
use pyo3::PyErr;

pub fn py_git_err(context: &str, err: git2::Error) -> PyErr {
    PyRuntimeError::new_err(format!("{}: {}", context, err))
}
