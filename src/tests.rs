use crate::repo::{PyRepo, open_repo};
use git2::{BranchType, Commit, Oid, Repository, Signature, build::CheckoutBuilder};
use pyo3::Python;
use pyo3::exceptions::PyValueError;
use pyo3::types::PyAnyMethods;
use pyo3::types::PyString;
use std::env;
use std::fs;
use std::path::Path;
use tempfile::tempdir;

#[test]
fn empty_repo_has_no_head_or_commits() {
    init_python();
    let dir = tempdir().unwrap();
    let repo = Repository::init(dir.path()).unwrap();
    let py_repo = PyRepo { repo };

    assert!(py_repo.head().unwrap().is_none());
    assert!(py_repo.list_commits(None).unwrap().is_empty());
}

#[test]
fn repo_with_commit_reports_head() {
    init_python();
    let dir = tempdir().unwrap();
    let repo = Repository::init(dir.path()).unwrap();
    create_commit_on_ref(&repo, "HEAD", &[], "initial commit", "hello");
    let py_repo = PyRepo { repo };

    let head = py_repo.head().unwrap();
    assert!(head.is_some());
    let commits = py_repo.list_commits(Some(10)).unwrap();
    assert_eq!(commits.len(), 1);
    assert_eq!(head.unwrap().id, commits[0].id);
}

#[test]
fn rev_parse_resolves_common_specs() {
    init_python();
    let dir = tempdir().unwrap();
    let repo = Repository::init(dir.path()).unwrap();
    let first = create_commit_on_ref(&repo, "HEAD", &[], "first", "one");
    let second = create_commit_on_ref(&repo, "HEAD", &[first], "second", "two");
    let third = create_commit_on_ref(&repo, "HEAD", &[second], "third", "three");

    repo.branch("feature", &repo.find_commit(second).unwrap(), false)
        .unwrap();
    repo.reference("refs/tags/v1.0", first, true, "tag v1.0")
        .unwrap();

    let py_repo = PyRepo { repo };

    assert_eq!(py_repo.rev_parse("HEAD").unwrap(), third.to_string());
    assert_eq!(py_repo.rev_parse("HEAD~1").unwrap(), second.to_string());
    assert_eq!(py_repo.rev_parse("HEAD^").unwrap(), second.to_string());
    assert_eq!(py_repo.rev_parse("feature").unwrap(), second.to_string());
    assert_eq!(py_repo.rev_parse("v1.0").unwrap(), first.to_string());

    let short = &third.to_string()[0..7];
    assert_eq!(py_repo.rev_parse(short).unwrap(), third.to_string());
}

#[test]
fn open_repo_accepts_pathlike() {
    init_python();
    let dir = tempdir().unwrap();
    let repo = Repository::init(dir.path()).unwrap();
    create_commit_on_ref(&repo, "HEAD", &[], "initial commit", "hello");

    Python::attach(|py| {
        let pathlib = py.import("pathlib").unwrap();
        let path_obj = pathlib
            .getattr("Path")
            .unwrap()
            .call1((dir.path(),))
            .unwrap();
        let py_repo = open_repo(py, path_obj.unbind()).unwrap();
        assert!(py_repo.head().unwrap().is_some());
    });
}

