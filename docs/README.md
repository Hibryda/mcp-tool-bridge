---
title: "Documentation"
role: part
parent: null
order: 1
description: "MCP Tool Bridge project documentation"
---

# MCP Tool Bridge — Documentation

## Overview

MCP Tool Bridge creates MCP server wrappers around common CLI tools, providing Claude Code (and other MCP clients) with structured JSON interfaces instead of unstructured text output. The project "plants" replacement commands that intercept tool calls and route them through MCP servers.

## Design Principles

### 1. Structured Over Unstructured

Every tool wrapper transforms text output into typed JSON:
- File paths → `{path: String, size: u64, modified: DateTime}`
- Match results → `{file: String, line: u32, content: String, context: Vec<String>}`
- Errors → `{code: String, message: String, suggestion: Option<String>}`

### 2. Faithful Wrapping

Tool wrappers should expose the same capabilities as the underlying tool, not a subset. If `grep` supports `-C` for context, the MCP wrapper should too. The goal is zero capability loss with structured gain.

### 3. Composable

MCP tools can call each other. `find` results can be piped to `grep`. This is modeled as tool composition, not shell piping.

## Architecture

### bridge-core

Shared infrastructure:
- MCP server setup (stdio transport, JSON-RPC)
- Tool registration framework
- Common types (file paths, match results, errors)
- Process execution utilities (spawn, capture, timeout)

### tools

Individual tool wrappers. Each wrapper:
1. Defines input schema (serde-derived structs)
2. Validates and translates to CLI arguments
3. Executes the underlying tool
4. Parses output into structured response
5. Maps exit codes to typed errors

## Rust MCP Ecosystem

Decision: Rust over Zig. Rationale:
- serde for zero-effort JSON serialization
- tokio for mature async I/O (MCP needs stdio streams)
- Community MCP crates exist (rmcp, mcp-server)
- Zig would require writing JSON-RPC protocol from scratch

MCP crate selection (TBD):
- `rmcp` — more active, higher-level API
- `mcp-server` — lower-level, more control

## Performance Considerations

MCP stdio transport adds overhead vs direct CLI:
- Process spawn: ~5ms (amortized if server stays running)
- JSON serialization: ~1ms per response
- Total overhead: ~6ms per tool call

For interactive use (Claude Code), this is negligible. For batch operations (find across 100K files), consider batch APIs or streaming responses.

## Usage Analysis (2026-03-28)

Scanned 5,050 Claude Code sessions (71,639 Bash invocations) to establish data-driven tool priority.

### Already Covered (no value in wrapping)

| Tool | Calls | Covered By |
|------|------:|------------|
| grep | 3,600 | Claude Code Grep tool |
| find | 2,692 | Claude Code Glob tool |
| cat/head/tail | 2,950 | Claude Code Read tool |
| sed/awk | 218 | Claude Code Edit tool |
| kubectl | 3,087 | Kubernetes MCP server |
| docker | 1,506 | Docker MCP server |

### Net-New Value — Revised Tool Priority

| Priority | Tool | Calls | % of Bash | Complexity |
|----------|------|------:|----------:|------------|
| 1 | ls | 4,273 | 6.0% | Low — structured dir metadata |
| 2 | wc | 1,489 | 2.1% | Low — trivial to wrap |
| 3 | curl | 1,342 | 1.9% | Medium — HTTP response parsing |
| 4 | ssh | 1,189 | 1.7% | High — security implications |
| 5 | ps | 347 | 0.5% | Low — process listing |
| 6 | sqlite3 | 246 | 0.3% | Medium — structured queries |

34.1% of all Bash calls are wrappable CLI tools. The original priority list (find/grep high, jq medium) was based on intuition. `jq` had exactly 1 call across 5,050 sessions.

## Open Questions

- Should each tool be a separate MCP server or one server with multiple tools?
- How to handle tools that produce very large output (e.g., `find /` )?
- Should the bridge intercept actual shell commands, or only be used via MCP?
- How to version tool wrappers independently?
