"""Passages of real meetings, each with what a person heard said: the measure the bench cannot
give, since its voices are synthesised.

    uv run python -m bench.real build <parquet-folder> <out-folder> [<minutes>]
    uv run python -m bench.real score <passage-folder>...

`build` reads the shards of SUMM-RE (linagora/SUMM-RE on Hugging Face, CC BY-SA 4.0: French
meetings, each participant on their own microphone, transcribed by people) that are in
`<parquet-folder>`, and for each meeting found whole makes `<out-folder>/<meeting>/`:
`v1/` and `v2/`, the same passage as both forms of a session — the participant who speaks most
is the local side, the others mixed are the remote — and `reference.json`, the turns said in it.
A passage is cut where nobody speaks, so that no turn is half in it.

`score` reads `transcript.jsonl` in each folder beside a `reference.json` — `v1/`, `v2/`, or a
copy made to try another engine or model — and prints, for each side apart and without the
fillers a transcript leaves out, the share of words said that it missed or misheard, and the
words it wrote beyond them, per passage, then the median. The two are kept apart because
SUMM-RE's participants often share a room: each microphone also carries the others faintly, so
a remote side can rightly hold words the reference gives to the local one.
"""

from __future__ import annotations

import io
import json
import statistics
import sys
from pathlib import Path

import jiwer
import numpy as np
from scipy.io import wavfile
from scipy.signal import resample_poly

from bench.counts import words
from bench.render import RATE, write_track, write_wav

FILLERS = {"euh", "heu", "hum", "hm", "hmm", "mh", "mm"}
"""What people say while thinking and a transcript rarely writes: counted apart."""

HEARD = 16_000
"""The rate the first form holds. SUMM-RE's microphones are mostly at the devices' rate, `RATE`;
a recording at another rate is brought to it."""


def spoken(text: str) -> list[str]:
    """A reference's words: its marks taken out — `_` joins a phrase, `+` cuts a word, `*` is
    a word nobody could hear."""
    return words(text.replace("_", " ").replace("+", " ").replace("*", " "))


def meetings(folder: Path) -> dict[str, list[dict]]:
    """Each meeting of the shards, its participants' rows: audio, timed segments, speaker."""
    import pyarrow.parquet as pq

    found: dict[str, list[dict]] = {}
    for shard in sorted(folder.glob("*.parquet")):
        for row in pq.read_table(shard, columns=["meeting_id", "speaker_id", "audio", "segments"]).to_pylist():
            found.setdefault(row["meeting_id"], []).append(row)
    return found


def quiet(at: float, turns: list[tuple[float, float]], reach: float = 15.0) -> float:
    """The instant nearest `at` that no turn spans."""
    for step in np.arange(0.0, reach, 0.1):
        for moment in (at - step, at + step):
            if not any(start < moment < end for start, end in turns):
                return float(moment)
    raise ValueError(f"no moment without speech within {reach} s of {at:.0f} s")


def window(turns: list[dict], length: float, total: float) -> tuple[float, float]:
    """The passage of `length` seconds where the most words are said, cut where nobody speaks;
    the first minute is left out, as introductions are."""
    spans = [(turn["start"], turn["end"]) for turn in turns]
    best = max(
        np.arange(60.0, max(61.0, total - length - 30.0), 30.0),
        key=lambda start: sum(
            len(spoken(turn["text"])) for turn in turns if start <= turn["start"] and turn["end"] <= start + length
        ),
    )
    return quiet(best, spans), quiet(best + length, spans)


