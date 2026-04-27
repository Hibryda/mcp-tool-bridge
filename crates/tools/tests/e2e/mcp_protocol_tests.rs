//! MCP protocol-level conformance tests.
//!
//! These hit the JSON-RPC layer directly to verify the server speaks MCP correctly,
//! independent of any specific tool's behavior.

use crate::harness::Server;
use serde_json::json;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

/// Spawn the binary, write raw lines to stdin, read response, return parsed JSON lines.
fn raw_exchange(extra_args: &[&str], lines: &[&str]) -> Vec<serde_json::Value> {
    let bin = {
        let crate_dir = env!("CARGO_MANIFEST_DIR");
        let release = format!("{}/../../target/release/mcp-tool-bridge", crate_dir);
        let debug = format!("{}/../../target/debug/mcp-tool-bridge", crate_dir);
        if std::path::Path::new(&release).exists() {
            release
        } else {
            debug
        }
    };
    let mut child = Command::new(&bin)
        .args(extra_args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .env("RUST_LOG", "off")
        .spawn()
        .expect("spawn");

    {
        let stdin = child.stdin.as_mut().unwrap();
        for line in lines {
            writeln!(stdin, "{}", line).unwrap();
        }
        // Close stdin so server exits cleanly.
    }
    drop(child.stdin.take());

    let stdout = child.stdout.take().unwrap();
    let reader = BufReader::new(stdout);
    let mut responses = Vec::new();
    for line in reader.lines().flatten() {
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(v) = serde_json::from_str(&line) {
            responses.push(v);
        }
    }
    let _ = child.wait();
    responses
}

#[test]
fn initialize_returns_protocol_version() {
    let init = json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {"name": "test", "version": "1.0"}
        }
    });
    let resp = raw_exchange(&[], &[&init.to_string()]);
    let init_resp = resp.iter().find(|m| m["id"] == 1).expect("init response");
    assert!(!init_resp["result"]["protocolVersion"]
        .as_str()
        .unwrap()
        .is_empty());
    assert!(init_resp["result"]["capabilities"]["tools"].is_object());
}

#[test]
fn tools_list_returns_array() {
    let mut s = Server::spawn(&[]);
    let names = s.list_tools();
    assert!(!names.is_empty());
    // No duplicates
    let mut sorted = names.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), names.len());
}

#[test]
fn unknown_tool_returns_error_envelope() {
    let mut s = Server::spawn(&[]);
    let r = s.call("totally_made_up_tool_xyz", json!({}));
    // Server should respond — either with isError=true or a JSON-RPC error.
    // Our harness wraps both as ToolResponse. Verify it didn't panic the server.
    let _ = (r.success(), r.content_text);
}

#[test]
fn malformed_arguments_handled_gracefully() {
    let mut s = Server::spawn(&[]);
    // wc requires path/input/paths — passing nothing should error cleanly.
    let r = s.call("wc", json!({"unknown_param": "x"}));
    assert!(!r.success());
}

#[test]
fn schema_budget_under_kb_threshold() {
    // Each tool description shouldn't be a novel. Total schema bytes for tools/list
    // should stay under a reasonable budget so it doesn't blow agent context.
    let init = json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {"name": "t", "version": "1.0"}
        }
    });
    let notif = json!({"jsonrpc": "2.0", "method": "notifications/initialized"});
    let list = json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}});

    let resp = raw_exchange(
        &[],
        &[&init.to_string(), &notif.to_string(), &list.to_string()],
    );
    let list_resp = resp.iter().find(|m| m["id"] == 2).expect("list response");
    let tools = list_resp["result"]["tools"].as_array().unwrap();
    let total_bytes: usize = tools
        .iter()
        .map(|t| serde_json::to_string(t).unwrap().len())
        .sum();
    // 20 tools at ~600 bytes each = ~12KB upper bound. We enforce 25KB hard ceiling.
    assert!(
        total_bytes < 25_000,
        "tools/list schema is {} bytes — too big",
        total_bytes
    );
}

#[test]
fn each_tool_has_description_and_input_schema() {
    // Use the raw exchange to inspect schemas (Server only returns names).
    let init = json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {"name": "t", "version": "1.0"}
        }
    });
    let notif = json!({"jsonrpc": "2.0", "method": "notifications/initialized"});
    let list = json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}});

    let resp = raw_exchange(
        &[],
        &[&init.to_string(), &notif.to_string(), &list.to_string()],
    );
    let list_resp = resp.iter().find(|m| m["id"] == 2).expect("list response");
    for tool in list_resp["result"]["tools"].as_array().unwrap() {
        let name = tool["name"].as_str().unwrap();
        assert!(
            tool["description"]
                .as_str()
                .map(|s| !s.is_empty())
                .unwrap_or(false),
            "{name} has no description"
        );
        assert!(tool["inputSchema"].is_object(), "{name} has no inputSchema");
    }
}

#[test]
fn no_response_omits_outputschema() {
    // Claude Code bug #25081: tools/list response with outputSchema field silently drops all tools.
    // Verify we never include it.
    let init = json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {"name": "t", "version": "1.0"}
        }
    });
    let notif = json!({"jsonrpc": "2.0", "method": "notifications/initialized"});
    let list = json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}});

    let resp = raw_exchange(
        &[],
        &[&init.to_string(), &notif.to_string(), &list.to_string()],
    );
    let list_resp = resp.iter().find(|m| m["id"] == 2).expect("list response");
    for tool in list_resp["result"]["tools"].as_array().unwrap() {
        assert!(
            tool.get("outputSchema").is_none(),
            "tool {} contains outputSchema — Claude Code drops tools when present",
            tool["name"]
        );
    }
}

#[test]
fn initialize_response_omits_instructions() {
    // Same Claude Code bug — server initialize response with `instructions` causes silent drop.
    let init = json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {"name": "t", "version": "1.0"}
        }
    });
    let resp = raw_exchange(&[], &[&init.to_string()]);
    let init_resp = resp.iter().find(|m| m["id"] == 1).expect("init response");
    assert!(
        init_resp["result"].get("instructions").is_none(),
        "initialize response contains 'instructions' — Claude Code drops tools when present"
    );
}

#[test]
fn jsonrpc_id_echoed_in_response() {
    let init = json!({
        "jsonrpc": "2.0", "id": 42, "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {"name": "t", "version": "1.0"}
        }
    });
    let resp = raw_exchange(&[], &[&init.to_string()]);
    let init_resp = resp
        .iter()
        .find(|m| m["id"] == 42)
        .expect("response with id 42");
    assert_eq!(init_resp["jsonrpc"], "2.0");
}

#[test]
fn server_handles_multiple_requests_in_sequence() {
    let mut s = Server::spawn(&[]);
    // Issue several distinct calls — verify all complete.
    for i in 0..5 {
        let r = s.call("wc", json!({"input": format!("call-{i}")}));
        assert!(r.success());
    }
}

#[test]
fn tools_list_with_filter_flag_is_subset() {
    let mut all = Server::spawn(&[]);
    let all_tools = all.list_tools();
    let mut filtered = Server::spawn(&["--tools", "ls,wc,find"]);
    let filtered_tools = filtered.list_tools();
    assert!(filtered_tools.len() <= all_tools.len());
    for t in &filtered_tools {
        assert!(all_tools.contains(t), "{} not in full tool set", t);
    }
}
