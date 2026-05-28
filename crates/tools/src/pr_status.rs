//! pr_status — forge-agnostic pull-request status (read-only).
//!
//! Detects the forge from the repo's `origin` remote (github → `gh`,
//! gitlab → `glab`, anything else → Forgejo REST), fetches the PR, and returns
//! a uniform structured status. `ready_to_merge` ANDs the cheaply-available
//! merge preconditions so an agent can decide "is this safe to merge?" in one
//! call instead of a multi-command poll.
//!
//! GitHub: shells out to `gh pr view --json`.
//! Forgejo: REST (`/api/v1/repos/{owner}/{repo}/pulls/{n}` + commit status),
//!          token from `FORGEJO_TOKEN` or `fj`'s `keys.json`.
//! GitLab: not wired (glab not installed) — returns a typed error.

use serde::Serialize;
use serde_json::Value;

use crate::curl;

/// CI check rollup counts.
#[derive(Debug, Serialize, Clone, Default, PartialEq)]
pub struct ChecksSummary {
    pub passing: u64,
    pub failing: u64,
    pub pending: u64,
}

/// Uniform PR status across forges.
#[derive(Debug, Serialize, Clone)]
pub struct PrStatus {
    pub forge: String,
    pub number: u64,
    /// "open" | "closed" | "merged"
    pub state: String,
    pub title: String,
    /// Whether the PR is a draft (never merge-ready while true).
    pub is_draft: bool,
    /// None when the forge reports an unknown/computing merge state.
    pub mergeable: Option<bool>,
    /// GitHub mergeStateStatus (CLEAN/DIRTY/BLOCKED/BEHIND/UNSTABLE/…). None elsewhere.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merge_state_status: Option<String>,
    pub checks: ChecksSummary,
    /// GitHub only: APPROVED | CHANGES_REQUESTED | REVIEW_REQUIRED. None elsewhere.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub review_decision: Option<String>,
    /// Unresolved review-thread count. GitHub only (via GraphQL). None for
    /// Forgejo/GitLab — those forges don't expose it the same way (Forgejo has
    /// no GraphQL; its review-bot findings are issue comments, not threads).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub review_threads_unresolved: Option<u64>,
    pub comments: u64,
    /// Conservative, fail-closed merge-readiness gate: open, not draft,
    /// mergeable, merge state clean, all checks known + green, not
    /// CHANGES_REQUESTED, and (GitHub) zero unresolved review threads. An
    /// unknown signal (failed CI/thread lookup) blocks readiness.
    pub ready_to_merge: bool,
}

/// Typed error.
#[derive(Debug, Serialize, Clone)]
pub struct PrError {
    pub code: String,
    pub message: String,
}

fn err(code: &str, message: impl Into<String>) -> PrError {
    PrError {
        code: code.into(),
        message: message.into(),
    }
}

// ── public entry ────────────────────────────────────────────────────

pub async fn pr_status(
    path: &str,
    number: u64,
    forge_override: Option<&str>,
) -> Result<PrStatus, PrError> {
    let remote = git_remote_origin(path).await?;
    let (host, owner, repo) = parse_remote_url(&remote).ok_or_else(|| {
        err(
            "BAD_REMOTE",
            format!("cannot parse origin remote: {remote}"),
        )
    })?;

    let forge = forge_override.unwrap_or_else(|| detect_forge_from_host(&host));

    match forge {
        "github" => github_pr_status(&owner, &repo, number).await,
        "forgejo" | "gitea" => forgejo_pr_status(&host, &owner, &repo, number).await,
        "gitlab" => Err(err(
            "GLAB_NOT_AVAILABLE",
            "GitLab backend not wired — glab is not installed on this host",
        )),
        other => Err(err("UNKNOWN_FORGE", format!("unknown forge '{other}'"))),
    }
}

// ── forge detection ─────────────────────────────────────────────────

