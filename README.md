# MCP Tool Bridge

MCP servers that wrap CLI tools with structured JSON output, targeting tools where parsing difficulty justifies the overhead. Measurement-first: benchmarks prove value before building. Strategy validated through adversarial debate (4 rounds, 49 objections).

## Getting Started

### Prerequisites

- Rust 1.75+ (with cargo)

### Installation

```bash
cargo build
```

### Running

```bash
# Run the MCP server
cargo run -p mcp-tool-bridge

# Register as MCP server in Claude Code settings
```

```json
{
  "mcpServers": {
    "tool-bridge": {
      "command": "cargo",
      "args": ["run", "-p", "mcp-tool-bridge"],
      "cwd": "/home/hibryda/code/ai/mcp-tool-bridge"
    }
  }
}
```

## Why?

LLMs interact with CLI tools by constructing commands, executing them, and parsing unstructured text. For tools with complex output (unified diffs, file descriptor tables), this parsing frequently fails. MCP Tool Bridge provides structured JSON for tools where the parsing difficulty justifies the MCP schema overhead.

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

## Tool Priority (tribunal-validated)

Prioritized by parsing-difficulty x error-cost, not frequency. Validated via Feynman first-principles, Plato invariant-consistency, and adversarial tribunal debate.

**Tier 1 (build first):**
| Tool | Rationale |
|------|-----------|
| `diff` | Complex unified diff hunks — agents misparse line ranges. Measurement-gated. |
| `lsof` | Structured fd table — genuine ecosystem gap. Measurement-gated. |
| `ls` | Structured dir metadata (size, type, perms). Highest frequency (4,273 calls). |
| `wc` | Typed `{lines, words, bytes, chars}` per file. High frequency (1,489 calls). |

**Tier 2 (conditional on category audit):**
| Tool | Rationale |
|------|-----------|
| `kubectl` | Structured replacement if existing MCP server returns raw text |
| `docker` | bollard-backed native API, sync-only operations |
| `sqlite3` | rusqlite with CLI-flag-only path whitelist, read-only default |

**Tier 3 (optional):** `curl` — structured HTTP response envelope. See `docs/README.md` for full analysis.

## License

Private — experimental.
