from __future__ import annotations

import os
import subprocess
from pathlib import Path

import pygitx


def run_git(repo: Path, *args: str, env: dict[str, str] | None = None) -> str:
    """Run a git command and return stdout (stripped)."""
    env = env or os.environ.copy()
    env.setdefault("GIT_COMMIT_GPGSIGN", "0")
    env.setdefault("GIT_TAG_GPGSIGN", "0")
    return subprocess.check_output(["git", *args], cwd=repo, text=True, env=env).strip()


def init_repo(tmp_path: Path) -> Path:
    repo = tmp_path / "repo"
    repo.mkdir()
    run_git(repo, "init", "-b", "main")
    run_git(repo, "config", "user.name", "PyGitX")
    run_git(repo, "config", "user.email", "pygitx@example.com")
    return repo


def commit_file(
    repo: Path,
    message: str,
    filename: str = "file.txt",
    content: str | None = None,
    author: str | None = None,
) -> str:
    (repo / filename).parent.mkdir(parents=True, exist_ok=True)
    body = content if content is not None else f"{message}\n"
    (repo / filename).write_text(body)
    run_git(repo, "add", filename)
    env = None
    if author:
        env = os.environ.copy()
        env["GIT_AUTHOR_NAME"] = env["GIT_COMMITTER_NAME"] = author
        env["GIT_AUTHOR_EMAIL"] = env["GIT_COMMITTER_EMAIL"] = "author@example.com"
    run_git(repo, "commit", "-m", message, env=env)
    return run_git(repo, "rev-parse", "HEAD")


def test_list_and_rev_parse(tmp_path: Path) -> None:
    repo = init_repo(tmp_path)
    first = commit_file(repo, "first")
    second = commit_file(repo, "second")

    py_repo = pygitx.open_repo(str(repo))
    head = py_repo.head()
    assert head and head.id == second

    commits = py_repo.list_commits(max=2)
    ids = [c.id for c in commits]
    assert [second, first] == ids
    assert py_repo.rev_parse("HEAD") == second
    assert py_repo.rev_parse("HEAD~1") == first


def test_amend_and_rewrite_author(tmp_path: Path) -> None:
    repo = init_repo(tmp_path)
    commit_file(repo, "initial")
    py_repo = pygitx.open_repo(str(repo))
    head_before = py_repo.head()
    assert head_before

    amended = py_repo.change_commit_message(head_before.id, "new message")
    new_head = py_repo.head()
    assert new_head and new_head.id != head_before.id
    assert amended.updated_refs.get("HEAD") == new_head.id

    rewritten = py_repo.rewrite_author(new_head.id, "New Author", "new@example.com")
    head_after = py_repo.head()
    assert head_after and head_after.id != new_head.id
    assert rewritten.updated_refs.get("HEAD") == head_after.id

    log = run_git(repo, "log", "-1", "--pretty=%an %ae %s")
    assert "New Author" in log and "new@example.com" in log and "new message" in log


def test_squash_rebase_filter_and_remove_path(tmp_path: Path) -> None:
    repo = init_repo(tmp_path)
    base = commit_file(repo, "base", "base.txt", "base")
    mid = commit_file(repo, "mid", "mid.txt", "mid")
    tip = commit_file(repo, "tip", "tip.txt", "tip")

    py_repo = pygitx.open_repo(str(repo))
    squashed = py_repo.squash_last(3, mode="squash", message="squashed")
    assert squashed.old_to_new.get(base)
    assert run_git(repo, "rev-list", "--count", "HEAD") == "1"

    # Build branch and rebase onto main.
    commit_file(repo, "after squash", "after.txt", "after")
    run_git(repo, "checkout", "-b", "feature")
    feat_tip = commit_file(repo, "feat change", "feature.txt", "branch change")
    run_git(repo, "checkout", "main")
    onto = commit_file(repo, "main change", "main.txt", "main change")

    rebase_result = py_repo.rebase_branch("feature", "main")
    assert rebase_result.old_to_new.get(feat_tip)
    feature_tip = run_git(repo, "rev-parse", "feature")
    assert feature_tip != feat_tip
    assert onto in run_git(repo, "rev-list", "feature")

    # Checkout feature to apply filters.
    run_git(repo, "checkout", "feature")
    py_repo = pygitx.open_repo(str(repo))
    commit_file(repo, "WIP: secret work", "secrets/secret.txt", "secret", author="Bad Actor")
    commit_file(repo, "cleanup", "cleanup.txt", "done")

    filtered = py_repo.filter_commits(message_contains="WIP")
    assert filtered.old_to_new
    log_subjects = run_git(repo, "log", "--pretty=%s")
    assert "WIP" not in log_subjects

    removed = py_repo.remove_path("secrets/*.txt")
    assert removed.old_to_new
    with subprocess.Popen(
        ["git", "show", "HEAD:secrets/secret.txt"],
        cwd=repo,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    ) as proc:
        stdout, stderr = proc.communicate()
        assert proc.returncode != 0
        assert b"fatal" in stderr or b"exists" in stderr


def test_reword_arbitrary_commit(tmp_path: Path) -> None:
    repo = init_repo(tmp_path)
    base = commit_file(repo, "base", "base.txt", "base")
    middle = commit_file(repo, "middle", "mid.txt", "mid")
    tip = commit_file(repo, "tip", "tip.txt", "tip")

    py_repo = pygitx.open_repo(str(repo))
    result = py_repo.reword(middle, "rewritten middle")
    new_head = py_repo.head()
    assert new_head and new_head.id != tip
    assert result.old_to_new.get(middle)

    log = run_git(repo, "log", "--pretty=%s")
    # Newest is rewritten tip, second line should be rewritten middle
    assert "rewritten middle" in log.splitlines()[1]


def test_keep_path_rewrites_history(tmp_path: Path) -> None:
    repo = init_repo(tmp_path)
    a1 = commit_file(repo, "add a", "a.txt", "a1")
    b1 = commit_file(repo, "add b", "b.txt", "b1")
    a2 = commit_file(repo, "update a", "a.txt", "a2")

    py_repo = pygitx.open_repo(str(repo))
    result = py_repo.keep_path("a.txt")
    assert result.old_to_new

    rewritten = set(result.old_to_new.values())
    for oid in rewritten:
        paths = subprocess.check_output(
            ["git", "ls-tree", "-r", "--name-only", oid], cwd=repo, text=True
        ).strip().splitlines()
        assert all(p == "a.txt" for p in paths)


def test_list_branches_tags_and_current_branch_flow(tmp_path: Path) -> None:
    repo = init_repo(tmp_path)
    base = commit_file(repo, "base")
    tip = commit_file(repo, "tip")
    run_git(repo, "branch", "feature")
    run_git(repo, "tag", "-a", "v1.0", "-m", "v1.0", base)
    run_git(repo, "update-ref", "refs/remotes/origin/main", tip)

    py_repo = pygitx.open_repo(str(repo))
    locals_default = py_repo.list_branches()
    assert "main" in locals_default and "feature" in locals_default

    remotes = py_repo.list_branches(False, True)
    assert any("origin" in r for r in remotes)

    tags = py_repo.list_tags()
    assert "v1.0" in tags

    assert py_repo.current_branch() == "main"
