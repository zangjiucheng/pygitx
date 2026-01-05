PyGitX
======

Python bindings for Git history access, powered by Rust and libgit2.

What is it?
-----------
- Fast, cross-platform Git operations via libgit2.
- Python wrapper around a small Rust core; heavy lifting stays in Rust.
- Tools for reading commits, resolving revisions, and rewriting history (amend, rewrite author, squash, rebase, filter, remove paths).

Installation
------------
- PyPI: ``pip install pygitx``
- Dev/editable: see ``README`` for venv/maturin steps.

Get started
-----------
- Try the demo: ``python examples/demo.py --help`` to generate a sample repo, list commits, or exercise rewriting commands.
- Build docs locally: ``make docs`` (HTML at ``docs/_build/html``).

Contents:

.. toctree::
   :maxdepth: 1
   :caption: Navigation

   installation
   usage
   repository
   revision_parsing
   history_rewrite
   backups
   api