/// Parse a git remote URL into (host, owner, repo). Handles scp-like
/// (`git@host:owner/repo.git`), `https://`, and `ssh://` forms.
pub fn parse_remote_url(url: &str) -> Option<(String, String, String)> {
    let url = url.trim();
    let stripped = url.strip_suffix(".git").unwrap_or(url);

    // scp-like: git@host:owner/repo
    if let Some(rest) = stripped.strip_prefix("git@") {
        if let Some((host, path)) = rest.split_once(':') {
            return split_owner_repo(host, path);
        }
    }

    for scheme in ["ssh://", "https://", "http://"] {
        if let Some(rest) = stripped.strip_prefix(scheme) {
            // Drop any userinfo (user[:pass]@) — never part of the host.
            let rest = rest.rsplit_once('@').map(|(_, h)| h).unwrap_or(rest);
            if let Some((authority, path)) = rest.split_once('/') {
                // Keep the port: it's required to build the API URL for
                // self-hosted instances on non-standard ports.
                return split_owner_repo(authority, path);
            }
        }
    }
    None
}

fn split_owner_repo(host: &str, path: &str) -> Option<(String, String, String)> {
    let parts: Vec<&str> = path
        .trim_start_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();
    if parts.len() < 2 {
        return None;
    }
    Some((host.to_string(), parts[0].to_string(), parts[1].to_string()))
}

pub fn detect_forge_from_host(host: &str) -> &'static str {
    // Strip the port before matching the hostname.
    let h = host.split(':').next().unwrap_or(host).to_lowercase();
    if h == "github.com" || h.ends_with(".github.com") || h == "github" {
        "github"
    } else if h == "gitlab.com" || h.ends_with(".gitlab.com") || h.contains("gitlab") {
        "gitlab"
    } else {
        "forgejo"
    }
}

/// Conservative, fail-closed merge-readiness gate. Backends fill the signals
/// they can determine; an unknown signal (failed CI/thread lookup) blocks.
struct MergeGate<'a> {
    state: &'a str,
    is_draft: bool,
    mergeable: Option<bool>,
    /// GitHub mergeStateStatus is one of the blocking states (DIRTY/BLOCKED/
    /// UNKNOWN/DRAFT). False when there's no such signal (e.g. Forgejo).
    merge_state_bad: bool,
    checks: &'a ChecksSummary,
    /// False when CI status couldn't be fetched → fail closed.
    checks_known: bool,
    changes_requested: bool,
    /// Unresolved review-thread count; None = unknown.
    threads_unresolved: Option<u64>,
    /// Whether the thread signal applies to this forge (GitHub yes, Forgejo no).
    threads_applicable: bool,
}

impl MergeGate<'_> {
    fn ready(&self) -> bool {
        self.state == "open"
            && !self.is_draft
            && self.mergeable == Some(true)
            && !self.merge_state_bad
            && self.checks_known
            && self.checks.failing == 0
            && self.checks.pending == 0
            && !self.changes_requested
            // GitHub: None (GraphQL failed) → not ready (fail-closed).
            && (!self.threads_applicable || self.threads_unresolved == Some(0))
    }
}

// ── GitHub backend ──────────────────────────────────────────────────

async fn github_pr_status(owner: &str, repo: &str, number: u64) -> Result<PrStatus, PrError> {
    let repo_flag = format!("{owner}/{repo}");
    let num = number.to_string();
    let mut cmd = tokio::process::Command::new(crate::gh_api::which_gh());
    cmd.args([
        "pr",
        "view",
        &num,
        "-R",
        &repo_flag,
        "--json",
        "number,state,title,isDraft,mergeable,mergeStateStatus,statusCheckRollup,reviewDecision,comments",
    ]);
    if let Ok(token) = std::env::var("GITHUB_PERSONAL_ACCESS_TOKEN") {
        cmd.env("GH_TOKEN", token);
    }

    let out = cmd
        .output()
        .await
        .map_err(|e| err("GH_EXEC", format!("cannot execute gh: {e}")))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let msg = stderr.trim().to_string();
        let code = if msg.to_lowercase().contains("no pull requests found")
            || msg.to_lowercase().contains("not found")
        {
            "NOT_FOUND"
        } else {
            "GH_ERROR"
        };
        return Err(err(code, msg));
    }

    let v: Value = serde_json::from_slice(&out.stdout)
        .map_err(|e| err("PARSE", format!("invalid gh JSON: {e}")))?;

    // Second call: unresolved review-thread count via GraphQL (not exposed by
    // `gh pr view --json`). Best-effort — a failure leaves the count unknown
    // (None) rather than blocking the whole status.
    let threads_unresolved = github_unresolved_threads(owner, repo, number).await;

    parse_gh_pr(&v, threads_unresolved)
}

