"""What the end-to-end scenarios measure in a session's folder — never listened to, and the
sound is a note made for the run."""
import datetime
import json
import os
import struct
import sys


def samples(path):
    data = open(path, "rb").read()
    assert data[:4] == b"RIFF" and data[8:16] == b"WAVEfmt ", f"{path}: not an audio file of a session"
    channels, rate, bits = struct.unpack("<H", data[22:24])[0], struct.unpack("<I", data[24:28])[0], struct.unpack("<H", data[34:36])[0]
    assert (channels, rate, bits) == (1, 16000, 16), f"{path}: {channels} channel(s), {rate} Hz, {bits} bits"
    body = data[44:]
    return struct.unpack(f"<{len(body) // 2}h", body[: len(body) // 2 * 2]), struct.unpack("<I", data[40:44])[0]


def pitch(values):
    """The note the sound holds, in Hz: its zero crossings between its first and last loud sample."""
    loud = [at for at, value in enumerate(values) if abs(value) > 400]
    if len(loud) < 16000:
        return 0
    crossings, sounding = 0, 0
    for at in range(loud[0], loud[-1]):
        if abs(values[at]) > 100 or abs(values[at + 1]) > 100:
            sounding += 1
            crossings += values[at] < 0 <= values[at + 1]
    return crossings * 16000 / max(sounding, 1)


def started(folder_or_file):
    path = folder_or_file if folder_or_file.endswith(".json") else os.path.join(folder_or_file, "session.json")
    session = json.load(open(path))
    assert session["contract"] == "session-audio/v1", session
    return datetime.datetime.fromisoformat(session["started"]).timestamp(), session


def onset(values):
    """When the sound starts, in seconds."""
    return next((at for at, value in enumerate(values) if abs(value) > 400), len(values)) / 16000


def longest_silence(values):
    longest = run = 0
    for value in values:
        run = run + 1 if value == 0 else 0
        longest = max(longest, run)
    return longest / 16000


def main(what, *args):
    if what == "stopped":
        folder, me_note, others_note = args[0], float(args[1]), float(args[2])
        me, me_length = samples(os.path.join(folder, "me.wav"))
        others, others_length = samples(os.path.join(folder, "others.wav"))
        start, session = started(folder)
        assert "stopped" in session, "session.json carries no stopped"
        # "started" is written to the second: a second of what follows is its doing.
        lasted = datetime.datetime.fromisoformat(session["stopped"]).timestamp() - start
        print(f"    me {len(me) / 16000:.2f} s at {pitch(me):.0f} Hz · others {len(others) / 16000:.2f} s at {pitch(others):.0f} Hz, "
              f"its longest silence {longest_silence(others):.1f} s · {lasted:.0f} s between started and stopped")
        assert len(me) == len(others), f"me {len(me)} samples, others {len(others)}"
        assert me_length == len(me) * 2 == others_length, "the headers do not carry the true lengths"
        assert abs(len(me) / 16000 - lasted) <= 2, f"the files hold {len(me) / 16000:.1f} s for {lasted:.0f} s recorded"
        assert abs(pitch(me) - me_note) < 20, f"me holds a note at {pitch(me):.0f} Hz, not {me_note:.0f}"
        assert abs(pitch(others) - others_note) < 40, f"others holds a note at {pitch(others):.0f} Hz, not {others_note:.0f}"
        assert longest_silence(others) >= 10, f"others' longest silence is {longest_silence(others):.1f} s where fourteen were played"
    elif what == "events":
        said = [json.loads(line) for line in open(args[0])]
        # The meter's levels, ten times a second each: read apart from the events they would drown.
        levels = [event for event in said if event["event"] == "level"]
        said = [event for event in said if event["event"] != "level"]
        loudest = {source: max((e["level"] for e in levels if e["source"] == source), default=None) for source in ("me", "others")}
        print(f"    {len(levels)} levels; the loudest: me {loudest['me']}, others {loudest['others']}")
        assert all(0 <= e["level"] <= 1 for e in levels), "a level out of 0 to 1"
        assert None not in loudest.values(), "a source said no level"
        assert loudest["others"] >= 0.3, "the others' level never rose while they played"
        names = [(event["event"], event.get("source")) for event in said]
        print(f"    {' · '.join(name if source is None else f'{name} {source}' for name, source in names)}")
        assert names[0][0] == "started" and names[-1][0] == "stopped", "the program did not say started first and stopped last"
        assert ("silent", "others") in names and ("heard", "others") in names, "silent then heard were not said of others"
        assert names.index(("silent", "others")) < names.index(("heard", "others")), "heard was said before silent"
    elif what == "same-clock":
        folder, allowed = args[0], float(args[1])
        me, _ = samples(os.path.join(folder, "me.wav"))
        others, _ = samples(os.path.join(folder, "others.wav"))
        apart = abs(onset(me) - onset(others))
        print(f"    the same sound starts at {onset(me):.3f} s in me and {onset(others):.3f} s in others: {apart * 1000:.0f} ms apart")
        assert onset(me) < len(me) / 16000 - 1, "the sound is in neither file"
        assert apart <= allowed, f"the two files are {apart:.2f} s apart, for {allowed} allowed"
    elif what == "read-while-written":
        copy, session, copied_at, allowed = args[0], args[1], float(args[2]), float(args[3])
        values, _ = samples(copy)
        behind = copied_at - started(session)[0] - len(values) / 16000
        print(f"    the copy holds {len(values) / 16000:.2f} s and is {behind:.1f} s behind, the second of started included")
        assert behind <= allowed + 1, f"the copy is {behind:.1f} s behind"
    elif what == "killed":
        folder, killed_at, allowed = args[0], float(args[1]), float(args[2])
        me, me_length = samples(os.path.join(folder, "me.wav"))
        others, _ = samples(os.path.join(folder, "others.wav"))
        start, session = started(folder)
        lost = killed_at - start - min(len(me), len(others)) / 16000
        print(f"    me {len(me) / 16000:.2f} s, others {len(others) / 16000:.2f} s · {lost:.1f} s lost, the second of started included")
        assert "stopped" not in session, "a killed recording says it was stopped"
        assert me_length == 0xFFFFFFFF, "a file never finished carries a length"
        assert lost <= allowed + 1, f"{lost:.1f} s lost"
        assert abs(len(me) - len(others)) <= 1600, f"me {len(me)} samples, others {len(others)}: more than a tenth of a second apart"
        assert abs(pitch(me) - 440) < 20 and abs(pitch(others) - 880) < 40, f"{pitch(me):.0f} Hz, {pitch(others):.0f} Hz"
    else:
        raise SystemExit(f"unknown measure: {what}")


if __name__ == "__main__":
    try:
        main(*sys.argv[1:])
    except AssertionError as failure:
        print(f"    {failure}")
        sys.exit(1)
