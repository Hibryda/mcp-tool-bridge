use bridge_core::BridgeError;
use serde::Serialize;

/// A single hunk from a unified diff.
#[derive(Debug, Serialize, Clone)]
pub struct DiffHunk {
    /// Starting line in the original file (1-indexed).
    pub old_start: u64,
    /// Number of lines in the original file.
    pub old_count: u64,
    /// Starting line in the new file (1-indexed).
    pub new_start: u64,
    /// Number of lines in the new file.
    pub new_count: u64,
    /// Optional section header (function name, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub section: Option<String>,
    /// Individual line changes within this hunk.
    pub lines: Vec<DiffLine>,
}

/// A single line in a diff hunk.
#[derive(Debug, Serialize, Clone)]
pub struct DiffLine {
    /// "add", "delete", or "context"
    pub kind: String,
    /// The line content (without the leading +/-/space).
    pub content: String,
    /// Line number in the old file (None for additions).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_line: Option<u64>,
    /// Line number in the new file (None for deletions).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_line: Option<u64>,
}

/// A single file's diff within a multi-file unified diff.
#[derive(Debug, Serialize, Clone)]
pub struct FileDiff {
    /// Original file path (from --- line).
    pub old_path: String,
    /// New file path (from +++ line).
    pub new_path: String,
    /// Whether this is a binary file diff.
    pub is_binary: bool,
    /// Hunks in this file diff.
    pub hunks: Vec<DiffHunk>,
}

/// Result of parsing a unified diff.
#[derive(Debug, Serialize, Clone)]
pub struct DiffResult {
    /// The format detected.
    pub format: String,
    /// Individual file diffs.
    pub files: Vec<FileDiff>,
    /// Total number of additions across all files.
    pub total_additions: u64,
    /// Total number of deletions across all files.
    pub total_deletions: u64,
}

/// Error returned when the diff format is not unified.
#[derive(Debug, Serialize, Clone)]
pub struct DiffFormatError {
    pub error: String,
    pub format_detected: String,
    pub directive: String,
}

/// Detect whether input looks like unified diff format.
/// Scans first 20 lines for unified diff markers.
fn detect_format(input: &str) -> Result<(), DiffFormatError> {
    let first_lines: Vec<&str> = input.lines().take(20).collect();

    let has_minus = first_lines.iter().any(|l| l.starts_with("--- "));
    let has_plus = first_lines.iter().any(|l| l.starts_with("+++ "));
    let has_hunk = first_lines.iter().any(|l| l.starts_with("@@ "));

    if has_minus && has_plus && has_hunk {
        return Ok(());
    }

    // Try to detect what format it actually is
    let format_detected = if first_lines.iter().any(|l| l.starts_with("diff --git")) {
        // Has git header but no hunks — might be --stat or --name-only
        if first_lines.iter().any(|l| l.contains('|') && (l.contains('+') || l.contains('-'))) {
            "stat"
        } else if first_lines.iter().all(|l| !l.starts_with("--- ")) {
            "name-only"
        } else {
            "unknown"
        }
    } else if first_lines.iter().any(|l| l.starts_with("Binary files")) {
        "binary"
    } else {
        "unknown"
    };

    Err(DiffFormatError {
        error: "format_not_unified".to_string(),
        format_detected: format_detected.to_string(),
        directive: "Use the shell tool to invoke diff/git diff directly; this tool only processes standard unified text diffs.".to_string(),
    })
}

