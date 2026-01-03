use crate::errors::py_git_err;
use crate::types::PyCommitInfo;
use git2::{ErrorClass, ErrorCode, Repository};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyString;
use std::path::Path;

/// Thin wrapper around git2::Repository exposed to Python.
#[pyclass(name = "Repo", unsendable)]
pub struct PyRepo {
    pub(crate) repo: Repository,
}

#[pymethods]
impl PyRepo {
    /// Return the current HEAD commit information, or None if HEAD is unborn/detached without a commit.
    #[pyo3(text_signature = "($self)")]
    pub fn head(&self) -> PyResult<Option<PyCommitInfo>> {
        let head = match self.repo.head() {
            Ok(reference) => reference,
            Err(err) if err.code() == ErrorCode::UnbornBranch || err.code() == ErrorCode::NotFound => return Ok(None),
            Err(err) => return Err(py_git_err("failed to read HEAD", err)),
        };

        let commit = match head.peel_to_commit() {
            Ok(commit) => commit,
            Err(err) if err.code() == ErrorCode::UnbornBranch || err.code() == ErrorCode::NotFound => return Ok(None),
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
            if matches!(err.code(), ErrorCode::UnbornBranch | ErrorCode::NotFound) || err.class() == ErrorClass::Reference
            {
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

    /// Amend the message of the HEAD commit.
    ///
    /// Args:
    ///     commit_id (str): Commit to amend. Currently must resolve to HEAD.
    ///     new_message (str): Replacement commit message.
    ///
    /// Returns:
    ///     CommitInfo: The amended commit (with a new id).
    ///
    /// Notes:
    ///     - This rewrites history; descendants will now point to a new commit.
    ///     - Only amending HEAD is supported; older commits require rebasing.
    #[pyo3(text_signature = "($self, commit_id, new_message)")]
    pub fn change_commit_message(&mut self, commit_id: &str, new_message: &str) -> PyResult<PyCommitInfo> {
        if new_message.trim().is_empty() {
            return Err(PyValueError::new_err("new commit message cannot be empty"));
        }

        let head_ref = match self.repo.head() {
            Ok(reference) => reference,
            Err(err) if err.code() == ErrorCode::UnbornBranch || err.code() == ErrorCode::NotFound => {
                return Err(PyValueError::new_err("cannot amend commit: repository has no HEAD commit"));
            }
            Err(err) => return Err(py_git_err("failed to read HEAD", err)),
        };

        let head_commit = match head_ref.peel_to_commit() {
            Ok(commit) => commit,
            Err(err) if err.code() == ErrorCode::UnbornBranch || err.code() == ErrorCode::NotFound => {
                return Err(PyValueError::new_err("cannot amend commit: repository has no HEAD commit"));
            }
            Err(err) => return Err(py_git_err("failed to resolve HEAD to commit", err)),
        };

        let target_obj = self
            .repo
            .revparse_single(commit_id)
            .map_err(|err| PyValueError::new_err(format!("invalid commit id '{}': {}", commit_id, err)))?;
        let target_commit = target_obj
            .peel_to_commit()
            .map_err(|err| PyValueError::new_err(format!("object '{}' is not a commit: {}", commit_id, err)))?;

        if target_commit.id() != head_commit.id() {
            return Err(PyValueError::new_err(
                "changing commit messages is currently supported only for HEAD; rebase needed for older commits",
            ));
        }

        let new_oid = target_commit
            .amend(Some("HEAD"), None, None, None, Some(new_message), None)
            .map_err(|err| py_git_err("failed to amend commit message", err))?;
        let amended = self
            .repo
            .find_commit(new_oid)
            .map_err(|err| py_git_err("failed to load amended commit", err))?;

        Ok(PyCommitInfo::from_commit(&amended))
    }
}

/// Open a git repository at the given path.
#[pyfunction]
#[pyo3(text_signature = "(path)")]
pub fn open_repo(py: Python<'_>, path: Py<PyAny>) -> PyResult<PyRepo> {
    let path_any = path.bind(py);
    let resolved_path = resolve_repo_path(py, path_any.as_any())?;
    let repo = Repository::open(Path::new(&resolved_path)).map_err(|err| match err.code() {
        ErrorCode::NotFound => PyValueError::new_err(format!("no git repository found at path: {}", resolved_path)),
        _ => py_git_err("failed to open repository", err),
    })?;
    Ok(PyRepo { repo })
}

fn resolve_repo_path(py: Python<'_>, path: &Bound<'_, PyAny>) -> PyResult<String> {
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
