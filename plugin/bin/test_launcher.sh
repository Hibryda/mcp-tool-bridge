#!/bin/sh
# POSIX-shell tests for plugin/bin/launcher.sh.
#
# Exercises:
#   1. Argument validation (missing / unknown binary names)
#   2. MCP_TOOL_BRIDGE_BIN_DIR override (skips download path entirely)
#   3. Warm cache: cached binary used directly
#   4. Cold cache: download from a local HTTP server, verify SHA, extract, exec
#   5. Cache hit after cold path
#   6. Tampered tarball (bad checksum) is rejected with exit 66
set -eu

LAUNCHER="$(cd "$(dirname "$0")" && pwd)/launcher.sh"
TMP="$(mktemp -d)"
HTTP_PID=""
cleanup() {
    [ -n "$HTTP_PID" ] && kill "$HTTP_PID" 2>/dev/null || true
    rm -rf "$TMP"
}
trap cleanup EXIT INT TERM HUP

OS="$(uname -s)"
ARCH="$(uname -m)"
case "$OS" in
    Linux)
        case "$ARCH" in
            x86_64|amd64)  TRIPLE="x86_64-unknown-linux-gnu" ;;
            aarch64|arm64) TRIPLE="aarch64-unknown-linux-gnu" ;;
            *) echo "test_launcher.sh: unsupported test arch '$ARCH'" >&2; exit 1 ;;
        esac
        ;;
    Darwin)
        case "$ARCH" in
            x86_64)        TRIPLE="x86_64-apple-darwin" ;;
            arm64|aarch64) TRIPLE="aarch64-apple-darwin" ;;
            *) echo "test_launcher.sh: unsupported test arch '$ARCH'" >&2; exit 1 ;;
        esac
        ;;
    *) echo "test_launcher.sh: unsupported test OS '$OS'" >&2; exit 1 ;;
esac

VERSION="0.1.0"

new_plugin_root() {
    PR="$1"
    mkdir -p "$PR/bin"
    cp "$LAUNCHER" "$PR/bin/launcher.sh"
    chmod +x "$PR/bin/launcher.sh"
    printf "%s\n" "$VERSION" > "$PR/version"
}

assert() {
    if ! eval "$1"; then
        echo "FAIL: $2"
        exit 1
    fi
    echo "PASS: $2"
}

# ── 1. argument validation ─────────────────────────────────────────
PR1="$TMP/pr1"
new_plugin_root "$PR1"
rc=0
"$PR1/bin/launcher.sh" >/dev/null 2>&1 || rc=$?
assert "[ $rc -eq 68 ]" "empty args -> exit 68 (got $rc)"

rc=0
CLAUDE_PLUGIN_ROOT="$PR1" "$PR1/bin/launcher.sh" not-a-binary >/dev/null 2>&1 || rc=$?
assert "[ $rc -eq 68 ]" "unknown bin name -> exit 68 (got $rc)"

# ── 2. MCP_TOOL_BRIDGE_BIN_DIR override ────────────────────────────
PR2="$TMP/pr2"
new_plugin_root "$PR2"
DEV_BINS="$TMP/dev-bins"
mkdir "$DEV_BINS"
cat > "$DEV_BINS/mcp-tool-bridge" <<'EOF'
#!/bin/sh
echo "from-bin-dir $@"
EOF
chmod +x "$DEV_BINS/mcp-tool-bridge"
out="$(MCP_TOOL_BRIDGE_BIN_DIR="$DEV_BINS" CLAUDE_PLUGIN_ROOT="$PR2" "$PR2/bin/launcher.sh" mcp-tool-bridge ARG1)"
assert "[ \"$out\" = \"from-bin-dir ARG1\" ]" "BIN_DIR override executes local binary"

# ── 3. warm cache ──────────────────────────────────────────────────
PR3="$TMP/pr3"
new_plugin_root "$PR3"
CACHE="$PR3/.bin-cache/v$VERSION/$TRIPLE"
mkdir -p "$CACHE"
cat > "$CACHE/mcp-tool-bridge" <<'EOF'
#!/bin/sh
echo "from-cache $@"
EOF
chmod +x "$CACHE/mcp-tool-bridge"
out="$(CLAUDE_PLUGIN_ROOT="$PR3" "$PR3/bin/launcher.sh" mcp-tool-bridge HELLO)"
assert "[ \"$out\" = \"from-cache HELLO\" ]" "warm cache fast-path"

