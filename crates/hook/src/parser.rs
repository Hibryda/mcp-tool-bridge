//! Conservative Bash command classifier. Only recognises forms we are confident
//! the MCP can serve — anything ambiguous is passed through (returns `None`),
//! never rewritten into something semantically different.
//!
//! Layer 1: refuse compound shells (pipes, redirections, command substitution)
//!          → no suggestion, since the MCP equivalents only cover single commands.
//! Layer 2: shell-words tokenize.
//! Layer 3: per-command match against our 7 covered tools.

const COMPOUND_MARKERS: &[&str] = &["|", "&&", "||", ";", ">", "<", "$(", "`", "\n"];

/// Returns a suggestion string if the command maps cleanly to an MCP tool.
/// Returns `None` when the command is unmappable, ambiguous, or covered by the
/// pass-through rules below.
pub fn analyze(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    // Compound shells: bail out. The MCP can't replace pipelines or redirections.
    for marker in COMPOUND_MARKERS {
        if trimmed.contains(marker) {
            return None;
        }
    }

    let tokens = shell_words::split(trimmed).ok()?;
    if tokens.is_empty() {
        return None;
    }

    let cmd = tokens[0].as_str();
    let args = &tokens[1..];

    match cmd {
        "ls" => suggest_ls(args),
        "wc" => suggest_wc(args),
        "find" => suggest_find(args),
        "diff" => suggest_diff(args),
        "lsof" => suggest_lsof(args),
        "ps" => suggest_ps(args),
        "git" => suggest_git(args),
        _ => None,
    }
}

// ── individual suggesters ───────────────────────────────────────────────

fn suggest_ls(args: &[String]) -> Option<String> {
    // The MCP `ls` covers metadata-rich listings. Plain `ls`, `ls -l`, `ls -la`,
    // `ls <path>`, `ls -la <path>` all map cleanly. Refuse exotic flags.
    let allowed_flags = ["-l", "-a", "-la", "-al", "-h", "-lh", "-lah", "-laH"];
    let mut path: Option<&str> = None;
    for a in args {
        if a.starts_with('-') {
            if !allowed_flags.contains(&a.as_str()) {
                return None;
            }
        } else if path.is_some() {
            // Multiple paths — MCP `ls` takes one.
            return None;
        } else {
            path = Some(a);
        }
    }
    Some(format!(
        "Consider `mcp__tool-bridge__ls` instead of `ls`{}. Returns structured FileEntry[] with size/perms/mtime in one call — no parsing needed.",
        match path {
            Some(p) => format!(" {}", p),
            None => String::new(),
        }
    ))
}

fn suggest_wc(args: &[String]) -> Option<String> {
    // `wc`, `wc -l`, `wc -w`, `wc -c`, `wc <file>`, `wc -l <file>` — all fine.
    // Multi-file forms fine too (MCP supports paths array).
    for a in args {
        if let Some(rest) = a.strip_prefix('-') {
            // Conservative: only single-letter combinations of l/w/c/m.
            if rest.is_empty() || !rest.chars().all(|c| matches!(c, 'l' | 'w' | 'c' | 'm')) {
                return None;
            }
        }
    }
    Some(
        "Consider `mcp__tool-bridge__wc` instead of `wc`. Returns {lines, words, bytes, chars} per path."
            .to_string(),
    )
}

fn suggest_find(args: &[String]) -> Option<String> {
    // `find <path> [-name PAT] [-type f|d] [-maxdepth N]`. Refuse `-exec`, `-print0`, etc.
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        match a.as_str() {
            "-name" | "-iname" | "-type" | "-maxdepth" | "-mindepth" | "-size" => {
                i += 2;
                if i > args.len() {
                    return None;
                }
            }
            "-exec" | "-execdir" | "-print0" | "-delete" | "-prune" | "-ok" | "-okdir" => {
                return None;
            }
            other if other.starts_with('-') => {
                // Unknown flag — bail, don't risk a wrong rewrite.
                return None;
            }
            _ => {
                i += 1;
            }
        }
    }
    Some(
        "Consider `mcp__tool-bridge__find` instead of `find`. Recursive search with name globs, type/size/depth filters, structured FileEntry[] output."
            .to_string(),
    )
}

fn suggest_diff(args: &[String]) -> Option<String> {
    // The MCP `diff` parses unified-diff TEXT (it's a parser, not a runner).
    // So a literal `diff a b` doesn't replace anything — but `diff -u a b`
    // produces input the parser consumes. Suggest the *pipeline* hint.
    let has_unified = args.iter().any(|a| a == "-u" || a.starts_with("-u"));
    if has_unified {
        Some(
            "Note: `mcp__tool-bridge__diff` parses unified diff text into typed hunks with line numbers. After running `diff -u`, feed the output to it instead of regex-parsing."
                .to_string(),
        )
    } else {
        None
    }
}

