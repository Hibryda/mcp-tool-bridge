# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added
- Working MCP server binary (`mcp-tool-bridge`) with stdio transport via rmcp 1.3.0
- `ls` tool — structured directory listing with name, path, type, size, permissions, modified time
- `wc` tool — typed word count (lines, words, bytes, chars) from file path or inline text
- bridge-core shared types: `BridgeError`, `FileEntry`, `WcResult`, `run_command()` utility
- 9 unit tests (4 for ls, 5 for wc)
- Codex independent cross-model verification of tribunal findings (74% confidence, partial divergence)
- Usage analysis of 71,639 Bash calls across 5,050 Claude Code sessions
- General ecosystem research (SO surveys, academic datasets, practitioner blogs)
- MCP ecosystem gap analysis (28+ existing servers reviewed)
- Feynman first-principles analysis of project strategy
- Plato invariant-consistency analysis of coupled relationships
- Tribunal adversarial debate (4 rounds, 49 objections, 62% confidence ruling)

### Changed
- **MCP crate:** selected rmcp 1.3.0 (official Anthropic SDK, 3.2K stars)
- **Strategy pivot:** measurement-first approach — benchmark before building complex tools
- **Tool priority:** hybrid — measurement-gated (diff, lsof) + frequency-justified (ls, wc)
- **Design principles:** "faithful wrapping" replaced with "pragmatic subset"; "composability" dropped
- **Architecture:** single binary with --tools opt-in (not separate servers)
- Package renamed from `tools` to `mcp-tool-bridge`

### Removed
- `curl` demoted to Tier 3 / optional (JSON body needs no wrapper)
- "Composability" design principle (MCP round-trips worse than shell piping)
- "Faithful wrapping" design principle (agents use ~5-10 flags per tool, not 200+)
