use crate::errors::py_git_err;
use git2::{ObjectType, Oid, Repository, Tree};
use globset::{Glob, GlobSet, GlobSetBuilder};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use std::path::Path;

pub(super) fn build_globset(pattern: &str) -> PyResult<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    let glob = Glob::new(pattern).map_err(|err| {
        PyValueError::new_err(format!("invalid path pattern '{}': {}", pattern, err))
    })?;
    builder.add(glob);
    builder
        .build()
        .map_err(|err| PyValueError::new_err(format!("failed to build globset: {}", err)))
}

pub(super) fn rewrite_tree_without_paths(
    repo: &Repository,
    tree: &Tree<'_>,
    matcher: &GlobSet,
    base: &Path,
) -> PyResult<Option<Oid>> {
    let mut builder = repo
        .treebuilder(Some(tree))
        .map_err(|err| py_git_err("failed to create treebuilder", err))?;
    let mut changed = false;

    for entry in tree.iter() {
        let name = match entry.name() {
            Some(n) => n,
            None => continue,
        };
        let full_path = base.join(name);
        let path_str = full_path.to_string_lossy();

        if matcher.is_match(path_str.as_ref()) {
            builder
                .remove(name)
                .map_err(|err| py_git_err("failed to remove matched path", err))?;
            changed = true;
            continue;
        }

        if entry.kind() == Some(ObjectType::Tree) {
            let child_tree = repo
                .find_tree(entry.id())
                .map_err(|err| py_git_err("failed to load subtree", err))?;
            if let Some(new_child_oid) =
                rewrite_tree_without_paths(repo, &child_tree, matcher, &full_path)?
            {
                builder
                    .insert(name, new_child_oid, entry.filemode())
                    .map_err(|err| py_git_err("failed to replace subtree", err))?;
                changed = true;
            }
        }
    }

    if changed {
        let oid = builder
            .write()
            .map_err(|err| py_git_err("failed to write rewritten tree", err))?;
        Ok(Some(oid))
    } else {
        Ok(None)
    }
}

pub(super) fn rewrite_tree_keep_paths(
    repo: &Repository,
    tree: &Tree<'_>,
    matcher: &GlobSet,
    base: &Path,
) -> PyResult<Option<Oid>> {
    // Build a new tree containing only entries that match the glob (or contain matching descendants).
    let mut builder = repo
        .treebuilder(None)
        .map_err(|err| py_git_err("failed to create treebuilder", err))?;
    let mut kept_any = false;

    for entry in tree.iter() {
        let name = match entry.name() {
            Some(n) => n,
            None => continue,
        };
        let full_path = base.join(name);
        let path_str = full_path.to_string_lossy();

        match entry.kind() {
            Some(ObjectType::Tree) => {
                // If the directory matches directly, keep it intact.
                if matcher.is_match(path_str.as_ref()) {
                    builder
                        .insert(name, entry.id(), entry.filemode())
                        .map_err(|err| py_git_err("failed to keep matching tree", err))?;
                    kept_any = true;
                    continue;
                }
                let child_tree = repo
                    .find_tree(entry.id())
                    .map_err(|err| py_git_err("failed to load subtree", err))?;
                if let Some(new_child_oid) =
                    rewrite_tree_keep_paths(repo, &child_tree, matcher, &full_path)?
                {
                    builder
                        .insert(name, new_child_oid, entry.filemode())
                        .map_err(|err| py_git_err("failed to insert rewritten subtree", err))?;
                    kept_any = true;
                }
            }
            Some(_) => {
                if matcher.is_match(path_str.as_ref()) {
                    builder
                        .insert(name, entry.id(), entry.filemode())
                        .map_err(|err| py_git_err("failed to keep matching entry", err))?;
                    kept_any = true;
                }
            }
            None => continue,
        }
    }

    if !kept_any {
        return Ok(None);
    }

    let oid = builder
        .write()
        .map_err(|err| py_git_err("failed to write rewritten tree", err))?;
    Ok(Some(oid))
}
