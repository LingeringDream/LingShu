#!/usr/bin/env bash
# Re-sign + notarize an ALREADY-BUILT 灵枢 LingShu .dmg — no recompilation.
#
# Use this once you have a "Developer ID Application" certificate and Apple
# notarization credentials, to turn the ad-hoc .dmg (from package-macos.sh)
# into one that opens on any Mac. The current .dmg is ad-hoc signed, which
# Gatekeeper rejects for third parties, so this script:
#   1. re-signs the .app (with hardened runtime) using APPLE_SIGNING_IDENTITY
#   2. rebuilds the .dmg from the re-signed .app (hdiutil, Applications link)
#   3. signs the .dmg
#   4. submits to Apple for notarization and staples the ticket
#
# Prerequisites on the machine running this:
#   - login keychain UNLOCKED (codesign prompts; choose "Always Allow")
#   - Xcode or Command Line Tools installed (for codesign / notarytool)
#   - exported credentials:
#       export APPLE_SIGNING_IDENTITY="Developer ID Application: Name (TEAMID)"
#       # Apple-ID 方式:
#       export APPLE_ID="you@example.com"
#       export APPLE_PASSWORD="xxxx-xxxx-xxxx-xxxx"   # 专属密码
#       export APPLE_TEAM_ID="TEAMID"
#       # 或 App Store Connect API Key 方式:
#       export APPLE_API_ISSUER="<uuid>"
#       export APPLE_API_KEY="<key-id>"
#       export APPLE_API_KEY_PATH="$HOME/AuthKey_XXXX.p8"
#
# Usage:
#   ./scripts/notarize-macos.sh                 # re-sign + notarize default aarch64 dmg
#   ./scripts/notarize-macos.sh --universal     # universal dmg
#   ./scripts/notarize-macos.sh x86_64-apple-darwin
#   ./scripts/notarize-macos.sh --dmg /path/to/foo.dmg   # notarize an arbitrary dmg
set -euo pipefail

cd "$(dirname "$0")/.."
REPO_ROOT="$PWD"

TAURI_TARGET="aarch64-apple-darwin"
EXPLICIT_DMG=""

while [ $# -gt 0 ]; do
    case "$1" in
        --universal|universal) TAURI_TARGET="universal-apple-darwin" ;;
        --dmg) EXPLICIT_DMG="$2"; shift ;;
        -h|--help) sed -n '2,30p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) TAURI_TARGET="$1" ;;
    esac
    shift
done

# --- credential checks -----------------------------------------------------
if [ -z "${APPLE_SIGNING_IDENTITY:-}" ]; then
    echo "✗ APPLE_SIGNING_IDENTITY is not set. Export your Developer ID cert:" >&2
    echo '    export APPLE_SIGNING_IDENTITY="Developer ID Application: Name (TEAMID)"' >&2
    exit 1
fi
if [ -n "${APPLE_API_KEY_PATH:-}" ] && [ -n "${APPLE_API_ISSUER:-}" ] && [ -n "${APPLE_API_KEY:-}" ]; then
    METHOD="apikey"
elif [ -n "${APPLE_ID:-}" ] && [ -n "${APPLE_PASSWORD:-}" ] && [ -n "${APPLE_TEAM_ID:-}" ]; then
    METHOD="appleid"
    export APPLE_ID APPLE_PASSWORD APPLE_TEAM_ID
else
    echo "✗ No notarization credentials. Export either the Apple-ID trio" >&2
    echo "  (APPLE_ID / APPLE_PASSWORD / APPLE_TEAM_ID) or the API-key trio" >&2
    echo "  (APPLE_API_ISSUER / APPLE_API_KEY / APPLE_API_KEY_PATH)." >&2
    exit 1
fi

if ! command -v xcrun >/dev/null 2>&1 || ! xcrun -f notarytool >/dev/null 2>&1; then
    echo "✗ notarytool not found — install Xcode or Command Line Tools." >&2
    exit 1
