from __future__ import annotations

import os
import sys
import subprocess
from pathlib import Path

import pygitx
import pytest


def run_git(repo: Path, *args: str, env: dict[str, str] | None = None) -> str:
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


def test_head_and_list_commits_wrappers_accept_repo_and_path(tmp_path: Path) -> None:
    repo_path = init_repo(tmp_path)
    first = commit_file(repo_path, "first")
    second = commit_file(repo_path, "second")
    py_repo = pygitx.open_repo(str(repo_path))

    head_from_repo = py_repo.head()
    head_from_path = pygitx.open_repo(str(repo_path)).head()
    assert head_from_repo and head_from_repo.id == second
    assert head_from_path and head_from_path.id == second

    ids = [c.id for c in py_repo.list_commits(max=2)]
    assert ids == [second, first]


def test_change_commit_message_wrapper_validates_and_updates(tmp_path: Path) -> None:
    repo_path = init_repo(tmp_path)
    commit_id = commit_file(repo_path, "initial")

    with pytest.raises(ValueError):
        py_repo = pygitx.open_repo(str(repo_path))
        py_repo.change_commit_message(commit_id, "   ")

    updated = pygitx.open_repo(str(repo_path)).change_commit_message(commit_id, "new message")
    new_head = pygitx.open_repo(str(repo_path)).head()
    assert new_head and new_head.id != commit_id
    assert updated.updated_refs.get("HEAD") == new_head.id
    log = run_git(repo_path, "log", "-1", "--pretty=%s")
    assert log == "new message"


def test_reword_wrapper(tmp_path: Path) -> None:
    repo_path = init_repo(tmp_path)
    base = commit_file(repo_path, "base", "base.txt", "base")
    middle = commit_file(repo_path, "middle", "mid.txt", "mid")
    tip = commit_file(repo_path, "tip", "tip.txt", "tip")

    py_repo = pygitx.open_repo(str(repo_path))
    with pytest.raises(ValueError):
        py_repo.reword(middle, "   ")

    result = py_repo.reword(middle, "reworded middle")
    new_head = py_repo.head()
    assert new_head and new_head.id != tip
    assert result.old_to_new.get(middle)
    log = run_git(repo_path, "log", "--pretty=%s")
    assert "reworded middle" in log.splitlines()[1]


def test_repo_summary_and_str(tmp_path: Path) -> None:
    repo_path = init_repo(tmp_path)
    commit_file(repo_path, "a1", "a.txt", "a1")
    commit_file(repo_path, "b1", "b.txt", "b1")

    py_repo = pygitx.open_repo(str(repo_path))
    summary = py_repo.summary()
    assert summary.branch is not None
    assert summary.commits >= 2
    assert summary.files >= 2

    rendered = str(py_repo)
    assert "branch" in rendered
    assert "commits" in rendered
    assert "dirty" in rendered

    # Module-level helper
    summary2 = pygitx.summary(py_repo)
    assert summary2.branch == summary.branch
    summary3 = pygitx.summary(str(repo_path))
    assert summary3.branch == summary.branch


def test_graph_helpers_basic(tmp_path: Path) -> None:
    repo_path = init_repo(tmp_path)
    base = commit_file(repo_path, "base")
    tip = commit_file(repo_path, "tip")
    run_git(repo_path, "branch", "feature", base)  # branch from base; main continues at tip
    run_git(repo_path, "checkout", "feature")
    feat_tip = commit_file(repo_path, "feature tip")

    py_repo = pygitx.open_repo(str(repo_path))
    assert py_repo.merge_base(base, feat_tip) == base
    assert py_repo.is_ancestor(base, feat_tip) is True
    ahead, behind = py_repo.ahead_behind(feat_tip, tip)
    ahead2, behind2 = pygitx.open_repo(str(repo_path)).ahead_behind(feat_tip, tip)
    assert ahead >= 0 and behind >= 0
    assert (ahead, behind) == (ahead2, behind2)
    with pytest.raises(ValueError):
        py_repo.merge_base("   ", tip)
    with pytest.raises(ValueError):
        py_repo.is_ancestor("", tip)
    with pytest.raises(ValueError):
        py_repo.ahead_behind("   ", tip)
    with pytest.raises(ValueError):
        py_repo.merge_base(base, "   ")
    with pytest.raises(ValueError):
        py_repo.is_ancestor(base, "   ")
    with pytest.raises(ValueError):
        py_repo.ahead_behind(base, "   ")


