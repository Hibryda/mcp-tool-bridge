# MCP Tool Bridge

MCP servers that replace common CLI tools with structured, LLM-friendly interfaces. Wraps tools like ls, wc, curl, ssh, ps, and sqlite3 — providing structured JSON I/O instead of unstructured text output. Tool selection is data-driven from 71K+ Bash calls across 5K sessions (see `docs/README.md § Usage Analysis`).

## Status

Early stage — scaffolding complete, implementation not started.

## Tech Stack

- Rust (2021 edition)
- Cargo workspace
- tokio (async runtime)
- serde / serde_json (JSON serialization)
- MCP crate (TBD — rmcp or mcp-server)

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
