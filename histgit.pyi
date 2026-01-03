from __future__ import annotations

import os
from typing import Optional

PathLikeStr = str | os.PathLike[str]

class CommitInfo:
    id: str
    summary: Optional[str]
    author: str
    email: Optional[str]
    time: int
    offset_minutes: int

class Repo:
    def head(self) -> Optional[CommitInfo]: ...
    def list_commits(self, max: Optional[int] = None) -> list[CommitInfo]: ...
    def change_commit_message(self, commit_id: str, new_message: str) -> CommitInfo: ...

def open_repo(path: PathLikeStr) -> Repo: ...

__all__ = ["CommitInfo", "Repo", "open_repo"]
