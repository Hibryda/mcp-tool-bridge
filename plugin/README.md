# mcp-tool-bridge plugin

Claude Code plugin that bundles:

1. **MCP server** (`mcp-tool-bridge`) — 20 structured-output tools.
2. **PreToolUse hook** (`mcp-tool-bridge-hook`) — nudges the agent to call the
   MCP tool instead of plain Bash when the command is covered.

## Build

The plugin uses release binaries built from this repo. From the repo root:

```bash
cargo build --release
```

This produces `target/release/mcp-tool-bridge` and `target/release/mcp-tool-bridge-hook`,
which the plugin references via `${CLAUDE_PLUGIN_ROOT}/../target/release/...`.

## Hook modes

Set `MCP_BRIDGE_HOOK_MODE` in your environment:

| Mode | Behaviour |
|------|-----------|
| `suggest` (default) | Adds an `additionalContext` hint to the agent. Never blocks. |
| `enforce` | Blocks the Bash call with exit code 2; agent sees the hint. |
| `off` | Hook is a no-op. |

## What's covered

The hook recognises these commands and points the agent at the structured equivalent:

| Bash command | MCP tool |
|--------------|----------|
| `ls`, `ls -la`, `ls /path` | `mcp__tool-bridge__ls` |
| `wc`, `wc -l`, `wc -lwc files…` | `mcp__tool-bridge__wc` |
| `find <path> -name -type -maxdepth -size` | `mcp__tool-bridge__find` |
| `diff -u a b` (parser hint) | `mcp__tool-bridge__diff` |
| `lsof -i`, `-iTCP:port`, `-p PID` | `mcp__tool-bridge__lsof` |
| `ps`, `ps aux`, `ps -ef` | `mcp__tool-bridge__ps` |
| `git status`, `git status --porcelain` | `mcp__tool-bridge__git_status` |
| `git log` (default form, not `--oneline`) | `mcp__tool-bridge__git_log` |
| `git show <ref>` | `mcp__tool-bridge__git_show` |

Anything else — pipelines, redirections, `&&` chains, `cargo`, `npm`, `kubectl`,
unrecognised flags — passes through silently. The parser is conservative by design.

## Install (local plugin)

In Claude Code:

```
/plugin install file:///home/hibryda/code/ai/mcp-tool-bridge/plugin
```

Or add to your settings:

```json
{
  "extensions": [
    {"path": "/home/hibryda/code/ai/mcp-tool-bridge/plugin"}
  ]
}
```
