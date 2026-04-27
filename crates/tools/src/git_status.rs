//! git_status — structured git status via --porcelain=v2 --branch.

use serde::Serialize;

/// Branch information from porcelain v2 header.
#[derive(Debug, Serialize, Clone)]
pub struct BranchInfo {
    pub head: String,
    pub upstream: Option<String>,
    pub ahead: Option<i64>,
    pub behind: Option<i64>,
}

/// A single file status entry.
#[derive(Debug, Serialize, Clone)]
pub struct StatusEntry {
    /// "modified", "added", "deleted", "renamed", "copied", "untracked", "ignored", "unmerged"
    pub status: String,
    /// Whether the file is staged.
    pub staged: bool,
    /// File path.
    pub path: String,
    /// Original path (for renames/copies).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_path: Option<String>,
}

/// Full git status result.
#[derive(Debug, Serialize, Clone)]
pub struct GitStatusResult {
    pub branch: BranchInfo,
    pub entries: Vec<StatusEntry>,
    pub clean: bool,
    pub counts: StatusCounts,
}

#[derive(Debug, Serialize, Clone)]
pub struct StatusCounts {
    pub modified: u32,
    pub added: u32,
    pub deleted: u32,
    pub renamed: u32,
    pub untracked: u32,
    pub unmerged: u32,
}

/// Typed error from git operations.
#[derive(Debug, Serialize, Clone)]
pub struct GitError {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_stderr: Option<String>,
}