#[test]
fn open_repo_expands_tilde() {
    init_python();
    let home = tempdir().unwrap();
    let repo_path = home.path().join("repo");
    let repo = Repository::init(&repo_path).unwrap();
    create_commit_on_ref(&repo, "HEAD", &[], "initial commit", "hello");

    let prev_home = env::var_os("HOME");
    let home_str = home
        .path()
        .to_str()
        .expect("home path should be valid unicode")
        .to_owned();
    unsafe {
        env::set_var("HOME", &home_str);
    }

    Python::attach(|py| {
        let os = py.import("os").unwrap();
        let environ = os.getattr("environ").unwrap();
        environ
            .call_method1("__setitem__", ("HOME", &home_str))
            .unwrap();

        let tilde_path = PyString::new(py, "~/repo");
        let py_repo = open_repo(py, tilde_path.unbind().into()).unwrap();
        assert!(py_repo.head().unwrap().is_some());

        // Restore Python-side HOME
        if let Some(val) = prev_home.as_ref().and_then(|s| s.to_str()) {
            environ.call_method1("__setitem__", ("HOME", val)).unwrap();
        } else {
            let _ = environ.call_method1("pop", ("HOME",));
        }
    });

    // Restore process HOME
    if let Some(val) = prev_home {
        unsafe {
            env::set_var("HOME", val);
        }
    } else {
        unsafe {
            env::remove_var("HOME");
        }
    }
}

#[test]
fn change_commit_message_updates_head_commit() {
    init_python();
    let dir = tempdir().unwrap();
    let repo = Repository::init(dir.path()).unwrap();
    create_commit_on_ref(&repo, "HEAD", &[], "initial commit", "hello");
    let mut py_repo = PyRepo { repo };

    let original = py_repo.head().unwrap().unwrap();
    let result = py_repo
        .change_commit_message(&original.id, "amended message")
        .unwrap();
    let original_oid = Oid::from_str(&original.id).unwrap();
    let amended_oid = *result
        .old_to_new
        .get(&original_oid)
        .expect("mapping for amended commit");
    assert_ne!(amended_oid, original_oid);
    assert_eq!(
        result.updated_refs.get("HEAD"),
        Some(&amended_oid)
    );
    let backup_root = result.backup_root.clone().expect("backup_root");
    let backup_ref = py_repo
        .repo
        .find_reference(&format!("{}/HEAD", backup_root))
        .unwrap();
    assert_eq!(backup_ref.target(), Some(original_oid));

    let head_after = py_repo.head().unwrap().unwrap();
    assert_eq!(head_after.id, amended_oid.to_string());

    let original_commit = py_repo
        .repo
        .find_commit(original_oid)
        .unwrap();
    let amended_commit = py_repo
        .repo
        .find_commit(amended_oid)
        .unwrap();
    assert_eq!(original_commit.tree_id(), amended_commit.tree_id());
    assert_eq!(
        original_commit.parent_count(),
        amended_commit.parent_count()
    );
    assert_eq!(amended_commit.summary(), Some("amended message"));
}

#[test]
fn change_commit_message_rejects_non_head() {
    init_python();
    let dir = tempdir().unwrap();
    let repo = Repository::init(dir.path()).unwrap();
    let first = create_commit_on_ref(&repo, "HEAD", &[], "first", "hello");

    // Second commit modifies content so there is something to commit.
    create_commit_on_ref(&repo, "HEAD", &[first], "second", "hello again");
    let mut py_repo = PyRepo { repo };

    let commits = py_repo.list_commits(Some(10)).unwrap();
    let non_head = commits.last().expect("should have two commits");
    match py_repo.change_commit_message(&non_head.id, "should fail") {
        Ok(_) => panic!("expected amending non-HEAD to fail"),
        Err(err) => Python::attach(|py| {
            assert!(err.is_instance_of::<PyValueError>(py));
        }),
    }
}

#[test]
fn rewrite_author_updates_head() {
    init_python();
    let dir = tempdir().unwrap();
    let repo = Repository::init(dir.path()).unwrap();
    create_commit_on_ref(&repo, "HEAD", &[], "initial commit", "hello");
    let mut py_repo = PyRepo { repo };

    let original = py_repo.head().unwrap().unwrap();
    assert_eq!(original.author, "PyGitX");

    let result = py_repo
        .rewrite_author(&original.id, "New Name", "new@example.com", Some(true))
        .unwrap();
    let original_oid = Oid::from_str(&original.id).unwrap();
    let amended_oid = *result
        .old_to_new
        .get(&original_oid)
        .expect("mapping for rewritten author");
    assert_eq!(result.updated_refs.get("HEAD"), Some(&amended_oid));

    let head_after = py_repo.head().unwrap().unwrap();
    assert_eq!(head_after.id, amended_oid.to_string());
    assert_eq!(head_after.author, "New Name");
}

