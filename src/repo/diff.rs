use crate::errors::py_git_err;
use crate::types::PyDiffStat;
use git2::{DiffOptions, Oid, Repository, Tree};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

pub(super) fn diff_stat(
    repo: &Repository,
    a: Oid,
    b: Oid,
    paths: Option<Vec<String>>,
) -> PyResult<PyDiffStat> {
    let a_tree = tree_from_commit(repo, a)?;
    let b_tree = tree_from_commit(repo, b)?;

    let mut opts = DiffOptions::new();
    if let Some(paths) = paths {
        for p in paths {
            opts.pathspec(p);
        }
    }

    let diff = repo
        .diff_tree_to_tree(Some(&a_tree), Some(&b_tree), Some(&mut opts))
        .map_err(|err| py_git_err("failed to compute diff", err))?;

    let stats = diff
        .stats()
        .map_err(|err| py_git_err("failed to compute diff stats", err))?;
    let files_changed = stats.files_changed();
    let insertions = stats.insertions();
    let deletions = stats.deletions();

    let mut paths_out = Vec::new();
    for delta in diff.deltas() {
        if let Some(path) = delta.new_file().path().or_else(|| delta.old_file().path()) {
            paths_out.push(path.to_string_lossy().to_string());
        }
    }

    Ok(PyDiffStat::new(
        files_changed,
        insertions,
        deletions,
        paths_out,
    ))
}

fn tree_from_commit(repo: &Repository, oid: Oid) -> PyResult<Tree<'_>> {
    let commit = repo
        .find_commit(oid)
        .map_err(|err| PyValueError::new_err(format!("unknown commit '{}': {}", oid, err)))?;
    commit
        .tree()
        .map_err(|err| py_git_err("failed to load commit tree", err))
}
