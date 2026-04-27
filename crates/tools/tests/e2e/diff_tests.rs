use crate::harness::Server;
use serde_json::json;

const SIMPLE: &str = "--- a/f.rs\n+++ b/f.rs\n@@ -1,3 +1,4 @@\n line\n+added\n line\n line\n";

const MULTI_HUNK: &str =
    "--- a/f.rs\n+++ b/f.rs\n@@ -1,3 +1,4 @@\n a\n+b\n c\n d\n@@ -10,3 +11,2 @@\n x\n-y\n z\n";

const RENAME: &str = "diff --git a/old.rs b/new.rs\nsimilarity index 90%\nrename from old.rs\nrename to new.rs\n--- a/old.rs\n+++ b/new.rs\n@@ -1 +1 @@\n-old\n+new\n";

const NEW_FILE: &str = "diff --git a/new.rs b/new.rs\nnew file mode 100644\n--- /dev/null\n+++ b/new.rs\n@@ -0,0 +1,2 @@\n+a\n+b\n";

#[test]
fn parses_simple_addition() {
    let mut s = Server::spawn(&[]);
    let r = s.call("diff", json!({"input": SIMPLE}));
    assert!(r.success());
    assert_eq!(r.data["total_additions"], 1);
    assert_eq!(r.data["total_deletions"], 0);
    assert_eq!(r.data["files"].as_array().unwrap().len(), 1);
}

#[test]
fn multi_hunk_counted_separately() {
    let mut s = Server::spawn(&[]);
    let r = s.call("diff", json!({"input": MULTI_HUNK}));
    assert!(r.success());
    let hunks = r.data["files"][0]["hunks"].as_array().unwrap();
    assert_eq!(hunks.len(), 2);
    assert_eq!(r.data["total_additions"], 1);
    assert_eq!(r.data["total_deletions"], 1);
}

#[test]
fn rename_paths_extracted() {
    let mut s = Server::spawn(&[]);
    let r = s.call("diff", json!({"input": RENAME}));
    assert!(r.success());
    assert_eq!(r.data["files"][0]["old_path"], "a/old.rs");
    assert_eq!(r.data["files"][0]["new_path"], "b/new.rs");
}

#[test]
fn new_file_old_path_is_devnull() {
    let mut s = Server::spawn(&[]);
    let r = s.call("diff", json!({"input": NEW_FILE}));
    assert!(r.success());
    assert_eq!(r.data["files"][0]["old_path"], "/dev/null");
    assert_eq!(r.data["total_additions"], 2);
}

#[test]
fn line_numbers_track_correctly() {
    let mut s = Server::spawn(&[]);
    let r = s.call("diff", json!({"input": SIMPLE}));
    assert!(r.success());
    let lines = r.data["files"][0]["hunks"][0]["lines"].as_array().unwrap();
    assert_eq!(lines[0]["kind"], "context");
    assert_eq!(lines[0]["old_line"], 1);
    assert_eq!(lines[1]["kind"], "add");
    assert!(lines[1]
        .get("old_line")
        .map(|v| v.is_null())
        .unwrap_or(true));
    assert_eq!(lines[1]["new_line"], 2);
}

#[test]
fn empty_input_returns_empty_result() {
    let mut s = Server::spawn(&[]);
    let r = s.call("diff", json!({"input": ""}));
    assert!(r.success());
    assert_eq!(r.data["files"].as_array().unwrap().len(), 0);
    assert_eq!(r.data["total_additions"], 0);
}

#[test]
fn non_unified_format_errors() {
    let mut s = Server::spawn(&[]);
    let r = s.call("diff", json!({"input": "diff --git a/f b/f\n f | 3 ++-\n"}));
    assert!(!r.success());
    assert!(r.content_text.contains("format_not_unified") || r.content_text.contains("stat"));
}

#[test]
fn binary_format_errors_with_directive() {
    let mut s = Server::spawn(&[]);
    let r = s.call(
        "diff",
        json!({"input": "Binary files a/img.png and b/img.png differ\n"}),
    );
    assert!(!r.success());
    assert!(r.content_text.contains("binary"));
}

#[test]
fn both_input_and_git_args_errors() {
    let mut s = Server::spawn(&[]);
    let r = s.call("diff", json!({"input": "x", "git_args": ["HEAD"]}));
    assert!(!r.success());
}

#[test]
fn no_params_errors() {
    let mut s = Server::spawn(&[]);
    let r = s.call("diff", json!({}));
    assert!(!r.success());
}

#[test]
fn git_args_against_real_repo() {
    // Run against the workspace repo itself.
    let mut s = Server::spawn(&[]);
    let r = s.call("diff", json!({"git_args": ["HEAD~1"]}));
    // Either succeeds (if HEAD~1 exists with changes) or fails gracefully.
    if r.success() {
        assert!(r.data["format"] == "unified");
    }
}

#[test]
fn many_hunks_in_single_file() {
    let mut input = String::from("--- a/f.rs\n+++ b/f.rs\n");
    for i in 0..10 {
        let s = i * 20 + 1;
        input.push_str(&format!(
            "@@ -{},2 +{},3 @@\n ctx\n+add{i}\n end\n",
            s,
            s + i
        ));
    }
    let mut s = Server::spawn(&[]);
    let r = s.call("diff", json!({"input": input}));
    assert!(r.success());
    assert_eq!(r.data["files"][0]["hunks"].as_array().unwrap().len(), 10);
}

#[test]
fn many_files_in_diff() {
    let mut input = String::new();
    for i in 0..20 {
        input.push_str(&format!(
            "diff --git a/f{i}.rs b/f{i}.rs\n--- a/f{i}.rs\n+++ b/f{i}.rs\n@@ -1,1 +1,2 @@\n existing\n+added\n"
        ));
    }
    let mut s = Server::spawn(&[]);
    let r = s.call("diff", json!({"input": input}));
    assert!(r.success());
    assert_eq!(r.data["files"].as_array().unwrap().len(), 20);
    assert_eq!(r.data["total_additions"], 20);
}

#[test]
fn deletion_only_hunk() {
    let input = "--- a/f.rs\n+++ b/f.rs\n@@ -1,3 +1,1 @@\n-a\n-b\n c\n";
    let mut s = Server::spawn(&[]);
    let r = s.call("diff", json!({"input": input}));
    assert!(r.success());
    assert_eq!(r.data["total_deletions"], 2);
    assert_eq!(r.data["total_additions"], 0);
}

#[test]
fn section_header_extracted() {
    let input = "--- a/f.rs\n+++ b/f.rs\n@@ -1,2 +1,3 @@ fn main() {\n ctx\n+added\n end\n";
    let mut s = Server::spawn(&[]);
    let r = s.call("diff", json!({"input": input}));
    assert!(r.success());
    assert_eq!(r.data["files"][0]["hunks"][0]["section"], "fn main() {");
}
