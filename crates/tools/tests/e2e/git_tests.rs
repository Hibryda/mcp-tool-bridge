//! E2E tests for git_status, git_log, git_show against a freshly-built repo fixture.

use crate::harness::Server;
use serde_json::json;
use std::process::Command;
use tempfile::TempDir;

fn init_git_repo() -> TempDir {
    let d = TempDir::new().unwrap();
    let p = d.path();
    let run = |args: &[&str]| {
        Command::new("git")
            .args(args)
            .current_dir(p)
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@example.com")
            .output()
            .expect("git command")
    };
    run(&["init", "-q", "-b", "main"]);
    run(&["config", "user.name", "Test"]);
    run(&["config", "user.email", "test@example.com"]);
    std::fs::write(p.join("README.md"), "# initial\n").unwrap();
    run(&["add", "README.md"]);
    run(&["commit", "-q", "-m", "feat: initial commit"]);
    std::fs::write(p.join("a.txt"), "alpha\n").unwrap();
    run(&["add", "a.txt"]);
    run(&["commit", "-q", "-m", "feat: add a"]);
    std::fs::write(p.join("b.txt"), "beta\n").unwrap();
    run(&["add", "b.txt"]);
    run(&["commit", "-q", "-m", "feat: add b\n\nWith body."]);
    d
}

// ── git_status ──────────────────────────────────────────────────────

#[test]
fn status_clean_repo() {
    let d = init_git_repo();
    let mut s = Server::spawn(&[]);
    let r = s.call("git_status", json!({"path": d.path()}));
    assert!(r.success());
    assert_eq!(r.data["clean"], true);
    assert_eq!(r.data["entries"].as_array().unwrap().len(), 0);
}

#[test]
fn status_branch_head() {
    let d = init_git_repo();
    let mut s = Server::spawn(&[]);
    let r = s.call("git_status", json!({"path": d.path()}));
    assert!(r.success());
    assert_eq!(r.data["branch"]["head"], "main");
}

#[test]
fn status_untracked_file() {
    let d = init_git_repo();
    std::fs::write(d.path().join("new.txt"), "x").unwrap();
    let mut s = Server::spawn(&[]);
    let r = s.call("git_status", json!({"path": d.path()}));
    assert!(r.success());
    assert_eq!(r.data["clean"], false);
    let entries = r.data["entries"].as_array().unwrap();
    assert!(entries.iter().any(|e| e["status"] == "untracked"));
}

#[test]
fn status_modified_file() {
    let d = init_git_repo();
    std::fs::write(d.path().join("README.md"), "# changed\n").unwrap();
    let mut s = Server::spawn(&[]);
    let r = s.call("git_status", json!({"path": d.path()}));
    assert!(r.success());
    let entries = r.data["entries"].as_array().unwrap();
    assert!(entries.iter().any(|e| e["status"] == "modified"));
}

#[test]
fn status_show_untracked_false_hides_untracked() {
    let d = init_git_repo();
    std::fs::write(d.path().join("hidden.txt"), "x").unwrap();
    let mut s = Server::spawn(&[]);
    let r = s.call(
        "git_status",
        json!({"path": d.path(), "show_untracked": false}),
    );
    assert!(r.success());
    let entries = r.data["entries"].as_array().unwrap();
    assert!(!entries.iter().any(|e| e["status"] == "untracked"));
}

#[test]
fn status_not_a_repo_errors() {
    let d = TempDir::new().unwrap();
    let mut s = Server::spawn(&[]);
    let r = s.call("git_status", json!({"path": d.path()}));
    assert!(!r.success());
    assert!(r.content_text.contains("NOT_A_REPO"));
}

#[test]
fn status_counts_field_present() {
    let d = init_git_repo();
    let mut s = Server::spawn(&[]);
    let r = s.call("git_status", json!({"path": d.path()}));
    assert!(r.success());
    let counts = &r.data["counts"];
    for k in [
        "modified",
        "added",
        "deleted",
        "renamed",
        "untracked",
        "unmerged",
    ] {
        assert!(counts[k].is_number(), "missing counts.{}", k);
    }
}

// ── git_log ─────────────────────────────────────────────────────────

#[test]
fn log_returns_commits() {
    let d = init_git_repo();
    let mut s = Server::spawn(&[]);
    let r = s.call("git_log", json!({"path": d.path()}));
    assert!(r.success());
    let commits = r.data["commits"].as_array().unwrap();
    assert_eq!(commits.len(), 3);
}

#[test]
fn log_max_count_limits() {
    let d = init_git_repo();
    let mut s = Server::spawn(&[]);
    let r = s.call("git_log", json!({"path": d.path(), "max_count": 2}));
    assert!(r.success());
    assert_eq!(r.data["commits"].as_array().unwrap().len(), 2);
}

