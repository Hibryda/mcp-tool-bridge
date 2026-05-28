# MCP Tool Bridge

20-tool Rust MCP server: ls, wc, diff, lsof, find, curl, git (status/log/show), gh_api, ps, kubectl (list/get), docker (list/inspect/images), sqlite (query/tables), batch, pipe. See `docs/README.md` and `.tribunal/tribunal-report.md`.

## Status

v0.1.0 released 2026-05-05 — 20 tools, 103 unit tests, 1740 integration tests. PreToolUse hook (`crates/hook/`, 25 unit + 11 e2e tests) and Claude Code plugin (`plugin/`) bundled. Repo doubles as a marketplace (`.claude-plugin/marketplace.json`) with a 4-arch release pipeline; `plugin/bin/launcher.sh` handles checksum-verified binary download per host. CI green on ubuntu+macos. Installed + verified working at https://github.com/Hibryda/mcp-tool-bridge.

**Adoption:** the PreToolUse suggest-hook is a no-op (Claude Code does not inject `additionalContext` on PreToolUse — measured ~1.6% tool adoption). Driving adoption via context rules in `plugin/rules/` instead. Next: composite tools `repo_snapshot` + `pr_status` (read-only), then gated mutating tools. See Memora `composite-tool-roadmap`, `hook-suggest-mode-noop`.

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

The Claude Code plugin lives in `plugin/`. `plugin/bin/launcher.sh` (POSIX) is the single entry point both the MCP server and the hook are wired to via the manifests; it detects the host triple, downloads the matching tarball from `${MCP_TOOL_BRIDGE_RELEASE_BASE_URL}/v<version>/`, verifies SHA-256 against `plugin/checksums.txt`, and caches under `${CLAUDE_PLUGIN_ROOT}/.bin-cache/`. `MCP_TOOL_BRIDGE_BIN_DIR` env override skips the download path for local development.

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
