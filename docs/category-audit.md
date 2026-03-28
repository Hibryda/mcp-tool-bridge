# MCP Server Category Audit

**Date:** 2026-03-28
**Purpose:** Determine whether existing kubectl/docker MCP servers return structured JSON or raw text.

## Summary

| Server | Category | Tool | Output Format | Structured? |
|--------|----------|------|---------------|:-----------:|
| kubectl | LIST | `pods_list` | Columnar text table | No |
| kubectl | LIST | `resources_list` | Columnar text table | No |
| kubectl | DESCRIBE | `pods_get` | Raw YAML string | No |
| kubectl | DESCRIBE | `resources_get` | Raw YAML string | No |
| kubectl | LOG | `pods_log` | Raw text lines | No |
| kubectl | LOG | `events_list` | Text sentinel (`# No events found`) | No |
| kubectl | EXEC | `pods_exec` | Raw stdout passthrough | No |
| docker | LIST | `list-containers` | `ID - NAME - STATUS` text | No |
| docker | LOG | `get-logs` | Raw text + `Debug Info:` footer | No |
| docker | INSPECT | *(not available)* | N/A | N/A |
| docker | EXEC | *(not available)* | N/A | N/A |

## Findings

1. **kubectl MCP: 0/7 operations return structured JSON.** The K8s API natively returns JSON, but this server formats it into human-readable text. It actively destroys structure.

2. **Docker MCP: 0/2 operations return structured JSON.** Docker Engine API is JSON-native. The MCP server converts to lossy text. Limited to 4 tools — missing inspect, exec, stats, network.

3. **Neither server provides typed fields.** No operation returns keyed data. Consumers must parse whitespace-aligned columns or YAML.

## Implications

- **kubectl**: High value for mcp-tool-bridge. Can call K8s API directly (via `kubectl -o json`) and return typed metadata.
- **docker**: High value. bollard crate accesses Docker Engine API directly, returning native JSON.
- **sqlite3**: Not tested (no server configured). CLI output is pipe-delimited or column-aligned — would benefit from rusqlite wrapping.

## Decision

All three Tier 2 tools (kubectl, docker, sqlite3) are **confirmed in scope**. The category audit gate passes — existing servers return raw text for every tested operation.
