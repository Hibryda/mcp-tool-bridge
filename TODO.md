# TODO

## High Priority

- [ ] Choose MCP crate (rmcp vs mcp-server) — evaluate both, prototype hello-world server
- [ ] Implement bridge-core: MCP server setup, tool registration, process execution utilities
- [ ] Implement `ls` wrapper — highest usage (4,273 calls), structured dir metadata output
- [ ] Implement `wc` wrapper — second highest net-new value (1,489 calls), trivial to wrap
- [ ] Implement `curl` wrapper — structured HTTP response (headers + body + status)

## Medium Priority

- [ ] Implement `ps` wrapper — structured process listing
- [ ] Design tool composition model (e.g., ls results piped to wc)
- [ ] Decide: one MCP server with multiple tools vs separate servers per tool

## Low Priority

- [ ] Implement `sqlite3` wrapper — structured query results
- [ ] Large output handling strategy (streaming, pagination, truncation)
- [ ] Benchmark MCP overhead vs direct CLI

## Completed

(none yet)
