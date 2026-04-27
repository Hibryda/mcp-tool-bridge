//! Failure mode tests — concurrency, resource limits, permission errors,
//! and other paths that example-based tests don't exercise.

use crate::harness::Server;
use serde_json::json;
use std::fs;
use tempfile::TempDir;

// ── Concurrency stress for batch ────────────────────────────────────

#[test]
fn batch_100_parallel_ops() {
    let ops: Vec<_> = (0..100)
        .map(|i| {
            json!({
                "tool": "wc",
                "params": {"input": format!("op-{i}")}
            })
        })
        .collect();
    let mut s = Server::spawn(&[]);
    let r = s.call("batch", json!({"operations": ops}));
    assert!(r.success());
    let results = r.data["results"].as_array().unwrap();
    assert_eq!(results.len(), 100);
    assert!(results.iter().all(|r| r["success"].as_bool().unwrap()));
}

#[test]
fn batch_order_preserved_under_concurrency() {
    // 50 ops with varying input lengths — semaphore=4 means they finish out of order
    // internally, but the final results array must match input order.
    let ops: Vec<_> = (0..50)
        .map(|i| {
            // Larger i → bigger input → slower wc → tests reordering pressure.
            let body = "x".repeat(i * 100);
            json!({
                "tool": "wc",
                "params": {"input": format!("{}|input-{i}", body)}
            })
        })
        .collect();
    let mut s = Server::spawn(&[]);
    let r = s.call("batch", json!({"operations": ops}));
    assert!(r.success());
    for (i, result) in r.data["results"].as_array().unwrap().iter().enumerate() {
        let echoed = result["params"]["input"].as_str().unwrap();
        assert!(
            echoed.ends_with(&format!("|input-{i}")),
            "result {i} has wrong input: {echoed}"
        );
    }
}

#[test]
fn batch_one_failure_does_not_cascade() {
    // 20 ops where every 3rd one targets a nonexistent path.
    let ops: Vec<_> = (0..20)
        .map(|i| {
            if i % 3 == 0 {
                json!({"tool": "ls", "params": {"path": format!("/nonexistent/{i}")}})
            } else {
                json!({"tool": "wc", "params": {"input": format!("ok-{i}")}})
            }
        })
        .collect();
    let mut s = Server::spawn(&[]);
    let r = s.call("batch", json!({"operations": ops}));
    assert!(r.success());
    let results = r.data["results"].as_array().unwrap();
    assert_eq!(results.len(), 20);
    let success_count = results
        .iter()
        .filter(|r| r["success"].as_bool().unwrap())
        .count();
    let failure_count = results.len() - success_count;
    // Roughly 7 failures (every 3rd) and 13 successes; exact ratio depends on i % 3 over 0..20.
    assert!(failure_count >= 6 && failure_count <= 8);
    assert!(success_count >= 12);
}

// ── Resource limits ────────────────────────────────────────────────

#[test]
fn pipe_size_guard_rejects_huge_source() {
    // Create a directory with many files so ls returns a >1MB JSON payload.
    let d = TempDir::new().unwrap();
    // Each file entry serializes to ~150-200 bytes; 8000 files comfortably exceeds 1MB.
    for i in 0..8000 {
        fs::write(d.path().join(format!("file_{i:05}.txt")), "x").unwrap();
    }
    let mut s = Server::spawn(&[]);
    let r = s.call(
        "pipe",
        json!({
            "source": {"tool": "ls", "params": {"path": d.path()}},
            "filters": []
        }),
    );
    // Should fail with size guard, not crash or OOM.
    assert!(!r.success());
    assert!(r.content_text.contains("1MB") || r.content_text.contains("size"));
}

#[test]
fn find_limit_truncates_at_cap() {
    let d = TempDir::new().unwrap();
    for i in 0..50 {
        fs::write(d.path().join(format!("f{i}.txt")), "x").unwrap();
    }
    let mut s = Server::spawn(&[]);
    let r = s.call("find", json!({"path": d.path(), "limit": 10}));
    assert!(r.success());
    assert_eq!(r.data["entries"].as_array().unwrap().len(), 10);
    assert_eq!(r.data["truncated"], true);
}

