//! Find tool — structured file search with type, name, size, and time filters.

use bridge_core::BridgeError;
use serde::Serialize;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time::UNIX_EPOCH;

/// A single find result entry.
#[derive(Debug, Serialize, Clone)]
pub struct FindEntry {
    pub path: String,
    #[serde(rename = "type")]
    pub entry_type: String,
    pub name: String,
    pub size: u64,
    pub permissions: String,
    pub modified: Option<String>,
    /// Depth relative to the search root.
    pub depth: u32,
}

/// Find result.
#[derive(Debug, Serialize, Clone)]
pub struct FindResult {
    pub root: String,
    pub entries: Vec<FindEntry>,
    pub count: u64,
    pub truncated: bool,
}

/// Recursively find files matching criteria.
pub async fn find_files(
    root: &str,
    name_pattern: Option<&str>,
    file_type: Option<&str>,
    max_depth: Option<u32>,
    min_size: Option<u64>,
    max_size: Option<u64>,
    limit: Option<usize>,
) -> Result<FindResult, BridgeError> {
    let root_path = Path::new(root);
    if !root_path.exists() {
        return Err(BridgeError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("path not found: {root}"),
        )));
    }

    let max_results = limit.unwrap_or(1000);
    let max_depth = max_depth.unwrap_or(10);
    let mut entries = Vec::new();
    let mut truncated = false;

    walk_dir(
        root_path,
        root_path,
        0,
        max_depth,
        name_pattern,
        file_type,
        min_size,
        max_size,
        max_results,
        &mut entries,
        &mut truncated,
    )?;

    entries.sort_by(|a, b| a.path.cmp(&b.path));
    let count = entries.len() as u64;

    Ok(FindResult {
        root: root.to_string(),
        entries,
        count,
        truncated,
    })
}

fn walk_dir(
    path: &Path,
    root: &Path,
    depth: u32,
    max_depth: u32,
    name_pattern: Option<&str>,
    file_type: Option<&str>,
    min_size: Option<u64>,
    max_size: Option<u64>,
    max_results: usize,
    results: &mut Vec<FindEntry>,
    truncated: &mut bool,
) -> Result<(), BridgeError> {
    if depth > max_depth || results.len() >= max_results {
        if results.len() >= max_results {
            *truncated = true;
        }
        return Ok(());
    }

    let read_dir = match std::fs::read_dir(path) {
        Ok(rd) => rd,
        Err(_) => return Ok(()), // Skip unreadable dirs
    };

    for entry in read_dir {
        if results.len() >= max_results {
            *truncated = true;
            return Ok(());
        }

        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        let name = entry.file_name().to_string_lossy().to_string();
        let entry_path = entry.path();
        let entry_type = if metadata.is_dir() {
            "directory"
        } else if metadata.is_symlink() {
            "symlink"
        } else {
            "file"
        };

        // Apply filters
        if let Some(ft) = file_type {
            let matches = match ft {
                "f" | "file" => entry_type == "file",
                "d" | "dir" | "directory" => entry_type == "directory",
                "l" | "link" | "symlink" => entry_type == "symlink",
                _ => true,
            };
            if !matches {
                // Still recurse into dirs even if type doesn't match
                if metadata.is_dir() {
                    walk_dir(
                        &entry_path, root, depth + 1, max_depth,
                        name_pattern, file_type, min_size, max_size,
                        max_results, results, truncated,
                    )?;
                }
                continue;
            }
        }

        if let Some(pattern) = name_pattern {
            if !matches_glob(&name, pattern) {
                if metadata.is_dir() {
                    walk_dir(
                        &entry_path, root, depth + 1, max_depth,
                        name_pattern, file_type, min_size, max_size,
                        max_results, results, truncated,
                    )?;
                }
                continue;
            }
        }

        if let Some(min) = min_size {
            if metadata.len() < min {
                if metadata.is_dir() {
                    walk_dir(
                        &entry_path, root, depth + 1, max_depth,
                        name_pattern, file_type, min_size, max_size,
                        max_results, results, truncated,
                    )?;
                }
                continue;
            }
        }

        if let Some(max) = max_size {
            if metadata.len() > max {
                if metadata.is_dir() {
                    walk_dir(
                        &entry_path, root, depth + 1, max_depth,
                        name_pattern, file_type, min_size, max_size,
                        max_results, results, truncated,
                    )?;
                }
                continue;
            }
        }

        let modified = metadata.modified().ok().and_then(|t| {
            t.duration_since(UNIX_EPOCH).ok().map(|d| {
                let secs = d.as_secs() as i64;
                chrono::DateTime::from_timestamp(secs, 0)
                    .map(|d| d.to_rfc3339())
                    .unwrap_or_else(|| secs.to_string())
            })
        });

        results.push(FindEntry {
            path: entry_path.to_string_lossy().to_string(),
            entry_type: entry_type.to_string(),
            name,
            size: metadata.len(),
            permissions: format!("{:o}", metadata.permissions().mode() & 0o777),
            modified,
            depth,
        });

        // Recurse into directories
        if metadata.is_dir() {
            walk_dir(
                &entry_path, root, depth + 1, max_depth,
                name_pattern, file_type, min_size, max_size,
                max_results, results, truncated,
            )?;
        }
    }

    Ok(())
}

