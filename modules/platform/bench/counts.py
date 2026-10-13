"""Reads transcripts against the bench's truth, and counts what they got wrong.

    uv run python -m bench.counts <session-folder> [<session-folder> …]

Each folder holds `truth.json`, written by `bench.render`, and `transcript.jsonl`, written by
the program under test (`transcript/v1`). Given several folders — the draws of one listening
condition — each count is printed as its lowest, its middle and its highest: a difference
smaller than that spread is not a difference.

The counts, each with what it is counted out of:

- words wrong, on each side: the words to change, add or take out of what the transcript
  gives a side to get what that side said — of which, those of turns it did not hold at all;
- local words lost under the remote: words the local side said while the remote side spoke,
  that the transcript does not have — what muting the microphone to kill an echo costs;
- words said twice: words the transcript gives the local side that are the remote side's,
  and that it gives the remote side too — the loudspeakers heard by the microphone;
- words on the wrong side: words of one side that the transcript gives only to the other;
- exchanges out of order: places where the meeting passes from one side to the other and
  the transcript puts the second turn before the first;
- turns far from their time: turns the transcript starts more than two seconds away from
  where they were said.
"""

from __future__ import annotations

import itertools
import json
import re
import statistics
import sys
import unicodedata
from pathlib import Path

import jiwer
from num2words import num2words

NEAR = 10.0
"""Seconds around a transcript's segment in which the other side's words are "just then"."""

FAR = 2.0
"""Seconds between where a turn was said and where the transcript starts it, beyond which a
reader is misled about when it was said."""

SIDES = {"me": "local", "others": "remote"}
OTHER = {"local": "remote", "remote": "local"}


def words(text: str) -> list[str]:
    """A text as the words a listener would count: no case, no accents, no punctuation — and
    a number as it is said, since an engine writes 12 where the script says douze."""
    spoken = re.sub(
        r"\d+", lambda number: " " + num2words(int(number.group()), lang="fr") + " ", text.replace("%", " pour cent ")
    )
    plain = unicodedata.normalize("NFKD", spoken.lower().replace("’", "'"))
    plain = "".join(letter for letter in plain if not unicodedata.combining(letter))
    return re.findall(r"[a-z]+", plain)


def said(truth: list[dict], side: str) -> list[dict]:
    """Each word a side really said: the word, its instant, its turn and that turn's times."""
    spoken = []
    for number, turn in enumerate(truth):
        if turn["side"] != side:
            continue
        for written, when in turn.get("words") or [[turn["text"], turn["start"]]]:
            spoken += [
                {"word": word, "at": when, "turn": number, "start": turn["start"], "end": turn["end"]}
                for word in words(written)
            ]
    return spoken


def heard(transcript: list[dict], side: str) -> list[dict]:
    """Each word the transcript gives a side, with its segment's times, in the transcript's order."""
    mine = sorted((s for s in transcript if SIDES.get(s["speaker"]) == side), key=lambda s: s["start"])
    return [{"word": word, "start": s["start"], "end": s["end"]} for s in mine for word in words(s["text"])]


def borrowed(extra: list[dict], theirs: list[dict]) -> list[bool]:
    """For each word a side was given and did not say: is it the other side's — said by them
    just then, and next to the word they said it next to? One word alone proves nothing: `le`
    is said by everyone, all the time."""
    spoken = {}
    for first, second in itertools.pairwise(theirs):
        if first["turn"] == second["turn"]:
            spoken.setdefault((first["word"], second["word"]), []).append((first["start"], first["end"]))

    def just_then(a: dict, b: dict) -> bool:
        return any(start - NEAR <= a["start"] <= end + NEAR for start, end in spoken.get((a["word"], b["word"]), []))

    taken = [False] * len(extra)
    for i in range(len(extra) - 1):
        if extra[i + 1].get("follows") and just_then(extra[i], extra[i + 1]):
            taken[i] = taken[i + 1] = True
    return taken


