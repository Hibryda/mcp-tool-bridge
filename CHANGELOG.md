# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added
- Usage analysis of 71,639 Bash calls across 5,050 Claude Code sessions
- General ecosystem research (SO surveys, academic datasets, practitioner blogs)
- MCP ecosystem gap analysis (28+ existing servers reviewed)
- Feynman first-principles analysis of project strategy
- Plato invariant-consistency analysis of coupled relationships
- Tribunal adversarial debate (4 rounds, 49 objections, 62% confidence ruling)
- Tribunal report at `.tribunal/tribunal-report.md`
- TODO.md with phased implementation roadmap

### Changed
- **Strategy pivot:** measurement-first approach — benchmark before building
- **Tool priority:** hybrid approach — measurement-gated (diff, lsof) + frequency-justified (ls, wc)
- **Design principles:** "faithful wrapping" replaced with "pragmatic subset"; "composability" dropped
- **Architecture:** single binary with --tools opt-in (not separate servers)
- Updated all docs, CLAUDE.md files, and README to reflect tribunal findings

### Removed
- `curl` demoted to Tier 3 / optional (JSON body needs no wrapper)
- "Composability" design principle (MCP round-trips worse than shell piping)
- "Faithful wrapping" design principle (agents use ~5-10 flags per tool, not 200+)
