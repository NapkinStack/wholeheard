#!/usr/bin/env bash
# What a desktop's own graph must not be put through (D66, ADR-0015), on a PipeWire and a WirePlumber
# of its own: their own runtime folder, no hardware, the desktop's never spoken to. Safe while a call
# or another run goes on. Nobody is recorded: every sound is a note made here.
#
#   private_graph.sh [capture's binary]
#
# Passive: with a stand-in source a call may hold, capture's stream never makes it run, whoever else
#   is linked; a call running carries it, its sound in the track.
# Started during a call: what cut a call in #494, passive from its first frame.
# Starved: the session manager stuck, a stream that never negotiates: the recording's loop goes on,
#   the healthy track gets no false zeros, and the stop is prompt.
set -euo pipefail
cd "$(dirname "$0")/.."
bin=$(realpath "${1:-${CARGO_TARGET_DIR:-$PWD/target}/release/wholeheard-capture}")
for tool in pipewire wireplumber pw-cli pw-dump pw-metadata pw-loopback pw-record pw-play ffmpeg python3; do
  command -v "$tool" >/dev/null || { echo "private graph: NOT RUN — $tool is missing"; exit 77; }
done
# A socket's path is short: never under a long folder.
work=$(mktemp -d /tmp/wh-XXXXXX)
real_dump=$(command -v pw-dump)
export XDG_RUNTIME_DIR=$work/rt PIPEWIRE_RUNTIME_DIR=$work/rt XDG_CONFIG_HOME=$work/cfg XDG_STATE_HOME=$work/state
export XDG_CACHE_HOME=$work/cache XDG_DATA_HOME=$work/data WHOLEHEARD_LOG_DIR=$work/logs DBUS_SESSION_BUS_ADDRESS=disabled:
unset PIPEWIRE_REMOTE DISPLAY WAYLAND_DISPLAY
mkdir -p "$work/rt" "$work/cfg/wireplumber/wireplumber.conf.d" "$work/shim" "$work/logs"
chmod 700 "$work/rt"
cat > "$work/cfg/wireplumber/wireplumber.conf.d/90-no-hardware.conf" <<CONF
wireplumber.profiles = { main = { monitor.alsa = disabled, monitor.bluez = disabled, monitor.bluez-midi = disabled, monitor.alsa-midi = disabled, monitor.v4l2 = disabled, monitor.libcamera = disabled, support.logind = disabled, support.reserve-device = disabled, support.portal-permissionstore = disabled } }
CONF
pipewire > "$work/pipewire.log" 2>&1 & daemon=$!
for _ in $(seq 20); do [ -S "$work/rt/pipewire-0" ] && break; sleep 0.25; done
wireplumber > "$work/wireplumber.log" 2>&1 & manager=$!
others=()
restore() {
  kill -CONT "$manager" 2>/dev/null || true
  kill "${others[@]:-}" "$manager" "$daemon" 2>/dev/null || true
  wait 2>/dev/null || true
  rm -rf "$work"
}
trap restore EXIT
sleep 2

failed=0
say() { echo "$*"; }
check() { if "$@"; then say "  ok    $*"; else say "  FAIL  $*"; failed=1; fi; }
measure2() { python3 scripts/e2e_measure_v2.py "$@"; }
sink() { pw-cli create-node adapter factory.name=support.null-audio-sink node.name="$1" node.description="$1" media.class=Audio/Sink object.linger=true audio.position=[FL,FR] > /dev/null; }
default_source() { pw-metadata -n default 0 default.configured.audio.source "{ \"name\": \"$1\" }" Spa:String:JSON > /dev/null; }
stand_in() {
  pw-loopback --name="$1-loopback" \
    --capture-props="{ node.name=$1_in media.class=Stream/Input/Audio/Internal target.object=wholeheard_e2e_mic_c stream.capture.sink=true node.passive=true node.dont-fallback=true node.linger=true ${2:-} }" \
    --playback-props="{ node.name=$1 node.description=$1 media.class=Audio/Source ${2:-} }" &
  others+=($!)
}
ffmpeg -loglevel error -y -f lavfi -i "sine=frequency=660:duration=30" -af volume=0.5 -ar 48000 -ac 2 "$work/660.wav"
ffmpeg -loglevel error -y -f lavfi -i "sine=frequency=880:duration=30" -af volume=0.5 -ar 48000 -ac 2 "$work/880.wav"
sink wholeheard_e2e_out; sink wholeheard_e2e_mic_c
stand_in wholeheard_e2e_headset "bluez5.loopback=true"
stand_in wholeheard_e2e_plain
stand_in wholeheard_e2e_other
sleep 2

