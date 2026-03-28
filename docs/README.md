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

Tool wrappers transform text output into typed JSON where parsing difficulty justifies the overhead:
- Diff hunks → `{hunks: [{start_a, count_a, start_b, count_b, lines}]}`
- File descriptors → `{pid, fd, type, protocol, address, file}`
- Errors → `{code: String, message: String, suggestion: Option<String>}`

### 2. Pragmatic Subset (not Faithful Wrapping)

Wrappers expose the ~5-10 most common flag combinations agents actually use, not all capabilities. `curl` has 200+ flags; agents use a handful. Machine-readable flags (`-o json`, `-F`, `--unified`) are preferred over parsing human-readable output.

### 3. Measurement-Gated

No tool is built without evidence that structured output measurably improves agent performance. The qualification gate: 30 adversarial extraction tasks, structured accuracy must exceed raw text accuracy by ≥15pp (calibration-derived threshold).

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

### Frequency Data (from 71K Bash calls)

| Tool | Calls | % of Bash | Notes |
|------|------:|----------:|-------|
| ls | 4,273 | 6.0% | Tier 1 — structured dir metadata |
| wc | 1,489 | 2.1% | Tier 1 — typed counts per file |
| curl | 1,342 | 1.9% | Tier 3 — structured HTTP response (optional) |
| ssh | 1,189 | 1.7% | Deferred — complex security model |
| ps | 347 | 0.5% | Tier 2 candidate |
| sqlite3 | 246 | 0.3% | Tier 2 — rusqlite with CLI-flag-only whitelist |

### Final Priority (tribunal + owner overrides, 2026-03-28)

Tribunal debate (4 rounds, 49 objections) established: value = parsing-difficulty × error-cost. Owner overrides: ls and wc retained for frequency value despite low parsing difficulty.

**Tier 1 (build first):**
- `diff` — complex unified diff hunks, agents misparse line ranges. Measurement-gated: 30-task benchmark.
- `lsof` — structured fd table, genuine ecosystem gap, version-keyed parsing. Measurement-gated.
- `ls` — structured dir metadata with file size, type, permissions. High frequency (4,273 calls). Owner override.
- `wc` — typed `{lines, words, bytes, chars}` per file. High frequency (1,489 calls). Owner override.

**Tier 2 (conditional on category audit of existing MCP servers):**
- `kubectl` — structured replacement if existing server returns raw text
- `docker` — bollard-backed native API (sync-only operations)
- `sqlite3` — rusqlite with security constraints

**Tier 3 (if time permits):**
- `curl` — structured HTTP response envelope (status, headers, timing, redirect chain). Body as string.

See `.tribunal/tribunal-report.md` for full debate transcript and rationale.

## Resolved Questions

- **One server or many?** Single binary with `--tools` opt-in flag (tribunal S-1/OBJ-19).
- **Faithful or pragmatic wrapping?** Pragmatic subset — agents use ~5-10 flags per tool (tribunal F4-PLATO).
- **Composability?** Dropped — MCP composability is strictly worse than shell piping (tribunal F5-PLATO).

## Open Questions

- Rust vs TypeScript: Rust acceptable if learning goal; TypeScript wins on velocity and ecosystem fit.
- Whether current Claude models actually misparse unified diff at rates justifying structured wrapping.
- Whether existing kubectl/docker MCP servers already return structured JSON (category audit needed).
- Statistical power of 30-task benchmark with ±18pp confidence interval.