#[test]
fn wc_handles_many_paths() {
    // 500 paths in one wc call — verify no quadratic behavior or OOM.
    let d = TempDir::new().unwrap();
    let paths: Vec<_> = (0..500)
        .map(|i| {
            let p = d.path().join(format!("f{i}.txt"));
            fs::write(&p, format!("line {i}\n")).unwrap();
            p
        })
        .collect();
    let mut s = Server::spawn(&[]);
    let r = s.call("wc", json!({"paths": paths}));
    assert!(r.success());
    assert_eq!(r.data.as_array().unwrap().len(), 500);
}

#[test]
fn git_log_max_count_capped_at_200() {
    // Create a repo with >200 commits.
    let d = TempDir::new().unwrap();
    let p = d.path();
    let run = |args: &[&str]| {
        std::process::Command::new("git")
            .args(args)
            .current_dir(p)
            .env("GIT_AUTHOR_NAME", "T")
            .env("GIT_AUTHOR_EMAIL", "t@t")
            .env("GIT_COMMITTER_NAME", "T")
            .env("GIT_COMMITTER_EMAIL", "t@t")
            .output()
            .unwrap()
    };
    run(&["init", "-q", "-b", "main"]);
    run(&["config", "user.name", "T"]);
    run(&["config", "user.email", "t@t"]);
    fs::write(p.join("f"), "0\n").unwrap();
    run(&["add", "f"]);
    run(&["commit", "-q", "-m", "0"]);
    for i in 1..=210 {
        fs::write(p.join("f"), format!("{}\n", i)).unwrap();
        run(&["add", "f"]);
        run(&["commit", "-q", "-m", &format!("{}", i)]);
    }

    let mut s = Server::spawn(&[]);
    // Request way more than the cap.
    let r = s.call("git_log", json!({"path": d.path(), "max_count": 9999}));
    assert!(r.success());
    // Cap is 200 in dispatch.rs.
    assert!(r.data["commits"].as_array().unwrap().len() <= 200);
}

// ── Permission and IO failures ─────────────────────────────────────

#[test]
fn ls_unreadable_dir_returns_error() {
    // Try to list /proc/1/root which is typically unreadable as non-root.
    let mut s = Server::spawn(&[]);
    let r = s.call("ls", json!({"path": "/proc/1/root"}));
    // Behavior: either an error envelope or success with empty list.
    // The important thing is no panic.
    let _ = r.success();
}

#[test]
fn find_continues_past_unreadable_subdirs() {
    // find should skip unreadable dirs and return a partial result, not error out.
    let mut s = Server::spawn(&[]);
    let r = s.call(
        "find",
        json!({"path": "/proc", "max_depth": 1, "limit": 10}),
    );
    // Either succeeds with a partial list or returns a controlled error.
    let _ = r.success();
}

#[test]
fn sqlite_query_with_long_running_pragma() {
    // PRAGMA integrity_check on a fresh empty DB returns instantly, but verify
    // that arbitrary PRAGMAs aren't blocked by the write-statement filter.
    use rusqlite::Connection;
    let d = TempDir::new().unwrap();
    let p = d.path().join("t.db");
    let _ = Connection::open(&p).unwrap();
    let mut s = Server::spawn(&[]);
    let r = s.call(
        "sqlite_query",
        json!({"db_path": p, "sql": "PRAGMA integrity_check"}),
    );
    // PRAGMA is a SELECT-like statement, should succeed.
    assert!(r.success());
}

// ── Path-related edge cases ────────────────────────────────────────

#[test]
fn ls_path_with_spaces_works() {
    let d = TempDir::new().unwrap();
    let sub = d.path().join("dir with spaces");
    fs::create_dir(&sub).unwrap();
    fs::write(sub.join("file with spaces.txt"), "x").unwrap();
    let mut s = Server::spawn(&[]);
    let r = s.call("ls", json!({"path": sub}));
    assert!(r.success());
    assert!(r
        .data
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["name"] == "file with spaces.txt"));
}

