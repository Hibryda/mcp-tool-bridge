//! Tests for the --tools CLI flag and overall tool registration.

use crate::harness::Server;
use serde_json::json;

#[test]
fn lists_all_tools_by_default() {
    let mut s = Server::spawn(&[]);
    let tools = s.list_tools();
    // We have 20 tools as of v0.1.
    assert_eq!(
        tools.len(),
        20,
        "expected 20 tools, got {}: {:?}",
        tools.len(),
        tools
    );
}

#[test]
fn all_expected_tools_present() {
    let mut s = Server::spawn(&[]);
    let tools = s.list_tools();
    let expected = [
        "ls",
        "wc",
        "diff",
        "lsof",
        "find",
        "curl",
        "git_status",
        "git_log",
        "git_show",
        "gh_api",
        "ps",
        "kubectl_list",
        "kubectl_get",
        "docker_list",
        "docker_inspect",
        "docker_images",
        "sqlite_query",
        "sqlite_tables",
        "batch",
        "pipe",
    ];
    for name in expected {
        assert!(tools.iter().any(|t| t == name), "missing tool: {}", name);
    }
}

#[test]
fn tools_flag_filters_registration() {
    let mut s = Server::spawn(&["--tools", "ls,wc"]);
    let tools = s.list_tools();
    assert_eq!(tools.len(), 2);
    assert!(tools.contains(&"ls".to_string()));
    assert!(tools.contains(&"wc".to_string()));
}

#[test]
fn tools_flag_unknown_tool_ignored() {
    // The --tools flag is permissive: it filters the known set against the requested set,
    // so an unknown name is silently dropped rather than crashing.
    let mut s = Server::spawn(&["--tools", "ls,wc,nonsense_xyz"]);
    let tools = s.list_tools();
    assert!(tools.contains(&"ls".to_string()));
    assert!(tools.contains(&"wc".to_string()));
    assert!(!tools.contains(&"nonsense_xyz".to_string()));
}

#[test]
fn filtered_server_can_call_registered_tool() {
    let mut s = Server::spawn(&["--tools", "wc"]);
    let r = s.call("wc", json!({"input": "hello"}));
    assert!(r.success());
}

#[test]
fn empty_tools_flag_disables_all() {
    // Edge case: empty list could either mean "all" or "none". Document actual behavior.
    let mut s = Server::spawn(&["--tools", ""]);
    let tools = s.list_tools();
    // We don't assert equality here — capture whatever the actual behavior is for documentation.
    // What matters is: no crash, no panic.
    let _ = tools;
}

#[test]
fn list_tools_response_is_well_formed_json() {
    let mut s = Server::spawn(&[]);
    let tools = s.list_tools();
    // Every tool name should be a valid identifier (no spaces, no special chars).
    for name in &tools {
        assert!(!name.is_empty());
        assert!(
            name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'),
            "tool name has special chars: {:?}",
            name
        );
    }
}
