Usage
=====

Quick start
-----------

Install the extension (editable) with `uv`:

.. code-block:: bash

   uv venv
   source .venv/bin/activate
   uv pip install -U maturin
   uv run maturin develop --features python-extension

Or use the Makefile helpers (requires `uv`):

.. code-block:: bash

   make venv
   source .venv/bin/activate
   make install
   make develop

Open a repository and read commits (or use ``examples/demo.py generate`` to create a sample repo):

.. code-block:: python

   import pygitx
   from pathlib import Path

   repo = pygitx.open_repo(Path("."))
   head = repo.head()
   print("HEAD:", head.id if head else "None")

   for c in repo.list_commits(max=5):
       email = c.email or ""
       summary = c.summary or ""
       print(f"{c.id[:7]} {c.author} <{email}> {summary}")

History rewriting (HEAD-only)
-----------------------------

Both `change_commit_message` and `rewrite_author` operate on the HEAD commit and
replace it with a new commit:

.. code-block:: python

   if head:
       updated = repo.change_commit_message(head.id, "new message for HEAD")
       print("Amended message:", updated.id, updated.summary)

       updated = repo.rewrite_author(head.id, "New Name", "new@example.com")
       print("Amended author:", updated.id, updated.author, updated.email)

**Warning:** These operations rewrite history. Avoid using them on published
branches unless you are prepared to force-push and coordinate with consumers.

Squashing commits
-----------------

Collapse the latest commits into one. Use ``mode="squash"`` (default) to concatenate messages, or
``mode="fixup"`` to keep only the oldest message.

.. code-block:: python

   squashed = repo.squash_last(3, mode="squash")
   print("Squashed:", squashed.id, squashed.summary)

Rebasing a branch
-----------------

Replay a branch on top of a new base (pick-only):

.. code-block:: python

   updated_commits = repo.rebase_branch("feature", "main")
   for old, new in updated_commits:
       print(f"{old[:7]} -> {new[:7]}")

Conflicts will abort the rebase and raise an error; resolve manually and retry
if needed.

Example script
--------------

`examples/demo.py` bundles the above: generate a repo, list commits, amend messages, rewrite author, and rebase (run with ``--help``).