/// Count unresolved review threads via GraphQL. Returns None on any failure.
async fn github_unresolved_threads(owner: &str, repo: &str, number: u64) -> Option<u64> {
    let query = "query($o:String!,$r:String!,$n:Int!){repository(owner:$o,name:$r){pullRequest(number:$n){reviewThreads(first:100){nodes{isResolved} pageInfo{hasNextPage}}}}}";
    let mut cmd = tokio::process::Command::new(crate::gh_api::which_gh());
    cmd.args([
        "api",
        "graphql",
        // `-f` (string) for owner/repo so gh never treats a leading '@' as a
        // file or coerces the value; `-F` (typed) only for the Int variable.
        "-f",
        &format!("query={query}"),
        "-f",
        &format!("o={owner}"),
        "-f",
        &format!("r={repo}"),
        "-F",
        &format!("n={number}"),
    ]);
    if let Ok(token) = std::env::var("GITHUB_PERSONAL_ACCESS_TOKEN") {
        cmd.env("GH_TOKEN", token);
    }
    let out = cmd.output().await.ok()?;
    if !out.status.success() {
        return None;
    }
    let v: Value = serde_json::from_slice(&out.stdout).ok()?;
    // A GraphQL-level error (HTTP 200 with an `errors` array, or missing data)
    // means the count is unknown — return None so readiness fails closed.
    count_unresolved_threads(&v)
}

/// Pure: count `isResolved == false` review threads. Returns None (unknown) on
/// GraphQL `errors`, absent nodes, or undecidable truncation — so callers fail
/// closed instead of assuming zero.
fn count_unresolved_threads(v: &Value) -> Option<u64> {
    if v.get("errors")
        .and_then(|e| e.as_array())
        .is_some_and(|a| !a.is_empty())
    {
        return None;
    }
    let threads = v.pointer("/data/repository/pullRequest/reviewThreads")?;
    let nodes = threads.pointer("/nodes")?.as_array()?;
    let unresolved = nodes
        .iter()
        .filter(|t| t.get("isResolved").and_then(|x| x.as_bool()) == Some(false))
        .count() as u64;
    let has_next = threads
        .pointer("/pageInfo/hasNextPage")
        .and_then(|x| x.as_bool())
        .unwrap_or(false);
    // If the first page already has an unresolved thread, it's not ready
    // regardless of further pages. If the first page is all-resolved but more
    // pages exist, the true count is unknown → None (fail closed).
    if unresolved == 0 && has_next {
        None
    } else {
        Some(unresolved)
    }
}

pub fn parse_gh_pr(v: &Value, threads_unresolved: Option<u64>) -> Result<PrStatus, PrError> {
    let number = v
        .get("number")
        .and_then(|x| x.as_u64())
        .ok_or_else(|| err("PARSE", "missing 'number' in gh response"))?;
    let state = v
        .get("state")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_lowercase();
    let title = v
        .get("title")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let is_draft = v.get("isDraft").and_then(|x| x.as_bool()).unwrap_or(false);
    let mergeable = match v.get("mergeable").and_then(|x| x.as_str()) {
        Some("MERGEABLE") => Some(true),
        Some("CONFLICTING") => Some(false),
        _ => None,
    };
    let merge_state_status = v
        .get("mergeStateStatus")
        .and_then(|x| x.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_uppercase());
    // States that mean "not mergeable right now". UNKNOWN = GitHub still
    // computing → fail closed.
    let merge_state_bad = matches!(
        merge_state_status.as_deref(),
        Some("DIRTY") | Some("BLOCKED") | Some("DRAFT") | Some("UNKNOWN")
    );
    let checks = count_gh_checks(v.get("statusCheckRollup"));
    let review_decision = v
        .get("reviewDecision")
        .and_then(|x| x.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from);
    let comments = v
        .get("comments")
        .and_then(|x| x.as_array())
        .map(|a| a.len() as u64)
        .unwrap_or(0);

    let ready_to_merge = MergeGate {
        state: &state,
        is_draft,
        mergeable,
        merge_state_bad,
        checks: &checks,
        checks_known: true, // rollup is part of the (succeeded) pr-view call
        changes_requested: review_decision.as_deref() == Some("CHANGES_REQUESTED"),
        threads_unresolved,
        threads_applicable: true,
    }
    .ready();

    Ok(PrStatus {
        forge: "github".into(),
        number,
        state,
        title,
        is_draft,
        mergeable,
        merge_state_status,
        checks,
        review_decision,
        review_threads_unresolved: threads_unresolved,
        comments,
        ready_to_merge,
    })
}

