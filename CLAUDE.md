# MCP Tool Bridge

MCP servers that wrap CLI tools with structured JSON output. Measurement-first for complex tools (diff, lsof); frequency-justified for simple tools (ls, wc). Tier 1: diff, lsof, ls, wc. Tier 2: kubectl/docker/sqlite3 structured replacements. Tier 3: curl (optional). See `docs/README.md` and `.tribunal/tribunal-report.md`.

## Status

ls and wc tools implemented and tested (9 unit tests). MCP server runs over stdio via rmcp 1.3.0. Next: benchmark phase (diff + lsof).

## Tech Stack

- Rust (2021 edition)
- Cargo workspace
- rmcp 1.3.0 (official MCP SDK — server, transport-io, macros, schemars features)
- tokio (async runtime)
- serde / serde_json / schemars 1.0 (JSON serialization + schema generation)
- thiserror 2, anyhow, tracing

## Architecture

Cargo workspace with shared core and individual tool crates:

```
mcp-tool-bridge/
├── Cargo.toml              # Workspace root
├── crates/
│   ├── bridge-core/        # Shared MCP server scaffolding
│   │   └── src/lib.rs
│   └── tools/              # Individual tool wrappers
│       └── src/lib.rs
└── docs/
```

Each tool wrapper:
1. Accepts structured JSON input via MCP
2. Invokes the underlying CLI tool
3. Parses output into structured JSON
4. Returns typed results with proper error handling

## Documentation (SOURCE OF TRUTH)

**All project documentation lives in [`docs/`](docs/README.md).**

## Development

### Setup

```bash
cargo build
```

### Testing

```bash
cargo test
```

### Running

```bash
cargo run -p tools
```
