use rmcp::{
    ErrorData as McpError, ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, Content, ServerCapabilities, ServerInfo},
    schemars, tool, tool_handler, tool_router,
    transport::stdio,
};
use serde::Deserialize;

mod diff;
mod docker;
mod kubectl;
mod ls;
mod lsof;
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

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct WcParams {
    /// File path to count. Mutually exclusive with `input`.
    path: Option<String>,
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

// ── MCP Server ────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct ToolBridge {
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl ToolBridge {
    fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "List directory contents as structured JSON. Returns entries with name, path, type, size, permissions, and modified time.")]
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

    #[tool(description = "Parse unified diff into structured hunks with line numbers. Provide raw diff text via `input`, or run `git diff` via `git_args`. Returns typed hunks with old/new line numbers, additions, deletions, and context.")]
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
                        return Ok(CallToolResult::error(vec![Content::text(
                            format!("git diff failed: {e}"),
                        )]));
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

    #[tool(description = "List open files and network sockets as structured JSON. Filter by port, PID, or protocol. Returns processes with typed file descriptors including fd, type, protocol, and name.")]
    async fn lsof(
        &self,
        Parameters(params): Parameters<LsofParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut args: Vec<String> = vec!["-n".to_string(), "-P".to_string()];

        if let Some(ref port) = params.port {
            let port_filter = if port.starts_with(':') {
                format!("-i{}", port)
            } else {
                format!("-i:{}", port)
            };
            args.push(port_filter);
        }

        if let Some(ref pid) = params.pid {
            args.push("-p".to_string());
            args.push(pid.clone());
        }

        if let Some(ref proto) = params.protocol {
            args.push(format!("-i{}", proto));
        }

        if params.network_only.unwrap_or(false) && params.port.is_none() && params.protocol.is_none() {
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
            Err(e) => Ok(CallToolResult::error(vec![Content::text(
                format!("lsof failed: {e}"),
            )])),
        }
    }

    #[tool(description = "List Kubernetes resources as structured JSON with typed metadata (name, namespace, uid, labels, annotations, timestamps). Spec and status are passthrough JSON. Works with any resource type.")]
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
            Err(e) => Ok(CallToolResult::error(vec![Content::text(format!("kubectl failed: {e}"))])),
        }
    }

    #[tool(description = "Get a single Kubernetes resource as structured JSON with typed metadata. Spec and status are passthrough JSON.")]
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
            Err(e) => Ok(CallToolResult::error(vec![Content::text(format!("kubectl failed: {e}"))])),
        }
    }

    #[tool(description = "List Docker containers as structured JSON with id, name, image, state, status, ports, and labels. Connects to local Docker daemon.")]
    async fn docker_list(
        &self,
        Parameters(params): Parameters<DockerListParams>,
    ) -> Result<CallToolResult, McpError> {
        let client = docker::connect()
            .map_err(|e| McpError::internal_error(e, None))?;

        match docker::list_containers(&client, params.all.unwrap_or(false)).await {
            Ok(result) => {
                let json = serde_json::to_string_pretty(&result)
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e)])),
        }
    }

    #[tool(description = "Inspect a Docker container. Returns structured state (running, pid, exit code), network settings, mounts, and config.")]
    async fn docker_inspect(
        &self,
        Parameters(params): Parameters<DockerInspectParams>,
    ) -> Result<CallToolResult, McpError> {
        let client = docker::connect()
            .map_err(|e| McpError::internal_error(e, None))?;

        match docker::inspect_container(&client, &params.container).await {
            Ok(result) => {
                let json = serde_json::to_string_pretty(&result)
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e)])),
        }
    }

    #[tool(description = "List Docker images as structured JSON with id, tags, size, and created timestamp.")]
    async fn docker_images(
        &self,
    ) -> Result<CallToolResult, McpError> {
        let client = docker::connect()
            .map_err(|e| McpError::internal_error(e, None))?;

        match docker::list_images(&client).await {
            Ok(result) => {
                let json = serde_json::to_string_pretty(&result)
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e)])),
        }
    }

    #[tool(description = "Execute a read-only SQL query against a SQLite database. Returns structured columns and typed rows (integers, floats, strings, nulls).")]
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

    #[tool(description = "List tables and their schemas in a SQLite database. Returns table names with column info (name, type, not_null, primary_key).")]
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

    #[tool(description = "Count lines, words, bytes, and characters. Provide either a file path or raw text input. Returns structured counts.")]
    async fn wc(
        &self,
        Parameters(params): Parameters<WcParams>,
    ) -> Result<CallToolResult, McpError> {
        let result = match (&params.path, &params.input) {
            (Some(path), None) => wc::word_count(path).await.map_err(|e| {
                McpError::internal_error(e.to_string(), None)
            })?,
            (None, Some(input)) => wc::word_count_str(input),
            (Some(_), Some(_)) => {
                return Ok(CallToolResult::error(vec![Content::text(
                    "Provide either 'path' or 'input', not both.",
                )]));
            }
            (None, None) => {
                return Ok(CallToolResult::error(vec![Content::text(
                    "Provide either 'path' (file to count) or 'input' (text to count).",
                )]));
            }
        };

        let json = serde_json::to_string_pretty(&result)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }
}

// ── Handler ───────────────────────────────────────────────────────────

#[tool_handler]
impl ServerHandler for ToolBridge {
    fn get_info(&self) -> ServerInfo {
        // NOTE: omit .with_instructions() — Claude Code bug #25081 silently drops tools
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
    }
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

    tracing::info!("mcp-tool-bridge v{} starting", env!("CARGO_PKG_VERSION"));

    let service = ToolBridge::new()
        .serve(stdio())
        .await
        .inspect_err(|e| tracing::error!("serving error: {:?}", e))?;

    service.waiting().await?;
    Ok(())
}