/// Count gh `statusCheckRollup`. Items are either CheckRun (has `conclusion`
/// once finished, `status` while running) or StatusContext (has `state`).
fn count_gh_checks(rollup: Option<&Value>) -> ChecksSummary {
    let mut c = ChecksSummary::default();
    let Some(arr) = rollup.and_then(|x| x.as_array()) else {
        return c;
    };
    for item in arr {
        let conclusion = item
            .get("conclusion")
            .and_then(|x| x.as_str())
            .filter(|s| !s.is_empty());
        let state = item
            .get("state")
            .and_then(|x| x.as_str())
            .filter(|s| !s.is_empty());

        if let Some(concl) = conclusion {
            match concl {
                "SUCCESS" | "NEUTRAL" | "SKIPPED" => c.passing += 1,
                _ => c.failing += 1, // FAILURE, TIMED_OUT, CANCELLED, ACTION_REQUIRED, STARTUP_FAILURE
            }
        } else if let Some(st) = state {
            match st {
                "SUCCESS" => c.passing += 1,
                "PENDING" | "EXPECTED" => c.pending += 1,
                _ => c.failing += 1, // FAILURE, ERROR
            }
        } else {
            // CheckRun still running (status IN_PROGRESS/QUEUED, no conclusion yet).
            c.pending += 1;
        }
    }
    c
}

// ── Forgejo backend ─────────────────────────────────────────────────

async fn forgejo_pr_status(
    host: &str,
    owner: &str,
    repo: &str,
    number: u64,
) -> Result<PrStatus, PrError> {
    // owner/repo come from a git remote; keep them to safe path segments so
    // they can't inject extra URL path/query.
    if !is_safe_path_segment(owner) || !is_safe_path_segment(repo) {
        return Err(err(
            "BAD_REMOTE",
            format!("unsafe owner/repo in remote: {owner}/{repo}"),
        ));
    }
    let token = resolve_forgejo_token(host).ok_or_else(|| {
        err(
            "NO_TOKEN",
            format!("no Forgejo token for {host} — set FORGEJO_TOKEN or log in with fj"),
        )
    })?;
    let base = format!("https://{host}/api/v1/repos/{owner}/{repo}");
    let headers = vec![("Authorization".to_string(), format!("token {token}"))];

    // Redirects disabled: a direct API call shouldn't redirect, and following
    // one could resend the auth header to another host.
    let pr_url = format!("{base}/pulls/{number}");
    let resp = curl::http_request(&pr_url, "GET", &headers, None, false, 30)
        .await
        .map_err(|e| err("HTTP", e.to_string()))?;
    if resp.status_code == 404 {
        return Err(err("NOT_FOUND", format!("PR #{number} not found")));
    }
    if resp.status_code >= 400 {
        return Err(err(
            "HTTP",
            format!("forgejo API returned {}", resp.status_code),
        ));
    }
    let pr: Value = serde_json::from_str(&resp.body)
        .map_err(|e| err("PARSE", format!("invalid forgejo JSON: {e}")))?;

    // CI status for the PR head commit. checks_known is false if we couldn't
    // determine it (missing sha, HTTP/parse failure) → readiness fails closed.
    let (checks, checks_known) = match pr
        .get("head")
        .and_then(|h| h.get("sha"))
        .and_then(|s| s.as_str())
    {
        Some(sha) => {
            let st_url = format!("{base}/commits/{sha}/status");
            match curl::http_request(&st_url, "GET", &headers, None, false, 30).await {
                Ok(r) if r.status_code < 400 => match serde_json::from_str::<Value>(&r.body) {
                    Ok(v) => (count_forgejo_checks(&v), true),
                    Err(_) => (ChecksSummary::default(), false),
                },
                _ => (ChecksSummary::default(), false),
            }
        }
        None => (ChecksSummary::default(), false),
    };

    parse_forgejo_pr(&pr, checks, checks_known)
}

