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

Graph helpers
-------------
``Repo.merge_base(a_spec: str, b_spec: str) -> str | None``

Compute merge base between two revisions (hex oid or None if none).

``Repo.is_ancestor(a_spec: str, b_spec: str) -> bool``

Return True if ``a_spec`` is an ancestor of ``b_spec``.

``Repo.ahead_behind(a_spec: str, b_spec: str) -> tuple[int, int]``

Return (ahead, behind) counts comparing ``a_spec`` to ``b_spec``.

diff_stat
---------
``Repo.diff_stat(a_spec: str, b_spec: str, paths: list[str] | None = None) -> DiffStat``

Return diff statistics between two revisions. ``paths`` can limit the diff to specific files. ``DiffStat`` prints nicely with ``str()``/``repr()`` to show counts and touched paths.

.. code-block:: python

   stats = repo.diff_stat("HEAD~1", "HEAD")
   print(stats.files_changed, stats.insertions, stats.deletions)

refs_tui
--------
``pygitx.refs_tui(repo, local=True, remote=False, tags=True, max_width=None) -> str``

Render branches/tags in a jj-style table with columns for name, short oid, age, author, and summary. Width is inferred from ``$COLUMNS`` unless ``max_width`` is provided.

log_tui
-------
``pygitx.log_tui(repo, rev="HEAD", max_commits=200, decorate=True, graph=True, max_width=None) -> str``

Render a compact oneline log similar to ``git log --graph --oneline`` with optional ASCII graph and decorations for HEAD/branches/tags. Limits to ``max_commits`` entries and truncates to fit ``max_width`` (or ``$COLUMNS``).
