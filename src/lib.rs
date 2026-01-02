use git2::{Commit, ErrorCode, Repository};
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyModule;
use std::path::Path;

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
    fn from_commit(commit: &Commit<'_>) -> Self {
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

/// Thin wrapper around git2::Repository exposed to Python.
#[pyclass(name = "Repo", unsendable)]
pub struct PyRepo {
    repo: Repository,
}

#[pymethods]
impl PyRepo {
    /// Return the current HEAD commit information, or None if HEAD is unborn/detached without a commit.
    #[pyo3(text_signature = "($self)")]
    pub fn head(&self) -> PyResult<Option<PyCommitInfo>> {
        let head = match self.repo.head() {
            Ok(reference) => reference,
            Err(err) if err.code() == ErrorCode::UnbornBranch || err.code() == ErrorCode::NotFound => {
                return Ok(None)
            }
            Err(err) => return Err(py_git_err("failed to read HEAD", err)),
        };

        let commit = match head.peel_to_commit() {
            Ok(commit) => commit,
            Err(err) if err.code() == ErrorCode::UnbornBranch || err.code() == ErrorCode::NotFound => {
                return Ok(None)
            }
            Err(err) => return Err(py_git_err("failed to resolve HEAD to commit", err)),
        };

        Ok(Some(PyCommitInfo::from_commit(&commit)))
    }

    /// List commits reachable from HEAD (most recent first).
    ///
    /// Args:
    ///     max (int | None): Maximum number of commits to return (default all).
    #[pyo3(text_signature = "($self, max=None)")]
    pub fn list_commits(&self, max: Option<usize>) -> PyResult<Vec<PyCommitInfo>> {
        let mut revwalk = self
            .repo
            .revwalk()
            .map_err(|err| py_git_err("failed to create revwalk", err))?;
        if let Err(err) = revwalk.push_head() {
            if err.code() == ErrorCode::UnbornBranch || err.code() == ErrorCode::NotFound {
                return Ok(vec![]);
            }
            return Err(py_git_err("failed to start revwalk from HEAD", err));
        }

        let limit = max.unwrap_or(usize::MAX);
        let mut commits = Vec::new();

        for oid_result in revwalk {
            if commits.len() >= limit {
                break;
            }
            let oid = oid_result.map_err(|err| py_git_err("failed to read commit id from revwalk", err))?;
            let commit = self
                .repo
                .find_commit(oid)
                .map_err(|err| py_git_err("failed to load commit", err))?;
            commits.push(PyCommitInfo::from_commit(&commit));
        }

        Ok(commits)
    }
}

/// Open a git repository at the given path.
#[pyfunction]
#[pyo3(text_signature = "(path)")]
pub fn open_repo(path: &str) -> PyResult<PyRepo> {
    let repo = Repository::open(Path::new(path)).map_err(|err| match err.code() {
        ErrorCode::NotFound => PyValueError::new_err(format!("no git repository found at path: {}", path)),
        _ => py_git_err("failed to open repository", err),
    })?;
    Ok(PyRepo { repo })
}

#[pymodule]
fn histgit(_py: Python, m: &Bound<PyModule>) -> PyResult<()> {
    m.add_class::<PyRepo>()?;
    m.add_class::<PyCommitInfo>()?;
    m.add_function(wrap_pyfunction!(open_repo, m)?)?;
    Ok(())
}

fn py_git_err(context: &str, err: git2::Error) -> PyErr {
    PyRuntimeError::new_err(format!("{}: {}", context, err))
}

#[cfg(all(test, feature = "python-tests"))]
mod tests {
    use super::*;
    use git2::Signature;
    use std::fs;
    use std::path::Path;
    use tempfile::tempdir;

    #[test]
    fn empty_repo_has_no_head_or_commits() {
        let dir = tempdir().unwrap();
        let repo = Repository::init(dir.path()).unwrap();
        let py_repo = PyRepo { repo };

        assert!(py_repo.head().unwrap().is_none());
        assert!(py_repo.list_commits(None).unwrap().is_empty());
    }

    #[test]
    fn repo_with_commit_reports_head() {
        let dir = tempdir().unwrap();
        let repo = Repository::init(dir.path()).unwrap();
        create_commit(&repo);
        let py_repo = PyRepo { repo };

        let head = py_repo.head().unwrap();
        assert!(head.is_some());
        let commits = py_repo.list_commits(Some(10)).unwrap();
        assert_eq!(commits.len(), 1);
        assert_eq!(head.unwrap().id, commits[0].id);
    }

    fn create_commit(repo: &Repository) {
        let sig = Signature::now("HistGit", "histgit@example.com").unwrap();
        let workdir = repo.workdir().expect("repo should have workdir");
        let file_path = workdir.join("file.txt");
        fs::write(&file_path, "hello").unwrap();

        let mut index = repo.index().unwrap();
        index.add_path(Path::new("file.txt")).unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();

        repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            "initial commit",
            &tree,
            &[],
        )
        .unwrap();
    }
}
