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

.. py:class:: pygitx.RepoSummary

   Lightweight summary of a repository.

   .. py:attribute:: path
      :type: str
   .. py:attribute:: head
      :type: str | None
   .. py:attribute:: branch
      :type: str | None
   .. py:attribute:: commits
      :type: int
   .. py:attribute:: branches
      :type: int
   .. py:attribute:: tags
      :type: int
   .. py:attribute:: remotes
      :type: int
   .. py:attribute:: authors
      :type: int
   .. py:attribute:: files
      :type: int
   .. py:attribute:: size_kb
      :type: int
   .. py:attribute:: is_dirty
      :type: bool
   .. py:attribute:: last_commit_time
      :type: int | None

.. py:class:: pygitx.Repo

   Thin wrapper around a git repository.

   .. py:method:: head() -> CommitInfo | None

      Return the current HEAD commit, or None if HEAD is unborn/detached without a commit.

   .. py:method:: summary() -> RepoSummary

      Return a structured summary (also used by ``__str__``/``__repr__``).

   .. py:method:: rev_parse(spec: str) -> str

      Resolve a revision string (e.g., ``HEAD``, ``HEAD~1``, branch/tag, or full/short oid) to a hex object id.

   .. py:method:: list_commits(max: int | None = None) -> list[CommitInfo]

      Commits reachable from HEAD (newest first). Pass ``max`` to limit results.

   .. py:method:: change_commit_message(commit_id: str, new_message: str) -> RewriteResult

      Amend HEAD with a new message (rewrites history). ``commit_id`` must resolve to HEAD.

   .. py:method:: reword(commit_id: str, new_message: str) -> RewriteResult

      Reword an arbitrary commit on the current branch (linear history only). Traverses the first-parent chain from HEAD to the target and replays commits with the updated message.

   .. py:method:: list_branches(local: bool = True, remote: bool = False) -> list[str]

      List branch names. Control inclusion with ``local``/``remote`` flags.

   .. py:method:: list_tags() -> list[str]

   List tag names (lightweight/annotated).

   .. py:method:: current_branch() -> str | None

      Return the current branch name, or ``None`` if detached/unborn.

   .. py:method:: merge_base(a_spec: str, b_spec: str) -> str | None

      Compute merge base between two revisions (hex oid or ``None``).

   .. py:method:: is_ancestor(a_spec: str, b_spec: str) -> bool

      Return True if ``a_spec`` is an ancestor of ``b_spec``.

   .. py:method:: ahead_behind(a_spec: str, b_spec: str) -> tuple[int, int]

      Return (ahead, behind) counts comparing ``a_spec`` to ``b_spec``.

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

.. py:function:: pygitx.summary(repo: Repo | str) -> RepoSummary

   Return a structured repository summary.

.. py:function:: pygitx.rev_parse(repo: Repo | str, spec: str) -> str

   Resolve a revision spec to a hex object id. Accepts typical git rev syntax (``HEAD``, ``HEAD~N``, branches, tags, short/full oids).

.. py:function:: pygitx.list_branches(repo: Repo | str, local: bool = True, remote: bool = False) -> list[str]

   List branch names (local/remote controlled by flags).

.. py:function:: pygitx.list_tags(repo: Repo | str) -> list[str]

   List tag names.

.. py:function:: pygitx.current_branch(repo: Repo | str) -> str | None

   Return the current branch name, or ``None`` if detached/unborn.

.. py:function:: pygitx.merge_base(repo: Repo | str, a_spec: str, b_spec: str) -> str | None

   Compute merge base between two revisions.

.. py:function:: pygitx.is_ancestor(repo: Repo | str, a_spec: str, b_spec: str) -> bool

   Return True if ``a_spec`` is an ancestor of ``b_spec``.

.. py:function:: pygitx.ahead_behind(repo: Repo | str, a_spec: str, b_spec: str) -> tuple[int, int]

   Return (ahead, behind) counts comparing ``a_spec`` to ``b_spec``.

.. py:function:: pygitx.reword(repo: Repo | str, commit_id: str, new_message: str) -> RewriteResult

   Reword an arbitrary commit on the current branch (linear history only).

.. py:function:: pygitx.change_commit_message(repo: Repo | str, commit_id: str, new_message: str) -> RewriteResult

   Amend HEAD with a new message (rewrites history).

.. py:function:: pygitx.rewrite_author(repo: Repo | str, commit_id: str, new_name: str, new_email: str, update_committer: bool = True) -> RewriteResult

   Amend HEAD with a new author (and committer if ``update_committer``).

.. py:function:: pygitx.filter_commits(repo: Repo | str, author: str | None = None, message_contains: str | None = None) -> RewriteResult

   Drop commits matching author or message substring; merge commits are rejected.

.. py:function:: pygitx.remove_path(repo: Repo | str, path_pattern: str) -> RewriteResult

   Purge a path (glob) from all commits reachable from HEAD.

.. py:function:: pygitx.rebase_branch(repo: Repo | str, branch: str, onto: str) -> RewriteResult

   Rebase a branch onto a new base (pick-only).

.. py:function:: pygitx.squash_last(repo: Repo | str, count: int, mode: str = "squash", message: str | None = None) -> RewriteResult

   Squash the most recent commits into one.
