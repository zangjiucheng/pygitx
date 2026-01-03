use git2::Commit;
use pyo3::prelude::*;

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
