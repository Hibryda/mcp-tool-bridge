use crate::harness::Server;
use serde_json::json;
use std::fs;
use tempfile::TempDir;

fn fixture() -> TempDir {
    let d = TempDir::new().unwrap();
    let p = d.path();
    fs::create_dir_all(p.join("src/nested/deep")).unwrap();
    fs::create_dir_all(p.join("docs")).unwrap();
    fs::create_dir_all(p.join("build")).unwrap();
    for i in 0..3 {
        fs::write(p.join(format!("src/file{i}.rs")), "x").unwrap();
    }
    fs::write(p.join("src/nested/deep/leaf.rs"), "x").unwrap();
    fs::write(p.join("docs/guide.md"), "doc").unwrap();
    fs::write(p.join("Cargo.toml"), "x").unwrap();
    fs::write(p.join("big.dat"), "x".repeat(10_000)).unwrap();
    d
}

#[test]
fn finds_all_files() {
    let d = fixture();
    let mut s = Server::spawn(&[]);
    let r = s.call("find", json!({"path": d.path()}));
    assert!(r.success());
    assert!(r.data["count"].as_u64().unwrap() >= 6);
}

#[test]
fn name_pattern_rs() {
    let d = fixture();
    let mut s = Server::spawn(&[]);
    let r = s.call(
        "find",
        json!({"path": d.path(), "name": "*.rs", "type": "file"}),
    );
    assert!(r.success());
    let entries = r.data["entries"].as_array().unwrap();
    assert!(!entries.is_empty());
    assert!(entries
        .iter()
        .all(|e| e["name"].as_str().unwrap().ends_with(".rs")));
}

#[test]
fn type_directory_only() {
    let d = fixture();
    let mut s = Server::spawn(&[]);
    let r = s.call("find", json!({"path": d.path(), "type": "directory"}));
    assert!(r.success());
    let entries = r.data["entries"].as_array().unwrap();
    assert!(entries.iter().all(|e| e["type"] == "directory"));
}

#[test]
fn type_file_only() {
    let d = fixture();
    let mut s = Server::spawn(&[]);
    let r = s.call("find", json!({"path": d.path(), "type": "file"}));
    assert!(r.success());
    let entries = r.data["entries"].as_array().unwrap();
    assert!(entries.iter().all(|e| e["type"] == "file"));
}

#[test]
fn max_depth_zero_returns_top_only() {
    let d = fixture();
    let mut s = Server::spawn(&[]);
    let r = s.call("find", json!({"path": d.path(), "max_depth": 0}));
    assert!(r.success());
    let entries = r.data["entries"].as_array().unwrap();
    assert!(entries.iter().all(|e| e["depth"].as_u64().unwrap() == 0));
}

#[test]
fn max_depth_two_includes_grandchildren() {
    let d = fixture();
    let mut s = Server::spawn(&[]);
    let r = s.call("find", json!({"path": d.path(), "max_depth": 2}));
    assert!(r.success());
    let max = r.data["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["depth"].as_u64().unwrap())
        .max()
        .unwrap();
    assert!(max <= 2);
}

#[test]
fn min_size_filter_excludes_small() {
    let d = fixture();
    let mut s = Server::spawn(&[]);
    let r = s.call(
        "find",
        json!({"path": d.path(), "type": "file", "min_size": 1000}),
    );
    assert!(r.success());
    let entries = r.data["entries"].as_array().unwrap();
    assert!(entries.iter().all(|e| e["size"].as_u64().unwrap() >= 1000));
}

#[test]
fn max_size_filter_excludes_large() {
    let d = fixture();
    let mut s = Server::spawn(&[]);
    let r = s.call(
        "find",
        json!({"path": d.path(), "type": "file", "max_size": 100}),
    );
    assert!(r.success());
    let entries = r.data["entries"].as_array().unwrap();
    assert!(entries.iter().all(|e| e["size"].as_u64().unwrap() <= 100));
}

#[test]
fn limit_truncates_with_flag() {
    let d = fixture();
    let mut s = Server::spawn(&[]);
    let r = s.call("find", json!({"path": d.path(), "limit": 2}));
    assert!(r.success());
    assert_eq!(r.data["entries"].as_array().unwrap().len(), 2);
    assert_eq!(r.data["truncated"], true);
}

#[test]
fn results_sorted_by_path() {
    let d = fixture();
    let mut s = Server::spawn(&[]);
    let r = s.call("find", json!({"path": d.path()}));
    assert!(r.success());
    let paths: Vec<&str> = r.data["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["path"].as_str().unwrap())
        .collect();
    let mut sorted = paths.clone();
    sorted.sort();
    assert_eq!(paths, sorted);
}

#[test]
fn nonexistent_path_errors() {
    let mut s = Server::spawn(&[]);
    let r = s.call("find", json!({"path": "/no/such/dir/zzz"}));
    assert!(!r.success());
}

#[test]
fn empty_directory_returns_empty_entries() {
    let d = TempDir::new().unwrap();
    let mut s = Server::spawn(&[]);
    let r = s.call("find", json!({"path": d.path()}));
    assert!(r.success());
    assert_eq!(r.data["count"], 0);
}

#[test]
fn root_field_echoes_input() {
    let d = fixture();
    let path_str = d.path().to_string_lossy().to_string();
    let mut s = Server::spawn(&[]);
    let r = s.call("find", json!({"path": &path_str}));
    assert!(r.success());
    assert_eq!(r.data["root"], path_str);
}

#[test]
fn nested_dir_traversal_finds_deep_file() {
    let d = fixture();
    let mut s = Server::spawn(&[]);
    let r = s.call("find", json!({"path": d.path(), "name": "leaf.rs"}));
    assert!(r.success());
    assert_eq!(r.data["count"], 1);
}

#[test]
fn no_match_returns_zero_count() {
    let d = fixture();
    let mut s = Server::spawn(&[]);
    let r = s.call(
        "find",
        json!({"path": d.path(), "name": "*.does_not_exist"}),
    );
    assert!(r.success());
    assert_eq!(r.data["count"], 0);
}
