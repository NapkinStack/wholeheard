#!/usr/bin/env bash
# The module's end-to-end scenarios (cycle 01, D3): Summit started for real, its tray's menu
# driven as a click drives it — over the desktop's bus — with a recorder played here: a
# script that says what contracts/recording/v1 describes. No sound, nobody recorded.
#
# It needs a desktop session that shows tray items; where there is none — a CI runner — it
# says so and runs nothing. For half a minute a tray icon and a few notifications appear.
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p .evidence
: > .evidence/e2e.txt
say() { echo "$*" | tee -a .evidence/e2e.txt; }

watcher="org.kde.StatusNotifierWatcher /StatusNotifierWatcher org.kde.StatusNotifierWatcher"
if ! command -v busctl >/dev/null || ! busctl --user get-property $watcher RegisteredStatusNotifierItems >/dev/null 2>&1; then
  say "e2e: NOT RUN — no desktop session that shows tray items here"; exit 0
fi
cargo build --locked 2>&1 | tail -1 >> .evidence/e2e.txt
bin=${CARGO_TARGET_DIR:-$PWD/target}/debug/summit
work=$(mktemp -d)
app=""
ended() { [ -z "$app" ] || kill "$app" 2>/dev/null || true; pkill -P $$ 2>/dev/null || true; rm -rf "$work"; }
trap ended EXIT

# The recorder, played: it notes what it was given and told, says a source is silent, and
# stops when told — or, named "deaf", never says it stopped.
cat > "$work/summit-capture" <<'PLAYED'
#!/bin/sh
echo "$1 $2" > "$2/given"
echo "{\"event\":\"started\",\"folder\":\"$2/2026-10-13_14-05\"}"
echo '{"event":"silent","source":"others"}'
# Its input closed is no order to stop: it records on, as the real one does.
read line || exec sleep 20
echo "$line" > "$2/told"
echo "{\"event\":\"stopped\",\"folder\":\"$2/2026-10-13_14-05\",\"seconds\":1912.4}"
PLAYED
chmod +x "$work/summit-capture"

# What the desktop is asked to show, kept. The desktop passes each request on to itself: only
# those addressed to it by name are Summit's own.
dbus-monitor --session "interface='org.freedesktop.Notifications',member='Notify'" > "$work/notified" 2>/dev/null &
titles() {
  awk -v title="   string \"$1\"" '/^method call/ { ours = /destination=org.freedesktop.Notifications / } ours && $0 == title { n++ } END { print n + 0 }' "$work/notified"
}

start() { # the language, the settings file's text, the recorder
  mkdir -p "$work/config/summit" "$work/sessions"; rm -f "$work/sessions/given" "$work/sessions/told"
  printf 'sessions = "%s"\n%s\n' "$work/sessions" "$2" > "$work/config/summit/settings.toml"
  LANGUAGE= LC_ALL="$1" LANG="$1" XDG_CONFIG_HOME="$work/config" SUMMIT_CAPTURE="$3" "$bin" 2>> "$work/errors" &
  app=$!
  for _ in $(seq 50); do
    item=$(busctl --user get-property $watcher RegisteredStatusNotifierItems | grep -o ':[0-9.]*@[^" ]*summit[^" ]*' | tail -1) && [ -n "$item" ] && break
    sleep 0.1
  done
  bus=${item%@*}; menu=${item#*@}/Menu
}
labels() { # busctl writes what is not ASCII as escapes: printf reads them back
  printf '%b' "$(busctl --user call "$bus" "$menu" com.canonical.dbusmenu GetLayout iias 0 -- -1 0 | grep -o '"label" s "[^"]*"' | cut -d'"' -f4 | paste -sd'|')"
}
click() { busctl --user call "$bus" "$menu" com.canonical.dbusmenu Event isvu "$1" clicked s "" 0 >/dev/null; sleep 1; }
failed=0
check() { if "$@"; then say "  ok    $*"; else say "  FAIL  $*"; failed=1; fi; }
is() { [ "$1" = "$2" ] || { say "        got: $1"; return 1; }; }

say "A. in French: start from the menu, the reminder, a silent source, stop"
start fr_FR.UTF-8 "" "$work/summit-capture"
check is "$(labels)" "Démarrer l’enregistrement|Quitter Summit"
click 2
check is "$(labels)" "Arrêter l’enregistrement|Quitter Summit"
check is "$(cat "$work/sessions/given")" "record $work/sessions"
check is "$(titles 'Enregistrement en cours')" 1
check is "$(titles 'Aucun son venant de la réunion')" 1
click 2
check is "$(cat "$work/sessions/told")" "stop"
check is "$(labels)" "Démarrer l’enregistrement|Quitter Summit"
check is "$(titles 'Enregistrement arrêté')" 1
check grep -q 'Durée.: 32 min' "$work/notified"

say "B. quit while recording: the recording is stopped first, then Summit leaves"
click 2; rm -f "$work/sessions/told"
click 3; sleep 1
check is "$(cat "$work/sessions/told")" "stop"
check is "$(kill -0 "$app" 2>/dev/null && echo running || echo gone)" "gone"

say "C. in English, the reminder switched off; Summit killed while recording"
start en_GB.UTF-8 "reminder = false" "$work/summit-capture"
check is "$(labels)" "Start recording|Quit Summit"
click 2
check is "$(titles 'Recording')" 0
recorder=$(pgrep -P "$app" -f summit-capture || true)
kill -KILL "$app"; sleep 1
check test ! -e "$work/sessions/told"          # the recorder was not told to stop
check kill -0 "$recorder"                       # and it records on

say "D. no recorder, or one that ends before it starts: said, and the menu still offers to start"
start en_GB.UTF-8 "" "$work/nowhere"
click 2
check is "$(titles 'Summit cannot record')" 1
check is "$(labels)" "Start recording|Quit Summit"
click 3
printf '#!/bin/sh\necho "summit-capture: no source could be opened: nothing to record" >&2\nexit 1\n' > "$work/refuses"; chmod +x "$work/refuses"
start en_GB.UTF-8 "" "$work/refuses"
click 2
check is "$(titles 'Summit cannot record')" 2
check is "$(titles 'The recording stopped by itself')" 0
check grep -q 'Installing Summit again puts it back. (no source could be opened: nothing to record)' "$work/notified"
click 3

say "E. one gesture, pressed twice: one recording; a recorder that never ends: quit asked twice leaves"
printf '#!/bin/sh\necho x >> "$2/starts"\necho "{\\"event\\":\\"started\\",\\"folder\\":\\"$2/s\\"}"\nwhile read line; do echo "$line" >> "$2/told"; done\nexec sleep 20\n' > "$work/stubborn"; chmod +x "$work/stubborn"
start en_GB.UTF-8 "reminder = false" "$work/stubborn"
busctl --user call "$bus" "$menu" com.canonical.dbusmenu Event isvu 2 clicked s "" 0 >/dev/null
click 2
check is "$(wc -l < "$work/sessions/starts")" 1
check is "$(labels)" "Stop recording|Quit Summit"
click 3
check is "$(cat "$work/sessions/told")" "stop"
check is "$(kill -0 "$app" 2>/dev/null && echo waiting || echo gone)" "waiting"
click 3
check is "$(kill -0 "$app" 2>/dev/null && echo waiting || echo gone)" "gone"

check test ! -s "$work/errors"
[ -s "$work/errors" ] && sed 's/^/    /' "$work/errors" >> .evidence/e2e.txt
[ "$failed" = 0 ] && say "e2e: passed" || say "e2e: FAILED"
exit "$failed"
