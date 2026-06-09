#!/usr/bin/env bash
# Build the lingshu-server sidecar binary for bundling into the Tauri .app.
# Can be run from anywhere — auto-locates the repository root.
set -euo pipefail

cd "$(dirname "$0")/.."

TARGET="${1:-}"
if [ -z "$TARGET" ]; then
    TARGET=$(rustc -vV | sed -n 's/^host: //p')
fi
echo "→ Building lingshu-server for $TARGET"

cargo build -p lingshu-server --release --target "$TARGET"

BIN_DIR="src-tauri/binaries"
mkdir -p "$BIN_DIR"
SRC="target/$TARGET/release/lingshu-server"
DST="$BIN_DIR/lingshu-server-$TARGET"

cp "$SRC" "$DST"
chmod +x "$DST"

echo "✓ Sidecar binary placed at $DST"
echo ""
echo "Next: cd frontend && npm run tauri build"