pub fn parse_forgejo_pr(
    pr: &Value,
    checks: ChecksSummary,
    checks_known: bool,
) -> Result<PrStatus, PrError> {
    let number = pr
        .get("number")
        .and_then(|x| x.as_u64())
        .ok_or_else(|| err("PARSE", "missing 'number' in forgejo response"))?;
    let title = pr
        .get("title")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let merged = pr.get("merged").and_then(|x| x.as_bool()).unwrap_or(false);
    let state = if merged {
        "merged".to_string()
    } else {
        pr.get("state")
            .and_then(|x| x.as_str())
            .unwrap_or("open")
            .to_lowercase()
    };
    let is_draft = pr.get("draft").and_then(|x| x.as_bool()).unwrap_or(false);
    let mergeable = pr.get("mergeable").and_then(|x| x.as_bool());
    let comments = pr.get("comments").and_then(|x| x.as_u64()).unwrap_or(0);

    // Forgejo has no GraphQL; thread resolution isn't exposed uniformly and the
    // review-bot uses issue comments, not threads. Thread signal not applicable.
    let ready_to_merge = MergeGate {
        state: &state,
        is_draft,
        mergeable,
        merge_state_bad: false,
        checks: &checks,
        checks_known,
        changes_requested: false,
        threads_unresolved: None,
        threads_applicable: false,
    }
    .ready();

    Ok(PrStatus {
        forge: "forgejo".into(),
        number,
        state,
        title,
        is_draft,
        mergeable,
        merge_state_status: None,
        checks,
        review_decision: None,
        review_threads_unresolved: None,
        comments,
        ready_to_merge,
    })
}

/// Forgejo combined commit status: `{ state, statuses: [{ status }] }`. When the
/// `statuses` array is empty, fall back to the combined top-level `state` so a
/// failing/pending overall status still registers.
fn count_forgejo_checks(status_resp: &Value) -> ChecksSummary {
    let mut c = ChecksSummary::default();
    let arr = status_resp
        .get("statuses")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    for s in &arr {
        match s.get("status").and_then(|x| x.as_str()).unwrap_or("") {
            "success" => c.passing += 1,
            "pending" => c.pending += 1,
            _ => c.failing += 1, // failure, error, warning
        }
    }
    // No per-context statuses but a non-success combined state → reflect it so
    // readiness isn't granted on an unevaluated/failed overall status.
    if arr.is_empty() {
        match status_resp
            .get("state")
            .and_then(|x| x.as_str())
            .unwrap_or("")
        {
            "pending" => c.pending += 1,
            "failure" | "error" => c.failing += 1,
            _ => {} // "success" or "" → no checks configured, leave empty
        }
    }
    c
}

