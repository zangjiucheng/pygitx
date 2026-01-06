use super::PyRepo;
use super::backup::{collect_head_refs, create_backup_refs, update_head};
use super::tree_ops::{build_globset, rewrite_tree_keep_paths, rewrite_tree_without_paths};
use super::util::{author_matches, str_contains_insensitive};
use crate::errors::py_git_err;
use crate::types::RewriteResult;
use git2::{BranchType, Commit, ErrorCode, Oid, Signature};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use std::collections::HashMap;
use std::path::Path;

pub(super) fn reword_commit(
    repo: &mut PyRepo,
    commit_id: &str,
    new_message: &str,
) -> PyResult<RewriteResult> {
    if new_message.trim().is_empty() {
        return Err(PyValueError::new_err("new_message cannot be empty"));
    }

    let target_oid = Oid::from_str(commit_id)
        .map_err(|_| PyValueError::new_err(format!("invalid commit id '{}'", commit_id)))?;
    let head_commit = repo.resolve_head_commit()?;

    // Collect commits from HEAD back to target along first-parent chain.
    let mut chain: Vec<Commit<'_>> = Vec::new();
    let mut current = head_commit.clone();
    loop {
        chain.push(current.clone());
        if current.id() == target_oid {
            break;
        }
        if current.parent_count() != 1 {
            return Err(PyValueError::new_err(
                "reword only supports linear history (first-parent traversal)",
            ));
        }
        current = current
            .parent(0)
            .map_err(|err| py_git_err("failed to walk parents during reword", err))?;
        if chain.len() > 10_000 {
            return Err(PyValueError::new_err(
                "reword traversal exceeded 10k commits; aborting",
            ));
        }
    }

    // Replay oldest -> newest, reusing original trees and metadata except for message on target.
    chain.reverse();
    let backup_refs = collect_head_refs(&repo.repo)?;
    let backup_root = create_backup_refs(&repo.repo, &backup_refs, None)?;

    let mut mapping: HashMap<Oid, Oid> = HashMap::new();

    for commit in &chain {
        let tree = commit
            .tree()
            .map_err(|err| py_git_err("failed to load tree during reword", err))?;

        let mut parent_refs: Vec<Commit<'_>> = Vec::new();
        for i in 0..commit.parent_count() {
            let parent = commit
                .parent(i)
                .map_err(|err| py_git_err("failed to load parent during reword", err))?;
            let rewritten_parent_oid = mapping.get(&parent.id()).copied().unwrap_or(parent.id());
            let rewritten_parent = repo
                .repo
                .find_commit(rewritten_parent_oid)
                .map_err(|err| py_git_err("failed to load rewritten parent during reword", err))?;
            parent_refs.push(rewritten_parent);
        }

        let msg = if commit.id() == target_oid {
            new_message
        } else {
            commit.message().unwrap_or("")
        };

        let parent_borrows: Vec<&Commit> = parent_refs.iter().collect();
        let new_oid = repo
            .repo
            .commit(
                None,
                &commit.author(),
                &commit.committer(),
                msg,
                &tree,
                &parent_borrows,
            )
            .map_err(|err| py_git_err("failed to create rewritten commit during reword", err))?;

        mapping.insert(commit.id(), new_oid);
    }

    let new_head_oid = mapping
        .get(&head_commit.id())
        .copied()
        .ok_or_else(|| PyValueError::new_err("failed to resolve rewritten HEAD after reword"))?;
    let updated_refs = update_head(&repo.repo, new_head_oid)?;

    Ok(RewriteResult::with_maps(
        mapping,
        updated_refs,
        Vec::new(),
        Some(backup_root),
    ))
}

