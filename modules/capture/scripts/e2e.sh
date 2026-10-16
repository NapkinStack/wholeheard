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

if ! command -v pactl >/dev/null || ! pactl info >/dev/null 2>&1; then
  say "e2e: NOT RUN — no sound server here"; exit 0
fi
if [ -n "$(pactl list short source-outputs)" ]; then
  say "e2e: NOT RUN — an application holds a microphone: a call may be in progress"; exit 0
fi
if pactl list short sinks | awk -v d="$(pactl get-default-sink)" '$2 == d && $NF == "RUNNING" {found = 1} END {exit !found}'; then
  say "e2e: NOT RUN — the system is playing sound: its output would be taken from it"; exit 0
fi
for tool in ffmpeg ffprobe pw-play pw-dump pw-cli python3; do
  command -v "$tool" >/dev/null || { say "e2e: NOT RUN — $tool is missing"; exit 0; }
done

cargo build --release --locked 2>&1 | tail -3 >> .evidence/e2e.txt
bin=${CARGO_TARGET_DIR:-$PWD/target}/release/summit-capture   # where cargo built it
work=$(mktemp -d)
# A run that was killed outright could not put anything back: its devices are removed first,
# and the system falls back on its own.
for stale in $(pactl list short modules | awk '/summit_e2e_/ {print $1}'); do
  say "e2e: a device of an earlier run that was killed is removed"; pactl unload-module "$stale" || true
done
old_sink=$(pactl get-default-sink); old_source=$(pactl get-default-source)
out=$(pactl load-module module-null-sink sink_name=summit_e2e_out)
mic=$(pactl load-module module-null-sink sink_name=summit_e2e_mic)
restore() {
  pkill -P $$ 2>/dev/null || true
  pactl set-default-sink "$old_sink" 2>/dev/null || true
  pactl set-default-source "$old_source" 2>/dev/null || true
  pactl unload-module "$out" 2>/dev/null || true
  pactl unload-module "$mic" 2>/dev/null || true
  rm -rf "$work"
}
trap restore EXIT
pactl set-default-sink summit_e2e_out
pactl set-default-source summit_e2e_mic.monitor
# A stream another application holds open follows the new default: it would play into what is
# recorded. Each goes back to the output it was on; one opened during the run still follows.
sleep 0.5
for stream in $(pactl list short sink-inputs | awk '{print $1}'); do
  pactl move-sink-input "$stream" "$old_sink" 2>/dev/null || true
done

# "me" is a 440 Hz note; "others" is an 880 Hz one, with fourteen seconds of nothing in it.
tone() { ffmpeg -loglevel error -y -f lavfi -i "sine=frequency=$1:duration=$2" -af volume=0.5 -ar 48000 -ac 2 "$3"; }
tone 440 40 "$work/me.wav"; tone 880 8 "$work/others-a.wav"; tone 880 8 "$work/others-b.wav"

measure() { python3 scripts/e2e_measure.py "$@" 2>&1 | tee -a .evidence/e2e.txt; return "${PIPESTATUS[0]}"; }
failed=0
check() { if "$@"; then say "  ok    $*"; else say "  FAIL  $*"; failed=1; fi; }

say "A. a recording stopped as a user stops it"
pw-play --target summit_e2e_mic "$work/me.wav" & mic_player=$!
sleep 0.5
"$bin" record "$work/sessions" > "$work/a.events" 2> "$work/a.errors" &
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

say "B. a recording killed"
kill "$mic_player" 2>/dev/null || true
pw-play --target summit_e2e_mic "$work/me.wav" & mic_player=$!
sleep 0.5
"$bin" record "$work/killed" > "$work/b.events" 2>/dev/null &
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
pactl set-default-source summit_e2e_out.monitor
"$bin" record "$work/clock" > "$work/c.events" 2>/dev/null &
recorder=$!
sleep 2; pw-play "$work/others-a.wav"; sleep 1
kill -INT "$recorder"; wait "$recorder" || true
check measure same-clock "$(ls -d "$work"/clock/*/ | head -1)" 0.1

