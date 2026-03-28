# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added
- `find` tool — recursive search with name globs, type/size/depth filters, limit (10 tests)
- `curl` tool — structured HTTP: status, headers, body, timing breakdown, JSON detection (4 tests)
- 15 adversarial benchmark tests for diff (9) and lsof (6) edge cases
- 740-test mega integration suite tested against real infrastructure
- `find` added to pipe source whitelist

### Fixed
- lsof protocol+port now combined into single -i flag (e.g., -iTCP:8766) — previously generated conflicting flags

### Changed
- 15 tools total (up from 13): added find and curl
- 74 unit tests (up from 45) + 740 integration tests