pub(super) fn filter_commits(
    repo: &mut PyRepo,
    author: Option<&str>,
    message_contains: Option<&str>,
) -> PyResult<RewriteResult> {
    if author.is_none() && message_contains.is_none() {
        return Err(PyValueError::new_err(
            "must provide at least one filter: author or message_contains",
        ));
    }

    let head_commit = repo.resolve_head_commit()?;
    let backup_refs = collect_head_refs(&repo.repo)?;
    let backup_root = create_backup_refs(&repo.repo, &backup_refs, None)?;
    let mut revwalk = repo
        .repo
        .revwalk()
        .map_err(|err| py_git_err("failed to create revwalk", err))?;
    revwalk
        .push(head_commit.id())
        .map_err(|err| py_git_err("failed to start revwalk from HEAD", err))?;
    let _ = revwalk.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::REVERSE);

    let mut mapping: HashMap<Oid, Oid> = HashMap::new();

    for oid_result in revwalk {
        let oid =
            oid_result.map_err(|err| py_git_err("failed to read commit id from revwalk", err))?;
        let commit = repo
            .repo
            .find_commit(oid)
            .map_err(|err| py_git_err("failed to load commit", err))?;

        if commit.parent_count() > 1 {
            return Err(PyValueError::new_err(
                "filter_commits currently supports linear history only (merge commit encountered)",
            ));
        }

        let mut new_parents = Vec::new();
        for i in 0..commit.parent_count() {
            let parent = commit
                .parent(i)
                .map_err(|err| py_git_err("failed to load parent during filter", err))?;
            let rewritten_parent = mapping
                .get(&parent.id())
                .ok_or_else(|| PyValueError::new_err("missing parent mapping during filter"))?;
            let parent_commit = repo
                .repo
                .find_commit(*rewritten_parent)
                .map_err(|err| py_git_err("failed to load rewritten parent", err))?;
            new_parents.push(parent_commit);
        }

        let author_matches = author.map(|a| author_matches(&commit, a)).unwrap_or(false);
        let message_matches = message_contains
            .map(|m| str_contains_insensitive(commit.message().unwrap_or(""), m))
            .unwrap_or(false);
        let should_drop = author_matches || message_matches;

        if should_drop {
            if new_parents.is_empty() {
                return Err(PyValueError::new_err(
                    "cannot drop the root commit; no parent to re-parent to",
                ));
            }
            let replacement = new_parents[0].id();
            mapping.insert(commit.id(), replacement);
            continue;
        }

        let tree = commit
            .tree()
            .map_err(|err| py_git_err("failed to load commit tree", err))?;
        let parent_refs: Vec<&Commit> = new_parents.iter().collect();
        let new_oid = repo
            .repo
            .commit(
                None,
                &commit.author(),
                &commit.committer(),
                commit.message().unwrap_or(""),
                &tree,
                &parent_refs,
            )
            .map_err(|err| py_git_err("failed to create rewritten commit", err))?;

        mapping.insert(commit.id(), new_oid);
    }

    let new_head_oid = mapping
        .get(&head_commit.id())
        .ok_or_else(|| PyValueError::new_err("failed to resolve rewritten HEAD"))?;
    let updated_refs = update_head(&repo.repo, *new_head_oid)?;

    Ok(RewriteResult::with_maps(
        mapping,
        updated_refs,
        Vec::new(),
        Some(backup_root),
    ))
}