def test_diff_stat_wrapper(tmp_path: Path) -> None:
    repo_path = init_repo(tmp_path)
    base = commit_file(repo_path, "base", "a.txt", "one")
    # Modify a.txt and add b.txt
    Path(repo_path / "a.txt").write_text("two")
    Path(repo_path / "b.txt").write_text("new")
    run_git(repo_path, "add", ".")
    run_git(repo_path, "commit", "-m", "update and add")
    tip = run_git(repo_path, "rev-parse", "HEAD")

    py_repo = pygitx.open_repo(str(repo_path))
    stats = py_repo.diff_stat(base, tip)
    assert stats.files_changed == 2
    assert "a.txt" in stats.paths and "b.txt" in stats.paths
    assert "DiffStat(" in repr(stats)
    assert "files_changed: 2" in str(stats)
    filtered = py_repo.diff_stat(base, tip, paths=["a.txt"])
    assert filtered.files_changed == 1
    assert filtered.paths == ["a.txt"]
    with pytest.raises(ValueError):
        py_repo.diff_stat("   ", tip)
    with pytest.raises(ValueError):
        py_repo.diff_stat(base, "   ")


def test_refs_and_log_tui(tmp_path: Path) -> None:
    repo_path = init_repo(tmp_path)
    base = commit_file(repo_path, "base", "a.txt", "a1")
    main_tip = commit_file(repo_path, "main tip", "a.txt", "a2")
    run_git(repo_path, "branch", "feature", base)
    run_git(repo_path, "checkout", "feature")
    feature_tip = commit_file(repo_path, "feature tip", "b.txt", "b1")
    # Merge feature into main
    run_git(repo_path, "checkout", "main")
    run_git(repo_path, "merge", "--no-ff", "feature", "-m", "merge feature")
    run_git(repo_path, "tag", "-a", "v1.0", "-m", "v1.0", main_tip)

    refs = pygitx.refs_tui(repo_path)
    assert "main" in refs and "feature" in refs and "tag:v1.0" in refs
    log = pygitx.log_tui(repo_path, max_commits=10)
    assert "*" in log
    # Verify that at least one branch name appears in decoration context (within square brackets)
    assert ("[main" in log or "main]" in log or "[feature" in log or "feature]" in log)
    assert main_tip[:7] in log or feature_tip[:7] in log
    with pytest.raises(ValueError):
        pygitx.refs_tui(repo_path, local=False, remote=False, tags=False)
    with pytest.raises(ValueError):
        pygitx.log_tui(repo_path, rev="   ")
    with pytest.raises(ValueError):
        pygitx.log_tui(repo_path, max_commits=0)


def test_log_graph(tmp_path: Path) -> None:
    repo_path = init_repo(tmp_path)
    base = commit_file(repo_path, "base", "a.txt", "a1")
    main_tip = commit_file(repo_path, "main tip", "a.txt", "a2")
    run_git(repo_path, "branch", "feature", base)
    run_git(repo_path, "checkout", "feature")
    commit_file(repo_path, "feature tip", "b.txt", "b1")
    run_git(repo_path, "checkout", "main")
    run_git(repo_path, "merge", "--no-ff", "feature", "-m", "merge feature")
    run_git(repo_path, "tag", "-a", "v1.0", "-m", "v1.0", main_tip)

    log = pygitx.log_graph(repo_path, max_commits=20)
    assert "main" in log and "feature" in log
    assert "tag:v1.0" in log or "v1.0" in log
    assert "* " in log
    assert "| " in log or "\\" in log or "/" in log
    log2 = pygitx.open_repo(str(repo_path)).log_graph(max_commits=10)
    assert "* " in log2
    with pytest.raises(ValueError):
        pygitx.log_graph(repo_path, max_commits=0)

def test_repo_alias_tui_methods(tmp_path: Path, monkeypatch) -> None:
    repo_path = init_repo(tmp_path)
    commit_file(repo_path, "base", "a.txt", "a1")
    commit_file(repo_path, "tip", "a.txt", "a2")
    repo = pygitx.open_repo(str(repo_path))
    monkeypatch.setattr(sys.stdout, "isatty", lambda: False)
    refs = repo.refs_tui()
    assert "main" in refs
    log = repo.log_tui(max_commits=5)
    assert "* " in log


