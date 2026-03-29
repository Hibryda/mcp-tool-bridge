//! git_log — structured commit log with stable pagination.

use serde::Serialize;

/// A single commit entry.
#[derive(Debug, Serialize, Clone)]
pub struct CommitEntry {
    pub hash: String,
    pub author_name: String,
    pub author_email: String,
    pub date: String,
    pub subject: String,
    pub parent_hashes: Vec<String>,
    pub refs: Vec<String>,
    pub is_merge: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stats: Option<CommitStats>,
}

/// File-level stats for a commit.
#[derive(Debug, Serialize, Clone)]
pub struct CommitStats {
    pub files_changed: u32,
    pub additions: u32,
    pub deletions: u32,
    pub file_stats: Vec<FileStat>,
}

#[derive(Debug, Serialize, Clone)]
pub struct FileStat {
    pub file: String,
    /// None for binary files.
    pub added: Option<u32>,
    /// None for binary files.
    pub deleted: Option<u32>,
}

/// Result of git log query.
#[derive(Debug, Serialize, Clone)]
pub struct GitLogResult {
    pub commits: Vec<CommitEntry>,
    pub count: u32,
    /// HEAD oid at query time — use for stable pagination.
    pub snapshot_oid: String,
    /// Hash of the last commit — use as after_hash for next page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_hash: Option<String>,
    pub truncated: bool,
    pub parse_warnings: Vec<String>,
}

