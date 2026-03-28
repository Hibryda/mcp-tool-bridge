# TODO

## Remaining

- [ ] Benchmark diff/lsof — 30 adversarial tasks, calibration pilot (optional per tribunal dissent)
- [ ] CI: GitHub Actions with ubuntu-latest + macos-latest runners
- [ ] Push to GitHub
- [ ] --log-calls flag for call logging (JSONL format for usage analysis)
- [ ] --batch-concurrency N and --batch-timeout-secs N flags
- [ ] Signal handler (SIGTERM/SIGINT) with CancellationToken for batch cleanup

## Phase 3: Optional

- [ ] Implement `curl` wrapper — structured HTTP response envelope
- [ ] Composite tools (ls_count, wc_multi) based on call logging data
- [ ] Batch-of-pipes (nested pipe inside batch operations)

## Completed

- [x] Choose MCP crate — rmcp 1.3.0 (official Anthropic SDK) | Done: 2026-03-28
- [x] Implement bridge-core: shared types, run_command utility | Done: 2026-03-28
- [x] Implement `ls` wrapper — structured dir metadata, 4 tests | Done: 2026-03-28
- [x] Implement `wc` wrapper — typed counts, multi-path support, 5 tests | Done: 2026-03-28
- [x] Implement `diff` wrapper — unified diff parser, line tracking, 8 tests | Done: 2026-03-28
- [x] Implement `lsof` wrapper — -F field parser, network filtering, 7 tests | Done: 2026-03-28
- [x] Implement `kubectl` wrapper — -o json, typed metadata, 7 tests | Done: 2026-03-28
- [x] Implement Docker wrapper — bollard native API, list/inspect/images | Done: 2026-03-28
- [x] Implement `sqlite3` wrapper — rusqlite read-only, 4 tests | Done: 2026-03-28
- [x] Implement `batch` meta-tool — parallel dispatch, error isolation, 3 tests | Done: 2026-03-28