say "D. whoever listened is gone"
# The reader takes "started" and leaves: every later event is said to a closed pipe.
mkfifo "$work/d.pipe"
"$bin" record "$work/alone" > "$work/d.pipe" 2> "$work/d.errors" &
recorder=$!
head -n 1 "$work/d.pipe" > "$work/d.events"
sleep 13                                                 # "silent" is owed at ten seconds, to nobody
kill -TERM "$recorder" 2>/dev/null || true; left_as=0; wait "$recorder" || left_as=$?
sed 's/^/    /' "$work/d.events" "$work/d.errors" >> .evidence/e2e.txt
check test "$left_as" = 0 -a ! -s "$work/d.errors"
check grep -q '"stopped"' "$(ls -d "$work"/alone/*/ | head -1)session.json"

# The same three, each source as delivered (session-audio/v2, ADR-0004). Their notes are
# made to tell a recorder that changes them: the second channel at half the first's level, so
# that channels mixed or swapped show, both resting away from zero, so that centring does, and
# in 24 bits, so that a track cut to 16 does.
tone() { ffmpeg -loglevel error -y -f lavfi -i "sine=frequency=$1:duration=$2" -af "volume=0.5,pan=stereo|c0=c0|c1=0.5*c0,dcshift=0.02" -ar 48000 -c:a pcm_s24le "$3"; }
tone 440 40 "$work/me.wav"; tone 880 8 "$work/others-a.wav"; tone 880 8 "$work/others-b.wav"
measure2() { python3 scripts/e2e_measure_v2.py "$@" 2>&1 | tee -a .evidence/e2e.txt; return "${PIPESTATUS[0]}"; }
pactl set-default-source summit_e2e_mic.monitor

say "E. each source as delivered: a recording stopped as a user stops it"
pw-play --target summit_e2e_mic "$work/me.wav" & mic_player=$!
sleep 0.5
"$bin" record "$work/v2" --as v2 > "$work/e.events" 2> "$work/e.errors" &
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
for name in local remote; do
  check test "$(ffprobe -v error -show_entries stream=codec_name -of csv=p=0 "$folder/$name.wav")" = pcm_s24le
done

say "F. each source as delivered: a recording killed"
pw-play --target summit_e2e_mic "$work/me.wav" & mic_player=$!
sleep 0.5
"$bin" record "$work/v2-killed" --as v2 > /dev/null 2>&1 &
recorder=$!
sleep 1; pw-play "$work/others-a.wav" & others_player=$!
sleep 7
kill -KILL "$recorder"; killed_at=$(date +%s.%N); wait "$recorder" 2>/dev/null || true
kill "$mic_player" "$others_player" 2>/dev/null || true
check measure2 killed "$(ls -d "$work"/v2-killed/*/ | head -1)" "$killed_at" 5

say "G. each source as delivered: the clock places the same sound at the same instant"
pactl set-default-source summit_e2e_out.monitor
"$bin" record "$work/v2-clock" --as v2 > /dev/null 2>&1 &
recorder=$!
sleep 2; pw-play "$work/others-a.wav"; sleep 1
kill -INT "$recorder"; wait "$recorder" || true
check measure2 same-instant "$(ls -d "$work"/v2-clock/*/ | head -1)" 0.005

say "H. each source as delivered: a source that stops, and comes back"
pactl set-default-source summit_e2e_mic.monitor
tone 880 30 "$work/others-long.wav"
pw-play --target summit_e2e_mic "$work/me.wav" & mic_player=$!
pw-play "$work/others-long.wav" & others_player=$!
sleep 0.5
"$bin" record "$work/v2-stops" --as v2 > /dev/null 2>&1 &
recorder=$!
sleep 4; python3 scripts/e2e_measure_v2.py pause "$recorder" remote 6   # six seconds without the remote source
sleep 6
kill -INT "$recorder"; wait "$recorder" || true
kill "$mic_player" "$others_player" 2>/dev/null || true
check measure2 stopped-source "$(ls -d "$work"/v2-stops/*/ | head -1)"

[ "$failed" = 0 ] && say "e2e: passed" || say "e2e: FAILED"
exit "$failed"
