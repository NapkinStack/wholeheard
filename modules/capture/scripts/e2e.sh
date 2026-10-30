#!/usr/bin/env bash
# The module's end-to-end scenarios (cycle 01, D3), on sound made here and on devices made
# for the run: nobody is recorded. It needs a PipeWire or PulseAudio desktop; where there is
# none — a CI runner — or where a microphone is in use, it says so and runs nothing.
#
# For about two minutes the system's default output and input are two silent devices of its
# own; they are put back whatever happens.
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p .evidence
: > .evidence/e2e.txt
say() { echo "$*" | tee -a .evidence/e2e.txt; }
# The run's logs, where NapkinStack reads them after it (nstack e2e capture).
evidence=${NSTACK_EVIDENCE:-$PWD/.evidence/e2e}

if ! command -v pactl >/dev/null || ! pactl info >/dev/null 2>&1; then
  say "e2e: NOT RUN — no sound server here"; exit 77
fi
if [ -n "$(pactl list short source-outputs)" ]; then
  say "e2e: NOT RUN — an application holds a microphone: a call may be in progress"; exit 77
fi
if pactl list short sinks | awk -v d="$(pactl get-default-sink)" '$2 == d && $NF == "RUNNING" {found = 1} END {exit !found}'; then
  say "e2e: NOT RUN — the system is playing sound: its output would be taken from it"; exit 77
fi
for tool in ffmpeg ffprobe pw-play pw-dump pw-cli pw-loopback pw-metadata python3; do
  command -v "$tool" >/dev/null || { say "e2e: NOT RUN — $tool is missing"; exit 77; }
done

cargo build --release --locked 2>&1 | tail -3 >> .evidence/e2e.txt
# Its clocks are measured against the devices' own, which a loaded machine stamps late: a
# quiet one is waited for, a few minutes at most — the build just made counts too (#281).
quiet=$(( $(nproc) > 1 ? $(nproc) / 2 : 1 ))   # one processor: under 1
for _ in $(seq 60); do [ "$(cut -d. -f1 /proc/loadavg)" -lt "$quiet" ] && break; sleep 5; done
if [ "$(cut -d. -f1 /proc/loadavg)" -ge "$quiet" ]; then
  say "e2e: NOT RUN — the machine is busy (load $(cut -d' ' -f1 /proc/loadavg), $(nproc) processors): a clock cannot be measured on it"; exit 77
fi
bin=${CARGO_TARGET_DIR:-$PWD/target}/release/wholeheard-capture   # where cargo built it
work=$(mktemp -d)
# Its logs in the run's evidence, this run's alone, never the user's (ADR-0007).
export WHOLEHEARD_LOG_DIR="$evidence/logs"
rm -rf "$WHOLEHEARD_LOG_DIR"
# A run that was killed outright could not put anything back: its devices are removed first,
# and the system falls back on its own.
for stale in $(pactl list short modules | awk '/wholeheard_e2e_/ {print $1}'); do
  say "e2e: a device of an earlier run that was killed is removed"; pactl unload-module "$stale" || true
done
old_sink=$(pactl get-default-sink); old_source=$(pactl get-default-source)
out=$(pactl load-module module-null-sink sink_name=wholeheard_e2e_out)
mic=$(pactl load-module module-null-sink sink_name=wholeheard_e2e_mic)
restore() {
  pkill -P $$ 2>/dev/null || true
  pactl set-default-sink "$old_sink" 2>/dev/null || true
  pactl set-default-source "$old_source" 2>/dev/null || true
  pactl unload-module "$out" 2>/dev/null || true
  pactl unload-module "$mic" 2>/dev/null || true
  # Scenario J's own, when it was stopped before it took them away.
  for module in ${mic_b:-} ${out_b:-} ${out_c:-} ${mic_c:-}; do pactl unload-module "$module" 2>/dev/null || true; done
  rm -rf "$work"
}
trap restore EXIT
pactl set-default-sink wholeheard_e2e_out
pactl set-default-source wholeheard_e2e_mic.monitor

# "me" is a 440 Hz note; "others" is an 880 Hz one, with fourteen seconds of nothing in it.
tone() { ffmpeg -loglevel error -y -f lavfi -i "sine=frequency=$1:duration=$2" -af volume=0.5 -ar 48000 -ac 2 "$3"; }
tone 440 40 "$work/me.wav"; tone 880 8 "$work/others-a.wav"; tone 880 8 "$work/others-b.wav"