def test_deprecated_shims_removed(tmp_path: Path) -> None:
    repo_path = init_repo(tmp_path)
    commit_file(repo_path, "initial")
    # Deprecated shims removed; ensure attributes no longer exist.
    for name in [
        "rev_parse",
        "list_tags",
        "list_branches",
        "list_commits",
        "head",
        "current_branch",
        "merge_base",
        "is_ancestor",
        "ahead_behind",
        "diff_stat",
        "change_commit_message",
        "reword",
        "rewrite_author",
        "filter_commits",
        "remove_path",
        "keep_path",
        "rebase_branch",
        "squash_last",
        "create_backup_ref",
    ]:
        assert not hasattr(pygitx, name)


def test_color_auto_defaults(monkeypatch) -> None:
    calls: dict[str, bool] = {}

    def fake_render_refs(self, local, remote, tags, max_width, color):
        calls["color"] = color
        return "ok"

    monkeypatch.setattr(pygitx.Repo, "render_refs", fake_render_refs)
    monkeypatch.setattr(sys.stdout, "isatty", lambda: True)
    repo = pygitx.open_repo(Path.cwd())
    pygitx.refs_tui(repo)
    assert calls["color"] is True


def test_color_auto_handles_isatty_exception(monkeypatch) -> None:
    calls: dict[str, bool] = {}

    def fake_render_refs(self, *_args, **kwargs):
        calls["color"] = kwargs.get("color", _args[-1] if _args else None)
        return "ok"

    class BrokenStdout:
        def isatty(self):
            raise RuntimeError("boom")

    monkeypatch.setattr(pygitx.Repo, "render_refs", fake_render_refs)
    monkeypatch.setattr(sys, "stdout", BrokenStdout())
    repo = pygitx.open_repo(Path.cwd())
    pygitx.refs_tui(repo)
    assert calls["color"] is False


def test_rewrite_author_wrapper_validates_and_updates(tmp_path: Path) -> None:
    repo_path = init_repo(tmp_path)
    commit_id = commit_file(repo_path, "initial")
    py_repo = pygitx.open_repo(str(repo_path))

    with pytest.raises(ValueError):
        py_repo.rewrite_author(commit_id, "", "email@example.com")
    with pytest.raises(ValueError):
        py_repo.rewrite_author(commit_id, "Name", "   ")

    rewritten = py_repo.rewrite_author(commit_id, "New Name", "new@example.com")
    new_head = py_repo.head()
    assert new_head and rewritten.updated_refs.get("HEAD") == new_head.id
    log = run_git(repo_path, "log", "-1", "--pretty=%an %ae")
    assert "New Name new@example.com" in log


def test_rev_parse_wrapper_validates(tmp_path: Path) -> None:
    repo_path = init_repo(tmp_path)
    commit_id = commit_file(repo_path, "initial")

    with pytest.raises(ValueError):
        pygitx.open_repo(str(repo_path)).rev_parse("   ")

    assert pygitx.open_repo(str(repo_path)).rev_parse("HEAD") == commit_id


def test_filter_and_remove_path_wrappers_validate_and_run(tmp_path: Path) -> None:
    repo_path = init_repo(tmp_path)
    base = commit_file(repo_path, "base", "base.txt", "base")
    wip = commit_file(repo_path, "WIP: temp", "wip.txt", "temp")
    commit_file(repo_path, "final", "final.txt", "final")

    py_repo = pygitx.open_repo(str(repo_path))

    filtered = py_repo.filter_commits(message_contains="WIP")
    assert filtered.old_to_new.get(wip)

    # Add a secret and ensure it is removed.
    commit_file(repo_path, "secret", "secrets/secret.txt", "secret")
    removed = py_repo.remove_path("secrets/*.txt")
    assert removed.old_to_new
    with subprocess.Popen(
        ["git", "show", "HEAD:secrets/secret.txt"], cwd=repo_path, stdout=subprocess.PIPE, stderr=subprocess.PIPE
    ) as proc:
        _, stderr = proc.communicate()
        assert proc.returncode != 0
        assert b"fatal" in stderr or b"exists" in stderr


def test_keep_path_wrapper(tmp_path: Path) -> None:
    repo_path = init_repo(tmp_path)
    commit_file(repo_path, "a1", "a.txt", "a1")
    commit_file(repo_path, "b1", "b.txt", "b1")
    commit_file(repo_path, "a2", "a.txt", "a2")

    py_repo = pygitx.open_repo(str(repo_path))
    result = py_repo.keep_path("a.txt")
    assert result.old_to_new
    rewritten = set(result.old_to_new.values())
    for oid in rewritten:
        paths = subprocess.check_output(
            ["git", "ls-tree", "-r", "--name-only", oid], cwd=repo_path, text=True
        ).strip().splitlines()
        assert all(p == "a.txt" for p in paths)


