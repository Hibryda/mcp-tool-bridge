//! gh_api — structured GitHub API access via gh CLI.

use serde::Serialize;
use serde_json::Value;

/// Structured API response.
#[derive(Debug, Serialize, Clone)]
pub struct GhApiResult {
    pub status_code: Option<u16>,
    pub body: Value,
    pub body_is_array: bool,
    pub item_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_limit_remaining: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pagination: Option<PaginationInfo>,
}

#[derive(Debug, Serialize, Clone)]
pub struct PaginationInfo {
    pub has_next: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_page: Option<String>,
}

/// Validate endpoint path for safety.
pub fn validate_endpoint(endpoint: &str) -> Result<(), String> {
    if !endpoint.starts_with('/') {
        return Err("endpoint must start with '/'".into());
    }
    if endpoint.contains('\0') {
        return Err("endpoint contains null byte".into());
    }
    if endpoint.contains("..") {
        return Err("endpoint contains '..' segment".into());
    }
    if endpoint.len() > 2048 {
        return Err("endpoint exceeds 2048 chars".into());
    }
    // No whitespace
    if endpoint.chars().any(|c| c.is_whitespace()) {
        return Err("endpoint contains whitespace".into());
    }
    Ok(())
}

/// Call GitHub API via gh CLI.
pub async fn gh_api(
    endpoint: &str,
    method: &str,
    body: Option<&str>,
    paginate: bool,
    max_items: usize,
) -> Result<GhApiResult, String> {
    validate_endpoint(endpoint)?;

    let mut args = vec!["api".to_string(), endpoint.to_string()];
    args.extend_from_slice(&["-X".to_string(), method.to_uppercase()]);

    let mut _rate_limit_remaining: Option<u64> = None;
    let mut _status_code: Option<u16> = None;

    if paginate {
        args.push("--paginate".to_string());
    } else {
        // Use --include to get headers (mutual exclusion with --paginate)
        args.push("--include".to_string());
    }

    if body.is_some() {
        args.push("--input".to_string());
        args.push("-".to_string());
    }

    // Use GH_TOKEN env if available
    let gh_path = which_gh();
    let mut cmd = tokio::process::Command::new(&gh_path);
    cmd.args(&args);

    if let Ok(token) = std::env::var("GITHUB_PERSONAL_ACCESS_TOKEN") {
        cmd.env("GH_TOKEN", &token);
    }

    if body.is_some() {
        cmd.stdin(std::process::Stdio::piped());
    }

    let output = cmd.output().await.map_err(|e| {
        // Redact any auth info from error messages
        redact_auth(format!("gh api failed: {e}"))
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(redact_auth(format!("gh api error: {}", stderr.trim())));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    if paginate {
        // --paginate: entire output is JSON body (array concatenation)
        return parse_paginate_output(&stdout, max_items);
    }

    // --include: headers + body separated by blank line
    parse_include_output(&stdout)
}

/// Parse --include output (headers + body).
fn parse_include_output(output: &str) -> Result<GhApiResult, String> {
    let (header_section, body_str) = if let Some(pos) = output.find("\r\n\r\n") {
        (&output[..pos], &output[pos + 4..])
    } else if let Some(pos) = output.find("\n\n") {
        (&output[..pos], &output[pos + 2..])
    } else {
        ("", output)
    };

    // Parse status code from first line
    let status_code = header_section
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse::<u16>().ok());

    // Parse rate limit
    let rate_limit = header_section
        .lines()
        .find(|l| l.to_lowercase().starts_with("x-ratelimit-remaining:"))
        .and_then(|l| l.split(':').nth(1))
        .and_then(|v| v.trim().parse::<u64>().ok());

    // Parse Link header for pagination
    let link_header = header_section
        .lines()
        .find(|l| l.to_lowercase().starts_with("link:"));
    let pagination = link_header.map(|l| {
        let has_next = l.contains("rel=\"next\"");
        PaginationInfo {
            has_next,
            next_page: if has_next { extract_next_url(l) } else { None },
        }
    });

    let body: Value =
        serde_json::from_str(body_str).unwrap_or(Value::String(body_str.trim().to_string()));

    let body_is_array = body.is_array();
    let item_count = body.as_array().map(|a| a.len() as u64);

    Ok(GhApiResult {
        status_code,
        body,
        body_is_array,
        item_count,
        rate_limit_remaining: rate_limit,
        pagination,
    })
}

/// Parse --paginate output (concatenated JSON arrays).
fn parse_paginate_output(output: &str, max_items: usize) -> Result<GhApiResult, String> {
    // gh --paginate concatenates JSON arrays — may need to handle multiple arrays
    let body: Value =
        serde_json::from_str(output).unwrap_or(Value::String(output.trim().to_string()));

    let (body, item_count) = if let Some(arr) = body.as_array() {
        let _truncated = arr.len() > max_items;
        let items: Vec<Value> = arr.iter().take(max_items).cloned().collect();
        let count = items.len() as u64;
        (Value::Array(items), Some(count))
    } else {
        (body, None)
    };

    Ok(GhApiResult {
        status_code: None, // not available with --paginate
        body,
        body_is_array: item_count.is_some(),
        item_count,
        rate_limit_remaining: None,
        pagination: None,
    })
}

/// Extract next URL from Link header.
fn extract_next_url(link: &str) -> Option<String> {
    for part in link.split(',') {
        if part.contains("rel=\"next\"") {
            let url = part
                .trim()
                .strip_prefix("Link: ")
                .unwrap_or(part.trim())
                .split('>')
                .next()?
                .strip_prefix('<')?;
            return Some(url.to_string());
        }
    }
    None
}

/// Redact auth tokens from error messages.
fn redact_auth(msg: String) -> String {
    let mut result = msg;
    // Redact common token patterns
    if let Ok(token) = std::env::var("GH_TOKEN") {
        result = result.replace(&token, "[REDACTED]");
    }
    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        result = result.replace(&token, "[REDACTED]");
    }
    if let Ok(token) = std::env::var("GITHUB_PERSONAL_ACCESS_TOKEN") {
        result = result.replace(&token, "[REDACTED]");
    }
    // Redact ghp_ prefixed tokens
    let mut i = 0;
    while let Some(pos) = result[i..].find("ghp_") {
        let start = i + pos;
        let end = result[start..]
            .find(|c: char| c.is_whitespace() || c == '"' || c == '\'')
            .map(|e| start + e)
            .unwrap_or(result.len());
        result = format!("{}[REDACTED]{}", &result[..start], &result[end..]);
        i = start + 10;
        if i >= result.len() {
            break;
        }
    }
    result
}

