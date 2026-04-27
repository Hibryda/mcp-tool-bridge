//! ps — structured process listing with CPU/memory info.

use serde::Serialize;

/// A single process entry.
#[derive(Debug, Serialize, Clone)]
pub struct ProcessInfo {
    pub pid: u64,
    pub ppid: u64,
    pub user: String,
    pub command: String,
    pub args: String,
    pub cpu_percent: f64,
    pub mem_rss_kb: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elapsed_seconds: Option<u64>,
}

/// Process listing result.
#[derive(Debug, Serialize, Clone)]
pub struct PsResult {
    pub processes: Vec<ProcessInfo>,
    pub count: u64,
    pub total_before_filter: u64,
}

/// List processes with optional filters.
pub async fn list_processes(
    name_pattern: Option<&str>,
    user: Option<&str>,
    pid_list: Option<&[u64]>,
    max_results: usize,
) -> Result<PsResult, String> {
    let output = tokio::process::Command::new("ps")
        .args([
            "-eo",
            "pid,ppid,user,comm,args,pcpu,rss,etimes",
            "--no-headers",
        ])
        .output()
        .await
        .map_err(|e| format!("ps failed: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // macOS doesn't support etimes — fallback
        if stderr.contains("etimes") || stderr.contains("unknown") {
            return list_processes_macos(name_pattern, user, pid_list, max_results).await;
        }
        return Err(format!("ps failed: {}", stderr.trim()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_ps_output(&stdout, name_pattern, user, pid_list, max_results)
}

/// macOS fallback (no etimes support).
async fn list_processes_macos(
    name_pattern: Option<&str>,
    user: Option<&str>,
    pid_list: Option<&[u64]>,
    max_results: usize,
) -> Result<PsResult, String> {
    let output = tokio::process::Command::new("ps")
        .args(["-eo", "pid,ppid,user,comm,args,pcpu,rss", "-r"])
        .output()
        .await
        .map_err(|e| format!("ps failed: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "ps failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Skip header line on macOS
    let body = stdout.lines().skip(1).collect::<Vec<_>>().join("\n");
    parse_ps_output(&body, name_pattern, user, pid_list, max_results)
}

/// Parse ps output into structured entries.
fn parse_ps_output(
    output: &str,
    name_pattern: Option<&str>,
    user_filter: Option<&str>,
    pid_list: Option<&[u64]>,
    max_results: usize,
) -> Result<PsResult, String> {
    let mut all_procs = Vec::new();

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Parse whitespace-separated fields. The tricky part is that 'args' can contain spaces.
        // Format: pid ppid user comm args pcpu rss [etimes]
        // We parse from left (fixed fields) and right (numeric fields).
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 7 {
            continue;
        }

        let pid: u64 = match parts[0].parse() {
            Ok(v) => v,
            Err(_) => continue,
        };
        let ppid: u64 = parts[1].parse().unwrap_or(0);
        let user = parts[2].to_string();
        let comm = parts[3].to_string();

        // Parse from the right: etimes (optional), rss, pcpu
        let has_etimes =
            parts.len() >= 8 && parts.last().and_then(|s| s.parse::<u64>().ok()).is_some();

        let (elapsed_seconds, rss_idx, pcpu_idx) = if has_etimes {
            let etimes: Option<u64> = parts.last().and_then(|s| s.parse().ok());
            (etimes, parts.len() - 2, parts.len() - 3)
        } else {
            (None, parts.len() - 1, parts.len() - 2)
        };

        let mem_rss_kb: u64 = parts.get(rss_idx).and_then(|s| s.parse().ok()).unwrap_or(0);
        let cpu_percent: f64 = parts
            .get(pcpu_idx)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0);

        // Everything between comm and pcpu is args
        let args_end = pcpu_idx;
        let args = if args_end > 4 {
            parts[4..args_end].join(" ")
        } else {
            comm.clone()
        };

        all_procs.push(ProcessInfo {
            pid,
            ppid,
            user,
            command: comm,
            args,
            cpu_percent,
            mem_rss_kb,
            elapsed_seconds,
        });
    }

    let total_before = all_procs.len() as u64;

    // Apply filters
    let filtered: Vec<ProcessInfo> = all_procs
        .into_iter()
        .filter(|p| {
            if let Some(pat) = name_pattern {
                if !p.command.contains(pat) && !p.args.contains(pat) {
                    return false;
                }
            }
            if let Some(u) = user_filter {
                if p.user != u {
                    return false;
                }
            }
            if let Some(pids) = pid_list {
                if !pids.contains(&p.pid) {
                    return false;
                }
            }
            true
        })
        .take(max_results)
        .collect();

    let count = filtered.len() as u64;

    Ok(PsResult {
        processes: filtered,
        count,
        total_before_filter: total_before,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
 1234     1 root     systemd  /sbin/init  0.1 12340 98765
 5678  1234 user     bash     /bin/bash   1.5  5432 54321
 9012  5678 user     cargo    cargo build 25.3 98765 12345
";

    #[test]
    fn parse_processes() {
        let r = parse_ps_output(SAMPLE, None, None, None, 100).unwrap();
        assert_eq!(r.count, 3);
        assert_eq!(r.processes[0].pid, 1234);
        assert_eq!(r.processes[0].command, "systemd");
        assert_eq!(r.processes[2].command, "cargo");
    }

    #[test]
    fn filter_by_name() {
        let r = parse_ps_output(SAMPLE, Some("cargo"), None, None, 100).unwrap();
        assert_eq!(r.count, 1);
        assert_eq!(r.processes[0].pid, 9012);
        assert_eq!(r.total_before_filter, 3);
    }

    #[test]
    fn filter_by_user() {
        let r = parse_ps_output(SAMPLE, None, Some("user"), None, 100).unwrap();
        assert_eq!(r.count, 2);
    }

    #[test]
    fn filter_by_pid() {
        let r = parse_ps_output(SAMPLE, None, None, Some(&[1234, 9012]), 100).unwrap();
        assert_eq!(r.count, 2);
    }

    #[test]
    fn max_results() {
        let r = parse_ps_output(SAMPLE, None, None, None, 2).unwrap();
        assert_eq!(r.count, 2);
    }

    #[test]
    fn parse_cpu_mem() {
        let r = parse_ps_output(SAMPLE, None, None, None, 100).unwrap();
        assert!(r.processes[2].cpu_percent > 25.0);
        assert!(r.processes[2].mem_rss_kb > 0);
    }

    #[test]
    fn elapsed_seconds() {
        let r = parse_ps_output(SAMPLE, None, None, None, 100).unwrap();
        assert_eq!(r.processes[0].elapsed_seconds, Some(98765));
    }
}
