#!/bin/sh
# Install the native binary and desktop integration for the current user.
set -eu
root=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
cd "$root"
cargo build --release --locked
bin="$HOME/.local/bin"
data=${XDG_DATA_HOME:-"$HOME/.local/share"}
install -Dm755 target/release/termlook "$bin/termlook"
install -Dm644 assets/io.termlook.TermLook.svg "$data/icons/hicolor/scalable/apps/io.termlook.TermLook.svg"
mkdir -p "$data/applications"
# Desktop Exec uses its own escaping rules, not shell quoting.
# shellcheck disable=SC2016 # Literal desktop-entry escaping, not variable expansion.
exec_path=$(printf '%s' "$bin/termlook" | sed 's/\\/\\\\/g; s/"/\\"/g; s/`/\\`/g; s/\$/\\$/g; s/%/%%/g')
while IFS= read -r line; do
    case "$line" in Exec=*) printf 'Exec="%s"\n' "$exec_path" ;; *) printf '%s\n' "$line" ;; esac
done < packaging/debian/termlook.desktop > "$data/applications/io.termlook.TermLook.desktop"
printf 'Installed %s and desktop entry. Add %s to PATH for command-line use.\n' "$bin/termlook" "$bin"