#[test]
fn log_commit_has_required_fields() {
    let d = init_git_repo();
    let mut s = Server::spawn(&[]);
    let r = s.call("git_log", json!({"path": d.path()}));
    assert!(r.success());
    let c = &r.data["commits"][0];
    for k in [
        "hash",
        "author_name",
        "author_email",
        "date",
        "subject",
        "parent_hashes",
        "refs",
        "is_merge",
    ] {
        assert!(!c[k].is_null(), "missing field {}", k);
    }
}

#[test]
fn log_hash_is_full_40_chars() {
    let d = init_git_repo();
    let mut s = Server::spawn(&[]);
    let r = s.call("git_log", json!({"path": d.path()}));
    assert!(r.success());
    let h = r.data["commits"][0]["hash"].as_str().unwrap();
    assert_eq!(h.len(), 40);
}

#[test]
fn log_with_stats_populates_numstat() {
    let d = init_git_repo();
    let mut s = Server::spawn(&[]);
    let r = s.call("git_log", json!({"path": d.path(), "include_stats": true}));
    assert!(r.success());
    let c = &r.data["commits"][0];
    assert!(!c["stats"].is_null());
    assert!(c["stats"]["additions"].is_number());
}

#[test]
fn log_snapshot_oid_is_full_hash() {
    let d = init_git_repo();
    let mut s = Server::spawn(&[]);
    let r = s.call("git_log", json!({"path": d.path()}));
    assert!(r.success());
    assert_eq!(r.data["snapshot_oid"].as_str().unwrap().len(), 40);
}

#[test]
fn log_snapshot_oid_stable_across_calls() {
    // Two consecutive log calls on a quiet repo should return the same snapshot_oid
    // (no new commits in between). This is the foundation of pagination stability.
    let d = init_git_repo();
    let mut s = Server::spawn(&[]);
    let p1 = s.call("git_log", json!({"path": d.path(), "max_count": 1}));
    assert!(p1.success());
    let p2 = s.call("git_log", json!({"path": d.path(), "max_count": 5}));
    assert!(p2.success());
    assert_eq!(p1.data["snapshot_oid"], p2.data["snapshot_oid"]);
}

#[test]
fn log_parse_warnings_always_present() {
    let d = init_git_repo();
    let mut s = Server::spawn(&[]);
    let r = s.call("git_log", json!({"path": d.path()}));
    assert!(r.success());
    assert!(r.data["parse_warnings"].is_array());
}

#[test]
fn log_not_a_repo_errors() {
    let d = TempDir::new().unwrap();
    let mut s = Server::spawn(&[]);
    let r = s.call("git_log", json!({"path": d.path()}));
    assert!(!r.success());
}

// ── git_show ────────────────────────────────────────────────────────

#[test]
fn show_head_commit() {
    let d = init_git_repo();
    let mut s = Server::spawn(&[]);
    let r = s.call("git_show", json!({"path": d.path(), "ref": "HEAD"}));
    assert!(r.success());
    assert_eq!(r.data["subject"], "feat: add b");
}

#[test]
fn show_body_extracted() {
    let d = init_git_repo();
    let mut s = Server::spawn(&[]);
    let r = s.call("git_show", json!({"path": d.path(), "ref": "HEAD"}));
    assert!(r.success());
    assert!(r.data["body"].as_str().unwrap().contains("With body"));
}

#[test]
fn show_invalid_ref_errors() {
    let d = init_git_repo();
    let mut s = Server::spawn(&[]);
    let r = s.call(
        "git_show",
        json!({"path": d.path(), "ref": "definitely-not-a-ref-xyz"}),
    );
    assert!(!r.success());
    assert!(r.content_text.contains("INVALID_REF") || r.content_text.contains("invalid"));
}

#[test]
fn show_with_stats() {
    let d = init_git_repo();
    let mut s = Server::spawn(&[]);
    let r = s.call(
        "git_show",
        json!({"path": d.path(), "ref": "HEAD", "include_stats": true}),
    );
    assert!(r.success());
    assert!(!r.data["stats"].is_null());
}

#[test]
fn show_parent_hashes_for_non_root() {
    let d = init_git_repo();
    let mut s = Server::spawn(&[]);
    let r = s.call("git_show", json!({"path": d.path(), "ref": "HEAD"}));
    assert!(r.success());
    assert_eq!(r.data["parent_hashes"].as_array().unwrap().len(), 1);
    assert_eq!(r.data["is_merge"], false);
}

#[test]
fn show_root_commit_has_no_parents() {
    let d = init_git_repo();
    // Find the root commit hash.
    let out = Command::new("git")
        .args(["rev-list", "--max-parents=0", "HEAD"])
        .current_dir(d.path())
        .output()
        .unwrap();
    let root = String::from_utf8(out.stdout).unwrap().trim().to_string();
    let mut s = Server::spawn(&[]);
    let r = s.call("git_show", json!({"path": d.path(), "ref": root}));
    assert!(r.success());
    assert_eq!(r.data["parent_hashes"].as_array().unwrap().len(), 0);
}
