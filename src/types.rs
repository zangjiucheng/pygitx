use git2::{Commit, Oid};
use pyo3::prelude::*;
use std::collections::HashMap;

/// Result of a history rewrite operation.
#[pyclass(name = "RewriteResult")]
#[derive(Default)]
pub struct RewriteResult {
    pub old_to_new: HashMap<Oid, Oid>,
    pub updated_refs: HashMap<String, Oid>,
    pub warnings: Vec<String>,
}

impl RewriteResult {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_maps(
        old_to_new: HashMap<Oid, Oid>,
        updated_refs: HashMap<String, Oid>,
        warnings: Vec<String>,
    ) -> Self {
        Self {
            old_to_new,
            updated_refs,
            warnings,
        }
    }

    pub fn add_mapping(&mut self, old: Oid, new: Oid) {
        self.old_to_new.insert(old, new);
    }

    pub fn add_updated_ref<S: Into<String>>(&mut self, name: S, oid: Oid) {
        self.updated_refs.insert(name.into(), oid);
    }
}

#[pymethods]
impl RewriteResult {
    #[new]
    fn py_new() -> Self {
        RewriteResult::new()
    }

    /// Mapping of original commit ids to rewritten commit ids (as hex strings).
    #[getter]
    fn old_to_new(&self) -> HashMap<String, String> {
        self.old_to_new
            .iter()
            .map(|(old, new)| (old.to_string(), new.to_string()))
            .collect()
    }

    /// Mapping of refs that were updated to point at rewritten commits.
    #[getter]
    fn updated_refs(&self) -> HashMap<String, String> {
        self.updated_refs
            .iter()
            .map(|(name, oid)| (name.clone(), oid.to_string()))
            .collect()
    }

    /// Any non-fatal warnings encountered while rewriting.
    #[getter]
    fn warnings(&self) -> Vec<String> {
        self.warnings.clone()
    }
}

/// Information about a Git commit exposed to Python.
#[pyclass(name = "CommitInfo")]
pub struct PyCommitInfo {
    /// Full commit SHA.
    #[pyo3(get)]
    pub id: String,
    /// Optional one-line summary.
    #[pyo3(get)]
    pub summary: Option<String>,
    /// Author name.
    #[pyo3(get)]
    pub author: String,
    /// Author email if present.
    #[pyo3(get)]
    pub email: Option<String>,
    /// Commit time (seconds since epoch).
    #[pyo3(get)]
    pub time: i64,
    /// Timezone offset in minutes.
    #[pyo3(get)]
    pub offset_minutes: i32,
}

impl PyCommitInfo {
    pub fn from_commit(commit: &Commit<'_>) -> Self {
        let author = commit.author();
        let time = author.when();
        PyCommitInfo {
            id: commit.id().to_string(),
            summary: commit.summary().map(|s| s.to_string()),
            author: author.name().unwrap_or_default().to_string(),
            email: author.email().map(|e| e.to_string()),
            time: time.seconds(),
            offset_minutes: time.offset_minutes(),
        }
    }
}