fn suggest_lsof(args: &[String]) -> Option<String> {
    // The MCP `lsof` covers `-i` (network), `-iTCP`, `-iTCP:port`, `-p PID`, `-c COMMAND`.
    // Refuse anything else.
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if a == "-p" || a == "-c" || a == "-u" {
            i += 2;
            if i > args.len() {
                return None;
            }
            continue;
        }
        if a.starts_with("-i") || a == "-n" || a == "-P" || a == "-F" {
            i += 1;
            continue;
        }
        if a.starts_with('-') || a.starts_with('+') {
            return None;
        }
        i += 1;
    }
    Some(
        "Consider `mcp__tool-bridge__lsof` instead of `lsof`. Returns {processes:[{pid, command, files:[{fd, type, protocol, name}]}]} — no -F field-letter parsing."
            .to_string(),
    )
}

fn suggest_ps(args: &[String]) -> Option<String> {
    // `ps`, `ps aux`, `ps -ef`, `ps -p PID`. Conservative.
    for a in args {
        match a.as_str() {
            "aux" | "-ef" | "-e" | "-f" | "-A" => continue,
            "-p" | "-u" | "-C" => continue, // followed by an arg; we don't strictly index here
            other if other.starts_with('-') => return None,
            _ => {}
        }
    }
    Some(
        "Consider `mcp__tool-bridge__ps` instead of `ps`. Returns typed processes with PID/user/CPU%/memory/args, supports name/user/PID filters."
            .to_string(),
    )
}

fn suggest_git(args: &[String]) -> Option<String> {
    if args.is_empty() {
        return None;
    }
    let sub = args[0].as_str();
    let rest = &args[1..];
    match sub {
        "status" => suggest_git_status(rest),
        "log" => suggest_git_log(rest),
        "show" => suggest_git_show(rest),
        _ => None,
    }
}

fn suggest_git_status(args: &[String]) -> Option<String> {
    // `git status`, `git status -s`, `git status --short`, `git status --porcelain` — MCP equivalent.
    for a in args {
        match a.as_str() {
            "-s" | "--short" | "--porcelain" | "-b" | "--branch" => continue,
            other if other.starts_with('-') => return None,
            _ => return None, // No path filtering supported by the MCP equivalent.
        }
    }
    Some(
        "Consider `mcp__tool-bridge__git_status` instead of `git status`. Branch ahead/behind + typed file entries via porcelain=v2; typed errors for NOT_A_REPO / DETACHED_HEAD."
            .to_string(),
    )
}

fn suggest_git_log(args: &[String]) -> Option<String> {
    // The MCP returns structured commits. `git log --oneline` has a different output
    // shape so still useful — pass through. For default / -n / --numstat forms, suggest.
    for a in args {
        match a.as_str() {
            "--oneline" | "--graph" | "--pretty" => return None,
            s if s.starts_with("--pretty=") || s.starts_with("--format=") => return None,
            _ => {}
        }
    }
    Some(
        "Consider `mcp__tool-bridge__git_log` instead of `git log`. STX/ETX-sentinel parsed commits with stable snapshot_oid pagination; optional --numstat stats."
            .to_string(),
    )
}

fn suggest_git_show(args: &[String]) -> Option<String> {
    // Only suggest for plain `git show <ref>` — refuse format overrides.
    for a in args {
        match a.as_str() {
            s if s.starts_with("--pretty=") || s.starts_with("--format=") => return None,
            "--stat" | "--numstat" | "--name-only" | "--name-status" => continue,
            other if other.starts_with('-') => return None,
            _ => {}
        }
    }
    Some(
        "Consider `mcp__tool-bridge__git_show` instead of `git show`. Restricted to commit objects via cat-file preflight; typed NOT_A_COMMIT error otherwise."
            .to_string(),
    )
}

// ── tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_suggested(cmd: &str, contains: &str) {
        let s = analyze(cmd).unwrap_or_else(|| panic!("expected suggestion for {:?}", cmd));
        assert!(
            s.contains(contains),
            "suggestion for {:?} = {:?} did not contain {:?}",
            cmd,
            s,
            contains
        );
    }

    fn assert_passthrough(cmd: &str) {
        assert!(
            analyze(cmd).is_none(),
            "expected no suggestion for {:?}",
            cmd
        );
    }

    // ── compound shells ───────────────────────────────────────────────

    #[test]
    fn pipes_pass_through() {
        assert_passthrough("ls | wc -l");
        assert_passthrough("ls -la | head");
        assert_passthrough("find . | xargs grep foo");
    }

    #[test]
    fn redirections_pass_through() {
        assert_passthrough("ls > out.txt");
        assert_passthrough("wc -l < input");
    }

    #[test]
    fn boolean_chains_pass_through() {
        assert_passthrough("ls && echo done");
        assert_passthrough("ls; echo done");
        assert_passthrough("ls || echo failed");
    }

    #[test]
    fn command_substitution_pass_through() {
        assert_passthrough("ls $(pwd)");
        assert_passthrough("ls `pwd`");
    }

    // ── ls ────────────────────────────────────────────────────────────

    #[test]
    fn ls_basic_suggested() {
        assert_suggested("ls", "mcp__tool-bridge__ls");
        assert_suggested("ls -la", "mcp__tool-bridge__ls");
        assert_suggested("ls -la /tmp", "mcp__tool-bridge__ls");
        assert_suggested("ls /var/log", "/var/log");
    }

    #[test]
    fn ls_exotic_flags_pass_through() {
        assert_passthrough("ls --color=always");
        assert_passthrough("ls -R");
        assert_passthrough("ls -lt"); // not in allow-list
    }

    #[test]
    fn ls_multiple_paths_pass_through() {
        assert_passthrough("ls /tmp /var");
    }

    // ── wc ────────────────────────────────────────────────────────────

    #[test]
    fn wc_suggested() {
        assert_suggested("wc -l file.txt", "mcp__tool-bridge__wc");
        assert_suggested("wc -lwc file.txt", "mcp__tool-bridge__wc");
        assert_suggested("wc file1 file2", "mcp__tool-bridge__wc");
    }

    #[test]
    fn wc_unknown_flag_pass_through() {
        assert_passthrough("wc --files0-from=list");
        assert_passthrough("wc -L file.txt");
    }

    // ── find ──────────────────────────────────────────────────────────

    #[test]
    fn find_basic_suggested() {
        assert_suggested("find . -name '*.rs'", "mcp__tool-bridge__find");
        assert_suggested("find /tmp -type f -maxdepth 2", "mcp__tool-bridge__find");
    }

    #[test]
    fn find_exec_pass_through() {
        assert_passthrough("find . -name '*.rs' -exec wc -l {} +");
        assert_passthrough("find . -delete");
    }

    // ── diff ──────────────────────────────────────────────────────────

    #[test]
    fn diff_unified_suggests_pipeline_hint() {
        assert_suggested("diff -u a.txt b.txt", "parses unified diff text");
    }

    #[test]
    fn diff_default_pass_through() {
        // Without -u, the MCP can't help (it parses unified format).
        assert_passthrough("diff a.txt b.txt");
    }

    // ── lsof ──────────────────────────────────────────────────────────

    #[test]
    fn lsof_suggested() {
        assert_suggested("lsof -i", "mcp__tool-bridge__lsof");
        assert_suggested("lsof -iTCP:8080", "mcp__tool-bridge__lsof");
        assert_suggested("lsof -p 1234", "mcp__tool-bridge__lsof");
    }

    #[test]
    fn lsof_unknown_flag_pass_through() {
        assert_passthrough("lsof +D /tmp");
    }

    // ── ps ────────────────────────────────────────────────────────────

    #[test]
    fn ps_suggested() {
        assert_suggested("ps", "mcp__tool-bridge__ps");
        assert_suggested("ps aux", "mcp__tool-bridge__ps");
        assert_suggested("ps -ef", "mcp__tool-bridge__ps");
    }

    #[test]
    fn ps_unknown_flag_pass_through() {
        assert_passthrough("ps --forest");
    }

    // ── git ───────────────────────────────────────────────────────────

    #[test]
    fn git_status_suggested() {
        assert_suggested("git status", "mcp__tool-bridge__git_status");
        assert_suggested("git status --short", "mcp__tool-bridge__git_status");
        assert_suggested("git status --porcelain", "mcp__tool-bridge__git_status");
    }

    #[test]
    fn git_status_path_filter_pass_through() {
        assert_passthrough("git status src/");
    }

    #[test]
    fn git_log_default_suggested() {
        assert_suggested("git log", "mcp__tool-bridge__git_log");
        assert_suggested("git log -n 5", "mcp__tool-bridge__git_log");
    }

    #[test]
    fn git_log_oneline_pass_through() {
        // --oneline output shape is materially different — let it through.
        assert_passthrough("git log --oneline -20");
        assert_passthrough("git log --pretty=format:%H");
    }

    #[test]
    fn git_show_suggested() {
        assert_suggested("git show HEAD", "mcp__tool-bridge__git_show");
        assert_suggested("git show abc123", "mcp__tool-bridge__git_show");
    }

    #[test]
    fn git_show_format_override_pass_through() {
        assert_passthrough("git show --pretty=format:%H HEAD");
    }

    #[test]
    fn git_unknown_subcommand_pass_through() {
        assert_passthrough("git diff");
        assert_passthrough("git commit");
        assert_passthrough("git push");
    }

    // ── unrecognised commands ─────────────────────────────────────────

    #[test]
    fn unrelated_commands_pass_through() {
        assert_passthrough("echo hello");
        assert_passthrough("cargo build");
        assert_passthrough("kubectl get pods");
        assert_passthrough("docker ps");
    }
}
