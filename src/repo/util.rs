use git2::Commit;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyString};

pub(super) fn resolve_repo_path(py: Python<'_>, path: &Bound<'_, PyAny>) -> PyResult<String> {
    // Accept strings or os.PathLike objects (e.g., pathlib.Path) and normalize via os.fspath.
    let os = py.import("os")?;
    let fs_path = os
        .call_method1("fspath", (path,))
        .map_err(|_| PyValueError::new_err("path must be a string or os.PathLike"))?;
    let expanded = os
        .getattr("path")?
        .call_method1("expanduser", (fs_path,))
        .map_err(|_| PyValueError::new_err("path must be a string or os.PathLike"))?;
    let path_str: Bound<'_, PyString> = expanded
        .cast_into()
        .map_err(|_| PyValueError::new_err("path must resolve to a string"))?;
    Ok(path_str.to_cow()?.into_owned())
}

pub(super) fn author_matches(commit: &Commit<'_>, filter: &str) -> bool {
    let author = commit.author();
    let filter = filter.to_lowercase();
    let name = author.name().unwrap_or_default().to_lowercase();
    let email = author.email().unwrap_or_default().to_lowercase();
    name == filter || name.contains(&filter) || email.contains(&filter)
}

pub(super) fn str_contains_insensitive(haystack: &str, needle: &str) -> bool {
    let h = haystack.to_lowercase();
    let n = needle.to_lowercase();
    h.contains(&n)
}
