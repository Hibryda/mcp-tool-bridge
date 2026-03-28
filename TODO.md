# TODO

## Phase 1: Benchmark (Days 1-11)

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

- [ ] Choose MCP crate (rmcp vs mcp-server)
- [ ] Implement bridge-core: MCP server setup, tool registration, --tools opt-in flag
- [ ] Implement `diff` wrapper — format detection pre-pass, unified diff only, graceful routing
- [ ] Implement `lsof` wrapper — version-keyed static field lookup (macOS 4.89 vs Linux 4.95+)
- [ ] Implement `kubectl` wrapper — all-resource passthrough via -o json, typed metadata only (if audit confirms gap)
- [ ] Implement Docker wrapper — bollard-backed native API, sync-only (if audit confirms gap)
- [ ] Implement `sqlite3` wrapper — rusqlite, CLI-flag-only whitelist, read-only default (if audit confirms gap)
- [ ] CI: GitHub Actions with ubuntu-latest + macos-latest runners
- [ ] README primary artifact: before/after parse error demonstration

## Completed

(none yet)