/// Allow only safe characters in a URL path segment (owner/repo).
fn is_safe_path_segment(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// Resolve a Forgejo token: FORGEJO_TOKEN env, else fj's keys.json by host
/// (resolving aliases).
fn resolve_forgejo_token(host: &str) -> Option<String> {
    if let Ok(t) = std::env::var("FORGEJO_TOKEN") {
        if !t.is_empty() {
            return Some(t);
        }
    }
    let home = std::env::var("HOME").ok()?;
    let path = format!("{home}/.local/share/forgejo-cli/keys.json");
    let data = std::fs::read_to_string(path).ok()?;
    let v: Value = serde_json::from_str(&data).ok()?;
    token_from_keys(&v, host)
}

/// Pure lookup over the keys.json structure (testable without the filesystem).
fn token_from_keys(v: &Value, host: &str) -> Option<String> {
    let hosts = v.get("hosts")?.as_object()?;
    if let Some(h) = hosts.get(host) {
        return h.get("token").and_then(|t| t.as_str()).map(String::from);
    }
    // Resolve alias → real host.
    let real = v
        .get("aliases")
        .and_then(|a| a.as_object())
        .and_then(|a| a.get(host))
        .and_then(|x| x.as_str())?;
    hosts
        .get(real)
        .and_then(|h| h.get("token"))
        .and_then(|t| t.as_str())
        .map(String::from)
}

// ── git remote ──────────────────────────────────────────────────────

async fn git_remote_origin(path: &str) -> Result<String, PrError> {
    let out = tokio::process::Command::new("git")
        .args(["-C", path, "remote", "get-url", "origin"])
        .output()
        .await
        .map_err(|e| err("GIT_EXEC", format!("cannot execute git: {e}")))?;
    if !out.status.success() {
        return Err(err("NO_ORIGIN", "no 'origin' remote found in repo"));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn remote_scp_form() {
        let (h, o, r) = parse_remote_url("git@github.com:Hibryda/mcp-tool-bridge.git").unwrap();
        assert_eq!(h, "github.com");
        assert_eq!(o, "Hibryda");
        assert_eq!(r, "mcp-tool-bridge");
    }

    #[test]
    fn remote_https_form() {
        let (h, o, r) =
            parse_remote_url("https://git.hemoglobina.store/science/mcp-tool-bridge.git").unwrap();
        assert_eq!(h, "git.hemoglobina.store");
        assert_eq!(o, "science");
        assert_eq!(r, "mcp-tool-bridge");
    }

    #[test]
    fn remote_ssh_with_port_is_preserved() {
        // The port must survive — it's needed to build the API URL.
        let (h, o, r) = parse_remote_url("ssh://git@git.example.com:222/team/proj").unwrap();
        assert_eq!(h, "git.example.com:222");
        assert_eq!(o, "team");
        assert_eq!(r, "proj");
    }

    #[test]
    fn remote_https_port_preserved_and_userinfo_dropped() {
        let (h, o, r) = parse_remote_url("https://user:pw@forge.corp:8443/grp/app.git").unwrap();
        assert_eq!(h, "forge.corp:8443");
        assert_eq!(o, "grp");
        assert_eq!(r, "app");
    }

    #[test]
    fn remote_rejects_garbage() {
        assert!(parse_remote_url("not-a-url").is_none());
        assert!(parse_remote_url("git@host:onlyowner").is_none());
    }

    #[test]
    fn forge_detection() {
        assert_eq!(detect_forge_from_host("github.com"), "github");
        assert_eq!(detect_forge_from_host("git.hemoglobina.store"), "forgejo");
        assert_eq!(detect_forge_from_host("gitlab.com"), "gitlab");
        assert_eq!(detect_forge_from_host("gitlab.internal.corp"), "gitlab");
        assert_eq!(detect_forge_from_host("codeberg.org"), "forgejo");
        // Port is ignored for matching.
        assert_eq!(detect_forge_from_host("git.example.com:222"), "forgejo");
        // Suffix match: a look-alike host is NOT github.
        assert_eq!(detect_forge_from_host("github.com.evil.example"), "forgejo");
        assert_eq!(detect_forge_from_host("api.github.com"), "github");
    }

    #[test]
    fn gh_checks_mixed() {
        let rollup = json!([
            {"conclusion": "SUCCESS"},
            {"conclusion": "FAILURE"},
            {"conclusion": "SKIPPED"},
            {"state": "PENDING"},
            {"state": "SUCCESS"},
            {"status": "IN_PROGRESS"}
        ]);
        let c = count_gh_checks(Some(&rollup));
        assert_eq!(c.passing, 3); // SUCCESS, SKIPPED, state SUCCESS
        assert_eq!(c.failing, 1);
        assert_eq!(c.pending, 2); // state PENDING + running checkrun
    }

    #[test]
    fn forgejo_checks_counts() {
        let resp = json!({
            "state": "pending",
            "statuses": [
                {"status": "success"},
                {"status": "pending"},
                {"status": "failure"},
                {"status": "error"}
            ]
        });
        let c = count_forgejo_checks(&resp);
        assert_eq!(c.passing, 1);
        assert_eq!(c.pending, 1);
        assert_eq!(c.failing, 2);
    }

    #[test]
    fn gh_pr_ready_when_green() {
        let v = json!({
            "number": 42,
            "state": "OPEN",
            "title": "Add thing",
            "mergeable": "MERGEABLE",
            "statusCheckRollup": [{"conclusion": "SUCCESS"}],
            "reviewDecision": "APPROVED",
            "comments": [{"id": 1}, {"id": 2}]
        });
        let pr = parse_gh_pr(&v, Some(0)).unwrap();
        assert_eq!(pr.number, 42);
        assert_eq!(pr.state, "open");
        assert_eq!(pr.mergeable, Some(true));
        assert_eq!(pr.checks.passing, 1);
        assert_eq!(pr.comments, 2);
        assert_eq!(pr.review_threads_unresolved, Some(0));
        assert!(pr.ready_to_merge);
    }

    #[test]
    fn gh_pr_not_ready_changes_requested() {
        let v = json!({
            "number": 7,
            "state": "OPEN",
            "title": "WIP",
            "mergeable": "MERGEABLE",
            "statusCheckRollup": [{"conclusion": "SUCCESS"}],
            "reviewDecision": "CHANGES_REQUESTED",
            "comments": []
        });
        let pr = parse_gh_pr(&v, Some(0)).unwrap();
        assert!(!pr.ready_to_merge);
    }

    #[test]
    fn gh_pr_not_ready_failing_check() {
        let v = json!({
            "number": 8,
            "state": "OPEN",
            "title": "Broken",
            "mergeable": "MERGEABLE",
            "statusCheckRollup": [{"conclusion": "FAILURE"}],
            "reviewDecision": "APPROVED",
            "comments": []
        });
        let pr = parse_gh_pr(&v, Some(0)).unwrap();
        assert!(!pr.ready_to_merge);
    }

    #[test]
    fn gh_pr_conflicting_not_ready() {
        let v = json!({
            "number": 9,
            "state": "OPEN",
            "title": "Conflicts",
            "mergeable": "CONFLICTING",
            "statusCheckRollup": [],
            "comments": []
        });
        let pr = parse_gh_pr(&v, None).unwrap();
        assert_eq!(pr.mergeable, Some(false));
        assert!(!pr.ready_to_merge);
    }

    #[test]
    fn gh_pr_not_ready_unresolved_threads() {
        let v = json!({
            "number": 10,
            "state": "OPEN",
            "title": "Has open threads",
            "mergeable": "MERGEABLE",
            "statusCheckRollup": [{"conclusion": "SUCCESS"}],
            "reviewDecision": "APPROVED",
            "comments": []
        });
        // Everything green, but 2 unresolved review threads block readiness.
        let pr = parse_gh_pr(&v, Some(2)).unwrap();
        assert_eq!(pr.review_threads_unresolved, Some(2));
        assert!(!pr.ready_to_merge);
    }

    #[test]
    fn gh_pr_draft_not_ready() {
        let v = json!({
            "number": 11, "state": "OPEN", "title": "Draft", "isDraft": true,
            "mergeable": "MERGEABLE", "statusCheckRollup": [{"conclusion": "SUCCESS"}],
            "reviewDecision": "APPROVED", "comments": []
        });
        let pr = parse_gh_pr(&v, Some(0)).unwrap();
        assert!(pr.is_draft);
        assert!(!pr.ready_to_merge);
    }

    #[test]
    fn gh_pr_bad_merge_state_not_ready() {
        for st in ["DIRTY", "BLOCKED", "UNKNOWN"] {
            let v = json!({
                "number": 12, "state": "OPEN", "title": "x", "mergeable": "MERGEABLE",
                "mergeStateStatus": st, "statusCheckRollup": [{"conclusion": "SUCCESS"}],
                "reviewDecision": "APPROVED", "comments": []
            });
            let pr = parse_gh_pr(&v, Some(0)).unwrap();
            assert!(!pr.ready_to_merge, "{st} should block readiness");
        }
        // CLEAN does not block.
        let v = json!({
            "number": 12, "state": "OPEN", "title": "x", "mergeable": "MERGEABLE",
            "mergeStateStatus": "CLEAN", "statusCheckRollup": [{"conclusion": "SUCCESS"}],
            "reviewDecision": "APPROVED", "comments": []
        });
        assert!(parse_gh_pr(&v, Some(0)).unwrap().ready_to_merge);
    }

    #[test]
    fn gh_pr_unknown_threads_fail_closed() {
        let v = json!({
            "number": 13, "state": "OPEN", "title": "x", "mergeable": "MERGEABLE",
            "statusCheckRollup": [{"conclusion": "SUCCESS"}], "reviewDecision": "APPROVED",
            "comments": []
        });
        // threads unknown (GraphQL failed) → not ready (fail closed).
        let pr = parse_gh_pr(&v, None).unwrap();
        assert!(!pr.ready_to_merge);
    }

    #[test]
    fn count_unresolved_threads_graphql() {
        let resp = json!({
            "data": {"repository": {"pullRequest": {"reviewThreads": {
                "nodes": [
                    {"isResolved": true}, {"isResolved": false},
                    {"isResolved": false}, {"isResolved": true}
                ],
                "pageInfo": {"hasNextPage": false}
            }}}}
        });
        assert_eq!(count_unresolved_threads(&resp), Some(2));
        // Missing path → None (unknown), never panics.
        assert_eq!(count_unresolved_threads(&json!({})), None);
        // GraphQL errors → None.
        assert_eq!(
            count_unresolved_threads(&json!({"errors": [{"message": "x"}]})),
            None
        );
        // All-resolved first page but more pages → None (undecidable).
        let more = json!({"data": {"repository": {"pullRequest": {"reviewThreads": {
            "nodes": [{"isResolved": true}], "pageInfo": {"hasNextPage": true}}}}}});
        assert_eq!(count_unresolved_threads(&more), None);
        // Unresolved on first page + more pages → Some (already not ready).
        let more_unres = json!({"data": {"repository": {"pullRequest": {"reviewThreads": {
            "nodes": [{"isResolved": false}], "pageInfo": {"hasNextPage": true}}}}}});
        assert_eq!(count_unresolved_threads(&more_unres), Some(1));
    }

    #[test]
    fn forgejo_combined_state_fallback() {
        // Empty statuses but failing combined state → counts as failing.
        let resp = json!({"state": "failure", "statuses": []});
        assert_eq!(count_forgejo_checks(&resp).failing, 1);
        // success + empty → no checks configured, all zero.
        let ok = json!({"state": "success", "statuses": []});
        assert_eq!(count_forgejo_checks(&ok), ChecksSummary::default());
    }

    #[test]
    fn forgejo_pr_merged_state() {
        let pr = json!({
            "number": 116,
            "title": "Allow-list editor",
            "state": "closed",
            "merged": true,
            "mergeable": false,
            "comments": 4
        });
        let r = parse_forgejo_pr(&pr, ChecksSummary::default(), true).unwrap();
        assert_eq!(r.state, "merged");
        assert_eq!(r.comments, 4);
        assert!(!r.ready_to_merge); // not open
    }

    #[test]
    fn forgejo_pr_ready_when_checks_known() {
        let pr = json!({
            "number": 5, "title": "Ready one", "state": "open",
            "merged": false, "mergeable": true, "comments": 0
        });
        let checks = ChecksSummary {
            passing: 3,
            failing: 0,
            pending: 0,
        };
        let r = parse_forgejo_pr(&pr, checks.clone(), true).unwrap();
        assert_eq!(r.state, "open");
        assert!(r.ready_to_merge);
        // Same PR but checks unknown (status fetch failed) → fail closed.
        let r2 = parse_forgejo_pr(&pr, checks, false).unwrap();
        assert!(!r2.ready_to_merge);
    }

    #[test]
    fn token_lookup_direct_and_alias() {
        let keys = json!({
            "hosts": {
                "git.hemoglobina.store": {"type": "Application", "name": "h", "token": "secret123"}
            },
            "aliases": {
                "git.hemoglobina.store:9722": "git.hemoglobina.store"
            }
        });
        assert_eq!(
            token_from_keys(&keys, "git.hemoglobina.store").as_deref(),
            Some("secret123")
        );
        assert_eq!(
            token_from_keys(&keys, "git.hemoglobina.store:9722").as_deref(),
            Some("secret123")
        );
        assert_eq!(token_from_keys(&keys, "unknown.host"), None);
    }
}