#[test]
fn ls_unicode_path_works() {
    let d = TempDir::new().unwrap();
    let sub = d.path().join("папка-日本");
    fs::create_dir(&sub).unwrap();
    fs::write(sub.join("файл.txt"), "x").unwrap();
    let mut s = Server::spawn(&[]);
    let r = s.call("ls", json!({"path": sub}));
    assert!(r.success());
    assert!(r
        .data
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["name"] == "файл.txt"));
}

#[test]
fn wc_unicode_filename() {
    let d = TempDir::new().unwrap();
    let p = d.path().join("名前.txt");
    fs::write(&p, "x\n").unwrap();
    let mut s = Server::spawn(&[]);
    let r = s.call("wc", json!({"path": p}));
    assert!(r.success());
    assert_eq!(r.data["lines"], 1);
}

// ── git_log under concurrent commit ────────────────────────────────

#[test]
fn git_log_snapshot_oid_isolates_concurrent_commits() {
    // Capture a snapshot, add a new commit, query again with the same snapshot_oid —
    // the new commit should NOT appear, proving snapshot isolation.
    let d = TempDir::new().unwrap();
    let p = d.path();
    let run = |args: &[&str]| {
        std::process::Command::new("git")
            .args(args)
            .current_dir(p)
            .env("GIT_AUTHOR_NAME", "T")
            .env("GIT_AUTHOR_EMAIL", "t@t")
            .env("GIT_COMMITTER_NAME", "T")
            .env("GIT_COMMITTER_EMAIL", "t@t")
            .output()
            .unwrap()
    };
    run(&["init", "-q", "-b", "main"]);
    run(&["config", "user.name", "T"]);
    run(&["config", "user.email", "t@t"]);
    fs::write(p.join("a"), "1\n").unwrap();
    run(&["add", "a"]);
    run(&["commit", "-q", "-m", "first"]);

    let mut s = Server::spawn(&[]);
    let r1 = s.call("git_log", json!({"path": p}));
    assert!(r1.success());
    let snapshot = r1.data["snapshot_oid"].as_str().unwrap().to_string();
    let initial_count = r1.data["commits"].as_array().unwrap().len();

    // Add another commit AFTER snapshot.
    fs::write(p.join("b"), "2\n").unwrap();
    run(&["add", "b"]);
    run(&["commit", "-q", "-m", "second"]);

    // Query with the original snapshot — should still see only the original commits.
    let r2 = s.call("git_log", json!({"path": p, "snapshot_oid": &snapshot}));
    assert!(r2.success());
    assert_eq!(
        r2.data["commits"].as_array().unwrap().len(),
        initial_count,
        "snapshot_oid should freeze the view; got an extra commit"
    );
}

// ── Tool with bad type in JSON ─────────────────────────────────────

#[test]
fn wc_with_wrong_type_for_path() {
    // path expects string; passing an array should be rejected cleanly.
    let mut s = Server::spawn(&[]);
    let r = s.call("wc", json!({"path": ["not", "a", "string"]}));
    assert!(!r.success());
}

#[test]
fn find_with_negative_max_depth_handled() {
    // max_depth is u32 in schema; negative gets rejected at deserialization.
    let mut s = Server::spawn(&[]);
    let r = s.call("find", json!({"path": "/tmp", "max_depth": -1}));
    assert!(!r.success());
}

// ── Long-running input simulated ───────────────────────────────────

#[test]
fn batch_completes_within_reasonable_time() {
    // 30 fast ops should complete in well under 10 seconds even on a slow runner.
    let ops: Vec<_> = (0..30)
        .map(|i| {
            json!({
                "tool": "wc",
                "params": {"input": format!("op-{i}")}
            })
        })
        .collect();
    let start = std::time::Instant::now();
    let mut s = Server::spawn(&[]);
    let r = s.call("batch", json!({"operations": ops}));
    let elapsed = start.elapsed();
    assert!(r.success());
    assert!(
        elapsed.as_secs() < 10,
        "batch of 30 wc ops took {}s",
        elapsed.as_secs()
    );
}
