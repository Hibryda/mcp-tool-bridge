# MCP Tool Bridge

MCP servers replacing common CLI tools with structured JSON I/O. Rust cargo workspace with shared core and per-tool crates.

## Operational Rules

All operational rules live in `.claude/rules/`. Every `.md` file in that directory is automatically loaded at session start by Claude Code with the same priority as this file.

### Rule Index

| # | File | Scope |
|---|------|-------|
| 01 | `security.md` | **PARAMOUNT** — secrets, input validation, least privilege |
| 02 | `error-handling.md` | **PARAMOUNT** — handle every error visibly |
| 03 | `environment-safety.md` | **PARAMOUNT** — verify target, data safety, K8s isolation, cleanup |
| 04 | `communication.md` | Stop on ambiguity, scope discipline |
| 05 | `git-practices.md` | Conventional commits, authorship |
| 06 | `testing.md` | TDD, unit tests, E2E tests |
| 07 | `documentation.md` | README, CLAUDE.md sync, docs/ |
| 08 | `branch-hygiene.md` | Branches, naming, clean state before refactors |
| 09 | `dependency-discipline.md` | No deps without consent |
| 10 | `code-consistency.md` | Match existing patterns |
| 11 | `api-contracts.md` | Contract-first, flag breaking changes (path-conditional) |
| 12 | `performance-awareness.md` | No N+1, no unbounded fetches (path-conditional) |
| 13 | `logging-observability.md` | Structured logging, OTEL (path-conditional) |
| 14 | `resilience-and-config.md` | Timeouts, circuit breakers, externalized config (path-conditional) |
| 15 | `memora.md` | Persistent memory across sessions |
| 16 | `sub-agents.md` | When to use sub-agents and team agents |
| 17 | `document-imports.md` | Resolve @ imports in CLAUDE.md before acting |

## Domain Routing

| Trigger Pattern | Specialist | Evidence |
|----------------|------------|----------|
<!-- Empty — specialists are created on demand from recurring friction patterns -->

## Output Efficiency

Minimize token waste from verbose CLI output. Use compact flags and targeted queries by default.

### Git
- `git diff --stat` first — only `git diff <file>` for files you need to inspect
- `git log --oneline -20` unless full commit messages are needed
- `git status --short` instead of verbose status

### Rust
- `cargo build 2>&1 | head -30` on first pass — expand only if needed
- `cargo test -- --format terse` for compact test output
- `cargo clippy --message-format short` for compact lint output
