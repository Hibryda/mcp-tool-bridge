# TODO

## Remaining

- [ ] --log-calls flag for call logging (JSONL format for usage analysis)
- [ ] --batch-concurrency N and --batch-timeout-secs N flags
- [ ] Signal handler (SIGTERM/SIGINT) with CancellationToken for batch cleanup
- [ ] Benchmark diff/lsof — 30 adversarial tasks, calibration pilot (optional per tribunal dissent)
- [ ] Soft tool ceiling warning at 21 tools (currently at 20)

## Phase 3: Optional / Deferred

See `DEFERRED.md` for the full list with deferral reasons.

- [ ] Composite tools (ls_count, wc_multi) — based on call logging data
- [ ] Batch-of-pipes (nested pipe inside batch operations)

## Completed

- [x] Choose MCP crate — rmcp 1.3.0 | Done: 2026-03-28
- [x] Implement bridge-core: shared types, run_command | Done: 2026-03-28
- [x] Implement `ls`, `wc`, `diff`, `lsof`, `find`, `curl` (Tier 1) | Done: 2026-03-28
- [x] Implement `kubectl`, `docker`, `sqlite3` (Tier 2) | Done: 2026-03-28
- [x] Implement `batch` and `pipe` meta-tools | Done: 2026-03-28
- [x] Implement `git_status`, `git_log`, `git_show`, `gh_api`, `ps` | Done: 2026-03-29
- [x] 740 integration tests for original 15 tools | Done: 2026-03-28
- [x] 1000 integration tests for 5 new tools (1740 total) | Done: 2026-03-29
- [x] Fix lsof protocol+port flag combination | Done: 2026-03-28
- [x] Fix git_log parse_warnings always-serialized | Done: 2026-03-29
- [x] GitHub Actions CI (ubuntu+macos, all green) | Done: 2026-04-27
- [x] DEFERRED.md governance doc | Done: 2026-04-27
- [x] Push to GitHub (https://github.com/Hibryda/mcp-tool-bridge) | Done: 2026-04-27
- [x] Fix sqlite path validation for macOS tempdir | Done: 2026-04-27
- [x] Tier 1 e2e: Rust harness + 139 tests (ports Python harness) | Done: 2026-04-27
- [x] Tier 2: 10 snapshots + 10 property tests + 11 MCP protocol + 5 doc tests | Done: 2026-04-27
- [x] Tier 3: 17 failure-mode tests (concurrency, limits, IO/permissions, unicode) | Done: 2026-04-27
- [x] Tier 4: criterion benches + cargo-llvm-cov coverage + nightly mutants | Done: 2026-04-27
