use crate::errors::py_git_err;
use crate::types::{PyCommitInfo, PyDiffStat, PyRepoSummary, RewriteResult};
use chrono::{DateTime, Utc};
use git2::{BranchType, Commit, ErrorClass, ErrorCode, Oid, Repository};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyAny;
use std::collections::HashSet;
use std::path::Path;

mod backup;
mod rewrite;
mod summary;
mod tree_ops;
mod util;
mod diff;
mod render;

use backup::{collect_head_refs, create_backup_refs};
use diff::diff_stat;
use rewrite::{
    change_commit_message, filter_commits, keep_path, rebase_branch, remove_path, reword_commit,
    rewrite_author, squash_last_commits,
};
use summary::summarize_repo;
use render::{render_log, render_refs};
use util::resolve_repo_path;

/// Thin wrapper around git2::Repository exposed to Python.
#[pyclass(name = "Repo", unsendable)]
pub struct PyRepo {
    pub(crate) repo: Repository,
}

impl PyRepo {
    pub(super) fn resolve_head_commit(&self) -> PyResult<Commit<'_>> {
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

    pub(super) fn resolve_spec_oid(&self, spec: &str) -> PyResult<Oid> {
        self.repo
            .revparse_single(spec)
            .map(|obj| obj.id())
            .map_err(|err| {
                PyValueError::new_err(format!("invalid revision spec '{}': {}", spec, err))
            })
    }
}

#[pymethods]
impl PyRepo {
    fn __repr__(&self) -> PyResult<String> {
        let summary = self.repo_summary_internal()?;
        Ok(format!(
            "Repo(path='{}', branch={}, head={})",
            summary.path,
            summary.branch.as_deref().unwrap_or("None"),
            summary.head.as_deref().unwrap_or("None")
        ))
    }

    fn __str__(&self) -> PyResult<String> {
        let summary = self.repo_summary_internal()?;
        let branch = summary.branch.as_deref().unwrap_or("None");
        let head = summary.head.as_deref().unwrap_or("None");
        let last_ts = summary
            .last_commit_time
            .and_then(|t| {
                DateTime::<Utc>::from_timestamp(t, 0)
                    .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
            })
            .unwrap_or_else(|| "N/A".to_string());
        Ok(format!(
            "Repo(\n  path: {}\n  branch: {}\n  head: {}\n  commits: {}\n  authors: {}\n  branches: {}  tags: {}  remotes: {}\n  files: {}\n  size: {} KB\n  dirty: {}\n  last commit: {}\n)",
            summary.path,
            branch,
            head,
            summary.commits,
            summary.authors,
            summary.branches,
            summary.tags,
            summary.remotes,
            summary.files,
            summary.size_kb,
            summary.is_dirty,
            last_ts
        ))
    }

    /// Create backup refs for the current HEAD and referenced branch (if any).
    ///
    /// Args:
    ///     prefix (str | None): Base prefix for backups (default: "refs/pygitx/backup").
    ///
    /// Returns:
    ///     str: Backup root reference (e.g., "refs/pygitx/backup/<timestamp>").
    #[pyo3(text_signature = "($self, prefix=None)", signature = (prefix = None))]
    pub fn create_backup_ref(&self, prefix: Option<&str>) -> PyResult<String> {
        let refs = collect_head_refs(&self.repo)?;
        create_backup_refs(&self.repo, &refs, prefix)
    }

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

    /// List branches.
    ///
    /// Args:
    ///     local (bool): Include local branches.
    ///     remote (bool): Include remote branches.
    ///
    /// Returns:
    ///     list[str]: Branch names.
    #[pyo3(text_signature = "($self, local=True, remote=False)", signature = (local = true, remote = false))]
    pub fn list_branches(&self, local: bool, remote: bool) -> PyResult<Vec<String>> {
        if !local && !remote {
            return Err(PyValueError::new_err(
                "must request at least one of local or remote branches",
            ));
        }
        let mut names = HashSet::new();
        if local {
            let branches = self
                .repo
                .branches(Some(BranchType::Local))
                .map_err(|err| py_git_err("failed to list local branches", err))?;
            for branch in branches {
                let (branch, _) = branch.map_err(|err| py_git_err("failed to read branch", err))?;
                if let Some(name) = branch
                    .name()
                    .map_err(|err| py_git_err("failed to read branch name", err))?
                {
                    names.insert(name.to_string());
                }
            }
        }
        if remote {
            let branches = self
                .repo
                .branches(Some(BranchType::Remote))
                .map_err(|err| py_git_err("failed to list remote branches", err))?;
            for branch in branches {
                let (branch, _) = branch.map_err(|err| py_git_err("failed to read branch", err))?;
                if let Some(name) = branch
                    .name()
                    .map_err(|err| py_git_err("failed to read remote branch name", err))?
                {
                    names.insert(name.to_string());
                }
            }
        }
        let mut out: Vec<String> = names.into_iter().collect();
        out.sort();
        Ok(out)
    }