pub(super) fn remove_path(repo: &mut PyRepo, path_pattern: &str) -> PyResult<RewriteResult> {
    let matcher = build_globset(path_pattern)?;
    let head_commit = repo.resolve_head_commit()?;
    let backup_refs = collect_head_refs(&repo.repo)?;
    let backup_root = create_backup_refs(&repo.repo, &backup_refs, None)?;
    let mut revwalk = repo
        .repo
        .revwalk()
        .map_err(|err| py_git_err("failed to create revwalk", err))?;
    revwalk
        .push(head_commit.id())
        .map_err(|err| py_git_err("failed to start revwalk from HEAD", err))?;
    let _ = revwalk.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::REVERSE);

    let mut mapping: HashMap<Oid, Oid> = HashMap::new();

    for oid_result in revwalk {
        let oid =
            oid_result.map_err(|err| py_git_err("failed to read commit id from revwalk", err))?;
        let commit = repo
            .repo
            .find_commit(oid)
            .map_err(|err| py_git_err("failed to load commit", err))?;

        if commit.parent_count() > 1 {
            return Err(PyValueError::new_err(
                "remove_path currently supports linear history only (merge commit encountered)",
            ));
        }

        let mut new_parents = Vec::new();
        for i in 0..commit.parent_count() {
            let parent = commit
                .parent(i)
                .map_err(|err| py_git_err("failed to load parent during remove_path", err))?;
            let rewritten_parent = mapping.get(&parent.id()).ok_or_else(|| {
                PyValueError::new_err("missing parent mapping during remove_path")
            })?;
            let parent_commit = repo
                .repo
                .find_commit(*rewritten_parent)
                .map_err(|err| py_git_err("failed to load rewritten parent", err))?;
            new_parents.push(parent_commit);
        }

        let tree = commit
            .tree()
            .map_err(|err| py_git_err("failed to load commit tree", err))?;
        let maybe_new_tree =
            rewrite_tree_without_paths(&repo.repo, &tree, &matcher, Path::new(""))?;
        let final_tree = match maybe_new_tree {
            Some(oid) => repo
                .repo
                .find_tree(oid)
                .map_err(|err| py_git_err("failed to load rewritten tree", err))?,
            None => tree,
        };

        let parent_refs: Vec<&Commit> = new_parents.iter().collect();
        let new_oid = repo
            .repo
            .commit(
                None,
                &commit.author(),
                &commit.committer(),
                commit.message().unwrap_or(""),
                &final_tree,
                &parent_refs,
            )
            .map_err(|err| py_git_err("failed to create rewritten commit", err))?;

        mapping.insert(commit.id(), new_oid);
    }

    let new_head_oid = mapping
        .get(&head_commit.id())
        .ok_or_else(|| PyValueError::new_err("failed to resolve rewritten HEAD"))?;
    let updated_refs = update_head(&repo.repo, *new_head_oid)?;

    Ok(RewriteResult::with_maps(
        mapping,
        updated_refs,
        Vec::new(),
        Some(backup_root),
    ))
}

pub(super) fn keep_path(repo: &mut PyRepo, glob_pattern: &str) -> PyResult<RewriteResult> {
    let matcher = build_globset(glob_pattern)?;
    let head_commit = repo.resolve_head_commit()?;
    let backup_refs = collect_head_refs(&repo.repo)?;
    let backup_root = create_backup_refs(&repo.repo, &backup_refs, None)?;
    let mut revwalk = repo
        .repo
        .revwalk()
        .map_err(|err| py_git_err("failed to create revwalk", err))?;
    revwalk
        .push(head_commit.id())
        .map_err(|err| py_git_err("failed to start revwalk from HEAD", err))?;
    let _ = revwalk.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::REVERSE);

    let mut mapping: HashMap<Oid, Oid> = HashMap::new();

    for oid_result in revwalk {
        let oid =
            oid_result.map_err(|err| py_git_err("failed to read commit id from revwalk", err))?;
        let commit = repo
            .repo
            .find_commit(oid)
            .map_err(|err| py_git_err("failed to load commit", err))?;

        if commit.parent_count() > 1 {
            return Err(PyValueError::new_err(
                "keep_path currently supports linear history only (merge commit encountered)",
            ));
        }

        let mut new_parents = Vec::new();
        for i in 0..commit.parent_count() {
            let parent = commit
                .parent(i)
                .map_err(|err| py_git_err("failed to load parent during keep_path", err))?;
            let rewritten_parent = mapping
                .get(&parent.id())
                .ok_or_else(|| PyValueError::new_err("missing parent mapping during keep_path"))?;
            let parent_commit = repo
                .repo
                .find_commit(*rewritten_parent)
                .map_err(|err| py_git_err("failed to load rewritten parent", err))?;
            new_parents.push(parent_commit);
        }

        let tree = commit
            .tree()
            .map_err(|err| py_git_err("failed to load commit tree", err))?;
        let maybe_new_tree = rewrite_tree_keep_paths(&repo.repo, &tree, &matcher, Path::new(""))?;
        let final_tree = match maybe_new_tree {
            Some(oid) => repo
                .repo
                .find_tree(oid)
                .map_err(|err| py_git_err("failed to load rewritten tree", err))?,
            None => {
                // If nothing matches, create an empty tree.
                let builder = repo
                    .repo
                    .treebuilder(None)
                    .map_err(|err| py_git_err("failed to create empty tree", err))?;
                let empty = builder
                    .write()
                    .map_err(|err| py_git_err("failed to write empty tree", err))?;
                repo.repo
                    .find_tree(empty)
                    .map_err(|err| py_git_err("failed to load empty tree", err))?
            }
        };

        let parent_refs: Vec<&Commit> = new_parents.iter().collect();
        let new_oid = repo
            .repo
            .commit(
                None,
                &commit.author(),
                &commit.committer(),
                commit.message().unwrap_or(""),
                &final_tree,
                &parent_refs,
            )
            .map_err(|err| py_git_err("failed to create rewritten commit", err))?;

        mapping.insert(commit.id(), new_oid);
    }

    let new_head_oid = mapping
        .get(&head_commit.id())
        .ok_or_else(|| PyValueError::new_err("failed to resolve rewritten HEAD"))?;
    let updated_refs = update_head(&repo.repo, *new_head_oid)?;

    Ok(RewriteResult::with_maps(
        mapping,
        updated_refs,
        Vec::new(),
        Some(backup_root),
    ))
}