measure() { python3 scripts/e2e_measure.py "$@" 2>&1 | tee -a .evidence/e2e.txt; return "${PIPESTATUS[0]}"; }
# Whether the run of an id logged these steps, in this order among its own, no level, and never a path of the run's folder.
logged() {
  python3 - "$work" "$@" <<'PY'
import json, os, pathlib, sys
work, trace, wanted = sys.argv[1], sys.argv[2], sys.argv[3:]
text = "".join(path.read_text() for path in sorted(pathlib.Path(os.environ["WHOLEHEARD_LOG_DIR"]).glob("capture.*.jsonl")))
steps = [line["message"] for line in map(json.loads, text.splitlines()) if line.get("span", {}).get("trace") == trace]
mine = iter(steps)
# The levels, ten a second, are said to the shell and never logged.
sys.exit(0 if all(step in mine for step in wanted) and work not in text and "capture.level" not in steps else f"{trace}: {steps}")
PY
}
failed=0
check() { if "$@"; then say "  ok    $*"; else say "  FAIL  $*"; failed=1; fi; }

say "A. a recording stopped as a user stops it"
pw-play --target wholeheard_e2e_mic "$work/me.wav" & mic_player=$!
sleep 0.5
"$bin" record "$work/sessions" --as v1 > "$work/a.events" 2> "$work/a.errors" &
recorder=$!
sleep 1; pw-play "$work/others-a.wav"                    # 8 s of the others
sleep 4; folder=$(ls -d "$work"/sessions/*/ | head -1)
cp "$folder/others.wav" "$work/copy.wav"; copied_at=$(date +%s.%N)   # read while it is written
sleep 10                                                 # 14 s of nothing: "silent" is owed
pw-play "$work/others-b.wav"; sleep 1                    # and "heard" when it comes back
kill -INT "$recorder"; stopped_as=0; wait "$recorder" || stopped_as=$?
sed 's/^/    /' "$work/a.events" "$work/a.errors" >> .evidence/e2e.txt
check test "$stopped_as" = 0 -a ! -s "$work/a.errors"
check measure events "$work/a.events"
check measure stopped "$folder" 440 880
check measure read-while-written "$work/copy.wav" "$folder/session.json" "$copied_at" 5
# Each default is named for the device it stands for, not as "default_input" or "default_output".
check grep -q '"me":"wholeheard_e2e_mic' "$work/a.events"
check grep -q '"others":"wholeheard_e2e_out' "$work/a.events"

say "B. a recording killed"
kill "$mic_player" 2>/dev/null || true
pw-play --target wholeheard_e2e_mic "$work/me.wav" & mic_player=$!
sleep 0.5
"$bin" record "$work/killed" --as v1 > "$work/b.events" 2>/dev/null &
recorder=$!
sleep 1; pw-play "$work/others-a.wav" & others_player=$!
sleep 7
kill -KILL "$recorder"; killed_at=$(date +%s.%N); wait "$recorder" 2>/dev/null || true
kill "$mic_player" "$others_player" 2>/dev/null || true
folder=$(ls -d "$work"/killed/*/ | head -1)
sed 's/^/    /' "$work/b.events" >> .evidence/e2e.txt
check measure killed "$folder" "$killed_at" 5

say "C. the two files on one clock"
# The same sound into both sources: what differs between the two files is the program's doing.
pactl set-default-source wholeheard_e2e_out.monitor
"$bin" record "$work/clock" --as v1 > "$work/c.events" 2>/dev/null &
recorder=$!
sleep 2; pw-play "$work/others-a.wav"; sleep 1
kill -INT "$recorder"; wait "$recorder" || true
check measure same-clock "$(ls -d "$work"/clock/*/ | head -1)" 0.1

