#!/usr/bin/env bash
# The module's end-to-end scenarios (cycle 01, D3): Wholeheard started for real, its tray's menu
# driven as a click drives it — over the desktop's bus — with a recorder played here: a
# script that says what contracts/recording/v1 describes. No sound, nobody recorded.
#
# It needs a desktop session that shows tray items; where there is none — a CI runner — it
# says so and runs nothing. For half a minute a tray icon and a few notifications appear, and
# the window, three times.
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
bin=${CARGO_TARGET_DIR:-$PWD/target}/debug/wholeheard
work=$(mktemp -d)
app=""
ended() { [ -z "$app" ] || kill "$app" 2>/dev/null || true; pkill -P $$ 2>/dev/null || true; rm -rf "$work"; }
trap ended EXIT

# The recorder, played: it notes what it was given and told, says a source is silent, and
# stops when told — or, named "deaf", never says it stopped.
cat > "$work/wholeheard-capture" <<'PLAYED'
#!/bin/sh
echo "$1 $2" > "$2/given"
echo "{\"event\":\"started\",\"folder\":\"$2/2026-10-13_14-05\"}"
echo '{"event":"silent","source":"others"}'
# The user switches devices: a headset's microphone, then no output left (D31).
echo '{"event":"device","source":"me","device":"Casque Mic"}'
echo '{"event":"device","source":"others","device":null}'
echo '{"event":"device","source":"others","device":"Casque"}'
# Its input closed is no order to stop: it records on, as the real one does.
read line || exec sleep 20
echo "$line" > "$2/told"
echo "{\"event\":\"stopped\",\"folder\":\"$2/2026-10-13_14-05\",\"seconds\":1912.4}"
PLAYED
chmod +x "$work/wholeheard-capture"

# What the desktop is asked to show, kept. The desktop passes each request on to itself: only
# those addressed to it by name are Wholeheard's own.
dbus-monitor --session "interface='org.freedesktop.Notifications',member='Notify'" > "$work/notified" 2>/dev/null &
titles() {
  awk -v title="   string \"$1\"" '/^method call/ { ours = /destination=org.freedesktop.Notifications / } ours && $0 == title { n++ } END { print n + 0 }' "$work/notified"
}

