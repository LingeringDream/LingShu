#!/bin/sh
# Install a prebuilt LingShu CLI bundle. No Rust, Node, Python or root required.
set -eu
prefix=${LINGSHU_INSTALL_PREFIX:-"$HOME/.local/share/lingshu-cli"}
bin_dir=${LINGSHU_BIN_DIR:-"$HOME/.local/bin"}
version=latest
archive=
uninstall=0
purge_data=
while [ "$#" -gt 0 ]; do
    case "$1" in
        --prefix|--bin-dir|--version|--archive|--purge-data)
            [ "$#" -ge 2 ] || { echo "Missing value for $1" >&2; exit 1; }
            case "$1" in
                --prefix) prefix=$2;; --bin-dir) bin_dir=$2;; --version) version=$2;;
                --archive) archive=$2;; --purge-data) purge_data=$2;;
            esac
            shift 2;;
        --uninstall) uninstall=1; shift;;
        -h|--help)
            echo 'Usage: sh install.sh [--version vX.Y.Z] [--archive FILE] [--prefix DIR] [--bin-dir DIR]'
            echo 'Offline archives require their adjacent .sha256 file. Uninstall retains data.'
            exit 0;;
        *) echo "Unknown option: $1" >&2; exit 1;;
    esac
done
case "$prefix" in /*) ;; *) echo '--prefix must be absolute' >&2; exit 1;; esac
case "$bin_dir" in /*) ;; *) echo '--bin-dir must be absolute' >&2; exit 1;; esac
[ "$prefix" != / ] && [ "$prefix" != "$HOME" ] || { echo 'Unsafe install prefix' >&2; exit 1; }
case "$version" in *[!a-zA-Z0-9._+-]*|'') echo 'Invalid version' >&2; exit 1;; esac
if [ "$uninstall" = 1 ]; then
    [ -f "$prefix/.lingshu-install" ] && [ "$(sed -n '1p' "$prefix/.lingshu-install")" = lingshu-cli-v1 ] || { echo 'Not a managed LingShu installation' >&2; exit 1; }
    bin_dir=$(sed -n '2p' "$prefix/.lingshu-install")
    if [ -x "$prefix/current/lingshu" ]; then "$prefix/current/lingshu" stop; fi
    # Copy this script out of the release directory before CLI calls it.
    if [ -L "$bin_dir/lingshu" ] && [ "$(readlink "$bin_dir/lingshu")" = "$prefix/current/lingshu" ]; then rm "$bin_dir/lingshu"; fi
    if [ -n "$purge_data" ]; then
        [ -f "$purge_data/config.json" ] && [ -n "$(sed -n '/"application": "lingshu-cli"/p' "$purge_data/config.json")" ] || { echo 'Unrecognized data directory; refusing purge' >&2; exit 1; }
        [ "$purge_data" != / ] && [ "$purge_data" != "$HOME" ] || exit 1
        rm -rf -- "$purge_data"
    fi
    rm -rf -- "$prefix"
    echo 'LingShu CLI uninstalled. Data retained unless --purge-data was supplied.'
    exit 0
fi
command -v tar >/dev/null || { echo 'tar is required' >&2; exit 1; }
case "$(uname -s):$(uname -m)" in
    Linux:x86_64) target=x86_64-unknown-linux-gnu;;
    Darwin:arm64) target=aarch64-apple-darwin;;
    Darwin:x86_64) target=x86_64-apple-darwin;;
    *) echo 'Supported: Linux x64, macOS Apple Silicon/Intel. Windows uses install.ps1.' >&2; exit 1;;
esac
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT HUP INT TERM
if [ -z "$archive" ]; then
    archive="$work/lingshu-$target.tar.gz"
    if [ -z "${LINGSHU_DOWNLOAD_BASE_URL:-}" ] && command -v gh >/dev/null && gh auth status >/dev/null 2>&1; then
        if [ "$version" = latest ]; then
            gh release download --repo LingeringDream/LingShu --pattern "lingshu-$target.tar.gz" --pattern "lingshu-$target.tar.gz.sha256" --dir "$work"
        else
            case "$version" in v*) ;; *) version="v$version";; esac
            gh release download "$version" --repo LingeringDream/LingShu --pattern "lingshu-$target.tar.gz" --pattern "lingshu-$target.tar.gz.sha256" --dir "$work"
        fi
    else
        command -v curl >/dev/null || { echo 'curl is required, or install and authenticate gh for private releases' >&2; exit 1; }
        base=${LINGSHU_DOWNLOAD_BASE_URL:-https://github.com/LingeringDream/LingShu/releases}
        if [ "$version" = latest ]; then base="$base/latest/download";
        else case "$version" in v*) ;; *) version="v$version";; esac; base="$base/download/$version"; fi
        download() {
            case "$1" in
                https://github.com/*|https://api.github.com/*)
                    if [ -n "${GH_TOKEN:-}" ]; then
                        curl --fail --location --retry 2 --header "Authorization: Bearer $GH_TOKEN" "$1" --output "$2"
                        return
                    fi;;
            esac
            curl --fail --location --retry 2 "$1" --output "$2"
        }

        download "$base/lingshu-$target.tar.gz" "$archive"
        download "$base/lingshu-$target.tar.gz.sha256" "$archive.sha256"
    fi
fi
[ -f "$archive" ] && [ -f "$archive.sha256" ] || { echo 'Archive and its .sha256 file are required' >&2; exit 1; }
expected=$(awk 'NR==1 {print tolower($1)}' "$archive.sha256")
case "$expected" in *[!0-9a-f]*|'') echo 'Invalid SHA-256 file' >&2; exit 1;; esac
[ "${#expected}" -eq 64 ] || { echo 'Invalid SHA-256 length' >&2; exit 1; }
if command -v sha256sum >/dev/null; then actual=$(sha256sum "$archive" | awk '{print $1}');
else actual=$(shasum -a 256 "$archive" | awk '{print $1}'); fi
[ "$actual" = "$expected" ] || { echo 'SHA-256 mismatch; existing installation was not changed' >&2; exit 1; }
# Only regular files/directories are accepted; reject traversal and link entries.
tar -tzf "$archive" > "$work/files"
while IFS= read -r entry; do
    case "$entry" in /*|../*|*/../*|*/..|..) echo 'Unsafe archive path' >&2; exit 1;; esac
