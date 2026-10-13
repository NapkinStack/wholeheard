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
            "said": struct.unpack("<I", data[at + 4:at + 8])[0], "bytes": len(body), "body": body}


def nothing(one):
    """The stretches of the track, a hundredth of a second or more, where every channel is zero."""
    body, size, found, at = one["body"], 3 * one["channels"], [], 0
    least = one["rate"] // 100
    while (at := body.find(bytes(size * least), at)) >= 0:
        start = -(-at // size)
        end = start
        while body[end * size:(end + 4096) * size] == bytes(size * 4096):
            end += 4096
        while end < one["frames"] and not any(body[end * size:(end + 1) * size]):
            end += 1
        if end - start >= least:
            found.append((start, end))
        at = max(end, start + 1) * size
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
        if one["first"][at] and one["first"][at + 1]:
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
    off = [(a, b) for a, b in zip(points, points[1:]) if abs((b[0] - a[0]) / rate - (b[1] - a[1])) > 0.005]
    # The system may hand a stream's first piece over late, with the second on its heels: the
    # recorder says so with a point at the second, and that is no stop — when the piece is one
    # period at most, and the clock holds from there. Anything else that opens a track is judged.
    if off and off[0][0] == points[0] and off[0][1][0] <= 0.1 * rate and abs(off[0][1][1] - off[0][0][1]) <= 0.1:
        off = off[1:]
    if not stops:
        assert not silences, f"{name}: {len(silences)} silence(s) of ours where the source never stopped"
        assert not off, f"{name}: its clock says {(off[0][1][0] - off[0][0][0]) / (off[0][1][1] - off[0][0][1]):.0f} frames a second"
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


def main(what, folder, *args):
    if what == "pause":
        # Not a measure: stops the recorder's own stream for a track, then starts it again.
        node, seconds = stream(int(folder), args[0]), float(args[1])
        subprocess.run(["pw-cli", "send-command", str(node), "Pause", "{}"], capture_output=True, check=True)
        time.sleep(seconds)
        subprocess.run(["pw-cli", "send-command", str(node), "Start", "{}"], capture_output=True, check=True)
        return
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
        assert all(abs(level / played - 1) < 0.03 for level, played in zip(levels, LEVELS)), (
            f"{one['name']}: its channels are at {[round(level, 4) for level in levels]} of full scale, played at {LEVELS}")
        assert all(abs(rest / REST - 1) < 0.1 for rest in rests), (
            f"{one['name']}: its channels rest at {[round(rest, 4) for rest in rests]} of full scale while they sound, played at {REST}")
        # Zeros of ours are every channel's, and the clock is the session's: a track starts
        # with the session and, once stopped, ends with it.
        size = 3 * one["channels"]
        for a, b in silences:
            assert not any(one["body"][a * size:b * size]), f"{one['name']}: the frames {a} to {b} are said zeros of ours, and hold sound"
        assert -0.5 <= points[0][1] - points[0][0] / one["rate"] <= 2.5, f"{one['name']}: its first frame is put {points[0][1]:.2f} s into the session"
        if "stopped" in read:
            late = when(points, one["frames"], one["rate"]) - (datetime.datetime.fromisoformat(read["stopped"]).timestamp() - start)
            assert -2.5 <= late <= 0.25, f"{one['name']}: its clock puts its last frame {late:.2f} s after the session stopped"
    # A source that played without a break delivered no zeros: every stretch of them is ours,
    # and the clock names it. (The first tenth of a second apart: a stream may open on a piece of nothing.)
    for name in {"stopped": ["local"], "killed": ["local"], "stopped-source": ["local", "remote"]}.get(what, []):
        one, named = tracks[name], []
        for a, b in sorted(clocks[name][1]):   # silences that touch are one stretch
            named[-1:] = [[named[-1][0], max(b, named[-1][1])]] if named and a <= named[-1][1] else [*named[-1:], [a, b]]
        unsaid = [(a, b) for a, b in nothing(one) if b > one["rate"] // 10 and not any(s <= a and b <= e for s, e in named)]
        assert not unsaid, f"{name}: the frames {unsaid[0][0]} to {unsaid[0][1]} are zeros, the source played, and no line of the clock names them"
    if what == "stopped":
        assert "stopped" in read, "session.json carries no stopped"
        lasted = datetime.datetime.fromisoformat(read["stopped"]).timestamp() - start
        assert set(tracks) == {"local", "remote"}, f"the session holds {sorted(tracks)}"
        for one, note in zip((tracks["local"], tracks["remote"]), (float(args[0]), float(args[1]))):
            assert one["said"] == one["bytes"], f"{one['name']}: its header does not carry its true length"
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
        # The note is played without a break: after the stop it is where the clock says, to the period.
        ticks = [when(points, frame, rate) for frame in range(after[0][0], after[0][0] + rate) if remote["first"][frame] < 0 <= remote["first"][frame + 1]]
        assert len(ticks) > 800, "no note after the stop"
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