say "D. whoever listened is gone"
# The reader takes "started" and leaves: every later event is said to a closed pipe.
mkfifo "$work/d.pipe"
"$bin" record "$work/alone" --as v1 > "$work/d.pipe" 2> "$work/d.errors" &
recorder=$!
head -n 1 "$work/d.pipe" > "$work/d.events"
sleep 13                                                 # "silent" is owed at ten seconds, to nobody
kill -TERM "$recorder" 2>/dev/null || true; left_as=0; wait "$recorder" || left_as=$?
sed 's/^/    /' "$work/d.events" "$work/d.errors" >> .evidence/e2e.txt
check test "$left_as" = 0 -a ! -s "$work/d.errors"
check grep -q '"stopped"' "$(ls -d "$work"/alone/*/ | head -1)session.json"

# The same three, each source as delivered (session-audio/v2, ADR-0004): what `record` writes
# unasked — E asks nothing, the others ask `--as v2`, which is the same. Their notes are
# made to tell a recorder that changes them: the second channel at half the first's level, so
# that channels mixed or swapped show, both resting away from zero, so that centring does, and
# in 24 bits, so that a track cut to 16 does.
tone() { ffmpeg -loglevel error -y -f lavfi -i "sine=frequency=$1:duration=$2" -af "volume=0.5,pan=stereo|c0=c0|c1=0.5*c0,dcshift=0.02" -ar 48000 -c:a pcm_s24le "$3"; }
tone 440 40 "$work/me.wav"; tone 880 8 "$work/others-a.wav"; tone 880 8 "$work/others-b.wav"
# How many gaps of zeros the run of an id logged — and where the first was, in seconds.
zeros() {
  python3 - "$work" "$@" <<'PY'
import json, os, pathlib, sys
work, trace, count = sys.argv[1], sys.argv[2], int(sys.argv[3])
lines = [json.loads(line) for path in pathlib.Path(os.environ["WHOLEHEARD_LOG_DIR"]).glob("capture.*.jsonl") for line in path.read_text().splitlines()]
gaps = [line for line in lines if line.get("span", {}).get("trace") == trace and line["message"] == "capture.zeros"]
first = float(sys.argv[4]) if len(sys.argv) > 4 else None
ok = len(gaps) == count and all(gap["track"] == "remote" for gap in gaps) and (first is None or abs(gaps[0]["at"] - first) < 0.5)
sys.exit(0 if ok else f"{trace}: {gaps}")
PY
}
measure2() { python3 scripts/e2e_measure_v2.py "$@" 2>&1 | tee -a .evidence/e2e.txt; return "${PIPESTATUS[0]}"; }
pactl set-default-source wholeheard_e2e_mic.monitor

say "E. each source as delivered: a recording stopped as a user stops it"
pw-play --target wholeheard_e2e_mic "$work/me.wav" & mic_player=$!
sleep 0.5
WHOLEHEARD_TRACE=e2e-e "$bin" record "$work/v2" > "$work/e.events" 2> "$work/e.errors" &
recorder=$!
sleep 1; pw-play "$work/others-a.wav"
sleep 14                                                 # nothing plays: "silent" is owed
pw-play "$work/others-b.wav"; sleep 1
kill -INT "$recorder"; stopped_as=0; wait "$recorder" || stopped_as=$?
kill "$mic_player" 2>/dev/null || true
folder=$(ls -d "$work"/v2/*/ | head -1)
sed 's/^/    /' "$work/e.events" "$work/e.errors" >> .evidence/e2e.txt
check test "$stopped_as" = 0 -a ! -s "$work/e.errors"
check measure events "$work/e.events"
check measure2 stopped "$folder" 440 880
check logged e2e-e capture.opened capture.opened capture.started capture.silent capture.heard capture.stopped
for name in local remote; do
  check test "$(ffprobe -v error -show_entries stream=codec_name -of csv=p=0 "$folder/$name.wav")" = pcm_s24le
done

say "F. each source as delivered: a recording killed"
pw-play --target wholeheard_e2e_mic "$work/me.wav" & mic_player=$!
sleep 0.5
"$bin" record "$work/v2-killed" --as v2 > /dev/null 2>&1 &
recorder=$!
sleep 1; pw-play "$work/others-a.wav" & others_player=$!
sleep 7
kill -KILL "$recorder"; killed_at=$(date +%s.%N); wait "$recorder" 2>/dev/null || true
kill "$mic_player" "$others_player" 2>/dev/null || true
check measure2 killed "$(ls -d "$work"/v2-killed/*/ | head -1)" "$killed_at" 5

