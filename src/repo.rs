use crate::errors::py_git_err;
use crate::types::{PyCommitInfo, RewriteResult};
use git2::{BranchType, Commit, ErrorClass, ErrorCode, ObjectType, Oid, Repository, Signature, Tree};
use globset::{Glob, GlobSet, GlobSetBuilder};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyString};
use std::collections::HashMap;
use std::path::Path;

/// Thin wrapper around git2::Repository exposed to Python.
#[pyclass(name = "Repo", unsendable)]
pub struct PyRepo {
    pub(crate) repo: Repository,
}

impl PyRepo {
    fn resolve_head_commit(&self) -> PyResult<Commit<'_>> {
        let head_ref = self.repo.head().map_err(|err| match err.code() {
            ErrorCode::UnbornBranch | ErrorCode::NotFound => {
                PyValueError::new_err("repository has no HEAD commit")
            }
            _ => py_git_err("failed to read HEAD", err),
        })?;

        head_ref
            .peel_to_commit()
            .map_err(|err| py_git_err("failed to resolve HEAD to commit", err))
    }
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

    /// Filter commits by author and/or message substring, dropping matches and rewriting history.
    ///
    /// Args:
    ///     author (str | None): Drop commits where the author name matches this string (exact).
    ///     message_contains (str | None): Drop commits whose message contains this substring.
    ///
    /// Returns:
    ///     RewriteResult: Mapping of old commit ids to new commit ids (dropped commits map to their parent).
    ///
    /// Notes:
    ///     - Currently supports linear history (merge commits are rejected).
    ///     - This rewrites history; branch ref/HEAD are updated to the rewritten tip.
    #[pyo3(
        text_signature = "($self, author=None, message_contains=None)",
        signature = (author = None, message_contains = None)
    )]
    pub fn filter_commits(
        &mut self,
        py: Python<'_>,
        author: Option<&str>,
        message_contains: Option<&str>,
    ) -> PyResult<RewriteResult> {
        py.detach(move || self.filter_commits_internal(author, message_contains))
    }

    fn filter_commits_internal(
        &mut self,
        author: Option<&str>,
        message_contains: Option<&str>,
    ) -> PyResult<RewriteResult> {
        if author.is_none() && message_contains.is_none() {
            return Err(PyValueError::new_err(
                "must provide at least one filter: author or message_contains",
            ));
        }

        let head_commit = self.resolve_head_commit()?;
        let mut revwalk = self
            .repo
            .revwalk()
            .map_err(|err| py_git_err("failed to create revwalk", err))?;
        revwalk
            .push(head_commit.id())
            .map_err(|err| py_git_err("failed to start revwalk from HEAD", err))?;
        let _ = revwalk.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::REVERSE);

        let mut mapping: HashMap<Oid, Oid> = HashMap::new();

        for oid_result in revwalk {
            let oid = oid_result
                .map_err(|err| py_git_err("failed to read commit id from revwalk", err))?;
            let commit = self
                .repo
                .find_commit(oid)
                .map_err(|err| py_git_err("failed to load commit", err))?;

            if commit.parent_count() > 1 {
                return Err(PyValueError::new_err(
                    "filter_commits currently supports linear history only (merge commit encountered)",
                ));
            }

            let mut new_parents = Vec::new();
            for i in 0..commit.parent_count() {
                let parent = commit
                    .parent(i)
                    .map_err(|err| py_git_err("failed to load parent during filter", err))?;
                let rewritten_parent = mapping
                    .get(&parent.id())
                    .ok_or_else(|| PyValueError::new_err("missing parent mapping during filter"))?;
                let parent_commit = self
                    .repo
                    .find_commit(*rewritten_parent)
                    .map_err(|err| py_git_err("failed to load rewritten parent", err))?;
                new_parents.push(parent_commit);
            }

            let author_matches = author
                .map(|a| author_matches(&commit, a))
                .unwrap_or(false);
            let message_matches = message_contains
                .map(|m| str_contains_insensitive(commit.message().unwrap_or(""), m))
                .unwrap_or(false);
            let should_drop = author_matches || message_matches;

            if should_drop {
                if new_parents.is_empty() {
                    return Err(PyValueError::new_err(
                        "cannot drop the root commit; no parent to re-parent to",
                    ));
                }
                let replacement = new_parents[0].id();
                mapping.insert(commit.id(), replacement);
                continue;
            }

            let tree = commit
                .tree()
                .map_err(|err| py_git_err("failed to load commit tree", err))?;
            let parent_refs: Vec<&Commit> = new_parents.iter().collect();
            let new_oid = self
                .repo
                .commit(
                    None,
                    &commit.author(),
                    &commit.committer(),
                    commit.message().unwrap_or(""),
                    &tree,
                    &parent_refs,
                )
                .map_err(|err| py_git_err("failed to create rewritten commit", err))?;

            mapping.insert(commit.id(), new_oid);
        }

        let new_head_oid = mapping
            .get(&head_commit.id())
            .ok_or_else(|| PyValueError::new_err("failed to resolve rewritten HEAD"))?;
        let updated_refs = update_head(&self.repo, *new_head_oid)?;

        Ok(RewriteResult::with_maps(
            mapping,
            updated_refs,
            Vec::new(),
        ))
    }

    /// Remove a path (glob) from all commits reachable from HEAD and rewrite history.
    ///
    /// Args:
    ///     path_pattern (str): Glob-style pattern relative to repo root (e.g., "secrets.env" or "config/*.yaml").
    ///
    /// Returns:
    ///     RewriteResult: Mapping of old commit ids to new commit ids (rewritten commits).
    ///
    /// Notes:
    ///     - Currently supports linear history (merge commits are rejected).
    ///     - This rewrites history; branch ref/HEAD are updated to the rewritten tip.
    #[pyo3(text_signature = "($self, path_pattern)")]
    pub fn remove_path(&mut self, py: Python<'_>, path_pattern: &str) -> PyResult<RewriteResult> {
        py.detach(move || self.remove_path_internal(path_pattern))
    }

    fn remove_path_internal(&mut self, path_pattern: &str) -> PyResult<RewriteResult> {
        let matcher = build_globset(path_pattern)?;
        let head_commit = self.resolve_head_commit()?;
        let mut revwalk = self
            .repo
            .revwalk()
            .map_err(|err| py_git_err("failed to create revwalk", err))?;
        revwalk
            .push(head_commit.id())
            .map_err(|err| py_git_err("failed to start revwalk from HEAD", err))?;
        let _ = revwalk.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::REVERSE);

        let mut mapping: HashMap<Oid, Oid> = HashMap::new();

        for oid_result in revwalk {
            let oid = oid_result
                .map_err(|err| py_git_err("failed to read commit id from revwalk", err))?;
            let commit = self
                .repo
                .find_commit(oid)
                .map_err(|err| py_git_err("failed to load commit", err))?;

            if commit.parent_count() > 1 {
                return Err(PyValueError::new_err(
                    "remove_path currently supports linear history only (merge commit encountered)",
                ));
            }

            let mut new_parents = Vec::new();
            for i in 0..commit.parent_count() {
                let parent = commit
                    .parent(i)
                    .map_err(|err| py_git_err("failed to load parent during remove_path", err))?;
                let rewritten_parent = mapping.get(&parent.id()).ok_or_else(|| {
                    PyValueError::new_err("missing parent mapping during remove_path")
                })?;
                let parent_commit = self
                    .repo
                    .find_commit(*rewritten_parent)
                    .map_err(|err| py_git_err("failed to load rewritten parent", err))?;
                new_parents.push(parent_commit);
            }

            let tree = commit
                .tree()
                .map_err(|err| py_git_err("failed to load commit tree", err))?;
            let maybe_new_tree = rewrite_tree_without_paths(&self.repo, &tree, &matcher, Path::new(""))?;
            let final_tree = match maybe_new_tree {
                Some(oid) => self
                    .repo
                    .find_tree(oid)
                    .map_err(|err| py_git_err("failed to load rewritten tree", err))?,
                None => tree,
            };

            let parent_refs: Vec<&Commit> = new_parents.iter().collect();
            let new_oid = self
                .repo
                .commit(
                    None,
                    &commit.author(),
                    &commit.committer(),
                    commit.message().unwrap_or(""),
                    &final_tree,
                    &parent_refs,
                )
                .map_err(|err| py_git_err("failed to create rewritten commit", err))?;

            mapping.insert(commit.id(), new_oid);
        }

        let new_head_oid = mapping
            .get(&head_commit.id())
            .ok_or_else(|| PyValueError::new_err("failed to resolve rewritten HEAD"))?;
        let updated_refs = update_head(&self.repo, *new_head_oid)?;

        Ok(RewriteResult::with_maps(
            mapping,
            updated_refs,
            Vec::new(),
        ))
    }

    /// Amend the message of the HEAD commit.
    ///
    /// Args:
    ///     commit_id (str): Commit to amend. Currently must resolve to HEAD.
    ///     new_message (str): Replacement commit message.
    ///
    /// Returns:
    ///     RewriteResult: Mapping of rewritten commits and updated refs (HEAD/branch).
    ///
    /// Notes:
    ///     - This rewrites history; descendants will now point to a new commit.
    ///     - Only amending HEAD is supported; older commits require rebasing.
    #[pyo3(text_signature = "($self, commit_id, new_message)")]
    pub fn change_commit_message(
        &mut self,
        commit_id: &str,
        new_message: &str,
    ) -> PyResult<RewriteResult> {
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

        let old_oid = target_commit.id();
        let new_oid = target_commit
            .amend(None, None, None, None, Some(new_message), None)
            .map_err(|err| py_git_err("failed to amend commit message", err))?;

        let updated_refs = update_head(&self.repo, new_oid)?;
        let mut result = RewriteResult::new();
        result.add_mapping(old_oid, new_oid);
        for (name, oid) in updated_refs {
            result.add_updated_ref(name, oid);
        }

        Ok(result)
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
    ///     RewriteResult: Mapping of rewritten commits and updated refs (HEAD/branch).
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
    ) -> PyResult<RewriteResult> {
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

        let old_oid = target_commit.id();
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
                None,
                Some(&new_author),
                committer_sig.as_ref(),
                None,
                None,
                None,
            )
            .map_err(|err| py_git_err("failed to rewrite author", err))?;

        let updated_refs = update_head(&self.repo, new_oid)?;
        let mut result = RewriteResult::new();
        result.add_mapping(old_oid, new_oid);
        for (name, oid) in updated_refs {
            result.add_updated_ref(name, oid);
        }

        Ok(result)
    }

    /// Squash the last N commits into a single commit (optionally fixup-style).
    ///
    /// Args:
    ///     count (int): Number of most recent commits to squash (must be >= 2).
    ///     mode (str | None): "squash" (default, keep all messages) or "fixup" (keep oldest message).
    ///     message (str | None): Optional explicit commit message for the squashed commit.
    ///
    /// Returns:
    ///     RewriteResult: Mapping of squashed commits to the new commit and updated refs.
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
    ) -> PyResult<RewriteResult> {
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

        let updated_refs = update_head(&self.repo, new_oid)?;
        let mut mapping = HashMap::new();
        for commit in &commits {
            mapping.insert(commit.id(), new_oid);
        }

        Ok(RewriteResult::with_maps(
            mapping,
            updated_refs,
            Vec::new(),
        ))
    }

    /// Rebase a branch onto a new base (non-interactive, pick-only).
    ///
    /// Args:
    ///     branch (str): Local branch name (e.g., "feature").
    ///     onto (str): Commit-ish to rebase onto (e.g., "main" or a commit id).
    ///
    /// Returns:
    ///     RewriteResult: Mapping of old commit ids to new commit ids in replay order.
    ///
    /// Notes:
    ///     - If conflicts occur, the rebase aborts and an error is raised.
    ///     - This rewrites history; branch ref is updated to the new tip.
    #[pyo3(text_signature = "($self, branch, onto)")]
    pub fn rebase_branch(&mut self, branch: &str, onto: &str) -> PyResult<RewriteResult> {
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

        let mut mapping: HashMap<Oid, Oid> = HashMap::new();
        let mut current_tip = self
            .repo
            .find_commit(onto_commit.id())
            .map_err(|err| py_git_err("failed to load onto commit", err))?;
        let mut last_new_oid: Option<Oid> = None;

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

            mapping.insert(commit.id(), new_oid);
            last_new_oid = Some(new_oid);
            current_tip = self
                .repo
                .find_commit(new_oid)
                .map_err(|err| py_git_err("failed to load rewritten commit", err))?;
        }

        let mut updated_refs = HashMap::new();
        if let Some(new_oid) = last_new_oid {
            let mut reference = branch_ref.into_reference();
            reference
                .set_target(new_oid, "rebase: update branch")
                .map_err(|err| py_git_err("failed to update branch after rebase", err))?;
            updated_refs.insert(branch_name.clone(), new_oid);

            if head_points_to_branch {
                self.repo
                    .set_head(&branch_name)
                    .map_err(|err| py_git_err("failed to update HEAD after rebase", err))?;
                updated_refs.insert("HEAD".to_string(), new_oid);
            }
        }

        Ok(RewriteResult::with_maps(
            mapping,
            updated_refs,
            Vec::new(),
        ))
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

