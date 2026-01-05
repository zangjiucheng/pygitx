History rewriting
=================

These operations rewrite commits; avoid using on published history unless you intend to force-push.

change_commit_message
---------------------
``Repo.change_commit_message(commit_id: str, new_message: str) -> RewriteResult``

Amend the HEAD commit message. ``commit_id`` must resolve to HEAD.

rewrite_author
--------------
``Repo.rewrite_author(commit_id: str, new_name: str, new_email: str, update_committer: bool = True) -> RewriteResult``

Rewrite the author (and optionally committer) of HEAD.

reword
------
``Repo.reword(commit_id: str, new_message: str) -> RewriteResult``

Reword an arbitrary commit on the current branch (linear history only). Traverses the first-parent chain from HEAD to the target, rewrites that commit's message, and replays the rest.

squash_last
-----------
``Repo.squash_last(count: int, mode: str = "squash", message: str | None = None) -> RewriteResult``

Squash the most recent commits into one. ``mode="squash"`` concatenates messages; ``mode="fixup"`` keeps only the oldest message. Linear history only.

rebase_branch
-------------
``Repo.rebase_branch(branch: str, onto: str) -> RewriteResult``

Replay a local branch onto a new base (pick-only). Conflicts abort the rebase.

filter_commits
--------------
``Repo.filter_commits(author: str | None = None, message_contains: str | None = None) -> RewriteResult``

Drop commits whose author matches (case-insensitive) or whose message contains the substring. Linear history only.

remove_path
-----------
``Repo.remove_path(path_pattern: str) -> RewriteResult``

Purge a path (glob pattern) from all commits reachable from HEAD. Linear history only.

keep_path
---------
``Repo.keep_path(glob_pattern: str) -> RewriteResult``

Keep only paths matching the glob across history; everything else is dropped from rewritten commits. Linear history only.
