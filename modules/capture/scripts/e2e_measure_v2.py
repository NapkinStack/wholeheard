"""What the end-to-end scenarios measure in a session of `session-audio/v2` — never listened
to, and the sound is a note made for the run."""
import datetime
import json
import os
import struct
import sys


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
    # The first channel is enough to find a note and where it starts.
    first = [int.from_bytes(body[n * 3 * channels:n * 3 * channels + 3], "little", signed=True) for n in range(frames)]
    return {"name": name, "rate": rate, "channels": channels, "frames": frames, "first": first,
            "said": struct.unpack("<I", data[at + 4:at + 8])[0], "bytes": len(body)}


def pitch(one):
    """The note the track holds, in Hz: its zero crossings while it sounds."""
    first = one["first"]
    loud = [at for at, value in enumerate(first) if abs(value) > 30_000]
    if len(loud) < one["rate"]:
        return 0
    crossings = sounding = 0
    for at in range(loud[0], loud[-1]):
        if abs(first[at]) > 3_000 or abs(first[at + 1]) > 3_000:
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


def placed(folder, one):
    lines = [line for line in clock(folder) if line["track"] == one["name"]]
    points = [(line["sample"], line["at"]) for line in lines if "sample" in line]
    silences = [line["silence"] for line in lines if "silence" in line]
    assert points, f"{one['name']}: no point on the clock"
    assert points[0][0] == 0 or (silences and silences[0][0] == 0), f"{one['name']}: nothing says where its first frame is"
    gaps = [b[1] - a[1] for a, b in zip(points, points[1:])]
    assert all(gap <= 10 for gap in gaps), f"{one['name']}: {max(gaps):.1f} s between two points"
    zeros = sum(to - start for start, to in silences)
    measured = (points[-1][0] - points[0][0] - zeros) / (points[-1][1] - points[0][1]) if len(points) > 1 and not silences else None
    return points, silences, measured


def main(what, folder, *args):
    read = session(folder)
    tracks = {name: track(folder, name) for name in read["tracks"]}
    start = datetime.datetime.fromisoformat(read["started"]).timestamp()
    for one in tracks.values():
        points, silences, measured = placed(folder, one)
        rate = "" if measured is None else f", {measured:.1f} frames a second by its clock"
        print(f"    {one['name']}: {one['frames'] / one['rate']:.2f} s, {one['channels']} channel(s) at {one['rate']} Hz, 24 bits, "
              f"{pitch(one):.0f} Hz · first heard at {points[0][1]:.3f} s, {len(points)} points, {len(silences)} silence(s) of ours{rate}")
        assert measured is None or abs(measured / one["rate"] - 1) < 0.005, f"{one['name']}: its clock says {measured:.0f} frames a second"
    if what == "stopped":
        lasted = datetime.datetime.fromisoformat(read["stopped"]).timestamp() - start
        assert set(tracks) == {"local", "remote"}, f"the session holds {sorted(tracks)}"
        for one, note in zip((tracks["local"], tracks["remote"]), (float(args[0]), float(args[1]))):
            assert one["said"] == one["bytes"], f"{one['name']}: its header does not carry its true length"
            assert abs(pitch(one) - note) < 20, f"{one['name']} holds a note at {pitch(one):.0f} Hz, not {note:.0f}"
            assert abs(one["frames"] / one["rate"] - lasted) <= 2.5, f"{one['name']} holds {one['frames'] / one['rate']:.1f} s for {lasted:.0f} s recorded"
        assert not os.path.exists(os.path.join(folder, "me.wav")), "a file of the first version beside the tracks"
    elif what == "same-instant":
        allowed, heard = float(args[0]), {}
        for one in tracks.values():
            assert onset(one) is not None, f"the sound is not in {one['name']}"
            heard[one["name"]] = when(placed(folder, one)[0], onset(one), one["rate"])
        apart = abs(heard["local"] - heard["remote"])
        print(f"    the same sound is heard at {heard['local']:.4f} s by local's clock and {heard['remote']:.4f} s by remote's: {apart * 1000:.1f} ms apart")
        assert apart <= allowed, f"the clock places the two tracks {apart * 1000:.0f} ms apart, for {allowed * 1000:.0f} allowed"
    elif what == "killed":
        killed_at, allowed = float(args[0]), float(args[1])
        assert "stopped" not in read, "a killed recording says it was stopped"
        for one, note in zip((tracks["local"], tracks["remote"]), (440, 880)):
            lost = killed_at - start - when(placed(folder, one)[0], one["frames"], one["rate"])
            print(f"    {one['name']}: {lost:.1f} s lost, the second of started included")
            assert one["said"] == 0xFFFFFFFF, f"{one['name']}: never finished, and it carries a length"
            assert lost <= allowed + 1, f"{one['name']}: {lost:.1f} s lost"
            assert abs(pitch(one) - note) < 40, f"{one['name']}: {pitch(one):.0f} Hz"
    else:
        raise SystemExit(f"unknown measure: {what}")


if __name__ == "__main__":
    try:
        main(*sys.argv[1:])
    except AssertionError as failure:
        print(f"    {failure}")
        sys.exit(1)
