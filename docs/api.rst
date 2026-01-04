API Reference
=============

.. contents::
   :local:
   :depth: 2

Module
------

.. module:: pygitx

Classes
-------

.. py:class:: pygitx.CommitInfo

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

.. py:class:: pygitx.RewriteResult

   Result of a history rewrite.

   .. py:attribute:: old_to_new
      :type: dict[str, str]

      Mapping of original commit ids to rewritten commit ids.

   .. py:attribute:: updated_refs
      :type: dict[str, str]

      References (e.g., ``HEAD`` or ``refs/heads/main``) updated to rewritten commits.

   .. py:attribute:: warnings
      :type: list[str]

      Non-fatal warnings emitted during the rewrite.

.. py:class:: pygitx.Repo

   Thin wrapper around a git repository.

   .. py:method:: head() -> CommitInfo | None

      Return the current HEAD commit, or None if HEAD is unborn/detached without a commit.

   .. py:method:: rev_parse(spec: str) -> str

      Resolve a revision string (e.g., ``HEAD``, ``HEAD~1``, branch/tag, or full/short oid) to a hex object id.

   .. py:method:: list_commits(max: int | None = None) -> list[CommitInfo]

      Commits reachable from HEAD (newest first). Pass ``max`` to limit results.

   .. py:method:: change_commit_message(commit_id: str, new_message: str) -> RewriteResult

      Amend HEAD with a new message (rewrites history). ``commit_id`` must resolve to HEAD.

   .. py:method:: rewrite_author(commit_id: str, new_name: str, new_email: str, update_committer: bool = True) -> RewriteResult

      Amend HEAD with a new author (and committer if ``update_committer``). ``commit_id`` must resolve to HEAD.

   .. py:method:: filter_commits(author: str | None = None, message_contains: str | None = None) -> RewriteResult

      Drop commits matching author or message substring; merge commits are rejected.

   .. py:method:: remove_path(path_pattern: str) -> RewriteResult

      Purge a path (glob) from all commits reachable from HEAD; merge commits are rejected.

   .. py:method:: rebase_branch(branch: str, onto: str) -> RewriteResult

      Rebase a local branch onto a new base (pick-only). Returns old-to-new commit mapping in replay order.

   .. py:method:: squash_last(count: int, mode: str = "squash", message: str | None = None) -> RewriteResult

      Squash the most recent commits into one. ``mode`` may be ``"squash"`` (concatenate messages) or ``"fixup"`` (keep oldest message). Raises if history is not linear across the requested range.

Functions
---------

.. py:function:: pygitx.open_repo(path: str | os.PathLike[str]) -> Repo

   Open a git repository at ``path``. Supports ``pathlib.Path`` and ``~`` expansion.

.. py:function:: pygitx.create_backup_ref(repo: Repo | str, prefix: str | None = None) -> str

   Create backup refs under ``refs/pygitx/backup/<timestamp>`` for HEAD and its branch (if attached).

.. py:function:: pygitx.rev_parse(repo: Repo | str, spec: str) -> str

   Resolve a revision spec to a hex object id. Accepts typical git rev syntax (``HEAD``, ``HEAD~N``, branches, tags, short/full oids).
