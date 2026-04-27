# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added
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
- 103 unit tests (up from 45) + 1740 integration tests