#[test]
fn rewrite_author_rejects_non_head() {
    init_python();
    let dir = tempdir().unwrap();
    let repo = Repository::init(dir.path()).unwrap();
    let first = create_commit_on_ref(&repo, "HEAD", &[], "first", "hello");
    create_commit_on_ref(&repo, "HEAD", &[first], "second", "hello again");
    let mut py_repo = PyRepo { repo };

    let commits = py_repo.list_commits(Some(10)).unwrap();
    let non_head = commits.last().expect("should have two commits");
    match py_repo.rewrite_author(&non_head.id, "New Name", "new@example.com", None) {
        Ok(_) => panic!("expected rewriting non-HEAD to fail"),
        Err(err) => Python::attach(|py| {
            assert!(err.is_instance_of::<PyValueError>(py));
        }),
    }
}

#[test]
fn rebase_branch_replays_commits() {
    init_python();
    let dir = tempdir().unwrap();
    let repo = Repository::init(dir.path()).unwrap();

    // Base commit on main.
    let base1 = create_commit_on_ref_with_path(&repo, "HEAD", &[], "file.txt", "base1", "base1");

    // Create feature branch from base1.
    {
        let base_commit = repo.find_commit(base1).unwrap();
        repo.branch("feature", &base_commit, false).unwrap();
    }

    // Commit on feature (divergent).
    let feature1 = create_commit_on_ref_with_path(
        &repo,
        "refs/heads/feature",
        &[base1],
        "feature.txt",
        "feature1",
        "feature1",
    );

    // New base on main after branch.
    let base2 =
        create_commit_on_ref_with_path(&repo, "HEAD", &[base1], "file.txt", "base2", "base2");

    // Ensure working tree matches base branch before rebase.
    let mut checkout = CheckoutBuilder::new();
    checkout.force().remove_untracked(true);
    repo.checkout_head(Some(&mut checkout)).unwrap();

    let mut py_repo = PyRepo { repo };
    let mappings = py_repo
        .rebase_branch("feature", &base2.to_string())
        .expect("rebase should succeed");
    assert_eq!(mappings.old_to_new.len(), 1);
    let new_oid = mappings
        .old_to_new
        .get(&feature1)
        .copied()
        .expect("mapping for feature commit");

    // Feature branch should now point to the new tip.
    let feature_branch = py_repo
        .repo
        .find_branch("feature", BranchType::Local)
        .unwrap();
    let feature_head = feature_branch.into_reference().target().unwrap();
    assert_eq!(feature_head, new_oid);
    assert_eq!(
        mappings
            .updated_refs
            .get("refs/heads/feature")
            .copied(),
        Some(new_oid)
    );
}

#[test]
fn squash_last_combines_commits() {
    init_python();
    let dir = tempdir().unwrap();
    let repo = Repository::init(dir.path()).unwrap();
    let first = create_commit_on_ref(&repo, "HEAD", &[], "first", "a");
    let second = create_commit_on_ref(&repo, "HEAD", &[first], "second", "b");
    create_commit_on_ref(&repo, "HEAD", &[second], "third", "c");
    let mut py_repo = PyRepo { repo };

    let head_before = py_repo.head().unwrap().unwrap();
    let squashed = py_repo.squash_last(3, Some("squash"), None).unwrap();
    let head = py_repo.head().unwrap().unwrap();
    let new_oid = Oid::from_str(&head.id).unwrap();
    let original_head_oid = Oid::from_str(&head_before.id).unwrap();
    assert_eq!(
        squashed.old_to_new.get(&original_head_oid),
        Some(&new_oid)
    );
    assert_eq!(py_repo.list_commits(Some(10)).unwrap().len(), 1);

    let commit = py_repo
        .repo
        .find_commit(new_oid)
        .unwrap();
    let message = commit.message().unwrap_or_default();
    assert!(message.contains("first"));
    assert!(message.contains("second"));
    assert!(message.contains("third"));
    assert_eq!(commit.parent_count(), 0);
}

