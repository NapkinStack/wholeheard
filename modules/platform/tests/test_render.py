"""The bench's renderer, tried with a voice played here: a tone as long as its words."""

import itertools
import json
import wave
from pathlib import Path

import numpy as np
import pytest

from bench.render import CONDITIONS, ECHO_DELAY, NOISE, RATE, VOICES, condition, lay_out, room, script, write_session

MEETING = Path(__file__).parent.parent / "bench" / "meeting.txt"


def tone(voice: str, text: str) -> np.ndarray:
    """A quarter of a second a word, at a pitch of the voice's own."""
    pitch = 200 + 100 * sorted(VOICES).index(voice)
    samples = np.arange(round(0.25 * len(text.split()) * RATE))
    return (0.3 * np.sin(2 * np.pi * pitch * samples / RATE)).astype(np.float32)


def test_the_script_is_a_meeting_of_two_sides_with_a_room_on_one_of_them():
    meeting = script(MEETING)
    sides = {voice: about["side"] for voice, about in meeting["voices"].items()}
    assert set(sides) == set(VOICES), "a voice of the script has no model to say it, or a model no voice"
    # Two people behind one microphone: what telling voices apart will be measured on.
    assert sorted(sides.values()) == ["local", "local", "remote", "remote"]
    turns = meeting["turns"]
    words = sum(len(turn["text"].split()) for turn in turns)
    assert len(turns) >= 50 and 600 <= words <= 1000, (
        f"{len(turns)} turns, {words} words: about five minutes is 600 to 900"
    )
    assert sum(turn["gap"] < 0 for turn in turns) >= 5, "a meeting where nobody ever speaks over another is not one"
    assert any(turn["gap"] >= 3 for turn in turns), "a meeting without a silence"


def test_a_turn_said_by_a_voice_the_script_does_not_name_is_refused(tmp_path):
    (tmp_path / "meeting.txt").write_text(
        "# a comment\nvoice claire local\n\nclaire 0.5 Bonjour.\nnobody 0 x\n", encoding="utf-8"
    )
    with pytest.raises(ValueError, match="line 5: a turn is said by 'nobody', a voice the script does not name"):
        script(tmp_path / "meeting.txt")


def test_each_turn_is_on_the_clock_where_the_script_puts_it_and_on_its_sides_track_only():
    meeting = script(MEETING)
    tracks, truth = lay_out(meeting, tone)
    assert [turn["text"] for turn in truth] == [turn["text"] for turn in meeting["turns"]]
    latest = 0.0
    for said, turn in zip(truth, meeting["turns"]):
        assert said["start"] == pytest.approx(max(0.0, latest + turn["gap"]), abs=0.002), said["text"]
        assert said["end"] - said["start"] == pytest.approx(0.25 * len(turn["text"].split()), abs=0.002)
        latest = max(latest, said["end"])
    # An overlap is two voices at once, never a turn moved to make room.
    assert sum(said["start"] < before["end"] for before, said in itertools.pairwise(truth)) >= 5
    # With a headset, a side's track holds its own voices and nothing of the other's.
    for said in truth:
        other = "remote" if said["side"] == "local" else "local"
        middle = round((said["start"] + said["end"]) / 2 * RATE)
        alone = not any(t["side"] == other and t["start"] <= middle / RATE <= t["end"] for t in truth)
        assert np.abs(tracks[said["side"]][middle - 100 : middle + 100]).max() > 0.1, said["text"]
        if alone:
            assert np.abs(tracks[other][middle - 100 : middle + 100]).max() == 0.0, said["text"]
    assert len(tracks["local"]) == len(tracks["remote"]) == round((latest + 1.0) * RATE)


