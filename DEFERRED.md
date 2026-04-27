# Deferred Tools

Tools considered but not added to mcp-tool-bridge, with the reason and a dual-criterion revisit threshold.

## Revisit Criteria

A deferred tool will be revisited when **either** of:
- **(a) Usage data:** `--log-calls` JSONL shows >200 invocations of the underlying CLI/operation per week for 4+ consecutive weeks
- **(b) Quality issue:** A GitHub issue includes a concrete use case with frequency data and parsing-difficulty justification

Issues without usage data will be labeled `needs-data` and closed with a metrics-collection suggestion.

## Deferred Tools

### `psql` (PostgreSQL CLI)
- **Frequency:** 55 calls (0.08% of 72K Bash calls)
- **Reason:** Frequency too low. Adding `tokio-postgres` brings TCP connection management, TLS, connection pooling, and CI infrastructure cost. The 5,000-token schema overhead per session is not justified by ~1 call per session for a single user.
- **Note:** sqlite_query covers structured DB queries for the file-based case. Postgres is a meaningfully different deployment model (network protocol, auth, multi-tenancy).

### `ssh` (Remote execution)
- **Frequency:** 1,189 calls (1.7%)
- **Reason:** Security complexity. Existing SSH MCP server with 37 tools already covers the use case. Wrapping ssh ourselves means handling key management, known_hosts, agent forwarding, and prompt redirection — all error-prone and security-sensitive.
- **Alternative:** Use the existing SSH MCP server.

### `helm` (Kubernetes package manager)
- **Frequency:** 68 calls (0.09%)
- **Reason:** Frequency too low. helm already supports `-o json` natively, so wrapping it adds little value (no parsing-difficulty problem). The technical claim that helm needs structured wrapping was incorrect.
- **Workaround:** Use `kubectl_list` for Helm-deployed resources, or call `helm` directly via Bash for the rare case.

### File operations: `rm`, `mkdir`, `cp`, `chmod`, `mv`
- **Combined frequency:** 1,764 calls (2.4%)
- **Reason:** These tools fail the value formula entirely (parsing-difficulty ≈ 0, error-cost ≈ 0). Their value is not in structured output but in **safety guardrails** — confirmation prompts, audit logging, path validation, undo. That is a fundamentally different product than mcp-tool-bridge ("safe agent execution" vs "structured CLI output").
- **Decision:** Out of scope. Could be a separate project (`mcp-safe-fs` or similar).

### `ss` (Socket statistics)
- **Frequency:** 78 calls (0.11%)
- **Reason:** Marginal value over `lsof`. ss shows socket states (TIME_WAIT, ESTABLISHED) but most agent workflows just need "what's listening on this port" which lsof covers.
- **Revisit if:** Agents demonstrably need socket state information (e.g., for connection-leak debugging) at >200 calls/week.

### `du` / `dust` (Disk usage)
- **Frequency:** 40 calls combined (0.06%)
- **Reason:** Frequency too low. `find` with size filters partially covers the use case for "files larger than N".
- **Note:** Tree-style aggregation (du -sh per directory) is not in `find`. If this becomes common, a `disk_usage` tool could be added.

### `journalctl` (Systemd logs)
- **Frequency:** 27 calls (0.04%)
- **Reason:** Frequency too low. Linux-only (no macOS analog), increasing portability burden.

### `tree` (Directory tree)
- **Frequency:** 14 calls (0.02%)
- **Reason:** `find` covers this use case with type/depth filters. Adding tree would duplicate functionality.

### `stat` (File metadata)
- **Frequency:** 16 calls (0.02%)
- **Reason:** `ls --long` already returns size, permissions, modified time. The extra fields (inode, blocks, access time) are rarely needed by agents.

### `rsync` (File sync)
- **Frequency:** 15 calls (0.02%)
- **Reason:** Frequency too low. Wrapping rsync's progress/dry-run/delete semantics correctly is non-trivial.

### `openssl` (Certificate inspection)
- **Frequency:** 5 calls (0.007%)
- **Reason:** Far below frequency threshold. The `curl` tool already exposes TLS handshake timing.

### `jq` (JSON processing)
- **Frequency:** 5 calls (0.007%)
- **Reason:** Counterintuitively low. Most tools in the bridge return JSON natively, and Claude can process JSON in-context without a tool call.

### `strace` (Syscall tracing)
- **Frequency:** 5 calls (0.007%)
- **Reason:** Highly specialized, Linux-only, output is voluminous and hard to structure usefully.

### Native API alternatives (`git2`, `tokio-postgres`)
- **Reason rejected:** Adversarial debate (4 rounds, 46 objections) showed CLI wrapping is simpler and sufficient. `git2`/libgit2 brings ~30MB dependency, async lifetime pain in Rust, and proc parsing complexity for git_log output. CLI wrapping with forced UTF-8 encoding (`-c i18n.logOutputEncoding=UTF-8 -c core.quotePath=false`) and STX/ETX sentinels achieves equivalent reliability at a fraction of the cost.

## Tool Ceiling

- **Current:** 20 tools
- **Soft warning:** 21 tools (see `TODO.md`)
- **Hard ceiling:** 25 tools

Beyond 21, schema overhead (~200-400 tokens per tool per conversation) consumes >25% of typical context budget. The `--tools` flag mitigates but requires user knowledge.

When approaching the soft ceiling, review this document for deprecation candidates rather than expanding.
