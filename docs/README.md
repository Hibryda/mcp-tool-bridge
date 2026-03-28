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

Shared types and utilities (`crates/bridge-core/src/lib.rs`):
- `BridgeError` — typed errors: CommandFailed, CommandNotFound, Io, Parse, Timeout
- `FileEntry` — structured file metadata (name, path, type, size, permissions, modified)
- `WcResult` — word count result (file, lines, words, bytes, chars)
- `run_command()` — async process execution with error mapping

### tools (binary: `mcp-tool-bridge`)

MCP server binary (`crates/tools/src/main.rs`) using rmcp 1.3.0 `#[tool_router]` pattern:
- `ls` tool — lists directory contents via `tokio::fs::read_dir`, returns `Vec<FileEntry>`
- `wc` tool — counts lines/words/bytes/chars from file path or inline text input

## Rust MCP Ecosystem

**Decision: rmcp 1.3.0** (official Anthropic SDK, 3.2K stars). Evaluated vs rust-mcp-sdk 0.9.0 (163 stars).

Rationale:
- Macro-driven tool definition (`#[tool_router]`, `#[tool]`, `Parameters<T>`)
- Official SDK — Anthropic-maintained, production adoption (OpenAI Codex migrated to it)
- serde + schemars for auto-generated JSON schemas from Rust structs

**CRITICAL: Claude Code bug #25081** — omit `outputSchema` and `instructions` fields from MCP responses, or `tools/list` silently returns empty with no error.

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

**Tier 2 (category audit confirmed — all existing servers return raw text):**
- `kubectl` — all-resource passthrough via `-o json`, typed metadata, `serde_json::Value` for spec/status
- `docker` — bollard-backed native Docker Engine API (sync-only: list, inspect, images)
- `sqlite3` — rusqlite read-only, CLI-flag-only path whitelist, `O_NOFOLLOW` security

**Meta-tools (supertribunal, 50 objections, 78% confidence):**
- `batch` — generic parallel executor. Any registered tool, concurrent dispatch, per-op error isolation.
- `pipe` — structured filtering on listing tool output. AND semantics, dot notation, limit.

**Tier 3 (optional):**
- `curl` — structured HTTP response envelope (status, headers, timing, redirect chain).

See `.tribunal/tribunal-report.md` for debate transcripts and rationale.

## Resolved Questions

- **One server or many?** Single binary with `--tools` opt-in flag.
- **Faithful or pragmatic wrapping?** Pragmatic subset — agents use ~5-10 flags per tool.
- **Composability?** Replaced by `pipe` meta-tool for filtering and `batch` for parallel ops.
- **Category audit?** Done — kubectl (0/7 structured), docker (0/2 structured). See `docs/category-audit.md`.
- **Piped commands?** `pipe` tool handles source→filter patterns. Batch-of-pipes deferred to v2.

## Open Questions

- Whether current Claude models actually misparse unified diff at rates justifying structured wrapping.
- Statistical power of 30-task benchmark with ±18pp confidence interval.
- Optimal batch concurrency limit (default 4) — no data on typical batch sizes.
- Whether to add composite tools (ls_count, wc_multi) based on call logging data.