pub(super) fn change_commit_message(
    repo: &mut PyRepo,
    commit_id: &str,
    new_message: &str,
) -> PyResult<RewriteResult> {
    if new_message.trim().is_empty() {
        return Err(PyValueError::new_err("new commit message cannot be empty"));
    }

    let head_ref = match repo.repo.head() {
        Ok(reference) => reference,
        Err(err) if err.code() == ErrorCode::UnbornBranch || err.code() == ErrorCode::NotFound => {
            return Err(PyValueError::new_err(
                "cannot amend commit: repository has no HEAD commit",
            ));
        }
        Err(err) => return Err(py_git_err("failed to read HEAD", err)),
    };

    let head_commit = match head_ref.peel_to_commit() {
        Ok(commit) => commit,
        Err(err) if err.code() == ErrorCode::UnbornBranch || err.code() == ErrorCode::NotFound => {
            return Err(PyValueError::new_err(
                "cannot amend commit: repository has no HEAD commit",
            ));
        }
        Err(err) => return Err(py_git_err("failed to resolve HEAD to commit", err)),
    };

    let target_obj = repo.repo.revparse_single(commit_id).map_err(|err| {
        PyValueError::new_err(format!("invalid commit id '{}': {}", commit_id, err))
    })?;
    let target_commit = target_obj.peel_to_commit().map_err(|err| {
        PyValueError::new_err(format!("object '{}' is not a commit: {}", commit_id, err))
    })?;

    if target_commit.id() != head_commit.id() {
        return Err(PyValueError::new_err(
            "changing commit messages is currently supported only for HEAD; rebase needed for older commits",
        ));
    }

    let backup_refs = collect_head_refs(&repo.repo)?;
    let backup_root = create_backup_refs(&repo.repo, &backup_refs, None)?;
    let old_oid = target_commit.id();
    let new_oid = target_commit
        .amend(None, None, None, None, Some(new_message), None)
        .map_err(|err| py_git_err("failed to amend commit message", err))?;

    let updated_refs = update_head(&repo.repo, new_oid)?;
    let mut result = RewriteResult::new();
    result.add_mapping(old_oid, new_oid);
    for (name, oid) in updated_refs {
        result.add_updated_ref(name, oid);
    }
    result.set_backup_root(backup_root);

    Ok(result)
}