start() { # the language, the settings file's text, the recorder
  mkdir -p "$work/config/wholeheard" "$work/sessions"; rm -f "$work/sessions/given" "$work/sessions/told"
  printf 'sessions = "%s"\n%b\n' "$work/sessions" "$2" > "$work/config/wholeheard/settings.toml"
  LANGUAGE= LC_ALL="$1" LANG="$1" XDG_CONFIG_HOME="$work/config" XDG_DATA_HOME="$work/data" XDG_STATE_HOME="$work/state" WHOLEHEARD_CAPTURE="$3" \
    WHOLEHEARD_TRANSCRIPTION="$work/wholeheard-transcription" WHOLEHEARD_MODEL_PUBLISHED="$published" WHOLEHEARD_SUMMARY="$work/wholeheard-summary" WHOLEHEARD_KEYRING=memory "$bin" 2>> "$work/errors" &
  app=$!
  # Its own tray item, found by its program's number: a Wholeheard the user runs has one too.
  item=""
  for _ in $(seq 100); do
    for one in $(busctl --user get-property $watcher RegisteredStatusNotifierItems | grep -o ':[0-9.]*@[^" ]*wholeheard[^" ]*'); do
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
# Quit may leave before its click is answered: then the bus says the peer left, which is no fault.
click() {
  busctl --user call "$bus" "$menu" com.canonical.dbusmenu Event isvu "$1" clicked s "" 0 >/dev/null 2> "$work/clicked" ||
    grep -q 'Remote peer disconnected' "$work/clicked"
  sleep 1
}
published=""
failed=0
check() { if "$@"; then say "  ok    $*"; else say "  FAIL  $*"; failed=1; fi; }
is() { [ "$1" = "$2" ] || { say "        got: $1"; return 1; }; }

# The program that hears, played: it notes what it was given, fails, is slow or stuck when told.
cat > "$work/wholeheard-transcription" <<'PLAYED'
#!/bin/sh
here=$(dirname "$1"); echo "$*" >> "$here/heard"; echo "${WHOLEHEARD_LOG:-unset}" >> "$here/log-level"
if [ -e "$here/fail-once" ]; then rm "$here/fail-once"; echo "wholeheard-transcription: the engine stopped" >&2; exit 1; fi
if [ -e "$here/slow" ]; then rm "$here/slow"; sleep 3; fi
if [ -e "$here/stuck" ]; then echo $$ > "$here/hearing"; sleep 30; fi
mkdir -p "$1" && printf '{"type":"header","contract":"transcript/v1","engine":"e","model":"m","language":null,"context":null}\n{"type":"complete","duration":1.0}\n' > "$1/transcript.jsonl"
PLAYED
chmod +x "$work/wholeheard-transcription"
: > "$work/model.bin"

say "A. in French: start from the menu, the reminder, a silent source, stop"
start fr_FR.UTF-8 "transcript = false" "$work/wholeheard-capture"
check is "$(labels)" "Ouvrir Wholeheard|Démarrer l’enregistrement|Transcrire le dernier enregistrement|Faire le compte rendu du dernier enregistrement|Quitter Wholeheard"
# A window is a web view, a process of WebKit's under Wholeheard: none before the user asks.
check is "$(windows)" 0
click 2; sleep 2
check test "$(windows)" -ge 1
click 4                                           # no session yet: said, nothing heard
check is "$(titles 'Aucun enregistrement à transcrire')" 1
click 3
check is "$(labels)" "Ouvrir Wholeheard|Arrêter l’enregistrement|Transcrire le dernier enregistrement|Faire le compte rendu du dernier enregistrement|Quitter Wholeheard"
check is "$(cat "$work/sessions/given")" "record $work/sessions"
check is "$(titles 'Enregistrement en cours')" 1
check is "$(titles 'Aucun son venant de la réunion')" 1
check is "$(titles 'Micro changé')" 1
check grep -q $'Enregistré maintenant\u00a0: Casque Mic' "$work/notified"
check is "$(titles 'Le son de la réunion n’est plus enregistré pour l’instant')" 1
check grep -q 'Votre micro est toujours enregistré, et une sortie branchée' "$work/notified"
check is "$(titles 'Le son de la réunion est de nouveau enregistré')" 1
click 3
check is "$(cat "$work/sessions/told")" "stop"
check is "$(labels)" "Ouvrir Wholeheard|Démarrer l’enregistrement|Transcrire le dernier enregistrement|Faire le compte rendu du dernier enregistrement|Quitter Wholeheard"
check is "$(titles 'Enregistrement arrêté')" 1
check grep -q 'Durée.: 32 min' "$work/notified"
check test ! -e "$work/sessions/heard"
check is "$(grep -c 'La transcription suit' "$work/notified")" 0

say "B. quit while recording: the recording is stopped first, then Wholeheard leaves"
click 3; rm -f "$work/sessions/told"
click 6; sleep 1
check is "$(cat "$work/sessions/told")" "stop"
check is "$(kill -0 "$app" 2>/dev/null && echo running || echo gone)" "gone"

say "C. in English, the reminder switched off; Wholeheard killed while recording"
start en_GB.UTF-8 "reminder = false\ntranscript = false" "$work/wholeheard-capture"
check is "$(labels)" "Open Wholeheard|Start recording|Transcribe the last recording|Write the minutes of the last recording|Quit Wholeheard"
click 3
check is "$(titles 'Recording')" 0
recorder=$(pgrep -P "$app" -f wholeheard-capture || true)
kill -KILL "$app"; sleep 1
check test ! -e "$work/sessions/told"          # the recorder was not told to stop
check kill -0 "$recorder"                       # and it records on

say "D. no recorder, or one that ends before it starts: said, and the menu still offers to start"
start en_GB.UTF-8 "transcript = false" "$work/nowhere"
click 3
check is "$(titles 'Wholeheard cannot record')" 1
check is "$(labels)" "Open Wholeheard|Start recording|Transcribe the last recording|Write the minutes of the last recording|Quit Wholeheard"
click 6
printf '#!/bin/sh\necho "wholeheard-capture: no source could be opened: nothing to record" >&2\nexit 1\n' > "$work/refuses"; chmod +x "$work/refuses"
start en_GB.UTF-8 "transcript = false" "$work/refuses"
click 3
check is "$(titles 'Wholeheard cannot record')" 2
check is "$(titles 'The recording stopped by itself')" 0
check grep -q 'Installing Wholeheard again puts it back.' "$work/notified"
# What the program said is the log's, never a notification's (D29).
check is "$(grep -c 'nothing to record' "$work/notified")" 0
click 6

say "E. one gesture, pressed twice: one recording; a recorder that never ends: quit asked twice leaves"
printf '#!/bin/sh\necho x >> "$2/starts"\necho "{\\"event\\":\\"started\\",\\"folder\\":\\"$2/s\\"}"\nwhile read line; do echo "$line" >> "$2/told"; done\nexec sleep 20\n' > "$work/stubborn"; chmod +x "$work/stubborn"
start en_GB.UTF-8 "reminder = false\ntranscript = false" "$work/stubborn"
busctl --user call "$bus" "$menu" com.canonical.dbusmenu Event isvu 3 clicked s "" 0 >/dev/null
click 3
check is "$(wc -l < "$work/sessions/starts")" 1
check is "$(windows)" 0                           # both presses on the recording's entry
check is "$(labels)" "Open Wholeheard|Stop recording|Transcribe the last recording|Write the minutes of the last recording|Quit Wholeheard"
click 6
check is "$(cat "$work/sessions/told")" "stop"
check is "$(kill -0 "$app" 2>/dev/null && echo waiting || echo gone)" "waiting"
click 6
check is "$(kill -0 "$app" 2>/dev/null && echo waiting || echo gone)" "gone"

say "F. the transcript follows the stop; a failed one is made again from the menu; no model is said"
# The logs' level chosen in the developer mode reaches the programs Wholeheard starts (D29).
start en_GB.UTF-8 "reminder = false\nmodel = \"$work/model.bin\"\nlog = \"debug\"" "$work/wholeheard-capture"
click 3; click 3; sleep 1
check is "$(head -1 "$work/sessions/log-level")" "debug"
check grep -q '32 min recorded. The transcript follows.' "$work/notified"
check is "$(cat "$work/sessions/heard")" "$work/sessions/2026-10-13_14-05 --model $work/model.bin"
check is "$(titles 'Transcript ready')" 1
check test -s "$work/sessions/2026-10-13_14-05/transcript.jsonl"
echo '{}' > "$work/sessions/2026-10-13_14-05/session.json"; touch "$work/sessions/fail-once"
click 4; sleep 1
check is "$(titles 'The transcript was not made')" 1
check grep -q 'tries again from where it stopped.' "$work/notified"
check is "$(grep -c 'engine stopped' "$work/notified")" 0
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
click 4; sleep 1; hearing=$(cat "$work/sessions/hearing"); click 6; sleep 1
check is "$(kill -0 "$hearing" 2>/dev/null && echo hearing || echo ended)" "ended"
check is "$(kill -0 "$app" 2>/dev/null && echo running || echo gone)" "gone"
rm "$work/sessions/stuck"
start en_GB.UTF-8 "reminder = false\nmodel = \"$work/model.bin\"" "$work/wholeheard-capture"
click 3; click 6; sleep 1
check is "$(cat "$work/sessions/told") $(kill -0 "$app" 2>/dev/null || echo gone)" "stop gone"
check is "$(wc -l < "$work/sessions/heard")" 7
start en_GB.UTF-8 "reminder = false\nmodel = \"$work/no-model.bin\"" "$work/wholeheard-capture"
click 3; click 3; sleep 1
check is "$(titles 'No transcript: the model is missing')" 1
check is "$(wc -l < "$work/sessions/heard")" 7
click 6

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
  sed '1d; s#2026-10-13_14-05#2026-10-17_09-0$n#g' "$work/wholeheard-capture"; } > "$work/numbering"
