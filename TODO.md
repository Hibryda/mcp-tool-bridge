# TODO

## Remaining

### Adoption (the 1.6% problem — see Memora `hook-suggest-mode-noop`)
- [ ] Place the `plugin/rules/*.md` drafts (global `~/.claude/rules/` vs per-project vs bundled — operator decides)
- [ ] Remove or disable the PreToolUse suggest-hook — it's a no-op (Claude Code doesn't inject `additionalContext` on PreToolUse); rules replace it
- [ ] Once adopted, re-measure tool usage vs the 1.6% baseline

### Composite tools (see Memora `composite-tool-roadmap`)
- [ ] `repo_snapshot` (read-only) — one call → status + ahead/behind + log -n + diff --stat (collapses the 1,052× status→diff→log combo)
- [ ] `pr_status` (read-only, forge-agnostic) — PR ref → {state, mergeable, checks_passing, unresolved_threads, comment_count} over gh/fj/Forgejo-REST
- [ ] `pr_merge` (mutating) — verify-then-act + `confirm:true` gate; refuse on failing checks / unresolved threads
- [ ] `git_commit_push` (mutating) — crosses the read-only boundary; opt-in write-tools set, off by default
- [ ] Decide the read→write boundary: keep bridge read-only, or add a gated opt-in write-tools tier

### Distribution / infra
- [ ] Forgejo Releases mirror — optional `release.yml` step to push tarballs to `git.hemoglobina.store` when `FORGEJO_RELEASE_TOKEN` secret is present
- [ ] GPG / cosign signing of release artifacts (deferred until first org rollout)
- [ ] --log-calls flag for call logging (JSONL format for usage analysis)
- [ ] --batch-concurrency N and --batch-timeout-secs N flags
- [ ] Signal handler (SIGTERM/SIGINT) with CancellationToken for batch cleanup
- [ ] Benchmark diff/lsof — 30 adversarial tasks, calibration pilot (optional per tribunal dissent)
- [ ] Soft tool ceiling warning at 21 tools (currently at 20)

## Phase 3: Optional / Deferred

See `DEFERRED.md` for the full list with deferral reasons.

- [ ] Composite tools (ls_count, wc_multi) — based on call logging data
- [ ] Batch-of-pipes (nested pipe inside batch operations)

## Completed (most recent 10; full history in git log)

- [x] Tier 3: 17 failure-mode tests (concurrency, limits, IO/permissions, unicode) | Done: 2026-04-27
- [x] Tier 4: criterion benches + cargo-llvm-cov coverage + nightly mutants | Done: 2026-04-27
- [x] PreToolUse hook (`crates/hook/`) — shell-words tokenizer + 7 command handlers, 25 unit + 11 e2e tests | Done: 2026-04-27
- [x] Claude Code plugin packaging (`plugin/`) — manifest + .mcp.json + hooks.json | Done: 2026-04-27
- [x] Claude Code marketplace manifest + multi-arch launcher with SHA-256 verification | Done: 2026-05-05
- [x] Release workflow (4-target native matrix, GitHub Releases, auto-commit checksums) | Done: 2026-05-05
- [x] v0.1.0 release tag — pipeline validated end-to-end, plugin installed + tools live | Done: 2026-05-05
- [x] Usage analysis (26k transcripts) — measured 1.6% adoption; found suggest-hook is a no-op | Done: 2026-05-28
- [x] Transcript pattern-mining for composite-tool candidates | Done: 2026-05-28
- [x] Draft adoption rules (`plugin/rules/`: read-inspection, http-json, chaining) | Done: 2026-05-28
