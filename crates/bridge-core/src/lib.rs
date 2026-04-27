use serde::Serialize;

/// Structured error returned from tool wrappers.
#[derive(Debug, thiserror::Error)]
pub enum BridgeError {
    #[error("command failed (exit code {code}): {stderr}")]
    CommandFailed { code: i32, stderr: String },

    #[error("command not found: {0}")]
    CommandNotFound(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("parse error: {0}")]
    Parse(String),

    #[error("timeout after {0}ms")]
    Timeout(u64),
}

/// Run a command with arguments, returning (stdout, stderr).
/// Returns BridgeError::CommandFailed if exit code != 0.
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

/// Word count result for a single file or input.
#[derive(Debug, Serialize, Clone)]
pub struct WcResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    pub lines: u64,
    pub words: u64,
    pub bytes: u64,
    pub chars: u64,
}
