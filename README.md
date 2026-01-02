# histgit

Cross-platform Git history access implemented in Rust with a Python API via PyO3.

## Getting started

Prerequisites:
- Rust toolchain (for building the extension module)
- Python 3.9+ and `pip`
- `maturin` for building wheels (`pip install maturin`)

### Build locally (editable)

```bash
maturin develop
```

### Build a release wheel

Uses the `python-extension` feature (set via `pyproject.toml`):

```bash
maturin build --release
```

### Python usage

```python
import histgit

repo = histgit.open_repo(".")

head = repo.head()
print("HEAD:", head.id if head else "None")

for commit in repo.list_commits(max=5):
    print(commit.id, commit.summary)
```

### Rust tests

- Default `cargo test` runs without the extension-module feature (no Python runtime needed).
- To exercise Python-facing code paths from Rust tests, enable the `python-tests` feature:

```bash
cargo test --features python-tests
```

## Notes

- Uses `git2` with vendored libgit2 for consistent cross-platform builds.
- Exposes an ABI3 wheel (`abi3-py39`) for Python 3.9+.
- Future versions can layer in history editing features (rebasing, author rewrites, filtering) atop this foundation.