/// Simple glob matching: supports * and ? wildcards.
fn matches_glob(name: &str, pattern: &str) -> bool {
    if pattern == "*" {
        return true;
    }

    // Handle *.ext pattern
    if let Some(ext) = pattern.strip_prefix("*.") {
        return name.ends_with(&format!(".{ext}"));
    }

    // Handle prefix* pattern
    if let Some(prefix) = pattern.strip_suffix('*') {
        return name.starts_with(prefix);
    }

    // Handle *contains* pattern
    if pattern.starts_with('*') && pattern.ends_with('*') {
        let inner = &pattern[1..pattern.len() - 1];
        return name.contains(inner);
    }

    // Handle *suffix
    if let Some(suffix) = pattern.strip_prefix('*') {
        return name.ends_with(suffix);
    }

    // Exact match
    name == pattern
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn create_test_tree() -> tempfile::TempDir {
        let dir = tempfile::TempDir::new().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::create_dir_all(root.join("src/nested")).unwrap();
        fs::create_dir_all(root.join("docs")).unwrap();
        fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();
        fs::write(root.join("src/lib.rs"), "pub fn lib() {}").unwrap();
        fs::write(root.join("src/nested/deep.rs"), "mod deep;").unwrap();
        fs::write(root.join("docs/README.md"), "# Docs").unwrap();
        fs::write(root.join("Cargo.toml"), "[package]").unwrap();
        fs::write(root.join("big.dat"), "x".repeat(10000)).unwrap();
        dir
    }

    #[tokio::test]
    async fn find_all() {
        let dir = create_test_tree();
        let r = find_files(dir.path().to_str().unwrap(), None, None, None, None, None, None).await.unwrap();
        assert!(r.count > 5);
    }

    #[tokio::test]
    async fn find_by_name() {
        let dir = create_test_tree();
        let r = find_files(dir.path().to_str().unwrap(), Some("*.rs"), None, None, None, None, None).await.unwrap();
        assert_eq!(r.count, 3);
        assert!(r.entries.iter().all(|e| e.name.ends_with(".rs")));
    }

    #[tokio::test]
    async fn find_by_type_file() {
        let dir = create_test_tree();
        let r = find_files(dir.path().to_str().unwrap(), None, Some("file"), None, None, None, None).await.unwrap();
        assert!(r.entries.iter().all(|e| e.entry_type == "file"));
    }

    #[tokio::test]
    async fn find_by_type_dir() {
        let dir = create_test_tree();
        let r = find_files(dir.path().to_str().unwrap(), None, Some("directory"), None, None, None, None).await.unwrap();
        assert!(r.entries.iter().all(|e| e.entry_type == "directory"));
        assert!(r.count >= 3); // src, src/nested, docs
    }

    #[tokio::test]
    async fn find_max_depth() {
        let dir = create_test_tree();
        let r = find_files(dir.path().to_str().unwrap(), None, None, Some(0), None, None, None).await.unwrap();
        assert!(r.entries.iter().all(|e| e.depth == 0));
    }

    #[tokio::test]
    async fn find_min_size() {
        let dir = create_test_tree();
        let r = find_files(dir.path().to_str().unwrap(), None, Some("file"), None, Some(5000), None, None).await.unwrap();
        assert_eq!(r.count, 1);
        assert!(r.entries[0].name == "big.dat");
    }

    #[tokio::test]
    async fn find_limit() {
        let dir = create_test_tree();
        let r = find_files(dir.path().to_str().unwrap(), None, None, None, None, None, Some(3)).await.unwrap();
        assert_eq!(r.count, 3);
        assert!(r.truncated);
    }

    #[tokio::test]
    async fn find_nonexistent_errors() {
        let r = find_files("/nonexistent/path/xyz", None, None, None, None, None, None).await;
        assert!(r.is_err());
    }

    #[tokio::test]
    async fn find_results_sorted() {
        let dir = create_test_tree();
        let r = find_files(dir.path().to_str().unwrap(), None, None, None, None, None, None).await.unwrap();
        let paths: Vec<&str> = r.entries.iter().map(|e| e.path.as_str()).collect();
        let mut sorted = paths.clone();
        sorted.sort();
        assert_eq!(paths, sorted);
    }

    #[test]
    fn glob_matching() {
        assert!(matches_glob("main.rs", "*.rs"));
        assert!(!matches_glob("main.py", "*.rs"));
        assert!(matches_glob("Cargo.toml", "Cargo*"));
        assert!(matches_glob("test", "*"));
        assert!(matches_glob("README.md", "README.md"));
        assert!(!matches_glob("README.txt", "README.md"));
    }
}
