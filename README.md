# MCP Tool Bridge

MCP server wrapping CLI tools with structured JSON output. 20 tools, single Rust binary (~10MB), stdio transport. 103 unit tests + 1740 integration tests.

## Before / After

### `diff` - raw text vs structured

**Without tool-bridge** (agent must regex-parse):
```
@@ -1,5 +1,7 @@ fn main() {
     let x = 1;
-    let y = 2;
-    let z = 3;
+    let y = 20;
+    let z = 30;
+    let w = 40;
```
Agent must: parse `@@` header for line ranges, track +/- prefixes, compute line numbers manually. Frequently miscounts across multi-hunk diffs.

**With tool-bridge** (structured JSON):
```json
{
  "hunks": [{
    "old_start": 1, "old_count": 5,
    "new_start": 1, "new_count": 7,
    "section": "fn main() {",
    "lines": [
      {"kind": "context", "content": "let x = 1;", "old_line": 1, "new_line": 1},
      {"kind": "delete", "content": "let y = 2;", "old_line": 2},
      {"kind": "add", "content": "let y = 20;", "new_line": 2},
      {"kind": "add", "content": "let w = 40;", "new_line": 4}
    ]
  }],
  "total_additions": 4, "total_deletions": 2
}
```

### `kubectl` - columnar text vs typed metadata

**Without tool-bridge** (existing MCP server returns):
```
NAMESPACE   NAME              READY   STATUS             RESTARTS   AGE
default     nginx-abc123      1/1     Running            0          2d
default     redis-def456      0/1     CrashLoopBackOff   62         5d
```

**With tool-bridge**:
```json
{
  "items": [{
    "kind": "Pod",
    "metadata": {"name": "redis-def456", "namespace": "default", "labels": {"app": "redis"}},
    "status": {"phase": "CrashLoopBackOff"}
  }],
  "count": 2
}
```

### `lsof` - field-separated text vs typed FDs

**Without tool-bridge** (raw `lsof` output):
```
COMMAND     PID   FD   TYPE  NAME
nginx      1234   7u   IPv4  127.0.0.1:8080
nginx      1234   8u   IPv4  127.0.0.1:8080->10.0.0.5:43210
```

**With tool-bridge**:
```json
{
  "processes": [{
    "pid": 1234, "command": "nginx",
    "files": [
      {"fd": "7", "type": "IPv4", "protocol": "TCP", "name": "127.0.0.1:8080"},
      {"fd": "8", "type": "IPv4", "protocol": "TCP", "name": "127.0.0.1:8080->10.0.0.5:43210"}
    ]
  }],
  "total_fds": 2
}
```

## Tools

| Tool | Description |
|------|-------------|
| `ls` | Directory listing with file metadata (size, type, perms, mtime) |
| `wc` | Word/line/byte/char counts from file, text, or multiple paths |
| `diff` | Parse unified diff into typed hunks with line numbers |
| `find` | Recursive file search with globs, type/size/depth filters |
| `lsof` | Open files and network sockets with typed FDs |
| `ps` | Process listing with PID, user, CPU%, memory, args (Linux+macOS) |
| `git_status` | Branch info + file entries via `--porcelain=v2`, typed errors |
| `git_log` | Structured commits with stable pagination, optional `--numstat` |
| `git_show` | Single commit details, restricted to commit objects |
| `gh_api` | GitHub API via gh CLI, path validation, auth redaction |
| `kubectl_list` | List K8s resources with typed metadata |
| `kubectl_get` | Get single K8s resource with typed metadata |
| `docker_list` | List containers via Docker Engine API |
| `docker_inspect` | Inspect container state, network, mounts |
| `docker_images` | List images with tags and sizes |
| `sqlite_query` | Read-only SQL queries with typed rows |
| `sqlite_tables` | Database schema introspection |
| `curl` | HTTP request with structured status, headers, timing, body |
| `batch` | Run multiple tools in parallel, one MCP call |
| `pipe` | Run listing tool + filter on structured fields |

## Installation

```bash
cargo build --release
```

### Claude Code

Add to `~/.claude/.claude.json`:
```json
{
  "mcpServers": {
    "tool-bridge": {
      "command": "/path/to/target/release/mcp-tool-bridge",
      "args": [],
      "type": "stdio"
    }
  }
}
```

### Limit tools (reduce schema overhead)

```bash
# Only register specific tools
mcp-tool-bridge --tools ls,wc,diff

# List available tools
mcp-tool-bridge --list-tools
```

Each registered tool adds ~200-400 tokens of schema to every conversation. Use `--tools` to limit to what you need.

## Architecture

```
mcp-tool-bridge/
├── crates/
│   ├── bridge-core/     # Shared types (BridgeError, FileEntry, WcResult)
│   └── tools/           # MCP server + all tool implementations
│       └── src/
│           ├── main.rs  # MCP server, tool_router, --tools flag
│           ├── dispatch.rs # Free functions for batch/pipe dispatch
│           ├── batch.rs   # Parallel multi-tool executor
│           ├── pipe.rs    # Structured filter on listing output
│           ├── ls.rs        # Directory listing via tokio::fs
│           ├── wc.rs        # Word counting (file/text/multi-path)
│           ├── diff.rs      # Unified diff parser
│           ├── find.rs      # Recursive file search with filters
│           ├── lsof.rs      # lsof -F parser
│           ├── ps.rs        # Cross-platform process listing
│           ├── git_status.rs # --porcelain=v2 with typed errors
│           ├── git_log.rs   # STX/ETX sentinels, snapshot_oid pagination
│           ├── git_show.rs  # cat-file preflight, commit-only
│           ├── gh_api.rs    # GitHub API via gh CLI, auth redaction
│           ├── curl.rs      # HTTP with structured response
│           ├── kubectl.rs   # kubectl -o json wrapper
│           ├── docker.rs    # bollard Docker Engine API
│           └── sqlite.rs    # rusqlite read-only queries
├── tests/               # 1740-test integration suite
└── docs/                # Design docs, category audit, analysis
```

## Tech Stack

- Rust 2021 + Cargo workspace
- rmcp 1.3.0 (official Anthropic MCP SDK)
- bollard (Docker Engine API)
- rusqlite (SQLite, bundled)
- tokio, serde, schemars

## License

Private — experimental.
