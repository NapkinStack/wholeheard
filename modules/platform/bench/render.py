"""Renders the bench's meeting: the tracks a recording of it would hold, and the truth beside.

    uv run python -m bench.render <voices-folder> <out-folder>

The voices are synthesised: nobody is recorded. `<voices-folder>` holds the four public
models `VOICES` names, fetched once with `python -m piper.download_voices`.
"""

from __future__ import annotations

import hashlib
import json
import sys
import wave
from collections.abc import Callable
from pathlib import Path

import numpy as np
from scipy.signal import resample_poly

RATE = 48_000
"""What a laptop's sound devices deliver: the rate the meeting is made at."""

VOICES = {
    "claire": ("fr_FR-siwis-medium", 0),
    "marc": ("fr_FR-tom-medium", 0),
    "sophie": ("fr_FR-upmc-medium", 0),
    "julien": ("fr_FR-upmc-medium", 1),
}
"""Which public model, and which of its speakers, says each voice of the script."""

Synth = Callable[[str, str], np.ndarray]
"""A voice and a text in, its sound out: one channel, `RATE` samples a second, between -1 and 1."""


def script(path: Path) -> dict:
    """The meeting as written: its voices, each on a side, and its turns in order."""
    meeting: dict = {"voices": {}, "turns": []}
    for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if not line.strip() or line.startswith("#"):
            continue
        first, second, rest = (line.split(None, 2) + ["", ""])[:3]
        if first == "voice":
            meeting["voices"][second] = {"side": rest}
        elif first in meeting["voices"]:
            meeting["turns"].append({"voice": first, "gap": float(second), "text": rest})
        else:
            raise ValueError(
                f"{path.name}, line {number}: a turn is said by {first!r}, a voice the script does not name"
            )
    return meeting


def piper(voices: Path, cache: Path) -> Synth:
    """The synthesiser: each sentence is made once and kept, so that a meeting rendered twice is the same."""
    from piper import PiperVoice, SynthesisConfig

    loaded: dict[str, PiperVoice] = {}

    def say(voice: str, text: str) -> np.ndarray:
        model, speaker = VOICES[voice]
        kept = cache / (hashlib.sha256(f"{model}/{speaker}/{text}".encode()).hexdigest()[:24] + ".npy")
        if kept.exists():
            return np.load(kept)
        if model not in loaded:
            loaded[model] = PiperVoice.load(voices / f"{model}.onnx")
        chunks = list(loaded[model].synthesize(text, SynthesisConfig(speaker_id=speaker)))
        sound = np.concatenate([chunk.audio_float_array for chunk in chunks])
        sound = resample_poly(sound, RATE, chunks[0].sample_rate).astype(np.float32)
        cache.mkdir(parents=True, exist_ok=True)
        np.save(kept, sound)
        return sound

    return say


def lay_out(meeting: dict, say: Synth) -> tuple[dict[str, np.ndarray], list[dict]]:
    """Places each turn on the meeting's clock, and mixes each side's voices into its track.

    Returns the two tracks — what each side said, and nothing of the other: a headset — and
    the truth: each turn with its voice, its side, its instants and its words.
    """
    placed, truth, cursor = [], [], 0.0
    for turn in meeting["turns"]:
        sound = say(turn["voice"], turn["text"])
        start = max(0.0, cursor + turn["gap"])
        end = start + len(sound) / RATE
        side = meeting["voices"][turn["voice"]]["side"]
        placed.append((side, round(start * RATE), sound))
        truth.append(
            {"voice": turn["voice"], "side": side, "start": round(start, 3), "end": round(end, 3), "text": turn["text"]}
        )
        cursor = max(cursor, end)
    length = round((cursor + 1.0) * RATE)
    tracks = {side: np.zeros(length, dtype=np.float32) for side in ("local", "remote")}
    for side, at, sound in placed:
        tracks[side][at : at + len(sound)] += sound
    return tracks, truth


def write_wav(path: Path, sound: np.ndarray, rate: int) -> None:
    """One channel, 16 bits: what `session-audio/v1` holds."""
    with wave.open(str(path), "wb") as wav:
        wav.setnchannels(1)
        wav.setsampwidth(2)
        wav.setframerate(rate)
        wav.writeframes((np.clip(sound, -1.0, 1.0) * 32767).astype("<i2").tobytes())


def write_session(folder: Path, tracks: dict[str, np.ndarray], truth: list[dict]) -> None:
    """A session as the programs read one today (`session-audio/v1`), and the truth beside it."""
    folder.mkdir(parents=True, exist_ok=True)
    for name, side in (("me", "local"), ("others", "remote")):
        write_wav(folder / f"{name}.wav", resample_poly(tracks[side], 16_000, RATE), 16_000)
    record = {
        "contract": "session-audio/v1",
        "started": "2026-10-14T09:00:00+02:00",
        "stopped": "2026-10-14T09:05:00+02:00",
    }
    (folder / "session.json").write_text(json.dumps(record), encoding="utf-8")
    (folder / "truth.json").write_text(json.dumps({"turns": truth}, ensure_ascii=False, indent=1), encoding="utf-8")


def main(argv: list[str]) -> int:
    voices, out = Path(argv[1]), Path(argv[2])
    meeting = script(Path(__file__).with_name("meeting.txt"))
    tracks, truth = lay_out(meeting, piper(voices, out / "cache"))
    write_session(out / "headset", tracks, truth)
    print(f"headset: {len(truth)} turns, {truth[-1]['end']:.0f} s, in {out / 'headset'}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
