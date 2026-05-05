#!/bin/sh
# mcp-tool-bridge plugin launcher
#
# Resolves the right native binary for the host, downloading + caching on
# first use. Verifies SHA-256 against the checksums committed in the plugin.
#
# Usage:  launcher.sh <bin-name> [args...]
#   <bin-name>: mcp-tool-bridge | mcp-tool-bridge-hook
#
# Env overrides:
#   MCP_TOOL_BRIDGE_BIN_DIR        — skip download, exec from this directory
#                                    (useful for local development)
#   MCP_TOOL_BRIDGE_RELEASE_BASE_URL — release host base URL; defaults to
#                                    GitHub. Set this to point at a forge
#                                    mirror (Forgejo / Gitea / GitLab) when
#                                    deploying to an org that can't reach
#                                    github.com. The expected URL pattern is
#                                    `<base>/v<version>/<tarball>`.
#
# Exit codes:
#   0   success (after exec)
#   64  unsupported platform
#   65  download / extract failed
#   66  checksum mismatch
#   67  missing required tool (curl/wget, sha256sum/shasum, tar)
#   68  bad usage
set -eu

if [ $# -lt 1 ]; then
    echo "launcher.sh: usage: launcher.sh <mcp-tool-bridge|mcp-tool-bridge-hook> [args...]" >&2
    exit 68
fi

BIN_NAME="$1"
shift

case "$BIN_NAME" in
    mcp-tool-bridge|mcp-tool-bridge-hook) ;;
    *)
        echo "launcher.sh: unknown binary '$BIN_NAME' (expected mcp-tool-bridge or mcp-tool-bridge-hook)" >&2
        exit 68
        ;;
esac

# Dev override: exec the locally built binary if MCP_TOOL_BRIDGE_BIN_DIR is set.
if [ -n "${MCP_TOOL_BRIDGE_BIN_DIR:-}" ] && [ -x "$MCP_TOOL_BRIDGE_BIN_DIR/$BIN_NAME" ]; then
    exec "$MCP_TOOL_BRIDGE_BIN_DIR/$BIN_NAME" "$@"
fi

# Resolve plugin root. Claude Code sets CLAUDE_PLUGIN_ROOT; in tests we fall
# back to the script's grandparent (script lives at <root>/bin/launcher.sh).
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PLUGIN_ROOT="${CLAUDE_PLUGIN_ROOT:-$(dirname "$SCRIPT_DIR")}"

VERSION_FILE="$PLUGIN_ROOT/version"
CHECKSUMS_FILE="$PLUGIN_ROOT/checksums.txt"

if [ ! -f "$VERSION_FILE" ]; then
    echo "launcher.sh: missing $VERSION_FILE" >&2
    exit 65
fi

VERSION="$(tr -d '[:space:]' < "$VERSION_FILE")"

# ── platform detection ─────────────────────────────────────────────
detect_triple() {
    os="$(uname -s)"
    arch="$(uname -m)"
    case "$os" in
        Linux)
            case "$arch" in
                x86_64|amd64)   echo "x86_64-unknown-linux-gnu" ;;
                aarch64|arm64)  echo "aarch64-unknown-linux-gnu" ;;
                *) return 1 ;;
            esac
            ;;
        Darwin)
            case "$arch" in
                x86_64)         echo "x86_64-apple-darwin" ;;
                arm64|aarch64)  echo "aarch64-apple-darwin" ;;
                *) return 1 ;;
            esac
            ;;
        *) return 1 ;;
    esac
}

TRIPLE="$(detect_triple)" || {
    echo "launcher.sh: unsupported platform $(uname -ms)" >&2
    exit 64
}

CACHE_DIR="$PLUGIN_ROOT/.bin-cache/v$VERSION/$TRIPLE"
BIN_PATH="$CACHE_DIR/$BIN_NAME"

# Fast path: cached binary present.
if [ -x "$BIN_PATH" ]; then
    exec "$BIN_PATH" "$@"
fi

# ── cold cache: download + verify + extract ────────────────────────
require_tool() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "launcher.sh: missing required tool '$1'" >&2
        exit 67
    fi
}

require_tool tar

if [ ! -f "$CHECKSUMS_FILE" ]; then
    echo "launcher.sh: missing $CHECKSUMS_FILE — has the plugin been released?" >&2
    exit 65
fi

BASE_URL="${MCP_TOOL_BRIDGE_RELEASE_BASE_URL:-https://github.com/Hibryda/mcp-tool-bridge/releases/download}"
TARBALL="mcp-tool-bridge-v$VERSION-$TRIPLE.tar.gz"
URL="$BASE_URL/v$VERSION/$TARBALL"

EXPECTED_SHA="$(awk -v t="$TARBALL" '$2 == t { print $1 }' "$CHECKSUMS_FILE")"
if [ -z "$EXPECTED_SHA" ]; then
    echo "launcher.sh: no checksum entry for '$TARBALL' in $CHECKSUMS_FILE" >&2
    exit 66
fi

mkdir -p "$CACHE_DIR"
TMPDIR_LAUNCHER="$(mktemp -d 2>/dev/null || mktemp -d -t mcp-tool-bridge)"
trap 'rm -rf "$TMPDIR_LAUNCHER"' EXIT INT TERM HUP

# Download. Prefer curl, fall back to wget.
if command -v curl >/dev/null 2>&1; then
    if ! curl --fail --silent --show-error --location --output "$TMPDIR_LAUNCHER/$TARBALL" "$URL"; then
        echo "launcher.sh: download failed: $URL" >&2
        exit 65
    fi
elif command -v wget >/dev/null 2>&1; then
    if ! wget --quiet -O "$TMPDIR_LAUNCHER/$TARBALL" "$URL"; then
        echo "launcher.sh: download failed: $URL" >&2
        exit 65
    fi
else
    echo "launcher.sh: need curl or wget to fetch release artifacts" >&2
    exit 67
fi

# Verify SHA-256.
if command -v sha256sum >/dev/null 2>&1; then
    ACTUAL_SHA="$(sha256sum "$TMPDIR_LAUNCHER/$TARBALL" | awk '{print $1}')"
elif command -v shasum >/dev/null 2>&1; then
    ACTUAL_SHA="$(shasum -a 256 "$TMPDIR_LAUNCHER/$TARBALL" | awk '{print $1}')"
else
    echo "launcher.sh: need sha256sum or shasum to verify checksum" >&2
    exit 67
fi

if [ "$EXPECTED_SHA" != "$ACTUAL_SHA" ]; then
    echo "launcher.sh: checksum mismatch for $TARBALL" >&2
    echo "  expected: $EXPECTED_SHA" >&2
    echo "  actual:   $ACTUAL_SHA" >&2
    exit 66
fi

# Extract. Tarball is flat: <bin1> <bin2> at top level.
if ! tar -xzf "$TMPDIR_LAUNCHER/$TARBALL" -C "$CACHE_DIR"; then
    echo "launcher.sh: extract failed for $TARBALL" >&2
    exit 65
fi

if [ ! -x "$BIN_PATH" ]; then
    chmod +x "$CACHE_DIR/mcp-tool-bridge" "$CACHE_DIR/mcp-tool-bridge-hook" 2>/dev/null || true
fi

if [ ! -x "$BIN_PATH" ]; then
    echo "launcher.sh: $BIN_NAME not present in tarball" >&2
    exit 65
fi

exec "$BIN_PATH" "$@"
