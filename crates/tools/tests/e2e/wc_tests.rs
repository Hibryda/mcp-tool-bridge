use crate::harness::Server;
use serde_json::json;
use std::fs;
use tempfile::TempDir;

fn write_file(dir: &TempDir, name: &str, content: &str) -> std::path::PathBuf {
    let p = dir.path().join(name);
    fs::write(&p, content).unwrap();
    p
}

#[test]
fn counts_single_file() {
    let d = TempDir::new().unwrap();
    let content = "line one\nline two\nline three\n";
    let p = write_file(&d, "f.txt", content);
    let mut s = Server::spawn(&[]);
    let r = s.call("wc", json!({"path": p}));
    assert!(r.success());
    assert_eq!(r.data["lines"], 3);
    assert_eq!(r.data["words"], 6);
    assert_eq!(r.data["bytes"], content.len() as u64);
}

#[test]
fn counts_inline_text() {
    let mut s = Server::spawn(&[]);
    let r = s.call("wc", json!({"input": "hello world\nfoo bar baz"}));
    assert!(r.success());
    assert_eq!(r.data["lines"], 2);
    assert_eq!(r.data["words"], 5);
}

#[test]
fn empty_string_yields_zero_counts() {
    let mut s = Server::spawn(&[]);
    let r = s.call("wc", json!({"input": ""}));
    assert!(r.success());
    assert_eq!(r.data["lines"], 0);
    assert_eq!(r.data["words"], 0);
    assert_eq!(r.data["bytes"], 0);
}

#[test]
fn multibyte_unicode_chars_lt_bytes() {
    let mut s = Server::spawn(&[]);
    let r = s.call("wc", json!({"input": "héllo wörld\n日本語"}));
    assert!(r.success());
    let chars = r.data["chars"].as_u64().unwrap();
    let bytes = r.data["bytes"].as_u64().unwrap();
    assert!(bytes > chars);
}

#[test]
fn empty_file_returns_zero() {
    let d = TempDir::new().unwrap();
    let p = write_file(&d, "empty.txt", "");
    let mut s = Server::spawn(&[]);
    let r = s.call("wc", json!({"path": p}));
    assert!(r.success());
    assert_eq!(r.data["lines"], 0);
    assert_eq!(r.data["bytes"], 0);
}

#[test]
fn large_file_count_correct() {
    let d = TempDir::new().unwrap();
    let mut content = String::new();
    for i in 0..1000 {
        content.push_str(&format!("line {i}\n"));
    }
    let p = write_file(&d, "big.log", &content);
    let mut s = Server::spawn(&[]);
    let r = s.call("wc", json!({"path": p}));
    assert!(r.success());
    assert_eq!(r.data["lines"], 1000);
}

#[test]
fn multi_path_returns_array() {
    let d = TempDir::new().unwrap();
    let p1 = write_file(&d, "a.txt", "a\n");
    let p2 = write_file(&d, "b.txt", "b\nc\n");
    let mut s = Server::spawn(&[]);
    let r = s.call("wc", json!({"paths": [p1, p2]}));
    assert!(r.success());
    let arr = r.data.as_array().unwrap();
    assert_eq!(arr.len(), 2);
}

#[test]
fn single_path_in_paths_returns_object() {
    let d = TempDir::new().unwrap();
    let p = write_file(&d, "a.txt", "x\n");
    let mut s = Server::spawn(&[]);
    let r = s.call("wc", json!({"paths": [p]}));
    assert!(r.success());
    assert!(r.data.is_object());
}

#[test]
fn multi_path_records_per_file_errors() {
    let d = TempDir::new().unwrap();
    let p_ok = write_file(&d, "ok.txt", "x\n");
    let mut s = Server::spawn(&[]);
    let r = s.call("wc", json!({"paths": [p_ok, "/nonexistent/zzz"]}));
    assert!(r.success());
    let arr = r.data.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert!(arr.iter().any(|e| e.get("error").is_some()));
}

#[test]
fn path_and_input_together_errors() {
    let mut s = Server::spawn(&[]);
    let r = s.call("wc", json!({"path": "/etc/hosts", "input": "x"}));
    assert!(!r.success());
}

#[test]
fn no_params_errors() {
    let mut s = Server::spawn(&[]);
    let r = s.call("wc", json!({}));
    assert!(!r.success());
}

#[test]
fn nonexistent_path_errors() {
    let mut s = Server::spawn(&[]);
    let r = s.call("wc", json!({"path": "/nope/file.txt"}));
    assert!(!r.success());
}

#[test]
fn only_newlines_counts_lines_no_words() {
    let mut s = Server::spawn(&[]);
    let r = s.call("wc", json!({"input": "\n\n\n\n"}));
    assert!(r.success());
    assert_eq!(r.data["lines"], 4);
    assert_eq!(r.data["words"], 0);
}

#[test]
fn whitespace_collapses_in_word_count() {
    let mut s = Server::spawn(&[]);
    let r = s.call("wc", json!({"input": "  one\ttwo \n three  "}));
    assert!(r.success());
    assert_eq!(r.data["words"], 3);
}

#[test]
fn very_long_single_line() {
    let mut s = Server::spawn(&[]);
    let big = "x".repeat(50_000);
    let r = s.call("wc", json!({"input": big}));
    assert!(r.success());
    assert_eq!(r.data["chars"], 50_000);
    assert_eq!(r.data["bytes"], 50_000);
}
