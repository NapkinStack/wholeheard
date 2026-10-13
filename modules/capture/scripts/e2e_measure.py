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


def main(what, *args):
    if what == "stopped":
        folder, me_note, others_note = args[0], float(args[1]), float(args[2])
        me, me_length = samples(os.path.join(folder, "me.wav"))
        others, others_length = samples(os.path.join(folder, "others.wav"))
        start, session = started(folder)
        lasted = datetime.datetime.fromisoformat(session["stopped"]).timestamp() - start
        assert len(me) == len(others), f"me {len(me)} samples, others {len(others)}"
        assert me_length == len(me) * 2 == others_length, "the headers do not carry the true lengths"
        assert abs(len(me) / 16000 - lasted) <= 2, f"the files hold {len(me) / 16000:.1f} s for {lasted:.0f} s recorded"
        assert abs(pitch(me) - me_note) < 20, f"me holds a note at {pitch(me):.0f} Hz, not {me_note:.0f}"
        assert abs(pitch(others) - others_note) < 40, f"others holds a note at {pitch(others):.0f} Hz, not {others_note:.0f}"
        quiet = sum(1 for v in others if v == 0) / 16000
        assert quiet >= 10, f"others holds {quiet:.1f} s of silence where fourteen were played"
    elif what == "read-while-written":
        copy, session, copied_at, allowed = args[0], args[1], float(args[2]), float(args[3])
        values, _ = samples(copy)
        behind = copied_at - started(session)[0] - len(values) / 16000
        assert behind <= allowed + 1, f"the copy is {behind:.1f} s behind"
    elif what == "killed":
        folder, killed_at, allowed = args[0], float(args[1]), float(args[2])
        me, me_length = samples(os.path.join(folder, "me.wav"))
        others, _ = samples(os.path.join(folder, "others.wav"))
        start, session = started(folder)
        assert "stopped" not in session, "a killed recording says it was stopped"
        assert me_length == 0xFFFFFFFF, "a file never finished carries a length"
        lost = killed_at - start - min(len(me), len(others)) / 16000
        assert lost <= allowed + 1, f"{lost:.1f} s lost"
        assert abs(len(me) - len(others)) <= 16000, f"me {len(me)}, others {len(others)}"
        assert abs(pitch(me) - 440) < 20 and abs(pitch(others) - 880) < 40, f"{pitch(me):.0f} Hz, {pitch(others):.0f} Hz"
    else:
        raise SystemExit(f"unknown measure: {what}")


if __name__ == "__main__":
    try:
        main(*sys.argv[1:])
    except AssertionError as failure:
        print(f"    {failure}")
        sys.exit(1)
