"""The counts, tried on transcripts written here: each wrong in one known way."""

import json

import pytest

from bench.counts import count, read, words

TRUTH = [
    {"voice": "claire", "side": "local", "start": 0.0, "end": 4.0, "text": "Bonjour à tous, on commence la réunion."},
    {
        "voice": "sophie",
        "side": "remote",
        "start": 5.0,
        "end": 9.0,
        "text": "Bonjour Claire, je vous entends très bien.",
    },
    {"voice": "marc", "side": "local", "start": 10.0, "end": 14.0, "text": "La version de test sera prête jeudi."},
    {"voice": "julien", "side": "remote", "start": 15.0, "end": 19.0, "text": "Parfait, le support sera au complet."},
]
NOTHING = {
    "words wrong, local": (0, 14),
    "of which in turns not found, local": (0, 14),
    "words wrong, remote": (0, 13),
    "of which in turns not found, remote": (0, 13),
    "local words lost under the remote": (0, 0),
    "words said twice": (0, 13),
    "words on the wrong side, given to the local": (0, 13),
    "words on the wrong side, given to the remote": (0, 14),
    "exchanges out of order": (0, 3),
    "exchanges not found": (0, 3),
    "turns far from their time": (0, 4),
    "turns not found": (0, 4),
}


def segment(speaker: str, start: float, text: str) -> dict:
    return {"type": "segment", "speaker": speaker, "start": start, "end": start + 4.0, "text": text}


def right(truth: list[dict] = TRUTH) -> list[dict]:
    """The transcript that gets everything right: each turn, on its side, at its time."""
    return [segment("me" if turn["side"] == "local" else "others", turn["start"], turn["text"]) for turn in truth]


def moved(result: dict) -> dict:
    """What a transcript's counts say beyond those of the one that is right."""
    return {name: value for name, value in result.items() if value != NOTHING[name]}


def test_words_are_counted_as_a_listener_counts_them_and_a_number_as_it_is_said():
    assert words("Bonjour à TOUS, l’équipe : c'est prêt !") == [
        "bonjour",
        "a",
        "tous",
        "l",
        "equipe",
        "c",
        "est",
        "pret",
    ]
    assert words("Le 12 novembre, 62 % de l'enveloppe.") == words(
        "Le douze novembre, soixante-deux pour cent de l'enveloppe."
    )


def test_a_transcript_that_is_right_counts_nothing():
    assert count(TRUTH, right()) == NOTHING


def test_words_wrong_are_those_to_change_add_or_take_out_on_their_side():
    transcript = right()
    transcript[0]["text"] = "Bonjour à tous, on commence la séance."  # one word changed
    transcript[3]["text"] = "Parfait, le support sera complet."  # one taken out
    assert moved(count(TRUTH, transcript)) == {"words wrong, local": (1, 14), "words wrong, remote": (1, 13)}


def test_a_turn_the_transcript_does_not_hold_is_said_apart_from_words_misheard():
    transcript = [s for s in right() if "version" not in s["text"]]
    result = moved(count(TRUTH, transcript))
    assert result["words wrong, local"] == (7, 14) and result["of which in turns not found, local"] == (7, 14)
    assert result["turns not found"] == (1, 4)


def test_what_the_microphone_heard_of_the_loudspeakers_is_words_said_twice():
    # Sophie's sentence, heard by the microphone, comes out under the local side too.
    echo = segment("me", 5.2, "Bonjour Claire, je vous entends très bien.")
    assert moved(count(TRUTH, right() + [echo])) == {"words wrong, local": (7, 14), "words said twice": (7, 13)}
    # In other segments, later in the file, cut in two: the same words, the same count.
    cut = [segment("me", 5.2, "Bonjour Claire, je vous"), segment("me", 7.0, "entends très bien.")]
    assert moved(count(TRUTH, right() + cut))["words said twice"][0] >= 6
    # A word everybody says, added alone, is nobody's echo.
    stray = segment("me", 15.5, "le")
    assert moved(count(TRUTH, right() + [stray])) == {"words wrong, local": (1, 14)}
    # Said long after, it is not the loudspeakers any more.
    late = segment("me", 60.0, "Bonjour Claire, je vous entends très bien.")
    assert "words said twice" not in moved(count(TRUTH, right() + [late]))


