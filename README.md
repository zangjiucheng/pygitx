# histgit

Cross-platform Git history access implemented in Rust with a Python API via PyO3 and libgit2.

## What you get
- `histgit.open_repo(path) -> Repo` to access repositories.
- `Repo.head()` for the current HEAD commit (or `None` if unborn).
- `Repo.list_commits(max=None)` to iterate commits from HEAD (newest first).
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

repo = histgit.open_repo(".")
head = repo.head()
print("HEAD:", head.id if head else "None")

for c in repo.list_commits(max=5):
    print(f"{c.id[:7]} {c.author} <{c.email}> {c.summary}")
```

Example script: `examples/list_commits.py`.

## Development
- Default tests (no Python runtime needed): `cargo test`
- Python-facing tests: `cargo test --features python-tests`

## Notes / future
- ABI3 wheel targeting Python 3.9+ (`abi3-py39`).
- Built on `git2` (libgit2) for speed and portability.
- Future roadmap: commit message edits, author rewrites, squash/rebase helpers, filtering.
