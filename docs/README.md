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

## Open Questions

- Should each tool be a separate MCP server or one server with multiple tools?
- How to handle tools that produce very large output (e.g., `find /` )?
- Should the bridge intercept actual shell commands, or only be used via MCP?
- How to version tool wrappers independently?
