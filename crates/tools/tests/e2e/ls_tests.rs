use crate::harness::Server;
use serde_json::json;
use std::fs;
use tempfile::TempDir;

fn fixture() -> TempDir {
    let dir = TempDir::new().unwrap();
    let p = dir.path();
    fs::create_dir_all(p.join("src")).unwrap();
    fs::create_dir_all(p.join(".hidden")).unwrap();
    fs::create_dir_all(p.join("empty")).unwrap();
    for i in 0..5 {
        fs::write(
            p.join(format!("src/file{i}.rs")),
            format!("fn f{i}() {{}}\n"),
        )
        .unwrap();
    }
    fs::write(p.join("README.md"), "# test\n\ncontent\n").unwrap();
    fs::write(p.join("Cargo.toml"), "[package]\n").unwrap();
    fs::write(p.join(".hidden/secret"), "x").unwrap();
    dir
}

#[test]
fn lists_directory() {
    let d = fixture();
    let mut s = Server::spawn(&[]);
    let r = s.call("ls", json!({"path": d.path()}));
    assert!(r.success());
    let entries = r.data.as_array().unwrap();
    assert!(entries.len() >= 4);
}

#[test]
fn excludes_hidden_by_default() {
    let d = fixture();
    let mut s = Server::spawn(&[]);
    let r = s.call("ls", json!({"path": d.path()}));
    assert!(r.success());
    assert!(!r
        .data
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["name"].as_str().unwrap().starts_with('.')));
}

#[test]
fn includes_hidden_with_all_flag() {
    let d = fixture();
    let mut s = Server::spawn(&[]);
    let r = s.call("ls", json!({"path": d.path(), "all": true}));
    assert!(r.success());
    assert!(r
        .data
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["name"].as_str().unwrap() == ".hidden"));
}

#[test]
fn long_mode_has_permissions_and_iso_mtime() {
    let d = fixture();
    let mut s = Server::spawn(&[]);
    let r = s.call("ls", json!({"path": d.path(), "long": true}));
    assert!(r.success());
    let entries = r.data.as_array().unwrap();
    assert!(entries
        .iter()
        .all(|e| !e["permissions"].as_str().unwrap().is_empty()));
    assert!(entries.iter().all(|e| {
        let m = e["modified"].as_str().unwrap_or("");
        m.contains('T') && (m.contains('+') || m.contains('Z'))
    }));
}

#[test]
fn short_mode_omits_permissions() {
    let d = fixture();
    let mut s = Server::spawn(&[]);
    let r = s.call("ls", json!({"path": d.path(), "long": false}));
    assert!(r.success());
    assert!(r
        .data
        .as_array()
        .unwrap()
        .iter()
        .all(|e| e["permissions"].as_str().unwrap().is_empty()));
}

#[test]
fn entries_sorted_alphabetically() {
    let d = fixture();
    let mut s = Server::spawn(&[]);
    let r = s.call("ls", json!({"path": d.path().join("src")}));
    assert!(r.success());
    let names: Vec<&str> = r
        .data
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["name"].as_str().unwrap())
        .collect();
    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(names, sorted);
}

#[test]
fn empty_directory_returns_empty_array() {
    let d = fixture();
    let mut s = Server::spawn(&[]);
    let r = s.call("ls", json!({"path": d.path().join("empty")}));
    assert!(r.success());
    assert!(r.data.as_array().unwrap().is_empty());
}

#[test]
fn nonexistent_path_errors() {
    let mut s = Server::spawn(&[]);
    let r = s.call("ls", json!({"path": "/definitely/does/not/exist/xyzzy"}));
    assert!(!r.success());
}

#[test]
fn detects_file_and_directory_types() {
    let d = fixture();
    let mut s = Server::spawn(&[]);
    let r = s.call("ls", json!({"path": d.path()}));
    assert!(r.success());
    let entries = r.data.as_array().unwrap();
    assert!(entries.iter().any(|e| e["type"] == "file"));
    assert!(entries.iter().any(|e| e["type"] == "directory"));
}

#[test]
fn returns_full_paths() {
    let d = fixture();
    let mut s = Server::spawn(&[]);
    let r = s.call("ls", json!({"path": d.path()}));
    assert!(r.success());
    let prefix = d.path().to_string_lossy().to_string();
    assert!(r
        .data
        .as_array()
        .unwrap()
        .iter()
        .all(|e| e["path"].as_str().unwrap().starts_with(&prefix)));
}

#[test]
fn file_sizes_are_correct() {
    let d = fixture();
    let mut s = Server::spawn(&[]);
    let r = s.call("ls", json!({"path": d.path()}));
    assert!(r.success());
    let readme = r
        .data
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["name"] == "README.md")
        .unwrap();
    let actual = std::fs::metadata(d.path().join("README.md")).unwrap().len();
    assert_eq!(readme["size"].as_u64().unwrap(), actual);
}

#[test]
fn lists_root_with_special_filesystems() {
    let mut s = Server::spawn(&[]);
    let r = s.call("ls", json!({"path": "/"}));
    assert!(r.success());
    assert!(r.data.as_array().unwrap().len() > 3);
}
