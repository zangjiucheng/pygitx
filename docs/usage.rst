Usage
=====

Quick start
-----------

Install the extension (editable):

.. code-block:: bash

   python -m venv .venv
   source .venv/bin/activate
   pip install -U pip maturin
   maturin develop --features python-extension

Open a repository and read commits:

.. code-block:: python

   import histgit
   from pathlib import Path

   repo = histgit.open_repo(Path("."))
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
