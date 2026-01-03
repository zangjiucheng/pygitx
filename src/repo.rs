use crate::errors::py_git_err;
use crate::types::PyCommitInfo;
use git2::{BranchType, Commit, ErrorClass, ErrorCode, Oid, Repository, Signature};
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
            Err(err)
                if err.code() == ErrorCode::UnbornBranch || err.code() == ErrorCode::NotFound =>
            {
                return Ok(None);
            }
            Err(err) => return Err(py_git_err("failed to read HEAD", err)),
        };

        let commit = match head.peel_to_commit() {
            Ok(commit) => commit,
            Err(err)
                if err.code() == ErrorCode::UnbornBranch || err.code() == ErrorCode::NotFound =>
            {
                return Ok(None);
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
            if matches!(err.code(), ErrorCode::UnbornBranch | ErrorCode::NotFound)
                || err.class() == ErrorClass::Reference
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
            let oid = oid_result
                .map_err(|err| py_git_err("failed to read commit id from revwalk", err))?;
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
    pub fn change_commit_message(
        &mut self,
        commit_id: &str,
        new_message: &str,
    ) -> PyResult<PyCommitInfo> {
        if new_message.trim().is_empty() {
            return Err(PyValueError::new_err("new commit message cannot be empty"));
        }

        let head_ref = match self.repo.head() {
            Ok(reference) => reference,
            Err(err)
                if err.code() == ErrorCode::UnbornBranch || err.code() == ErrorCode::NotFound =>
            {
                return Err(PyValueError::new_err(
                    "cannot amend commit: repository has no HEAD commit",
                ));
            }
            Err(err) => return Err(py_git_err("failed to read HEAD", err)),
        };

        let head_commit = match head_ref.peel_to_commit() {
            Ok(commit) => commit,
            Err(err)
                if err.code() == ErrorCode::UnbornBranch || err.code() == ErrorCode::NotFound =>
            {
                return Err(PyValueError::new_err(
                    "cannot amend commit: repository has no HEAD commit",
                ));
            }
            Err(err) => return Err(py_git_err("failed to resolve HEAD to commit", err)),
        };

        let target_obj = self.repo.revparse_single(commit_id).map_err(|err| {
            PyValueError::new_err(format!("invalid commit id '{}': {}", commit_id, err))
        })?;
        let target_commit = target_obj.peel_to_commit().map_err(|err| {
            PyValueError::new_err(format!("object '{}' is not a commit: {}", commit_id, err))
        })?;

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

    /// Rewrite the author (and optionally committer) of the HEAD commit.
    ///
    /// Args:
    ///     commit_id (str): Commit to rewrite. Must resolve to HEAD.
    ///     new_name (str): New author name.
    ///     new_email (str): New author email.
    ///     update_committer (bool): If True, also update committer to the same identity (default True).
    ///
    /// Returns:
    ///     CommitInfo: The amended commit (with a new id).
    ///
    /// Notes:
    ///     - This rewrites history; descendants will now point to a new commit.
    ///     - Only HEAD is supported; older commits require a rebase-style rewrite.
    #[pyo3(
        text_signature = "($self, commit_id, new_name, new_email, update_committer=True)",
        signature = (commit_id, new_name, new_email, update_committer = true)
    )]
    pub fn rewrite_author(
        &mut self,
        commit_id: &str,
        new_name: &str,
        new_email: &str,
        update_committer: Option<bool>,
    ) -> PyResult<PyCommitInfo> {
        if new_name.trim().is_empty() {
            return Err(PyValueError::new_err("new author name cannot be empty"));
        }
        if new_email.trim().is_empty() {
            return Err(PyValueError::new_err("new author email cannot be empty"));
        }

        let head_ref = match self.repo.head() {
            Ok(reference) => reference,
            Err(err)
                if err.code() == ErrorCode::UnbornBranch || err.code() == ErrorCode::NotFound =>
            {
                return Err(PyValueError::new_err(
                    "cannot rewrite author: repository has no HEAD commit",
                ));
            }
            Err(err) => return Err(py_git_err("failed to read HEAD", err)),
        };

        let head_commit = match head_ref.peel_to_commit() {
            Ok(commit) => commit,
            Err(err)
                if err.code() == ErrorCode::UnbornBranch || err.code() == ErrorCode::NotFound =>
            {
                return Err(PyValueError::new_err(
                    "cannot rewrite author: repository has no HEAD commit",
                ));
            }
            Err(err) => return Err(py_git_err("failed to resolve HEAD to commit", err)),
        };

        let target_obj = self.repo.revparse_single(commit_id).map_err(|err| {
            PyValueError::new_err(format!("invalid commit id '{}': {}", commit_id, err))
        })?;
        let target_commit = target_obj.peel_to_commit().map_err(|err| {
            PyValueError::new_err(format!("object '{}' is not a commit: {}", commit_id, err))
        })?;

        if target_commit.id() != head_commit.id() {
            return Err(PyValueError::new_err(
                "rewriting author is currently supported only for HEAD; rebase needed for older commits",
            ));
        }

        let author_time = target_commit.author().when();
        let new_author = Signature::new(new_name, new_email, &author_time)
            .map_err(|err| PyValueError::new_err(format!("invalid author identity: {}", err)))?;
        let committer_sig = if update_committer.unwrap_or(true) {
            Some(
                Signature::new(new_name, new_email, &target_commit.committer().when()).map_err(
                    |err| PyValueError::new_err(format!("invalid committer identity: {}", err)),
                )?,
            )
        } else {
            None
        };

        let new_oid = target_commit
            .amend(
                Some("HEAD"),
                Some(&new_author),
                committer_sig.as_ref(),
                None,
                None,
                None,
            )
            .map_err(|err| py_git_err("failed to rewrite author", err))?;

        let amended = self
            .repo
            .find_commit(new_oid)
            .map_err(|err| py_git_err("failed to load amended commit", err))?;

        Ok(PyCommitInfo::from_commit(&amended))
    }

    /// Squash the last N commits into a single commit (optionally fixup-style).
    ///
    /// Args:
    ///     count (int): Number of most recent commits to squash (must be >= 2).
    ///     mode (str | None): "squash" (default, keep all messages) or "fixup" (keep oldest message).
    ///     message (str | None): Optional explicit commit message for the squashed commit.
    ///
    /// Returns:
    ///     CommitInfo: The new squashed commit (with a new id).
    ///
    /// Notes:
    ///     - Only linear history is supported (merge commits in the range are rejected).
    ///     - Branch HEAD is updated when attached; detached HEAD is updated to the new commit.
    #[pyo3(text_signature = "($self, count, mode='squash', message=None)", signature = (count, mode = None, message = None))]
    pub fn squash_last(
        &mut self,
        count: usize,
        mode: Option<&str>,
        message: Option<&str>,
    ) -> PyResult<PyCommitInfo> {
        if count < 2 {
            return Err(PyValueError::new_err(
                "count must be at least 2 to squash commits",
            ));
        }

        let squash_mode = match mode.unwrap_or("squash") {
            "squash" => SquashMode::Squash,
            "fixup" => SquashMode::Fixup,
            other => {
                return Err(PyValueError::new_err(format!(
                    "unknown squash mode '{}'; expected 'squash' or 'fixup'",
                    other
                )));
            }
        };

        let head_ref = self.repo.head().map_err(|err| {
            PyValueError::new_err(format!("cannot squash: repository has no HEAD ({})", err))
        })?;
        let head_commit = head_ref.peel_to_commit().map_err(|err| {
            PyValueError::new_err(format!(
                "cannot squash: failed to resolve HEAD to commit ({})",
                err
            ))
        })?;

        // Walk first-parents to collect the range (newest -> oldest).
        let mut commits: Vec<Commit<'_>> = Vec::with_capacity(count);
        let mut current = head_commit;
        commits.push(current.clone());

        for _ in 1..count {
            match current.parent_count() {
                0 => {
                    return Err(PyValueError::new_err(format!(
                        "not enough commits to squash: repository has only {} commit(s)",
                        commits.len()
                    )));
                }
                1 => {}
                _ => {
                    return Err(PyValueError::new_err(
                        "squash_last only supports linear history (merge commits not supported in range)",
                    ));
                }
            }

            current = current
                .parent(0)
                .map_err(|err| py_git_err("failed to load commit parent during squash", err))?;
            commits.push(current.clone());
        }

        // Oldest commit in the range is last; its parent becomes the parent of the squashed commit.
        let oldest = commits
            .last()
            .ok_or_else(|| PyValueError::new_err("unexpected empty commit range during squash"))?;
        let parent_commit = if oldest.parent_count() == 0 {
            None
        } else {
            Some(
                oldest
                    .parent(0)
                    .map_err(|err| py_git_err("failed to load parent when building squash", err))?,
            )
        };

        let combined_message = match message {
            Some(msg) => msg.to_string(),
            None => compose_squash_message(&commits, squash_mode),
        };

        // Resulting tree is the tree of the newest commit in the range.
        let tree = commits[0]
            .tree()
            .map_err(|err| py_git_err("failed to load tree for squashed commit", err))?;

        let mut parent_refs: Vec<&Commit> = Vec::new();
        if let Some(ref parent) = parent_commit {
            parent_refs.push(parent);
        }

        let new_oid = self
            .repo
            .commit(
                None,
                &commits[0].author(),
                &commits[0].committer(),
                &combined_message,
                &tree,
                &parent_refs,
            )
            .map_err(|err| py_git_err("failed to create squashed commit", err))?;

        // Update ref/HEAD to the rewritten commit.
        match head_ref.resolve() {
            Ok(mut resolved) => {
                let name = resolved.name().map(|s| s.to_string());
                if let Some(name) = name {
                    resolved
                        .set_target(new_oid, "squash: update ref")
                        .map_err(|err| py_git_err("failed to update ref after squash", err))?;
                    self.repo
                        .set_head(&name)
                        .map_err(|err| py_git_err("failed to update HEAD after squash", err))?;
                } else {
                    self.repo.set_head_detached(new_oid).map_err(|err| {
                        py_git_err("failed to detach HEAD to squashed commit", err)
                    })?;
                }
            }
            Err(_) => {
                self.repo
                    .set_head_detached(new_oid)
                    .map_err(|err| py_git_err("failed to detach HEAD to squashed commit", err))?;
            }
        }

        let new_commit = self
            .repo
            .find_commit(new_oid)
            .map_err(|err| py_git_err("failed to load squashed commit", err))?;
        Ok(PyCommitInfo::from_commit(&new_commit))
    }

    /// Rebase a branch onto a new base (non-interactive, pick-only).
    ///
    /// Args:
    ///     branch (str): Local branch name (e.g., "feature").
    ///     onto (str): Commit-ish to rebase onto (e.g., "main" or a commit id).
    ///
    /// Returns:
    ///     list[tuple[str, str]]: Mapping of old commit ids to new commit ids in replay order.
    ///
    /// Notes:
    ///     - If conflicts occur, the rebase aborts and an error is raised.
    ///     - This rewrites history; branch ref is updated to the new tip.
    #[pyo3(text_signature = "($self, branch, onto)")]
    pub fn rebase_branch(&mut self, branch: &str, onto: &str) -> PyResult<Vec<(String, String)>> {
        let branch_ref = self
            .repo
            .find_branch(branch, BranchType::Local)
            .map_err(|err| {
                PyValueError::new_err(format!("unknown branch '{}': {}", branch, err))
            })?;
        let branch_name = branch_ref
            .get()
            .name()
            .ok_or_else(|| PyValueError::new_err("branch name is not valid utf-8"))?
            .to_string();
        let branch_tip = branch_ref
            .get()
            .target()
            .ok_or_else(|| PyValueError::new_err("branch has no target oid"))?;

        let onto_obj = self
            .repo
            .revparse_single(onto)
            .map_err(|err| PyValueError::new_err(format!("invalid onto '{}': {}", onto, err)))?;
        let onto_commit = onto_obj.peel_to_commit().map_err(|err| {
            PyValueError::new_err(format!("onto '{}' is not a commit: {}", onto, err))
        })?;

        let head_points_to_branch = self
            .repo
            .head()
            .ok()
            .and_then(|h| h.name().map(|name| name == branch_name))
            .unwrap_or(false);

        // Collect commits reachable from branch but not from onto (oldest first).
        let mut revwalk = self
            .repo
            .revwalk()
            .map_err(|err| py_git_err("failed to start revwalk for rebase", err))?;
        revwalk
            .push(branch_tip)
            .map_err(|err| py_git_err("failed to push branch tip for rebase", err))?;
        revwalk
            .hide(onto_commit.id())
            .map_err(|err| py_git_err("failed to hide onto for rebase", err))?;
        let _ = revwalk.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::REVERSE);

        let mut to_replay = Vec::new();
        for oid_result in revwalk {
            let oid = oid_result
                .map_err(|err| py_git_err("failed to read commit during rebase walk", err))?;
            to_replay.push(oid);
        }

        let mut mappings = Vec::new();
        let mut current_tip = self
            .repo
            .find_commit(onto_commit.id())
            .map_err(|err| py_git_err("failed to load onto commit", err))?;

        for oid in to_replay {
            let commit = self
                .repo
                .find_commit(oid)
                .map_err(|err| py_git_err("failed to load commit during rebase", err))?;
            let parent = commit
                .parent(0)
                .map_err(|err| py_git_err("failed to load commit parent during rebase", err))?;

            let ancestor_tree = parent
                .tree()
                .map_err(|err| py_git_err("failed to load ancestor tree", err))?;
            let current_tree = current_tip
                .tree()
                .map_err(|err| py_git_err("failed to load current tree", err))?;
            let commit_tree = commit
                .tree()
                .map_err(|err| py_git_err("failed to load commit tree", err))?;

            let mut idx = self
                .repo
                .merge_trees(&ancestor_tree, &current_tree, &commit_tree, None)
                .map_err(|err| py_git_err("failed to merge trees during rebase", err))?;
            if idx.has_conflicts() {
                return Err(py_git_err(
                    "conflicts detected during rebase; rebase aborted",
                    git2::Error::from_str("merge conflicts"),
                ));
            }

            let tree_oid = idx
                .write_tree_to(&self.repo)
                .map_err(|err| py_git_err("failed to write tree during rebase", err))?;
            let tree = self
                .repo
                .find_tree(tree_oid)
                .map_err(|err| py_git_err("failed to load rewritten tree", err))?;

            let new_oid = self
                .repo
                .commit(
                    None,
                    &commit.author(),
                    &commit.committer(),
                    commit.message().unwrap_or("rebase: pick"),
                    &tree,
                    &[&current_tip],
                )
                .map_err(|err| py_git_err("failed to create commit during rebase", err))?;

            mappings.push((commit.id().to_string(), new_oid.to_string()));
            current_tip = self
                .repo
                .find_commit(new_oid)
                .map_err(|err| py_git_err("failed to load rewritten commit", err))?;
        }

        if let Some((_, new_oid_str)) = mappings.last() {
            let new_oid = Oid::from_str(new_oid_str)
                .map_err(|_| PyValueError::new_err("invalid new oid produced"))?;
            let mut reference = branch_ref.into_reference();
            reference
                .set_target(new_oid, "rebase: update branch")
                .map_err(|err| py_git_err("failed to update branch after rebase", err))?;

            if head_points_to_branch {
                self.repo
                    .set_head(&branch_name)
                    .map_err(|err| py_git_err("failed to update HEAD after rebase", err))?;
            }
        }

        Ok(mappings)
    }
}