def build(source: Path, out: Path, minutes: float = 5.0) -> None:
    for meeting, rows in sorted(meetings(source).items()):
        tracks, turns = {}, []
        for row in rows:
            rate, sound = wavfile.read(io.BytesIO(row["audio"]["bytes"]))
            assert sound.ndim == 1, f"{meeting}: {sound.ndim} channels"
            sound = sound.astype(np.float32) / 32768
            if rate != RATE:
                shared = np.gcd(rate, RATE)
                sound = resample_poly(sound, RATE // shared, rate // shared).astype(np.float32)
            tracks[row["speaker_id"]] = sound
            turns += [
                {"speaker": row["speaker_id"], "start": s["start"], "end": s["end"], "text": s["transcript"]}
                for s in row["segments"]
            ]
        if len(tracks) < 3:
            print(f"{meeting}: {len(tracks)} participant(s) in these shards — left out")
            continue
        total = min(len(sound) for sound in tracks.values()) / RATE
        start, end = window(turns, minutes * 60, total)
        kept = sorted(
            (
                dict(turn, start=round(turn["start"] - start, 2), end=round(turn["end"] - start, 2))
                for turn in turns
                if start <= turn["start"] and turn["end"] <= end
            ),
            key=lambda turn: turn["start"],
        )
        said = {speaker: sum(len(spoken(t["text"])) for t in kept if t["speaker"] == speaker) for speaker in tracks}
        local = max(said, key=said.get)
        cut = {speaker: sound[round(start * RATE) : round(end * RATE)] for speaker, sound in tracks.items()}
        sides = {"local": cut[local], "remote": np.clip(sum(s for k, s in cut.items() if k != local), -1.0, 1.0)}
        for turn in kept:
            turn["side"] = "local" if turn["speaker"] == local else "remote"
        folder = out / meeting
        write_v1(folder / "v1", sides)
        write_v2(folder / "v2", sides)
        (folder / "reference.json").write_text(
            json.dumps(
                {
                    "meeting": meeting,
                    "from": round(start, 2),
                    "to": round(end, 2),
                    "local": local,
                    "source": "linagora/SUMM-RE, CC BY-SA 4.0",
                    "turns": kept,
                },
                ensure_ascii=False,
                indent=1,
            ),
            encoding="utf-8",
        )
        print(
            f"{meeting}: {end - start:.0f} s from {start:.0f} s, {len(kept)} turns, local {local} of {sorted(tracks)}"
        )


def session(folder: Path, record: dict) -> None:
    record |= {"started": "2026-10-17T09:00:00+02:00", "stopped": "2026-10-17T09:05:00+02:00"}
    (folder / "session.json").write_text(json.dumps(record), encoding="utf-8")


def write_v1(folder: Path, sides: dict[str, np.ndarray]) -> None:
    folder.mkdir(parents=True, exist_ok=True)
    for name, side in (("me", "local"), ("others", "remote")):
        write_wav(folder / f"{name}.wav", resample_poly(sides[side], HEARD, RATE), HEARD)
    session(
        folder,
        {"contract": "session-audio/v1", "sources": {"me": {"device": "SUMM-RE"}, "others": {"device": "SUMM-RE"}}},
    )


def write_v2(folder: Path, sides: dict[str, np.ndarray]) -> None:
    """As a capture writes it: each side as its microphone delivered it, from the session's
    start, a point of the clock every five seconds; no source stops."""
    folder.mkdir(parents=True, exist_ok=True)
    lines = []
    for side, sound in sides.items():
        write_track(folder / f"{side}.wav", sound)
        lines += [{"track": side, "sample": n, "at": n / RATE} for n in range(0, len(sound), 5 * RATE)]
    (folder / "clock.jsonl").write_text("".join(json.dumps(line) + "\n" for line in lines), encoding="utf-8")
    session(
        folder,
        {"contract": "session-audio/v2", "tracks": {"local": {"device": "SUMM-RE"}, "remote": {"device": "SUMM-RE"}}},
    )


def errors(reference: list[str], transcript: list[str]) -> tuple[float, float]:
    """The share of the reference's words missed or misheard, and of words written beyond them."""
    if not reference:
        return 0.0, 0.0
    found = jiwer.process_words(" ".join(reference), " ".join(transcript) or "")
    return (found.substitutions + found.deletions) / len(reference), found.insertions / len(reference)


def score(passages: list[Path]) -> None:
    rows: dict[tuple[str, str], list[tuple[float, float]]] = {}
    for passage in passages:
        reference = json.loads((passage / "reference.json").read_text(encoding="utf-8"))
        for path in sorted(passage.glob("*/transcript.jsonl")):
            form = path.parent.name
            lines = [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line.strip()]
            segments = sorted((s for s in lines if s.get("type") == "segment"), key=lambda s: s["start"])
            for side in ("local", "remote"):
                truth = [w for t in reference["turns"] if t["side"] == side for w in spoken(t["text"])]
                heard = [w for s in segments if s.get("track") == side for w in words(s["text"])]
                found = errors([w for w in truth if w not in FILLERS], [w for w in heard if w not in FILLERS])
                rows.setdefault((form, side), []).append(found)
                print(
                    f"{reference['meeting']} {form} {side}: {found[0]:.1%} missed or misheard, "
                    f"{found[1]:.1%} written beyond"
                )
    for (form, side), found in sorted(rows.items()):
        print(
            f"median {form} {side}: {statistics.median(a for a, _ in found):.1%} missed or misheard, "
            f"{statistics.median(b for _, b in found):.1%} written beyond, over {len(found)} passage(s)"
        )


def main(argv: list[str]) -> int:
    if len(argv) >= 4 and argv[1] == "build":
        build(Path(argv[2]), Path(argv[3]), float(argv[4]) if len(argv) > 4 else 5.0)
    elif len(argv) >= 3 and argv[1] == "score":
        score([Path(arg) for arg in argv[2:]])
    else:
        print(__doc__)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