fn update_head(repo: &Repository, new_oid: Oid) -> PyResult<HashMap<String, Oid>> {
    let mut updated_refs = HashMap::new();
    match repo.head() {
        Ok(mut head_ref) => {
            let name = head_ref.name().map(|s| s.to_string());
            if let Some(name) = name {
                head_ref
                    .set_target(new_oid, "history rewrite")
                    .map_err(|err| py_git_err("failed to update ref after rewrite", err))?;
                repo.set_head(&name)
                    .map_err(|err| py_git_err("failed to update HEAD after rewrite", err))?;
                updated_refs.insert(name, new_oid);
            } else {
                repo.set_head_detached(new_oid)
                    .map_err(|err| py_git_err("failed to detach HEAD to rewritten commit", err))?;
            }
        }
        Err(_) => {
            repo.set_head_detached(new_oid)
                .map_err(|err| py_git_err("failed to detach HEAD to rewritten commit", err))?;
        }
    }
    updated_refs.insert("HEAD".to_string(), new_oid);
    Ok(updated_refs)
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

fn build_globset(pattern: &str) -> PyResult<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    let glob = Glob::new(pattern).map_err(|err| {
        PyValueError::new_err(format!("invalid path pattern '{}': {}", pattern, err))
    })?;
    builder.add(glob);
    builder
        .build()
        .map_err(|err| PyValueError::new_err(format!("failed to build globset: {}", err)))
}

