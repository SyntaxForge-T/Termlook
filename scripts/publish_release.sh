#!/bin/sh
# Publish only the tested artifact. Never replace an existing public release.
set -eu
version=$(cat VERSION)
printf '%s\n' "$version" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$'
repo=${GITHUB_REPOSITORY:?}
commit=${GITHUB_SHA:?}
tag="v$version"
release=$(mktemp)
error=$(mktemp)
trap 'rm -f "$release" "$error"' EXIT HUP INT TERM
if gh api "repos/$repo/releases/tags/$tag" > "$release" 2> "$error"; then
    if [ "$(jq -r .draft "$release")" = false ]; then printf '%s already published\n' "$tag"; exit 0; fi
    exists=true
elif grep -q 'HTTP 404' "$error"; then exists=false
else cat "$error" >&2; exit 1
fi
if gh api "repos/$repo/git/ref/tags/$tag" > "$release" 2> "$error"; then
    type=$(jq -r .object.type "$release")
    sha=$(jq -r .object.sha "$release")
    while [ "$type" = tag ]; do
        gh api "repos/$repo/git/tags/$sha" > "$release"
        type=$(jq -r .object.type "$release")
        sha=$(jq -r .object.sha "$release")
    done
    if [ "$type" != commit ] || [ "$sha" != "$commit" ]; then echo 'Tag points to another commit' >&2; exit 1; fi
elif ! grep -q 'HTTP 404' "$error"; then cat "$error" >&2; exit 1
fi
set -- dist/*.deb
if [ "$#" != 1 ] || [ ! -f "$1" ]; then echo 'Expected one tested .deb' >&2; exit 1; fi
package=$1
[ "$(dpkg-deb -f "$package" Version)" = "$version" ]
arch=$(dpkg-deb -f "$package" Architecture)
[ "$package" = "dist/termlook_${version}_${arch}.deb" ]
(cd dist && sha256sum "$(basename "$package")" > SHA256SUMS)
changes=$(awk -v v="$version" '/^## \[/ { if (found) exit; if (index($0, "[" v "]")) { found = 1; next } } found { print }' CHANGELOG.md 2> /dev/null || true)
{
    printf 'Native Rust/GTK/VTE Debian package, tested on Ubuntu 24.04 %s.\n\n' "$arch"
    if [ -n "$changes" ]; then printf '%s\n\n' "$changes"; fi
    cat <<NOTES
Install or upgrade with: sudo apt install ./$(basename "$package")
Verify the download with: sha256sum -c SHA256SUMS

Existing settings and layouts remain compatible. This download does not configure automatic updates.
NOTES
} > dist/release-notes.md
if [ "$exists" = false ]; then gh release create "$tag" --repo "$repo" --target "$commit" --title "TermLook $version" --draft --notes-file dist/release-notes.md; fi
gh release upload "$tag" "$package" dist/SHA256SUMS --repo "$repo" --clobber
case "$version" in 0.*) prerelease=true ;; *) prerelease=false ;; esac
gh release edit "$tag" --repo "$repo" --draft=false --prerelease="$prerelease"
