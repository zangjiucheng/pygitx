# histgit

Cross-platform Git history access implemented in Rust with a Python API via PyO3 and libgit2.

## What you get
- `histgit.open_repo(path) -> Repo` to access repositories.
- `Repo.head()` for the current HEAD commit (or `None` if unborn).
- `Repo.list_commits(max=None)` to iterate commits from HEAD (newest first).
- `Repo.change_commit_message(commit_id, new_message)` to amend the HEAD commit message.
- `Repo.rewrite_author(commit_id, new_name, new_email, update_committer=True)` to rewrite HEAD author (optionally committer).
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
import histgit
from pathlib import Path

repo = histgit.open_repo(Path("."))
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
```

Example script: `examples/list_commits.py`.

## Development
- Default tests (no Python runtime needed): `cargo test`
- Python-facing tests: `cargo test --features python-tests`
- Build docs (HTML): `python -m venv .venv && source .venv/bin/activate && pip install sphinx sphinx_rtd_theme && sphinx-build -b html docs docs/_build/html`

## Notes / future
- ABI3 wheel targeting Python 3.9+ (`abi3-py39`).
- Built on `git2` (libgit2) for speed and portability.
- Amending commit messages rewrites history; avoid on published branches unless you know the consequences.
- Rewriting author/committer also rewrites history and is currently limited to HEAD; rewriting older commits requires a rebase-like flow.
- Future roadmap: commit message edits, author rewrites, squash/rebase helpers, filtering.
