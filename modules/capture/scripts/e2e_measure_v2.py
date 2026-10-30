"""What the end-to-end scenarios measure in a session of `session-audio/v2` — never listened
to, and the sound is a note made for the run."""
import datetime
import json
import os
import struct
import subprocess
import sys
import time

# What the run's own devices deliver, and what its notes are played as: ffmpeg's sine is an
# eighth of full scale, halved by the run, the second channel at half the first — around a
# rest away from zero.
DELIVERED = (2, 48000)
LEVELS = (0.125 * 0.5, 0.125 * 0.25)
REST = 0.02
FULL = 8_388_608
# A stream's first second is the sound server's: it may hand pieces of nothing, or one late
# with the next on its heels. What a track holds whole, and what its clock holds steady, is
# judged from there on — on every run, not on the runs that opened quietly.
SETTLED = 1.0


def track(folder, name):
    """A track read as the contract says: found by its chunks, its lengths ignored, to the end."""
    data = open(os.path.join(folder, f"{name}.wav"), "rb").read()
    assert data[:4] == b"RIFF" and data[8:12] == b"WAVE", f"{name}: not a track"
    at = 12
    while data[at:at + 4] != b"data":
        size = struct.unpack("<I", data[at + 4:at + 8])[0]
        if data[at:at + 4] == b"fmt ":
            channels, rate = struct.unpack("<HI", data[at + 10:at + 16])
            bits = struct.unpack("<H", data[at + 22:at + 24])[0]
        at += 8 + size
    assert bits == 24, f"{name}: {bits} bits"
    body = data[at + 8:]
    frames = len(body) // (3 * channels)
    assert (channels, rate) == DELIVERED, f"{name}: {channels} channel(s) at {rate} Hz, where the run's devices deliver {DELIVERED}"
    # The first channel is enough to find a note and where it starts; every channel has its loudest.
    first = [int.from_bytes(body[n * 3 * channels:n * 3 * channels + 3], "little", signed=True) for n in range(frames)]
    # Where each channel rests while it sounds, and its note's level around that rest — a
    # note's, not a click's: the level one sample in two hundred passes.
    peaks, rests = [], []
    for c in range(channels):
        played = [v for v in (int.from_bytes(body[n:n + 3], "little", signed=True) for n in range(3 * c, len(body) - 2, 3 * channels)) if v]
        rests.append(sum(played) / max(len(played), 1))
        around = sorted(abs(v - rests[-1]) for v in played)
        peaks.append(around[len(around) * 199 // 200] if around else 0)
    return {"name": name, "rate": rate, "channels": channels, "frames": frames, "first": first, "peaks": peaks, "rests": rests,
            "said": struct.unpack("<I", data[at + 4:at + 8])[0], "bytes": len(body), "body": body,
            "riff": struct.unpack("<I", data[4:8])[0], "file": len(data)}


def breaks(one, hertz, told):
    """Where a note played without a break is not whole in its track: two of its periods
    further apart, or closer, than a period — frames lost, added or left as zeros. Where the
    clock tells of something (`told`: the stretches it names, and the stops it places), what
    it does not cover between the two is two periods at most: the note left and came back."""
    rest, first, period = one["rests"][0], one["first"], one["rate"] / hertz
    found, last = [], None

    def whole(since, until):
        covered = sum(max(0, min(b, until) - max(a, since)) for a, b in told)
        if any(a <= until and since <= b for a, b in told):
            return until - since - covered <= 2 * period + 2
        return abs(until - since - period) <= 1.5

    for at in range(int(SETTLED * one["rate"]), one["frames"] - 1):
        if first[at] - rest < 0 <= first[at + 1] - rest:
            if last is not None and not whole(last, at):
                found.append((last, at))
            last = at
    if last is not None and one["frames"] - last > 2 * period and not whole(last, one["frames"]) and not any(b >= one["frames"] for _, b in told):
        found.append((last, one["frames"]))
    return found


def pitch(one):
    """The note the track holds, in Hz: its zero crossings while it sounds."""
    rest = one["rests"][0]
    first = [value - rest for value in one["first"]]
    loud = [at for at, value in enumerate(one["first"]) if abs(value) > 30_000]
    if len(loud) < one["rate"]:
        return 0
    crossings = sounding = 0
    for at in range(loud[0], loud[-1]):
        # Either: a note may fall on exactly zero at each crossing, and zeros of a gap are both.
        if one["first"][at] or one["first"][at + 1]:
            sounding += 1
            crossings += first[at] < 0 <= first[at + 1]
    return crossings * one["rate"] / max(sounding, 1)


def onset(one):
    return next((at for at, value in enumerate(one["first"]) if abs(value) > 30_000), None)


def session(folder):
    read = json.load(open(os.path.join(folder, "session.json")))
    assert read["contract"] == "session-audio/v2", read
    return read


def clock(folder):
    """The clock's lines; one cut by a kill is the last and is passed over."""
    lines = open(os.path.join(folder, "clock.jsonl")).read().split("\n")
    assert lines[-1] == "" or len(lines) > 1, "the clock holds nothing whole"
    read = []
    for number, line in enumerate(line for line in lines if line):
        try:
            read.append(json.loads(line))
        except ValueError:
            assert number == len([line for line in lines if line]) - 1, f"a line of the clock that is not the last does not read: {line}"
    return read


def when(points, frame, rate):
    """The instant of a frame: counted from the last point before it, at the track's named rate."""
    sample, at = max((p for p in points if p[0] <= frame), default=points[0])
    return at + (frame - sample) / rate


def placed(folder, one, stops=False):
    """A track's points and silences, judged: unless the scenario stops a source, the clock
    must hold the track's rate from its first point to its last, with no silence of ours."""
    lines = [line for line in clock(folder) if line["track"] == one["name"]]
    points = [(line["sample"], line["at"]) for line in lines if "sample" in line]
    silences = [line["silence"] for line in lines if "silence" in line]
    name, rate = one["name"], one["rate"]
    assert points, f"{name}: no point on the clock"
    assert points[0][0] == 0 or (silences and silences[0][0] == 0), f"{name}: nothing says where its first frame is"
    # A point at least every ten seconds while the source delivers: not across zeros of ours.
    ends = [*points[1:], (one["frames"], points[-1][1] + (one["frames"] - points[-1][0]) / rate)]
    gaps = [b[1] - a[1] for a, b in zip(points, ends) if not any(a[0] <= zeros[0] < b[0] for zeros in silences)]
    assert all(gap <= 10 for gap in gaps), f"{name}: {max(gaps):.1f} s of the track without a point"
    assert all(0 <= a < b <= one["frames"] + rate for a, b in silences), f"{name}: a silence that is no stretch of the track: {silences}"
    # Between two points, the frames of the track's rate: off by more, the source stopped there.
    settled = [point for point in points if point[0] >= SETTLED * rate]
    off = [(a, b) for a, b in zip(settled, settled[1:]) if abs((b[0] - a[0]) / rate - (b[1] - a[1])) > 0.005]
    if not stops:
        assert not silences, f"{name}: {len(silences)} silence(s) of ours where the source never stopped"
        assert not off, f"{name}: its clock says {(off[0][1][0] - off[0][0][0]) / (off[0][1][1] - off[0][0][1]):.0f} frames a second"
        # And from its first settled point to its last, the same rate: a hundredth of a second
        # over the whole run, where two points apart may each be five thousandths off.
        if len(settled) > 1:
            drift = (settled[-1][0] - settled[0][0]) / rate - (settled[-1][1] - settled[0][1])
            assert abs(drift) <= 0.01, f"{name}: over the run its clock is {drift * 1000:.0f} ms from its frames"
    return points, silences, off


def stream(pid, name):
    """The recorder's stream for a track, as the sound server numbers it."""
    dump = json.loads(subprocess.run(["pw-dump"], capture_output=True, text=True, check=True).stdout)
    props = lambda thing: thing["info"]["props"]
    clients = [thing["id"] for thing in dump if thing["type"].endswith("Client") and str(props(thing).get("pipewire.sec.pid")) == str(pid)]
    for thing in dump:
        if thing["type"].endswith("Node") and props(thing).get("client.id") in clients:
            if ("remote" if props(thing).get("stream.capture.sink") else "local") == name:
                return thing["id"]
    raise AssertionError(f"the recorder has no stream for {name}")


def playing(pid):
    """A process's playing stream: its client names the process, the node does not always."""
    dump = json.loads(subprocess.run(["pw-dump"], capture_output=True, text=True, check=True).stdout)
    props = lambda thing: thing["info"]["props"]
    clients = [thing["id"] for thing in dump if thing["type"].endswith("Client") and str(props(thing).get("pipewire.sec.pid")) == str(pid)]
    for thing in dump:
        if thing["type"].endswith("Node") and props(thing).get("client.id") in clients and props(thing).get("media.class") == "Stream/Output/Audio":
            return thing["id"]
    raise AssertionError(f"process {pid} has no playing stream")


def graph_nodes():
    """The nodes of the graph, each with its client's properties beneath its own."""
    dump = json.loads(subprocess.run(["pw-dump"], capture_output=True, text=True, check=True).stdout)
    clients = {o["id"]: o["info"].get("props", {}) for o in dump if o["type"].endswith(":Client")}
    nodes = []
    for o in dump:
        if o["type"].endswith(":Node"):
            props = o["info"].get("props", {})
            nodes.append((o["info"].get("state"), {**clients.get(props.get("client.id"), {}), **props}))
    return nodes


def own_microphone(pid):
    return [props for _, props in graph_nodes() if props.get("media.class") == "Stream/Input/Audio"
            and str(props.get("application.process.id")) == str(pid) and props.get("stream.capture.sink") not in (True, "true")]


def passive(pid):
    """capture's local stream says the value this PipeWire reads as passive, its remote stream none (ADR-0015)."""
    dump = json.loads(subprocess.run(["pw-dump"], capture_output=True, text=True, check=True).stdout)
    version = next(o["info"]["version"] for o in dump if o["type"].endswith(":Core"))
    wanted = "in-follow" if tuple(map(int, version.split(".")[:2])) >= (1, 7) else "in"
    local = own_microphone(pid)
    assert local, f"process {pid} has no local stream"
    assert local[0].get("node.passive") == wanted, f"PipeWire {version}: the local stream says node.passive={local[0].get('node.passive')}"
    remote = [props for _, props in graph_nodes() if str(props.get("application.process.id")) == str(pid) and props.get("stream.capture.sink") in (True, "true")]
    assert remote and "node.passive" not in remote[0], "the remote stream changed"
    print(f"    PipeWire {version}: the local stream says node.passive={wanted}; the remote does not")


def state(name):
    states = [state for state, props in graph_nodes() if props.get("node.name") == name]
    assert states, f"{name} is not in the graph"
    return states[0]


def not_running(name, seconds):
    """The source is never `running`, read every tenth of a second for that long."""
    until = time.time() + seconds
    while time.time() < until:
        assert state(name) != "running", f"{name} runs with nothing holding it"
        time.sleep(0.1)
    print(f"    {name}: not running, read for {seconds} s")


def running(name):
    assert state(name) == "running", f"{name} is {state(name)}, with a call holding it"
    print(f"    {name}: running")


def waited(folder):
    """The track of a microphone that waited: zeros said from its first frame, the note after."""
    read, one = session(folder), track(folder, "local")
    assert "local" in read["tracks"], "session.json holds no local track"
    assert read["tracks"]["local"]["device"] == "wholeheard_e2e_headset", f"local recorded from {read['tracks']['local']['device']}"
    assert read["tracks"]["local"]["changed"][-1]["device"] is None, "the call left, and no change says so"
    lines = [line for line in clock(folder) if line["track"] == "local"]
    silences = [line["silence"] for line in lines if "silence" in line]
    points = [line["sample"] for line in lines if "sample" in line]
    assert silences and silences[0][0] == 0, "the track opens on zeros no line of the clock names"
    assert points and points[0] >= silences[0][1], "a point inside the zeros"
    assert abs(pitch(one) - 660) < 40, f"local holds {pitch(one):.0f} Hz, not the call's 660"
    print(f"    local opens on {silences[0][1] / one['rate']:.1f} s of zeros said, then the call's note")


def notes(one):
    """The note each whole second of a track holds, in Hz — 0 for a second of silence."""
    rate, first, found = one["rate"], one["first"], []
    for second in range(one["frames"] // rate):
        piece = first[second * rate:(second + 1) * rate]
        rest = sum(piece) / rate
        if max(abs(value - rest) for value in piece) < 30_000:
            found.append(0)
            continue
        found.append(sum(a - rest < 0 <= b - rest for a, b in zip(piece, piece[1:])))
    return found


def moves_said(events, local, remote):
    """The `device` events of a recording, against the devices it moved to, in order: each named
    exactly, never by a name it had before."""
    said = [json.loads(line) for line in open(events) if line.strip()]
    for source, wanted in (("me", local), ("others", remote)):
        devices = [event["device"] for event in said if event["event"] == "device" and event["source"] == source]
        moves = [move.split(":")[0] for move in wanted.split(",")]
        assert [device.split(" ")[0] if device else device for device in devices] == moves, f"{source}: its device events say {devices}, for {moves}"
    print(f"    the device events, in order: {[(event['source'], event['device']) for event in said if event['event'] == 'device']}")


def followed(folder, events, local, remote, during):
    """A recording whose devices were switched while it ran (D31): each track goes on in the form
    it opened with, holds each device's note in turn with no zeros of ours, and the events and
    the session name each device it moved to, in order. `local` and `remote`: the devices it
    moved to, comma-separated, and each one's note after a colon."""
    read = session(folder)
    said = [json.loads(line) for line in open(events) if line.strip()]
    kinds = [event["event"] for event in said if event["event"] != "level"]
    assert kinds[0] == "started" and kinds[-1] == "stopped" and kinds.count("started") == 1, f"the events in this order: {kinds}"
    for name, source, wanted in (("local", "me", local), ("remote", "others", remote)):
        moves = [move.split(":") for move in wanted.split(",")]
        one = track(folder, name)  # its form is the one it opened with, or this fails
        lines = [line for line in clock(folder) if line["track"] == name]
        silences = [line["silence"] for line in lines if "silence" in line]
        assert not silences, f"{name}: zeros of ours at {silences}: a switch of device lost its sound"
        devices = [event["device"] for event in said if event["event"] == "device" and event["source"] == source]
        changed = [change["device"] for change in read["tracks"][name].get("changed", [])]
        print(f"    {name}: {one['channels']} channel(s) at {one['rate']} Hz; moved to {devices}; the session keeps {changed}")
        assert [device.split(" ")[0] if device else device for device in devices] == [move for move, _ in moves], (
            f"{name}: its device events say {devices}, for {[move for move, _ in moves]}")
        assert changed == devices, f"{name}: the session keeps {changed}, where the events said {devices}"
        heard = notes(one)
        print(f"    {name}: its notes, second by second: {heard}")
        for move, note in moves:
            assert any(abs(hertz - int(note)) < 15 for hertz in heard), f"{name}: {move}'s note, {note} Hz, is not in its track"
        # Each change where it was made: the first after the run's first notes, about five seconds
        # in, the next after it.
        ats = [change["at"] for change in read["tracks"][name].get("changed", [])]
        assert ats == sorted(ats) and 3 < ats[0] < 9, f"{name}: changes put at {ats} s on the session's clock"
    # Kept while it records, not at the stop alone.
    kept = json.load(open(during))["tracks"]
    for name, wanted in (("local", local), ("remote", remote)):
        first = wanted.split(",")[0].split(":")[0]
        changed = [change["device"].split(" ")[0] for change in kept[name].get("changed", [])]
        assert changed[:1] == [first], f"{name}: while it recorded, session.json kept {changed}, not {first}"


def call_followed(folder, events, call, default, heard, left, after):
    """A call heard on an output other than the default (D41): the remote track moved to the
    call's output and holds its note `heard`, then back on the default once the call left it,
    holding `after` and never `left`, played on the call's output after it was left. The local
    track does not move; the events and the session name each move, in order."""
    read = session(folder)
    said = [json.loads(line) for line in open(events) if line.strip()]
    for source, wanted in (("me", []), ("others", [call, default])):
        devices = [event["device"] for event in said if event["event"] == "device" and event["source"] == source]
        assert [device.split(" ")[0] if device else device for device in devices] == wanted, f"{source}: its device events say {devices}, for {wanted}"
        changed = [change["device"] for change in read["tracks"]["local" if source == "me" else "remote"].get("changed", [])]
        assert changed == devices, f"{source}: the session keeps {changed}, where the events said {devices}"
    heard_notes = notes(track(folder, "remote"))
    print(f"    remote: moved to {[call, default]}; its notes, second by second: {heard_notes}")
    near = lambda note: any(abs(hertz - int(note)) < 15 for hertz in heard_notes)
    assert near(heard), f"remote: the call's note, {heard} Hz, is not in its track"
    assert near(after), f"remote: the default's note after the call left, {after} Hz, is not in its track"
    assert not near(left), f"remote: {left} Hz, played on the output the call had left, is in its track"


def main(what, folder, *args):
    if what == "pause":
        # Not a measure: stops the recorder's own stream for a track, then starts it again.
        node, seconds = stream(int(folder), args[0]), float(args[1])
        subprocess.run(["pw-cli", "send-command", str(node), "Pause", "{}"], capture_output=True, check=True)
        time.sleep(seconds)
        subprocess.run(["pw-cli", "send-command", str(node), "Start", "{}"], capture_output=True, check=True)
        return
    if what == "followed":
        return followed(folder, *args)
    if what == "moves-said":
        return moves_said(folder, *args)
    if what == "passive":
        return passive(folder)
    if what == "not-running":
        return not_running(folder, float(args[0]))
    if what == "running":
        return running(folder)
    if what == "not-following":
        assert not own_microphone(folder), "capture still has a local stream, after the call left"
        return print("    capture holds no local stream")
    if what == "waited":
        return waited(folder)
    if what == "playing":
        # Not a measure: the stand-in call's playing stream, as the sound server numbers it.
        return print(playing(int(folder)))
    if what == "call-followed":
        return call_followed(folder, *args)
    read = session(folder)
    tracks = {name: track(folder, name) for name in read["tracks"]}
    start = datetime.datetime.fromisoformat(read["started"]).timestamp()
    clocks = {}
    for one in tracks.values():
        points, silences, off = clocks[one["name"]] = placed(folder, one, stops=what == "stopped-source")
        loudest = max(one["peaks"]) / FULL
        print(f"    {one['name']}: {one['frames'] / one['rate']:.2f} s, {one['channels']} channel(s) at {one['rate']} Hz, 24 bits, "
              f"{pitch(one):.0f} Hz at {loudest:.4f} of full scale · first heard at {points[0][1]:.3f} s, {len(points)} points, "
              f"{len(silences)} silence(s) of ours, {len(off)} place(s) the source stopped")
        # As delivered: every channel as loud as it was played and resting where it was played
        # — not mixed, swapped, halved, centred or brought to a level.
        levels, rests = [peak / FULL for peak in one["peaks"]], [rest / FULL for rest in one["rests"]]
        assert all(abs(level / played - 1) < 0.01 for level, played in zip(levels, LEVELS)), (
            f"{one['name']}: its channels are at {[round(level, 4) for level in levels]} of full scale, played at {LEVELS}")
        assert all(abs(rest / REST - 1) < 0.02 for rest in rests), (
            f"{one['name']}: its channels rest at {[round(rest, 4) for rest in rests]} of full scale while they sound, played at {REST}")
        # Zeros of ours are every channel's, and the clock is the session's: a track starts
        # with the session and, once stopped, ends with it.
        size = 3 * one["channels"]
        for a, b in silences:
            assert not any(one["body"][a * size:b * size]), f"{one['name']}: the frames {a} to {b} are said zeros of ours, and hold sound"
        assert any(one["body"][0::3]), f"{one['name']}: the last eight bits of its samples are never used"
        assert -0.5 <= points[0][1] - points[0][0] / one["rate"] <= 2.5, f"{one['name']}: its first frame is put {points[0][1]:.2f} s into the session"
        if "stopped" in read:
            # `started` and `stopped` are written to the second: a last frame is heard before
            # the stop, so its instant is no more than a second past what the two say.
            late = when(points, one["frames"], one["rate"]) - (datetime.datetime.fromisoformat(read["stopped"]).timestamp() - start)
            assert -3.5 <= late <= 1.05, f"{one['name']}: its clock puts its last frame {late:.2f} s after the session stopped"
    # A note played without a break is whole in its track, but where the clock says otherwise.
    for name, hertz in {"stopped": [("local", 440)], "killed": [("local", 440)], "stopped-source": [("local", 440), ("remote", 880)]}.get(what, []):
        points, silences, off = clocks[name]
        broken = breaks(tracks[name], hertz, [*silences, *((b[0] - 1, b[0]) for _, b in off)])
        assert not broken, (f"{name}: its note breaks between the frames {broken[0][0]} and {broken[0][1]} — {len(broken)} place(s) — "
                            "and no line of the clock tells of it")
    if what == "stopped":
        assert "stopped" in read, "session.json carries no stopped"
        lasted = datetime.datetime.fromisoformat(read["stopped"]).timestamp() - start
        assert set(tracks) == {"local", "remote"}, f"the session holds {sorted(tracks)}"
        for one, note in zip((tracks["local"], tracks["remote"]), (float(args[0]), float(args[1]))):
            assert one["said"] == one["bytes"] and one["riff"] == one["file"] - 8, f"{one['name']}: its header does not carry its true length"
            assert abs(pitch(one) - note) < 20, f"{one['name']} holds a note at {pitch(one):.0f} Hz, not {note:.0f}"
            assert abs(one["frames"] / one["rate"] - lasted) <= 2.5, f"{one['name']} holds {one['frames'] / one['rate']:.1f} s for {lasted:.0f} s recorded"
        assert not os.path.exists(os.path.join(folder, "me.wav")), "a file of the first version beside the tracks"
    elif what == "stopped-source":
        # The remote source was stopped for several seconds: the capture wrote zeros and said which.
        points, silences, _ = clocks["remote"]
        remote, rate = tracks["remote"], tracks["remote"]["rate"]
        assert silences, "remote stopped for seconds, and the clock says no silence of ours"
        zeros = sum(to - start for start, to in silences)
        assert zeros >= 2 * rate, f"{zeros / rate:.1f} s of zeros said for a stop of several seconds"
        after = [point for point in points if point[0] >= silences[-1][1]]
        assert after and after[0][0] == silences[-1][1], "the first sound after the zeros has no point of its own"
        assert not clocks["local"][1] and not clocks["local"][2], "local never stopped, and its clock says it did"
        print(f"    remote stopped: {len(silences)} silence(s) of ours, {zeros / rate:.2f} s of zeros, a point at the first frame after")
    elif what == "same-instant":
        allowed, heard = float(args[0]), {}
        for one in tracks.values():
            assert onset(one) is not None, f"the sound is not in {one['name']}"
            heard[one["name"]] = when(clocks[one["name"]][0], onset(one), one["rate"])
        apart = abs(heard["local"] - heard["remote"])
        print(f"    the same sound is heard at {heard['local']:.4f} s by local's clock and {heard['remote']:.4f} s by remote's: {apart * 1000:.1f} ms apart")
        assert apart <= allowed, f"the clock places the two tracks {apart * 1000:.0f} ms apart, for {allowed * 1000:.0f} allowed"
    elif what == "killed":
        killed_at, allowed = float(args[0]), float(args[1])
        assert "stopped" not in read, "a killed recording says it was stopped"
        for one, note in zip((tracks["local"], tracks["remote"]), (440, 880)):
            lost = killed_at - start - when(clocks[one["name"]][0], one["frames"], one["rate"])
            print(f"    {one['name']}: {lost:.1f} s lost, the second of started included")
            assert one["said"] == 0xFFFFFFFF, f"{one['name']}: never finished, and it carries a length"
            assert lost <= allowed + 1, f"{one['name']}: {lost:.1f} s lost"
            assert lost >= -0.25, f"{one['name']}: its clock puts its last frame {-lost:.2f} s after the recording was killed"
            assert abs(pitch(one) - note) < 40, f"{one['name']}: {pitch(one):.0f} Hz"
    else:
        raise SystemExit(f"unknown measure: {what}")


if __name__ == "__main__":
    try:
        main(*sys.argv[1:])
    except AssertionError as failure:
        print(f"    {failure}")
        sys.exit(1)