pub(super) fn rewrite_author(
    repo: &mut PyRepo,
    commit_id: &str,
    new_name: &str,
    new_email: &str,
    update_committer: Option<bool>,
) -> PyResult<RewriteResult> {
    if new_name.trim().is_empty() {
        return Err(PyValueError::new_err("new author name cannot be empty"));
    }
    if new_email.trim().is_empty() {
        return Err(PyValueError::new_err("new author email cannot be empty"));
    }

    let head_ref = match repo.repo.head() {
        Ok(reference) => reference,
        Err(err) if err.code() == ErrorCode::UnbornBranch || err.code() == ErrorCode::NotFound => {
            return Err(PyValueError::new_err(
                "cannot rewrite author: repository has no HEAD commit",
            ));
        }
        Err(err) => return Err(py_git_err("failed to read HEAD", err)),
    };

    let head_commit = match head_ref.peel_to_commit() {
        Ok(commit) => commit,
        Err(err) if err.code() == ErrorCode::UnbornBranch || err.code() == ErrorCode::NotFound => {
            return Err(PyValueError::new_err(
                "cannot rewrite author: repository has no HEAD commit",
            ));
        }
        Err(err) => return Err(py_git_err("failed to resolve HEAD to commit", err)),
    };

    let target_obj = repo.repo.revparse_single(commit_id).map_err(|err| {
        PyValueError::new_err(format!("invalid commit id '{}': {}", commit_id, err))
    })?;
    let target_commit = target_obj.peel_to_commit().map_err(|err| {
        PyValueError::new_err(format!("object '{}' is not a commit: {}", commit_id, err))
    })?;

    if target_commit.id() != head_commit.id() {
        return Err(PyValueError::new_err(
            "rewriting author is currently supported only for HEAD; rebase needed for older commits",
        ));
    }

    let backup_refs = collect_head_refs(&repo.repo)?;
    let backup_root = create_backup_refs(&repo.repo, &backup_refs, None)?;
    let old_oid = target_commit.id();
    let author_time = target_commit.author().when();
    let new_author = Signature::new(new_name, new_email, &author_time)
        .map_err(|err| PyValueError::new_err(format!("invalid author identity: {}", err)))?;
    let committer_sig = if update_committer.unwrap_or(true) {
        Some(
            Signature::new(new_name, new_email, &target_commit.committer().when()).map_err(
                |err| PyValueError::new_err(format!("invalid committer identity: {}", err)),
            )?,
        )
    } else {
        None
    };

    let new_oid = target_commit
        .amend(
            None,
            Some(&new_author),
            committer_sig.as_ref(),
            None,
            None,
            None,
        )
        .map_err(|err| py_git_err("failed to rewrite author", err))?;

    let updated_refs = update_head(&repo.repo, new_oid)?;
    let mut result = RewriteResult::new();
    result.add_mapping(old_oid, new_oid);
    for (name, oid) in updated_refs {
        result.add_updated_ref(name, oid);
    }
    result.set_backup_root(backup_root);

    Ok(result)
}