/// Open a git repository at the given path.
#[pyfunction]
#[pyo3(text_signature = "(path)")]
pub fn open_repo(py: Python<'_>, path: Py<PyAny>) -> PyResult<PyRepo> {
    let path_any = path.bind(py);
    let resolved_path = resolve_repo_path(py, path_any.as_any())?;
    let repo = Repository::open(Path::new(&resolved_path)).map_err(|err| match err.code() {
        ErrorCode::NotFound => PyValueError::new_err(format!(
            "no git repository found at path: {}",
            resolved_path
        )),
        _ => py_git_err("failed to open repository", err),
    })?;
    Ok(PyRepo { repo })
}

#[derive(Copy, Clone)]
enum SquashMode {
    Squash,
    Fixup,
}

fn compose_squash_message(commits: &[Commit<'_>], mode: SquashMode) -> String {
    if commits.is_empty() {
        return "squashed commit".to_string();
    }

    match mode {
        SquashMode::Squash => {
            let parts: Vec<String> = commits
                .iter()
                .rev() // oldest -> newest
                .filter_map(|c| c.message().map(str::trim).filter(|m| !m.is_empty()))
                .map(|m| m.to_string())
                .collect();

            if parts.is_empty() {
                "squashed commit".to_string()
            } else {
                parts.join("\n\n")
            }
        }
        SquashMode::Fixup => commits
            .last()
            .and_then(|c| c.summary().map(|s| s.to_string()))
            .unwrap_or_else(|| "fixup commit".to_string()),
    }
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