def test_a_turn_given_to_the_other_side_is_words_on_the_wrong_side():
    transcript = right()
    transcript[2]["speaker"] = "others"  # Marc's sentence, given to the remote side
    result = moved(count(TRUTH, transcript))
    assert result["words on the wrong side, given to the remote"] == (7, 14)
    assert "words said twice" not in result and result["turns not found"] == (1, 4)
    # The two sides exchanged whole: nearly every word is on the wrong side — all but those
    # the two sides happen to say alike, which no count can tell apart.
    swapped = [{**s, "speaker": "others" if s["speaker"] == "me" else "me"} for s in right()]
    result = count(TRUTH, swapped)
    assert result["words on the wrong side, given to the remote"][0] >= 10
    assert result["words on the wrong side, given to the local"][0] >= 10
    assert result["words said twice"][0] == 0


def test_a_turn_placed_before_the_one_it_answers_is_out_of_order_and_far_from_its_time():
    transcript = right()
    transcript[1]["start"] = -3.0  # the remote's first turn, placed before the local one it answers
    assert moved(count(TRUTH, transcript)) == {"exchanges out of order": (1, 3), "turns far from their time": (1, 4)}
    # Every turn where the one before ended, as an engine does: in order, and far from its time.
    early = right()
    for s in early[1:]:
        s["start"] -= 2.5
    assert moved(count(TRUTH, early)) == {"turns far from their time": (3, 4)}
    # One segment a side, both starting at nought: nothing can be said of the order, and that is said.
    lumps = [segment("me", 0.0, " ".join(t["text"] for t in TRUTH if t["side"] == "local"))]
    lumps += [segment("others", 0.0, " ".join(t["text"] for t in TRUTH if t["side"] == "remote"))]
    assert moved(count(TRUTH, lumps)) == {"turns far from their time": (3, 4)}


def test_words_said_over_the_remote_side_and_lost_are_counted_which_muting_the_microphone_costs():
    over = {"voice": "marc", "side": "local", "start": 6.0, "end": 8.0, "text": "Oui, tout à fait d'accord."}
    truth = [TRUTH[0], TRUTH[1], over, TRUTH[2], TRUTH[3]]
    kept = count(truth, right(truth))
    assert kept["local words lost under the remote"] == (0, 6) and kept["words wrong, local"] == (0, 20)
    muted = count(truth, [s for s in right(truth) if "tout à fait" not in s["text"]])
    assert muted["local words lost under the remote"] == (6, 6)
    assert muted["words said twice"] == (0, 13)  # an echo killed that way looks perfect on that count alone


def test_a_side_the_transcript_never_heard_is_every_word_wrong_and_nothing_crashes():
    result = count(TRUTH, [s for s in right() if s["speaker"] == "me"])
    assert result["words wrong, remote"] == (13, 13) and result["words wrong, local"] == (0, 14)
    assert count(TRUTH, [])["turns not found"] == (4, 4)


def test_a_transcript_that_is_not_whole_is_not_counted(tmp_path):
    (tmp_path / "truth.json").write_text(json.dumps({"turns": TRUTH}), encoding="utf-8")
    lines = [{"type": "header", "contract": "transcript/v1"}, *right(), {"type": "complete", "duration": 20.0}]
    (tmp_path / "transcript.jsonl").write_text("\n".join(json.dumps(line) for line in lines), encoding="utf-8")
    assert count(*read(tmp_path)) == NOTHING
    (tmp_path / "transcript.jsonl").write_text("\n".join(json.dumps(line) for line in lines[:-1]), encoding="utf-8")
    with pytest.raises(ValueError, match="not a whole one"):
        read(tmp_path)