/// Run git status and parse into structured output.
pub async fn git_status(path: &str, show_untracked: bool) -> Result<GitStatusResult, GitError> {
    // Canonicalize path
    let canonical = std::fs::canonicalize(path).map_err(|e| GitError {
        code: "PATH_ERROR".into(),
        message: format!("cannot resolve path: {e}"),
        raw_stderr: None,
    })?;
    let path_str = canonical.to_string_lossy();

    // Check git version
    check_git_version().await?;

    let untracked_mode = if show_untracked { "normal" } else { "no" };

    let output = tokio::process::Command::new("git")
        .args([
            "-c",
            "i18n.logOutputEncoding=UTF-8",
            "-c",
            "core.quotePath=false",
            "-C",
            &path_str,
            "status",
            "--porcelain=v2",
            "--branch",
            &format!("--untracked-files={untracked_mode}"),
        ])
        .output()
        .await
        .map_err(|e| GitError {
            code: "GIT_NOT_FOUND".into(),
            message: format!("cannot execute git: {e}"),
            raw_stderr: None,
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let code = classify_git_error(&stderr);
        return Err(GitError {
            code,
            message: stderr.trim().to_string(),
            raw_stderr: Some(stderr),
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_porcelain_v2(&stdout)
}

/// Parse --porcelain=v2 --branch output.
fn parse_porcelain_v2(output: &str) -> Result<GitStatusResult, GitError> {
    let mut branch = BranchInfo {
        head: String::new(),
        upstream: None,
        ahead: None,
        behind: None,
    };
    let mut entries = Vec::new();

    for line in output.lines() {
        if line.starts_with("# branch.oid") {
            // Skip — we use head from branch.head
        } else if line.starts_with("# branch.head") {
            branch.head = line
                .strip_prefix("# branch.head ")
                .unwrap_or("")
                .to_string();
        } else if line.starts_with("# branch.upstream") {
            branch.upstream = Some(
                line.strip_prefix("# branch.upstream ")
                    .unwrap_or("")
                    .to_string(),
            );
        } else if line.starts_with("# branch.ab") {
            if let Some(ab) = line.strip_prefix("# branch.ab ") {
                let parts: Vec<&str> = ab.split_whitespace().collect();
                if parts.len() >= 2 {
                    branch.ahead = parts[0].strip_prefix('+').and_then(|s| s.parse().ok());
                    branch.behind = parts[1].strip_prefix('-').and_then(|s| s.parse().ok());
                }
            }
        } else if line.starts_with("1 ") || line.starts_with("2 ") {
            // Changed entry (1 = ordinary, 2 = rename/copy)
            if let Some(entry) = parse_changed_entry(line) {
                entries.push(entry);
            }
        } else if line.starts_with("u ") {
            // Unmerged entry
            if let Some(path) = line.split('\t').nth(1).or_else(|| {
                // fallback: split by space, take last
                line.split(' ').last()
            }) {
                entries.push(StatusEntry {
                    status: "unmerged".into(),
                    staged: false,
                    path: path.to_string(),
                    original_path: None,
                });
            }
        } else if line.starts_with("? ") {
            // Untracked
            let path = line.strip_prefix("? ").unwrap_or("").to_string();
            entries.push(StatusEntry {
                status: "untracked".into(),
                staged: false,
                path,
                original_path: None,
            });
        } else if line.starts_with("! ") {
            // Ignored
            let path = line.strip_prefix("! ").unwrap_or("").to_string();
            entries.push(StatusEntry {
                status: "ignored".into(),
                staged: false,
                path,
                original_path: None,
            });
        }
    }

    let counts = StatusCounts {
        modified: entries.iter().filter(|e| e.status == "modified").count() as u32,
        added: entries.iter().filter(|e| e.status == "added").count() as u32,
        deleted: entries.iter().filter(|e| e.status == "deleted").count() as u32,
        renamed: entries.iter().filter(|e| e.status == "renamed").count() as u32,
        untracked: entries.iter().filter(|e| e.status == "untracked").count() as u32,
        unmerged: entries.iter().filter(|e| e.status == "unmerged").count() as u32,
    };

    let clean = entries.is_empty();

    Ok(GitStatusResult {
        branch,
        entries,
        clean,
        counts,
    })
}

/// Parse a porcelain v2 changed entry (type 1 or 2).
fn parse_changed_entry(line: &str) -> Option<StatusEntry> {
    let parts: Vec<&str> = line.splitn(9, ' ').collect();
    if parts.len() < 9 {
        return None;
    }

    let entry_type = parts[0]; // "1" or "2"
    let xy = parts[1]; // XY status codes
    let staged_code = xy.as_bytes().first().copied().unwrap_or(b'.');
    let unstaged_code = xy.as_bytes().get(1).copied().unwrap_or(b'.');

    let staged = staged_code != b'.';

    // For type 2 (rename/copy), the path field contains "path\toriginal_path"
    let path_field = parts[8];

    let (status, path, original_path) = if entry_type == "2" {
        let tab_parts: Vec<&str> = path_field.splitn(2, '\t').collect();
        let path = tab_parts.first().unwrap_or(&"").to_string();
        let orig = tab_parts.get(1).map(|s| s.to_string());

        let status = if staged_code == b'R' || unstaged_code == b'R' {
            "renamed"
        } else {
            "copied"
        };
        (status, path, orig)
    } else {
        let path = path_field.to_string();
        let status = match (staged_code, unstaged_code) {
            (b'A', _) | (_, b'A') => "added",
            (b'D', _) | (_, b'D') => "deleted",
            (b'M', _) | (_, b'M') => "modified",
            _ => "modified",
        };
        (status, path, None)
    };

    Some(StatusEntry {
        status: status.to_string(),
        staged,
        path,
        original_path,
    })
}

/// Classify git stderr into error codes.
pub fn classify_git_error(stderr: &str) -> String {
    let lower = stderr.to_lowercase();
    if lower.contains("not a git repository") {
        "NOT_A_REPO".into()
    } else if lower.contains("head detached") {
        "DETACHED_HEAD".into()
    } else if lower.contains("bare repository") {
        "BARE_REPO".into()
    } else if lower.contains("permission denied") {
        "PERMISSION_DENIED".into()
    } else {
        "UNKNOWN".into()
    }
}

/// Check git version >= 2.11 (required for --porcelain=v2).
async fn check_git_version() -> Result<(), GitError> {
    let output = tokio::process::Command::new("git")
        .args(["--version"])
        .output()
        .await
        .map_err(|_| GitError {
            code: "GIT_NOT_FOUND".into(),
            message: "git is not installed".into(),
            raw_stderr: None,
        })?;

    let version_str = String::from_utf8_lossy(&output.stdout);
    // Parse "git version 2.43.0" etc.
    if let Some(ver) = version_str.split_whitespace().nth(2) {
        let parts: Vec<u32> = ver.split('.').filter_map(|s| s.parse().ok()).collect();
        if parts.len() >= 2 && (parts[0] > 2 || (parts[0] == 2 && parts[1] >= 11)) {
            return Ok(());
        }
        return Err(GitError {
            code: "VERSION_TOO_OLD".into(),
            message: format!("git {ver} is too old. Requires 2.11+ for --porcelain=v2"),
            raw_stderr: None,
        });
    }

    Ok(()) // Can't parse version — proceed optimistically
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_OUTPUT: &str = "\
# branch.oid abc123def456
# branch.head main
# branch.upstream origin/main
# branch.ab +2 -1
1 .M N... 100644 100644 100644 abc123 def456 src/main.rs
1 A. N... 000000 100644 100644 000000 abc123 new_file.rs
? untracked.txt
";

    #[test]
    fn parse_branch_info() {
        let r = parse_porcelain_v2(SAMPLE_OUTPUT).unwrap();
        assert_eq!(r.branch.head, "main");
        assert_eq!(r.branch.upstream, Some("origin/main".to_string()));
        assert_eq!(r.branch.ahead, Some(2));
        assert_eq!(r.branch.behind, Some(1));
    }

    #[test]
    fn parse_entries() {
        let r = parse_porcelain_v2(SAMPLE_OUTPUT).unwrap();
        assert_eq!(r.entries.len(), 3);
        assert_eq!(r.entries[0].status, "modified");
        assert_eq!(r.entries[0].path, "src/main.rs");
        assert_eq!(r.entries[1].status, "added");
        assert_eq!(r.entries[1].staged, true);
        assert_eq!(r.entries[2].status, "untracked");
    }

    #[test]
    fn counts_correct() {
        let r = parse_porcelain_v2(SAMPLE_OUTPUT).unwrap();
        assert_eq!(r.counts.modified, 1);
        assert_eq!(r.counts.added, 1);
        assert_eq!(r.counts.untracked, 1);
        assert!(!r.clean);
    }

    #[test]
    fn clean_repo() {
        let output = "# branch.oid abc\n# branch.head main\n";
        let r = parse_porcelain_v2(output).unwrap();
        assert!(r.clean);
        assert_eq!(r.entries.len(), 0);
    }

    #[test]
    fn no_upstream() {
        let output = "# branch.oid abc\n# branch.head feature\n";
        let r = parse_porcelain_v2(output).unwrap();
        assert_eq!(r.branch.upstream, None);
        assert_eq!(r.branch.ahead, None);
        assert_eq!(r.branch.behind, None);
    }

    #[test]
    fn error_classification() {
        assert_eq!(
            classify_git_error("fatal: not a git repository"),
            "NOT_A_REPO"
        );
        assert_eq!(
            classify_git_error("error: permission denied"),
            "PERMISSION_DENIED"
        );
        assert_eq!(classify_git_error("something else"), "UNKNOWN");
    }
}
