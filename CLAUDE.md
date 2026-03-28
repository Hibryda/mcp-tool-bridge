# MCP Tool Bridge

MCP servers wrapping CLI tools with structured JSON output. 13 tools in a single Rust binary: ls, wc, diff, lsof, kubectl (list/get), docker (list/inspect/images), sqlite (query/tables), batch, pipe. See `docs/README.md` and `.tribunal/tribunal-report.md`.

## Status

v0.1 complete — all tools implemented and tested (45 unit tests). MCP server registered globally in Claude Code. Release binary: 9.6MB. Remaining: benchmark diff/lsof (optional), CI, curl (Tier 3).

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
cargo build --release                # release (9.6MB binary)
cargo test                           # 45 tests
cargo run -p mcp-tool-bridge         # run with all tools
cargo run -p mcp-tool-bridge -- --tools ls,wc,diff  # selective
```
