//! Shared E2E test harness.
//!
//! Spawns the release binary, communicates over stdio JSON-RPC, and provides
//! ergonomic helpers for tool calls.

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;

/// One running mcp-tool-bridge process held open across multiple calls.
pub struct Server {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl Server {
    /// Spawn the binary, perform the MCP initialize handshake, return ready server.
    pub fn spawn(extra_args: &[&str]) -> Self {
        let bin = binary_path();
        let mut cmd = Command::new(&bin);
        cmd.args(extra_args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .env("RUST_LOG", "off");

        let mut child = cmd.spawn().expect("spawn mcp-tool-bridge");
        let stdin = child.stdin.take().expect("stdin");
        let stdout = BufReader::new(child.stdout.take().expect("stdout"));

        let mut s = Server {
            child,
            stdin,
            stdout,
            next_id: 1,
        };
        s.handshake();
        s
    }

    fn handshake(&mut self) {
        let init = json!({
            "jsonrpc": "2.0",
            "id": 0,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "e2e", "version": "1.0"}
            }
        });
        self.write(&init);
        let _ = self.read_id(0);
        let notif = json!({"jsonrpc": "2.0", "method": "notifications/initialized"});
        self.write(&notif);
    }

    /// Call a tool, return parsed JSON response or error envelope.
    pub fn call(&mut self, name: &str, args: Value) -> ToolResponse {
        let id = self.next_id;
        self.next_id += 1;
        let req = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/call",
            "params": {"name": name, "arguments": args}
        });
        self.write(&req);
        let msg = self.read_id(id);
        ToolResponse::from_message(msg)
    }

    /// List all registered tool names.
    pub fn list_tools(&mut self) -> Vec<String> {
        let id = self.next_id;
        self.next_id += 1;
        let req = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/list",
            "params": {}
        });
        self.write(&req);
        let msg = self.read_id(id);
        msg["result"]["tools"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|t| t["name"].as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn write(&mut self, msg: &Value) {
        writeln!(self.stdin, "{}", msg).expect("write");
        self.stdin.flush().ok();
    }

    fn read_id(&mut self, want_id: u64) -> Value {
        loop {
            let mut line = String::new();
            self.stdout.read_line(&mut line).expect("read");
            if line.trim().is_empty() {
                panic!("server closed stdout");
            }
            let v: Value = serde_json::from_str(line.trim()).expect("valid json");
            // Skip server-initiated notifications, match on id.
            if v.get("id").and_then(|i| i.as_u64()) == Some(want_id) {
                return v;
            }
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Parsed tool response: success returns content as JSON, error returns the message.
pub struct ToolResponse {
    pub is_error: bool,
    pub content_text: String,
    pub data: Value,
}

impl ToolResponse {
    fn from_message(msg: Value) -> Self {
        let result = &msg["result"];
        let is_error = result["isError"].as_bool().unwrap_or(false);
        let content_text = result["content"]
            .as_array()
            .and_then(|a| a.first())
            .and_then(|c| c["text"].as_str())
            .unwrap_or("")
            .to_string();
        let data =
            serde_json::from_str(&content_text).unwrap_or(Value::String(content_text.clone()));
        ToolResponse {
            is_error,
            content_text,
            data,
        }
    }

    pub fn success(&self) -> bool {
        !self.is_error
    }
}

/// Resolve the binary path. Prefer release if it exists, otherwise debug.
fn binary_path() -> String {
    // Cargo sets CARGO_TARGET_TMPDIR for integration tests, but the binary
    // we built via `cargo build` lives at target/release/ or target/debug/.
    let target_dir = std::env::var("CARGO_TARGET_DIR").unwrap_or_else(|_| {
        // tests run from crate dir; binary is at ../../target relative to crate
        let crate_dir = env!("CARGO_MANIFEST_DIR");
        format!("{}/../../target", crate_dir)
    });
    let release = format!("{}/release/mcp-tool-bridge", target_dir);
    let debug = format!("{}/debug/mcp-tool-bridge", target_dir);
    if std::path::Path::new(&release).exists() {
        release
    } else if std::path::Path::new(&debug).exists() {
        debug
    } else {
        panic!(
            "binary not found. Run `cargo build` or `cargo build --release` first.\nTried: {}\n       {}",
            release, debug
        );
    }
}

/// Skip a test if the `integration-real` feature is not enabled.
/// Real-infra tests (docker/k8s/github/public-internet) are gated by this.
#[macro_export]
macro_rules! require_real_infra {
    () => {
        if !cfg!(feature = "integration-real") {
            eprintln!("[skip] requires --features integration-real");
            return;
        }
    };
}

/// Lazy global build lock to avoid races when multiple test files spawn servers.
#[allow(dead_code)]
pub static BUILD_LOCK: Mutex<()> = Mutex::new(());

/// Sleep helper for occasional timing tests.
#[allow(dead_code)]
pub fn sleep(ms: u64) {
    std::thread::sleep(Duration::from_millis(ms));
}