chmod +x "$work/numbering"
start en_GB.UTF-8 "reminder = false" "$work/numbering"
before=$(wc -l < "$work/sessions/heard")
click 3; click 3; sleep 1; click 3; click 3; sleep 1    # two sessions, both waiting for the model
check is "$(titles 'The transcription model is needed')" 2
check is "$(labels)" "Open Wholeheard|Start recording|Transcribe the last recording|Download the model (0 MB, 127.0.0.1:$port)|Write the minutes of the last recording|Quit Wholeheard"
check is "$(grep -c 'GET' "$work/server")" 0      # nothing fetched before the click
# Served first is not the published file: refused, nothing in place, and the entry tries again.
click "$(entry Download)"; sleep 7
check is "$(titles 'The model was not downloaded')" 1
check test ! -e "$work/data/wholeheard/models/ggml-large-v3-turbo-q8_0.bin"
check test ! -e "$work/data/wholeheard/models/ggml-large-v3-turbo-q8_0.bin.part"
check is "$(greyed)" 0
cp "$work/right.bin" "$work/served/model.bin"
# A third stop during the fetch: it waits in line, nothing is offered again, the entry stays greyed.
click "$(entry Download)"; click 3; click 3
check is "$(titles 'The transcription model is needed')" 2
check is "$(greyed)" 1
sleep 5
check is "$(titles 'Model ready')" 1
check is "$(titles 'The transcription model is needed')" 2    # nor later in the fetch
check cmp -s "$work/right.bin" "$work/data/wholeheard/models/ggml-large-v3-turbo-q8_0.bin"
check is "$(wc -l < "$work/sessions/heard")" "$((before + 3))"
# No level chosen, none is named: the programs log at their usual one.
check is "$(tail -3 "$work/sessions/log-level" | sort -u)" "unset"
check is "$(tail -3 "$work/sessions/heard" | grep -o '2026-10-17_09-0[0-9]' | paste -sd' ')" "2026-10-17_09-01 2026-10-17_09-02 2026-10-17_09-03"   # in the order stopped
check is "$(labels)" "Open Wholeheard|Start recording|Transcribe the last recording|Write the minutes of the last recording|Quit Wholeheard"
click 6
published=""

