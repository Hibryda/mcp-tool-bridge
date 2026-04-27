use rmcp::{
    handler::server::{router::tool::ToolRouter, tool::ToolCallContext, wrapper::Parameters},
    model::{
        CallToolRequestParams, CallToolResult, Content, ListToolsResult, PaginatedRequestParams,
        ServerCapabilities, ServerInfo,
    },
    schemars,
    service::{RequestContext, RoleServer},
    tool, tool_router,
    transport::stdio,
    ErrorData as McpError, ServerHandler, ServiceExt,
};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};

mod batch;
mod curl;
mod diff;
mod dispatch;
mod docker;
mod find;
mod gh_api;
mod git_log;
mod git_show;
mod git_status;
mod kubectl;
mod ls;
mod lsof;
mod pipe;
mod ps;
mod sqlite;
mod wc;

// ── Tool parameter types ──────────────────────────────────────────────

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct KubectlListParams {
    /// Kubernetes resource type. Examples: "pods", "deployments", "services", "configmaps", "nodes".
    resource_type: String,
    /// Namespace to query. Defaults to "default".
    namespace: Option<String>,
    /// Label selector. Example: "app=nginx".
    label_selector: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct KubectlGetParams {
    /// Kubernetes resource type. Examples: "pod", "deployment", "service".
    resource_type: String,
    /// Resource name.
    name: String,
    /// Namespace. Defaults to "default".
    namespace: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct LsParams {
    /// Directory path to list. Defaults to current directory.
    path: Option<String>,
    /// Include hidden files (dotfiles).
    all: Option<bool>,
    /// Include size, permissions, and modification time.
    long: Option<bool>,
}

#[derive(Debug, Deserialize, serde::Serialize, schemars::JsonSchema)]
struct WcParams {
    /// File path to count. Mutually exclusive with `input` and `paths`.
    path: Option<String>,
    /// Multiple file paths to count in one call. Returns array of results.
    paths: Option<Vec<String>>,
    /// Raw text input to count (for piped/inline content).
    input: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct DiffParams {
    /// Unified diff text to parse. Provide either this or `git_args`.
    input: Option<String>,
    /// Arguments to pass to `git diff --no-ext-diff`. Example: ["HEAD~1"], ["--cached"], ["main..feature"].
    git_args: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct LsofParams {
    /// Filter by port number. Example: "8080" or ":8080".
    port: Option<String>,
    /// Filter by PID.
    pid: Option<String>,
    /// Filter by protocol: "TCP", "UDP".
    protocol: Option<String>,
    /// Show only network sockets (equivalent to -i).
    #[schemars(default)]
    network_only: Option<bool>,
    /// Extra arguments to pass to lsof.
    extra_args: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct DockerListParams {
    /// Include stopped containers.
    #[schemars(default)]
    all: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct DockerInspectParams {
    /// Container ID or name.
    container: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct SqliteQueryParams {
    /// Path to the SQLite database file.
    db_path: String,
    /// SQL query to execute (read-only).
    sql: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct SqliteTablesParams {
    /// Path to the SQLite database file.
    db_path: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct GitStatusParams {
    /// Path to the git repository. Defaults to current directory.
    path: Option<String>,
    /// Show untracked files. Defaults to true.
    show_untracked: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct GitLogParams {
    /// Path to the git repository. Defaults to current directory.
    path: Option<String>,
    /// Maximum number of commits (default 50, max 200).
    max_count: Option<u32>,
    /// Include file-level stats (additions/deletions per file).
    include_stats: Option<bool>,
    /// Hash to start after (for pagination). Use last_hash from previous result.
    after_hash: Option<String>,
    /// Snapshot OID for stable pagination. Use snapshot_oid from first page.
    snapshot_oid: Option<String>,
    /// Branch name to query. Defaults to HEAD.
    branch: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct GitShowParams {
    /// Path to the git repository. Defaults to current directory.
    path: Option<String>,
    /// Git reference (commit hash, tag, branch, HEAD, HEAD~1, etc.).
    #[serde(rename = "ref")]
    reference: String,
    /// Include file-level stats.
    include_stats: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct PsParams {
    /// Filter by process name or command (substring match).
    name_pattern: Option<String>,
    /// Filter by user (exact match).
    user: Option<String>,
    /// Filter by PIDs.
    pid_list: Option<Vec<u64>>,
    /// Maximum results (default 100, max 500).
    max_results: Option<u32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct GhApiParams {
    /// GitHub API endpoint path (must start with /). Example: /repos/owner/repo/pulls.
    endpoint: String,
    /// HTTP method (default GET). Use GET for read operations.
    method: Option<String>,
    /// Request body as JSON string (for POST/PATCH/PUT).
    body: Option<String>,
    /// Enable pagination (mutually exclusive with rate limit info).
    paginate: Option<bool>,
    /// Maximum items to return when paginating (default 200, max 1000).
    max_items: Option<u32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct FindParams {
    /// Root directory to search from.
    path: Option<String>,
    /// Name pattern with glob support (*.rs, Cargo*, etc).
    name: Option<String>,
    /// File type filter: "file", "directory", or "symlink".
    #[serde(rename = "type")]
    file_type: Option<String>,
    /// Maximum depth to recurse.
    max_depth: Option<u32>,
    /// Minimum file size in bytes.
    min_size: Option<u64>,
    /// Maximum file size in bytes.
    max_size: Option<u64>,
    /// Maximum results to return (default 1000).
    limit: Option<usize>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct CurlParams {
    /// URL to request.
    url: String,
    /// HTTP method. Defaults to GET.
    method: Option<String>,
    /// Request headers as key-value pairs.
    headers: Option<HashMap<String, String>>,
    /// Request body (for POST, PUT, PATCH).
    body: Option<String>,
    /// Follow redirects. Defaults to true.
    follow_redirects: Option<bool>,
    /// Timeout in seconds. Defaults to 30.
    timeout_secs: Option<u64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct BatchParams {
    /// Array of operations to execute in parallel. Each has a tool name and params.
    operations: Vec<BatchOperationParams>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct BatchOperationParams {
    /// Tool name (must be registered in this server).
    tool: String,
    /// Parameters for the tool (same schema as calling the tool directly).
    params: serde_json::Value,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct PipeParams {
    /// Source tool to run. Must be a listing tool: ls, lsof, kubectl_list, docker_list, docker_images.
    source: PipeSourceParams,
    /// Filters to apply (AND semantics — all must match). Each filter checks a field against a pattern.
    filters: Vec<PipeFilterParams>,
    /// Maximum number of results to return.
    limit: Option<usize>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct PipeSourceParams {
    /// Tool name (must be a listing tool).
    tool: String,
    /// Parameters for the source tool.
    params: serde_json::Value,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct PipeFilterParams {
    /// Field name to filter on. Supports dot notation for nested fields (e.g. "metadata.name").
    field: String,
    /// Pattern to match against the field value.
    pattern: String,
    /// Match mode: "contains", "equals", or "starts_with".
    mode: String,
}

// ── MCP Server ────────────────────────────────────────────────────────

#[derive(Clone)]
struct ToolBridge {
    tool_router: ToolRouter<Self>,
    enabled_tools: Option<HashSet<String>>,
    dispatch_table: HashMap<String, dispatch::DispatchFn>,
}

impl std::fmt::Debug for ToolBridge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolBridge")
            .field("enabled_tools", &self.enabled_tools)
            .field(
                "dispatch_table_keys",
                &self.dispatch_table.keys().collect::<Vec<_>>(),
            )
            .finish()
    }
}

#[tool_router]
impl ToolBridge {
    fn new(enabled: Option<HashSet<String>>) -> Self {
        let dispatch_table = dispatch::build_dispatch_table(&enabled);
        Self {
            tool_router: Self::tool_router(),
            enabled_tools: enabled,
            dispatch_table,
        }
    }

    fn is_enabled(&self, name: &str) -> bool {
        self.enabled_tools
            .as_ref()
            .is_none_or(|set| set.contains(name))
    }

    #[tool(
        description = "List directory contents as structured JSON. Returns entries with name, path, type, size, permissions, and modified time."
    )]
    async fn ls(
        &self,
        Parameters(params): Parameters<LsParams>,
    ) -> Result<CallToolResult, McpError> {
        let path = params.path.as_deref().unwrap_or(".");
        let all = params.all.unwrap_or(false);
        let long = params.long.unwrap_or(true);

        match ls::list_directory(path, all, long).await {
            Ok(entries) => {
                let json = serde_json::to_string_pretty(&entries)
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
        }
    }

    #[tool(
        description = "Parse unified diff into structured hunks with line numbers. Provide raw diff text via `input`, or run `git diff` via `git_args`. Returns typed hunks with old/new line numbers, additions, deletions, and context."
    )]
    async fn diff(
        &self,
        Parameters(params): Parameters<DiffParams>,
    ) -> Result<CallToolResult, McpError> {
        let diff_text = match (&params.input, &params.git_args) {
            (Some(input), None) => input.clone(),
            (None, Some(args)) => {
                let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
                match diff::run_diff(&arg_refs).await {
                    Ok(output) => output,
                    Err(e) => {
                        return Ok(CallToolResult::error(vec![Content::text(format!(
                            "git diff failed: {e}"
                        ))]));
                    }
                }
            }
            (Some(_), Some(_)) => {
                return Ok(CallToolResult::error(vec![Content::text(
                    "Provide either 'input' (raw diff text) or 'git_args', not both.",
                )]));
            }
            (None, None) => {
                return Ok(CallToolResult::error(vec![Content::text(
                    "Provide either 'input' (raw diff text) or 'git_args' (e.g. [\"HEAD~1\"]).",
                )]));
            }
        };

        if diff_text.trim().is_empty() {
            let result = diff::DiffResult {
                format: "unified".to_string(),
                files: vec![],
                total_additions: 0,
                total_deletions: 0,
            };
            let json = serde_json::to_string_pretty(&result)
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;
            return Ok(CallToolResult::success(vec![Content::text(json)]));
        }

        match diff::parse_unified_diff(&diff_text) {
            Ok(result) => {
                let json = serde_json::to_string_pretty(&result)
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
            Err(fmt_err) => {
                let json = serde_json::to_string_pretty(&fmt_err)
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                Ok(CallToolResult::error(vec![Content::text(json)]))
            }
        }
    }

    #[tool(
        description = "List open files and network sockets as structured JSON. Filter by port, PID, or protocol. Returns processes with typed file descriptors including fd, type, protocol, and name."
    )]
    async fn lsof(
        &self,
        Parameters(params): Parameters<LsofParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut args: Vec<String> = vec!["-n".to_string(), "-P".to_string()];

        // Combine protocol and port into single -i flag
        match (params.protocol.as_deref(), params.port.as_deref()) {
            (Some(pr), Some(pt)) => {
                let pt = pt.strip_prefix(':').unwrap_or(pt);
                args.push(format!("-i{pr}:{pt}"));
            }
            (None, Some(pt)) => {
                if pt.starts_with(':') {
                    args.push(format!("-i{pt}"));
                } else {
                    args.push(format!("-i:{pt}"));
                }
            }
            (Some(pr), None) => {
                args.push(format!("-i{pr}"));
            }
            (None, None) => {}
        }

        if let Some(ref pid) = params.pid {
            args.push("-p".to_string());
            args.push(pid.clone());
        }

        if params.network_only.unwrap_or(false)
            && params.port.is_none()
            && params.protocol.is_none()
        {
            args.push("-i".to_string());
        }

        if let Some(ref extra) = params.extra_args {
            args.extend(extra.iter().cloned());
        }

        let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        match lsof::run_lsof(&arg_refs).await {
            Ok(output) => {
                let result = lsof::parse_lsof_output(&output);
                let json = serde_json::to_string_pretty(&result)
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(format!(
                "lsof failed: {e}"
            ))])),
        }
    }

    #[tool(
        description = "Structured git status: branch info (head, upstream, ahead/behind), file entries (modified/added/deleted/renamed/untracked), counts. Uses --porcelain=v2. Typed errors: NOT_A_REPO, DETACHED_HEAD, VERSION_TOO_OLD."
    )]
    async fn git_status(
        &self,
        Parameters(params): Parameters<GitStatusParams>,
    ) -> Result<CallToolResult, McpError> {
        let value = serde_json::json!({
            "path": params.path.as_deref().unwrap_or("."),
            "show_untracked": params.show_untracked.unwrap_or(true),
        });
        match dispatch::do_git_status(value).await {
            Ok(result) => {
                let json = serde_json::to_string_pretty(&result)
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e)])),
        }
    }

    #[tool(
        description = "Structured git log: commits with hash, author, date, subject, parent hashes, refs, merge detection. Stable pagination via snapshot_oid. Optional file-level stats (--numstat)."
    )]
    async fn git_log(
        &self,
        Parameters(params): Parameters<GitLogParams>,
    ) -> Result<CallToolResult, McpError> {
        let value = serde_json::json!({
            "path": params.path.as_deref().unwrap_or("."),
            "max_count": params.max_count.unwrap_or(50),
            "include_stats": params.include_stats.unwrap_or(false),
            "after_hash": params.after_hash,
            "snapshot_oid": params.snapshot_oid,
            "branch": params.branch,
        });
        match dispatch::do_git_log(value).await {
            Ok(result) => {
                let json = serde_json::to_string_pretty(&result)
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e)])),
        }
    }

    #[tool(
        description = "Show a single git commit: hash, author, date, subject, body, parents, merge detection. Restricted to commit objects (blobs/trees return typed error). Optional file stats."
    )]
    async fn git_show(
        &self,
        Parameters(params): Parameters<GitShowParams>,
    ) -> Result<CallToolResult, McpError> {
        let value = serde_json::json!({
            "path": params.path.as_deref().unwrap_or("."),
            "ref": params.reference,
            "include_stats": params.include_stats.unwrap_or(false),
        });
        match dispatch::do_git_show(value).await {
            Ok(result) => {
                let json = serde_json::to_string_pretty(&result)
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e)])),
        }
    }

    #[tool(
        description = "Structured process listing: PID, PPID, user, command, args, CPU%, memory RSS, elapsed time. Filter by name pattern, user, or PID list. Cross-platform (Linux + macOS)."
    )]
    async fn ps(
        &self,
        Parameters(params): Parameters<PsParams>,
    ) -> Result<CallToolResult, McpError> {
        let value = serde_json::json!({
            "name_pattern": params.name_pattern,
            "user": params.user,
            "pid_list": params.pid_list,
            "max_results": params.max_results.unwrap_or(100),
        });
        match dispatch::do_ps(value).await {
            Ok(result) => {
                let json = serde_json::to_string_pretty(&result)
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e)])),
        }
    }

    #[tool(
        description = "GitHub API access via gh CLI. Returns structured JSON with status code, body, rate limit info, and pagination. Read-only by default. Auth tokens redacted from errors."
    )]
    async fn gh_api(
        &self,
        Parameters(params): Parameters<GhApiParams>,
    ) -> Result<CallToolResult, McpError> {
        let value = serde_json::json!({
            "endpoint": params.endpoint,
            "method": params.method.as_deref().unwrap_or("GET"),
            "body": params.body,
            "paginate": params.paginate.unwrap_or(false),
            "max_items": params.max_items.unwrap_or(200),
        });
        match dispatch::do_gh_api(value).await {
            Ok(result) => {
                let json = serde_json::to_string_pretty(&result)
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e)])),
        }
    }

    #[tool(
        description = "Recursively find files with filters: name glob (*.rs, Cargo*), type (file/directory/symlink), size range, max depth. Returns structured entries with path, type, size, permissions, modified time, depth."
    )]
    async fn find(
        &self,
        Parameters(params): Parameters<FindParams>,
    ) -> Result<CallToolResult, McpError> {
        let value = serde_json::json!({
            "path": params.path.as_deref().unwrap_or("."),
            "name": params.name,
            "type": params.file_type,
            "max_depth": params.max_depth,
            "min_size": params.min_size,
            "max_size": params.max_size,
            "limit": params.limit,
        });
        match dispatch::do_find(value).await {
            Ok(result) => {
                let json = serde_json::to_string_pretty(&result)
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e)])),
        }
    }

    #[tool(
        description = "HTTP request with structured response: status code, headers, body, timing breakdown (DNS, connect, TLS, first byte, total), redirect count, content type detection, and JSON body detection."
    )]
    async fn curl(
        &self,
        Parameters(params): Parameters<CurlParams>,
    ) -> Result<CallToolResult, McpError> {
        let value = serde_json::json!({
            "url": params.url,
            "method": params.method.as_deref().unwrap_or("GET"),
            "headers": params.headers.unwrap_or_default(),
            "body": params.body,
            "follow_redirects": params.follow_redirects.unwrap_or(true),
            "timeout_secs": params.timeout_secs.unwrap_or(30),
        });
        match dispatch::do_curl(value).await {
            Ok(result) => {
                let json = serde_json::to_string_pretty(&result)
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e)])),
        }
    }

    #[tool(
        description = "List Kubernetes resources as structured JSON with typed metadata (name, namespace, uid, labels, annotations, timestamps). Spec and status are passthrough JSON. Works with any resource type."
    )]
    async fn kubectl_list(
        &self,
        Parameters(params): Parameters<KubectlListParams>,
    ) -> Result<CallToolResult, McpError> {
        let ns = params.namespace.as_deref().unwrap_or("default");
        let mut extra: Vec<String> = Vec::new();
        if let Some(ref sel) = params.label_selector {
            extra.push("-l".to_string());
            extra.push(sel.clone());
        }
        let extra_refs: Vec<&str> = extra.iter().map(|s| s.as_str()).collect();

        match kubectl::kubectl_get(&params.resource_type, None, ns, &extra_refs).await {
            Ok(output) => match kubectl::parse_list_response(&output, &params.resource_type, ns) {
                Ok(result) => {
                    let json = serde_json::to_string_pretty(&result)
                        .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                    Ok(CallToolResult::success(vec![Content::text(json)]))
                }
                Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
            },
            Err(e) => Ok(CallToolResult::error(vec![Content::text(format!(
                "kubectl failed: {e}"
            ))])),
        }
    }

    #[tool(
        description = "Get a single Kubernetes resource as structured JSON with typed metadata. Spec and status are passthrough JSON."
    )]
    async fn kubectl_get(
        &self,
        Parameters(params): Parameters<KubectlGetParams>,
    ) -> Result<CallToolResult, McpError> {
        let ns = params.namespace.as_deref().unwrap_or("default");

        match kubectl::kubectl_get(&params.resource_type, Some(&params.name), ns, &[]).await {
            Ok(output) => match kubectl::parse_get_response(&output) {
                Ok(result) => {
                    let json = serde_json::to_string_pretty(&result)
                        .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                    Ok(CallToolResult::success(vec![Content::text(json)]))
                }
                Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
            },
            Err(e) => Ok(CallToolResult::error(vec![Content::text(format!(
                "kubectl failed: {e}"
            ))])),
        }
    }

    #[tool(
        description = "List Docker containers as structured JSON with id, name, image, state, status, ports, and labels. Connects to local Docker daemon."
    )]
    async fn docker_list(
        &self,
        Parameters(params): Parameters<DockerListParams>,
    ) -> Result<CallToolResult, McpError> {
        let client = docker::connect().map_err(|e| McpError::internal_error(e, None))?;

        match docker::list_containers(&client, params.all.unwrap_or(false)).await {
            Ok(result) => {
                let json = serde_json::to_string_pretty(&result)
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e)])),
        }
    }

    #[tool(
        description = "Inspect a Docker container. Returns structured state (running, pid, exit code), network settings, mounts, and config."
    )]
    async fn docker_inspect(
        &self,
        Parameters(params): Parameters<DockerInspectParams>,
    ) -> Result<CallToolResult, McpError> {
        let client = docker::connect().map_err(|e| McpError::internal_error(e, None))?;

        match docker::inspect_container(&client, &params.container).await {
            Ok(result) => {
                let json = serde_json::to_string_pretty(&result)
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e)])),
        }
    }

    #[tool(
        description = "List Docker images as structured JSON with id, tags, size, and created timestamp."
    )]
    async fn docker_images(&self) -> Result<CallToolResult, McpError> {
        let client = docker::connect().map_err(|e| McpError::internal_error(e, None))?;

        match docker::list_images(&client).await {
            Ok(result) => {
                let json = serde_json::to_string_pretty(&result)
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e)])),
        }
    }

    #[tool(
        description = "Execute a read-only SQL query against a SQLite database. Returns structured columns and typed rows (integers, floats, strings, nulls)."
    )]
    async fn sqlite_query(
        &self,
        Parameters(params): Parameters<SqliteQueryParams>,
    ) -> Result<CallToolResult, McpError> {
        match sqlite::query(&params.db_path, &params.sql) {
            Ok(result) => {
                let json = serde_json::to_string_pretty(&result)
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e)])),
        }
    }

    #[tool(
        description = "List tables and their schemas in a SQLite database. Returns table names with column info (name, type, not_null, primary_key)."
    )]
    async fn sqlite_tables(
        &self,
        Parameters(params): Parameters<SqliteTablesParams>,
    ) -> Result<CallToolResult, McpError> {
        match sqlite::list_tables(&params.db_path) {
            Ok(tables) => {
                let json = serde_json::to_string_pretty(&tables)
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e)])),
        }
    }

    #[tool(
        description = "Count lines, words, bytes, and characters. Provide a file path, multiple paths, or raw text input. Returns structured counts."
    )]
    async fn wc(
        &self,
        Parameters(params): Parameters<WcParams>,
    ) -> Result<CallToolResult, McpError> {
        // Use dispatch free function which handles paths/path/input
        let value = serde_json::to_value(&params)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        match dispatch::do_wc(value).await {
            Ok(result) => {
                let json = serde_json::to_string_pretty(&result)
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e)])),
        }
    }

    #[tool(
        description = "Run multiple tool operations in a single call. Operations execute in parallel. Each operation uses the same params as calling the tool directly. Returns all results with per-operation success/error isolation."
    )]
    async fn batch(
        &self,
        Parameters(params): Parameters<BatchParams>,
    ) -> Result<CallToolResult, McpError> {
        let ops: Vec<batch::BatchOperation> = params
            .operations
            .into_iter()
            .map(|op| batch::BatchOperation {
                tool: op.tool,
                params: op.params,
            })
            .collect();

        let result = batch::execute_batch(ops, &self.dispatch_table, 4, 30).await;
        let json = serde_json::to_string_pretty(&result)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(
        description = "Run a listing tool and filter its output on structured fields. Filters use AND semantics. Source tools: ls, lsof, kubectl_list, docker_list, docker_images. Filter modes: contains, equals, starts_with. Supports dot notation for nested fields (e.g. metadata.name)."
    )]
    async fn pipe(
        &self,
        Parameters(params): Parameters<PipeParams>,
    ) -> Result<CallToolResult, McpError> {
        let request = pipe::PipeRequest {
            source: pipe::PipeSource {
                tool: params.source.tool,
                params: params.source.params,
            },
            filters: params
                .filters
                .into_iter()
                .map(|f| pipe::Filter {
                    field: f.field,
                    pattern: f.pattern,
                    mode: match f.mode.as_str() {
                        "equals" => pipe::FilterMode::Equals,
                        "starts_with" => pipe::FilterMode::StartsWith,
                        _ => pipe::FilterMode::Contains,
                    },
                })
                .collect(),
            limit: params.limit,
        };

        match pipe::execute_pipe(request, &self.dispatch_table).await {
            Ok(result) => {
                let json = serde_json::to_string_pretty(&result)
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e)])),
        }
    }
}