done < "$work/files"
if tar -tvzf "$archive" | awk 'substr($1,1,1) != "-" && substr($1,1,1) != "d" {bad=1} END {exit !bad}'; then echo 'Archive links/special files are not supported' >&2; exit 1; fi
mkdir "$work/release"
tar -xzf "$archive" -C "$work/release"
for file in lingshu lingshu-server lingshu-mcp web/index.html VERSION TARGET install.sh release.json; do
    [ -f "$work/release/$file" ] || { echo "Bundle missing $file" >&2; exit 1; }
done
chmod +x "$work/release/lingshu" "$work/release/lingshu-server" "$work/release/lingshu-mcp"
[ "$(cat "$work/release/TARGET")" = "$target" ] || { echo 'Bundle target does not match this platform' >&2; exit 1; }
bundle_version=$(cat "$work/release/VERSION")
case "$bundle_version" in *[!a-zA-Z0-9._+-]*|'') echo 'Invalid bundle version' >&2; exit 1;; esac
if [ "$target" = x86_64-unknown-linux-gnu ] && [ -f "$work/release/GLIBC_MIN" ]; then
    minimum=$(cat "$work/release/GLIBC_MIN")
    available=$(getconf GNU_LIBC_VERSION 2>/dev/null | awk '{print $2}')
    if ! awk -v have="$available" -v need="$minimum" 'BEGIN {
        if (have !~ /^[0-9]+[.][0-9]+/ || need !~ /^[0-9]+[.][0-9]+/) exit 1
        n=split(have,h,"."); m=split(need,r,"."); count=n>m?n:m
        for (i=1;i<=count;i++) { if (h[i]+0>r[i]+0) exit 0; if (h[i]+0<r[i]+0) exit 1 }
        exit 0
    }'; then echo "This bundle needs glibc $minimum or newer (found ${available:-none}); installation was not changed." >&2; exit 1; fi
fi
[ "$("$work/release/lingshu" --version)" = "lingshu $bundle_version" ] || { echo 'Binary/bundle version mismatch' >&2; exit 1; }
if [ -n "$(find "$prefix" -mindepth 1 -maxdepth 1 -print 2>/dev/null || true)" ]; then
    [ -f "$prefix/.lingshu-install" ] && [ "$(sed -n '1p' "$prefix/.lingshu-install")" = lingshu-cli-v1 ] || { echo 'Install prefix is nonempty and unmanaged; choose an empty directory' >&2; exit 1; }
    bin_dir=$(sed -n '2p' "$prefix/.lingshu-install")
fi
mkdir -p "$bin_dir"
if [ -e "$bin_dir/lingshu" ] || [ -L "$bin_dir/lingshu" ]; then
    [ -L "$bin_dir/lingshu" ] && [ "$(readlink "$bin_dir/lingshu")" = "$prefix/current/lingshu" ] || { echo 'Existing lingshu command is unmanaged; choose another --bin-dir' >&2; exit 1; }
fi
if [ -x "$prefix/current/lingshu" ]; then "$prefix/current/lingshu" stop; fi
mkdir -p "$prefix/releases"
release="$prefix/releases/$bundle_version-$(date +%s)-$$"
mv "$work/release" "$release"
printf 'lingshu-cli-v1\n%s\n' "$bin_dir" > "$prefix/.lingshu-install"
printf '{"application":"lingshu-cli"}\n' > "$prefix/install.json"
ln -s "$release" "$prefix/.current-$$"
case "$(uname -s)" in Darwin) mv -fh "$prefix/.current-$$" "$prefix/current";; *) mv -fT "$prefix/.current-$$" "$prefix/current";; esac
ln -sf "$prefix/current/lingshu" "$bin_dir/lingshu"
echo "Installed LingShu $bundle_version: $bin_dir/lingshu"
case ":$PATH:" in *":$bin_dir:"*) ;; *) echo "Add $bin_dir to PATH, or invoke $bin_dir/lingshu directly.";; esac
echo 'Next: lingshu start; lingshu connect cursor; lingshu open'
