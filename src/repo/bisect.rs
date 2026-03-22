//! Stateless bisect helpers using revision walks (`rev-list bad ^good` semantics).
//!
//! libgit2 does not expose a dedicated bisect API; this matches the commit set Git
//! considers "still suspect" and picks a midpoint for binary search.

use git2::{Oid, Repository, Sort};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

use crate::errors::py_git_err;

/// Resolve revision specs to commit OIDs (peels tags etc. to the underlying commit).
fn resolve_to_oids(repo: &Repository, specs: &[String]) -> PyResult<Vec<Oid>> {
    let mut oids = Vec::with_capacity(specs.len());
    for spec in specs {
        let obj = repo.revparse_single(spec).map_err(|err| {
            PyValueError::new_err(format!("invalid revision spec '{}': {}", spec, err))
        })?;
        oids.push(obj.id());
    }
    Ok(oids)
}

/// Commits reachable from any `bad` OID but not reachable from any `good` OID
/// (same idea as `git rev-list <bad>... --not <good>...` / `bad ^good`).
pub(crate) fn bisect_suspect_oids(
    repo: &Repository,
    good: &[Oid],
    bad: &[Oid],
    first_parent_only: bool,
) -> Result<Vec<Oid>, git2::Error> {
    let mut walk = repo.revwalk()?;
    walk.set_sorting(Sort::TOPOLOGICAL | Sort::TIME)?;
    if first_parent_only {
        walk.simplify_first_parent()?;
    }
    for b in bad {
        walk.push(*b)?;
    }
    for g in good {
        walk.hide(*g)?;
    }
    let mut out = Vec::new();
    for oid in walk {
        out.push(oid?);
    }
    Ok(out)
}

/// Next OID to check out and test (~middle of the suspect list in walk order).
pub(crate) fn bisect_next_oid(
    repo: &Repository,
    good: &[Oid],
    bad: &[Oid],
    first_parent_only: bool,
) -> Result<Option<Oid>, git2::Error> {
    let suspects = bisect_suspect_oids(repo, good, bad, first_parent_only)?;
    if suspects.is_empty() {
        return Ok(None);
    }
    let mid = suspects.len() / 2;
    Ok(Some(suspects[mid]))
}

pub(crate) fn bisect_suspects_py(
    repo: &Repository,
    good: Vec<String>,
    bad: Vec<String>,
    first_parent_only: bool,
) -> PyResult<Vec<String>> {
    if good.is_empty() {
        return Err(PyValueError::new_err(
            "bisect requires at least one known-good revision",
        ));
    }
    if bad.is_empty() {
        return Err(PyValueError::new_err(
            "bisect requires at least one known-bad revision",
        ));
    }
    let good_oids = resolve_to_oids(repo, &good)?;
    let bad_oids = resolve_to_oids(repo, &bad)?;
    let oids = bisect_suspect_oids(repo, &good_oids, &bad_oids, first_parent_only)
        .map_err(|e| py_git_err("bisect revwalk failed", e))?;
    Ok(oids.into_iter().map(|o| o.to_string()).collect())
}

pub(crate) fn bisect_next_py(
    repo: &Repository,
    good: Vec<String>,
    bad: Vec<String>,
    first_parent_only: bool,
) -> PyResult<Option<String>> {
    if good.is_empty() {
        return Err(PyValueError::new_err(
            "bisect requires at least one known-good revision",
        ));
    }
    if bad.is_empty() {
        return Err(PyValueError::new_err(
            "bisect requires at least one known-bad revision",
        ));
    }
    let good_oids = resolve_to_oids(repo, &good)?;
    let bad_oids = resolve_to_oids(repo, &bad)?;
    let next = bisect_next_oid(repo, &good_oids, &bad_oids, first_parent_only)
        .map_err(|e| py_git_err("bisect revwalk failed", e))?;
    Ok(next.map(|o| o.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::{Commit, Repository, Signature};
    use std::fs;
    use std::path::Path;
    use tempfile::tempdir;

    fn add_commit(repo: &Repository, parents: &[Oid], message: &str, content: &str) -> Oid {
        let sig = Signature::now("t", "t@t").unwrap();
        let wd = repo.workdir().expect("workdir");
        fs::write(wd.join("f.txt"), content).unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("f.txt")).unwrap();
        index.write().unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let parents: Vec<Commit<'_>> = parents
            .iter()
            .map(|o| repo.find_commit(*o).unwrap())
            .collect();
        let parent_refs: Vec<&Commit> = parents.iter().collect();
        repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parent_refs)
            .unwrap()
    }

    #[test]
    fn linear_suspects_excludes_good_and_ancestors() {
        let dir = tempdir().unwrap();
        let repo = Repository::init(dir.path()).unwrap();
        let a = add_commit(&repo, &[], "a", "1");
        let b = add_commit(&repo, &[a], "b", "2");
        let c = add_commit(&repo, &[b], "c", "3");
        let d = add_commit(&repo, &[c], "d", "4");

        let suspects = bisect_suspect_oids(&repo, &[a], &[d], false).unwrap();
        assert_eq!(suspects.len(), 3);
        assert_eq!(suspects.iter().filter(|o| **o == a).count(), 0);

        let next = bisect_next_oid(&repo, &[a], &[d], false).unwrap().unwrap();
        assert_eq!(next, c);
    }
}