pub(super) fn squash_last_commits(
    repo: &mut PyRepo,
    count: usize,
    mode: Option<&str>,
    message: Option<&str>,
) -> PyResult<RewriteResult> {
    if count < 2 {
        return Err(PyValueError::new_err(
            "count must be at least 2 to squash commits",
        ));
    }

    let squash_mode = match mode.unwrap_or("squash") {
        "squash" => SquashMode::Squash,
        "fixup" => SquashMode::Fixup,
        other => {
            return Err(PyValueError::new_err(format!(
                "unknown squash mode '{}'; expected 'squash' or 'fixup'",
                other
            )));
        }
    };

    let head_ref = repo.repo.head().map_err(|err| {
        PyValueError::new_err(format!("cannot squash: repository has no HEAD ({})", err))
    })?;
    let head_commit = head_ref.peel_to_commit().map_err(|err| {
        PyValueError::new_err(format!(
            "cannot squash: failed to resolve HEAD to commit ({})",
            err
        ))
    })?;
    let backup_refs = collect_head_refs(&repo.repo)?;
    let backup_root = create_backup_refs(&repo.repo, &backup_refs, None)?;

    // Walk first-parents to collect the range (newest -> oldest).
    let mut commits: Vec<Commit<'_>> = Vec::with_capacity(count);
    let mut current = head_commit;
    commits.push(current.clone());

    for _ in 1..count {
        match current.parent_count() {
            0 => {
                return Err(PyValueError::new_err(format!(
                    "not enough commits to squash: repository has only {} commit(s)",
                    commits.len()
                )));
            }
            1 => {}
            _ => {
                return Err(PyValueError::new_err(
                    "squash_last only supports linear history (merge commits not supported in range)",
                ));
            }
        }

        current = current
            .parent(0)
            .map_err(|err| py_git_err("failed to load commit parent during squash", err))?;
        commits.push(current.clone());
    }

    // Oldest commit in the range is last; its parent becomes the parent of the squashed commit.
    let oldest = commits
        .last()
        .ok_or_else(|| PyValueError::new_err("unexpected empty commit range during squash"))?;
    let parent_commit = if oldest.parent_count() == 0 {
        None
    } else {
        Some(
            oldest
                .parent(0)
                .map_err(|err| py_git_err("failed to load parent when building squash", err))?,
        )
    };

    let combined_message = match message {
        Some(msg) => msg.to_string(),
        None => compose_squash_message(&commits, squash_mode),
    };

    // Resulting tree is the tree of the newest commit in the range.
    let tree = commits[0]
        .tree()
        .map_err(|err| py_git_err("failed to load tree for squashed commit", err))?;

    let mut parent_refs: Vec<&Commit> = Vec::new();
    if let Some(ref parent) = parent_commit {
        parent_refs.push(parent);
    }

    let new_oid = repo
        .repo
        .commit(
            None,
            &commits[0].author(),
            &commits[0].committer(),
            &combined_message,
            &tree,
            &parent_refs,
        )
        .map_err(|err| py_git_err("failed to create squashed commit", err))?;

    let updated_refs = update_head(&repo.repo, new_oid)?;
    let mut mapping = HashMap::new();
    for commit in &commits {
        mapping.insert(commit.id(), new_oid);
    }

    Ok(RewriteResult::with_maps(
        mapping,
        updated_refs,
        Vec::new(),
        Some(backup_root),
    ))
}

