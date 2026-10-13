#!/usr/bin/env bash
# The module's end-to-end scenarios (cycle 01, D3), on sound made here and on devices made
# for the run: nobody is recorded. It needs a PipeWire or PulseAudio desktop; where there is
# none — a CI runner — or where a microphone is in use, it says so and runs nothing.
#
# For about a minute the system's default output and input are two silent devices of its
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
for tool in ffmpeg pw-play python3; do
  command -v "$tool" >/dev/null || { say "e2e: NOT RUN — $tool is missing"; exit 0; }
done

cargo build --release --locked >/dev/null 2>&1
bin=$PWD/target/release/summit-capture
work=$(mktemp -d)
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

# "me" is a 440 Hz note; "others" is an 880 Hz one, with fourteen seconds of nothing in it.
tone() { ffmpeg -loglevel error -y -f lavfi -i "sine=frequency=$1:duration=$2" -af volume=0.5 -ar 48000 -ac 2 "$3"; }
tone 440 40 "$work/me.wav"; tone 880 8 "$work/others-a.wav"; tone 880 8 "$work/others-b.wav"

measure() { python3 scripts/e2e_measure.py "$@"; }
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
kill -INT "$recorder"; wait "$recorder" || true
sed 's/^/    /' "$work/a.events" >> .evidence/e2e.txt
check grep -q '"event":"silent","source":"others"' "$work/a.events"
check grep -q '"event":"heard","source":"others"' "$work/a.events"
check grep -q '"event":"stopped"' "$work/a.events"
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
check measure killed "$folder" "$killed_at" 5

[ "$failed" = 0 ] && say "e2e: passed" || say "e2e: FAILED"
exit "$failed"
