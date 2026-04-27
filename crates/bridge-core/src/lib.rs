//! Shared types and utilities for mcp-tool-bridge tools.
//!
//! This crate holds the small set of types reused across tool implementations
//! (errors, file metadata, word counts) and the [`run_command`] helper that
//! standardises subprocess invocation with structured errors.
//!
//! # Example
//!
//! ```no_run
//! # async fn ex() -> Result<(), bridge_core::BridgeError> {
//! let output = bridge_core::run_command("echo", &["hello"]).await?;
//! assert!(output.contains("hello"));
//! # Ok(()) }
//! ```

use serde::Serialize;

/// Structured error returned from tool wrappers.
///
/// Variants distinguish four common failure modes so callers can react
/// programmatically (retry, surface to user, fall back) rather than parsing
/// stringly-typed errors.
///
/// # Example
///
/// ```
/// use bridge_core::BridgeError;
///
/// let err = BridgeError::CommandNotFound("zzzfake".into());
/// assert_eq!(err.to_string(), "command not found: zzzfake");
/// ```
#[derive(Debug, thiserror::Error)]
pub enum BridgeError {
    /// External command exited with a non-zero status.
    #[error("command failed (exit code {code}): {stderr}")]
    CommandFailed { code: i32, stderr: String },

    /// Binary not on `$PATH`.
    #[error("command not found: {0}")]
    CommandNotFound(String),

    /// Wraps a `std::io::Error` for file-system / subprocess I/O failures.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// Tool-specific parse failure (e.g. malformed unified diff).
    #[error("parse error: {0}")]
    Parse(String),

    /// Operation exceeded its allowed wall-clock budget.
    #[error("timeout after {0}ms")]
    Timeout(u64),
}

/// Run a command with arguments and return its stdout.
///
/// Returns [`BridgeError::CommandNotFound`] if the binary is missing,
/// [`BridgeError::CommandFailed`] if it exits non-zero, or
/// [`BridgeError::Io`] for other I/O errors. Stderr is captured and
/// surfaced via the error variant, never silently dropped.
///
/// # Example
///
/// ```no_run
/// # async fn ex() -> Result<(), bridge_core::BridgeError> {
/// let out = bridge_core::run_command("uname", &["-s"]).await?;
/// assert!(!out.is_empty());
/// # Ok(()) }
/// ```
pub async fn run_command(cmd: &str, args: &[&str]) -> Result<String, BridgeError> {
    let output = tokio::process::Command::new(cmd)
        .args(args)
        .output()
        .await
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                BridgeError::CommandNotFound(cmd.to_string())
            } else {
                BridgeError::Io(e)
            }
        })?;

    if !output.status.success() {
        return Err(BridgeError::CommandFailed {
            code: output.status.code().unwrap_or(-1),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Metadata for a file system entry.
///
/// Used by `ls` and `find`. The `type` JSON field is renamed from the Rust
/// `entry_type` because `type` is a Rust keyword.
///
/// # Example
///
/// ```
/// use bridge_core::FileEntry;
///
/// let entry = FileEntry {
///     name: "Cargo.toml".into(),
///     path: "/repo/Cargo.toml".into(),
///     entry_type: "file".into(),
///     size: 256,
///     permissions: "644".into(),
///     modified: Some("2026-04-27T12:00:00+00:00".into()),
/// };
/// let json = serde_json::to_value(&entry).unwrap();
/// assert_eq!(json["type"], "file");  // serialized as "type", not "entry_type"
/// ```
#[derive(Debug, Serialize, Clone)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    #[serde(rename = "type")]
    pub entry_type: String,
    pub size: u64,
    pub permissions: String,
    pub modified: Option<String>,
}

/// Word count result for a single file or text input.
///
/// `file` is `Some` when the result came from a path argument, `None` when it
/// came from inline text. `bytes` is the raw byte length; `chars` is the UTF-8
/// codepoint count, so `chars <= bytes` always holds.
///
/// # Example
///
/// ```
/// use bridge_core::WcResult;
///
/// let r = WcResult {
///     file: None,
///     lines: 1,
///     words: 2,
///     bytes: 11,
///     chars: 11,
/// };
/// assert!(r.chars <= r.bytes);
/// ```
#[derive(Debug, Serialize, Clone)]
pub struct WcResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    pub lines: u64,
    pub words: u64,
    pub bytes: u64,
    pub chars: u64,
}
