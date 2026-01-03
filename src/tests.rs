use crate::repo::{open_repo, PyRepo};
use git2::{Commit, ErrorCode, Oid, Repository, Signature};
use pyo3::exceptions::PyValueError;
use pyo3::types::PyString;
use pyo3::types::PyAnyMethods;
use pyo3::Python;
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
    create_commit_with_message(&repo, "initial commit", "hello");
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
    create_commit_with_message(&repo, "initial commit", "hello");

    Python::with_gil(|py| {
        let pathlib = py.import("pathlib").unwrap();
        let path_obj = pathlib.getattr("Path").unwrap().call1((dir.path(),)).unwrap();
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
    create_commit_with_message(&repo, "initial commit", "hello");

    let prev_home = env::var_os("HOME");
    let home_str = home.path().to_str().expect("home path should be valid unicode").to_owned();
    unsafe { env::set_var("HOME", &home_str); }

    Python::with_gil(|py| {
        let os = py.import("os").unwrap();
        let environ = os.getattr("environ").unwrap();
        environ.call_method1("__setitem__", ("HOME", &home_str)).unwrap();

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
        unsafe { env::set_var("HOME", val); }
    } else {
        unsafe { env::remove_var("HOME"); }
    }
}

#[test]
fn change_commit_message_updates_head_commit() {
    init_python();
    let dir = tempdir().unwrap();
    let repo = Repository::init(dir.path()).unwrap();
    create_commit_with_message(&repo, "initial commit", "hello");
    let mut py_repo = PyRepo { repo };

    let original = py_repo.head().unwrap().unwrap();
    let amended = py_repo
        .change_commit_message(&original.id, "amended message")
        .unwrap();
    assert_ne!(amended.id, original.id);
    assert_eq!(amended.summary.as_deref(), Some("amended message"));

    let head_after = py_repo.head().unwrap().unwrap();
    assert_eq!(head_after.id, amended.id);

    let original_commit = py_repo.repo.find_commit(Oid::from_str(&original.id).unwrap()).unwrap();
    let amended_commit = py_repo.repo.find_commit(Oid::from_str(&amended.id).unwrap()).unwrap();
    assert_eq!(original_commit.tree_id(), amended_commit.tree_id());
    assert_eq!(original_commit.parent_count(), amended_commit.parent_count());
}

#[test]
fn change_commit_message_rejects_non_head() {
    init_python();
    let dir = tempdir().unwrap();
    let repo = Repository::init(dir.path()).unwrap();
    create_commit_with_message(&repo, "first", "hello");

    // Second commit modifies content so there is something to commit.
    create_commit_with_message(&repo, "second", "hello again");
    let mut py_repo = PyRepo { repo };

    let commits = py_repo.list_commits(Some(10)).unwrap();
    let non_head = commits.last().expect("should have two commits");
    match py_repo.change_commit_message(&non_head.id, "should fail") {
        Ok(_) => panic!("expected amending non-HEAD to fail"),
        Err(err) => Python::with_gil(|py| {
            assert!(err.is_instance_of::<PyValueError>(py));
        }),
    }
}

#[test]
fn rewrite_author_updates_head() {
    init_python();
    let dir = tempdir().unwrap();
    let repo = Repository::init(dir.path()).unwrap();
    create_commit_with_message(&repo, "initial commit", "hello");
    let mut py_repo = PyRepo { repo };

    let original = py_repo.head().unwrap().unwrap();
    assert_eq!(original.author, "HistGit");

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
    create_commit_with_message(&repo, "first", "hello");
    create_commit_with_message(&repo, "second", "hello again");
    let mut py_repo = PyRepo { repo };

    let commits = py_repo.list_commits(Some(10)).unwrap();
    let non_head = commits.last().expect("should have two commits");
    match py_repo.rewrite_author(&non_head.id, "New Name", "new@example.com", None) {
        Ok(_) => panic!("expected rewriting non-HEAD to fail"),
        Err(err) => Python::with_gil(|py| {
            assert!(err.is_instance_of::<PyValueError>(py));
        }),
    }
}

fn init_python() {
    // Initialize the embedded Python interpreter once for PyO3-bound types used in tests.
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| {
        Python::initialize();
    });
}

fn create_commit_with_message(repo: &Repository, message: &str, content: &str) {
    let sig = Signature::now("HistGit", "histgit@example.com").unwrap();
    let workdir = repo.workdir().expect("repo should have workdir");
    let file_path = workdir.join("file.txt");
    fs::write(&file_path, content).unwrap();

    let mut index = repo.index().unwrap();
    index.add_path(Path::new("file.txt")).unwrap();
    index.write().unwrap();
    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();

    let parents: Vec<Commit<'_>> = match repo.head() {
        Ok(head_ref) => match head_ref.peel_to_commit() {
            Ok(commit) => vec![commit],
            Err(err) if err.code() == ErrorCode::UnbornBranch || err.code() == ErrorCode::NotFound => vec![],
            Err(err) => panic!("failed to resolve HEAD: {err}"),
        },
        Err(err) if err.code() == ErrorCode::UnbornBranch || err.code() == ErrorCode::NotFound => vec![],
        Err(err) => panic!("failed to read HEAD: {err}"),
    };
    let parent_refs: Vec<&Commit> = parents.iter().collect();

    repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parent_refs)
        .unwrap();
}
