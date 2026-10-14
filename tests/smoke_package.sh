#!/bin/sh
# Run against an installed package inside a private D-Bus/Xvfb session.
set -eu
export GDK_BACKEND=x11
bin=${1:-/usr/bin/termlook}
config=$(mktemp -d)
trap 'rm -rf "$config"' EXIT HUP INT TERM
export XDG_CONFIG_HOME="$config"
mkdir -p "$config/termlook"
printf '{"shell":"/bin/sh","close_to_tray":false,"keybindings":{"new-tab":"<Primary><Shift>y"}}\n' > "$config/termlook/settings.json"
"$bin" --version
"$bin" > "$config/log" 2>&1 &
pid=$!
trap 'kill "$pid" 2>/dev/null || true; rm -rf "$config"' EXIT HUP INT TERM
window=$(timeout 20s xdotool search --sync --onlyvisible --pid "$pid" --name TermLook 2>/dev/null | head -n 1)
if [ -z "$window" ]; then cat "$config/log" >&2; exit 1; fi
wait_result() {
    n=0
    while [ ! -s "$config/$1" ]; do
        n=$((n+1))
        if [ "$n" -ge 100 ]; then cat "$config/log" >&2; echo "Missing shell result: $1" >&2; exit 1; fi
        sleep 0.1
    done
}
xdotool windowfocus --sync "$window"
xdotool type --clearmodifiers "TERMLOOK_SMOKE=original; printf 'native-shell-ok' > '$config/shell-result'"
xdotool key Return
wait_result shell-result
[ "$(cat "$config/shell-result")" = native-shell-ok ]
# Use a customized binding, then the Shift+Tab keysym to return to the old shell.
xdotool key ctrl+shift+y
sleep 0.3
xdotool type --clearmodifiers "printf 'second-tab' > '$config/second-result'"
xdotool key Return
wait_result second-result
xdotool key ctrl+shift+Tab
sleep 0.1
xdotool type --clearmodifiers "printf '%s' \"\$TERMLOOK_SMOKE\" > '$config/restored-result'"
xdotool key Return
wait_result restored-result
[ "$(cat "$config/restored-result")" = original ]
# A second invocation must activate the existing process and exit.
"$bin"
kill -0 "$pid"
xdotool key ctrl+shift+q
confirm=$(timeout 10s xdotool search --sync --onlyvisible --name '^Confirm close$' 2>/dev/null | head -n 1)
xdotool windowfocus --sync "$confirm"
# Enter must cancel by default, keeping the application alive.
xdotool key Return
sleep 0.1
kill -0 "$pid"
xdotool windowfocus --sync "$window"
xdotool key ctrl+shift+q
confirm=$(timeout 10s xdotool search --sync --onlyvisible --name '^Confirm close$' 2>/dev/null | head -n 1)
xdotool windowfocus --sync "$confirm"
xdotool key Tab Return
wait "$pid"
[ -s "$config/termlook/layout.json" ]
cat "$config/log"