#[test]
fn squash_last_fixup_uses_oldest_message() {
    init_python();
    let dir = tempdir().unwrap();
    let repo = Repository::init(dir.path()).unwrap();
    let base = create_commit_on_ref(&repo, "HEAD", &[], "base", "base");
    create_commit_on_ref(&repo, "HEAD", &[base], "tip", "tip");
    let mut py_repo = PyRepo { repo };

    let head_before = py_repo.head().unwrap().unwrap();
    let squashed = py_repo.squash_last(2, Some("fixup"), None).unwrap();
    let head = py_repo.head().unwrap().unwrap();
    let new_oid = Oid::from_str(&head.id).unwrap();
    let commit = py_repo
        .repo
        .find_commit(new_oid)
        .unwrap();
    assert_eq!(commit.summary(), Some("base"));
    assert_eq!(commit.parent_count(), 0);
    assert_eq!(
        squashed
            .old_to_new
            .get(&Oid::from_str(&head_before.id).unwrap())
            .copied(),
        Some(new_oid)
    );
}

#[test]
fn filter_commits_drops_matching_and_reparents() {
    init_python();
    let dir = tempdir().unwrap();
    let repo = Repository::init(dir.path()).unwrap();
    let base = create_commit_on_ref(&repo, "HEAD", &[], "base", "a");
    let middle = create_commit_on_ref(&repo, "HEAD", &[base], "WIP change", "b");
    let tip = create_commit_on_ref(&repo, "HEAD", &[middle], "final", "c");
    let mut py_repo = PyRepo { repo };

    let mappings = Python::attach(|py| py_repo.filter_commits(py, None, Some("WIP")))
        .expect("filter should succeed");
    let map = mappings.old_to_new;

    // Dropped commit should map to its parent's rewritten id.
    let base_new = map.get(&base).expect("base mapping");
    let middle_new = map.get(&middle).expect("middle mapping");
    assert_eq!(middle_new, base_new);

    // Head should point to rewritten tip; commit count should shrink by one.
    let head = py_repo.head().unwrap().unwrap();
    let tip_new = map.get(&tip).expect("tip mapping");
    assert_eq!(&head.id, &tip_new.to_string());
    assert_eq!(py_repo.list_commits(None).unwrap().len(), 2);
}

#[test]
fn filter_commits_matches_author_case_insensitive() {
    init_python();
    let dir = tempdir().unwrap();
    let repo = Repository::init(dir.path()).unwrap();
    let base = create_commit_on_ref(&repo, "HEAD", &[], "base", "a");
    let special = create_commit_with_author(
        &repo,
        "HEAD",
        &[base],
        "by Special",
        "b",
        "Jiucheng",
        "Jiucheng@example.com",
    );
    let tip = create_commit_on_ref(&repo, "HEAD", &[special], "final", "c");
    let mut py_repo = PyRepo { repo };

    let mappings = Python::attach(|py| py_repo.filter_commits(py, Some("jiucheng"), None))
        .expect("filter should succeed");
    let map = mappings.old_to_new;

    // Special commit should be dropped and reparented.
    let base_new = map.get(&base).expect("base mapping");
    let special_new = map.get(&special).expect("special mapping");
    assert_eq!(special_new, base_new);

    let head = py_repo.head().unwrap().unwrap();
    let tip_new = map.get(&tip).expect("tip mapping");
    assert_eq!(&head.id, &tip_new.to_string());
    assert_eq!(py_repo.list_commits(None).unwrap().len(), 2);
}

