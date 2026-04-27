use crate::harness::Server;
use serde_json::json;
use std::fs;
use tempfile::TempDir;

fn fixture() -> TempDir {
    let d = TempDir::new().unwrap();
    let p = d.path();
    fs::create_dir_all(p.join("subdir")).unwrap();
    fs::write(p.join("a.rs"), "x").unwrap();
    fs::write(p.join("b.rs"), "x").unwrap();
    fs::write(p.join("c.md"), "x").unwrap();
    fs::write(p.join("d.txt"), "x").unwrap();
    d
}

#[test]
fn pipe_filters_by_type() {
    let d = fixture();
    let mut s = Server::spawn(&[]);
    let r = s.call(
        "pipe",
        json!({
            "source": {"tool": "ls", "params": {"path": d.path()}},
            "filters": [{"field": "type", "pattern": "file", "mode": "equals"}]
        }),
    );
    assert!(r.success());
    assert!(r.data["items"]
        .as_array()
        .unwrap()
        .iter()
        .all(|i| i["type"] == "file"));
}

#[test]
fn pipe_filters_by_name_contains() {
    let d = fixture();
    let mut s = Server::spawn(&[]);
    let r = s.call(
        "pipe",
        json!({
            "source": {"tool": "ls", "params": {"path": d.path()}},
            "filters": [{"field": "name", "pattern": ".rs", "mode": "contains"}]
        }),
    );
    assert!(r.success());
    let items = r.data["items"].as_array().unwrap();
    assert!(!items.is_empty());
    assert!(items
        .iter()
        .all(|i| i["name"].as_str().unwrap().contains(".rs")));
}

#[test]
fn pipe_and_semantics_multiple_filters() {
    let d = fixture();
    let mut s = Server::spawn(&[]);
    let r = s.call(
        "pipe",
        json!({
            "source": {"tool": "ls", "params": {"path": d.path()}},
            "filters": [
                {"field": "type", "pattern": "file", "mode": "equals"},
                {"field": "name", "pattern": ".rs", "mode": "contains"}
            ]
        }),
    );
    assert!(r.success());
    let items = r.data["items"].as_array().unwrap();
    assert!(items
        .iter()
        .all(|i| { i["type"] == "file" && i["name"].as_str().unwrap().contains(".rs") }));
}

#[test]
fn pipe_starts_with_mode() {
    let d = fixture();
    let mut s = Server::spawn(&[]);
    let r = s.call(
        "pipe",
        json!({
            "source": {"tool": "ls", "params": {"path": d.path()}},
            "filters": [{"field": "name", "pattern": "a", "mode": "starts_with"}]
        }),
    );
    assert!(r.success());
    assert!(r.data["items"]
        .as_array()
        .unwrap()
        .iter()
        .all(|i| i["name"].as_str().unwrap().starts_with("a")));
}

#[test]
fn pipe_limit_caps_results() {
    let d = fixture();
    let mut s = Server::spawn(&[]);
    let r = s.call(
        "pipe",
        json!({
            "source": {"tool": "ls", "params": {"path": d.path()}},
            "filters": [],
            "limit": 2
        }),
    );
    assert!(r.success());
    assert_eq!(r.data["items"].as_array().unwrap().len(), 2);
}

#[test]
fn pipe_records_before_after_counts() {
    let d = fixture();
    let mut s = Server::spawn(&[]);
    let r = s.call(
        "pipe",
        json!({
            "source": {"tool": "ls", "params": {"path": d.path()}},
            "filters": [{"field": "name", "pattern": ".rs", "mode": "contains"}]
        }),
    );
    assert!(r.success());
    let before = r.data["total_before_filter"].as_u64().unwrap();
    let after = r.data["total_after_filter"].as_u64().unwrap();
    assert!(after <= before);
}

#[test]
fn pipe_non_whitelisted_source_errors() {
    let mut s = Server::spawn(&[]);
    let r = s.call(
        "pipe",
        json!({
            "source": {"tool": "diff", "params": {"input": "x"}},
            "filters": []
        }),
    );
    assert!(!r.success());
}

#[test]
fn pipe_find_source_supported() {
    let d = fixture();
    let mut s = Server::spawn(&[]);
    let r = s.call(
        "pipe",
        json!({
            "source": {"tool": "find", "params": {"path": d.path(), "type": "file"}},
            "filters": [{"field": "name", "pattern": ".rs", "mode": "contains"}]
        }),
    );
    assert!(r.success());
}

#[test]
fn pipe_no_match_returns_empty() {
    let d = fixture();
    let mut s = Server::spawn(&[]);
    let r = s.call(
        "pipe",
        json!({
            "source": {"tool": "ls", "params": {"path": d.path()}},
            "filters": [{"field": "name", "pattern": "no-such-name-xyz", "mode": "equals"}]
        }),
    );
    assert!(r.success());
    assert_eq!(r.data["items"].as_array().unwrap().len(), 0);
    assert_eq!(r.data["total_after_filter"], 0);
}

#[test]
fn pipe_propagates_source_errors() {
    let mut s = Server::spawn(&[]);
    let r = s.call(
        "pipe",
        json!({
            "source": {"tool": "ls", "params": {"path": "/nonexistent/zzz"}},
            "filters": []
        }),
    );
    assert!(!r.success());
}

#[test]
fn pipe_source_tool_name_in_response() {
    let d = fixture();
    let mut s = Server::spawn(&[]);
    let r = s.call(
        "pipe",
        json!({
            "source": {"tool": "ls", "params": {"path": d.path()}},
            "filters": []
        }),
    );
    assert!(r.success());
    assert_eq!(r.data["source_tool"], "ls");
}