/// Run git log with structured output using STX/ETX sentinels.
pub async fn git_log(
    path: &str,
    max_count: u32,
    include_stats: bool,
    after_hash: Option<&str>,
    snapshot_oid: Option<&str>,
    branch: Option<&str>,
) -> Result<GitLogResult, super::git_status::GitError> {
    let canonical = std::fs::canonicalize(path).map_err(|e| super::git_status::GitError {
        code: "PATH_ERROR".into(),
        message: format!("cannot resolve path: {e}"),
        raw_stderr: None,
    })?;
    let path_str = canonical.to_string_lossy().to_string();

    // Get HEAD oid for snapshot
    let head_oid = get_head_oid(&path_str).await?;
    let effective_snapshot = snapshot_oid.unwrap_or(&head_oid);

    // Build range spec
    let range = match (after_hash, branch) {
        (Some(after), _) => format!("{after}..{effective_snapshot}"),
        (None, Some(br)) => br.to_string(),
        (None, None) => effective_snapshot.to_string(),
    };

    // STX (0x02) and ETX (0x03) as record delimiters, NUL (0x00) as field separators
    // Fields: hash, author_name, author_email, author_date, subject, parent_hashes, refs
    let format_str = "%x02%H%x00%an%x00%ae%x00%ai%x00%s%x00%P%x00%D%x03";

    let format_arg = format!("--format={format_str}");
    let count_arg = format!("-{max_count}");

    let mut args = vec![
        "-c", "i18n.logOutputEncoding=UTF-8",
        "-c", "core.quotePath=false",
        "-C", &path_str,
        "log",
        &format_arg,
        &count_arg,
        "--decorate=short",
    ];

    args.push(&range);

    if include_stats {
        args.push("--numstat");
    }

    let output = tokio::process::Command::new("git")
        .args(&args)
        .output()
        .await
        .map_err(|e| super::git_status::GitError {
            code: "GIT_NOT_FOUND".into(),
            message: format!("cannot execute git: {e}"),
            raw_stderr: None,
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(super::git_status::GitError {
            code: super::git_status::classify_git_error(&stderr),
            message: stderr.trim().to_string(),
            raw_stderr: Some(stderr),
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_log_output(&stdout, max_count, include_stats, &head_oid)
}

/// Parse the STX/ETX delimited output.
fn parse_log_output(
    output: &str,
    max_count: u32,
    include_stats: bool,
    snapshot_oid: &str,
) -> Result<GitLogResult, super::git_status::GitError> {
    let mut commits = Vec::new();
    let mut parse_warnings = Vec::new();

    // Split by STX (0x02) to get records
    for record in output.split('\x02') {
        let record = record.trim();
        if record.is_empty() {
            continue;
        }

        // Find ETX (0x03) — everything before it is the commit metadata
        let (meta_part, stats_part) = match record.find('\x03') {
            Some(pos) => (&record[..pos], Some(record[pos + 1..].trim())),
            None => {
                parse_warnings.push(format!("missing ETX in record: {}", &record[..record.len().min(40)]));
                continue;
            }
        };

        // Split metadata by NUL
        let fields: Vec<&str> = meta_part.split('\x00').collect();
        if fields.len() != 7 {
            parse_warnings.push(format!(
                "expected 7 fields, got {} in commit starting with '{}'",
                fields.len(),
                fields.first().unwrap_or(&"?")
            ));
            continue;
        }

        let parent_str = fields[5];
        let parents: Vec<String> = if parent_str.is_empty() {
            vec![]
        } else {
            parent_str.split(' ').map(|s| s.to_string()).collect()
        };

        let refs_str = fields[6];
        let refs: Vec<String> = if refs_str.is_empty() {
            vec![]
        } else {
            refs_str.split(", ").map(|s| s.to_string()).collect()
        };

        let stats = if include_stats {
            stats_part.and_then(|s| parse_numstat(s))
        } else {
            None
        };

        commits.push(CommitEntry {
            hash: fields[0].to_string(),
            author_name: fields[1].to_string(),
            author_email: fields[2].to_string(),
            date: fields[3].to_string(),
            subject: fields[4].to_string(),
            parent_hashes: parents.clone(),
            refs,
            is_merge: parents.len() > 1,
            stats,
        });
    }

    let count = commits.len() as u32;
    let truncated = count >= max_count;
    let last_hash = commits.last().map(|c| c.hash.clone());

    Ok(GitLogResult {
        commits,
        count,
        snapshot_oid: snapshot_oid.to_string(),
        last_hash,
        truncated,
        parse_warnings,
    })
}

/// Parse --numstat output lines.
/// Parse numstat — also used by git_show.
pub fn parse_numstat_public(stats_text: &str) -> Option<CommitStats> {
    parse_numstat(stats_text)
}

fn parse_numstat(stats_text: &str) -> Option<CommitStats> {
    let mut file_stats = Vec::new();
    let mut total_add = 0u32;
    let mut total_del = 0u32;

    for line in stats_text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() >= 3 {
            let added = parts[0].parse::<u32>().ok();
            let deleted = parts[1].parse::<u32>().ok();
            if let Some(a) = added { total_add += a; }
            if let Some(d) = deleted { total_del += d; }
            file_stats.push(FileStat {
                file: parts[2].to_string(),
                added,
                deleted,
            });
        }
    }

    if file_stats.is_empty() {
        return None;
    }

    Some(CommitStats {
        files_changed: file_stats.len() as u32,
        additions: total_add,
        deletions: total_del,
        file_stats,
    })
}

/// Get HEAD oid for stable pagination.
async fn get_head_oid(path: &str) -> Result<String, super::git_status::GitError> {
    let output = tokio::process::Command::new("git")
        .args(["-C", path, "rev-parse", "HEAD"])
        .output()
        .await
        .map_err(|e| super::git_status::GitError {
            code: "GIT_NOT_FOUND".into(),
            message: e.to_string(),
            raw_stderr: None,
        })?;

    if !output.status.success() {
        return Err(super::git_status::GitError {
            code: "NOT_A_REPO".into(),
            message: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            raw_stderr: None,
        });
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_basic_log() {
        let output = "\x02abc123\x00Alice\x00alice@test\x002026-03-28 10:00\x00feat: add thing\x00def456\x00HEAD -> main\x03\n";
        let r = parse_log_output(output, 50, false, "abc123").unwrap();
        assert_eq!(r.count, 1);
        assert_eq!(r.commits[0].hash, "abc123");
        assert_eq!(r.commits[0].author_name, "Alice");
        assert_eq!(r.commits[0].subject, "feat: add thing");
        assert_eq!(r.commits[0].parent_hashes, vec!["def456"]);
        assert!(r.commits[0].refs.contains(&"HEAD -> main".to_string()));
        assert!(!r.commits[0].is_merge);
    }

    #[test]
    fn parse_merge_commit() {
        let output = "\x02aaa\x00Bob\x00bob@t\x002026-01-01\x00Merge branch\x00bbb ccc\x00\x03\n";
        let r = parse_log_output(output, 50, false, "aaa").unwrap();
        assert!(r.commits[0].is_merge);
        assert_eq!(r.commits[0].parent_hashes.len(), 2);
    }

    #[test]
    fn parse_with_numstat() {
        let output = "\x02abc\x00A\x00a@t\x002026-01-01\x00msg\x00def\x00\x03\n5\t3\tsrc/main.rs\n-\t-\timage.png\n";
        let r = parse_log_output(output, 50, true, "abc").unwrap();
        let stats = r.commits[0].stats.as_ref().unwrap();
        assert_eq!(stats.files_changed, 2);
        assert_eq!(stats.additions, 5);
        assert_eq!(stats.deletions, 3);
        assert_eq!(stats.file_stats[1].added, None); // binary
    }

    #[test]
    fn parse_multiple_commits() {
        let output = "\x02a1\x00A\x00a@t\x00d1\x00msg1\x00p1\x00\x03\n\x02a2\x00B\x00b@t\x00d2\x00msg2\x00p2\x00\x03\n\x02a3\x00C\x00c@t\x00d3\x00msg3\x00p3\x00\x03\n";
        let r = parse_log_output(output, 50, false, "a1").unwrap();
        assert_eq!(r.count, 3);
        assert_eq!(r.last_hash, Some("a3".to_string()));
    }

    #[test]
    fn parse_truncation() {
        let output = "\x02a1\x00A\x00a@t\x00d\x00m\x00p\x00\x03\n\x02a2\x00B\x00b@t\x00d\x00m\x00p\x00\x03\n";
        let r = parse_log_output(output, 2, false, "a1").unwrap();
        assert!(r.truncated);
    }

    #[test]
    fn parse_empty_refs() {
        let output = "\x02abc\x00A\x00a@t\x002026-01\x00msg\x00def\x00\x03\n";
        let r = parse_log_output(output, 50, false, "abc").unwrap();
        assert!(r.commits[0].refs.is_empty());
    }

    #[test]
    fn parse_root_commit_no_parents() {
        let output = "\x02abc\x00A\x00a@t\x002026-01\x00initial\x00\x00\x03\n";
        let r = parse_log_output(output, 50, false, "abc").unwrap();
        assert!(r.commits[0].parent_hashes.is_empty());
    }

    #[test]
    fn malformed_record_warns() {
        let output = "\x02abc\x00only_two_fields\x03\n";
        let r = parse_log_output(output, 50, false, "abc").unwrap();
        assert_eq!(r.count, 0);
        assert!(!r.parse_warnings.is_empty());
    }
}