say "L. audio kept longer than the settings say goes to the trash, the transcript stays"
old="$work/sessions/2026-01-05_09-00"
mkdir -p "$old/derived"
printf '{"contract":"session-audio/v2","started":"2026-01-05T09:00:00+01:00","stopped":"2026-01-05T10:00:00+01:00","tracks":{}}' > "$old/session.json"
printf '{"type":"header","contract":"transcript/v1","engine":"e","model":"m","language":null,"context":null}\n{"type":"complete","duration":3600.0}\n' > "$old/transcript.jsonl"
head -c 1000 /dev/zero > "$old/local.wav"; head -c 1000 /dev/zero > "$old/derived/local.wav"
# A trash that cannot be made: the audio stays, and the refusal is said.
mkdir -p "$work/data"; : > "$work/data/Trash"
start fr_FR.UTF-8 "keep_audio = 1" "$work/wholeheard-capture"
sleep 3
check test -e "$old/local.wav" -a -e "$old/derived/local.wav"
check is "$(titles 'L’audio n’a pas été entièrement mis à la corbeille')" 1
check grep -q "rien n.a .t. supprim" "$work/notified"
check test -z "$(grep -E 'trash. operation|FileSystem|Os \{' "$work/notified")"
click 6
rm "$work/data/Trash"
start fr_FR.UTF-8 "keep_audio = 1" "$work/wholeheard-capture"
sleep 3
check test ! -e "$old/local.wav" -a ! -e "$old/derived/local.wav"
check test -s "$old/transcript.jsonl" -a -s "$old/session.json"
# The system's trash — here the scenario's own, its data folder being the scenario's.
check test -n "$(find "$work/data/Trash/files" -name 'local*.wav' 2>/dev/null)"
check is "$(titles 'Audio mis à la corbeille')" 1
click 6

