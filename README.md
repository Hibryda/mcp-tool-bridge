# MCP Tool Bridge

MCP servers that replace common CLI tools with structured, LLM-friendly interfaces. Instead of parsing unstructured text output from `find`, `grep`, `jq`, etc., Claude Code gets structured JSON with proper types, error handling, and composability.

## Getting Started

### Prerequisites

- Rust 1.75+ (with cargo)

### Installation

```bash
cargo build
```

### Running

```bash
# Run a specific tool server
cargo run -p tools

# Register as MCP server in Claude Code settings
```

```json
{
  "mcpServers": {
    "tool-bridge": {
      "command": "cargo",
      "args": ["run", "-p", "tools"],
      "cwd": "/home/hibryda/code/ai/mcp-tool-bridge"
    }
  }
}
```

## Why?

LLMs interact with CLI tools by constructing commands, executing them, and parsing unstructured text. MCP Tool Bridge replaces this with structured JSON-RPC — typed inputs, typed outputs, proper errors.

## Project Structure

```
mcp-tool-bridge/
├── Cargo.toml              # Workspace root
├── crates/
│   ├── bridge-core/        # Shared MCP server scaffolding
│   │   └── src/lib.rs
│   └── tools/              # Individual tool wrappers
│       └── src/lib.rs
└── docs/                   # Documentation
```

## Tool Candidates

| Tool | Priority | Rationale |
|------|----------|-----------|
| `find` | High | Most common, output is trivially structured |
| `grep` | High | Frequently used, benefits from structured match results |
| `jq` | Medium | Already structured, but error handling is poor |
| `curl` | Medium | Headers + body as structured response |
| `docker` | Low | Complex, many subcommands |
| `git` | Low | Already has good MCP wrappers |

## License

Private — experimental.
