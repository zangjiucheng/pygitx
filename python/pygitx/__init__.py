"""High-level Python wrappers around the native pygitx bindings.

The native extension is kept minimal and focused on libgit2 operations;
this module provides user-facing helpers and re-exports.
"""

from __future__ import annotations

from importlib import import_module
from typing import Optional

_native = import_module("pygitx._native")

# Re-export native classes for direct use.
CommitInfo = _native.CommitInfo
RewriteResult = _native.RewriteResult
Repo = _native.Repo
open_repo = _native.open_repo
RepoSummary = _native.RepoSummary
DiffStat = _native.DiffStat

__all__ = [
    "CommitInfo",
    "RewriteResult",
    "Repo",
    "open_repo",
    "RepoSummary",
    "DiffStat",
    "create_backup_ref",
    "summary",
    "head",
    "rev_parse",
    "reword",
    "list_branches",
    "list_tags",
    "current_branch",
    "list_commits",
    "change_commit_message",
    "rewrite_author",
    "filter_commits",
    "remove_path",
    "keep_path",
    "rebase_branch",
    "squash_last",
    "merge_base",
    "is_ancestor",
    "ahead_behind",
    "diff_stat",
    "refs_tui",
    "log_tui",
]


def _ensure_repo(repo: Repo | str) -> Repo:
    if isinstance(repo, Repo):
        return repo
    return open_repo(repo)


def create_backup_ref(repo: Repo | str, prefix: str | None = None) -> str:
    """Create backup refs for HEAD (and its branch, if attached)."""
    return _ensure_repo(repo).create_backup_ref(prefix)


def change_commit_message(repo: Repo | str, commit_id: str, new_message: str) -> RewriteResult:
    """Amend HEAD with a new message (rewrites history)."""
    repo_obj = _ensure_repo(repo)
    if not new_message or not new_message.strip():
        raise ValueError("new_message cannot be empty")
    return repo_obj.change_commit_message(commit_id, new_message)


def rewrite_author(
    repo: Repo | str,
    commit_id: str,
    new_name: str,
    new_email: str,
    update_committer: bool = True,
) -> RewriteResult:
    """Rewrite HEAD author (and optionally committer)."""
    repo_obj = _ensure_repo(repo)
    if not new_name.strip():
        raise ValueError("new_name cannot be empty")
    if not new_email.strip():
        raise ValueError("new_email cannot be empty")
    return repo_obj.rewrite_author(commit_id, new_name, new_email, update_committer)


def list_commits(repo: Repo | str, max: Optional[int] = None) -> list[CommitInfo]:
    """Convenience wrapper around Repo.list_commits."""
    return _ensure_repo(repo).list_commits(max)


def list_branches(repo: Repo | str, local: bool = True, remote: bool = False) -> list[str]:
    """List branch names (local/remote controlled by flags)."""
    return _ensure_repo(repo).list_branches(local, remote)


def list_tags(repo: Repo | str) -> list[str]:
    """List tag names."""
    return _ensure_repo(repo).list_tags()


def head(repo: Repo | str) -> Optional[CommitInfo]:
    """Return HEAD commit info."""
    return _ensure_repo(repo).head()


def current_branch(repo: Repo | str) -> Optional[str]:
    """Return the current branch name, or None if detached/unborn."""
    return _ensure_repo(repo).current_branch()


def rev_parse(repo: Repo | str, spec: str) -> str:
    """Resolve a revision spec (HEAD, branch, tag, or oid) to a hex object id."""
    if not spec.strip():
        raise ValueError("spec cannot be empty")
    return _ensure_repo(repo).rev_parse(spec)


def reword(repo: Repo | str, commit_id: str, new_message: str) -> RewriteResult:
    """Reword an arbitrary commit on the current branch (linear history only)."""
    if not new_message.strip():
        raise ValueError("new_message cannot be empty")
    return _ensure_repo(repo).reword(commit_id, new_message)


def summary(repo: Repo | str) -> RepoSummary:
    """Return a summary of the repository (path, branch, head, counts, size, dirty flag)."""
    return _ensure_repo(repo).summary()

