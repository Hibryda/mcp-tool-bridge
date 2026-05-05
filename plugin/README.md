# mcp-tool-bridge plugin

Claude Code plugin that bundles:

1. **MCP server** (`mcp-tool-bridge`) — 20 structured-output tools.
2. **PreToolUse hook** (`mcp-tool-bridge-hook`) — nudges the agent toward the
   structured equivalents when it reaches for plain Bash.

Native binaries are downloaded on first use from the configured release host
(GitHub by default) and cached under `${CLAUDE_PLUGIN_ROOT}/.bin-cache/`. SHA-256
checksums committed alongside the manifest are verified before any binary runs.

## Install

The repo doubles as a Claude Code marketplace. To install from the canonical
GitHub remote:

```sh
claude plugin marketplace add https://github.com/Hibryda/mcp-tool-bridge.git
claude plugin install mcp-tool-bridge@mcp-tool-bridge
```

(The `@mcp-tool-bridge` qualifier is the marketplace name; the manifest also
calls the plugin `mcp-tool-bridge`.)

To install from a self-hosted Forgejo / Gitea / GitLab mirror, point the same
command at the mirror URL.

## Hook modes

`MCP_BRIDGE_HOOK_MODE` selects the hook's behaviour:

| Mode | Behaviour |
|------|-----------|
| `suggest` (default) | Adds an `additionalContext` hint to the agent. Never blocks. |
| `enforce` | Blocks the Bash call (exit 2); the agent sees the hint via stderr. |
| `off` | Hook is a no-op. |

Set it in `~/.claude/settings.json` `env` block, or per-shell via `export`.

## What the hook covers

| Bash command | MCP tool |
|--------------|----------|
| `ls`, `ls -la`, `ls /path` | `mcp__tool-bridge__ls` |
| `wc`, `wc -l`, `wc -lwc files…` | `mcp__tool-bridge__wc` |
| `find <path> -name -type -maxdepth -size` | `mcp__tool-bridge__find` |
| `diff -u a b` (parser hint) | `mcp__tool-bridge__diff` |
| `lsof -i`, `-iTCP:port`, `-p PID` | `mcp__tool-bridge__lsof` |
| `ps`, `ps aux`, `ps -ef` | `mcp__tool-bridge__ps` |
| `git status`, `git status --porcelain` | `mcp__tool-bridge__git_status` |
| `git log` (default form, not `--oneline`) | `mcp__tool-bridge__git_log` |
| `git show <ref>` | `mcp__tool-bridge__git_show` |

Pipelines, redirections, `&&` chains, and unrecognised flags pass through
silently. The parser is conservative: a missed match is preferable to a
wrong rewrite.

## Distribution model

The plugin manifest (`plugin/.claude-plugin/plugin.json`) and the MCP +
hook configs (`plugin/.mcp.json`, `plugin/hooks/hooks.json`) reference a
launcher script (`plugin/bin/launcher.sh`). On first use the launcher:

1. Reads the version pinned in `plugin/version`.
2. Detects the host triple via `uname -ms` (supports
   `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`,
   `x86_64-apple-darwin`, `aarch64-apple-darwin`).
3. Downloads `mcp-tool-bridge-v<version>-<triple>.tar.gz` from
   `${MCP_TOOL_BRIDGE_RELEASE_BASE_URL}/v<version>/`, defaulting to the
   GitHub Releases URL.
4. Verifies the SHA-256 against `plugin/checksums.txt`.
5. Extracts to `${CLAUDE_PLUGIN_ROOT}/.bin-cache/v<version>/<triple>/`.
6. `exec`s the requested binary.

Subsequent invocations hit the warm cache (~5ms exec overhead).

## Self-hosting the release artifacts

For organisations that can't reach `github.com`, set
`MCP_TOOL_BRIDGE_RELEASE_BASE_URL` to your internal release host:

```json
// ~/.claude/settings.json
{
  "env": {
    "MCP_TOOL_BRIDGE_RELEASE_BASE_URL": "https://forge.internal.company/api/packages/.../releases/download"
  }
}
```

The expected URL pattern is `<base>/v<version>/<tarball>`. Forgejo, Gitea, and
GitLab Releases all serve assets with the same path structure as GitHub.

## Local development

To exec your locally built binaries instead of the cached download (useful for
hacking on the MCP server without bumping versions and re-releasing):

```sh
export MCP_TOOL_BRIDGE_BIN_DIR=/path/to/repo/target/release
```

The launcher checks this env var first and skips the download path entirely.

## Releasing

1. Bump `plugin/version` and create a matching tag:
   ```sh
   echo "0.2.0" > plugin/version
   git commit -am "chore: bump plugin to 0.2.0"
   git tag v0.2.0
   git push origin master v0.2.0
   ```
2. The `.github/workflows/release.yml` workflow:
   - Builds release tarballs for all four supported triples on native runners.
   - Uploads them + `checksums.txt` to GitHub Releases under the tag.
   - Auto-commits `plugin/checksums.txt` back to `master` so subsequent
     marketplace installs can verify the binaries.

## Running the launcher tests

```sh
./plugin/bin/test_launcher.sh
```

Eight POSIX-shell tests cover argument validation, dev-binary override,
warm-cache fast path, cold-cache download (against a local HTTP server),
cache hit after cold path, and tampered-checksum rejection.