#[test]
fn remove_path_purges_files_from_history() {
    init_python();
    let dir = tempdir().unwrap();
    let repo = Repository::init(dir.path()).unwrap();
    let base = create_commit_on_ref_with_path(
        &repo,
        "HEAD",
        &[],
        "secrets/secret.txt",
        "add secret",
        "topsecret",
    );
    let tip = create_commit_on_ref_with_path(
        &repo,
        "HEAD",
        &[base],
        "file.txt",
        "modify other file",
        "hello",
    );
    let mut py_repo = PyRepo { repo };

    let mappings = Python::attach(|py| py_repo.remove_path(py, "secrets/*.txt"))
        .expect("remove_path should succeed");
    let map = mappings.old_to_new;

    let head = py_repo.head().unwrap().unwrap();
    let tip_new = map.get(&tip).expect("tip mapping");
    assert_eq!(&head.id, &tip_new.to_string());
    assert_eq!(py_repo.list_commits(None).unwrap().len(), 2);

    // All rewritten commits should no longer contain the secret path.
    let repo = &py_repo.repo;
    for oid in map.values() {
        assert!(
            !tree_has_path(repo, *oid, Path::new("secrets/secret.txt")),
            "rewritten commit {oid} still contains purged path"
        );
    }
}

#[test]
fn reword_updates_arbitrary_commit() {
    init_python();
    let dir = tempdir().unwrap();
    let repo = Repository::init(dir.path()).unwrap();
    let base = create_commit_on_ref(&repo, "HEAD", &[], "base", "base");
    let middle = create_commit_on_ref(&repo, "HEAD", &[base], "middle", "mid");
    let tip = create_commit_on_ref(&repo, "HEAD", &[middle], "tip", "tip");
    let mut py_repo = PyRepo { repo };

    let result = py_repo.reword(&middle.to_string(), "rewritten middle").unwrap();
    assert!(result.old_to_new.get(&middle).is_some());
    let head = py_repo.head().unwrap().unwrap();
    assert_ne!(head.id, tip.to_string());
    let middle_new = result.old_to_new.get(&middle).copied().unwrap();
    let rewritten = py_repo.repo.find_commit(middle_new).unwrap();
    assert_eq!(rewritten.message().unwrap_or("").trim(), "rewritten middle");
}

#[test]
fn keep_path_rewrites_history() {
    init_python();
    let dir = tempdir().unwrap();
    let repo = Repository::init(dir.path()).unwrap();
    let a1 = create_commit_on_ref_with_path(&repo, "HEAD", &[], "a.txt", "add a", "a1");
    let b1 = create_commit_on_ref_with_path(&repo, "HEAD", &[a1], "b.txt", "add b", "b1");
    let _a2 = create_commit_on_ref_with_path(&repo, "HEAD", &[b1], "a.txt", "update a", "a2");
    let mut py_repo = PyRepo { repo };

    let result = Python::attach(|py| py_repo.keep_path(py, "a.txt")).unwrap();
    assert!(!result.old_to_new.is_empty());

    let head = py_repo.head().unwrap().unwrap();
    let commit = py_repo.repo.find_commit(Oid::from_str(&head.id).unwrap()).unwrap();
    let tree = commit.tree().unwrap();
    assert!(tree.get_name("a.txt").is_some());
    assert!(tree.get_name("b.txt").is_none());
}

#[test]
fn create_backup_ref_creates_backup_refs() {
    init_python();
    let dir = tempdir().unwrap();
    let repo = Repository::init(dir.path()).unwrap();
    let head_oid = create_commit_on_ref(&repo, "HEAD", &[], "initial", "hello");
    let py_repo = PyRepo { repo };

    let root = py_repo.create_backup_ref(None).expect("backup creation");
    assert!(root.starts_with("refs/pygitx/backup/"));
    let head_backup = py_repo
        .repo
        .find_reference(&format!("{}/HEAD", root))
        .unwrap();
    assert_eq!(head_backup.target(), Some(head_oid));
}

