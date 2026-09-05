#!/usr/bin/env bash
# Build a fully distributable macOS bundle (.app + .dmg) for 灵枢 LingShu.
#
# The sidecar carries its own PostgreSQL runtime, so recipients do NOT need to
# install a database — they drag the app to /Applications and double-click.
#
# For the bundle to open on *other people's* Macs it must be code-signed with a
# "Developer ID Application" certificate AND notarized with Apple. That needs a
# paid Apple Developer Program membership (¥688/年) and a keychain that is
# UNLOCKED on the machine running this script. Export the credentials first:
#
#   export APPLE_SIGNING_IDENTITY="Developer ID Application: Your Name (TEAMID)"
#   export APPLE_TEAM_ID="TEAMID"                       # 10-char team id
#   # Apple-ID 方式:
#   export APPLE_ID="you@example.com"                  # Apple ID
#   export APPLE_PASSWORD="xxxx-xxxx-xxxx-xxxx"        # 专属密码 (app-specific)
#   # 或 App Store Connect API Key 方式 (更稳定, 推荐 CI):
#   export APPLE_API_ISSUER="<uuid>"                    # Issuer ID
#   export APPLE_API_KEY="<key-id>"                     # Key ID
#   export APPLE_API_KEY_PATH="$HOME/AuthKey_XXXX.p8"   # .p8 私钥路径
#
# 还需要: Xcode (含 notarytool) 或 Xcode Command Line Tools (`xcode-select
# --install`), 且登录钥匙串已解锁 (否则 codesign 报 errSecInternalComponent)。
#
# 不设置这些变量也能成功打包 —— 只是 ad-hoc 签名, 你自己机器可用, 别人机器
# 需右键→打开。
#
# Usage:
#   ./scripts/package-macos.sh                 # Apple Silicon (默认)
#   ./scripts/package-macos.sh --universal    # Apple Silicon + Intel (通用二进制)
#   ./scripts/package-macos.sh x86_64-apple-darwin
set -euo pipefail

cd "$(dirname "$0")/.."
REPO_ROOT="$PWD"

TAURI_TARGET="aarch64-apple-darwin"
SIDECAR_TARGETS=("aarch64-apple-darwin")

case "${1:-}" in
    --universal|universal)
        TAURI_TARGET="universal-apple-darwin"
        SIDECAR_TARGETS=("aarch64-apple-darwin" "x86_64-apple-darwin")
        ;;
    -h|--help)
        sed -n '2,34p' "$0" | sed 's/^# \{0,1\}//'
        exit 0
        ;;
    "")
        ;;
    *)
        TAURI_TARGET="$1"
        SIDECAR_TARGETS=("$1")
        ;;
esac

echo "→ Target: $TAURI_TARGET"

# A universal build needs both Rust targets installed.
if [ "$TAURI_TARGET" = "universal-apple-darwin" ]; then
    for target in "${SIDECAR_TARGETS[@]}"; do
        if ! rustup target list --installed | grep -qx "$target"; then
            echo "→ Installing Rust target $target"
            rustup target add "$target"
        fi
    done
fi

# The sidecar must exist before `tauri build` — it is listed in
# bundle.externalBin and is what makes the app self-contained.
for target in "${SIDECAR_TARGETS[@]}"; do
    echo "→ Building sidecar for $target"
    "$REPO_ROOT/scripts/build-sidecar.sh" "$target"
done

# Decide on the signing path. Tauri reads APPLE_SIGNING_IDENTITY from the
# environment; pass it through explicitly so the caller need not `export` it.
USE_SIGNING=0
if [ -n "${APPLE_SIGNING_IDENTITY:-}" ]; then
    USE_SIGNING=1
    export APPLE_SIGNING_IDENTITY
    echo "→ Signing: Developer ID ($APPLE_SIGNING_IDENTITY)"
else
    echo "→ Signing: ad-hoc (Gatekeeper will block other Macs)"
fi

# Credentials present ⇒ we will notarize after the build.
CAN_NOTARIZE=0
if [ -n "${APPLE_API_KEY_PATH:-}" ] && [ -n "${APPLE_API_ISSUER:-}" ] && [ -n "${APPLE_API_KEY:-}" ]; then
    CAN_NOTARIZE=1
    NOTARIZE_METHOD="apikey"
elif [ -n "${APPLE_ID:-}" ] && [ -n "${APPLE_PASSWORD:-}" ] && [ -n "${APPLE_TEAM_ID:-}" ]; then
    CAN_NOTARIZE=1
    NOTARIZE_METHOD="appleid"
    export APPLE_ID APPLE_PASSWORD APPLE_TEAM_ID
