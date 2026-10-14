#!/bin/sh
# Build a native package for the host architecture; no root required.
set -eu
root=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
cd "$root"
version=$(cat VERSION)
[ "$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n 1)" = "$version" ] || { echo 'Cargo.toml and VERSION differ' >&2; exit 1; }
dpkg --validate-version "$version"
cargo build --release --locked
arch=$(dpkg --print-architecture)
output=${1:-"$root/dist"}
mkdir -p "$output"
output=$(CDPATH='' cd -- "$output" && pwd)
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT HUP INT TERM
install -Dm755 target/release/termlook "$stage/usr/bin/termlook"
install -Dm644 packaging/debian/termlook.desktop "$stage/usr/share/applications/io.termlook.TermLook.desktop"
install -Dm644 assets/io.termlook.TermLook.svg "$stage/usr/share/icons/hicolor/scalable/apps/io.termlook.TermLook.svg"
install -Dm644 LICENSE "$stage/usr/share/doc/termlook/copyright"
mkdir -p "$stage/DEBIAN"
sed -e "s/@VERSION@/$version/g" -e "s/@ARCH@/$arch/g" packaging/debian/control > "$stage/DEBIAN/control"
dpkg-deb --root-owner-group --build "$stage" "$output/termlook_${version}_${arch}.deb"