say "M. the minutes from the tray: on this computer at once; online only once agreed, nothing sent before"
# wholeheard-summary played here: it says what it was given and writes the minutes.
cat > "$work/wholeheard-summary" <<'PLAYED'
#!/bin/sh
here=$(dirname "$0")
echo "$*" >> "$here/summarised"
printf '%s\n' "${WHOLEHEARD_PROVIDER_KEY:-none}" >> "$here/summary-keys"
echo '{"event":"sending","host":"h","model":"m","characters":1}'
printf -- '---\ncontract: minutes/v1\n---\n\n## A\n\n## B\n\n## C\n' > "$1/minutes.md"
echo "{\"event\":\"written\",\"file\":\"$1/minutes.md\"}"
PLAYED
chmod +x "$work/wholeheard-summary"
rm -rf "$work/sessions"; meeting="$work/sessions/2026-10-18_10-00"; mkdir -p "$meeting"
printf '{"contract":"session-audio/v2","started":"2026-10-18T10:00:00+02:00","stopped":"2026-10-18T10:30:00+02:00","tracks":{}}' > "$meeting/session.json"
printf '{"type":"header","contract":"transcript/v1","engine":"e","model":"m","language":null,"context":null}\n{"type":"segment","speaker":"others","track":"remote","start":0.0,"end":2.0,"text":"On confirme le 3 juin."}\n{"type":"complete","duration":1800.0}\n' > "$meeting/transcript.jsonl"
start fr_FR.UTF-8 "minutes_provider = \"http://127.0.0.1:9/v1\"\nminutes_model = \"mistral\"" "$work/wholeheard-capture"
click 5; sleep 2
check test -s "$meeting/minutes.md"
check is "$(cat "$work/summarised")" "$meeting --provider http://127.0.0.1:9/v1 --model mistral"
check is "$(titles 'Compte rendu prêt')" 1
click 6
# An address that names no host: said, nothing run.
start fr_FR.UTF-8 "minutes_provider = \"nowhere\"\nminutes_model = \"m\"" "$work/wholeheard-capture"
click 5; sleep 2
check is "$(titles 'Le compte rendu n’a pas été fait')" 1
check is "$(wc -l < "$work/summarised")" 1
click 6
# Online, not agreed: nothing is sent, the window opens to ask.
rm "$meeting/minutes.md"
start fr_FR.UTF-8 "minutes_provider = \"https://openrouter.ai/api/v1\"\nminutes_model = \"a-model\"" "$work/wholeheard-capture"
click 5; sleep 3
check is "$(titles 'Le compte rendu attend votre accord')" 1
check is "$(wc -l < "$work/summarised")" 1
check test ! -e "$meeting/minutes.md"
check is "$(windows)" 1
click 6
# Agreed to that host: sent, with no key kept.
start fr_FR.UTF-8 "minutes_provider = \"https://openrouter.ai/api/v1\"\nminutes_model = \"a-model\"\nminutes_agreed = \"openrouter.ai\"" "$work/wholeheard-capture"
click 5; sleep 2
check is "$(tail -1 "$work/summarised")" "$meeting --provider https://openrouter.ai/api/v1 --model a-model"
check is "$(tail -1 "$work/summary-keys")" none
check is "$(titles 'Compte rendu prêt')" 2
click 6
# Chained: a recording stopped, its transcript, then its minutes — no other gesture.
start fr_FR.UTF-8 "reminder = false\nafter = \"minutes\"\nmodel = \"$work/model.bin\"\nminutes_provider = \"http://127.0.0.1:9/v1\"\nminutes_model = \"mistral\"" "$work/wholeheard-capture"
click 3; click 3; sleep 4
check test -s "$work/sessions/2026-10-13_14-05/minutes.md"
check is "$(tail -1 "$work/summarised")" "$work/sessions/2026-10-13_14-05 --provider http://127.0.0.1:9/v1 --model mistral"
check is "$(titles 'Compte rendu prêt')" 3
click 6

# The window (cycle 03): Wholeheard started with --window by tauri-driver, its page read over
# WebDriver (scripts/e2e_window.py). For a few seconds a window appears, twice.
if ! command -v WebKitWebDriver >/dev/null; then
  say "H. the window: NOT RUN — WebKitGTK's WebDriver is missing (Fedora: webkit2gtk4.1-devel)"
  failed=1
else
  command -v tauri-driver >/dev/null || cargo install tauri-driver --locked --version 2.1.0 2>&1 | tail -1 >> .evidence/e2e.txt
  mkdir -p "$work/window"
  # A window that stops answering fails the run, never holds it: it takes about 6 minutes.
  if ! timeout --kill-after=30 1200 uv run --quiet --with selenium==4.51.0 python scripts/e2e_window.py "$bin" "$work/window" | tee -a .evidence/e2e.txt; then
    failed=1
  fi
fi

# Its log (ADR-0007): each recording and making under an id, never a path of the run's folder.
shell_log=$(cat "$work"/state/wholeheard/logs/shell.*.jsonl 2>/dev/null || true)
check grep -q '"message":"shell.recording_started"' <<< "$shell_log"
check grep -q '"message":"shell.minutes_written"' <<< "$shell_log"
check test -z "$(grep -F "$work" <<< "$shell_log")"
check test ! -s "$work/errors"
[ -s "$work/errors" ] && sed 's/^/    /' "$work/errors" >> .evidence/e2e.txt
[ "$failed" = 0 ] && say "e2e: passed" || say "e2e: FAILED"
exit "$failed"
