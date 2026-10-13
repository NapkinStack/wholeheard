"""Reads transcripts against the bench's truth, and counts what they got wrong.

    uv run python -m bench.counts <session-folder> [<session-folder> …]

Each folder holds `truth.json`, written by `bench.render`, and `transcript.jsonl`, written by
the program under test (`transcript/v1`). Given several folders — the draws of one listening
condition — each count is printed as its lowest, its middle and its highest: a difference
smaller than that spread is not a difference.

The counts, each with what it is counted out of:

- words wrong, on each side: the words to change, add or take out of what the transcript
  gives a side to get what that side said — of which, those of turns it did not hold at all;
  and, for the local side, net of the echo: without the remote's words it was given;
- local words lost under the remote: words the local side said while the remote side spoke,
  or in the third of a second after, that the transcript does not have — what muting the
  microphone to kill an echo costs;
- words said twice: words the transcript gives the local side that are the remote side's,
  and that it gives the remote side too — the loudspeakers heard by the microphone;
- words on the wrong side: words of one side that the transcript gives only to the other;
- exchanges out of order: places where the meeting passes from one side to the other and
  the transcript puts the two turns the other way round than they were said;
- turns far from their time: turns the transcript starts more than two seconds away from
  where they were said.

A turn is where the transcript puts its first words. Order and time are read as a share of
the turns found, with the turns not found beside: a transcript that loses turns has fewer to
get wrong.
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

TAIL = 0.3
"""Seconds after a remote turn in which a local word is still under it: an echo rings, and
whatever silences the microphone holds on that long."""

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
        instants = [when for _, when in turn["words"]] + [turn["end"]]
        for (written, when), until in zip(turn["words"], instants[1:]):
            spoken += [
                {"word": word, "at": when, "until": until, "turn": number, "start": turn["start"], "end": turn["end"]}
                for word in words(written)
            ]
    return spoken


def heard(transcript: list[dict], side: str) -> list[dict]:
    """Each word the transcript gives a side, with its segment's times, in the transcript's order."""
    mine = sorted((s for s in transcript if SIDES.get(s["speaker"]) == side), key=lambda s: s["start"])
    return [{"word": word, "start": s["start"], "end": s["end"]} for s in mine for word in words(s["text"])]


def borrowed(extra: list[dict], theirs: list[dict]) -> list[int | None]:
    """For each word a side was given and did not say: the turn of the other side it was
    taken from — said by them just then, next to the word they said it next to — or none.
    One word alone proves nothing: `le` is said by everyone, all the time."""
    spoken: dict[tuple[str, str], list[dict]] = {}
    for first, second in itertools.pairwise(theirs):
        if first["turn"] == second["turn"]:
            spoken.setdefault((first["word"], second["word"]), []).append(first)

    def from_turn(a: dict, b: dict) -> int | None:
        # The pair's own turn, when the segment holding it is around when that turn was said.
        near = [
            t
            for t in spoken.get((a["word"], b["word"]), [])
            if t["start"] - NEAR <= a["end"] and a["start"] <= t["end"] + NEAR
        ]
        return min(near, key=lambda t: abs(t["at"] - a["start"]))["turn"] if near else None

    taken: list[int | None] = [None] * len(extra)
    for i in range(len(extra) - 1):
        if extra[i + 1].get("follows") and (turn := from_turn(extra[i], extra[i + 1])) is not None:
            taken[i] = turn if taken[i] is None else taken[i]
            taken[i + 1] = turn
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

    # A turn is found when the transcript has half its words, and placed where it puts the
    # first of them: the middle of a long turn is not when it was said.
    size = {number: len(words(turn["text"])) for number, turn in enumerate(truth)}
    found = {
        number: statistics.median(starts[:3]) for number, starts in placed.items() if 2 * len(starts) >= size[number]
    }
    for side in ("local", "remote"):
        dropped = sum(1 for i, word in enumerate(was[side]) if word["turn"] not in found and i not in matched[side])
        result[f"of which in turns not found, {side}"] = (dropped, len(was[side]))

    under = [
        i
        for i, word in enumerate(was["local"])
        if any(t["side"] == "remote" and word["at"] <= t["end"] + TAIL and word["until"] >= t["start"] for t in truth)
    ]
    result["local words lost under the remote"] = (sum(i not in matched["local"] for i in under), len(under))

    # The other side's words in what a side was given: said twice when the transcript gives
    # them to their own side too — their turn is found there — on the wrong side when it is not.
    for side in ("local", "remote"):
        theirs = was[OTHER[side]]
        taken = [turn for turn in borrowed(extra[side], theirs) if turn is not None]
        elsewhere = sum(1 for turn in taken if turn in found)
        if side == "local":
            result["words said twice"] = (elsewhere, len(theirs))
            # What is wrong of the local side's own words: the measure a canceller is read on —
            # muting the microphone removes the echo's words and cannot lower this.
            net = result["words wrong, local"][0] - len(taken)
            result["words wrong net of the echo, local"] = (net, len(was["local"]))
        result[f"words on the wrong side, given to the {side}"] = (len(taken) - elsewhere, len(theirs))

    exchanges = [(a, a + 1) for a in range(len(truth) - 1) if truth[a]["side"] != truth[a + 1]["side"]]
    known = [(a, b) for a, b in exchanges if a in found and b in found]
    # Speech over speech: the second turn of an exchange may have started first. The order
    # said is the truth's own, not the script's.
    reversed_ = sum((found[b] < found[a]) != (truth[b]["start"] < truth[a]["start"]) for a, b in known)
    result["exchanges out of order"] = (reversed_, len(known))
    result["exchanges not found"] = (len(exchanges) - len(known), len(exchanges))
    result["turns far from their time"] = (sum(abs(found[n] - truth[n]["start"]) > FAR for n in found), len(found))
    result["turns not found"] = (len(truth) - len(found), len(truth))
    return result


def read(folder: Path) -> tuple[list[dict], list[dict]]:
    """A session's truth and its transcript — a whole one, of that meeting, in the form this
    reads: anything else is refused, never counted as if it were."""
    truth = json.loads((folder / "truth.json").read_text(encoding="utf-8"))["turns"]
    lines = [
        json.loads(line)
        for line in (folder / "transcript.jsonl").read_text(encoding="utf-8").splitlines()
        if line.strip()
    ]
    kinds = [line.get("type") for line in lines]
    refusal = None
    if (
        not lines
        or kinds[0] != "header"
        or kinds[-1] != "complete"
        or kinds.count("header") + kinds.count("complete") != 2
    ):
        refusal = "the transcript is not one whole transcript — a header first, `complete` last, each once"
    elif lines[0].get("contract") != "transcript/v1":
        refusal = f"the transcript is {lines[0].get('contract')}, and this reads transcript/v1"
    elif any(line.get("speaker") not in SIDES for line in lines if line.get("type") == "segment"):
        refusal = "a segment names a speaker this does not know"
    elif not all(turn.get("words") for turn in truth):
        refusal = "the truth does not hold each word's instant"
    elif abs(lines[-1].get("duration", 0.0) - truth[-1]["end"]) > 5:
        refusal = f"the transcript is of {lines[-1].get('duration')} s, the meeting of {truth[-1]['end']:.0f} s"
    if refusal:
        raise ValueError(f"{folder}: {refusal}")
    return truth, [line for line in lines if line.get("type") == "segment"]


def main(argv: list[str]) -> int:
    if len(argv) < 2:
        print("usage: python -m bench.counts <session-folder> [<session-folder> …]", file=sys.stderr)
        return 2
    results = [count(*read(Path(folder))) for folder in argv[1:]]
    for name in results[0]:
        numbers, totals = sorted(result[name][0] for result in results), sorted(result[name][1] for result in results)
        spread = (
            f"{numbers[0]}" if len(numbers) == 1 else f"{numbers[0]} – {statistics.median(numbers):g} – {numbers[-1]}"
        )
        of = f"{totals[0]}" if totals[0] == totals[-1] else f"{totals[0]} to {totals[-1]}"
        print(f"{name}: {spread} of {of}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
