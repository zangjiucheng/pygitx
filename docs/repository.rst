Repository basics
=================

Open and inspect a repository.

open_repo
---------
``pygitx.open_repo(path: str | os.PathLike[str]) -> Repo``

Open a git repository at ``path``. Accepts strings or ``pathlib.Path`` and expands ``~``.

.. code-block:: python

   import pygitx
   repo = pygitx.open_repo("~/.config/git")

head
----
``Repo.head() -> CommitInfo | None``

Return the current HEAD commit info, or ``None`` if HEAD is unborn/detached without a commit.

list_commits
------------
``Repo.list_commits(max: int | None = None) -> list[CommitInfo]``

Commits reachable from HEAD (newest first). Pass ``max`` to limit results.

.. code-block:: python

   for c in repo.list_commits(max=5):
       print(f"{c.id[:7]} {c.summary}")

list_branches
-------------
``Repo.list_branches(local: bool = True, remote: bool = False) -> list[str]``

List branch names. Control inclusion with ``local``/``remote`` flags.

.. code-block:: python

   print(repo.list_branches(local=True, remote=False))

list_tags
---------
``Repo.list_tags() -> list[str]``

List tag names (lightweight or annotated).

current_branch
--------------
``Repo.current_branch() -> str | None``

Return the current branch name, or ``None`` if detached/unborn.

summary
-------
``Repo.summary() -> RepoSummary``

Return a summary object with key repo stats. ``__str__``/``__repr__`` on ``Repo`` display this summary.