say "G. each source as delivered: the clock places the same sound at the same instant"
pactl set-default-source wholeheard_e2e_out.monitor
"$bin" record "$work/v2-clock" --as v2 > /dev/null 2>&1 &
recorder=$!
sleep 2; pw-play "$work/others-a.wav"; sleep 1
kill -INT "$recorder"; wait "$recorder" || true
check measure2 same-instant "$(ls -d "$work"/v2-clock/*/ | head -1)" 0.005

say "H. each source as delivered: a source that stops, and comes back — twice"
pactl set-default-source wholeheard_e2e_mic.monitor
tone 880 30 "$work/others-long.wav"
pw-play --target wholeheard_e2e_mic "$work/me.wav" & mic_player=$!
pw-play "$work/others-long.wav" & others_player=$!
sleep 0.5
WHOLEHEARD_TRACE=e2e-h "$bin" record "$work/v2-stops" --as v2 > /dev/null 2>&1 &
recorder=$!
sleep 4; python3 scripts/e2e_measure_v2.py pause "$recorder" remote 6   # six seconds without the remote source
# Back for three seconds, then gone again: each gap is logged, the second as the first (#285).
sleep 3; python3 scripts/e2e_measure_v2.py pause "$recorder" remote 4
sleep 3
kill -INT "$recorder"; wait "$recorder" || true
kill "$mic_player" "$others_player" 2>/dev/null || true
check measure2 stopped-source "$(ls -d "$work"/v2-stops/*/ | head -1)"
check logged e2e-h capture.started capture.zeros capture.stopped
check zeros e2e-h 2

say "I. each source as delivered: a source mute from its start"
# Its stream paused as soon as it is there: it delivers nothing, and its zeros are logged once.
pw-play --target wholeheard_e2e_mic "$work/me.wav" & mic_player=$!
sleep 0.5
WHOLEHEARD_TRACE=e2e-i "$bin" record "$work/v2-mute" --as v2 > /dev/null 2>&1 &
recorder=$!
for _ in $(seq 40); do python3 scripts/e2e_measure_v2.py pause "$recorder" remote 5 2>/dev/null && break; sleep 0.05; done
pw-play "$work/others-a.wav" & others_player=$!
sleep 3
kill -INT "$recorder"; wait "$recorder" || true
kill "$mic_player" "$others_player" 2>/dev/null || true
check zeros e2e-i 1 0

say "J. a recording that follows the devices (D31): switched to others of their own form, and one taken away"
# A microphone of one channel at 16 kHz and an output of six at 44.1 kHz: the tracks keep the
# form they opened with. Every note is played on a device of the run, never on the system's.
mic_b=$(pactl load-module module-null-sink sink_name=wholeheard_e2e_mic_b rate=16000 channels=1)
out_b=$(pactl load-module module-null-sink sink_name=wholeheard_e2e_out_b rate=44100 channels=6 channel_map=surround-51)
tone 660 4 "$work/660.wav"; tone 1320 4 "$work/1320.wav"; tone 440 4 "$work/440.wav"; tone 880 4 "$work/880.wav"
both() { pw-play --target "$1" "$work/$2.wav" & local p=$!; pw-play --target "$3" "$work/$4.wav"; wait "$p"; }
WHOLEHEARD_TRACE=e2e-j "$bin" record "$work/v2-moved" --as v2 > "$work/j.events" 2> "$work/j.errors" &
recorder=$!
# The first form, beside it: its lines are the same.
"$bin" record "$work/v1-moved" --as v1 > "$work/j1.events" 2> /dev/null &
recorder_v1=$!
sleep 1; both wholeheard_e2e_mic 440 wholeheard_e2e_out 880
pactl set-default-source wholeheard_e2e_mic_b.monitor; pactl set-default-sink wholeheard_e2e_out_b
sleep 1; both wholeheard_e2e_mic_b 660 wholeheard_e2e_out_b 1320
cp "$(ls -d "$work"/v2-moved/*/ | head -1)session.json" "$work/j-during.json"   # read while it records
# The microphone chosen back; the output recorded taken away: the system names the next.
pactl set-default-source wholeheard_e2e_mic.monitor
pactl unload-module "$mic_b"
pactl unload-module "$out_b"
sleep 1; pactl set-default-sink wholeheard_e2e_out
sleep 1; both wholeheard_e2e_mic 440 wholeheard_e2e_out 880
sleep 1
kill -INT "$recorder" "$recorder_v1"; stopped_as=0; wait "$recorder" || stopped_as=$?; wait "$recorder_v1" || true
grep -v '"level"' "$work/j.events" | sed 's/^/    /' >> .evidence/e2e.txt
sed 's/^/    /' "$work/j.errors" >> .evidence/e2e.txt
check test "$stopped_as" = 0 -a ! -s "$work/j.errors"
check measure2 followed "$(ls -d "$work"/v2-moved/*/ | head -1)" "$work/j.events" \
  "wholeheard_e2e_mic_b:660,wholeheard_e2e_mic:440" "wholeheard_e2e_out_b:1320,wholeheard_e2e_out:880" "$work/j-during.json"
