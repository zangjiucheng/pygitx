<img src=".github/icon.png" alt="PyGitX icon" width="60%" />

[![pypi](https://img.shields.io/pypi/v/pygitx.svg)](https://pypi.org/project/pygitx/)
[![coverage](https://img.shields.io/codecov/c/github/zangjiucheng/pygitx)](https://codecov.io/gh/zangjiucheng/pygitx)
[![support-version](https://img.shields.io/pypi/pyversions/pygitx)](https://img.shields.io/pypi/pyversions/pygitx)
[![license](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![build](https://img.shields.io/github/actions/workflow/status/zangjiucheng/pygitx/publish-to-pypi.yml?label=build)](https://github.com/zangjiucheng/pygitx/actions)
[![commit](https://img.shields.io/github/last-commit/zangjiucheng/pygitx.svg)](https://github.com/jiucheng-xie/pygitx/commits/main)

Cross-platform Git history access implemented in Rust with a Python API via PyO3 and libgit2.

## Highlights
- Fast Git history access via libgit2 with a lightweight Python wrapper.
- Repo helpers for HEAD info, revision parsing, commit listing, and branch/tag lookup.
- History rewriting tools: amend messages, rewrite author/committer, reword arbitrary commits, squash, rebase (pick-only), drop commits, and purge paths.
- All rewrites return a `RewriteResult` with mappings and ref updates.

## Install
```bash
pip install pygitx
```

## Python usage
```python
import pygitx
from pathlib import Path

repo = pygitx.open_repo(Path("."))
head = repo.head()
print("HEAD:", head.id if head else "None")

# Resolve a revision (branch/tag/oid) to a hex id
print("main ->", repo.rev_parse("main"))

for c in repo.list_commits(max=5):
    print(f"{c.id[:7]} {c.author} <{c.email}> {c.summary}")

# Quick repo summary (also printed by str(repo))
print(repo.summary())

# Graph helpers
print("Branches:", repo.list_branches(local=True, remote=False))
print("Tags:", repo.list_tags())
print("Current branch:", repo.current_branch())
print("Merge base of main and feature:", repo.merge_base("main", "feature"))
print("Ahead/behind main vs feature:", repo.ahead_behind("main", "feature"))
```

Example script: `examples/demo.py` (run with `--help` to see options).

## Documentation
Full API and usage docs live in `docs/` (Sphinx). Build locally with:
```bash
make docs
```
Or browse the published docs if available in your environment.

## Development
- Run Rust and Python checks: `make test`
- Pytest for Python wrappers: `pytest`
- Make targets: `make venv`, `make install`, `make develop`, `make release`, `make docs`, `make test`
- Optional pre-commit prompt: set `git config core.hooksPath .githooks` and `chmod +x .githooks/pre-commit` to be prompted to run `make develop`, `make test`, and `pytest` before each commit.
