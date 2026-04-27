# MCP Tool Bridge

20-tool Rust MCP server: ls, wc, diff, lsof, find, curl, git (status/log/show), gh_api, ps, kubectl (list/get), docker (list/inspect/images), sqlite (query/tables), batch, pipe. See `docs/README.md` and `.tribunal/tribunal-report.md`.

## Status

v0.1 complete — 20 tools, 103 unit tests, 1740 integration tests. PreToolUse hook (`crates/hook/`, 25 unit + 11 e2e tests) and Claude Code plugin (`plugin/`) bundled. CI green on ubuntu+macos. Pushed to https://github.com/Hibryda/mcp-tool-bridge.

## Tech Stack

- Rust (2021 edition), Cargo workspace
- rmcp 1.3.0 (official MCP SDK — server, transport-io, macros, schemars features)
- bollard (Docker Engine API), rusqlite (bundled SQLite)
- tokio, serde / serde_json / schemars 1.0, chrono
- thiserror 2, anyhow, tracing

## Architecture

Three crates in a Cargo workspace:
- `bridge-core` — shared types (BridgeError, FileEntry, WcResult, run_command).
- `tools` — MCP server (`mcp-tool-bridge` binary). Dispatch layer: free functions in `dispatch.rs` shared by rmcp `tool_router` and batch `HashMap`. `--tools` flag filters registration at startup.
- `hook` — PreToolUse hook (`mcp-tool-bridge-hook` binary). Reads JSON on stdin, parses Bash commands via `shell-words`, suggests/blocks via `MCP_BRIDGE_HOOK_MODE`.

The Claude Code plugin lives in `plugin/` and references both binaries via `${CLAUDE_PLUGIN_ROOT}/../target/release/`.

## Documentation (SOURCE OF TRUTH)

**All project documentation lives in [`docs/`](docs/README.md).**

## Development

```bash
cargo build                          # dev build
cargo build --release                # release (server + hook binaries)
cargo test --workspace               # 103 unit + 187 e2e + 25 hook unit + 11 hook e2e + 5 doc
cargo run -p mcp-tool-bridge         # run server with all tools
cargo run -p mcp-tool-bridge -- --tools ls,wc,diff  # selective
cargo test -p mcp-tool-bridge-hook   # hook tests only
```