    /// List tag names.
    ///
    /// Returns:
    ///     list[str]: Tag names.
    #[pyo3(text_signature = "($self)")]
    pub fn list_tags(&self) -> PyResult<Vec<String>> {
        let names = self
            .repo
            .tag_names(None)
            .map_err(|err| py_git_err("failed to list tags", err))?;
        let mut out = Vec::new();
        for i in 0..names.len() {
            if let Some(name) = names.get(i) {
                out.push(name.to_string());
            }
        }
        out.sort();
        Ok(out)
    }

    /// Return the current branch name, or None if detached/unborn.
    #[pyo3(text_signature = "($self)")]
    pub fn current_branch(&self) -> PyResult<Option<String>> {
        let head = match self.repo.head() {
            Ok(h) => h,
            Err(err)
                if err.code() == ErrorCode::UnbornBranch || err.code() == ErrorCode::NotFound =>
            {
                return Ok(None);
            }
            Err(err) => return Err(py_git_err("failed to read HEAD", err)),
        };
        if !head.is_branch() {
            return Ok(None);
        }
        Ok(head.shorthand().map(|s| s.to_string()))
    }

    /// Compute merge base between two revisions.
    #[pyo3(text_signature = "($self, a_spec, b_spec)")]
    pub fn merge_base(&self, a_spec: &str, b_spec: &str) -> PyResult<Option<String>> {
        let a = self.resolve_spec_oid(a_spec)?;
        let b = self.resolve_spec_oid(b_spec)?;
        match self.repo.merge_base(a, b) {
            Ok(oid) => Ok(Some(oid.to_string())),
            Err(err) if err.code() == ErrorCode::NotFound => Ok(None),
            Err(err) => Err(py_git_err("failed to compute merge base", err)),
        }
    }

    /// Return true if a_spec is ancestor of b_spec.
    #[pyo3(text_signature = "($self, a_spec, b_spec)")]
    pub fn is_ancestor(&self, a_spec: &str, b_spec: &str) -> PyResult<bool> {
        let a = self.resolve_spec_oid(a_spec)?;
        let b = self.resolve_spec_oid(b_spec)?;
        self.repo
            .graph_descendant_of(b, a)
            .map_err(|err| py_git_err("failed to check ancestry", err))
    }

    /// Return (ahead, behind) counts comparing two revisions.
    #[pyo3(text_signature = "($self, a_spec, b_spec)")]
    pub fn ahead_behind(&self, a_spec: &str, b_spec: &str) -> PyResult<(usize, usize)> {
        let a = self.resolve_spec_oid(a_spec)?;
        let b = self.resolve_spec_oid(b_spec)?;
        self.repo
            .graph_ahead_behind(a, b)
            .map_err(|err| py_git_err("failed to compute ahead/behind", err))
    }

    /// Diff stats between two commits (optionally limited to paths).
    #[pyo3(text_signature = "($self, a_spec, b_spec, paths=None)", signature = (a_spec, b_spec, paths = None))]
    pub fn diff_stat(
        &self,
        a_spec: &str,
        b_spec: &str,
        paths: Option<Vec<String>>,
    ) -> PyResult<PyDiffStat> {
        let a = self.resolve_spec_oid(a_spec)?;
        let b = self.resolve_spec_oid(b_spec)?;
        diff_stat(&self.repo, a, b, paths)
    }