// ── Handler ───────────────────────────────────────────────────────────

#[allow(clippy::manual_async_fn)]
impl ServerHandler for ToolBridge {
    fn get_info(&self) -> ServerInfo {
        // NOTE: omit .with_instructions() — Claude Code bug #25081 silently drops tools
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
    }

    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListToolsResult, McpError>> + Send + '_ {
        async move {
            let mut tools = self.tool_router.list_all();
            if let Some(ref enabled) = self.enabled_tools {
                tools.retain(|t| enabled.contains(t.name.as_ref()));
            }
            Ok(ListToolsResult {
                meta: None,
                next_cursor: None,
                tools,
            })
        }
    }

    fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<CallToolResult, McpError>> + Send + '_ {
        async move {
            if !self.is_enabled(&request.name) {
                return Ok(CallToolResult::error(vec![Content::text(format!(
                    "Tool '{}' is not enabled. Restart server with --tools to change.",
                    request.name,
                ))]));
            }
            let tcc = ToolCallContext::new(self, request, context);
            self.tool_router.call(tcc).await
        }
    }

    fn get_tool(&self, name: &str) -> Option<rmcp::model::Tool> {
        if !self.is_enabled(name) {
            return None;
        }
        self.tool_router.get(name).cloned()
    }
}

// ── CLI argument parsing ──────────────────────────────────────────────

