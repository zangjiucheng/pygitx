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
    let amended = py_repo
        .change_commit_message(&original.id, "amended message")
        .unwrap();
    assert_ne!(amended.id, original.id);
    assert_eq!(amended.summary.as_deref(), Some("amended message"));

    let head_after = py_repo.head().unwrap().unwrap();
    assert_eq!(head_after.id, amended.id);

    let original_commit = py_repo
        .repo
        .find_commit(Oid::from_str(&original.id).unwrap())
        .unwrap();
    let amended_commit = py_repo
        .repo
        .find_commit(Oid::from_str(&amended.id).unwrap())
        .unwrap();
    assert_eq!(original_commit.tree_id(), amended_commit.tree_id());
    assert_eq!(
        original_commit.parent_count(),
        amended_commit.parent_count()
    );
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

    let amended = py_repo
        .rewrite_author(&original.id, "New Name", "new@example.com", Some(true))
        .unwrap();
    assert_eq!(amended.author, "New Name");

    let head_after = py_repo.head().unwrap().unwrap();
    assert_eq!(head_after.id, amended.id);
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
    assert_eq!(mappings.len(), 1);
    assert_eq!(mappings[0].0, feature1.to_string());

    // Feature branch should now point to the new tip.
    let feature_branch = py_repo
        .repo
        .find_branch("feature", BranchType::Local)
        .unwrap();
    let feature_head = feature_branch.into_reference().target().unwrap();
    assert_eq!(feature_head.to_string(), mappings[0].1);
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

    let squashed = py_repo.squash_last(3, Some("squash"), None).unwrap();
    let head = py_repo.head().unwrap().unwrap();
    assert_eq!(head.id, squashed.id);
    assert_eq!(py_repo.list_commits(Some(10)).unwrap().len(), 1);

    let commit = py_repo
        .repo
        .find_commit(Oid::from_str(&squashed.id).unwrap())
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

    let squashed = py_repo.squash_last(2, Some("fixup"), None).unwrap();
    let commit = py_repo
        .repo
        .find_commit(Oid::from_str(&squashed.id).unwrap())
        .unwrap();
    assert_eq!(commit.summary(), Some("base"));
    assert_eq!(commit.parent_count(), 0);
}

fn init_python() {
    // Initialize the embedded Python interpreter once for PyO3-bound types used in tests.
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| {
        Python::initialize();
    });
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
