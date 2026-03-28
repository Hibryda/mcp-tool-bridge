use rmcp::{
    ErrorData as McpError, ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, Content, ServerCapabilities, ServerInfo},
    schemars, tool, tool_handler, tool_router,
    transport::stdio,
};
use serde::Deserialize;

mod ls;
mod wc;

// ── Tool parameter types ──────────────────────────────────────────────

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
