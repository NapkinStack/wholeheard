"""The four counts, tried on transcripts written here: each wrong in one known way."""

from bench.counts import count, words

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


def segment(speaker: str, start: float, text: str) -> dict:
    return {"type": "segment", "speaker": speaker, "start": start, "end": start + 4.0, "text": text}


def right() -> list[dict]:
    """The transcript that gets everything right: each turn, on its side, at its time."""
    return [segment("me" if turn["side"] == "local" else "others", turn["start"], turn["text"]) for turn in TRUTH]


def test_words_are_counted_as_a_listener_counts_them():
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


def test_a_transcript_that_is_right_counts_nothing():
    assert count(TRUTH, right()) == {
        "words wrong": {"local": (0, 14), "remote": (0, 13)},
        "words said twice": (0, 13),
        "exchanges out of order": (0, 3),
        "exchanges not found": (0, 3),
        "words on the wrong side": (0, 27),
    }


def test_words_wrong_are_those_to_change_add_or_take_out_on_their_side():
    transcript = right()
    transcript[0]["text"] = "Bonjour à tous, on commence la séance."  # one word changed
    transcript[3]["text"] = "Parfait, le support sera complet."  # one taken out
    result = count(TRUTH, transcript)
    assert result["words wrong"] == {"local": (1, 14), "remote": (1, 13)}
    assert result["words said twice"][0] == 0 and result["words on the wrong side"][0] == 0


def test_what_the_microphone_heard_of_the_loudspeakers_is_words_said_twice():
    # Sophie's sentence, heard by the microphone, comes out under the local side too.
    transcript = right() + [segment("me", 5.2, "Bonjour Claire, je vous entends très bien.")]
    result = count(TRUTH, transcript)
    assert result["words said twice"] == (7, 13)
    assert result["words wrong"]["local"] == (7, 14) and result["words wrong"]["remote"] == (0, 13)
    # They were not taken from the remote side: nothing is on the wrong side.
    assert result["words on the wrong side"][0] == 0


def test_a_turn_placed_before_the_one_it_answers_is_an_exchange_out_of_order():
    transcript = right()
    transcript[1]["start"] = -3.0  # the remote's first turn, placed before the local one it answers
    result = count(TRUTH, transcript)
    assert result["exchanges out of order"] == (1, 3)
    assert result["words wrong"] == {"local": (0, 14), "remote": (0, 13)}


def test_a_turn_given_to_the_other_side_is_words_on_the_wrong_side_and_an_exchange_not_found():
    transcript = right()
    transcript[2]["speaker"] = "others"  # Marc's sentence, given to the remote side
    result = count(TRUTH, transcript)
    assert result["words on the wrong side"] == (7, 27)
    assert result["exchanges not found"][0] >= 1


def test_a_side_the_transcript_never_heard_is_every_word_wrong_and_nothing_crashes():
    result = count(TRUTH, [s for s in right() if s["speaker"] == "me"])
    assert result["words wrong"]["remote"] == (13, 13) and result["words wrong"]["local"] == (0, 14)
    assert count(TRUTH, [])["words wrong"] == {"local": (14, 14), "remote": (13, 13)}
