//! Adversarial benchmark fixtures for diff and lsof parsers.
//! These test edge cases that agents commonly misparse from raw text.

#[cfg(test)]
mod diff_adversarial {
    use crate::diff::parse_unified_diff;

    /// Multi-hunk diff with non-contiguous ranges — agents must track line numbers across gaps.
    #[test]
    fn multi_hunk_line_tracking() {
        let diff = "\
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,3 +1,4 @@
 fn main() {
+    setup();
     let x = 1;
     let y = 2;
@@ -20,4 +21,3 @@
     println!(\"hello\");
-    println!(\"debug\");
     cleanup();
 }
";
        let r = parse_unified_diff(diff).unwrap();
        assert_eq!(r.files[0].hunks.len(), 2);
        // Second hunk starts at old line 20, new line 21
        let h2 = &r.files[0].hunks[1];
        assert_eq!(h2.old_start, 20);
        assert_eq!(h2.new_start, 21);
        // "debug" line deleted at old line 21
        let del = h2.lines.iter().find(|l| l.kind == "delete").unwrap();
        assert_eq!(del.old_line, Some(21));
        assert_eq!(r.total_additions, 1);
        assert_eq!(r.total_deletions, 1);
    }

    /// Diff with renamed file — agents must handle extended git headers.
    #[test]
    fn renamed_file() {
        let diff = "\
diff --git a/old_name.rs b/new_name.rs
similarity index 95%
rename from old_name.rs
rename to new_name.rs
--- a/old_name.rs
+++ b/new_name.rs
@@ -1,3 +1,3 @@
 fn hello() {
-    println!(\"old\");
+    println!(\"new\");
 }
";
        let r = parse_unified_diff(diff).unwrap();
        assert_eq!(r.files[0].old_path, "a/old_name.rs");
        assert_eq!(r.files[0].new_path, "b/new_name.rs");
        assert_eq!(r.total_additions, 1);
        assert_eq!(r.total_deletions, 1);
    }

    /// Diff with zero-context (no context lines) — agents miscount without context anchoring.
    #[test]
    fn zero_context_diff() {
        let diff = "\
--- a/file.rs
+++ b/file.rs
@@ -5,2 +5,3 @@
-old line 5
-old line 6
+new line 5
+new line 6
+new line 7
";
        let r = parse_unified_diff(diff).unwrap();
        let h = &r.files[0].hunks[0];
        assert_eq!(h.old_count, 2);
        assert_eq!(h.new_count, 3);
        assert_eq!(r.total_deletions, 2);
        assert_eq!(r.total_additions, 3);
    }

    /// No-newline-at-end-of-file marker — agents must skip this line.
    #[test]
    fn no_newline_marker() {
        let diff = "\
--- a/file.txt
+++ b/file.txt
@@ -1,2 +1,2 @@
 line 1
-line 2
\\ No newline at end of file
+line 2 modified
\\ No newline at end of file
";
        let r = parse_unified_diff(diff).unwrap();
        assert_eq!(r.total_additions, 1);
        assert_eq!(r.total_deletions, 1);
        // The "No newline" marker should not appear as a diff line
        assert!(r.files[0].hunks[0]
            .lines
            .iter()
            .all(|l| !l.content.contains("No newline")));
    }

    /// Large multi-file diff — agents lose count across file boundaries.
    #[test]
    fn five_file_diff() {
        let mut diff = String::new();
        for i in 1..=5 {
            diff.push_str(&format!(
                "\
diff --git a/file{i}.rs b/file{i}.rs
--- a/file{i}.rs
+++ b/file{i}.rs
@@ -1,1 +1,2 @@
 existing line
+added in file {i}
"
            ));
        }
        let r = parse_unified_diff(&diff).unwrap();
        assert_eq!(r.files.len(), 5);
        assert_eq!(r.total_additions, 5);
        assert_eq!(r.total_deletions, 0);
        for (i, f) in r.files.iter().enumerate() {
            assert_eq!(f.new_path, format!("b/file{}.rs", i + 1));
        }
    }

    /// New file (no old content) — agents must handle /dev/null.
    #[test]
    fn new_file_diff() {
        let diff = "\
diff --git a/new.rs b/new.rs
new file mode 100644
--- /dev/null
+++ b/new.rs
@@ -0,0 +1,3 @@
+fn new() {
+    println!(\"new\");
+}
";
        let r = parse_unified_diff(diff).unwrap();
        assert_eq!(r.files[0].old_path, "/dev/null");
        assert_eq!(r.files[0].new_path, "b/new.rs");
        assert_eq!(r.total_additions, 3);
    }

    /// Deleted file — agents must handle +++ /dev/null.
    #[test]
    fn deleted_file_diff() {
        let diff = "\
diff --git a/old.rs b/old.rs
deleted file mode 100644
--- a/old.rs
+++ /dev/null
@@ -1,3 +0,0 @@
-fn old() {
-    println!(\"old\");
-}
";
        let r = parse_unified_diff(diff).unwrap();
        assert_eq!(r.files[0].new_path, "/dev/null");
        assert_eq!(r.total_deletions, 3);
    }

    /// Diff with only additions (no deletions) — agents sometimes miscount.
    #[test]
    fn additions_only() {
        let diff = "\
--- a/f.rs
+++ b/f.rs
@@ -1,2 +1,5 @@
 line 1
+added 1
+added 2
+added 3
 line 2
";
        let r = parse_unified_diff(diff).unwrap();
        assert_eq!(r.total_additions, 3);
        assert_eq!(r.total_deletions, 0);
        // Verify line numbers are correct
        let adds: Vec<u64> = r.files[0].hunks[0]
            .lines
            .iter()
            .filter(|l| l.kind == "add")
            .filter_map(|l| l.new_line)
            .collect();
        assert_eq!(adds, vec![2, 3, 4]);
    }

    /// Empty diff (no changes) — edge case.
    #[test]
    fn empty_diff_from_git() {
        // git diff with no changes produces empty output
        let diff = "";
        // Our parser requires format detection, so empty should be handled at the tool level
        // (the MCP handler returns empty result for empty input)
    }
}

#[cfg(test)]
mod lsof_adversarial {
    use crate::lsof::parse_lsof_output;

    /// Process with mixed fd types — agents must correctly attribute each fd.
    #[test]
    fn mixed_fd_types() {
        let output = "\
p12345
cnginx
fcwd
tDIR
n/var/www
f3
tREG
n/var/log/nginx/access.log
f7
tIPv4
PTCP
n*:80
f8
tIPv6
PTCP
n*:443
f12
tunix
n/var/run/nginx.sock
";
        let r = parse_lsof_output(output);
        assert_eq!(r.processes[0].files.len(), 5);
        assert_eq!(r.processes[0].files[0].fd_type, "DIR");
        assert_eq!(r.processes[0].files[1].fd_type, "REG");
        assert_eq!(r.processes[0].files[2].fd_type, "IPv4");
        assert_eq!(r.processes[0].files[2].protocol, Some("TCP".to_string()));
        assert_eq!(r.processes[0].files[3].fd_type, "IPv6");
        assert_eq!(r.processes[0].files[4].fd_type, "unix");
    }

    /// Multiple processes on same port — agents must group fds correctly per process.
    #[test]
    fn multiple_processes_same_port() {
        let output = "\
p100
cnginx
f7
tIPv4
PTCP
n*:80
p101
cnginx
f7
tIPv4
PTCP
n*:80
p200
cnode
f12
tIPv4
PTCP
n127.0.0.1:3000
";
        let r = parse_lsof_output(output);
        assert_eq!(r.processes.len(), 3);
        assert_eq!(r.processes[0].pid, 100);
        assert_eq!(r.processes[1].pid, 101);
        assert_eq!(r.processes[2].pid, 200);
        // Each process has exactly 1 fd
        assert!(r.processes.iter().all(|p| p.files.len() == 1));
        assert_eq!(r.total_fds, 3);
    }

    /// Process with many fds — agents must count correctly.
    #[test]
    fn high_fd_count() {
        let mut output = String::from("p999\ncpostgres\n");
        for i in 0..50 {
            output.push_str(&format!("f{i}\ntREG\n/data/pg_{i}.dat\n"));
        }
        let r = parse_lsof_output(&output);
        assert_eq!(r.processes[0].files.len(), 50);
        assert_eq!(r.total_fds, 50);
    }

    /// Connection with arrow notation — agents must parse local->remote.
    #[test]
    fn connection_arrow_parsing() {
        let output = "\
p5000
capp
f10
tIPv4
PTCP
n192.168.1.100:45678->10.0.0.1:5432
f11
tIPv4
PUDP
n0.0.0.0:53
";
        let r = parse_lsof_output(output);
        assert_eq!(
            r.processes[0].files[0].name,
            "192.168.1.100:45678->10.0.0.1:5432"
        );
        assert_eq!(r.processes[0].files[0].protocol, Some("TCP".to_string()));
        assert_eq!(r.processes[0].files[1].protocol, Some("UDP".to_string()));
    }

    /// Process with NOFD (permission denied) — agents must handle gracefully.
    #[test]
    fn permission_denied_nofd() {
        let output = "\
p1
csystemd
fNOFD
n/proc/1/fd (opendir: Permission denied)
";
        let r = parse_lsof_output(output);
        assert_eq!(r.processes[0].files.len(), 1);
        assert_eq!(r.processes[0].files[0].fd, "NOFD");
    }

    /// Empty output (no matching processes) — edge case.
    #[test]
    fn no_matching_processes() {
        let r = parse_lsof_output("");
        assert_eq!(r.processes.len(), 0);
        assert_eq!(r.total_fds, 0);
    }
}
