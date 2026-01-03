# pygitx

Cross-platform Git history access implemented in Rust with a Python API via PyO3 and libgit2.

## What you get
- `pygitx.open_repo(path) -> Repo` to access repositories.
- `Repo.head()` for the current HEAD commit (or `None` if unborn).
- `Repo.list_commits(max=None)` to iterate commits from HEAD (newest first).
- `Repo.change_commit_message(commit_id, new_message)` to amend the HEAD commit message.
- `Repo.rewrite_author(commit_id, new_name, new_email, update_committer=True)` to rewrite HEAD author (optionally committer).
- `Repo.rebase_branch(branch, onto)` to replay a branch onto a new base (pick-only).
- `Repo.squash_last(count, mode="squash", message=None)` to squash the latest commits (or fixup-style).
- Vendored libgit2 for predictable, cross-platform builds.

## Quick start (editable install)
```bash
python3 -m venv .venv
source .venv/bin/activate
python -m pip install -U pip maturin
maturin develop
```

## Build a wheel
Uses the `python-extension` feature via `pyproject.toml`:
```bash
maturin build --release
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
    updated = repo.change_commit_message(head.id, "new message for HEAD")
    print("Amended HEAD:", updated.id, updated.summary)

# Rewriting author/committer on HEAD (also rewrites history)
if head:
    updated = repo.rewrite_author(head.id, "New Name", "new@example.com")
    print("Amended author:", updated.id, updated.author, updated.email)

# Squash the last 3 commits (keep all messages); use mode="fixup" to keep only the oldest message
if head:
    squashed = repo.squash_last(3)
    print("Squashed tip:", squashed.id, squashed.summary)

# Rebase a branch onto a new base (pick-only)
updated_commits = repo.rebase_branch("feature", "main")
print("Rebased commits:", updated_commits)
```

Example script: `examples/demo.py` (see `--help` for options to generate a demo repo, list commits, amend/rewrite, or rebase).

## Development
- Default tests (no Python runtime needed): `cargo test`
- Python-facing tests: `cargo test --features python-tests`
- Build docs (HTML): `python -m venv .venv && source .venv/bin/activate && pip install sphinx sphinx_rtd_theme && sphinx-build -b html docs docs/_build/html`

## Notes / future
- ABI3 wheel targeting Python 3.9+ (`abi3-py39`).
- Built on `git2` (libgit2) for speed and portability.
- Amending commit messages rewrites history; avoid on published branches unless you know the consequences.
- Rewriting author/committer also rewrites history and is currently limited to HEAD; rewriting older commits requires a rebase-like flow.
- Rebase is non-interactive (pick-only) and rewrites branch history; conflicts will abort with an error.
- Future roadmap: commit message edits, author rewrites, squash/rebase helpers, filtering.
