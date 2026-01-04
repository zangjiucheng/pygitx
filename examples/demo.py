"""
Unified demo script for pygitx.

- Create a throwaway repo with sample branches/commits.
- List commits from a repo.
- Amend HEAD message or author.
- Rebase a branch onto a new base (pick-only).
- Squash the latest commits (squash or fixup message handling).
- Filter commits by author/message substring.
- Remove a path (glob) across history.
"""

from __future__ import annotations

import argparse
from pathlib import Path
import subprocess

import pygitx
from repo_factory import clean_repo, generate_repo, load_repo_path


def run(cmd: list[str], cwd: Path) -> None:
    import subprocess

    subprocess.run(cmd, cwd=cwd, check=True)


def fmt_mapping(mapping: dict[str, str]) -> str:
    return ", ".join(f"{old[:7]} -> {new[:7]}" for old, new in mapping.items()) or "none"


def list_commits(repo: pygitx.Repo, max_commits: int | None) -> None:
    head = repo.head()
    print(f"HEAD: {head.id if head else 'None'}")
    for c in repo.list_commits(max=max_commits):
        email = c.email or ""
        summary = c.summary or ""
        print(f"{c.id[:7]} {c.author} <{email}> {summary}")


def amend_message(repo: pygitx.Repo, message: str) -> None:
    head = repo.head()
    if not head:
        raise SystemExit("No HEAD to amend")
    result = repo.change_commit_message(head.id, message)
    head_update = result.updated_refs.get("HEAD")
    print(f"Amended message mapping: {fmt_mapping(result.old_to_new)}")
    if head_update:
        print(f"HEAD now at {head_update[:7]}")


def rewrite_author(repo: pygitx.Repo, name: str, email: str) -> None:
    head = repo.head()
    if not head:
        raise SystemExit("No HEAD to rewrite")
    result = repo.rewrite_author(head.id, name, email)
    head_update = result.updated_refs.get("HEAD")
    print(f"Amended author mapping: {fmt_mapping(result.old_to_new)}")
    if head_update:
        print(f"HEAD now at {head_update[:7]}")


def rebase_branch(repo: pygitx.Repo, branch: str, onto: str) -> None:
    mappings = repo.rebase_branch(branch, onto)
    print("Replayed commits:")
    for old, new in mappings.old_to_new.items():
        print(f"  {old[:7]} -> {new[:7]}")


def squash_last(repo: pygitx.Repo, count: int, mode: str, message: str | None) -> None:
    squashed = repo.squash_last(count, mode=mode, message=message)
    print(f"Squashed top {count} commits -> {fmt_mapping(squashed.old_to_new)}")

def filter_commits(repo: pygitx.Repo, author: str | None, message_contains: str | None) -> None:
    mappings = repo.filter_commits(author=author, message_contains=message_contains)
    print(f"Filtered commits (old -> new): {fmt_mapping(mappings.old_to_new)}")

def remove_path(repo: pygitx.Repo, path_pattern: str) -> None:
    mappings = repo.remove_path(path_pattern)
    print(f"Purged '{path_pattern}' from history (old -> new): {fmt_mapping(mappings.old_to_new)}")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="pygitx demo utilities")
    sub = parser.add_subparsers(dest="cmd", required=True)

    gen = sub.add_parser("init", help="clone git2-rs and add demo commits (persists path)")
    gen.add_argument("--dest", type=Path, help="destination directory (default: temp dir)")
    gen.add_argument("--no-persist", action="store_true", help="do not persist generated repo path")

    clean = sub.add_parser("clean", help="delete the last generated repo and reset state")

    ls = sub.add_parser("list", help="list commits from a repo")
    ls.add_argument("path", nargs="?", type=Path, help="path to repo (default: last generated)")
    ls.add_argument("--max", type=int, default=None, help="max commits to show")

    amend = sub.add_parser("amend-message", help="amend HEAD commit message")
    amend.add_argument("path", nargs="?", type=Path, help="path to repo (default: last generated)")
    amend.add_argument("message", help="new commit message")

    author = sub.add_parser("rewrite-author", help="rewrite HEAD author/committer")
    author.add_argument("path", nargs="?", type=Path, help="path to repo (default: last generated)")
    author.add_argument("name", help="new author name")
    author.add_argument("email", help="new author email")

    rebase = sub.add_parser("rebase", help="rebase a branch onto a new base (pick-only)")
    rebase.add_argument("path", nargs="?", type=Path, help="path to repo (default: last generated)")
    rebase.add_argument("branch", help="branch to rebase")
    rebase.add_argument("onto", help="onto commit-ish (e.g., main or a commit id)")

    squash = sub.add_parser("squash", help="squash the most recent commits")
    squash.add_argument("path", nargs="?", type=Path, help="path to repo (default: last generated)")
    squash.add_argument("count", type=int, help="number of latest commits to squash (>=2)")
    squash.add_argument("--mode", choices=["squash", "fixup"], default="squash", help="message handling")
    squash.add_argument("--message", help="explicit commit message for the squashed commit")

    filt = sub.add_parser("filter-commits", help="drop commits by author/message substring (linear history only)")
    filt.add_argument("--path", type=Path, help="path to repo (default: last generated)")
    filt.add_argument("author_pos", nargs="?", help="author to drop (shorthand for --author)")
    filt.add_argument("--author", dest="author_filter", help="drop commits with matching author name")
    filt.add_argument("--message-contains", dest="message_contains", help="drop commits whose message contains this substring")

    rm = sub.add_parser("remove-path", help="remove a path (glob) from all commits (linear history only)")
    rm.add_argument("path", nargs="?", type=Path, help="path to repo (default: last generated)")
    rm.add_argument("pattern", help="glob pattern to purge (e.g., 'secrets/*.txt')")

    return parser.parse_args()


def main() -> None:
    args = parse_args()

    if args.cmd == "init":
        path = generate_repo(args.dest, persist=not args.no_persist)
        print(f"Repo created at: {path}")
        print("Branches:")
        subprocess.run(["git", "show-branch", "--list"], cwd=path, check=True)
        return
    if args.cmd == "clean":
        clean_repo()
        print("Cleared generated repo and state.")
        return

    repo_path = getattr(args, "path", None) or load_repo_path()
    if not repo_path:
        raise SystemExit("No repo path provided and no generated_repo.py found. Run `demo.py init` first or pass a path.")
    try:
        repo = pygitx.open_repo(str(repo_path.expanduser()))
    except ValueError as err:
        raise SystemExit(str(err))

    if args.cmd == "list":
        list_commits(repo, args.max)
    elif args.cmd == "amend-message":
        amend_message(repo, args.message)
    elif args.cmd == "rewrite-author":
        rewrite_author(repo, args.name, args.email)
    elif args.cmd == "rebase":
        rebase_branch(repo, args.branch, args.onto)
    elif args.cmd == "squash":
        squash_last(repo, args.count, args.mode, args.message)
    elif args.cmd == "filter-commits":
        author_filter = args.author_filter or args.author_pos
        filter_commits(repo, author_filter, args.message_contains)
    elif args.cmd == "remove-path":
        remove_path(repo, args.pattern)


if __name__ == "__main__":
    main()
