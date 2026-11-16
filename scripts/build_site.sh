#!/bin/sh
# Assemble the project website into a directory (default: _site). No root needed.
#   ./scripts/build_site.sh [output-dir]
# Preview locally with: python3 -m http.server --directory _site 8000
set -eu
root=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
out=${1:-"$root/_site"}
version=$(cat "$root/VERSION")
printf '%s\n' "$version" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$'
case "$out" in
    ''|/) echo 'Refusing to use an empty or root output directory' >&2; exit 1 ;;
esac
rm -rf "$out"
mkdir -p "$out/images"
cp -R "$root/site/." "$out/"
# Screenshots and the logo are shared with the README instead of being duplicated.
cp "$root"/docs/images/*.png "$out/images/"
cp "$root/assets/io.termlook.TermLook.svg" "$out/favicon.svg"
# Stamp the current version into pages that mention it.
find "$out" -type f \( -name '*.html' -o -name '*.xml' -o -name '*.txt' \) \
    -exec sed -i "s/@VERSION@/$version/g" {} +
printf 'Built site for TermLook %s in %s\n' "$version" "$out"