fi

if [ "$CAN_NOTARIZE" = "1" ]; then
    if ! command -v xcrun >/dev/null 2>&1 || ! xcrun -f notarytool >/dev/null 2>&1; then
        echo "⚠ notarytool not found (install Xcode or Command Line Tools)." >&2
        echo "⚠ The bundle will be signed but NOT notarized." >&2
        CAN_NOTARIZE=0
    else
        echo "→ Notarization: enabled ($NOTARIZE_METHOD)"
    fi
else
    echo "→ Notarization: skipped (no APPLE_* credentials)"
fi

cd "$REPO_ROOT/frontend"
npm run tauri -- build --bundles app --target "$TAURI_TARGET"

BUNDLE_DIR="$REPO_ROOT/src-tauri/target/$TAURI_TARGET/release/bundle"
APP_PATH="$BUNDLE_DIR/macos/灵枢 LingShu.app"
DMG_DIR="$BUNDLE_DIR/dmg"
mkdir -p "$DMG_DIR"
VERSION="$(node -p "require('./package.json').version" 2>/dev/null || echo unknown)"
case "$TAURI_TARGET" in
    aarch64-apple-darwin)   SUFFIX=aarch64 ;;
    x86_64-apple-darwin)    SUFFIX=x86_64 ;;
    universal-apple-darwin) SUFFIX=universal ;;
    *)                      SUFFIX="$TAURI_TARGET" ;;
esac
DMG_FILE="$DMG_DIR/灵枢 LingShu_${VERSION}_${SUFFIX}.dmg"

# Build the .dmg deterministically with hdiutil (a drag-to-Installations
# style image with an Applications symlink). hdiutil does not mount a
# writable volume the way Tauri's bundled create-dmg fork does, so this is
# robust in CI / sandboxed environments where volume mounting is blocked.
echo "→ Creating .dmg from the .app"
STAGE="$(mktemp -d)/灵枢 LingShu"
mkdir -p "$STAGE"
cp -R "$APP_PATH" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
hdiutil create -volname "灵枢 LingShu" -srcfolder "$STAGE" -ov -format UDZO "$DMG_FILE" \
    | tail -1
rm -rf "$(dirname "$STAGE")"

echo ""
echo "=== Build artifacts ==="
[ -d "$APP_PATH" ] && echo "app: $APP_PATH"
[ -f "$DMG_FILE" ] && echo "dmg: $DMG_FILE"

# Sanity check: the bundled sidecar must be inside the .app.
if [ -d "$APP_PATH" ]; then
    if [ -f "$APP_PATH/Contents/MacOS/lingshu-server" ]; then
        echo "✓ Sidecar bundled in Contents/MacOS"
    else
        echo "⚠ Sidecar NOT found in Contents/MacOS — the app will have no backend"
    fi

    echo ""
    echo "=== Code signature ==="
    codesign -dv --verbose=2 "$APP_PATH" 2>&1 | grep -E "Identifier|TeamIdentifier|Signature|Authority" || true
    spctl -a -vv -t exec "$APP_PATH" 2>&1 | tail -2 || true
fi

# Notarize + staple the .dmg (the part Gatekeeper actually checks on first
# launch). Tauri already signs; this submits to Apple and binds the ticket.
if [ "$CAN_NOTARIZE" = "1" ]; then
    if [ ! -f "$DMG_FILE" ]; then
        echo "⚠ No .dmg produced; skipping notarization."
    else
        echo ""
        echo "=== Notarizing $DMG_FILE ==="
        # Retry once on transient failures.
        for attempt in 1 2; do
            echo "→ Submit attempt $attempt"
            if [ "$NOTARIZE_METHOD" = "apikey" ]; then
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
    fi
fi

echo ""
if [ "$USE_SIGNING" = "1" ] && [ "$CAN_NOTARIZE" = "1" ]; then
    echo "✓ Signed + notarized — safe to distribute to anyone."
elif [ "$USE_SIGNING" = "1" ]; then
    echo "✓ Signed with Developer ID, but NOT notarized — Gatekeeper may still"
    echo "  warn on other Macs. Set APPLE_ID/APPLE_PASSWORD/APPLE_TEAM_ID (or"
    echo "  the API-key trio) and re-run to notarize."
else
    echo "⚠ Ad-hoc signed only. Recipients must right-click the app and choose"
    echo "  打开/Open the first time. To produce a bundle that opens on any Mac,"
    echo "  unlock your login keychain, export APPLE_SIGNING_IDENTITY + the"
    echo "  notarization credentials, then re-run this script."
fi
