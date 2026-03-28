# TODO

## Phase 1: Benchmark (Days 1-11) — diff + lsof only

- [ ] Create pre-registration document: candidate tools (diff, lsof), 30 tasks/tool, calibration pilot, suspension criterion
- [ ] Run calibration pilot: 10 raw-text tasks per tool via Anthropic API in Jupyter
- [ ] Run 30-task adversarial benchmark per tool (raw vs structured accuracy)
- [ ] Run 8-task MCP-context validation subset if any tool passes threshold
- [ ] Decision point: proceed or suspend per pre-registered criterion

## Phase 1.5: Category Audit (Days 11-12)

- [ ] Audit existing kubectl MCP server — test 2 ops per category (list/query, describe/inspect, log/stream, exec/apply)
- [ ] Audit existing Docker MCP server — same category matrix
- [ ] Audit existing sqlite3 MCP server — same category matrix
- [ ] Document which categories return raw text vs structured JSON

## Phase 2: Implementation (Days 13-25, conditional)

- [ ] Implement `diff` wrapper — format detection pre-pass, unified diff only, graceful routing (if benchmark passes)
- [ ] Implement `lsof` wrapper — version-keyed static field lookup, macOS 4.89 vs Linux 4.95+ (if benchmark passes)
- [ ] Implement `kubectl` wrapper — all-resource passthrough via -o json, typed metadata only (if audit confirms gap)
- [ ] Implement Docker wrapper — bollard-backed native API, sync-only (if audit confirms gap)
- [ ] Implement `sqlite3` wrapper — rusqlite, CLI-flag-only whitelist, read-only default (if audit confirms gap)
- [ ] Implement --tools opt-in flag for selective tool registration
- [ ] CI: GitHub Actions with ubuntu-latest + macos-latest runners
- [ ] README primary artifact: before/after parse error demonstration

## Phase 3: Optional

- [ ] Implement `curl` wrapper — structured HTTP response envelope (status, headers, timing, redirect chain)

## Completed

- [x] Choose MCP crate — rmcp 1.3.0 (official Anthropic SDK) | Done: 2026-03-28
- [x] Implement bridge-core: shared types (BridgeError, FileEntry, WcResult), run_command utility | Done: 2026-03-28
- [x] Implement `ls` wrapper — structured dir metadata via tokio::fs, 4 unit tests | Done: 2026-03-28
- [x] Implement `wc` wrapper — typed counts from file or inline text, 5 unit tests | Done: 2026-03-28
- [x] MCP server main.rs with rmcp tool_router, stdio transport, working tools/list + tools/call | Done: 2026-03-28
