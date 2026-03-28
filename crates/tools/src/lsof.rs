use bridge_core::BridgeError;
use serde::Serialize;

/// A process with its open file descriptors.
#[derive(Debug, Serialize, Clone)]
pub struct ProcessEntry {
    pub pid: u64,
    pub command: String,
    pub files: Vec<FileDescriptor>,
}

/// A single open file descriptor.
#[derive(Debug, Serialize, Clone)]
pub struct FileDescriptor {
    /// File descriptor number or special name (cwd, rtd, txt, mem, etc.)
    pub fd: String,
    /// File type: IPv4, IPv6, REG, DIR, CHR, FIFO, unix, unknown, etc.
    #[serde(rename = "type")]
    pub fd_type: String,
    /// Protocol (TCP, UDP) — only for network sockets.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protocol: Option<String>,
    /// File name, path, or network address.
    pub name: String,
}

/// Result of an lsof query.
#[derive(Debug, Serialize, Clone)]
pub struct LsofResult {
    pub processes: Vec<ProcessEntry>,
    /// Total open file descriptors across all processes.
    pub total_fds: u64,
}

/// Parse lsof -F output into structured entries.
/// The -F format uses single-character field codes at the start of each line:
/// p=PID, c=command, f=fd, t=type, P=protocol, n=name
pub fn parse_lsof_output(output: &str) -> LsofResult {
    let mut processes: Vec<ProcessEntry> = Vec::new();
    let mut current_process: Option<ProcessEntry> = None;
    let mut current_fd: Option<FileDescriptor> = None;

    for line in output.lines() {
        if line.is_empty() {
            continue;
        }

        let code = line.as_bytes()[0] as char;
        let value = &line[1..];

        match code {
            'p' => {
                // New process — save previous
                if let Some(mut proc) = current_process.take() {
                    if let Some(fd) = current_fd.take() {
                        proc.files.push(fd);
                    }
                    processes.push(proc);
                }
                current_process = Some(ProcessEntry {
                    pid: value.parse().unwrap_or(0),
                    command: String::new(),
                    files: Vec::new(),
                });
            }
            'c' => {
                if let Some(ref mut proc) = current_process {
                    proc.command = value.to_string();
                }
            }
            'f' => {
                // New fd — save previous
                if let Some(ref mut proc) = current_process {
                    if let Some(fd) = current_fd.take() {
                        proc.files.push(fd);
                    }
                }
                current_fd = Some(FileDescriptor {
                    fd: value.to_string(),
                    fd_type: String::new(),
                    protocol: None,
                    name: String::new(),
                });
            }
            't' => {
                if let Some(ref mut fd) = current_fd {
                    fd.fd_type = value.to_string();
                }
            }
            'P' => {
                if let Some(ref mut fd) = current_fd {
                    fd.protocol = Some(value.to_string());
                }
            }
            'n' => {
                if let Some(ref mut fd) = current_fd {
                    fd.name = value.to_string();
                }
            }
            _ => {
                // Unknown field code — skip (forward compatible)
            }
        }
    }

    // Push final entries
    if let Some(mut proc) = current_process.take() {
        if let Some(fd) = current_fd.take() {
            proc.files.push(fd);
        }
        processes.push(proc);
    }

    let total_fds: u64 = processes.iter().map(|p| p.files.len() as u64).sum();

    LsofResult {
        processes,
        total_fds,
    }
}

/// Run lsof with the given arguments, using -F for machine-readable output.
pub async fn run_lsof(args: &[&str]) -> Result<String, BridgeError> {
    let mut cmd_args = vec!["-F", "pcftnP"];
    cmd_args.extend_from_slice(args);
    bridge_core::run_command("lsof", &cmd_args).await
}

/// Detect lsof version at startup. Will be used for version-keyed field parsing.
#[allow(dead_code)]
pub async fn detect_version() -> Option<String> {
    let output = bridge_core::run_command("lsof", &["-v"]).await;
    // lsof -v writes to stderr and exits non-zero, so we handle both
    match output {
        Ok(stdout) => parse_version_string(&stdout),
        Err(BridgeError::CommandFailed { stderr, .. }) => parse_version_string(&stderr),
        _ => None,
    }
}

fn parse_version_string(output: &str) -> Option<String> {
    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("revision:") {
            return Some(trimmed.strip_prefix("revision:")?.trim().to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_OUTPUT: &str = "\
p1407
cmemora-server
f7
tIPv4
PTCP
n127.0.0.1:8766
f8
tIPv4
PTCP
n127.0.0.1:8766->127.0.0.1:47532
p4250
cvivaldi-bin
f25
tIPv4
PTCP
n192.168.0.105:43726->160.79.104.10:443
";

    #[test]
    fn parse_processes() {
        let result = parse_lsof_output(SAMPLE_OUTPUT);
        assert_eq!(result.processes.len(), 2);
        assert_eq!(result.processes[0].pid, 1407);
        assert_eq!(result.processes[0].command, "memora-server");
        assert_eq!(result.processes[1].pid, 4250);
        assert_eq!(result.processes[1].command, "vivaldi-bin");
    }

    #[test]
    fn parse_file_descriptors() {
        let result = parse_lsof_output(SAMPLE_OUTPUT);
        assert_eq!(result.processes[0].files.len(), 2);
        assert_eq!(result.processes[0].files[0].fd, "7");
        assert_eq!(result.processes[0].files[0].fd_type, "IPv4");
        assert_eq!(result.processes[0].files[0].protocol, Some("TCP".to_string()));
        assert_eq!(result.processes[0].files[0].name, "127.0.0.1:8766");
    }

    #[test]
    fn total_fd_count() {
        let result = parse_lsof_output(SAMPLE_OUTPUT);
        assert_eq!(result.total_fds, 3);
    }

    #[test]
    fn empty_input() {
        let result = parse_lsof_output("");
        assert_eq!(result.processes.len(), 0);
        assert_eq!(result.total_fds, 0);
    }

    #[test]
    fn parse_filesystem_fds() {
        let input = "\
p100
cbash
fcwd
tDIR
n/home/user
frtd
tDIR
n/
ftxt
tREG
n/usr/bin/bash
";
        let result = parse_lsof_output(input);
        assert_eq!(result.processes[0].files.len(), 3);
        assert_eq!(result.processes[0].files[0].fd, "cwd");
        assert_eq!(result.processes[0].files[0].fd_type, "DIR");
        assert_eq!(result.processes[0].files[2].fd, "txt");
    }

    #[test]
    fn version_string_parsing() {
        let output = "lsof version information:\n    revision: 4.99.4\n    copyright notice: foo\n";
        assert_eq!(parse_version_string(output), Some("4.99.4".to_string()));
    }

    #[test]
    fn unknown_fields_skipped() {
        let input = "p1\ncbash\nf0\ntREG\nXunknown_field\nn/dev/null\n";
        let result = parse_lsof_output(input);
        assert_eq!(result.processes[0].files[0].name, "/dev/null");
    }
}