say "Passive: the source, capture's stream, and a call"
default_source wholeheard_e2e_headset
sleep 1
"$bin" record "$work/passive" --as v2 > "$work/p.events" 2> "$work/p.errors" & recorder=$!
sleep 3
folder=$(ls -d "$work"/passive/*/ | head -1)
check grep -q 'unavailable.*waits' "$work/p.events"
check test ! -e "$folder/local.wav"
check measure2 not-running wholeheard_e2e_headset 2
# A call linked but not running, as a browser holding the microphone idle: capture's stream is
# linked, alone in asking nothing of the source.
pw-record --target wholeheard_e2e_headset -P '{ node.passive=in node.name=call-idle }' "$work/idle.wav" & idle=$!
sleep 3
check measure2 passive "$recorder"
check measure2 not-running wholeheard_e2e_headset 3
check test ! -e "$folder/local.wav"    # linked, and no form before the source runs
# It runs: capture hears it.
pw-record --target wholeheard_e2e_headset -P '{ node.name=call-active }' "$work/call.wav" & call=$!
pw-play --target wholeheard_e2e_mic_c "$work/660.wav" & player=$!
sleep 3
check grep -q '"device":"wholeheard_e2e_headset"' "$work/p.events"
check measure2 running wholeheard_e2e_headset
sleep 2
kill "$player" "$call" 2>/dev/null || true; wait "$player" "$call" 2>/dev/null || true
# It stops: capture's stream still linked, the source does not stay up for it.
sleep 1
check measure2 passive "$recorder"
check measure2 not-running wholeheard_e2e_headset 3
kill "$idle" 2>/dev/null || true; wait "$idle" 2>/dev/null || true
sleep 3
check measure2 not-following "$recorder"
check grep -q '"device":null' "$work/p.events"
check measure2 not-running wholeheard_e2e_headset 2
kill -INT "$recorder"; stopped_as=0; wait "$recorder" || stopped_as=$?
check test "$stopped_as" = 0 -a ! -s "$work/p.errors"
check measure2 waited "$folder"

say "Started during a call: the track at once, from the headset, passive"
pw-record --target wholeheard_e2e_headset -P '{ node.name=call-active }' "$work/call2.wav" & call=$!
pw-play --target wholeheard_e2e_mic_c "$work/660.wav" & player=$!
sleep 2
"$bin" record "$work/during" --as v2 > "$work/d.events" 2> "$work/d.errors" & recorder=$!
sleep 3
folder=$(ls -d "$work"/during/*/ | head -1)
check grep -q '"me":"wholeheard_e2e_headset"' "$work/d.events"
check test -e "$folder/local.wav"
check measure2 passive "$recorder"
kill "$player" "$call" 2>/dev/null || true; wait "$player" "$call" 2>/dev/null || true
sleep 3
kill -INT "$recorder"; stopped_as=0; wait "$recorder" || stopped_as=$?
check test "$stopped_as" = 0 -a ! -s "$work/d.errors"

say "Followed: a stream capture closes itself is no device lost"
default_source wholeheard_e2e_plain
sleep 1
"$bin" record "$work/followed" --as v2 > "$work/f.events" 2> "$work/f.errors" & recorder=$!
sleep 2
pw-record --target wholeheard_e2e_other -P '{ node.name=call-active }' "$work/call3.wav" & call=$!
sleep 3
check grep -q '"device":"wholeheard_e2e_other"' "$work/f.events"
kill "$call" 2>/dev/null || true; wait "$call" 2>/dev/null || true
sleep 3
check grep -q '"device":"wholeheard_e2e_plain"' "$work/f.events"
check test -z "$(grep '"device":null' "$work/f.events")"
kill -INT "$recorder"; wait "$recorder" || true

say "Starved: the session manager stuck, a stream that never negotiates"
default_source wholeheard_e2e_plain
cat > "$work/shim/pw-dump" <<SHIM
#!/bin/sh
[ -f "$work/hold.json" ] && exec cat "$work/hold.json"
exec "$real_dump" "\$@"
SHIM
chmod +x "$work/shim/pw-dump"
sleep 1
# What the graph is said to be, later: a call holding the stand-in headset.
pw-record --target wholeheard_e2e_headset -P '{ node.passive=in node.name=call-idle }' "$work/idle2.wav" & idle=$!
sleep 2
"$real_dump" > "$work/hold.tmp"
kill "$idle" 2>/dev/null || true; wait "$idle" 2>/dev/null || true
sleep 1
PATH="$work/shim:$PATH" "$bin" record "$work/starved" --as v2 > "$work/s.events" 2> "$work/s.errors" & recorder=$!
pw-play --target wholeheard_e2e_out "$work/880.wav" & player=$!
sleep 3
kill -STOP "$manager"
before=$(grep -c '"level"' "$work/s.events" || true)
mv "$work/hold.tmp" "$work/hold.json"
sleep 6
# Not `pw-dump`: it waits on the metadata, the stopped manager's. The registry answers.
timeout 3 pw-cli ls Node > "$work/nodes.txt" || true
timeout 3 pw-cli ls Link > "$work/links.txt" || true
sleep 6
after=$(grep -c '"level"' "$work/s.events" || true)
stopping=$(date +%s.%N)
kill -INT "$recorder"; stopped_as=0; wait "$recorder" || stopped_as=$?
took=$(python3 -c "import time; print(f'{time.time() - $stopping:.1f}')")
kill "$player" 2>/dev/null || true; wait "$player" 2>/dev/null || true
kill -CONT "$manager"
folder=$(ls -d "$work"/starved/*/ | head -1)
say "    levels in 12 s: $((after - before)), stop took $took s"
check test "$stopped_as" = 0
check test "$((after - before))" -ge 60
check python3 -c "assert $took < 3.5, 'the stop took $took s'"
# It was truly in that state: capture's stream in the graph, and no session manager to link it.
check grep -q 'node.name = "wholeheard-capture-me"' "$work/nodes.txt"
capture_me=$(awk '/^\tid /{id=$2} /node.name = "wholeheard-capture-me"/{print id}' "$work/nodes.txt" | tr -d ,)
check test -z "$(grep -E "link.input.node = \"$capture_me\"" "$work/links.txt")"
# The healthy track, the remote, was delivered all along: no zeros said for it.
check python3 - "$folder/clock.jsonl" <<'PY'
import json, sys
lines = [json.loads(line) for line in open(sys.argv[1])]
false = [line["silence"] for line in lines if line.get("track") == "remote" and "silence" in line]
assert not false, f"zeros said on the healthy track: {false}"
PY

[ "$failed" = 0 ] && say "private graph: passed" || say "private graph: FAILED"
exit "$failed"