fn init_python() {
    // Initialize the embedded Python interpreter once for PyO3-bound types used in tests.
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| {
        if std::env::var_os("PYTHONHOME").is_none() {
            if let Some((home, paths)) = detect_python_env() {
                unsafe {
                    std::env::set_var("PYTHONHOME", &home);
                    if std::env::var_os("PYTHONPATH").is_none() {
                        std::env::set_var("PYTHONPATH", paths);
                    }
                }
            }
        }
        Python::initialize();
    });
}

fn detect_python_env() -> Option<(String, String)> {
    // Prefer explicit Python from env (PYO3_PYTHON) else python3.
    let candidate = std::env::var("PYO3_PYTHON").unwrap_or_else(|_| "python3".to_string());
    let output = std::process::Command::new(candidate)
        .arg("-c")
        .arg(
            r#"import sys, sysconfig; print(sys.base_prefix); print(sysconfig.get_path('stdlib') or ''); print(sysconfig.get_path('platlib') or '')"#,
        )
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut lines = stdout.lines();
    let home = lines.next()?.trim().to_string();
    let stdlib = lines.next().unwrap_or("").trim();
    let platlib = lines.next().unwrap_or("").trim();
    let mut joined = Vec::new();
    if !stdlib.is_empty() {
        joined.push(stdlib);
    }
    if !platlib.is_empty() {
        joined.push(platlib);
    }
    let path_val = joined.join(":");
    Some((home, path_val))
}

fn create_commit_on_ref(
    repo: &Repository,
    reference: &str,
    parent_oids: &[Oid],
    message: &str,
    content: &str,
) -> Oid {
    create_commit_on_ref_with_path(repo, reference, parent_oids, "file.txt", message, content)
}

fn create_commit_on_ref_with_path(
    repo: &Repository,
    reference: &str,
    parent_oids: &[Oid],
    path: &str,
    message: &str,
    content: &str,
) -> Oid {
    let sig = Signature::now("PyGitX", "pygitx@example.com").unwrap();
    let workdir = repo.workdir().expect("repo should have workdir");
    let file_path = workdir.join(path);
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(&file_path, content).unwrap();

    let mut index = repo.index().unwrap();
    index.add_path(Path::new(path)).unwrap();
    index.write().unwrap();
    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();

    let parents: Vec<Commit<'_>> = parent_oids
        .iter()
        .map(|oid| repo.find_commit(*oid).unwrap())
        .collect();
    let parent_refs: Vec<&Commit> = parents.iter().collect();

    repo.commit(Some(reference), &sig, &sig, message, &tree, &parent_refs)
        .unwrap()
}

fn create_commit_with_author(
    repo: &Repository,
    reference: &str,
    parent_oids: &[Oid],
    message: &str,
    content: &str,
    author_name: &str,
    author_email: &str,
) -> Oid {
    let sig = Signature::now(author_name, author_email).unwrap();
    let workdir = repo.workdir().expect("repo should have workdir");
    let file_path = workdir.join("file.txt");
    fs::write(&file_path, content).unwrap();

    let mut index = repo.index().unwrap();
    index.add_path(Path::new("file.txt")).unwrap();
    index.write().unwrap();
    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();

    let parents: Vec<Commit<'_>> = parent_oids
        .iter()
        .map(|oid| repo.find_commit(*oid).unwrap())
        .collect();
    let parent_refs: Vec<&Commit> = parents.iter().collect();

    repo.commit(Some(reference), &sig, &sig, message, &tree, &parent_refs)
        .unwrap()
}

fn tree_has_path(repo: &Repository, commit_oid: Oid, path: &Path) -> bool {
    let commit = repo.find_commit(commit_oid).unwrap();
    let tree = commit.tree().unwrap();
    tree.get_path(path).is_ok()
}
