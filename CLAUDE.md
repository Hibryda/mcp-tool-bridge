# MCP Tool Bridge

15-tool Rust MCP server: ls, wc, diff, lsof, find, kubectl (list/get), docker (list/inspect/images), sqlite (query/tables), batch, pipe, curl. See `docs/README.md` and `.tribunal/tribunal-report.md`.

## Status

v0.1 complete — 15 tools, 74 unit tests, 740 integration tests. Registered globally in Claude Code. Remaining: CI, push to GitHub.

## Tech Stack

- Rust (2021 edition), Cargo workspace
- rmcp 1.3.0 (official MCP SDK — server, transport-io, macros, schemars features)
- bollard (Docker Engine API), rusqlite (bundled SQLite)
- tokio, serde / serde_json / schemars 1.0, chrono
- thiserror 2, anyhow, tracing

## Architecture

Single binary with dispatch layer: free functions in `dispatch.rs` shared by rmcp `tool_router` and batch `HashMap`. `--tools` flag filters registration at startup.

## Documentation (SOURCE OF TRUTH)

**All project documentation lives in [`docs/`](docs/README.md).**

## Development

```bash
cargo build                          # dev build
cargo build --release                # release (~10MB binary)
cargo test                           # 74 unit tests
cargo run -p mcp-tool-bridge         # run with all tools
cargo run -p mcp-tool-bridge -- --tools ls,wc,diff  # selective
```
