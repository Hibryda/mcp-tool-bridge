//! PreToolUse hook for Claude Code: when the agent runs `Bash` with a command we
//! cover via the MCP server, emit a suggestion (suggest mode) or block the call
//! (enforce mode), pointing the agent at the structured equivalent.
//!
//! Modes (env `MCP_BRIDGE_HOOK_MODE`): `suggest` (default), `enforce`, `off`.
//!
//! Suggest mode: exit 0, JSON on stdout with `hookSpecificOutput.additionalContext`.
//! Enforce mode: exit 2, message on stderr (Claude Code feeds stderr back to the agent).
//! Off / unrecognised tool / un-coverable command: exit 0 silently.

use serde::Deserialize;
use std::io::{self, Read, Write};
use std::process::ExitCode;

mod parser;

#[derive(Debug, Deserialize)]
struct HookInput {
    #[serde(default)]
    tool_name: String,
    #[serde(default)]
    tool_input: ToolInput,
}

#[derive(Debug, Default, Deserialize)]
struct ToolInput {
    #[serde(default)]
    command: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Suggest,
    Enforce,
    Off,
}

impl Mode {
    fn from_env() -> Self {
        match std::env::var("MCP_BRIDGE_HOOK_MODE")
            .unwrap_or_default()
            .to_lowercase()
            .as_str()
        {
            "off" | "disabled" => Mode::Off,
            "enforce" | "block" => Mode::Enforce,
            // Empty / unrecognised / explicit "suggest" → suggest.
            _ => Mode::Suggest,
        }
    }
}

fn main() -> ExitCode {
    let mode = Mode::from_env();
    if mode == Mode::Off {
        return ExitCode::SUCCESS;
    }

    let mut buf = String::new();
    if io::stdin().read_to_string(&mut buf).is_err() || buf.trim().is_empty() {
        return ExitCode::SUCCESS;
    }

    let input: HookInput = match serde_json::from_str(&buf) {
        Ok(v) => v,
        Err(_) => return ExitCode::SUCCESS,
    };

    if input.tool_name != "Bash" || input.tool_input.command.is_empty() {
        return ExitCode::SUCCESS;
    }

    let suggestion = match parser::analyze(&input.tool_input.command) {
        Some(s) => s,
        None => return ExitCode::SUCCESS,
    };

    match mode {
        Mode::Off => ExitCode::SUCCESS,
        Mode::Suggest => emit_suggest(&suggestion),
        Mode::Enforce => emit_enforce(&suggestion),
    }
}

fn emit_suggest(suggestion: &str) -> ExitCode {
    let payload = serde_json::json!({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "additionalContext": suggestion,
        }
    });
    let _ = writeln!(io::stdout(), "{}", payload);
    ExitCode::SUCCESS
}

fn emit_enforce(suggestion: &str) -> ExitCode {
    let _ = writeln!(io::stderr(), "{}", suggestion);
    // Exit code 2 = blocking error; Claude Code feeds stderr back to the agent.
    ExitCode::from(2)
}