def merge_base(repo: Repo | str, a_spec: str, b_spec: str) -> str | None:
    """Return merge base (hex oid) between two revisions, or None if none exists."""
    if not a_spec.strip():
        raise ValueError("a_spec cannot be empty")
    if not b_spec.strip():
        raise ValueError("b_spec cannot be empty")
    return _ensure_repo(repo).merge_base(a_spec, b_spec)

def is_ancestor(repo: Repo | str, a_spec: str, b_spec: str) -> bool:
    """Return True if a_spec is ancestor of b_spec."""
    if not a_spec.strip():
        raise ValueError("a_spec cannot be empty")
    if not b_spec.strip():
        raise ValueError("b_spec cannot be empty")
    return _ensure_repo(repo).is_ancestor(a_spec, b_spec)

def ahead_behind(repo: Repo | str, a_spec: str, b_spec: str) -> tuple[int, int]:
    """Return (ahead, behind) counts comparing a_spec to b_spec."""
    if not a_spec.strip():
        raise ValueError("a_spec cannot be empty")
    if not b_spec.strip():
        raise ValueError("b_spec cannot be empty")
    return _ensure_repo(repo).ahead_behind(a_spec, b_spec)


def diff_stat(
    repo: Repo | str, a_spec: str, b_spec: str, paths: list[str] | None = None
) -> DiffStat:
    """Return diff stats (files/insertions/deletions/paths) between two revisions."""
    if not a_spec.strip():
        raise ValueError("a_spec cannot be empty")
    if not b_spec.strip():
        raise ValueError("b_spec cannot be empty")
    return _ensure_repo(repo).diff_stat(a_spec, b_spec, paths)


def refs_tui(
    repo: Repo | str,
    local: bool = True,
    remote: bool = False,
    tags: bool = True,
    max_width: int | None = None,
) -> str:
    """Render branches/tags in a jj-style table."""
    if not (local or remote or tags):
        raise ValueError("at least one of local, remote, or tags must be True")
    return _ensure_repo(repo).render_refs(local, remote, tags, max_width)


def log_tui(
    repo: Repo | str,
    rev: str = "HEAD",
    max_commits: int = 200,
    decorate: bool = True,
    graph: bool = True,
    max_width: int | None = None,
) -> str:
    """Render a jj/git-style oneline log with optional graph/decorations."""
    if not rev.strip():
        raise ValueError("rev cannot be empty")
    if max_commits <= 0:
        raise ValueError("max_commits must be positive")
    return _ensure_repo(repo).render_log(rev, max_commits, decorate, graph, max_width)


def filter_commits(
    repo: Repo | str,
    author: str | None = None,
    message_contains: str | None = None,
) -> RewriteResult:
    """Drop commits by author/message substring; returns mapping of old->new ids."""
    repo_obj = _ensure_repo(repo)
    if author is None and message_contains is None:
        raise ValueError("provide at least one filter: author or message_contains")
    return repo_obj.filter_commits(author=author, message_contains=message_contains)


def remove_path(repo: Repo | str, path_pattern: str) -> RewriteResult:
    """Purge a path (glob) from all commits reachable from HEAD."""
    repo_obj = _ensure_repo(repo)
    if not path_pattern:
        raise ValueError("path_pattern cannot be empty")
    return repo_obj.remove_path(path_pattern)


def keep_path(repo: Repo | str, glob_pattern: str) -> RewriteResult:
    """Keep only paths matching the glob across history (linear history only)."""
    repo_obj = _ensure_repo(repo)
    if not glob_pattern:
        raise ValueError("glob_pattern cannot be empty")
    return repo_obj.keep_path(glob_pattern)


def rebase_branch(repo: Repo | str, branch: str, onto: str) -> RewriteResult:
    """Rebase a branch onto a new base (pick-only)."""
    if not branch.strip():
        raise ValueError("branch cannot be empty")
    if not onto.strip():
        raise ValueError("onto cannot be empty")
    return _ensure_repo(repo).rebase_branch(branch, onto)


def squash_last(
    repo: Repo | str,
    count: int,
    mode: str = "squash",
    message: str | None = None,
) -> RewriteResult:
    """Squash the last N commits into one."""
    if count < 2:
        raise ValueError("count must be at least 2")
    if mode not in {"squash", "fixup"}:
        raise ValueError("mode must be 'squash' or 'fixup'")
    return _ensure_repo(repo).squash_last(count, mode=mode, message=message)