/// Parse a unified diff string into structured output.
pub fn parse_unified_diff(input: &str) -> Result<DiffResult, DiffFormatError> {
    detect_format(input)?;

    let mut files: Vec<FileDiff> = Vec::new();
    let mut current_file: Option<FileDiff> = None;
    let mut current_hunk: Option<DiffHunk> = None;
    let mut old_line: u64 = 0;
    let mut new_line: u64 = 0;

    for line in input.lines() {
        if line.starts_with("--- ") {
            // Save previous file if any
            if let Some(mut file) = current_file.take() {
                if let Some(hunk) = current_hunk.take() {
                    file.hunks.push(hunk);
                }
                files.push(file);
            }
            let old_path = line.strip_prefix("--- ").unwrap_or("").to_string();
            // Strip tab-separated timestamp if present
            let old_path = old_path.split('\t').next().unwrap_or("").to_string();
            current_file = Some(FileDiff {
                old_path,
                new_path: String::new(),
                is_binary: false,
                hunks: Vec::new(),
            });
            continue;
        }

        if line.starts_with("+++ ") {
            if let Some(ref mut file) = current_file {
                let new_path = line.strip_prefix("+++ ").unwrap_or("").to_string();
                file.new_path = new_path.split('\t').next().unwrap_or("").to_string();
            }
            continue;
        }

        if line.starts_with("@@ ") {
            // Save previous hunk
            if let Some(ref mut file) = current_file {
                if let Some(hunk) = current_hunk.take() {
                    file.hunks.push(hunk);
                }
            }

            // Parse hunk header: @@ -old_start,old_count +new_start,new_count @@ section
            if let Some(hunk) = parse_hunk_header(line) {
                old_line = hunk.old_start;
                new_line = hunk.new_start;
                current_hunk = Some(hunk);
            }
            continue;
        }

        // Skip git diff headers and index lines
        if line.starts_with("diff --git")
            || line.starts_with("index ")
            || line.starts_with("old mode")
            || line.starts_with("new mode")
            || line.starts_with("similarity index")
            || line.starts_with("rename from")
            || line.starts_with("rename to")
            || line.starts_with("new file mode")
            || line.starts_with("deleted file mode")
        {
            continue;
        }

        if line.starts_with("Binary files") {
            if let Some(ref mut file) = current_file {
                file.is_binary = true;
            }
            continue;
        }

        // Parse diff lines within a hunk
        if let Some(ref mut hunk) = current_hunk {
            if let Some(content) = line.strip_prefix('+') {
                hunk.lines.push(DiffLine {
                    kind: "add".to_string(),
                    content: content.to_string(),
                    old_line: None,
                    new_line: Some(new_line),
                });
                new_line += 1;
            } else if let Some(content) = line.strip_prefix('-') {
                hunk.lines.push(DiffLine {
                    kind: "delete".to_string(),
                    content: content.to_string(),
                    old_line: Some(old_line),
                    new_line: None,
                });
                old_line += 1;
            } else if let Some(content) = line.strip_prefix(' ') {
                hunk.lines.push(DiffLine {
                    kind: "context".to_string(),
                    content: content.to_string(),
                    old_line: Some(old_line),
                    new_line: Some(new_line),
                });
                old_line += 1;
                new_line += 1;
            } else if line == "\\ No newline at end of file" {
                // Skip no-newline marker
            } else if !line.is_empty() {
                // Context line without leading space (some diff implementations)
                hunk.lines.push(DiffLine {
                    kind: "context".to_string(),
                    content: line.to_string(),
                    old_line: Some(old_line),
                    new_line: Some(new_line),
                });
                old_line += 1;
                new_line += 1;
            }
        }
    }

    // Push final hunk and file
    if let Some(mut file) = current_file.take() {
        if let Some(hunk) = current_hunk.take() {
            file.hunks.push(hunk);
        }
        files.push(file);
    }

    let total_additions: u64 = files
        .iter()
        .flat_map(|f| &f.hunks)
        .flat_map(|h| &h.lines)
        .filter(|l| l.kind == "add")
        .count() as u64;

    let total_deletions: u64 = files
        .iter()
        .flat_map(|f| &f.hunks)
        .flat_map(|h| &h.lines)
        .filter(|l| l.kind == "delete")
        .count() as u64;

    Ok(DiffResult {
        format: "unified".to_string(),
        files,
        total_additions,
        total_deletions,
    })
}

/// Parse a hunk header line like "@@ -1,5 +1,7 @@ fn main()"
fn parse_hunk_header(line: &str) -> Option<DiffHunk> {
    // Find the range specs between @@ markers
    let after_at = line.strip_prefix("@@ ")?;
    let end_at = after_at.find(" @@")?;
    let range_part = &after_at[..end_at];
    let section_part = after_at.get(end_at + 3..).map(|s| s.trim().to_string());

    let mut parts = range_part.split_whitespace();

    let old_range = parts.next()?.strip_prefix('-')?;
    let new_range = parts.next()?.strip_prefix('+')?;

    let (old_start, old_count) = parse_range(old_range);
    let (new_start, new_count) = parse_range(new_range);

    Some(DiffHunk {
        old_start,
        old_count,
        new_start,
        new_count,
        section: section_part.filter(|s| !s.is_empty()),
        lines: Vec::new(),
    })
}

/// Parse "start,count" or just "start" (count defaults to 1).
fn parse_range(s: &str) -> (u64, u64) {
    if let Some((start, count)) = s.split_once(',') {
        (
            start.parse().unwrap_or(1),
            count.parse().unwrap_or(1),
        )
    } else {
        (s.parse().unwrap_or(1), 1)
    }
}

