#!/usr/bin/env bash
# The module's end-to-end scenarios (cycle 01, D3): Summit started for real, its tray's menu
# driven as a click drives it — over the desktop's bus — with a recorder played here: a
# script that says what contracts/recording/v1 describes. No sound, nobody recorded.
#
# It needs a desktop session that shows tray items; where there is none — a CI runner — it
# says so and runs nothing. For half a minute a tray icon and a few notifications appear, and
# a window.
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
  printf 'sessions = "%s"\n%b\n' "$work/sessions" "$2" > "$work/config/summit/settings.toml"
  LANGUAGE= LC_ALL="$1" LANG="$1" XDG_CONFIG_HOME="$work/config" XDG_DATA_HOME="$work/data" SUMMIT_CAPTURE="$3" \
    SUMMIT_TRANSCRIPTION="$work/summit-transcription" SUMMIT_MODEL_PUBLISHED="$published" "$bin" 2>> "$work/errors" &
  app=$!
  # Its own tray item, found by its program's number: a Summit the user runs has one too.
  item=""
  for _ in $(seq 100); do
    for one in $(busctl --user get-property $watcher RegisteredStatusNotifierItems | grep -o ':[0-9.]*@[^" ]*summit[^" ]*'); do
      [ "$(busctl --user status "${one%@*}" 2>/dev/null | awk -F= '/^PID=/ {print $2}')" = "$app" ] && item=$one
    done
    [ -n "$item" ] && break
    sleep 0.1
  done
  bus=${item%@*}; menu=${item#*@}/Menu
}
greyed() { busctl --user call "$bus" "$menu" com.canonical.dbusmenu GetLayout iias 0 -- -1 0 | grep -o '"enabled" b false' | wc -l; }
labels() { # busctl writes what is not ASCII as escapes: printf reads them back
  printf '%b' "$(busctl --user call "$bus" "$menu" com.canonical.dbusmenu GetLayout iias 0 -- -1 0 | grep -o '"label" s "[^"]*"' | cut -d'"' -f4 | paste -sd'|')"
}
entry() { # the menu's number of an entry, by its label
  busctl --user --json=short call "$bus" "$menu" com.canonical.dbusmenu GetLayout iias 0 -- -1 0 | python3 -c '
import json, sys
def walk(item):
    number, props, children = item["data"] if "data" in item else item
    if props.get("label", {}).get("data", "").startswith(sys.argv[1]): print(number)
    for child in children: walk(child["data"] if "data" in child else child)
walk(json.load(sys.stdin)["data"][1])' "$1"
}
windows() { ps -o comm= --ppid "$app" | grep -c '^WebKitWebProces' || true; }
click() { busctl --user call "$bus" "$menu" com.canonical.dbusmenu Event isvu "$1" clicked s "" 0 >/dev/null; sleep 1; }
published=""
failed=0
check() { if "$@"; then say "  ok    $*"; else say "  FAIL  $*"; failed=1; fi; }
is() { [ "$1" = "$2" ] || { say "        got: $1"; return 1; }; }

# The program that hears, played: it notes what it was given, fails, is slow or stuck when told.
cat > "$work/summit-transcription" <<'PLAYED'
#!/bin/sh
here=$(dirname "$1"); echo "$*" >> "$here/heard"
if [ -e "$here/fail-once" ]; then rm "$here/fail-once"; echo "summit-transcription: the engine stopped" >&2; exit 1; fi
if [ -e "$here/slow" ]; then rm "$here/slow"; sleep 3; fi
if [ -e "$here/stuck" ]; then echo $$ > "$here/hearing"; sleep 30; fi
mkdir -p "$1" && echo '{}' > "$1/transcript.jsonl"
PLAYED
chmod +x "$work/summit-transcription"
: > "$work/model.bin"

say "A. in French: start from the menu, the reminder, a silent source, stop"
start fr_FR.UTF-8 "transcript = false" "$work/summit-capture"
check is "$(labels)" "Ouvrir Summit|Démarrer l’enregistrement|Transcrire le dernier enregistrement|Quitter Summit"
# A window is a web view, a process of WebKit's under Summit: none before the user asks.
check is "$(windows)" 0
click 2; sleep 2
check test "$(windows)" -ge 1
click 4                                           # no session yet: said, nothing heard
check is "$(titles 'Aucun enregistrement à transcrire')" 1
click 3
check is "$(labels)" "Ouvrir Summit|Arrêter l’enregistrement|Transcrire le dernier enregistrement|Quitter Summit"
check is "$(cat "$work/sessions/given")" "record $work/sessions"
check is "$(titles 'Enregistrement en cours')" 1
check is "$(titles 'Aucun son venant de la réunion')" 1
click 3
check is "$(cat "$work/sessions/told")" "stop"
check is "$(labels)" "Ouvrir Summit|Démarrer l’enregistrement|Transcrire le dernier enregistrement|Quitter Summit"
check is "$(titles 'Enregistrement arrêté')" 1
check grep -q 'Durée.: 32 min' "$work/notified"
check test ! -e "$work/sessions/heard"
check is "$(grep -c 'La transcription suit' "$work/notified")" 0

say "B. quit while recording: the recording is stopped first, then Summit leaves"
click 3; rm -f "$work/sessions/told"
click 5; sleep 1
check is "$(cat "$work/sessions/told")" "stop"
check is "$(kill -0 "$app" 2>/dev/null && echo running || echo gone)" "gone"

say "C. in English, the reminder switched off; Summit killed while recording"
start en_GB.UTF-8 "reminder = false\ntranscript = false" "$work/summit-capture"
check is "$(labels)" "Open Summit|Start recording|Transcribe the last recording|Quit Summit"
click 3
check is "$(titles 'Recording')" 0
recorder=$(pgrep -P "$app" -f summit-capture || true)
kill -KILL "$app"; sleep 1
check test ! -e "$work/sessions/told"          # the recorder was not told to stop
check kill -0 "$recorder"                       # and it records on

say "D. no recorder, or one that ends before it starts: said, and the menu still offers to start"
start en_GB.UTF-8 "transcript = false" "$work/nowhere"
click 3
check is "$(titles 'Summit cannot record')" 1
check is "$(labels)" "Open Summit|Start recording|Transcribe the last recording|Quit Summit"
click 5
printf '#!/bin/sh\necho "summit-capture: no source could be opened: nothing to record" >&2\nexit 1\n' > "$work/refuses"; chmod +x "$work/refuses"
start en_GB.UTF-8 "transcript = false" "$work/refuses"
click 3
check is "$(titles 'Summit cannot record')" 2
check is "$(titles 'The recording stopped by itself')" 0
check grep -q 'Installing Summit again puts it back. (no source could be opened: nothing to record)' "$work/notified"
click 5

say "E. one gesture, pressed twice: one recording; a recorder that never ends: quit asked twice leaves"
printf '#!/bin/sh\necho x >> "$2/starts"\necho "{\\"event\\":\\"started\\",\\"folder\\":\\"$2/s\\"}"\nwhile read line; do echo "$line" >> "$2/told"; done\nexec sleep 20\n' > "$work/stubborn"; chmod +x "$work/stubborn"
start en_GB.UTF-8 "reminder = false\ntranscript = false" "$work/stubborn"
busctl --user call "$bus" "$menu" com.canonical.dbusmenu Event isvu 3 clicked s "" 0 >/dev/null
click 3
check is "$(wc -l < "$work/sessions/starts")" 1
check is "$(windows)" 0                           # both presses on the recording's entry
check is "$(labels)" "Open Summit|Stop recording|Transcribe the last recording|Quit Summit"
click 5
check is "$(cat "$work/sessions/told")" "stop"
check is "$(kill -0 "$app" 2>/dev/null && echo waiting || echo gone)" "waiting"
click 5
check is "$(kill -0 "$app" 2>/dev/null && echo waiting || echo gone)" "gone"

say "F. the transcript follows the stop; a failed one is made again from the menu; no model is said"
start en_GB.UTF-8 "reminder = false\nmodel = \"$work/model.bin\"" "$work/summit-capture"
click 3; click 3; sleep 1
check grep -q '32 min recorded. The transcript follows.' "$work/notified"
check is "$(cat "$work/sessions/heard")" "$work/sessions/2026-10-13_14-05 --model $work/model.bin"
check is "$(titles 'Transcript ready')" 1
check test -s "$work/sessions/2026-10-13_14-05/transcript.jsonl"
echo '{}' > "$work/sessions/2026-10-13_14-05/session.json"; touch "$work/sessions/fail-once"
click 4; sleep 1
check is "$(titles 'The transcript was not made')" 1
check grep -q 'tries again from where it stopped. (the engine stopped)' "$work/notified"
click 4; sleep 1
check is "$(titles 'Transcript ready')" 2
check is "$(wc -l < "$work/sessions/heard")" 3
check is "$(greyed)" 0
# Asked for while recording, it is greyed: the recording's transcript follows its stop.
click 3
check is "$(greyed)" 1
click 4; click 3; sleep 1
check is "$(wc -l < "$work/sessions/heard")" 4
check is "$(titles 'Transcript ready')" 3
touch "$work/sessions/slow"
click 4
check is "$(greyed)" 1
click 3; click 3; sleep 5
check is "$(wc -l < "$work/sessions/heard")" 6
check is "$(titles 'Transcript ready')" 5
check is "$(greyed)" 0
touch "$work/sessions/stuck"
click 4; sleep 1; hearing=$(cat "$work/sessions/hearing"); click 5; sleep 1
check is "$(kill -0 "$hearing" 2>/dev/null && echo hearing || echo ended)" "ended"
check is "$(kill -0 "$app" 2>/dev/null && echo running || echo gone)" "gone"
rm "$work/sessions/stuck"
start en_GB.UTF-8 "reminder = false\nmodel = \"$work/model.bin\"" "$work/summit-capture"
click 3; click 5; sleep 1
check is "$(cat "$work/sessions/told") $(kill -0 "$app" 2>/dev/null || echo gone)" "stop gone"
check is "$(wc -l < "$work/sessions/heard")" 7
start en_GB.UTF-8 "reminder = false\nmodel = \"$work/no-model.bin\"" "$work/summit-capture"
click 3; click 3; sleep 1
check is "$(titles 'No transcript: the model is missing')" 1
check is "$(wc -l < "$work/sessions/heard")" 7
click 5

say "G. the default model missing: offered with its size and host, fetched on the click, then the transcripts in line"
mkdir -p "$work/served"; head -c 300000 /dev/urandom > "$work/served/model.bin"; head -c 300000 /dev/urandom > "$work/right.bin"
# Served slowly — 30 kB every 0.6 s, six seconds a fetch — so that a stop can come during one.
(cd "$work/served" && exec python3 -u -c '
import http.server, time
class Slow(http.server.SimpleHTTPRequestHandler):
    def copyfile(self, source, sink):
        while chunk := source.read(30000):
            sink.write(chunk); sink.flush(); time.sleep(0.6)
server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Slow)
print("port", server.server_address[1]); server.serve_forever()' > "$work/server" 2>&1) &
for _ in $(seq 50); do port=$(grep -o 'port [0-9]*' "$work/server" | cut -d' ' -f2) && [ -n "$port" ] && break; sleep 0.1; done
published="http://127.0.0.1:$port/model.bin 300000 $(sha256sum "$work/right.bin" | cut -d' ' -f1)"
# A recorder that names each session anew, so that several wait in line.
{ echo '#!/bin/sh'; echo 'n=$(( $(cat "$2/numbered" 2>/dev/null || echo 0) + 1 )); echo "$n" > "$2/numbered"'
  sed '1d; s#2026-10-13_14-05#2026-10-17_09-0$n#g' "$work/summit-capture"; } > "$work/numbering"
chmod +x "$work/numbering"
start en_GB.UTF-8 "reminder = false" "$work/numbering"
before=$(wc -l < "$work/sessions/heard")
click 3; click 3; sleep 1; click 3; click 3; sleep 1    # two sessions, both waiting for the model
check is "$(titles 'The transcription model is needed')" 2
check is "$(labels)" "Open Summit|Start recording|Transcribe the last recording|Download the model (0 MB, 127.0.0.1:$port)|Quit Summit"
check is "$(grep -c 'GET' "$work/server")" 0      # nothing fetched before the click
# Served first is not the published file: refused, nothing in place, and the entry tries again.
click "$(entry Download)"; sleep 7
check is "$(titles 'The model was not downloaded')" 1
check test ! -e "$work/data/summit/models/ggml-large-v3-turbo-q8_0.bin"
check test ! -e "$work/data/summit/models/ggml-large-v3-turbo-q8_0.bin.part"
check is "$(greyed)" 0
cp "$work/right.bin" "$work/served/model.bin"
# A third stop during the fetch: it waits in line, nothing is offered again, the entry stays greyed.
click "$(entry Download)"; click 3; click 3
check is "$(titles 'The transcription model is needed')" 2
check is "$(greyed)" 1
sleep 5
check is "$(titles 'Model ready')" 1
check is "$(titles 'The transcription model is needed')" 2    # nor later in the fetch
check cmp -s "$work/right.bin" "$work/data/summit/models/ggml-large-v3-turbo-q8_0.bin"
check is "$(wc -l < "$work/sessions/heard")" "$((before + 3))"
check is "$(tail -3 "$work/sessions/heard" | grep -o '2026-10-17_09-0[0-9]' | paste -sd' ')" "2026-10-17_09-01 2026-10-17_09-02 2026-10-17_09-03"   # in the order stopped
check is "$(labels)" "Open Summit|Start recording|Transcribe the last recording|Quit Summit"
click 5
published=""

check test ! -s "$work/errors"
[ -s "$work/errors" ] && sed 's/^/    /' "$work/errors" >> .evidence/e2e.txt
[ "$failed" = 0 ] && say "e2e: passed" || say "e2e: FAILED"
exit "$failed"