    /// Render a TUI-style ref list (branches/tags) similar to jj bookmark list.
    #[pyo3(
        text_signature = "($self, local=True, remote=False, tags=True, max_width=None)",
        signature = (local = true, remote = false, tags = true, max_width = None)
    )]
    pub fn render_refs(
        &self,
        local: bool,
        remote: bool,
        tags: bool,
        max_width: Option<usize>,
    ) -> PyResult<String> {
        render_refs(&self.repo, local, remote, tags, max_width)
    }

    /// Render a TUI-style commit log (graph/decorate) similar to jj log / git log --graph.
    #[pyo3(
        text_signature = "($self, rev, max_commits=200, decorate=True, graph=True, max_width=None)",
        signature = (rev, max_commits = 200, decorate = true, graph = true, max_width = None)
    )]
    pub fn render_log(
        &self,
        rev: &str,
        max_commits: usize,
        decorate: bool,
        graph: bool,
        max_width: Option<usize>,
    ) -> PyResult<String> {
        let start = self.resolve_spec_oid(rev)?;
        render_log(&self.repo, start, max_commits, decorate, graph, max_width)
    }

    /// Resolve a revision spec to an object id (hex).
    ///
    /// Args:
    ///     spec (str): Revision string (e.g., "HEAD", "HEAD~1", "main", "v1.0", full or short oid).
    ///
    /// Returns:
    ///     str: Hexadecimal object id for the resolved spec.
    #[pyo3(text_signature = "($self, spec)")]
    pub fn rev_parse(&self, spec: &str) -> PyResult<String> {
        let obj = self.repo.revparse_single(spec).map_err(|err| {
            PyValueError::new_err(format!("invalid revision spec '{}': {}", spec, err))
        })?;
        Ok(obj.id().to_string())
    }

    /// Reword an arbitrary commit on the current branch (linear histories only).
    ///
    /// Args:
    ///     commit_id (str): Target commit to reword (must be on the current branch).
    ///     new_message (str): Replacement commit message.
    ///
    /// Returns:
    ///     RewriteResult: Mapping of old commit ids to rewritten ids and updated refs.
    ///
    /// Notes:
    ///     - Only supports linear history along the current HEAD's first-parent chain.
    ///     - Fails if the target commit is not reachable from HEAD via first parents.
    #[pyo3(text_signature = "($self, commit_id, new_message)")]
    pub fn reword(&mut self, commit_id: &str, new_message: &str) -> PyResult<RewriteResult> {
        reword_commit(self, commit_id, new_message)
    }

    /// Return a summary of the repository (similar to onefetch).
    ///
    /// Returns:
    ///     RepoSummary: Structured summary (path, head, branch, counts, size, dirty flag, last commit time).
    #[pyo3(text_signature = "($self)")]
    pub fn repo_summary(&self) -> PyResult<PyRepoSummary> {
        self.repo_summary_internal()
    }

    /// Alias for repo_summary.
    #[pyo3(text_signature = "($self)")]
    pub fn summary(&self) -> PyResult<PyRepoSummary> {
        self.repo_summary_internal()
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
        py.detach(move || filter_commits(self, author, message_contains))
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
        py.detach(move || remove_path(self, path_pattern))
    }

    /// Keep only paths matching the glob across history (drop everything else) and rewrite commits.
    ///
    /// Args:
    ///     glob_pattern (str): Glob-style pattern relative to repo root (e.g., "src/**" or "docs/*.md").
    ///
    /// Returns:
    ///     RewriteResult: Mapping of rewritten commits and updated refs (HEAD/branch).
    ///
    /// Notes:
    ///     - Currently supports linear history (merge commits emit an error).
    ///     - This rewrites history; branch ref/HEAD are updated to the rewritten tip.
    #[pyo3(text_signature = "($self, glob_pattern)")]
    pub fn keep_path(&mut self, py: Python<'_>, glob_pattern: &str) -> PyResult<RewriteResult> {
        py.detach(move || keep_path(self, glob_pattern))
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
        change_commit_message(self, commit_id, new_message)
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
        rewrite_author(self, commit_id, new_name, new_email, update_committer)
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
        squash_last_commits(self, count, mode, message)
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
        rebase_branch(self, branch, onto)
    }

    fn repo_summary_internal(&self) -> PyResult<PyRepoSummary> {
        summarize_repo(&self.repo)
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
