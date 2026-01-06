use crate::errors::py_git_err;
use crate::types::PyRepoSummary;
use git2::{BranchType, ObjectType, Repository, Tree};
use pyo3::prelude::*;
use std::collections::HashSet;

pub(super) fn summarize_repo(repo: &Repository) -> PyResult<PyRepoSummary> {
    let path = repo
        .path()
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "<unknown>".to_string());

    let mut head_oid = None;
    let mut branch = None;
    let mut last_commit_time = None;
    let mut commits = 0usize;
    let mut authors: HashSet<(String, String)> = HashSet::new();

    if let Ok(head_ref) = repo.head() {
        if head_ref.is_branch() {
            if let Some(name) = head_ref.shorthand() {
                branch = Some(name.to_string());
            }
        }
        if let Some(target) = head_ref.target() {
            head_oid = Some(target);
            let head_commit = repo.find_commit(target).ok();
            if let Some(commit) = head_commit.as_ref() {
                last_commit_time = Some(commit.time().seconds());
            }
            let mut revwalk = repo
                .revwalk()
                .map_err(|err| py_git_err("failed to create revwalk", err))?;
            revwalk
                .push(target)
                .map_err(|err| py_git_err("failed to start revwalk from HEAD", err))?;
            for oid_result in revwalk {
                let oid = oid_result
                    .map_err(|err| py_git_err("failed to read oid during summary", err))?;
                commits += 1;
                if let Ok(commit) = repo.find_commit(oid) {
                    let sig = commit.author();
                    let name = sig.name().unwrap_or("").to_string();
                    let email = sig.email().unwrap_or("").to_string();
                    authors.insert((name, email));
                }
            }
        }
    }

    let branches = repo
        .branches(Some(BranchType::Local))
        .map(|iter| iter.filter_map(Result::ok).count())
        .unwrap_or(0);
    let tags = repo.tag_names(None).map(|t| t.len()).unwrap_or(0);
    let remotes = repo.remotes().map(|r| r.len()).unwrap_or(0);

    let files = head_oid
        .and_then(|oid| repo.find_commit(oid).ok())
        .and_then(|c| c.tree().ok())
        .map(|tree| count_tree_entries(repo, &tree).unwrap_or(0))
        .unwrap_or(0);

    let size_kb = estimate_repo_size_kb(repo)?;

    let is_dirty = repo.statuses(None).map(|s| !s.is_empty()).unwrap_or(false);

    let head_short = head_oid.map(|oid| {
        let s = oid.to_string();
        s.chars().take(7).collect()
    });

    Ok(PyRepoSummary::new(
        path,
        head_short,
        branch,
        commits,
        branches,
        tags,
        remotes,
        authors.len(),
        files,
        size_kb,
        is_dirty,
        last_commit_time,
    ))
}

fn count_tree_entries(repo: &Repository, tree: &Tree<'_>) -> PyResult<usize> {
    let mut count = 0usize;
    for entry in tree.iter() {
        match entry.kind() {
            Some(ObjectType::Blob) => {
                count += 1;
            }
            Some(ObjectType::Tree) => {
                let child = repo
                    .find_tree(entry.id())
                    .map_err(|err| py_git_err("failed to load subtree", err))?;
                count += count_tree_entries(repo, &child)?;
            }
            _ => {}
        }
    }
    Ok(count)
}

fn estimate_repo_size_kb(repo: &Repository) -> PyResult<usize> {
    let odb = repo
        .odb()
        .map_err(|err| py_git_err("failed to open odb", err))?;
    let mut total: u64 = 0;
    odb.foreach(|oid| {
        if let Ok((_kind, size)) = odb.read_header(*oid) {
            total = total.saturating_add(size as u64);
        }
        true
    })
    .map_err(|err| py_git_err("failed to walk odb", err))?;
    Ok(((total + 1023) / 1024) as usize)
}
