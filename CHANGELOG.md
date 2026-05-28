# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

## [0.2.0] - 2026-05-28

### Added
- **`repo_snapshot` tool** (read-only composite) — one call returns branch + ahead/behind, working-tree change counts + entries, an aggregate working diff stat (`git diff --numstat` vs HEAD), and the N most recent commits. Collapses the common `git status`→`git diff`→`git log` inspection combo (1,052× in history). 4 unit + 3 e2e tests.
- **`pr_status` tool** (read-only, forge-agnostic) — detects the forge from the `origin` remote (github→`gh`, gitlab→`glab`, else Forgejo REST) and returns `{state, mergeable, checks{passing,failing,pending}, comments, review_decision (github), review_threads_unresolved (github), ready_to_merge}`. GitHub unresolved-thread count comes from a GraphQL call (`reviewThreads.isResolved`); Forgejo/GitLab leave it `None` (no GraphQL; Forgejo review-bot uses issue comments, not threads). Forgejo token resolves from `FORGEJO_TOKEN` or `fj`'s `keys.json` (alias-aware). GitLab backend is a typed-error stub (glab not installed). 16 unit + 3 e2e tests; verified live against GitHub `cli/cli` and Forgejo `hib-pr-reviewer#116`.
- **Adoption rules** in `.claude/rules/70-72` (`bridge-read-inspection`, `bridge-http-json`, `bridge-chaining`) — prescriptive command→tool tables instructing agents to prefer the bridge over Bash for covered shapes, each with explicit "when Bash is still correct" boundaries. Replaces the PreToolUse suggest-hook as the adoption mechanism.

### Changed
- 22 tools total (up from 20).

### Notes
- **Adoption finding (2026-05-28):** measured ~1.6% MCP tool usage vs equivalent Bash across 26k transcripts. The PreToolUse suggest-hook is a no-op — Claude Code does not inject `hookSpecificOutput.additionalContext` on PreToolUse (it fired 108×/20 sessions with empty model-visible content). Pivoting to context rules; suggest-hook slated for removal.
- **Composite-tool roadmap (2026-05-28):** transcript mining ranked `repo_snapshot` + `pr_status` (read-only) as the highest-value next tools; `pr_merge` / `git_commit_push` (mutating) accepted in principle behind a verify-then-act confirm gate.

## [0.1.0] - 2026-05-05

First released version. Installed via the Claude Code marketplace; tools surface as `mcp__plugin_mcp-tool-bridge_tool-bridge__*`.

### Added
- **Claude Code marketplace + multi-arch release pipeline** — repo root carries `.claude-plugin/marketplace.json` so `claude plugin marketplace add <repo-url>` works against any git host (GitHub canonical, Forgejo / Gitea / GitLab mirrors). `plugin/bin/launcher.sh` is a POSIX-shell launcher that detects the host triple, downloads the matching tarball from `${MCP_TOOL_BRIDGE_RELEASE_BASE_URL}/v<version>/`, verifies the SHA-256 against `plugin/checksums.txt`, and caches under `${CLAUDE_PLUGIN_ROOT}/.bin-cache/`. Pinned version in `plugin/version`. `MCP_TOOL_BRIDGE_BIN_DIR` env override skips the download path for local development. 8 launcher tests cover argument validation, override, warm cache, cold-path download (against a local HTTP server), checksum tampering rejection.
- **`.github/workflows/release.yml`** — tag-triggered (`v*`), four-target build matrix on native runners (x86_64-linux, aarch64-linux, x86_64-darwin, aarch64-darwin), publishes tarballs + `checksums.txt` to GitHub Releases, auto-commits the new checksums back to `master`.
- **PreToolUse hook** (`mcp-tool-bridge-hook`) — parses Bash invocations of `ls`, `wc`, `find`, `diff -u`, `lsof`, `ps`, `git status|log|show`; nudges agents toward the structured MCP equivalent. Modes: `suggest` (default, JSON `additionalContext`), `enforce` (exit 2), `off`. Conservative parser refuses pipelines/redirections/unknown flags rather than risk wrong rewrites. 25 unit + 11 e2e tests, including a 300-command real-history suite (~8% suggestion rate, no false-positive blocks).
- **Claude Code plugin** (`plugin/`) — bundles MCP server + hook via `.claude-plugin/plugin.json`, `.mcp.json`, `hooks/hooks.json`.
- `git_status` tool — `--porcelain=v2 --branch` with typed error envelope (NOT_A_REPO, DETACHED_HEAD, VERSION_TOO_OLD), branch ahead/behind, staged/unstaged detection (6 unit tests)
- `git_log` tool — STX/ETX sentinel format with NUL field separators, snapshot_oid stable pagination, optional `--numstat` stats, merge detection, ref decoration (8 unit tests)
- `git_show` tool — `cat-file -t` preflight restricting to commit objects, typed NOT_A_COMMIT error for non-commits, optional stats (3 unit tests)
- `gh_api` tool — structural path validator, `--include`/`--paginate` mutual exclusion, auth token redaction in errors, rate limit extraction, pagination info (5 unit tests)
- `ps` tool — cross-platform process listing (Linux + macOS fallback), filter by name/user/PID, total_before_filter for truncation visibility (7 unit tests)
- `find` tool — recursive search with name globs, type/size/depth filters, limit (10 unit tests)
- `curl` tool — structured HTTP: status, headers, body, timing breakdown, JSON detection (4 unit tests)
- 15 adversarial benchmark tests for diff (9) and lsof (6) edge cases
- 1740-test mega integration suite tested against real infrastructure (k3d cluster, Docker daemon, httpbin.org, real git repo)
- `find` added to `pipe` source whitelist
- DEFERRED.md governance doc with rejected tools and revisit criteria
- GitHub Actions CI workflow (ubuntu + macos runners)

### Fixed
- lsof protocol+port now combined into single `-i` flag (e.g., `-iTCP:8766`) — previously generated conflicting flags
- git_log `parse_warnings` field always serialized — previously omitted when empty (caused inconsistent agent contracts)
- sqlite path validator now allows files under canonicalized `std::env::temp_dir()` — fixes macOS test failures where tempfile creates under `/var/folders/.../T/`

### Changed
- 20 tools total (up from 13)
- 103 unit tests (up from 45)
- E2E suite ported from Python to native Rust (`crates/tools/tests/e2e/`):
  187 tests covering all tools, MCP protocol conformance, JSON output snapshots,
  property-based parser invariants, failure modes (concurrency, limits, IO).
- 5 doc tests on bridge-core public API
- Criterion benchmarks for diff and lsof parsers (`cargo bench --bench parsers`)
- Coverage tracking via `cargo-llvm-cov` in CI
- Nightly workflow for: real-infra integration tests (docker/k8s/gh_api),
  performance baselines, mutation testing on parser modules