pub(super) fn rebase_branch(
    repo: &mut PyRepo,
    branch: &str,
    onto: &str,
) -> PyResult<RewriteResult> {
    let branch_ref = repo
        .repo
        .find_branch(branch, BranchType::Local)
        .map_err(|err| PyValueError::new_err(format!("unknown branch '{}': {}", branch, err)))?;
    let branch_name = branch_ref
        .get()
        .name()
        .ok_or_else(|| PyValueError::new_err("branch name is not valid utf-8"))?
        .to_string();
    let branch_tip = branch_ref
        .get()
        .target()
        .ok_or_else(|| PyValueError::new_err("branch has no target oid"))?;

    let onto_obj = repo
        .repo
        .revparse_single(onto)
        .map_err(|err| PyValueError::new_err(format!("invalid onto '{}': {}", onto, err)))?;
    let onto_commit = onto_obj.peel_to_commit().map_err(|err| {
        PyValueError::new_err(format!("onto '{}' is not a commit: {}", onto, err))
    })?;

    let head_points_to_branch = repo
        .repo
        .head()
        .ok()
        .and_then(|h| h.name().map(|name| name == branch_name))
        .unwrap_or(false);

    // Collect commits reachable from branch but not from onto (oldest first).
    let mut revwalk = repo
        .repo
        .revwalk()
        .map_err(|err| py_git_err("failed to start revwalk for rebase", err))?;
    revwalk
        .push(branch_tip)
        .map_err(|err| py_git_err("failed to push branch tip for rebase", err))?;
    revwalk
        .hide(onto_commit.id())
        .map_err(|err| py_git_err("failed to hide onto for rebase", err))?;
    let _ = revwalk.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::REVERSE);

    let mut to_replay = Vec::new();
    for oid_result in revwalk {
        let oid = oid_result
            .map_err(|err| py_git_err("failed to read commit during rebase walk", err))?;
        to_replay.push(oid);
    }

    let mut mapping: HashMap<Oid, Oid> = HashMap::new();
    let mut current_tip = repo
        .repo
        .find_commit(onto_commit.id())
        .map_err(|err| py_git_err("failed to load onto commit", err))?;
    let mut last_new_oid: Option<Oid> = None;
    let mut backup_refs = vec![(branch_name.clone(), branch_tip)];
    if head_points_to_branch {
        backup_refs.push(("HEAD".to_string(), branch_tip));
    }
    let backup_root = create_backup_refs(&repo.repo, &backup_refs, None)?;

    for oid in to_replay {
        let commit = repo
            .repo
            .find_commit(oid)
            .map_err(|err| py_git_err("failed to load commit during rebase", err))?;
        let parent = commit
            .parent(0)
            .map_err(|err| py_git_err("failed to load commit parent during rebase", err))?;

        let ancestor_tree = parent
            .tree()
            .map_err(|err| py_git_err("failed to load ancestor tree", err))?;
        let current_tree = current_tip
            .tree()
            .map_err(|err| py_git_err("failed to load current tree", err))?;
        let commit_tree = commit
            .tree()
            .map_err(|err| py_git_err("failed to load commit tree", err))?;

        let mut idx = repo
            .repo
            .merge_trees(&ancestor_tree, &current_tree, &commit_tree, None)
            .map_err(|err| py_git_err("failed to merge trees during rebase", err))?;
        if idx.has_conflicts() {
            return Err(py_git_err(
                "conflicts detected during rebase; rebase aborted",
                git2::Error::from_str("merge conflicts"),
            ));
        }

        let tree_oid = idx
            .write_tree_to(&repo.repo)
            .map_err(|err| py_git_err("failed to write tree during rebase", err))?;
        let tree = repo
            .repo
            .find_tree(tree_oid)
            .map_err(|err| py_git_err("failed to load rewritten tree", err))?;

        let new_oid = repo
            .repo
            .commit(
                None,
                &commit.author(),
                &commit.committer(),
                commit.message().unwrap_or("rebase: pick"),
                &tree,
                &[&current_tip],
            )
            .map_err(|err| py_git_err("failed to create commit during rebase", err))?;

        mapping.insert(commit.id(), new_oid);
        last_new_oid = Some(new_oid);
        current_tip = repo
            .repo
            .find_commit(new_oid)
            .map_err(|err| py_git_err("failed to load rewritten commit", err))?;
    }

    let mut updated_refs = HashMap::new();
    if let Some(new_oid) = last_new_oid {
        let mut reference = branch_ref.into_reference();
        reference
            .set_target(new_oid, "rebase: update branch")
            .map_err(|err| py_git_err("failed to update branch after rebase", err))?;
        updated_refs.insert(branch_name.clone(), new_oid);

        if head_points_to_branch {
            repo.repo
                .set_head(&branch_name)
                .map_err(|err| py_git_err("failed to update HEAD after rebase", err))?;
            updated_refs.insert("HEAD".to_string(), new_oid);
        }
    }

    Ok(RewriteResult::with_maps(
        mapping,
        updated_refs,
        Vec::new(),
        Some(backup_root),
    ))
}

#[derive(Copy, Clone)]
enum SquashMode {
    Squash,
    Fixup,
}

fn compose_squash_message(commits: &[Commit<'_>], mode: SquashMode) -> String {
    if commits.is_empty() {
        return "squashed commit".to_string();
    }

    match mode {
        SquashMode::Squash => {
            let parts: Vec<String> = commits
                .iter()
                .rev() // oldest -> newest
                .filter_map(|c| c.message().map(str::trim).filter(|m| !m.is_empty()))
                .map(|m| m.to_string())
                .collect();

            if parts.is_empty() {
                "squashed commit".to_string()
            } else {
                parts.join("\n\n")
            }
        }
        SquashMode::Fixup => commits
            .last()
            .and_then(|c| c.summary().map(|s| s.to_string()))
            .unwrap_or_else(|| "fixup commit".to_string()),
    }
}
