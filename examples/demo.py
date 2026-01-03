"""
Unified demo script for histgit.

- Create a throwaway repo with sample branches/commits.
- List commits from a repo.
- Amend HEAD message or author.
- Rebase a branch onto a new base (pick-only).
"""

from __future__ import annotations

import argparse
from pathlib import Path
import subprocess

import histgit
from repo_factory import generate_repo, load_repo_path


def run(cmd: list[str], cwd: Path) -> None:
    import subprocess

    subprocess.run(cmd, cwd=cwd, check=True)


def list_commits(repo: histgit.Repo, max_commits: int | None) -> None:
    head = repo.head()
    print(f"HEAD: {head.id if head else 'None'}")
    for c in repo.list_commits(max=max_commits):
        email = c.email or ""
        summary = c.summary or ""
        print(f"{c.id[:7]} {c.author} <{email}> {summary}")


def amend_message(repo: histgit.Repo, message: str) -> None:
    head = repo.head()
    if not head:
        raise SystemExit("No HEAD to amend")
    updated = repo.change_commit_message(head.id, message)
    print(f"Amended message: {head.id[:7]} -> {updated.id[:7]} [{updated.summary}]")


def rewrite_author(repo: histgit.Repo, name: str, email: str) -> None:
    head = repo.head()
    if not head:
        raise SystemExit("No HEAD to rewrite")
    updated = repo.rewrite_author(head.id, name, email)
    print(f"Amended author: {head.id[:7]} -> {updated.id[:7]} [{updated.author} <{updated.email}>]")


def rebase_branch(repo: histgit.Repo, branch: str, onto: str) -> None:
    mappings = repo.rebase_branch(branch, onto)
    print("Replayed commits:")
    for old, new in mappings:
        print(f"  {old[:7]} -> {new[:7]}")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="histgit demo utilities")
    sub = parser.add_subparsers(dest="cmd", required=True)

    gen = sub.add_parser("generate", help="create a throwaway repo with sample commits (persists path)")
    gen.add_argument("--dest", type=Path, help="destination directory (default: temp dir)")

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

    return parser.parse_args()


def main() -> None:
    args = parse_args()

    if args.cmd == "generate":
        path = generate_repo(args.dest)
        print(f"Repo created at: {path}")
        print("Branches:")
        subprocess.run(["git", "show-branch", "--list"], cwd=path, check=True)
        return

    repo_path = args.path or load_repo_path()
    if not repo_path:
        raise SystemExit("No repo path provided and no generated_repo.py found. Run `demo.py generate` first or pass a path.")
    repo = histgit.open_repo(str(repo_path.expanduser()))

    if args.cmd == "list":
        list_commits(repo, args.max)
    elif args.cmd == "amend-message":
        amend_message(repo, args.message)
    elif args.cmd == "rewrite-author":
        rewrite_author(repo, args.name, args.email)
    elif args.cmd == "rebase":
        rebase_branch(repo, args.branch, args.onto)


if __name__ == "__main__":
    main()