/// Find gh binary path.
fn which_gh() -> String {
    // Check common locations
    for path in &[
        std::env::var("HOME").unwrap_or_default() + "/.local/bin/gh",
        "/usr/local/bin/gh".to_string(),
        "/usr/bin/gh".to_string(),
        "gh".to_string(),
    ] {
        if std::path::Path::new(path).exists() || path == "gh" {
            return path.clone();
        }
    }
    "gh".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_good_endpoints() {
        assert!(validate_endpoint("/repos/owner/repo").is_ok());
        assert!(validate_endpoint("/repos/owner/repo/pulls").is_ok());
        assert!(validate_endpoint("/orgs/myorg/repos").is_ok());
        assert!(validate_endpoint("/repos/owner/repo/git/refs/heads%2Fmain").is_ok());
    }

    #[test]
    fn validate_bad_endpoints() {
        assert!(validate_endpoint("repos/no-slash").is_err());
        assert!(validate_endpoint("/repos/../etc/passwd").is_err());
        assert!(validate_endpoint("/repos/\x00evil").is_err());
        assert!(validate_endpoint("/repos/with space").is_err());
        assert!(validate_endpoint(&format!("/{}", "a".repeat(2049))).is_err());
    }

    #[test]
    fn parse_include_response() {
        let output = "HTTP/2 200\r\nx-ratelimit-remaining: 4999\r\nlink: <https://api.github.com/next>; rel=\"next\"\r\n\r\n[{\"id\":1},{\"id\":2}]";
        let r = parse_include_output(output).unwrap();
        assert_eq!(r.status_code, Some(200));
        assert_eq!(r.rate_limit_remaining, Some(4999));
        assert!(r.body_is_array);
        assert_eq!(r.item_count, Some(2));
        assert!(r.pagination.as_ref().unwrap().has_next);
    }

    #[test]
    fn parse_paginate_response() {
        let output = "[{\"id\":1},{\"id\":2},{\"id\":3}]";
        let r = parse_paginate_output(output, 2).unwrap();
        assert_eq!(r.item_count, Some(2));
    }

    #[test]
    fn redact_tokens() {
        std::env::set_var("GH_TOKEN", "ghp_secrettoken123");
        let msg = "error: ghp_secrettoken123 is invalid".to_string();
        let redacted = redact_auth(msg);
        assert!(!redacted.contains("secrettoken123"));
        assert!(redacted.contains("[REDACTED]"));
        std::env::remove_var("GH_TOKEN");
    }
}
