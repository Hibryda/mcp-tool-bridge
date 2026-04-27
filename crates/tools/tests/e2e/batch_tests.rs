use crate::harness::Server;
use serde_json::json;
use tempfile::TempDir;

#[test]
fn batch_runs_multiple_ops() {
    let mut s = Server::spawn(&[]);
    let r = s.call(
        "batch",
        json!({
            "operations": [
                {"tool": "wc", "params": {"input": "hello world"}},
                {"tool": "wc", "params": {"input": "foo bar baz"}}
            ]
        }),
    );
    assert!(r.success());
    let results = r.data["results"].as_array().unwrap();
    assert_eq!(results.len(), 2);
    assert!(results.iter().all(|r| r["success"].as_bool().unwrap()));
}

#[test]
fn batch_preserves_order() {
    let mut s = Server::spawn(&[]);
    let r = s.call(
        "batch",
        json!({
            "operations": [
                {"tool": "wc", "params": {"input": "first"}},
                {"tool": "wc", "params": {"input": "second"}},
                {"tool": "wc", "params": {"input": "third"}}
            ]
        }),
    );
    assert!(r.success());
    let results = r.data["results"].as_array().unwrap();
    assert_eq!(results[0]["params"]["input"], "first");
    assert_eq!(results[1]["params"]["input"], "second");
    assert_eq!(results[2]["params"]["input"], "third");
}

#[test]
fn batch_error_isolation() {
    let mut s = Server::spawn(&[]);
    let r = s.call(
        "batch",
        json!({
            "operations": [
                {"tool": "wc", "params": {"input": "ok"}},
                {"tool": "ls", "params": {"path": "/nonexistent/zzz"}},
                {"tool": "wc", "params": {"input": "ok2"}}
            ]
        }),
    );
    assert!(r.success());
    let results = r.data["results"].as_array().unwrap();
    assert!(results[0]["success"].as_bool().unwrap());
    assert!(!results[1]["success"].as_bool().unwrap());
    assert!(results[2]["success"].as_bool().unwrap());
}

#[test]
fn batch_unregistered_tool() {
    let mut s = Server::spawn(&[]);
    let r = s.call(
        "batch",
        json!({
            "operations": [
                {"tool": "nonexistent_tool_xyz", "params": {}}
            ]
        }),
    );
    assert!(r.success());
    let results = r.data["results"].as_array().unwrap();
    assert!(!results[0]["success"].as_bool().unwrap());
    let err = results[0]["error"].as_str().unwrap();
    assert!(err.contains("not registered") || err.contains("unknown"));
}

#[test]
fn batch_empty_operations_allowed() {
    let mut s = Server::spawn(&[]);
    let r = s.call("batch", json!({"operations": []}));
    assert!(r.success());
    assert_eq!(r.data["results"].as_array().unwrap().len(), 0);
}

#[test]
fn batch_durations_recorded() {
    let mut s = Server::spawn(&[]);
    let r = s.call(
        "batch",
        json!({
            "operations": [{"tool": "wc", "params": {"input": "x"}}]
        }),
    );
    assert!(r.success());
    assert!(r.data["total_duration_ms"].is_number());
    assert!(r.data["results"][0]["duration_ms"].is_number());
}

#[test]
fn batch_params_echoed_in_results() {
    let mut s = Server::spawn(&[]);
    let r = s.call(
        "batch",
        json!({
            "operations": [{"tool": "wc", "params": {"input": "echo-me"}}]
        }),
    );
    assert!(r.success());
    assert_eq!(r.data["results"][0]["params"]["input"], "echo-me");
}

#[test]
fn batch_mixed_tools() {
    let d = TempDir::new().unwrap();
    std::fs::write(d.path().join("a.txt"), "x").unwrap();
    let mut s = Server::spawn(&[]);
    let r = s.call(
        "batch",
        json!({
            "operations": [
                {"tool": "ls", "params": {"path": d.path()}},
                {"tool": "wc", "params": {"input": "test"}},
                {"tool": "find", "params": {"path": d.path(), "type": "file"}}
            ]
        }),
    );
    assert!(r.success());
    let results = r.data["results"].as_array().unwrap();
    assert_eq!(results.len(), 3);
    assert!(results.iter().all(|r| r["success"].as_bool().unwrap()));
}

#[test]
fn batch_large_op_count() {
    let ops: Vec<_> = (0..50)
        .map(|i| {
            json!({
                "tool": "wc", "params": {"input": format!("item-{i}")}
            })
        })
        .collect();
    let mut s = Server::spawn(&[]);
    let r = s.call("batch", json!({"operations": ops}));
    assert!(r.success());
    assert_eq!(r.data["results"].as_array().unwrap().len(), 50);
}