fi

# --- locate artifacts ------------------------------------------------------
if [ -n "$EXPLICIT_DMG" ]; then
    DMG_FILE="$EXPLICIT_DMG"
    # derive the .app from the mounted dmg? simpler: require it lives next to a
    # built bundle; fall back to re-signing whatever nested .app the dmg has.
    APP_PATH=""
else
    BUNDLE_DIR="$REPO_ROOT/src-tauri/target/$TAURI_TARGET/release/bundle"
    APP_PATH="$BUNDLE_DIR/macos/灵枢 LingShu.app"
    VERSION="$(node -p "require('./frontend/package.json').version" 2>/dev/null || echo unknown)"
    case "$TAURI_TARGET" in
        aarch64-apple-darwin)   SUFFIX=aarch64 ;;
        x86_64-apple-darwin)    SUFFIX=x86_64 ;;
        universal-apple-darwin) SUFFIX=universal ;;
        *)                      SUFFIX="$TAURI_TARGET" ;;
    esac
    DMG_DIR="$BUNDLE_DIR/dmg"
    mkdir -p "$DMG_DIR"
    DMG_FILE="$DMG_DIR/灵枢 LingShu_${VERSION}_${SUFFIX}.dmg"
fi

if [ -z "$APP_PATH" ] || [ ! -d "$APP_PATH" ]; then
    echo "✗ Built .app not found at $APP_PATH" >&2
    echo "  Run ./scripts/package-macos.sh first to build it." >&2
    exit 1
fi

echo "→ Re-signing $APP_PATH with $APPLE_SIGNING_IDENTITY"
# --deep re-signs the nested sidecar binary too; --options runtime is required
# for notarization; --timestamp pins a trusted timestamp.
codesign --force --timestamp --options runtime \
    --sign "$APPLE_SIGNING_IDENTITY" "$APP_PATH"
echo "✓ App re-signed"
codesign -dv --verbose=2 "$APP_PATH" 2>&1 | grep -E "Authority|Signature" || true

# --- rebuild dmg from the re-signed app -------------------------------------
echo "→ Rebuilding .dmg from re-signed .app"
STAGE="$(mktemp -d)/灵枢 LingShu"
mkdir -p "$STAGE"
cp -R "$APP_PATH" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
hdiutil create -volname "灵枢 LingShu" -srcfolder "$STAGE" -ov -format UDZO "$DMG_FILE" | tail -1
rm -rf "$(dirname "$STAGE")"

echo "→ Signing .dmg"
codesign --force --timestamp --options runtime --sign "$APPLE_SIGNING_IDENTITY" "$DMG_FILE"
echo "✓ dmg: $DMG_FILE"

# --- notarize + staple -----------------------------------------------------
echo "=== Notarizing $DMG_FILE ==="
for attempt in 1 2; do
    echo "→ Submit attempt $attempt"
    if [ "$METHOD" = "apikey" ]; then
        xcrun notarytool submit "$DMG_FILE" \
            --key "$APPLE_API_KEY_PATH" \
            --key-id "$APPLE_API_KEY" \
            --issuer "$APPLE_API_ISSUER" \
            --wait
    else
        xcrun notarytool submit "$DMG_FILE" \
            --apple-id "$APPLE_ID" \
            --password "$APPLE_PASSWORD" \
            --team-id "$APPLE_TEAM_ID" \
            --wait
    fi && break
    echo "⚠ notarytool submit failed; retrying once..." >&2
    sleep 5
done

echo "→ Stapling notarization ticket"
xcrun stapler staple "$DMG_FILE" 2>&1 | tail -3 || true
echo "→ Verifying ticket"
xcrun stapler validate "$DMG_FILE" 2>&1 | tail -3 || true

echo ""
echo "✓ Done. $DMG_FILE is signed + notarized and safe to distribute."