check measure2 moves-said "$work/j1.events" "wholeheard_e2e_mic_b,wholeheard_e2e_mic" "wholeheard_e2e_out_b,wholeheard_e2e_out"
check logged e2e-j capture.started capture.device capture.device capture.stopped

say "K. a call heard on an output other than the default is recorded there (D41)"
# The call's own output, beside the default; the stand-in call, one process as a call is: it
# records the microphone and plays what it hears on that output. Nobody is recorded.
out_c=$(pactl load-module module-null-sink sink_name=wholeheard_e2e_call)
tone 1100 4 "$work/1100.wav"; tone 660 4 "$work/660.wav"; tone 440 4 "$work/440.wav"
pw-loopback --capture=wholeheard_e2e_mic.monitor --playback=wholeheard_e2e_call --name=wholeheard-e2e-call & call=$!
sleep 1
WHOLEHEARD_TRACE=e2e-k "$bin" record "$work/v2-call" --as v2 > "$work/k.events" 2> "$work/k.errors" &
recorder=$!
sleep 3; pw-play --target wholeheard_e2e_mic "$work/1100.wav"   # heard by the call, played on its output
# The call goes back to the default output: so does the remote side, within two seconds.
playing=$(python3 scripts/e2e_measure_v2.py playing "$call")
pw-metadata -n default "$playing" target.object wholeheard_e2e_out > /dev/null
sleep 2; pw-play --target wholeheard_e2e_call "$work/660.wav"   # two seconds after: the remote side followed, not on it
sleep 1; pw-play --target wholeheard_e2e_out "$work/440.wav"
sleep 1
kill -INT "$recorder"; stopped_as=0; wait "$recorder" || stopped_as=$?
kill "$call" 2>/dev/null || true; wait "$call" 2>/dev/null || true
pactl unload-module "$out_c"
grep -v '"level"' "$work/k.events" | sed 's/^/    /' >> .evidence/e2e.txt
sed 's/^/    /' "$work/k.errors" >> .evidence/e2e.txt
check test "$stopped_as" = 0 -a ! -s "$work/k.errors"
check measure2 call-followed "$(ls -d "$work"/v2-call/*/ | head -1)" "$work/k.events" wholeheard_e2e_call wholeheard_e2e_out 1100 660 440
check logged e2e-k capture.started capture.device capture.device capture.stopped

say "L. the microphone a call holds, and a headset's never opened actively (D66, ADR-0015)"
# A source of the run's, fed by a device of the run's, made as WirePlumber makes a headset's virtual
# input: stand_in NAME [more properties of the source]. No Bluetooth card: its script is not at work.
mic_c=$(pactl load-module module-null-sink sink_name=wholeheard_e2e_mic_c)
stand_in() {
  pw-loopback --name="$1-loopback" \
    --capture-props="{ node.name=$1_in media.class=Stream/Input/Audio/Internal target.object=wholeheard_e2e_mic_c stream.capture.sink=true node.passive=true node.dont-fallback=true node.linger=true ${2:-} }" \
    --playback-props="{ node.name=$1 node.description=$1 media.class=Audio/Source ${2:-} }" &
}
call_on() { pw-record --target "$1" "$work/call-$$.wav" & }
tone 660 30 "$work/660-long.wav"
pw-play --target wholeheard_e2e_mic_c "$work/660-long.wav" & mic_player=$!