const ALL_TOOLS: &[&str] = &[
    "batch",
    "curl",
    "diff",
    "docker_images",
    "docker_inspect",
    "docker_list",
    "find",
    "gh_api",
    "git_log",
    "git_show",
    "git_status",
    "kubectl_get",
    "kubectl_list",
    "ls",
    "lsof",
    "pipe",
    "ps",
    "sqlite_query",
    "sqlite_tables",
    "wc",
];

fn parse_tools_flag() -> Option<HashSet<String>> {
    let args: Vec<String> = std::env::args().collect();
    for (i, arg) in args.iter().enumerate() {
        if arg == "--tools" {
            if let Some(value) = args.get(i + 1) {
                let tools: HashSet<String> = value
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                return Some(tools);
            }
        }
        if let Some(value) = arg.strip_prefix("--tools=") {
            let tools: HashSet<String> = value
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            return Some(tools);
        }
        if arg == "--list-tools" {
            eprintln!("Available tools:");
            for tool in ALL_TOOLS {
                eprintln!("  {tool}");
            }
            std::process::exit(0);
        }
    }
    None // None = all tools enabled
}

// ── Entry point ───────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // MCP protocol owns stdout — all logging goes to stderr
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    let enabled = parse_tools_flag();
    if let Some(ref tools) = enabled {
        tracing::info!(
            "mcp-tool-bridge v{} starting with tools: {}",
            env!("CARGO_PKG_VERSION"),
            tools.iter().cloned().collect::<Vec<_>>().join(", ")
        );
    } else {
        tracing::info!(
            "mcp-tool-bridge v{} starting (all tools)",
            env!("CARGO_PKG_VERSION")
        );
    }

    let service = ToolBridge::new(enabled)
        .serve(stdio())
        .await
        .inspect_err(|e| tracing::error!("serving error: {:?}", e))?;

    service.waiting().await?;
    Ok(())
}
