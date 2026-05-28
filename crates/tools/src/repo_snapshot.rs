//! repo_snapshot — one-call read-only repo overview.
//!
//! Collapses the common `git status` → `git diff` → `git log` inspection combo
//! into a single structured result: branch info + ahead/behind, working-tree
//! change counts and entries, an aggregate working diff stat (uncommitted
//! changes vs HEAD), and the N most recent commits. Pure composition of the
//! existing read-only git tools — never mutates.

use serde::Serialize;

use crate::git_log::{self, CommitEntry};
use crate::git_status::{self, BranchInfo, GitError, StatusCounts, StatusEntry};

/// Aggregate diff stat for the working tree (staged + unstaged vs HEAD).
#[derive(Debug, Serialize, Clone, Default, PartialEq)]
pub struct DiffStat {
    pub files_changed: u32,
    pub additions: u32,
    pub deletions: u32,
}

/// Full snapshot result.
#[derive(Debug, Serialize, Clone)]
pub struct RepoSnapshot {
    pub branch: BranchInfo,
    pub clean: bool,
    pub counts: StatusCounts,
    pub entries: Vec<StatusEntry>,
    /// Uncommitted changes vs HEAD (staged + unstaged), aggregated.
    pub working_diff: DiffStat,
    pub recent_commits: Vec<CommitEntry>,
}

/// Build a repo snapshot. Read-only.
pub async fn repo_snapshot(path: &str, log_limit: u32) -> Result<RepoSnapshot, GitError> {
    // git_status canonicalizes + checks the git version; let it be the gate.
    let status = git_status::git_status(path, true).await?;

    // git_log can legitimately fail on an unborn HEAD (fresh repo, no commits).
    // A snapshot of a commit-less repo is still useful, so degrade to empty.
    let recent_commits = match git_log::git_log(path, log_limit, false, None, None, None).await {
        Ok(r) => r.commits,
        Err(_) => Vec::new(),
    };

    let working_diff = working_tree_diffstat(path).await;

    Ok(RepoSnapshot {
        branch: status.branch,
        clean: status.clean,
        counts: status.counts,
        entries: status.entries,
        working_diff,
        recent_commits,
    })
}

/// Run `git diff HEAD --numstat` and aggregate. Returns zeros on any failure
/// (e.g. unborn HEAD) — the snapshot stays useful without it.
async fn working_tree_diffstat(path: &str) -> DiffStat {
    let output = tokio::process::Command::new("git")
        .args([
            "-c",
            "core.quotePath=false",
            "-C",
            path,
            "diff",
            "HEAD",
            "--numstat",
            "--no-ext-diff",
        ])
        .output()
        .await;

    match output {
        Ok(out) if out.status.success() => parse_numstat(&String::from_utf8_lossy(&out.stdout)),
        _ => DiffStat::default(),
    }
}

/// Parse `git diff --numstat` output. Each line is `<added>\t<deleted>\t<path>`;
/// binary files report `-\t-\t<path>` (counted as a changed file, 0 lines).
fn parse_numstat(text: &str) -> DiffStat {
    let mut stat = DiffStat::default();
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let mut cols = line.split('\t');
        let added = cols.next().unwrap_or("-");
        let deleted = cols.next().unwrap_or("-");
        // Only count it as a file if there's a path column.
        if cols.next().is_none() {
            continue;
        }
        stat.files_changed += 1;
        if let Ok(n) = added.parse::<u32>() {
            stat.additions += n;
        }
        if let Ok(n) = deleted.parse::<u32>() {
            stat.deletions += n;
        }
    }
    stat
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numstat_basic() {
        let text = "3\t1\tsrc/main.rs\n0\t5\tREADME.md\n";
        let s = parse_numstat(text);
        assert_eq!(s.files_changed, 2);
        assert_eq!(s.additions, 3);
        assert_eq!(s.deletions, 6);
    }

    #[test]
    fn numstat_binary_counts_file_not_lines() {
        let text = "10\t2\tsrc/a.rs\n-\t-\tlogo.png\n";
        let s = parse_numstat(text);
        assert_eq!(s.files_changed, 2);
        assert_eq!(s.additions, 10);
        assert_eq!(s.deletions, 2);
    }

    #[test]
    fn numstat_empty_is_zero() {
        assert_eq!(parse_numstat(""), DiffStat::default());
        assert_eq!(parse_numstat("\n\n"), DiffStat::default());
    }

    #[test]
    fn numstat_ignores_malformed_lines_without_path() {
        // A line with only two columns (no path) is not a file entry.
        let text = "3\t1\n5\t2\tok.rs\n";
        let s = parse_numstat(text);
        assert_eq!(s.files_changed, 1);
        assert_eq!(s.additions, 5);
        assert_eq!(s.deletions, 2);
    }
}