def test_list_branches_tags_and_current_branch(tmp_path: Path) -> None:
    repo_path = init_repo(tmp_path)
    base = commit_file(repo_path, "base")
    tip = commit_file(repo_path, "tip")

    run_git(repo_path, "branch", "feature")
    run_git(repo_path, "tag", "-a", "v1.0", "-m", "v1.0", base)
    run_git(repo_path, "update-ref", "refs/remotes/origin/main", tip)

    py_repo = pygitx.open_repo(str(repo_path))
    locals_default = py_repo.list_branches()
    assert "main" in locals_default and "feature" in locals_default

    remotes = py_repo.list_branches(local=False, remote=True)
    assert any("origin" in r for r in remotes)

    tags = py_repo.list_tags()
    assert "v1.0" in tags

    assert py_repo.current_branch() == "main"


def test_graph_helpers(tmp_path: Path) -> None:
    repo_path = init_repo(tmp_path)
    base = commit_file(repo_path, "base")
    commit_file(repo_path, "main1")
    commit_file(repo_path, "main2")

    # Create feature branch from base and add two commits
    run_git(repo_path, "branch", "feature", base)
    run_git(repo_path, "checkout", "feature")
    commit_file(repo_path, "feature1")
    feat_tip = commit_file(repo_path, "feature2")

    # Diverge main further
    run_git(repo_path, "checkout", "main")
    main_tip = commit_file(repo_path, "main3")

    py_repo = pygitx.open_repo(str(repo_path))
    # merge base of main tip and feature tip should be base
    assert py_repo.merge_base(main_tip, feat_tip) == base
    assert py_repo.is_ancestor(base, feat_tip) is True
    assert py_repo.is_ancestor(feat_tip, main_tip) is False

    ahead_feat_vs_main, behind_feat_vs_main = py_repo.ahead_behind(feat_tip, main_tip)
    ahead_main_vs_feat, behind_main_vs_feat = py_repo.ahead_behind(main_tip, feat_tip)

    # feature has 2 commits not in main; main has 3 not in feature
    assert ahead_feat_vs_main == 2
    assert behind_feat_vs_main == 3
    assert ahead_main_vs_feat == 3
    assert behind_main_vs_feat == 2


def test_rebase_branch_wrapper_validates_and_runs(tmp_path: Path) -> None:
    repo_path = init_repo(tmp_path)
    base = commit_file(repo_path, "base")
    commit_file(repo_path, "main change", "main.txt", "main")
    run_git(repo_path, "checkout", "-b", "feature")
    feat_commit = commit_file(repo_path, "feat change", "feature.txt", "feat")
    run_git(repo_path, "checkout", "main")
    onto = commit_file(repo_path, "onto change", "onto.txt", "onto")

    py_repo = pygitx.open_repo(str(repo_path))
    with pytest.raises(ValueError):
        py_repo.rebase_branch("   ", "main")
    with pytest.raises(ValueError):
        py_repo.rebase_branch("feature", "")

    result = py_repo.rebase_branch("feature", "main")
    assert result.old_to_new.get(feat_commit)
    feature_tip = run_git(repo_path, "rev-parse", "feature")
    assert feature_tip != feat_commit
    assert onto in run_git(repo_path, "rev-list", "feature")


def test_squash_last_wrapper_validates_and_runs(tmp_path: Path) -> None:
    repo_path = init_repo(tmp_path)
    first = commit_file(repo_path, "first")
    second = commit_file(repo_path, "second")

    py_repo = pygitx.open_repo(str(repo_path))
    with pytest.raises(ValueError):
        py_repo.squash_last(1)
    with pytest.raises(ValueError):
        py_repo.squash_last(2, mode="merge")  # invalid mode

    squashed = py_repo.squash_last(2, mode="squash")
    assert squashed.old_to_new.get(second)
    assert run_git(repo_path, "rev-list", "--count", "HEAD") == "1"


def test_create_backup_ref_wrapper(tmp_path: Path) -> None:
    repo_path = init_repo(tmp_path)
    commit_file(repo_path, "first")
    backup_root = pygitx.open_repo(str(repo_path)).create_backup_ref()
    assert backup_root.startswith("refs/pygitx/backup/")
    refs = run_git(repo_path, "show-ref")
    assert f"{backup_root}/HEAD" in refs