fn author_matches(commit: &Commit<'_>, filter: &str) -> bool {
    let author = commit.author();
    let filter = filter.to_lowercase();
    let name = author.name().unwrap_or_default().to_lowercase();
    let email = author.email().unwrap_or_default().to_lowercase();
    name == filter || name.contains(&filter) || email.contains(&filter)
}

fn str_contains_insensitive(haystack: &str, needle: &str) -> bool {
    let h = haystack.to_lowercase();
    let n = needle.to_lowercase();
    h.contains(&n)
}

fn rewrite_tree_without_paths(
    repo: &Repository,
    tree: &Tree<'_>,
    matcher: &GlobSet,
    base: &Path,
) -> PyResult<Option<Oid>> {
    let mut builder = repo
        .treebuilder(Some(tree))
        .map_err(|err| py_git_err("failed to create treebuilder", err))?;
    let mut changed = false;

    for entry in tree.iter() {
        let name = match entry.name() {
            Some(n) => n,
            None => continue,
        };
        let full_path = base.join(name);
        let path_str = full_path.to_string_lossy();

        if matcher.is_match(path_str.as_ref()) {
            builder
                .remove(name)
                .map_err(|err| py_git_err("failed to remove matched path", err))?;
            changed = true;
            continue;
        }

        if entry.kind() == Some(ObjectType::Tree) {
            let child_tree = repo
                .find_tree(entry.id())
                .map_err(|err| py_git_err("failed to load subtree", err))?;
            if let Some(new_child_oid) =
                rewrite_tree_without_paths(repo, &child_tree, matcher, &full_path)?
            {
                builder
                    .insert(name, new_child_oid, entry.filemode())
                    .map_err(|err| py_git_err("failed to replace subtree", err))?;
                changed = true;
            }
        }
    }

    if changed {
        let oid = builder
            .write()
            .map_err(|err| py_git_err("failed to write rewritten tree", err))?;
        Ok(Some(oid))
    } else {
        Ok(None)
    }
}
