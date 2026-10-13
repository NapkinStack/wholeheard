"""Reads a transcript against the bench's truth, and counts what it got wrong.

    uv run python -m bench.counts <session-folder>

The folder holds `truth.json`, written by `bench.render`, and `transcript.jsonl`, written by
the program under test (`transcript/v1`). Four counts, each with what it is counted out of:

- words wrong, on each side: the words to change, add or take out of what the transcript
  gives a side to get what that side said;
- words said twice: words the transcript gives the local side that the remote side was
  saying just then — the loudspeakers heard by the microphone;
- exchanges out of order: places where the meeting passes from one side to the other and
  the transcript puts the second turn before the first;
- words on the wrong side: words a side said that the transcript gives only to the other.
"""

from __future__ import annotations

import json
import re
import sys
import unicodedata
from pathlib import Path

import jiwer

NEAR = 10.0
"""Seconds around a transcript's segment in which the other side's words are "just then"."""

SIDES = {"me": "local", "others": "remote"}


def words(text: str) -> list[str]:
    """A text as the words a listener would count: no case, no accents, no punctuation."""
    plain = unicodedata.normalize("NFKD", text.lower().replace("’", "'"))
    plain = "".join(letter for letter in plain if not unicodedata.combining(letter))
    return re.findall(r"[a-z0-9]+", plain)


def said(truth: list[dict], side: str) -> list[tuple[str, float, float, int]]:
    """Each word a side really said: the word, when its turn started and ended, and which turn."""
    return [
        (word, turn["start"], turn["end"], number)
        for number, turn in enumerate(truth)
        if turn["side"] == side
        for word in words(turn["text"])
    ]


def heard(transcript: list[dict], side: str) -> list[tuple[str, float, float]]:
    """Each word the transcript gives a side, with its segment's times, in the transcript's order."""
    mine = sorted((s for s in transcript if SIDES.get(s["speaker"]) == side), key=lambda s: s["start"])
    return [(word, segment["start"], segment["end"]) for segment in mine for word in words(segment["text"])]


def count(truth: list[dict], transcript: list[dict]) -> dict:
    """The four counts of a transcript against the truth, each with its total."""
    result: dict = {"words wrong": {}}
    placed: dict[int, float] = {}
    added: dict[str, list[tuple[str, float, float]]] = {}
    missed: dict[str, list[tuple[str, float, float]]] = {}
    for side in ("local", "remote"):
        was, got = said(truth, side), heard(transcript, side)
        if not got:
            result["words wrong"][side] = (len(was), len(was))
            added[side], missed[side] = [], [(word, start, end) for word, start, end, _ in was]
            continue
        aligned = jiwer.process_words(" ".join(w[0] for w in was), " ".join(g[0] for g in got))
        result["words wrong"][side] = (aligned.substitutions + aligned.deletions + aligned.insertions, len(was))
        added[side], missed[side] = [], []
        for chunk in aligned.alignments[0]:
            if chunk.type == "insert":
                added[side] += got[chunk.hyp_start_idx : chunk.hyp_end_idx]
            elif chunk.type == "delete":
                missed[side] += [w[:3] for w in was[chunk.ref_start_idx : chunk.ref_end_idx]]
            elif chunk.type == "equal":
                # A turn is placed where the transcript puts the first of its words it got right.
                for was_at, got_at in zip(
                    range(chunk.ref_start_idx, chunk.ref_end_idx), range(chunk.hyp_start_idx, chunk.hyp_end_idx)
                ):
                    placed.setdefault(was[was_at][3], got[got_at][1])

    def near(word: tuple[str, float, float], others: list[tuple[str, float, float]]) -> bool:
        return any(word[0] == other[0] and other[1] - NEAR <= word[1] <= other[2] + NEAR for other in others)

    remote_said = [w[:3] for w in said(truth, "remote")]
    twice = sum(near(word, remote_said) for word in added["local"])
    result["words said twice"] = (twice, len(remote_said))

    exchanges = [
        (a, b) for a, b in zip(range(len(truth)), range(1, len(truth))) if truth[a]["side"] != truth[b]["side"]
    ]
    known = [(a, b) for a, b in exchanges if a in placed and b in placed]
    result["exchanges out of order"] = (sum(placed[b] < placed[a] for a, b in known), len(exchanges))
    result["exchanges not found"] = (len(exchanges) - len(known), len(exchanges))

    wrong = sum(near(word, added["remote"]) for word in missed["local"]) + sum(
        near(word, added["local"]) for word in missed["remote"]
    )
    result["words on the wrong side"] = (wrong, len(said(truth, "local")) + len(remote_said))
    return result


def read(folder: Path) -> tuple[list[dict], list[dict]]:
    truth = json.loads((folder / "truth.json").read_text(encoding="utf-8"))["turns"]
    lines = [
        json.loads(line)
        for line in (folder / "transcript.jsonl").read_text(encoding="utf-8").splitlines()
        if line.strip()
    ]
    return truth, [line for line in lines if line.get("type") == "segment"]


def main(argv: list[str]) -> int:
    result = count(*read(Path(argv[1])))
    for name, value in result.items():
        for label, (number, total) in value.items() if isinstance(value, dict) else [("", value)]:
            print(f"{name}{' — ' + label if label else ''}: {number} of {total}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
