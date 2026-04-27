//! Property-based tests for parsers — generate random inputs and verify invariants.
//!
//! These tests catch parser edge cases that example-based tests miss: panics, off-by-one
//! errors, sentinel collisions, and inconsistent error envelopes.

use crate::harness::Server;
use proptest::prelude::*;
use serde_json::json;

// Reduce iteration count — each test spawns a server process, so we trade some
// search depth for keeping CI total runtime under a minute.
const PROPTEST_CASES: u32 = 16;

// ── diff parser ─────────────────────────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig { cases: PROPTEST_CASES, .. ProptestConfig::default() })]
    /// Random unified diff with valid hunks should never panic and should produce
    /// add/del counts matching the input.
    #[test]
    fn diff_count_invariant(adds in 0u32..6, dels in 0u32..6) {
        // Skip the no-op case; format detection requires --- +++ @@ markers and content.
        prop_assume!(adds + dels > 0);
        let mut input = String::from("--- a/f\n+++ b/f\n");
        let old_count = dels + 1;
        let new_count = adds + 1;
        input.push_str(&format!("@@ -1,{} +1,{} @@\n ctx\n", old_count, new_count));
        for i in 0..dels { input.push_str(&format!("-del{}\n", i)); }
        for i in 0..adds { input.push_str(&format!("+add{}\n", i)); }

        let mut s = Server::spawn(&[]);
        let r = s.call("diff", json!({"input": input}));
        prop_assert!(r.success(), "expected success, got: {}", r.content_text);
        prop_assert_eq!(r.data["total_additions"].as_u64().unwrap(), adds as u64);
        prop_assert_eq!(r.data["total_deletions"].as_u64().unwrap(), dels as u64);
    }

    /// Random multi-file diffs preserve file count and total counts.
    #[test]
    fn diff_multi_file_invariant(n in 1usize..8) {
        let mut input = String::new();
        for i in 0..n {
            input.push_str(&format!(
                "diff --git a/f{i}.rs b/f{i}.rs\n--- a/f{i}.rs\n+++ b/f{i}.rs\n@@ -1,1 +1,2 @@\n existing\n+added\n"
            ));
        }
        let mut s = Server::spawn(&[]);
        let r = s.call("diff", json!({"input": input}));
        prop_assert!(r.success());
        prop_assert_eq!(r.data["files"].as_array().unwrap().len(), n);
        prop_assert_eq!(r.data["total_additions"].as_u64().unwrap(), n as u64);
    }

    /// Random non-diff input should fail without panicking. We test arbitrary ASCII bytes.
    #[test]
    fn diff_garbage_never_panics(s_in in "[\\x20-\\x7e\\n\\t]{0,200}") {
        let mut s = Server::spawn(&[]);
        let r = s.call("diff", json!({"input": s_in}));
        // Either succeeds (parsed valid diff) or returns a structured error — never crashes.
        // Just verify the response was received without panic.
        let _ = (r.success(), r.content_text);
    }
}

// ── pipe filter ────────────────────────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig { cases: PROPTEST_CASES, .. ProptestConfig::default() })]
    /// Pipe with `equals` filter on a literal pattern should return exactly the matching items.
    /// Verified against `find` source where we control the input set.
    #[test]
    fn pipe_equals_filter_exact(n in 1usize..6) {
        use std::fs;
        let d = tempfile::TempDir::new().unwrap();
        let target_name = "target_special.rs";
        fs::write(d.path().join(target_name), "x").unwrap();
        for i in 0..n {
            fs::write(d.path().join(format!("other_{i}.txt")), "x").unwrap();
        }

        let mut s = Server::spawn(&[]);
        let r = s.call("pipe", json!({
            "source": {"tool": "find", "params": {"path": d.path(), "type": "file"}},
            "filters": [{"field": "name", "pattern": target_name, "mode": "equals"}]
        }));
        prop_assert!(r.success());
        prop_assert_eq!(r.data["items"].as_array().unwrap().len(), 1);
    }

    /// Pipe limit always caps the items array length, regardless of source size.
    #[test]
    fn pipe_limit_cap(limit in 1usize..10, source_count in 0usize..15) {
        use std::fs;
        let d = tempfile::TempDir::new().unwrap();
        for i in 0..source_count {
            fs::write(d.path().join(format!("f{i}.txt")), "x").unwrap();
        }

        let mut s = Server::spawn(&[]);
        let r = s.call("pipe", json!({
            "source": {"tool": "ls", "params": {"path": d.path()}},
            "filters": [],
            "limit": limit
        }));
        prop_assert!(r.success());
        prop_assert!(r.data["items"].as_array().unwrap().len() <= limit);
    }
}

