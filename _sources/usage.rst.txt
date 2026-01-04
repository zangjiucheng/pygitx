Usage
=====

Quick start
-----------

Create a venv and install in editable mode:

.. code-block:: bash

   python -m venv .venv
   source .venv/bin/activate
   pip install --upgrade pip setuptools wheel maturin
   pip install -e .

Or use the Makefile helpers:

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
       print("Amended message mapping:", updated.old_to_new)
       print("Updated refs:", updated.updated_refs)

       updated = repo.rewrite_author(head.id, "New Name", "new@example.com")
       print("Amended author mapping:", updated.old_to_new)

**Warning:** These operations rewrite history. Avoid using them on published
branches unless you are prepared to force-push and coordinate with consumers.

Squashing commits
-----------------

Collapse the latest commits into one. Use ``mode="squash"`` (default) to concatenate messages, or
``mode="fixup"`` to keep only the oldest message.

.. code-block:: python

   squashed = repo.squash_last(3, mode="squash")
   print("Squashed mapping:", squashed.old_to_new)

Rebasing a branch
-----------------

Replay a branch on top of a new base (pick-only):

.. code-block:: python

   updated_commits = repo.rebase_branch("feature", "main")
   for old, new in updated_commits.old_to_new.items():
       print(f"{old[:7]} -> {new[:7]}")

Conflicts will abort the rebase and raise an error; resolve manually and retry
if needed.

Filtering history
-----------------

Drop commits by author or message substring:

.. code-block:: python

   mapping = repo.filter_commits(author="bad actor", message_contains="wip")
   print(mapping.old_to_new)

Remove a path (glob) from all commits:

.. code-block:: python

   purged = repo.remove_path("secrets/*.txt")
   print(purged.old_to_new)

Notes:
- Filtering/removal currently supports linear history (merge commits are rejected).
- Author/email/message matching is case-insensitive; path matching uses globs.

Example script
--------------

`examples/demo.py` bundles the above: generate a repo, list commits, amend messages, rewrite author, and rebase (run with ``--help``).