# ── 4./5. cold cache via local HTTP server ─────────────────────────
PR4="$TMP/pr4"
new_plugin_root "$PR4"
RELEASE_ROOT="$TMP/release-host"
RELEASE_DIR="$RELEASE_ROOT/v$VERSION"
mkdir -p "$RELEASE_DIR"
STAGE="$TMP/stage"
mkdir "$STAGE"
cat > "$STAGE/mcp-tool-bridge" <<'EOF'
#!/bin/sh
echo "from-tarball $@"
EOF
cat > "$STAGE/mcp-tool-bridge-hook" <<'EOF'
#!/bin/sh
echo "from-tarball-hook $@"
EOF
chmod +x "$STAGE/mcp-tool-bridge" "$STAGE/mcp-tool-bridge-hook"
TARBALL="mcp-tool-bridge-v${VERSION}-${TRIPLE}.tar.gz"
( cd "$STAGE" && tar -czf "$RELEASE_DIR/$TARBALL" mcp-tool-bridge mcp-tool-bridge-hook )

if command -v sha256sum >/dev/null 2>&1; then
    EXPECTED_SHA="$(sha256sum "$RELEASE_DIR/$TARBALL" | awk '{print $1}')"
else
    EXPECTED_SHA="$(shasum -a 256 "$RELEASE_DIR/$TARBALL" | awk '{print $1}')"
fi
printf "%s  %s\n" "$EXPECTED_SHA" "$TARBALL" > "$PR4/checksums.txt"

# Local HTTP server. Bind to a free ephemeral port; print it for the test.
PORT_FILE="$TMP/port"
python3 - "$RELEASE_ROOT" "$PORT_FILE" <<'PYEOF' &
import http.server, socketserver, sys, os
root, port_file = sys.argv[1], sys.argv[2]
os.chdir(root)
class Q(http.server.SimpleHTTPRequestHandler):
    def log_message(self, *a, **k): pass
with socketserver.TCPServer(("127.0.0.1", 0), Q) as s:
    open(port_file, "w").write(str(s.server_address[1]))
    s.serve_forever()
PYEOF
HTTP_PID=$!

# Wait for the port to be written.
for _ in $(seq 1 50); do
    [ -s "$PORT_FILE" ] && break
    sleep 0.1
done
PORT="$(cat "$PORT_FILE")"
[ -n "$PORT" ] || { echo "FAIL: HTTP server did not start"; exit 1; }

out="$(MCP_TOOL_BRIDGE_RELEASE_BASE_URL="http://127.0.0.1:${PORT}" CLAUDE_PLUGIN_ROOT="$PR4" "$PR4/bin/launcher.sh" mcp-tool-bridge VIA_DOWNLOAD)"
assert "[ \"$out\" = \"from-tarball VIA_DOWNLOAD\" ]" "cold path: download + verify + exec"

out="$(MCP_TOOL_BRIDGE_RELEASE_BASE_URL="http://127.0.0.1:${PORT}" CLAUDE_PLUGIN_ROOT="$PR4" "$PR4/bin/launcher.sh" mcp-tool-bridge AGAIN)"
assert "[ \"$out\" = \"from-tarball AGAIN\" ]" "cache hit after cold path"

out="$(MCP_TOOL_BRIDGE_RELEASE_BASE_URL="http://127.0.0.1:${PORT}" CLAUDE_PLUGIN_ROOT="$PR4" "$PR4/bin/launcher.sh" mcp-tool-bridge-hook HOOK)"
assert "[ \"$out\" = \"from-tarball-hook HOOK\" ]" "hook binary exec from same tarball"

# ── 6. tampered checksum ───────────────────────────────────────────
PR5="$TMP/pr5"
new_plugin_root "$PR5"
printf "%s  %s\n" "0000000000000000000000000000000000000000000000000000000000000000" "$TARBALL" > "$PR5/checksums.txt"
rc=0
MCP_TOOL_BRIDGE_RELEASE_BASE_URL="http://127.0.0.1:${PORT}" CLAUDE_PLUGIN_ROOT="$PR5" "$PR5/bin/launcher.sh" mcp-tool-bridge >/dev/null 2>&1 || rc=$?
assert "[ $rc -eq 66 ]" "bad checksum -> exit 66 (got $rc)"

echo
echo "ALL TESTS PASSED"
