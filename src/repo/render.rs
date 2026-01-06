use crate::errors::py_git_err;
use chrono::Utc;
use git2::{BranchType, ErrorCode, Oid, Repository};
use pyo3::prelude::*;
use std::collections::{HashMap, HashSet};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

// Column widths for refs table output
const REF_NAME_WIDTH: usize = 22;
const REF_OID_WIDTH: usize = 7;
const REF_AGE_WIDTH: usize = 6;
const REF_AUTHOR_WIDTH: usize = 16;
// Number of single-space separators between the 5 columns (name, oid, age, author, summary)
const REF_SEPARATORS: usize = 4;

struct RefRow {
    name: String,
    oid7: String,
    age: String,
    author: String,
    summary: String,
}

pub(super) fn render_refs(
    repo: &Repository,
    local: bool,
    remote: bool,
    tags: bool,
    max_width: Option<usize>,
) -> PyResult<String> {
    if !local && !remote && !tags {
        return Ok(String::new());
    }
    let width = terminal_width(max_width);
    let mut rows = Vec::new();

    if local {
        collect_branches(repo, BranchType::Local, &mut rows)?;
    }
    if remote {
        collect_branches(repo, BranchType::Remote, &mut rows)?;
    }
    if tags {
        collect_tags(repo, &mut rows)?;
    }

    rows.sort_by(|a, b| a.name.cmp(&b.name));

    // Total width of all fixed-width columns plus REF_SEPARATORS single-space separators between the 5 columns.
    let fixed_columns_width = REF_NAME_WIDTH + REF_OID_WIDTH + REF_AGE_WIDTH + REF_AUTHOR_WIDTH + REF_SEPARATORS;
    // Ensure there is always at least one character available for the summary
    let min_width = fixed_columns_width + 1;
    let effective_width = if width < min_width { min_width } else { width };
    let summary_w = effective_width - fixed_columns_width;

    let mut out = String::new();
    for row in rows {
        let line = format!(
            "{:<name_w$} {:<oid_w$} {:<age_w$} {:<author_w$} {}",
            truncate(&row.name, REF_NAME_WIDTH),
            row.oid7,
            truncate(&row.age, REF_AGE_WIDTH),
            truncate(&row.author, REF_AUTHOR_WIDTH),
            truncate(&row.summary, summary_w),
            name_w = REF_NAME_WIDTH,
            oid_w = REF_OID_WIDTH,
            age_w = REF_AGE_WIDTH,
            author_w = REF_AUTHOR_WIDTH,
        );
        out.push_str(&line);
        out.push('\n');
    }
    Ok(out.trim_end().to_string())
}

pub(super) fn render_log(
    repo: &Repository,
    start: Oid,
    max_commits: usize,
    decorate: bool,
    graph: bool,
    max_width: Option<usize>,
) -> PyResult<String> {
    let width = terminal_width(max_width);
    let mut revwalk = repo.revwalk().map_err(|err| py_git_err("revwalk", err))?;
    revwalk
        .push(start)
        .map_err(|err| py_git_err("revwalk push", err))?;
    revwalk
        .set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::TIME)
        .map_err(|err| py_git_err("revwalk sort", err))?;

    let decorations = collect_decorations(repo)?;
    let head_target = repo.head().ok().and_then(|h| h.target());

    let mut lanes: Vec<Oid> = Vec::new();
    let mut lines = Vec::new();

    for (idx, oid_res) in revwalk.enumerate() {
        if idx >= max_commits {
            break;
        }
        let oid = oid_res.map_err(|err| py_git_err("revwalk oid", err))?;
        let commit = repo
            .find_commit(oid)
            .map_err(|err| py_git_err("find commit", err))?;
        let parents: Vec<Oid> = commit.parents().map(|p| p.id()).collect();

        let lane_idx = match lanes.iter().position(|o| *o == oid) {
            Some(i) => i,
            None => {
                lanes.push(oid);
                lanes.len() - 1
            }
        };

        let graph_prefix = if graph {
            render_graph_prefix(lanes.len(), lane_idx, parents.len())
        } else {
            String::new()
        };

        let mut deco: Vec<String> = decorations
            .get(&oid)
            .cloned()
            .unwrap_or_else(Vec::new);
        if Some(oid) == head_target {
            deco.push("HEAD".to_string());
        }
        deco.sort();
        let deco_str = if decorate && !deco.is_empty() {
            format!("[{}]", deco.join(", "))
        } else {
            String::new()
        };

        let oid7 = oid.to_string()[0..7].to_string();
        let mut summary = commit
            .summary()
            .unwrap_or("<no message>")
            .lines()
            .next()
            .unwrap_or("<no message>")
            .to_string();
        let available = {
            let base = width
                .saturating_sub(graph_prefix.len() + 1 + oid7.len() + 1 + deco_str.len() + 1);
            std::cmp::max(base, 1)
        };
        summary = truncate(&summary, available);

        let line = if deco_str.is_empty() {
            format!("{} {} {}", graph_prefix, oid7, summary).trim().to_string()
        } else {
            format!("{} {} {} {}", graph_prefix, oid7, deco_str, summary)
                .trim()
                .to_string()
        };
        lines.push(line);

        // Update lanes: replace current lane with parents (first parent stays in place).
        if !parents.is_empty() {
            // Normal case: current commit is followed by one or more parent commits.
            lanes.remove(lane_idx);
            for parent in parents.iter().rev() {
                lanes.insert(lane_idx, *parent);
            }
        } else {
            // Zero-parent commit (root or orphan): explicitly terminate this lane.
            lanes.remove(lane_idx);
        }
        // Deduplicate lanes keeping order.
        let mut seen = HashSet::new();
        lanes.retain(|o| seen.insert(*o));
    }

    Ok(lines.join("\n"))
}