def test_a_rendered_meeting_is_a_session_the_programs_read_with_its_truth_beside(tmp_path):
    tracks, truth = lay_out(script(MEETING), tone)
    write_session(tmp_path / "headset", tracks, truth)
    again = tmp_path / "again"
    write_session(again, *lay_out(script(MEETING), tone))
    for name in ("me.wav", "others.wav"):
        with wave.open(str(tmp_path / "headset" / name)) as wav:
            assert (wav.getframerate(), wav.getnchannels(), wav.getsampwidth()) == (16_000, 1, 2)
            assert wav.getnframes() == pytest.approx(len(tracks["local"]) / 3, abs=2)
        # Rendered twice, the same bytes: a count that moves is the programs', not the bench's.
        assert (tmp_path / "headset" / name).read_bytes() == (again / name).read_bytes()
    record = json.loads((tmp_path / "headset" / "session.json").read_text())
    assert record["contract"] == "session-audio/v1" and isinstance(record["stopped"], str)
    kept = json.loads((tmp_path / "headset" / "truth.json").read_text(encoding="utf-8"))["turns"]
    assert kept == truth and {"voice", "side", "start", "end", "text", "words"} == set(kept[0])
    # Each word has its instant, inside its turn and never before the word it follows.
    for turn in kept:
        instants = [when for _, when in turn["words"]]
        assert [word for word, _ in turn["words"]] == turn["text"].split()
        assert instants == sorted(instants) and turn["start"] <= instants[0] and instants[-1] <= turn["end"]


def level(sound: np.ndarray) -> float:
    return float(np.sqrt(np.mean(sound.astype(np.float64) ** 2)))


def test_on_loudspeakers_the_microphone_hears_the_remote_side_later_weaker_and_as_loud_as_measured():
    tracks, truth = lay_out(script(MEETING), tone)
    assert list(CONDITIONS) == ["headset", "low", "loud"]
    first = next(turn for turn in truth if turn["side"] == "remote")
    starts, ends = round(first["start"] * RATE), round(first["end"] * RATE)
    for name in ("low", "loud"):
        echo = room(tracks["remote"], name)
        assert 20 * np.log10(level(echo) / level(tracks["remote"])) == pytest.approx(CONDITIONS[name], abs=0.1)
        # Nothing of a remote turn is in the microphone before the loudspeakers' delay has passed.
        assert np.abs(echo[starts : starts + round(ECHO_DELAY * RATE) - 2]).max() < 1e-6
        assert np.abs(echo[starts + round(ECHO_DELAY * RATE) : starts + RATE // 2]).max() > 0.01
        # It rings on after the turn has ended, as a room does — and most of it came at once.
        ringing = level(echo[ends + round(0.06 * RATE) : ends + round(0.15 * RATE)])
        assert 1e-4 < ringing < 0.5 * level(echo[starts + RATE // 4 : ends])
    # Pushed hard, the loudspeakers do not play what they are sent: louder is not the same echo, louder.
    low, loud = room(tracks["remote"], "low"), room(tracks["remote"], "loud")
    assert not np.allclose(loud, low * (level(loud) / level(low)), atol=1e-3)


def test_a_condition_is_the_remote_as_played_and_the_microphone_with_its_own_noise():
    tracks, _ = lay_out(script(MEETING), tone)
    for name in CONDITIONS:
        heard = condition(tracks, name)
        # What the machine plays is the same whoever listens; only the microphone changes.
        assert np.array_equal(heard["remote"], tracks["remote"])
        echo = room(tracks["remote"], name) if CONDITIONS[name] is not None else 0
        noise = heard["local"] - tracks["local"] - echo
        assert 20 * np.log10(level(noise)) == pytest.approx(NOISE[name], abs=0.2)
        # The same draw is the same noise; another draw is another, at the same level.
        assert np.array_equal(condition(tracks, name)["local"], heard["local"])
        other = condition(tracks, name, 1)["local"]
        assert not np.array_equal(other, heard["local"]) and level(other) == pytest.approx(
            level(heard["local"]), rel=0.01
        )
    assert (
        level(condition(tracks, "loud")["local"])
        > level(condition(tracks, "low")["local"])
        > level(condition(tracks, "headset")["local"])
    )
