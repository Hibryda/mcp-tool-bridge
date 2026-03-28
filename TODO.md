# TODO

## Remaining

- [ ] CI: GitHub Actions with ubuntu-latest + macos-latest runners
- [ ] Push to GitHub
- [ ] --log-calls flag for call logging (JSONL format for usage analysis)
- [ ] --batch-concurrency N and --batch-timeout-secs N flags
- [ ] Signal handler (SIGTERM/SIGINT) with CancellationToken for batch cleanup
- [ ] Benchmark diff/lsof — 30 adversarial tasks, calibration pilot (optional per tribunal dissent)

## Phase 3: Optional

- [ ] Composite tools (ls_count, wc_multi) based on call logging data
- [ ] Batch-of-pipes (nested pipe inside batch operations)

## Completed

- [x] Choose MCP crate — rmcp 1.3.0 | Done: 2026-03-28
- [x] Implement bridge-core: shared types, run_command | Done: 2026-03-28
- [x] Implement `ls` — structured dir metadata, 4 tests | Done: 2026-03-28
- [x] Implement `wc` — typed counts, multi-path, 5 tests | Done: 2026-03-28
- [x] Implement `diff` — unified diff parser, 8 + 9 adversarial tests | Done: 2026-03-28
- [x] Implement `lsof` — -F field parser, 7 + 6 adversarial tests | Done: 2026-03-28
- [x] Implement `find` — recursive search, globs, type/size filters, 10 tests | Done: 2026-03-28
- [x] Implement `kubectl` — -o json, typed metadata, 7 tests | Done: 2026-03-28
- [x] Implement Docker — bollard native API, list/inspect/images | Done: 2026-03-28
- [x] Implement `sqlite3` — rusqlite read-only, 4 tests | Done: 2026-03-28
