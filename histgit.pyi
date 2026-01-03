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

class Repo:
    """Thin wrapper around a git repository."""

    def head(self) -> Optional[CommitInfo]:
        """Return the current HEAD commit, or None if HEAD is unborn/detached without a commit."""
    def list_commits(self, max: Optional[int] = None) -> list[CommitInfo]:
        """Commits reachable from HEAD (newest first). Pass max to limit results."""
    def change_commit_message(self, commit_id: str, new_message: str) -> CommitInfo:
        """Amend HEAD with a new message (rewrites history). commit_id must resolve to HEAD."""
    def rewrite_author(self, commit_id: str, new_name: str, new_email: str, update_committer: bool = True) -> CommitInfo:
        """Amend HEAD with new author (and committer if update_committer). commit_id must resolve to HEAD."""

def open_repo(path: PathLikeStr) -> Repo:
    """Open a git repository at path (str or os.PathLike)."""

__all__ = ["CommitInfo", "Repo", "open_repo"]
