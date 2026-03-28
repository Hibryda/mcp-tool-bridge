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

## Tool Priority (data-driven)

Based on analysis of 71,639 Bash calls across 5,050 Claude Code sessions. Tools already covered by native Claude Code tools (grep, find, cat) or existing MCP servers (kubectl, docker) are excluded.

| Tool | Calls | Priority | Rationale |
|------|------:|----------|-----------|
| `ls` | 4,273 | High | Structured directory listings with file metadata |
| `wc` | 1,489 | High | Line/word/byte counts — trivial to wrap |
| `curl` | 1,342 | High | HTTP with structured response parsing |
| `ssh` | 1,189 | Medium | Remote execution — complex security model |
| `ps` | 347 | Medium | Process listing with structured fields |
| `sqlite3` | 246 | Low | Structured query results |

## License

Private — experimental.
