from __future__ import annotations

import os
from typing import Optional

PathLikeStr = str | os.PathLike[str]

class CommitInfo:
    """Lightweight, read-only view of a git commit."""

    id: str
    summary: Optional[str]
    author: str
    email: Optional[str]
    time: int
    offset_minutes: int

class RewriteResult:
    """Outcome of a history rewrite."""

    old_to_new: dict[str, str]
    updated_refs: dict[str, str]
    warnings: list[str]

class Repo:
    """Thin wrapper around a git repository."""

    def head(self) -> Optional[CommitInfo]:
        """Return the current HEAD commit, or None if HEAD is unborn/detached without a commit."""
    def list_commits(self, max: Optional[int] = None) -> list[CommitInfo]:
        """Commits reachable from HEAD (newest first). Pass max to limit results."""
    def change_commit_message(self, commit_id: str, new_message: str) -> RewriteResult:
        """Amend HEAD with a new message (rewrites history). commit_id must resolve to HEAD."""
    def rewrite_author(self, commit_id: str, new_name: str, new_email: str, update_committer: bool = True) -> RewriteResult:
        """Amend HEAD with new author (and committer if update_committer). commit_id must resolve to HEAD."""
    def filter_commits(self, author: str | None = None, message_contains: str | None = None) -> RewriteResult:
        """Drop commits matching author or message substring; returns mapping of old->new ids (dropped map to parent)."""
    def remove_path(self, path_pattern: str) -> RewriteResult:
        """Purge a path (glob) from all commits reachable from HEAD; returns mapping of old->new ids."""
    def rebase_branch(self, branch: str, onto: str) -> RewriteResult:
        """Rebase a local branch onto a new base (pick-only). Returns mapping of old->new ids."""
    def squash_last(self, count: int, mode: str = "squash", message: str | None = None) -> RewriteResult:
        """Squash the last N commits into one. mode: 'squash' (keep all messages) or 'fixup' (keep oldest message)."""

def open_repo(path: PathLikeStr) -> Repo:
    """Open a git repository at path (str or os.PathLike)."""

__all__ = ["CommitInfo", "Repo", "RewriteResult", "open_repo"]
