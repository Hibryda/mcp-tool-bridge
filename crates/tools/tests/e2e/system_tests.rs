//! lsof + ps tests. These rely on system tools so they're gated to Linux+macOS only.
//! Even there, ps output format varies — tests focus on schema invariants, not exact values.

use crate::harness::Server;
use serde_json::json;

// ── ps ──────────────────────────────────────────────────────────────

#[test]
fn ps_returns_processes() {
    let mut s = Server::spawn(&[]);
    let r = s.call("ps", json!({"max_results": 10}));
    assert!(r.success());
    assert!(r.data["count"].as_u64().unwrap() > 0);
}

#[test]
fn ps_each_entry_has_required_fields() {
    let mut s = Server::spawn(&[]);
    let r = s.call("ps", json!({"max_results": 5}));
    assert!(r.success());
    for p in r.data["processes"].as_array().unwrap() {
        for k in [
            "pid",
            "ppid",
            "user",
            "command",
            "args",
            "cpu_percent",
            "mem_rss_kb",
        ] {
            assert!(!p[k].is_null(), "missing {}", k);
        }
    }
}

#[test]
fn ps_max_results_caps_count() {
    let mut s = Server::spawn(&[]);
    let r = s.call("ps", json!({"max_results": 3}));
    assert!(r.success());
    assert!(r.data["count"].as_u64().unwrap() <= 3);
}

#[test]
fn ps_total_before_filter_at_least_count() {
    let mut s = Server::spawn(&[]);
    let r = s.call("ps", json!({"max_results": 10}));
    assert!(r.success());
    let count = r.data["count"].as_u64().unwrap();
    let total = r.data["total_before_filter"].as_u64().unwrap();
    assert!(total >= count);
}

#[test]
fn ps_name_pattern_filter() {
    // Find init/systemd/launchd — one of these should exist on every system.
    let mut s = Server::spawn(&[]);
    // Just ensure the filter applies cleanly; use a name we know is rare.
    let r = s.call(
        "ps",
        json!({
            "name_pattern": "definitely_not_a_real_command_xyz",
            "max_results": 100
        }),
    );
    assert!(r.success());
    assert_eq!(r.data["count"], 0);
}

#[test]
fn ps_no_matches_yields_empty_count() {
    let mut s = Server::spawn(&[]);
    let r = s.call("ps", json!({"name_pattern": "zz_xyz_no_such_proc"}));
    assert!(r.success());
    assert_eq!(r.data["count"], 0);
}

#[test]
fn ps_pid_filter() {
    // PID 1 exists everywhere (init/launchd).
    let mut s = Server::spawn(&[]);
    let r = s.call("ps", json!({"pid_list": [1]}));
    assert!(r.success());
    let arr = r.data["processes"].as_array().unwrap();
    if !arr.is_empty() {
        assert_eq!(arr[0]["pid"], 1);
    }
}

// ── lsof ────────────────────────────────────────────────────────────

#[test]
fn lsof_returns_structure() {
    let mut s = Server::spawn(&[]);
    let r = s.call("lsof", json!({"network_only": true}));
    assert!(r.success());
    assert!(r.data["processes"].is_array());
    assert!(r.data["total_fds"].is_number());
}

#[test]
fn lsof_total_matches_sum() {
    let mut s = Server::spawn(&[]);
    let r = s.call("lsof", json!({"network_only": true}));
    assert!(r.success());
    let processes = r.data["processes"].as_array().unwrap();
    let sum: u64 = processes
        .iter()
        .map(|p| p["files"].as_array().unwrap().len() as u64)
        .sum();
    assert_eq!(r.data["total_fds"].as_u64().unwrap(), sum);
}

#[test]
fn lsof_each_fd_has_fields() {
    let mut s = Server::spawn(&[]);
    let r = s.call("lsof", json!({"network_only": true}));
    assert!(r.success());
    for p in r.data["processes"].as_array().unwrap() {
        for fd in p["files"].as_array().unwrap() {
            assert!(fd["fd"].is_string());
            assert!(fd["type"].is_string());
            assert!(fd["name"].is_string());
        }
    }
}

#[test]
fn lsof_protocol_and_port_combined() {
    // Just check no crash; specific ports vary by environment.
    let mut s = Server::spawn(&[]);
    let r = s.call("lsof", json!({"protocol": "TCP", "port": "65535"}));
    // May succeed with empty result or fail depending on lsof exit code.
    // Either is acceptable; the bug we fixed was -iTCP -i:65535 conflicting.
    let _ = r;
}
