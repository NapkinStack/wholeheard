"""A session as capture writes it on a Mac (session-audio/v2), from a sound `say` spoke: the
remote track that sound in the Mac's channels, 24 bits at 48 kHz, scaled by a gain; the local
track as long, all zeros; its session.json and clock.jsonl. For the macOS workflow (#426).

Usage: session.py <spoken.wav> <folder> <gain>
"""
import json
import os
import struct
import sys

spoken, folder, gain = sys.argv[1], sys.argv[2], float(sys.argv[3])
raw = open(spoken, "rb").read()
at, fmt, data = 12, None, None
while at < len(raw) - 8:
    kind, size = raw[at:at + 4], struct.unpack("<I", raw[at + 4:at + 8])[0]
    if kind == b"fmt ":
        fmt = raw[at + 8:at + 8 + size]
    if kind == b"data":
        data = raw[at + 8:at + 8 + size]
    at += 8 + size + (size & 1)
channels, bits = struct.unpack("<H", fmt[2:4])[0], struct.unpack("<H", fmt[14:16])[0]
assert bits == 24, "afconvert was asked for 24 bits"
scaled = bytearray()
for k in range(0, len(data) - len(data) % 3, 3):
    value = round(int.from_bytes(data[k:k + 3], "little", signed=True) * gain)
    scaled += max(-8388608, min(8388607, value)).to_bytes(3, "little", signed=True)
frames = len(scaled) // (3 * channels)


def header(channels: int, frames: int) -> bytes:
    """The extensible form a 24-bit track of session-audio/v2 is written in."""
    size = frames * channels * 3
    guid = bytes([1, 0, 0, 0, 0, 0, 0x10, 0, 0x80, 0, 0, 0xAA, 0, 0x38, 0x9B, 0x71])
    body = struct.pack("<HHIIHHHHI", 0xFFFE, channels, 48000, 48000 * channels * 3, channels * 3, 24, 22, 24, 0) + guid
    return (b"RIFF" + struct.pack("<I", 4 + 8 + len(body) + 8 + size) + b"WAVEfmt " + struct.pack("<I", len(body))
            + body + b"data" + struct.pack("<I", size))


os.makedirs(folder, exist_ok=True)
open(os.path.join(folder, "remote.wav"), "wb").write(header(channels, frames) + bytes(scaled))
open(os.path.join(folder, "local.wav"), "wb").write(header(1, frames) + bytes(frames * 3))
seconds = frames / 48000
minutes, rest = divmod(int(seconds) + 1, 60)
session = {"contract": "session-audio/v2", "started": "2026-10-27T10:00:00+01:00",
           "stopped": "2026-10-27T10:%02d:%02d+01:00" % (minutes, rest),
           "tracks": {"local": {"device": "MacBook Pro Microphone"}, "remote": {"device": "MacBook Pro Speakers"}}}
json.dump(session, open(os.path.join(folder, "session.json"), "w"))
with open(os.path.join(folder, "clock.jsonl"), "w") as clock:
    for second in range(0, int(seconds) + 1, 10):
        for track in ("local", "remote"):
            clock.write(json.dumps({"track": track, "sample": second * 48000, "at": float(second)}) + "\n")
print(f"{folder}: {channels} channels, {seconds:.0f} s, gain {gain}")
