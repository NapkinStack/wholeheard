"""Passages of real meetings, tried on a meeting made here: three tones, each a participant's
microphone, with their turns written as SUMM-RE writes them."""

import io
import json
import wave
from pathlib import Path

import jsonschema
import numpy as np
import pyarrow as pa
import pyarrow.parquet as pq
from scipy.io import wavfile

from bench.real import FILLERS, build, quiet, score, spoken, window
from bench.render import RATE


def valid(schema: str, documents: list) -> bool:
    path = Path(__file__).parent.parent.parent.parent / "contracts" / schema
    reads = jsonschema.Draft202012Validator(json.loads(path.read_text(encoding="utf-8")))
    return all(reads.is_valid(document) for document in documents)


def test_a_reference_is_read_without_its_marks():
    assert spoken("par_rapport_à ce * que j' ai vingt+ quatre") == [
        "par",
        "rapport",
        "a",
        "ce",
        "que",
        "j",
        "ai",
        "vingt",
        "quatre",
    ]


def test_a_passage_starts_and_ends_where_nobody_speaks_and_holds_the_most_words():
    turns = [
        {"start": s, "end": s + 4.0, "text": "un mot " * n} for s, n in ((70.0, 1), (130.0, 1), (200.0, 9), (290.0, 9))
    ]
    start, end = window(turns, 120.0, 600.0)
    assert not any(t["start"] < moment < t["end"] for t in turns for moment in (start, end))
    held = [t for t in turns if start <= t["start"] and t["end"] <= end]
    assert [t["start"] for t in held] == [200.0, 290.0]
    # The instant nearest a turn's middle that no turn spans is one of its ends.
    assert round(quiet(72.5, [(70.0, 74.0)]), 1) == 74.0


def shard(folder: Path) -> None:
    """A meeting of three participants, ten minutes, each on a tone of their own, speaking in turn;
    the third recorded at another rate, as some of SUMM-RE's are."""
    rows = []
    seconds = 600
    for n, (speaker, hertz, rate) in enumerate((("001", 220, RATE), ("002", 330, RATE), ("003", 440, 32_000))):
        sound = np.zeros(seconds * rate, dtype=np.float32)
        segments = []
        for start in range(60 + 5 * n, seconds - 30, 15):
            sound[start * rate : (start + 4) * rate] = 0.2 * np.sin(2 * np.pi * hertz * np.arange(4 * rate) / rate)
            text = "euh bonjour à tous" if speaker == "001" else "oui"
            segments.append({"start": float(start), "end": start + 4.0, "transcript": text, "words": []})
        data = io.BytesIO()
        wavfile.write(data, rate, (sound * 32767).astype(np.int16))
        rows.append(
            {
                "meeting_id": "m1",
                "speaker_id": speaker,
                "audio": {"bytes": data.getvalue(), "path": f"{speaker}.wav"},
                "segments": segments,
            }
        )
    folder.mkdir(parents=True)
    pq.write_table(pa.Table.from_pylist(rows), folder / "test-00000.parquet")


def test_a_meeting_becomes_a_passage_in_both_forms_with_what_was_said_in_it(tmp_path):
    shard(tmp_path / "shards")
    build(tmp_path / "shards", tmp_path / "out", 2.0)
    passage = tmp_path / "out" / "m1"
    reference = json.loads((passage / "reference.json").read_text(encoding="utf-8"))
    # The one who speaks most is the local side; every turn kept lies whole in the passage.
    assert reference["local"] == "001"
    length = reference["to"] - reference["from"]
    assert 100 <= length <= 140 and all(0 <= t["start"] < t["end"] <= length for t in reference["turns"])
    assert {t["side"] for t in reference["turns"] if t["speaker"] == "001"} == {"local"}
    assert {t["side"] for t in reference["turns"] if t["speaker"] != "001"} == {"remote"}
    # The first form: one channel at 16 kHz each side, the local only on its own track.
    with wave.open(str(passage / "v1" / "me.wav")) as me:
        assert (me.getframerate(), me.getnchannels()) == (16_000, 1)
    assert valid("session-audio/v1/session.schema.json", [json.loads((passage / "v1" / "session.json").read_text())])
    # The second form: as the contract holds it, its clock read against its schema.
    clock = [json.loads(line) for line in (passage / "v2" / "clock.jsonl").read_text().splitlines()]
    assert valid("session-audio/v2/session.schema.json", [json.loads((passage / "v2" / "session.json").read_text())])
    assert valid("session-audio/v2/clock.schema.json", clock)
    assert {line["track"] for line in clock} == {"local", "remote"}


def test_a_transcript_that_says_what_was_said_is_scored_none_missed_side_by_side(tmp_path, capsys):
    shard(tmp_path / "shards")
    build(tmp_path / "shards", tmp_path / "out", 2.0)
    passage = tmp_path / "out" / "m1"
    turns = json.loads((passage / "reference.json").read_text(encoding="utf-8"))["turns"]
    # A transcript writing every turn on its side but its fillers, and one word more on the remote.
    lines = [
        {
            "type": "segment",
            "start": t["start"],
            "end": t["end"],
            "speaker": "me" if t["side"] == "local" else None,
            "track": t["side"],
            "text": " ".join(w for w in t["text"].split() if w not in FILLERS),
        }
        for t in turns
    ]
    lines.append({"type": "segment", "start": 0.0, "end": 0.5, "speaker": None, "track": "remote", "text": "oui"})
    (passage / "v1" / "transcript.jsonl").write_text("".join(json.dumps(line) + "\n" for line in lines))
    score([passage])
    said = capsys.readouterr().out
    assert "m1 v1 local: 0.0% missed or misheard, 0.0% written beyond" in said
    assert "m1 v1 remote: 0.0% missed or misheard" in said and "m1 v1 remote: 0.0% missed or misheard, 0.0%" not in said
    assert "median v1 local:" in said and "median v1 remote:" in said