/// Run git diff and parse the output.
pub async fn run_diff(
    args: &[&str],
) -> Result<String, BridgeError> {
    let mut cmd_args = vec!["diff", "--no-ext-diff"];
    cmd_args.extend_from_slice(args);
    bridge_core::run_command("git", &cmd_args).await
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_DIFF: &str = r#"diff --git a/src/main.rs b/src/main.rs
index abc1234..def5678 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,5 +1,7 @@ fn main() {
     let x = 1;
-    let y = 2;
-    let z = 3;
+    let y = 20;
+    let z = 30;
+    let w = 40;
     println!("{}", x);
+    println!("{}", w);
 }
"#;

    #[test]
    fn parse_basic_diff() {
        let result = parse_unified_diff(SAMPLE_DIFF).unwrap();
        assert_eq!(result.format, "unified");
        assert_eq!(result.files.len(), 1);
        assert_eq!(result.files[0].old_path, "a/src/main.rs");
        assert_eq!(result.files[0].new_path, "b/src/main.rs");
    }

    #[test]
    fn counts_additions_deletions() {
        let result = parse_unified_diff(SAMPLE_DIFF).unwrap();
        assert_eq!(result.total_additions, 4);
        assert_eq!(result.total_deletions, 2);
    }

    #[test]
    fn parses_hunk_header() {
        let result = parse_unified_diff(SAMPLE_DIFF).unwrap();
        let hunk = &result.files[0].hunks[0];
        assert_eq!(hunk.old_start, 1);
        assert_eq!(hunk.old_count, 5);
        assert_eq!(hunk.new_start, 1);
        assert_eq!(hunk.new_count, 7);
        assert_eq!(hunk.section.as_deref(), Some("fn main() {"));
    }

    #[test]
    fn tracks_line_numbers() {
        let result = parse_unified_diff(SAMPLE_DIFF).unwrap();
        let lines = &result.files[0].hunks[0].lines;

        // First line is context at old:1, new:1
        assert_eq!(lines[0].kind, "context");
        assert_eq!(lines[0].old_line, Some(1));
        assert_eq!(lines[0].new_line, Some(1));

        // Second line is deletion at old:2
        assert_eq!(lines[1].kind, "delete");
        assert_eq!(lines[1].old_line, Some(2));
        assert_eq!(lines[1].new_line, None);

        // Fourth line is addition at new:2
        assert_eq!(lines[3].kind, "add");
        assert_eq!(lines[3].old_line, None);
        assert_eq!(lines[3].new_line, Some(2));
    }

    #[test]
    fn rejects_non_unified_format() {
        let stat_output = "diff --git a/file.rs b/file.rs\n file.rs | 5 ++---\n 1 file changed\n";
        let err = parse_unified_diff(stat_output).unwrap_err();
        assert_eq!(err.error, "format_not_unified");
        assert_eq!(err.format_detected, "stat");
    }

    #[test]
    fn rejects_binary_diff() {
        let binary = "Binary files a/image.png and b/image.png differ\n";
        let err = parse_unified_diff(binary).unwrap_err();
        assert_eq!(err.format_detected, "binary");
    }

    const MULTI_FILE_DIFF: &str = r#"diff --git a/a.rs b/a.rs
--- a/a.rs
+++ b/a.rs
@@ -1,3 +1,4 @@
 line1
+added
 line2
 line3
diff --git a/b.rs b/b.rs
--- a/b.rs
+++ b/b.rs
@@ -1,2 +1,2 @@
-old
+new
 same
"#;

    #[test]
    fn parses_multi_file_diff() {
        let result = parse_unified_diff(MULTI_FILE_DIFF).unwrap();
        assert_eq!(result.files.len(), 2);
        assert_eq!(result.files[0].new_path, "b/a.rs");
        assert_eq!(result.files[1].new_path, "b/b.rs");
        assert_eq!(result.total_additions, 2);
        assert_eq!(result.total_deletions, 1);
    }

    #[test]
    fn handles_no_count_in_range() {
        // Single-line hunk: @@ -1 +1 @@
        let diff = "--- a/f\n+++ b/f\n@@ -1 +1 @@\n-old\n+new\n";
        let result = parse_unified_diff(diff).unwrap();
        assert_eq!(result.files[0].hunks[0].old_count, 1);
        assert_eq!(result.files[0].hunks[0].new_count, 1);
    }
}
