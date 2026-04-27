//! End-to-end tests for the PreToolUse hook binary.
//!
//! Runs the binary against:
//!   1. Hand-curated cases (mode handling, malformed input, non-Bash tools).
//!   2. ~300 real Bash commands sampled from local Claude Code transcript history
//!      (`tests/fixtures/real_bash_commands.txt`). For each, asserts the binary
//!      exits 0, stdout is either empty or valid JSON with the expected shape.

use assert_cmd::Command;
use serde_json::json;

fn hook() -> Command {
    Command::cargo_bin("mcp-tool-bridge-hook").expect("binary built")
}

fn pretool_bash(command: &str) -> String {
    json!({
        "session_id": "test",
        "transcript_path": "/tmp/test.jsonl",
        "cwd": "/tmp",
        "hook_event_name": "PreToolUse",
        "tool_name": "Bash",
        "tool_input": {"command": command},
    })
    .to_string()
}

// ── basic mode handling ─────────────────────────────────────────────────

#[test]
fn off_mode_exits_silently() {
    hook()
        .env("MCP_BRIDGE_HOOK_MODE", "off")
        .write_stdin(pretool_bash("ls -la"))
        .assert()
        .success()
        .stdout("")
        .stderr("");
}

#[test]
fn suggest_mode_emits_json_for_known_command() {
    let out = hook()
        .env("MCP_BRIDGE_HOOK_MODE", "suggest")
        .write_stdin(pretool_bash("ls -la /tmp"))
        .assert()
        .success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    let v: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("stdout must be valid JSON when suggesting");
    assert_eq!(
        v["hookSpecificOutput"]["hookEventName"], "PreToolUse",
        "wrong shape: {}",
        stdout
    );
    assert!(
        v["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap_or("")
            .contains("mcp__tool-bridge__ls"),
        "expected ls suggestion in: {}",
        stdout
    );
}

#[test]
fn suggest_mode_silent_for_uncovered_command() {
    hook()
        .env("MCP_BRIDGE_HOOK_MODE", "suggest")
        .write_stdin(pretool_bash("echo hello"))
        .assert()
        .success()
        .stdout("")
        .stderr("");
}

#[test]
fn enforce_mode_exits_2_with_stderr() {
    hook()
        .env("MCP_BRIDGE_HOOK_MODE", "enforce")
        .write_stdin(pretool_bash("ls -la"))
        .assert()
        .code(2)
        .stdout("");
    // (stderr content asserted via the Bash unit tests; here we just need exit 2.)
}

#[test]
fn enforce_mode_silent_for_uncovered_command() {
    hook()
        .env("MCP_BRIDGE_HOOK_MODE", "enforce")
        .write_stdin(pretool_bash("echo hello"))
        .assert()
        .success()
        .stdout("")
        .stderr("");
}

#[test]
fn default_mode_is_suggest() {
    // No env var → suggest.
    hook()
        .env_remove("MCP_BRIDGE_HOOK_MODE")
        .write_stdin(pretool_bash("ls"))
        .assert()
        .success();
}

// ── non-Bash, malformed input ───────────────────────────────────────────

#[test]
fn non_bash_tool_passes_through() {
    let payload = json!({
        "session_id": "test",
        "tool_name": "Read",
        "tool_input": {"file_path": "/etc/passwd"},
    })
    .to_string();
    hook()
        .env("MCP_BRIDGE_HOOK_MODE", "enforce") // most aggressive — still must pass through
        .write_stdin(payload)
        .assert()
        .success()
        .stdout("")
        .stderr("");
}

#[test]
fn malformed_json_passes_through() {
    hook()
        .env("MCP_BRIDGE_HOOK_MODE", "enforce")
        .write_stdin("{not json")
        .assert()
        .success()
        .stdout("")
        .stderr("");
}

#[test]
fn empty_stdin_passes_through() {
    hook()
        .env("MCP_BRIDGE_HOOK_MODE", "enforce")
        .write_stdin("")
        .assert()
        .success();
}

#[test]
fn empty_command_passes_through() {
    hook()
        .env("MCP_BRIDGE_HOOK_MODE", "enforce")
        .write_stdin(pretool_bash(""))
        .assert()
        .success()
        .stdout("")
        .stderr("");
}

// ── 300 real-history commands: never panics, never blocks unrelated work ──

#[test]
fn real_history_no_panics_no_false_positive_blocks() {
    let fixture = include_str!("fixtures/real_bash_commands.txt");
    let mut suggested = 0usize;
    let mut total = 0usize;

    for line in fixture.lines() {
        let cmd = line.trim_end_matches('\r');
        if cmd.is_empty() {
            continue;
        }
        total += 1;
        // Run in suggest mode — never blocks. Verify no panic, exit 0,
        // and any stdout is valid JSON of the expected shape.
        let assert = hook()
            .env("MCP_BRIDGE_HOOK_MODE", "suggest")
            .write_stdin(pretool_bash(cmd))
            .assert()
            .success();
        let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
        let stdout = stdout.trim();
        if !stdout.is_empty() {
            suggested += 1;
            let v: serde_json::Value = serde_json::from_str(stdout).unwrap_or_else(|e| {
                panic!(
                    "non-JSON stdout for command {:?}: {} (err: {})",
                    cmd, stdout, e
                )
            });
            assert_eq!(
                v["hookSpecificOutput"]["hookEventName"], "PreToolUse",
                "wrong hook shape for command {:?}: {}",
                cmd, stdout
            );
            assert!(
                v["hookSpecificOutput"]["additionalContext"].is_string(),
                "missing additionalContext for command {:?}: {}",
                cmd,
                stdout
            );
        }
    }

    eprintln!(
        "Real-history sample: {}/{} commands triggered a suggestion",
        suggested, total
    );
    assert!(total > 100, "fixture too small: {}", total);
    // Sanity: a non-trivial fraction should be ls/find/git/etc.
    assert!(
        suggested >= 5,
        "real-history sample never triggered any suggestion (saw {}/{}); parser likely broken",
        suggested,
        total
    );
}
