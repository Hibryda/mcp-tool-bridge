# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added
- `diff` tool — unified diff parser with typed hunks, line numbers, format detection pre-pass (8 tests)
- `lsof` tool — -F field parser, port/PID/protocol filtering, version detection (7 tests)
- `kubectl_list` / `kubectl_get` tools — all-resource passthrough via -o json, typed metadata (7 tests)
- `docker_list` / `docker_inspect` / `docker_images` — bollard native Docker Engine API
- `sqlite_query` / `sqlite_tables` — rusqlite read-only, path validation, schema introspection (4 tests)
- `batch` meta-tool — parallel execution of any registered tools, per-op error isolation (3 tests)
- `pipe` meta-tool — structured filtering on listing tool output (AND semantics, dot notation) (7 tests)
- `--tools` flag for selective tool registration (reduces schema overhead)
- `--list-tools` flag to print available tools
- dispatch.rs free function architecture shared by rmcp router and batch dispatcher
- Multi-path `wc` support via `paths: Vec<String>` parameter
- Category audit documenting kubectl (0/7 structured) and docker (0/2 structured)
- Supertribunal debate for batch/pipe design (50 objections, 78% confidence)
- Before/after demos in README (diff, kubectl, lsof)
- bollard and rusqlite dependencies (tribunal-approved)
- Release binary registered globally in Claude Code MCP settings

### Changed
- Manual ServerHandler impl replaces #[tool_handler] macro (enables --tools filtering)
- wc tool now accepts `paths`, `path`, or `input` parameters
- 45 unit tests total (up from 9)
