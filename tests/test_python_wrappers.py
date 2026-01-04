from __future__ import annotations

import os
import subprocess
from pathlib import Path

import pygitx
import pytest


def run_git(repo: Path, *args: str, env: dict[str, str] | None = None) -> str:
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


def test_head_and_list_commits_wrappers_accept_repo_and_path(tmp_path: Path) -> None:
    repo_path = init_repo(tmp_path)
    first = commit_file(repo_path, "first")
    second = commit_file(repo_path, "second")
    py_repo = pygitx.open_repo(str(repo_path))

    head_from_repo = pygitx.head(py_repo)
    head_from_path = pygitx.head(str(repo_path))
    assert head_from_repo and head_from_repo.id == second
    assert head_from_path and head_from_path.id == second

    ids = [c.id for c in pygitx.list_commits(py_repo, max=2)]
    assert ids == [second, first]


def test_change_commit_message_wrapper_validates_and_updates(tmp_path: Path) -> None:
    repo_path = init_repo(tmp_path)
    commit_id = commit_file(repo_path, "initial")

    with pytest.raises(ValueError):
        pygitx.change_commit_message(repo_path, commit_id, "   ")

    updated = pygitx.change_commit_message(repo_path, commit_id, "new message")
    new_head = pygitx.head(repo_path)
    assert new_head and new_head.id != commit_id
    assert updated.updated_refs.get("HEAD") == new_head.id
    log = run_git(repo_path, "log", "-1", "--pretty=%s")
    assert log == "new message"


def test_rewrite_author_wrapper_validates_and_updates(tmp_path: Path) -> None:
    repo_path = init_repo(tmp_path)
    commit_id = commit_file(repo_path, "initial")
    py_repo = pygitx.open_repo(str(repo_path))

    with pytest.raises(ValueError):
        pygitx.rewrite_author(py_repo, commit_id, "", "email@example.com")
    with pytest.raises(ValueError):
        pygitx.rewrite_author(py_repo, commit_id, "Name", "   ")

    rewritten = pygitx.rewrite_author(py_repo, commit_id, "New Name", "new@example.com")
    new_head = pygitx.head(py_repo)
    assert new_head and rewritten.updated_refs.get("HEAD") == new_head.id
    log = run_git(repo_path, "log", "-1", "--pretty=%an %ae")
    assert "New Name new@example.com" in log


def test_rev_parse_wrapper_validates(tmp_path: Path) -> None:
    repo_path = init_repo(tmp_path)
    commit_id = commit_file(repo_path, "initial")

    with pytest.raises(ValueError):
        pygitx.rev_parse(repo_path, "   ")

    assert pygitx.rev_parse(repo_path, "HEAD") == commit_id


def test_filter_and_remove_path_wrappers_validate_and_run(tmp_path: Path) -> None:
    repo_path = init_repo(tmp_path)
    base = commit_file(repo_path, "base", "base.txt", "base")
    wip = commit_file(repo_path, "WIP: temp", "wip.txt", "temp")
    commit_file(repo_path, "final", "final.txt", "final")

    with pytest.raises(ValueError):
        pygitx.filter_commits(repo_path, author=None, message_contains=None)
    filtered = pygitx.filter_commits(repo_path, message_contains="WIP")
    assert filtered.old_to_new.get(wip)

    with pytest.raises(ValueError):
        pygitx.remove_path(repo_path, "")
    # Add a secret and ensure it is removed.
    commit_file(repo_path, "secret", "secrets/secret.txt", "secret")
    removed = pygitx.remove_path(repo_path, "secrets/*.txt")
    assert removed.old_to_new
    with subprocess.Popen(
        ["git", "show", "HEAD:secrets/secret.txt"], cwd=repo_path, stdout=subprocess.PIPE, stderr=subprocess.PIPE
    ) as proc:
        _, stderr = proc.communicate()
        assert proc.returncode != 0
        assert b"fatal" in stderr or b"exists" in stderr


def test_rebase_branch_wrapper_validates_and_runs(tmp_path: Path) -> None:
    repo_path = init_repo(tmp_path)
    base = commit_file(repo_path, "base")
    commit_file(repo_path, "main change", "main.txt", "main")
    run_git(repo_path, "checkout", "-b", "feature")
    feat_commit = commit_file(repo_path, "feat change", "feature.txt", "feat")
    run_git(repo_path, "checkout", "main")
    onto = commit_file(repo_path, "onto change", "onto.txt", "onto")

    with pytest.raises(ValueError):
        pygitx.rebase_branch(repo_path, "   ", "main")
    with pytest.raises(ValueError):
        pygitx.rebase_branch(repo_path, "feature", "")

    result = pygitx.rebase_branch(repo_path, "feature", "main")
    assert result.old_to_new.get(feat_commit)
    feature_tip = run_git(repo_path, "rev-parse", "feature")
    assert feature_tip != feat_commit
    assert onto in run_git(repo_path, "rev-list", "feature")


def test_squash_last_wrapper_validates_and_runs(tmp_path: Path) -> None:
    repo_path = init_repo(tmp_path)
    first = commit_file(repo_path, "first")
    second = commit_file(repo_path, "second")

    with pytest.raises(ValueError):
        pygitx.squash_last(repo_path, 1)
    with pytest.raises(ValueError):
        pygitx.squash_last(repo_path, 2, mode="merge")  # invalid mode

    squashed = pygitx.squash_last(repo_path, 2, mode="squash")
    assert squashed.old_to_new.get(second)
    assert run_git(repo_path, "rev-list", "--count", "HEAD") == "1"


def test_create_backup_ref_wrapper(tmp_path: Path) -> None:
    repo_path = init_repo(tmp_path)
    commit_file(repo_path, "first")
    backup_root = pygitx.create_backup_ref(repo_path)
    assert backup_root.startswith("refs/pygitx/backup/")
    refs = run_git(repo_path, "show-ref")
    assert f"{backup_root}/HEAD" in refs
