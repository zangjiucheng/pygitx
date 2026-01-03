API Reference
=============

Module
------

.. module:: histgit

Classes
-------

.. py:class:: histgit.CommitInfo

   Lightweight, read-only view of a git commit.

   .. py:attribute:: id
      :type: str

      Full commit SHA.

   .. py:attribute:: summary
      :type: str | None

      One-line summary if present.

   .. py:attribute:: author
      :type: str

      Author name.

   .. py:attribute:: email
      :type: str | None

      Author email.

   .. py:attribute:: time
      :type: int

      Commit time (seconds since epoch).

   .. py:attribute:: offset_minutes
      :type: int

      Timezone offset in minutes.

.. py:class:: histgit.Repo

   Thin wrapper around a git repository.

   .. py:method:: head() -> CommitInfo | None

      Return the current HEAD commit, or None if HEAD is unborn/detached without a commit.

   .. py:method:: list_commits(max: int | None = None) -> list[CommitInfo]

      Commits reachable from HEAD (newest first). Pass ``max`` to limit results.

   .. py:method:: change_commit_message(commit_id: str, new_message: str) -> CommitInfo

      Amend HEAD with a new message (rewrites history). ``commit_id`` must resolve to HEAD.

   .. py:method:: rewrite_author(commit_id: str, new_name: str, new_email: str, update_committer: bool = True) -> CommitInfo

      Amend HEAD with a new author (and committer if ``update_committer``). ``commit_id`` must resolve to HEAD.

Functions
---------

.. py:function:: histgit.open_repo(path: str | os.PathLike[str]) -> Repo

   Open a git repository at ``path``. Supports ``pathlib.Path`` and ``~`` expansion.