// ── wc invariants ──────────────────────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig { cases: PROPTEST_CASES, .. ProptestConfig::default() })]
    /// wc on inline text: bytes should equal input.len(); chars <= bytes.
    #[test]
    fn wc_byte_count_matches_input(text in "[\\x20-\\x7e\\n]{0,500}") {
        let mut s = Server::spawn(&[]);
        let r = s.call("wc", json!({"input": &text}));
        prop_assert!(r.success());
        prop_assert_eq!(r.data["bytes"].as_u64().unwrap(), text.len() as u64);
        prop_assert!(r.data["chars"].as_u64().unwrap() <= text.len() as u64);
    }

    /// wc line count equals the number of \n in input.
    #[test]
    fn wc_line_count(n_lines in 0usize..50) {
        let text: String = "line\n".repeat(n_lines);
        let mut s = Server::spawn(&[]);
        let r = s.call("wc", json!({"input": &text}));
        prop_assert!(r.success());
        prop_assert_eq!(r.data["lines"].as_u64().unwrap(), n_lines as u64);
    }
}

// ── batch invariants ───────────────────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig { cases: PROPTEST_CASES, .. ProptestConfig::default() })]
    /// batch with N ops returns exactly N results in input order.
    #[test]
    fn batch_preserves_count_and_order(n in 1usize..20) {
        let ops: Vec<_> = (0..n).map(|i| json!({
            "tool": "wc",
            "params": {"input": format!("op-{i}")}
        })).collect();
        let mut s = Server::spawn(&[]);
        let r = s.call("batch", json!({"operations": ops}));
        prop_assert!(r.success());
        let results = r.data["results"].as_array().unwrap();
        prop_assert_eq!(results.len(), n);
        for (i, result) in results.iter().enumerate() {
            prop_assert_eq!(
                result["params"]["input"].as_str().unwrap(),
                format!("op-{i}")
            );
        }
    }
}

// ── find invariants ────────────────────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig { cases: PROPTEST_CASES, .. ProptestConfig::default() })]
    /// find with limit always returns count <= limit.
    #[test]
    fn find_limit_caps_count(file_count in 1usize..15, limit in 1usize..10) {
        use std::fs;
        let d = tempfile::TempDir::new().unwrap();
        for i in 0..file_count {
            fs::write(d.path().join(format!("f{i}.txt")), "x").unwrap();
        }
        let mut s = Server::spawn(&[]);
        let r = s.call("find", json!({"path": d.path(), "limit": limit}));
        prop_assert!(r.success());
        prop_assert!(r.data["count"].as_u64().unwrap() <= limit as u64);
    }

    /// find max_depth=0 always returns entries with depth=0.
    #[test]
    fn find_depth_zero_invariant(_seed in any::<u8>()) {
        use std::fs;
        let d = tempfile::TempDir::new().unwrap();
        fs::create_dir_all(d.path().join("a/b/c")).unwrap();
        fs::write(d.path().join("a/b/c/deep.txt"), "x").unwrap();
        fs::write(d.path().join("top.txt"), "x").unwrap();

        let mut s = Server::spawn(&[]);
        let r = s.call("find", json!({"path": d.path(), "max_depth": 0}));
        prop_assert!(r.success());
        for entry in r.data["entries"].as_array().unwrap() {
            prop_assert_eq!(entry["depth"].as_u64().unwrap(), 0);
        }
    }
}
