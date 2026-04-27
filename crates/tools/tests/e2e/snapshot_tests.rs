//! Snapshot tests for tool output stability across versions.
//!
//! Run `cargo insta review` to update snapshots after intentional schema changes.
//! These guard against accidentally adding/removing fields without an explicit review.

use crate::harness::Server;
use serde_json::{json, Value};
use std::fs;
use tempfile::TempDir;

/// Strip volatile fields (paths, timestamps, sizes that depend on the host) so the
/// snapshot only captures schema shape. We replace volatile values with placeholders.
fn redact_volatile(mut v: Value) -> Value {
    redact_walk(&mut v);
    v
}

fn redact_walk(v: &mut Value) {
    match v {
        Value::Object(map) => {
            for (k, val) in map.iter_mut() {
                let key = k.as_str();
                if key == "path"
                    || key == "file"
                    || key == "modified"
                    || key == "creation_timestamp"
                    || key == "resource_version"
                    || key == "uid"
                    || key == "size"
                    || key == "mem_rss_kb"
                    || key == "elapsed_seconds"
                    || key == "cpu_percent"
                    || key == "duration_ms"
                    || key == "total_duration_ms"
                    || key == "snapshot_oid"
                    || key == "last_hash"
                    || key == "hash"
                    || key == "date"
                    || key == "permissions"
                    || key == "pid"
                    || key == "ppid"
                    || key == "user"
                    || key == "root"
                    || key == "created"
                    || key == "id"
                    || key == "started_at"
                    || key == "finished_at"
                {
                    *val = Value::String(format!("<{key}>"));
                } else {
                    redact_walk(val);
                }
            }
        }
        Value::Array(arr) => {
            for item in arr.iter_mut() {
                redact_walk(item);
            }
        }
        _ => {}
    }
}

#[test]
fn snapshot_ls_output_shape() {
    let d = TempDir::new().unwrap();
    fs::write(d.path().join("a.txt"), "x").unwrap();
    fs::write(d.path().join("b.rs"), "fn main() {}\n").unwrap();
    fs::create_dir(d.path().join("subdir")).unwrap();

    let mut s = Server::spawn(&[]);
    let r = s.call("ls", json!({"path": d.path(), "long": true}));
    let redacted = redact_volatile(r.data.clone());
    insta::assert_json_snapshot!("ls_output", redacted);
}

#[test]
fn snapshot_wc_inline_shape() {
    let mut s = Server::spawn(&[]);
    let r = s.call("wc", json!({"input": "hello world\nfoo bar"}));
    insta::assert_json_snapshot!("wc_inline", r.data);
}

#[test]
fn snapshot_wc_multipath_shape() {
    let d = TempDir::new().unwrap();
    fs::write(d.path().join("a.txt"), "a\n").unwrap();
    fs::write(d.path().join("b.txt"), "b\nc\n").unwrap();

    let mut s = Server::spawn(&[]);
    let r = s.call(
        "wc",
        json!({"paths": [d.path().join("a.txt"), d.path().join("b.txt")]}),
    );
    let redacted = redact_volatile(r.data.clone());
    insta::assert_json_snapshot!("wc_multipath", redacted);
}

#[test]
fn snapshot_diff_simple_shape() {
    let input = "--- a/f.rs\n+++ b/f.rs\n@@ -1,3 +1,4 @@\n line\n+added\n line\n line\n";
    let mut s = Server::spawn(&[]);
    let r = s.call("diff", json!({"input": input}));
    insta::assert_json_snapshot!("diff_simple", r.data);
}

#[test]
fn snapshot_find_basic_shape() {
    let d = TempDir::new().unwrap();
    fs::write(d.path().join("a.rs"), "x").unwrap();
    fs::write(d.path().join("b.md"), "x").unwrap();

    let mut s = Server::spawn(&[]);
    let r = s.call("find", json!({"path": d.path(), "type": "file"}));
    let redacted = redact_volatile(r.data.clone());
    insta::assert_json_snapshot!("find_basic", redacted);
}

#[test]
fn snapshot_sqlite_query_shape() {
    use rusqlite::Connection;
    let d = TempDir::new().unwrap();
    let p = d.path().join("t.db");
    let conn = Connection::open(&p).unwrap();
    conn.execute_batch(
        "CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT);
         INSERT INTO t VALUES (1, 'a'); INSERT INTO t VALUES (2, 'b');",
    )
    .unwrap();
    drop(conn);

    let mut s = Server::spawn(&[]);
    let r = s.call(
        "sqlite_query",
        json!({"db_path": p, "sql": "SELECT id, name FROM t ORDER BY id"}),
    );
    insta::assert_json_snapshot!("sqlite_query", r.data);
}

#[test]
fn snapshot_sqlite_tables_shape() {
    use rusqlite::Connection;
    let d = TempDir::new().unwrap();
    let p = d.path().join("t.db");
    let conn = Connection::open(&p).unwrap();
    conn.execute_batch("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL);")
        .unwrap();
    drop(conn);

    let mut s = Server::spawn(&[]);
    let r = s.call("sqlite_tables", json!({"db_path": p}));
    insta::assert_json_snapshot!("sqlite_tables", r.data);
}

#[test]
fn snapshot_batch_result_shape() {
    let mut s = Server::spawn(&[]);
    let r = s.call(
        "batch",
        json!({
            "operations": [
                {"tool": "wc", "params": {"input": "x"}},
                {"tool": "wc", "params": {"input": "y z"}}
            ]
        }),
    );
    let redacted = redact_volatile(r.data.clone());
    insta::assert_json_snapshot!("batch_result", redacted);
}

#[test]
fn snapshot_pipe_result_shape() {
    let d = TempDir::new().unwrap();
    fs::write(d.path().join("a.rs"), "x").unwrap();
    fs::write(d.path().join("b.md"), "x").unwrap();

    let mut s = Server::spawn(&[]);
    let r = s.call(
        "pipe",
        json!({
            "source": {"tool": "ls", "params": {"path": d.path()}},
            "filters": [{"field": "name", "pattern": ".rs", "mode": "contains"}]
        }),
    );
    let redacted = redact_volatile(r.data.clone());
    insta::assert_json_snapshot!("pipe_result", redacted);
}

#[test]
fn snapshot_diff_format_error_shape() {
    let mut s = Server::spawn(&[]);
    let r = s.call(
        "diff",
        json!({"input": "Binary files a/x and b/x differ\n"}),
    );
    // Errors are also stable contracts.
    let parsed: Value =
        serde_json::from_str(&r.content_text).unwrap_or(Value::String(r.content_text.clone()));
    insta::assert_json_snapshot!("diff_format_error", parsed);
}
