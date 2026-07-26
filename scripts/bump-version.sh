#!/usr/bin/env bash
# Bump the project version in one shot.
#
# Single source of truth: the workspace version in the root Cargo.toml.
# Everything else either derives from it (OpenAPI via CARGO_PKG_VERSION,
# tauri.conf.json via frontend/package.json, README badge via Cargo.toml) or
# is updated by this script. The only files that physically carry a literal
# version are the three manifests bumped below — plus CHANGELOG.md, which you
# write by hand.
#
# Usage:  scripts/bump-version.sh <semver>      e.g. scripts/bump-version.sh 1.2.0
# Can be run from anywhere — auto-locates the repository root.
set -euo pipefail

NEW="${1:-}"
if [ -z "$NEW" ]; then
    echo "usage: scripts/bump-version.sh <semver>   e.g. 1.2.0" >&2
    exit 1
fi
if ! [[ "$NEW" =~ ^[0-9]+\.[0-9]+\.[0-9]+([-+].*)?$ ]]; then
    echo "✗ not a valid semver: $NEW" >&2
    exit 1
fi

cd "$(dirname "$0")/.."

echo "→ Bumping to $NEW"

# 1) Root Cargo workspace version — THE source of truth.
#    [^\[] keeps the match inside the [workspace.package] table.
perl -0pi -e 's/(\[workspace\.package\][^\[]*?\bversion\s*=\s*")[^"]+(")/${1}'"$NEW"'${2}/s' Cargo.toml

# 2) src-tauri crate version (separate cargo workspace, can't inherit the root).
perl -0pi -e 's/(\[package\][^\[]*?\bversion\s*=\s*")[^"]+(")/${1}'"$NEW"'${2}/s' src-tauri/Cargo.toml

# 3) Frontend package.json (+ package-lock.json). tauri.conf.json reads from here.
( cd frontend && npm version "$NEW" --no-git-tag-version --allow-same-version >/dev/null )

# 4) Regenerate the committed OpenAPI contract. This also recompiles
#    lingshu-server, which refreshes the root Cargo.lock with the new version.
echo "→ Regenerating openapi.json"
cargo run -q -p lingshu-server -- --dump-openapi > openapi.json

echo "✓ Bumped to $NEW"
echo "  written:  Cargo.toml · src-tauri/Cargo.toml · frontend/package.json · openapi.json"
echo "  derived:  tauri.conf.json · OpenAPI info.version · README version badge"
echo ""
echo "Next:"
echo "  1. Add a CHANGELOG.md entry for $NEW"
echo "  2. git commit -am \"chore: release v$NEW\""
echo "  3. git tag v$NEW"
