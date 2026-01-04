# PyGitX

[![pypi](https://img.shields.io/pypi/v/pygitx.svg)](https://pypi.org/project/pygitx/)
[![support-version](https://img.shields.io/pypi/pyversions/pygitx)](https://img.shields.io/pypi/pyversions/pygitx)
[![license](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![build](https://img.shields.io/github/actions/workflow/status/zangjiucheng/pygitx/publish-to-pypi.yml?label=build)](https://github.com/zangjiucheng/pygitx/actions)
[![commit](https://img.shields.io/github/last-commit/zangjiucheng/pygitx.svg)](https://github.com/jiucheng-xie/pygitx/commits/main)



Cross-platform Git history access implemented in Rust with a Python API via PyO3 and libgit2.

## What you get
- `pygitx.open_repo(path) -> Repo` to access repositories.
- `Repo.head()` for the current HEAD commit (or `None` if unborn).
- `Repo.list_commits(max=None)` to iterate commits from HEAD (newest first).
- `Repo.change_commit_message(commit_id, new_message)` to amend the HEAD commit message.
- `Repo.rewrite_author(commit_id, new_name, new_email, update_committer=True)` to rewrite HEAD author (optionally committer).
- `Repo.rebase_branch(branch, onto)` to replay a branch onto a new base (pick-only).
- `Repo.squash_last(count, mode="squash", message=None)` to squash the latest commits (or fixup-style).
- `Repo.filter_commits(author=None, message_contains=None)` to drop commits matching simple criteria (case-insensitive author/email/message).
- `Repo.remove_path(path_pattern)` to purge a path (glob) from all commits.
- Rewrite operations return a `RewriteResult` with `old_to_new` commit ids, `updated_refs` (e.g., HEAD/branch), and any `warnings`.
- Vendored libgit2 for predictable, cross-platform builds.
- Python package `pygitx` wraps the native extension at `pygitx._native` and handles lightweight validation/convenience in Python; heavy history work stays in Rust/libgit2.

## Quick start (editable install)
```bash
python -m venv .venv
source .venv/bin/activate
pip install --upgrade pip setuptools wheel maturin
pip install -e .
```

Or use the Makefile helpers:
```bash
make venv
source .venv/bin/activate
make install
make develop   # pip install -e .
```

## Build a wheel
```bash
make release   # pip wheel . -w dist
```

## Python usage
```python
import pygitx
from pathlib import Path

repo = pygitx.open_repo(Path("."))
head = repo.head()
print("HEAD:", head.id if head else "None")

for c in repo.list_commits(max=5):
    print(f"{c.id[:7]} {c.author} <{c.email}> {c.summary}")

# Amending the latest commit message (rewrites history)
if head:
    result = repo.change_commit_message(head.id, "new message for HEAD")
    print("Amended HEAD mapping:", result.old_to_new)
    print("Updated refs:", result.updated_refs)

# Rewriting author/committer on HEAD (also rewrites history)
if head:
    result = repo.rewrite_author(head.id, "New Name", "new@example.com")
    print("Author rewrite mapping:", result.old_to_new)

# Squash the last 3 commits (keep all messages); use mode="fixup" to keep only the oldest message
if head:
    squashed = repo.squash_last(3)
    print("Squashed mapping:", squashed.old_to_new)

# Rebase a branch onto a new base (pick-only)
updated_commits = repo.rebase_branch("feature", "main")
print("Rebased commits:", updated_commits.old_to_new)

# Drop commits by author or message substring
filtered = repo.filter_commits(author="bad actor", message_contains="wip")
print("Filtered mapping:", filtered.old_to_new)

# Remove a path across history (glob)
purged = repo.remove_path("secrets/*.txt")
print("Purged path mapping:", purged.old_to_new)
```

Example script: `examples/demo.py` (see `--help` for options to generate a demo repo, list commits, amend/rewrite, or rebase).

## Development
- Default tests (no Python runtime needed): `cargo test`
- Python-facing tests: `cargo test --features python-tests`
- Build docs (HTML): `source .venv/bin/activate && make docs` (installs `docs/requirements.txt` via uv, then runs sphinx)
- Make targets: `make venv`, `make install`, `make develop`, `make release`, `make docs`, `make test`, `make clean`, `make cleancargo`, `make cleanvenv`

## Notes / future
- ABI3 wheel targeting Python 3.9+ (`abi3-py39`).
- Built on `git2` (libgit2) for speed and portability.
- History filtering/removal currently supports linear histories; merge commits are rejected.
- Filtering/search is case-insensitive for author/email/message; globbing is used for path removal.
- Amending commit messages rewrites history; avoid on published branches unless you know the consequences.
- Rewriting author/committer also rewrites history and is currently limited to HEAD; rewriting older commits requires a rebase-like flow.
- Rebase is non-interactive (pick-only) and rewrites branch history; conflicts will abort with an error.
