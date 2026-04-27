//! git_show — structured commit details with optional diff.

use serde::Serialize;

/// Result of git show for a commit.
#[derive(Debug, Serialize, Clone)]
pub struct GitShowResult {
    pub hash: String,
    pub author_name: String,
    pub author_email: String,
    pub date: String,
    pub subject: String,
    pub body: String,
    pub parent_hashes: Vec<String>,
    pub is_merge: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stats: Option<super::git_log::CommitStats>,
}

/// Run git show on a ref, restricted to commit objects.
pub async fn git_show(
    path: &str,
    reference: &str,
    include_stats: bool,
) -> Result<GitShowResult, super::git_status::GitError> {
    let canonical = std::fs::canonicalize(path).map_err(|e| super::git_status::GitError {
        code: "PATH_ERROR".into(),
        message: format!("cannot resolve path: {e}"),
        raw_stderr: None,
    })?;
    let path_str = canonical.to_string_lossy().to_string();

    // Preflight: verify ref is a commit
    let obj_type = get_object_type(&path_str, reference).await?;
    if obj_type != "commit" {
        return Err(super::git_status::GitError {
            code: "NOT_A_COMMIT".into(),
            message: format!("'{reference}' is a {obj_type}, not a commit"),
            raw_stderr: None,
        });
    }

    // STX/ETX format: hash, author, email, date, subject, body, parents
    let format_str = "%x02%H%x00%an%x00%ae%x00%ai%x00%s%x00%b%x00%P%x03";

    let mut args = vec![
        "-c".to_string(),
        "i18n.logOutputEncoding=UTF-8".to_string(),
        "-c".to_string(),
        "core.quotePath=false".to_string(),
        "-C".to_string(),
        path_str,
        "show".to_string(),
        format!("--format={format_str}"),
        "--no-patch".to_string(),
    ];

    if include_stats {
        // Remove --no-patch and add --numstat
        args.pop(); // remove --no-patch
        args.push("--numstat".to_string());
    }

    args.push(reference.to_string());

    let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let output = tokio::process::Command::new("git")
        .args(&arg_refs)
        .output()
        .await
        .map_err(|e| super::git_status::GitError {
            code: "GIT_NOT_FOUND".into(),
            message: e.to_string(),
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
    parse_show_output(&stdout, include_stats)
}

fn parse_show_output(
    output: &str,
    include_stats: bool,
) -> Result<GitShowResult, super::git_status::GitError> {
    // Find STX..ETX block
    let start = output
        .find('\x02')
        .ok_or_else(|| super::git_status::GitError {
            code: "PARSE_ERROR".into(),
            message: "no STX marker in git show output".into(),
            raw_stderr: None,
        })?;
    let etx = output[start..]
        .find('\x03')
        .ok_or_else(|| super::git_status::GitError {
            code: "PARSE_ERROR".into(),
            message: "no ETX marker in git show output".into(),
            raw_stderr: None,
        })?;

    let meta = &output[start + 1..start + etx];
    let fields: Vec<&str> = meta.split('\x00').collect();
    if fields.len() != 7 {
        return Err(super::git_status::GitError {
            code: "PARSE_ERROR".into(),
            message: format!("expected 7 fields, got {}", fields.len()),
            raw_stderr: None,
        });
    }

    let parents: Vec<String> = if fields[6].is_empty() {
        vec![]
    } else {
        fields[6].split(' ').map(|s| s.to_string()).collect()
    };

    let stats = if include_stats {
        let after_etx = &output[start + etx + 1..];
        super::git_log::parse_numstat_public(after_etx)
    } else {
        None
    };

    Ok(GitShowResult {
        hash: fields[0].to_string(),
        author_name: fields[1].to_string(),
        author_email: fields[2].to_string(),
        date: fields[3].to_string(),
        subject: fields[4].to_string(),
        body: fields[5].trim().to_string(),
        parent_hashes: parents.clone(),
        is_merge: parents.len() > 1,
        stats,
    })
}

async fn get_object_type(
    path: &str,
    reference: &str,
) -> Result<String, super::git_status::GitError> {
    let output = tokio::process::Command::new("git")
        .args(["-C", path, "cat-file", "-t", reference])
        .output()
        .await
        .map_err(|e| super::git_status::GitError {
            code: "GIT_NOT_FOUND".into(),
            message: e.to_string(),
            raw_stderr: None,
        })?;

    if !output.status.success() {
        return Err(super::git_status::GitError {
            code: "INVALID_REF".into(),
            message: format!("'{reference}' is not a valid git reference"),
            raw_stderr: Some(String::from_utf8_lossy(&output.stderr).to_string()),
        });
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_basic_show() {
        let output = "\x02abc123\x00Alice\x00alice@test\x002026-03-28\x00feat: thing\x00Detailed body here.\x00def456\x03\n";
        let r = parse_show_output(output, false).unwrap();
        assert_eq!(r.hash, "abc123");
        assert_eq!(r.subject, "feat: thing");
        assert_eq!(r.body, "Detailed body here.");
        assert_eq!(r.parent_hashes, vec!["def456"]);
    }

    #[test]
    fn parse_merge_show() {
        let output = "\x02abc\x00A\x00a@t\x00d\x00merge\x00body\x00p1 p2\x03\n";
        let r = parse_show_output(output, false).unwrap();
        assert!(r.is_merge);
        assert_eq!(r.parent_hashes.len(), 2);
    }

    #[test]
    fn parse_with_stats() {
        let output = "\x02abc\x00A\x00a@t\x00d\x00msg\x00body\x00p1\x03\n3\t1\tfile.rs\n";
        let r = parse_show_output(output, true).unwrap();
        let stats = r.stats.unwrap();
        assert_eq!(stats.additions, 3);
        assert_eq!(stats.deletions, 1);
    }
}
