use crate::errors::py_git_err;
use git2::{ErrorCode, Oid, Repository};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use std::collections::HashMap;
use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};

pub(super) fn update_head(repo: &Repository, new_oid: Oid) -> PyResult<HashMap<String, Oid>> {
    let mut updated_refs = HashMap::new();
    match repo.head() {
        Ok(mut head_ref) => {
            let name = head_ref.name().map(|s| s.to_string());
            if let Some(name) = name {
                head_ref
                    .set_target(new_oid, "history rewrite")
                    .map_err(|err| py_git_err("failed to update ref after rewrite", err))?;
                repo.set_head(&name)
                    .map_err(|err| py_git_err("failed to update HEAD after rewrite", err))?;
                updated_refs.insert(name, new_oid);
            } else {
                repo.set_head_detached(new_oid)
                    .map_err(|err| py_git_err("failed to detach HEAD to rewritten commit", err))?;
            }
        }
        Err(_) => {
            repo.set_head_detached(new_oid)
                .map_err(|err| py_git_err("failed to detach HEAD to rewritten commit", err))?;
        }
    }
    updated_refs.insert("HEAD".to_string(), new_oid);
    Ok(updated_refs)
}

pub(super) fn collect_head_refs(repo: &Repository) -> PyResult<Vec<(String, Oid)>> {
    let mut refs = Vec::new();
    let head_ref = repo.head().map_err(|err| match err.code() {
        ErrorCode::UnbornBranch | ErrorCode::NotFound => {
            PyValueError::new_err("repository has no HEAD commit")
        }
        _ => py_git_err("failed to read HEAD", err),
    })?;
    let head_commit = head_ref
        .peel_to_commit()
        .map_err(|err| py_git_err("failed to resolve HEAD to commit", err))?;
    refs.push(("HEAD".to_string(), head_commit.id()));
    if let Some(name) = head_ref.name().map(|s| s.to_string()) {
        refs.push((name, head_commit.id()));
    }
    Ok(refs)
}

pub(super) fn create_backup_refs(
    repo: &Repository,
    refs: &[(String, Oid)],
    prefix: Option<&str>,
) -> PyResult<String> {
    let base = prefix.unwrap_or("refs/pygitx/backup").trim();
    if base.is_empty() {
        return Err(PyValueError::new_err("backup prefix cannot be empty"));
    }
    let base = if base.starts_with("refs/") {
        base.to_string()
    } else {
        format!("refs/{}", base)
    };
    let base = base.trim_end_matches('/').to_string();
    if refs.is_empty() {
        return Err(PyValueError::new_err("no references to back up"));
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| std::time::Duration::from_secs(0))
        .as_secs();
    let root = format!("{}/{}", base, now);

    let mut seen = HashSet::new();
    for (name, oid) in refs {
        let sanitized = name.trim_start_matches('/');
        if !seen.insert(sanitized.to_string()) {
            continue;
        }
        let backup_name = format!("{}/{}", root, sanitized);
        repo.reference(&backup_name, *oid, true, "pygitx backup")
            .map_err(|err| py_git_err("failed to create backup ref", err))?;
    }

    Ok(root)
}
