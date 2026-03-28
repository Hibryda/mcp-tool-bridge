use bridge_core::FileEntry;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time::UNIX_EPOCH;

/// List directory contents as structured entries.
pub async fn list_directory(
    path: &str,
    all: bool,
    long: bool,
) -> Result<Vec<FileEntry>, bridge_core::BridgeError> {
    let dir_path = Path::new(path);

    let mut entries = Vec::new();
    let mut read_dir = tokio::fs::read_dir(dir_path).await?;

    while let Some(entry) = read_dir.next_entry().await? {
        let name = entry.file_name().to_string_lossy().to_string();

        // Skip hidden files unless --all
        if !all && name.starts_with('.') {
            continue;
        }

        let metadata = entry.metadata().await?;
        let file_type = if metadata.is_dir() {
            "directory"
        } else if metadata.is_symlink() {
            "symlink"
        } else {
            "file"
        };

        let permissions = if long {
            format!("{:o}", metadata.permissions().mode() & 0o777)
        } else {
            String::new()
        };

        let modified = if long {
            metadata
                .modified()
                .ok()
                .and_then(|t| {
                    t.duration_since(UNIX_EPOCH).ok().map(|d| {
                        let secs = d.as_secs() as i64;
                        let dt = chrono::DateTime::from_timestamp(secs, 0);
                        dt.map(|d| d.to_rfc3339())
                            .unwrap_or_else(|| secs.to_string())
                    })
                })
        } else {
            None
        };

        entries.push(FileEntry {
            name,
            path: entry.path().to_string_lossy().to_string(),
            entry_type: file_type.to_string(),
            size: metadata.len(),
            permissions,
            modified,
        });
    }

    // Sort by name for consistent output
    entries.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn list_current_dir() {
        let entries = list_directory(".", false, true).await.unwrap();
        assert!(!entries.is_empty());
        // Should find Cargo.toml in the workspace root
        assert!(!entries.is_empty());
    }

    #[tokio::test]
    async fn hidden_files_filtered() {
        let dir = std::env::temp_dir().join("mcp-ls-test");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("visible.txt"), "").unwrap();
        std::fs::write(dir.join(".hidden"), "").unwrap();

        let without_hidden = list_directory(dir.to_str().unwrap(), false, false).await.unwrap();
        let with_hidden = list_directory(dir.to_str().unwrap(), true, false).await.unwrap();

        assert!(without_hidden.iter().all(|e| !e.name.starts_with('.')));
        assert!(with_hidden.iter().any(|e| e.name == ".hidden"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn entries_sorted_by_name() {
        let dir = std::env::temp_dir().join("mcp-ls-sort-test");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("zebra.txt"), "").unwrap();
        std::fs::write(dir.join("alpha.txt"), "").unwrap();
        std::fs::write(dir.join("middle.txt"), "").unwrap();

        let entries = list_directory(dir.to_str().unwrap(), false, false).await.unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["alpha.txt", "middle.txt", "zebra.txt"]);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn nonexistent_dir_errors() {
        let result = list_directory("/nonexistent/path/xyz", false, false).await;
        assert!(result.is_err());
    }
}
