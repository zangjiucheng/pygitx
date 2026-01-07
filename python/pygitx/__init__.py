"""High-level Python wrappers around the native pygitx bindings.

`Repo` methods are the canonical API. Module-level functions are thin convenience
helpers (a few remain) or deprecated shims for backward compatibility.
"""

from __future__ import annotations

import os
import sys
from importlib import import_module
from typing import Literal, Optional

_native = import_module("pygitx._native")

# Re-export native classes for direct use.
CommitInfo = _native.CommitInfo
RewriteResult = _native.RewriteResult
Repo = _native.Repo
open_repo = _native.open_repo
RepoSummary = _native.RepoSummary
DiffStat = _native.DiffStat

# Canonical public API
__all__ = [
    "CommitInfo",
    "RewriteResult",
    "Repo",
    "open_repo",
    "RepoSummary",
    "DiffStat",
    "summary",
    "refs_tui",
    "log_tui",
    "log_graph",
]


# Validation helpers -------------------------------------------------------
def _nonempty(name: str, value: str) -> str:
    if not value.strip():
        raise ValueError(f"{name} cannot be empty")
    return value


def _positive_int(name: str, value: int) -> int:
    if value <= 0:
        raise ValueError(f"{name} must be positive")
    return value


def _ensure_repo(repo: Repo | str | os.PathLike[str]) -> Repo:
    if isinstance(repo, Repo):
        return repo
    return open_repo(os.fspath(repo))


def _color_bool(color: bool | Literal["auto"]) -> bool:
    if color == "auto":
        try:
            return sys.stdout.isatty()
        except Exception:
            return False
    return bool(color)


# Convenience wrappers -----------------------------------------------------
def summary(repo: Repo | str | os.PathLike[str]) -> RepoSummary:
    """Convenience: Repo.summary()."""
    return _ensure_repo(repo).summary()


def refs_tui(
    repo: Repo | str | os.PathLike[str],
    local: bool = True,
    remote: bool = False,
    tags: bool = True,
    max_width: int | None = None,
    color: bool | Literal["auto"] = "auto",
) -> str:
    """Render branches/tags in a jj-style table (NAME/OID/AGE/AUTHOR/SUMMARY)."""
    if not (local or remote or tags):
        raise ValueError("at least one of local, remote, or tags must be True")
    return _ensure_repo(repo).render_refs(
        local, remote, tags, max_width, _color_bool(color)
    )


def log_tui(
    repo: Repo | str | os.PathLike[str],
    rev: str = "HEAD",
    max_commits: int = 200,
    decorate: bool = True,
    graph: bool = True,
    max_width: int | None = None,
    color: bool | Literal["auto"] = "auto",
) -> str:
    """Render a jj/git-style oneline log with optional graph/decorations."""
    return _ensure_repo(repo).render_log(
        _nonempty("rev", rev),
        _positive_int("max_commits", max_commits),
        decorate,
        graph,
        max_width,
        _color_bool(color),
    )


def log_graph(
    repo: Repo | str | os.PathLike[str],
    refs: list[str] | None = None,
    max_commits: int = 400,
    decorate: bool = True,
    max_width: int | None = None,
    color: bool | Literal["auto"] = "auto",
) -> str:
    """Render a multi-branch graph starting from refs (or all local branches)."""
    return _ensure_repo(repo).log_graph(
        refs,
        _positive_int("max_commits", max_commits),
        decorate,
        max_width,
        _color_bool(color),
    )


# Alias methods on Repo for parity with the convenience helpers.
def _repo_refs_tui(
    self: Repo,
    local: bool = True,
    remote: bool = False,
    tags: bool = True,
    max_width: int | None = None,
    color: bool | Literal["auto"] = "auto",
) -> str:
    return self.render_refs(local, remote, tags, max_width, _color_bool(color))


def _repo_log_tui(
    self: Repo,
    rev: str = "HEAD",
    max_commits: int = 200,
    decorate: bool = True,
    graph: bool = True,
    max_width: int | None = None,
    color: bool | Literal["auto"] = "auto",
) -> str:
    return self.render_log(
        _nonempty("rev", rev),
        _positive_int("max_commits", max_commits),
        decorate,
        graph,
        max_width,
        _color_bool(color),
    )


def _repo_log_graph(
    self: Repo,
    refs: list[str] | None = None,
    max_commits: int = 400,
    decorate: bool = True,
    max_width: int | None = None,
    color: bool | Literal["auto"] = "auto",
) -> str:
    return self.render_log_graph(
        refs,
        _positive_int("max_commits", max_commits),
        decorate,
        max_width,
        _color_bool(color),
    )


Repo.refs_tui = _repo_refs_tui  # type: ignore[attr-defined]
Repo.log_tui = _repo_log_tui  # type: ignore[attr-defined]
Repo.log_graph = _repo_log_graph  # type: ignore[attr-defined]
