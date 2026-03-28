# TODO

## Phase 1: Benchmark (Days 1-11) — diff + lsof only

- [ ] Create pre-registration document: candidate tools (diff, lsof), 30 tasks/tool, calibration pilot, suspension criterion
- [ ] Run calibration pilot: 10 raw-text tasks per tool via Anthropic API in Jupyter
- [ ] Run 30-task adversarial benchmark per tool (raw vs structured accuracy)
- [ ] Run 8-task MCP-context validation subset if any tool passes threshold
- [ ] Decision point: proceed or suspend per pre-registered criterion

## Phase 2: Implementation

- [ ] Implement `kubectl` wrapper — all-resource passthrough via -o json, typed metadata only
- [ ] Implement Docker wrapper — bollard-backed native API, sync-only
- [ ] Implement `sqlite3` wrapper — rusqlite, CLI-flag-only whitelist, read-only default
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
- [x] Implement `diff` wrapper — unified diff parser, format detection, line tracking, 8 tests | Done: 2026-03-28
- [x] Implement `lsof` wrapper — -F field parser, network filtering, version detection, 7 tests | Done: 2026-03-28
- [x] MCP server main.rs with rmcp tool_router, stdio transport, 4 tools registered | Done: 2026-03-28
- [x] Register mcp-tool-bridge in global Claude Code MCP settings | Done: 2026-03-28
- [x] Category audit: kubectl (0/7 structured), docker (0/2 structured) — all raw text | Done: 2026-03-28
