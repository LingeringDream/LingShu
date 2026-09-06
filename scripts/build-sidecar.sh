#!/usr/bin/env bash
# Build the lingshu-server and lingshu-mcp sidecar binaries for Tauri bundles.
# Can be run from anywhere — auto-locates the repository root.
#
# By default the sidecar carries its own PostgreSQL runtime (cargo feature
# `desktop-embedded-db`), so end users never install a database. Pass
# --no-embedded-db for a lean dev binary that expects a running PostgreSQL.
#
# Usage: ./scripts/build-sidecar.sh [TARGET] [--no-embedded-db]
set -euo pipefail

cd "$(dirname "$0")/.."

EMBEDDED_DB=1
TARGET=""

while [ $# -gt 0 ]; do
    case "$1" in
        --no-embedded-db)
            EMBEDDED_DB=0
            ;;
        -h|--help)
            echo "Usage: ./scripts/build-sidecar.sh [TARGET] [--no-embedded-db]"
            echo ""
            echo "  TARGET            Rust target triple (default: host, e.g. aarch64-apple-darwin)"
            echo "  --no-embedded-db  Skip the bundled PostgreSQL runtime (dev/CI only)"
            exit 0
            ;;
        -*)
            echo "Unknown option: $1" >&2
            exit 1
            ;;
        *)
            TARGET="$1"
            ;;
    esac
    shift
done

if [ -z "$TARGET" ]; then
    TARGET=$(rustc -vV | sed -n 's/^host: //p')
fi

BIN_DIR="src-tauri/binaries"

echo "→ Building lingshu-server for $TARGET"

if [ "$EMBEDDED_DB" = "1" ]; then
    # The crate's build script downloads the PostgreSQL runtime from
    # github.com/zonkyio/embedded-postgres-binaries and embeds it, so the
    # packaged app is self-contained (~30 MB larger).
    cargo build -p lingshu-server --release --target "$TARGET" --features desktop-embedded-db
else
    echo "⚠ Building WITHOUT the embedded PostgreSQL runtime — the packaged app"
    echo "  will require a reachable PostgreSQL on the user's machine."
    cargo build -p lingshu-server --release --target "$TARGET"
fi

echo "→ Building lingshu-mcp for $TARGET"
cargo build -p lingshu-mcp --release --target "$TARGET"

mkdir -p "$BIN_DIR"
for binary in lingshu-server lingshu-mcp; do
    src="target/$TARGET/release/$binary"
    dst="$BIN_DIR/$binary-$TARGET"
    if [ ! -f "$src" ]; then
        echo "Missing built sidecar: $src" >&2
        exit 1
    fi
    cp "$src" "$dst"
    chmod +x "$dst"
    echo "✓ Sidecar binary placed at $dst"
done
echo ""
echo "Next: cd frontend && npm run tauri build"