# A call on a microphone other than the default: recorded, named, and left when it leaves.
stand_in wholeheard_e2e_other; other=$!
sleep 1
WHOLEHEARD_TRACE=e2e-l "$bin" record "$work/v2-mic" --as v2 > "$work/l.events" 2> "$work/l.errors" &
recorder=$!
sleep 2; call_on wholeheard_e2e_other; call=$!
sleep 4; folder=$(ls -d "$work"/v2-mic/*/ | head -1); cp "$folder/session.json" "$work/l-during.json"
kill "$call" 2>/dev/null || true; wait "$call" 2>/dev/null || true
sleep 3
kill -INT "$recorder"; stopped_as=0; wait "$recorder" || stopped_as=$?
kill "$other" 2>/dev/null || true; wait "$other" 2>/dev/null || true
grep -v '"level"' "$work/l.events" | sed 's/^/    /' >> .evidence/e2e.txt
sed 's/^/    /' "$work/l.errors" >> .evidence/e2e.txt
check test "$stopped_as" = 0 -a ! -s "$work/l.errors"
check grep -q '"device":"wholeheard_e2e_other"' "$work/l.events"
check grep -q 'wholeheard_e2e_other' "$work/l-during.json"
check grep -q '"device":"wholeheard_e2e_mic' "$work/l.events"    # back on the default when it left

# The default is a headset's, and no call: the microphone waits, nothing opens it. A call takes it:
# a passive stream, which it carries. It leaves: capture lets go, and the source no longer runs.
stand_in wholeheard_e2e_headset "bluez5.loopback=true"; headset=$!
sleep 1
pactl set-default-source wholeheard_e2e_headset
WHOLEHEARD_TRACE=e2e-m "$bin" record "$work/v2-headset" --as v2 > "$work/m.events" 2> "$work/m.errors" &
recorder=$!
sleep 3
folder=$(ls -d "$work"/v2-headset/*/ | head -1)
check grep -q 'unavailable.*waits' "$work/m.events"
check test ! -e "$folder/local.wav"
# The stand-in runs whenever its sink is played to, unlike a real headset: what is judged is that
# nothing of capture is on it. WirePlumber's own script is judged on real AirPods alone (D66).
check measure2 not-following "$recorder"
call_on wholeheard_e2e_headset; call=$!
sleep 3
check grep -q '"device":"wholeheard_e2e_headset"' "$work/m.events"
check measure2 in-follow "$recorder"
check test -e "$folder/local.wav"
kill "$call" 2>/dev/null || true; wait "$call" 2>/dev/null || true
sleep 3     # what WirePlumber's restore takes, 2 s, and a reading of the graph
check measure2 not-following "$recorder"
# The stand-in runs whenever its sink is played to, unlike a real headset: what is judged is that
# nothing of capture is on it. WirePlumber's own script is judged on real AirPods alone (D66).
check measure2 not-following "$recorder"
check grep -q '"device":null' "$work/m.events"
pw-play --target wholeheard_e2e_out "$work/880.wav"
kill -INT "$recorder"; stopped_as=0; wait "$recorder" || stopped_as=$?
kill "$mic_player" 2>/dev/null || true; wait "$mic_player" 2>/dev/null || true
kill "$headset" 2>/dev/null || true; wait "$headset" 2>/dev/null || true
pactl set-default-source wholeheard_e2e_mic.monitor
pactl unload-module "$mic_c"; mic_c=
grep -v '"level"' "$work/m.events" | sed 's/^/    /' >> .evidence/e2e.txt
sed 's/^/    /' "$work/m.errors" >> .evidence/e2e.txt
check test "$stopped_as" = 0 -a ! -s "$work/m.errors"
check measure2 waited "$folder"
check logged e2e-m capture.unavailable capture.started capture.opened capture.device capture.me_waits capture.stopped
# Never an address, in a line of the output, a session or a log: a stand-in has none, this holds the path.
check test -z "$(grep -lE '([0-9A-Fa-f]{2}[:_-]){5}[0-9A-Fa-f]{2}' "$work/m.events" "$folder/session.json" "$WHOLEHEARD_LOG_DIR"/capture.*.jsonl 2>/dev/null)"

[ "$failed" = 0 ] && say "e2e: passed" || say "e2e: FAILED"
exit "$failed"