fn collect_branches(repo: &Repository, kind: BranchType, out: &mut Vec<RefRow>) -> PyResult<()> {
    let branches = repo
        .branches(Some(kind))
        .map_err(|err| py_git_err("list branches", err))?;
    for branch in branches {
        let (branch, _) = branch.map_err(|err| py_git_err("branch", err))?;
        let name = branch
            .name()
            .map_err(|err| py_git_err("branch name", err))?
            .unwrap_or("<???>")
            .to_string();
        if let Some(target) = branch.get().target() {
            if let Some(row) = make_row(repo, &name, target)? {
                out.push(row);
            }
        } else {
            // Ignore unborn/missing.
            continue;
        }
    }
    Ok(())
}

fn collect_tags(repo: &Repository, out: &mut Vec<RefRow>) -> PyResult<()> {
    let names = repo
        .tag_names(None)
        .map_err(|err| py_git_err("tag names", err))?;
    for name in names.iter().flatten() {
        let full = format!("refs/tags/{}", name);
        let obj = match repo.revparse_single(&full) {
            Ok(o) => o,
            Err(_) => continue,
        };
        let commit = match obj.peel_to_commit() {
            Ok(c) => c,
            Err(_) => continue,
        };
        let row = make_row(repo, &format!("tag:{}", name), commit.id())?;
        if let Some(row) = row {
            out.push(row);
        }
    }
    Ok(())
}

fn make_row(repo: &Repository, name: &str, oid: Oid) -> PyResult<Option<RefRow>> {
    let commit = match repo.find_commit(oid) {
        Ok(c) => c,
        Err(err) if err.code() == ErrorCode::NotFound => return Ok(None),
        Err(err) => return Err(py_git_err("find commit", err)),
    };
    let oid7 = commit.id().to_string()[0..7].to_string();
    let age = format_age(commit.time().seconds());
    let author = commit
        .author()
        .name()
        .unwrap_or("<unknown>")
        .to_string();
    let summary = commit
        .summary()
        .unwrap_or("<no message>")
        .lines()
        .next()
        .unwrap_or("<no message>")
        .to_string();
    Ok(Some(RefRow {
        name: name.to_string(),
        oid7,
        age,
        author,
        summary,
    }))
}

fn collect_decorations(repo: &Repository) -> PyResult<HashMap<Oid, Vec<String>>> {
    let mut map: HashMap<Oid, Vec<String>> = HashMap::new();
    // Branches (local + remote).
    for kind in [BranchType::Local, BranchType::Remote] {
        let branches = repo
            .branches(Some(kind))
            .map_err(|err| py_git_err("list branches", err))?;
        for branch in branches {
            let (branch, _) = branch.map_err(|err| py_git_err("branch", err))?;
            let name = branch
                .name()
                .map_err(|err| py_git_err("branch name", err))?
                .unwrap_or("<???>")
                .to_string();
            if let Some(target) = branch.get().target() {
                map.entry(target).or_default().push(name);
            }
        }
    }
    // Tags.
    let names = repo
        .tag_names(None)
        .map_err(|err| py_git_err("tag names", err))?;
    for name in names.iter().flatten() {
        let full = format!("refs/tags/{}", name);
        if let Ok(obj) = repo.revparse_single(&full) {
            if let Ok(commit) = obj.peel_to_commit() {
                map.entry(commit.id())
                    .or_default()
                    .push(format!("tag:{}", name));
            }
        }
    }
    Ok(map)
}

fn render_graph_prefix(lanes: usize, current_idx: usize, parent_count: usize) -> String {
    let mut out = String::new();
    for lane in 0..lanes {
        if lane == current_idx {
            out.push('*');
        } else {
            out.push('|');
        }
        if lane + 1 < lanes {
            out.push(' ');
        }
    }
    if parent_count > 1 {
        out.push(' ');
        out.push('\\');
    } else if parent_count == 1 && lanes > 1 {
        out.push(' ');
        out.push('|');
    }
    out
}

fn truncate(s: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    
    // Use display width instead of character count
    let display_width = s.width();
    if display_width <= width {
        return s.to_string();
    }
    
    const ELLIPSIS: char = '…';
    let ellipsis_width = ELLIPSIS.width().unwrap_or(1);
    
    if width <= ellipsis_width {
        return ELLIPSIS.to_string();
    }
    
    // Build string up to the target width, accounting for display width
    let mut result = String::new();
    let mut current_width = 0;
    let target_width = width - ellipsis_width;
    
    for ch in s.chars() {
        let ch_width = ch.width().unwrap_or(0);
        if current_width + ch_width > target_width {
            break;
        }
        result.push(ch);
        current_width += ch_width;
    }
    
    result.push(ELLIPSIS);
    result
}

fn format_age(commit_secs: i64) -> String {
    let now = Utc::now().timestamp();
    let diff = now.saturating_sub(commit_secs);
    // Handle negative age (future timestamps) by showing "0s"
    if diff <= 0 {
        return "0s".to_string();
    }
    if diff < 60 {
        format!("{}s", diff)
    } else if diff < 3600 {
        format!("{}m", diff / 60)
    } else if diff < 86_400 {
        format!("{}h", diff / 3600)
    } else {
        format!("{}d", diff / 86_400)
    }
}

fn terminal_width(max_width: Option<usize>) -> usize {
    if let Some(w) = max_width {
        return w;
    }
    if let Ok(cols) = std::env::var("COLUMNS") {
        if let Ok(val) = cols.parse::<usize>() {
            return val;
        }
    }
    80
}
