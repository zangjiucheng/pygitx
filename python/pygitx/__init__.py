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

__all__ = [
    "CommitInfo",
    "RewriteResult",
    "Repo",
    "open_repo",
    "create_backup_ref",
    "head",
    "rev_parse",
    "reword",
    "list_commits",
    "change_commit_message",
    "rewrite_author",
    "filter_commits",
    "remove_path",
    "keep_path",
    "rebase_branch",
    "squash_last",
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


def head(repo: Repo | str) -> Optional[CommitInfo]:
    """Return HEAD commit info."""
    return _ensure_repo(repo).head()


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