def count(truth: list[dict], transcript: list[dict]) -> dict[str, tuple[int, int]]:
    """Every count of a transcript against the truth, each with what it is counted out of."""
    result: dict[str, tuple[int, int]] = {}
    was = {side: said(truth, side) for side in SIDES.values()}
    got = {side: heard(transcript, side) for side in SIDES.values()}
    matched: dict[str, set[int]] = {}
    extra: dict[str, list[dict]] = {}
    placed: dict[int, list[float]] = {}
    for side in ("local", "remote"):
        matched[side], extra[side] = set(), []
        wrong = len(was[side])
        if got[side]:
            aligned = jiwer.process_words(
                " ".join(w["word"] for w in was[side]), " ".join(g["word"] for g in got[side])
            )
            wrong = aligned.substitutions + aligned.deletions + aligned.insertions
            for chunk in aligned.alignments[0]:
                if chunk.type == "equal":
                    for was_at, got_at in zip(
                        range(chunk.ref_start_idx, chunk.ref_end_idx), range(chunk.hyp_start_idx, chunk.hyp_end_idx)
                    ):
                        matched[side].add(was_at)
                        placed.setdefault(was[side][was_at]["turn"], []).append(got[side][got_at]["start"])
                elif chunk.type in ("insert", "substitute"):
                    # What the side was given and did not say, each word knowing whether it follows the one before.
                    run = got[side][chunk.hyp_start_idx : chunk.hyp_end_idx]
                    extra[side] += [{**word, "follows": i > 0} for i, word in enumerate(run)]
        result[f"words wrong, {side}"] = (wrong, len(was[side]))

    # A turn is found when the transcript has half its words, and placed where most of them are.
    size = {number: len(words(turn["text"])) for number, turn in enumerate(truth)}
    found = {number: statistics.median(starts) for number, starts in placed.items() if 2 * len(starts) >= size[number]}
    for side in ("local", "remote"):
        dropped = sum(1 for i, word in enumerate(was[side]) if word["turn"] not in found and i not in matched[side])
        result[f"of which in turns not found, {side}"] = (dropped, len(was[side]))

    under = [
        i
        for i, word in enumerate(was["local"])
        if any(t["side"] == "remote" and t["start"] <= word["at"] <= t["end"] for t in truth)
    ]
    result["local words lost under the remote"] = (sum(i not in matched["local"] for i in under), len(under))

    # The other side's words in what a side was given: said twice when the transcript gives
    # them to their own side too — their turn is found there — on the wrong side when it is not.
    for side in ("local", "remote"):
        theirs = was[OTHER[side]]
        taken = [word for word, is_theirs in zip(extra[side], borrowed(extra[side], theirs)) if is_theirs]
        # A borrowed word belongs to the turn of theirs nearest in time.
        elsewhere = sum(1 for word in taken if min(theirs, key=lambda t: abs(t["at"] - word["start"]))["turn"] in found)
        if side == "local":
            result["words said twice"] = (elsewhere, len(theirs))
        result[f"words on the wrong side, given to the {side}"] = (len(taken) - elsewhere, len(theirs))

    exchanges = [(a, a + 1) for a in range(len(truth) - 1) if truth[a]["side"] != truth[a + 1]["side"]]
    known = [(a, b) for a, b in exchanges if a in found and b in found]
    result["exchanges out of order"] = (sum(found[b] < found[a] for a, b in known), len(known))
    result["exchanges not found"] = (len(exchanges) - len(known), len(exchanges))
    result["turns far from their time"] = (sum(abs(found[n] - truth[n]["start"]) > FAR for n in found), len(found))
    result["turns not found"] = (len(truth) - len(found), len(truth))
    return result


def read(folder: Path) -> tuple[list[dict], list[dict]]:
    """A session's truth and its transcript — a whole one: opened by its header, closed by `complete`."""
    truth = json.loads((folder / "truth.json").read_text(encoding="utf-8"))["turns"]
    lines = [
        json.loads(line)
        for line in (folder / "transcript.jsonl").read_text(encoding="utf-8").splitlines()
        if line.strip()
    ]
    if not lines or lines[0].get("type") != "header" or lines[-1].get("type") != "complete":
        raise ValueError(f"{folder}: the transcript is not a whole one — no header, or no `complete` line")
    return truth, [line for line in lines if line.get("type") == "segment"]


def main(argv: list[str]) -> int:
    results = [count(*read(Path(folder))) for folder in argv[1:]]
    for name in results[0]:
        numbers, total = sorted(result[name][0] for result in results), results[0][name][1]
        spread = (
            f"{numbers[0]}" if len(numbers) == 1 else f"{numbers[0]} – {statistics.median_low(numbers)} – {numbers[-1]}"
        )
        print(f"{name}: {spread} of {total}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
