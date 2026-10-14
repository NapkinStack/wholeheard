"""Renders the bench's meeting: the tracks a recording of it would hold, and the truth beside.

    uv run python -m bench.render <voices-folder> <out-folder> [<draws> [v1|v2]]

Four listening conditions — `headset`, `low`, `loud`, `television`, see `CONDITIONS` — and for each, six
draws of the microphone's and the line's noise unless told otherwise: `<out-folder>/<condition>/<draw>`.

The voices are synthesised: nobody is recorded. `<voices-folder>` holds the public
models `VOICES` names, fetched once with `python -m piper.download_voices`.
"""

from __future__ import annotations

import hashlib
import itertools
import json
import struct
import sys
import wave
from collections.abc import Callable
from pathlib import Path

import numpy as np
from scipy.signal import fftconvolve, resample_poly

RATE = 48_000
"""What a laptop's sound devices deliver: the rate the meeting is made at."""

VOICES = {
    "claire": ("fr_FR-siwis-medium", 0),
    "marc": ("fr_FR-tom-medium", 0),
    "sophie": ("fr_FR-gilles-low", 0),
    "julien": ("fr_FR-upmc-medium", 1),
}
"""Which public model, and which of its speakers, says each voice of the script.

Chosen on a measure, 2026-10-13: each candidate said one voice's 192 words alone, with no
chance in the synthesis, and the engine wrote them back. Words wrong: tom 2, siwis 3, upmc's
second speaker 7, gilles 13 — and upmc's first speaker 43, the ten speakers tried of the
many-voiced `mls` model 61 to 163: a voice the engine cannot follow alone measures nothing
of a meeting."""

PACE = (1.2, 5.0)
"""Words a second a person says a sentence at, at the least and at the most. A voice outside
it is not a voice an engine is built for: the first `sophie`, upmc's first speaker, said
eight words a second — and most of what the bench first counted wrong on the remote side
was that."""

CONDITIONS = {"headset": None, "low": -10.0, "loud": -5.0, "television": None}
"""How loud the microphone hears what the loudspeakers play, in decibels against what was
played — or not at all, with a headset. Measured on the decider's laptop on 2026-10-13, one
minute of the bench's remote side played while Summit recorded: -9.9 dB at the volume of a
usual meeting, -5.0 dB near the maximum. `television`: earphones on, so no echo — and a
room that is not silent, see `TELEVISION`."""

ECHO_DELAY = 0.046
"""How late the microphone hears the loudspeakers, in seconds: 46 ms in both trials."""

ECHO_DIRECT = 0.8
"""The share of the echo's energy that arrives at once, the rest ringing: 86 % and 77 %."""

ECHO_RINGS = 0.2
"""The ringing's time constant, in seconds: 99 % of the echo is in by 284 and 406 ms."""

NOISE = {"headset": -50.0, "low": -36.0, "loud": -36.0, "television": -36.0}
"""What the microphone holds when nobody speaks, in decibels of full scale: -36 measured on
the laptop's own microphone in both trials; a headset's was not measured, and is supposed."""

LINE = -70.0
"""What a call's sound holds when nobody speaks, in decibels of full scale — supposed, not
measured: enough for each draw to be another, so that the remote side has a spread too."""

DRIVE = {"low": 1.0, "loud": 4.0}
"""How hard the loudspeakers are pushed: near their maximum they no longer play what they
are sent. The played and the heard agreed at 0.93 at a usual volume, 0.80 near the maximum."""

TELEVISION = (
    "Et maintenant la météo de demain sur l'ensemble du pays.",
    "Des nuages le matin au nord, quelques averses en Bretagne, du soleil l'après-midi.",
    "Les températures sont en hausse, vingt-deux degrés à Lyon, vingt-cinq à Marseille.",
    "Dans l'actualité sportive, le club local a gagné son match de samedi soir.",
    "Les supporters étaient nombreux à attendre les joueurs devant le stade.",
    "Restez avec nous, après la pause, le grand jeu du soir.",
)
"""What a television beside the laptop says, over and over, from the first second to the last:
a voice that is no participant's, in the turns and between them. Nobody says it in the
meeting — any of it written is written wrong."""

TELEVISION_VOICE = ("fr_FR-upmc-medium", 0)
"""Its voice: the public speaker the meeting refused for its pace, which a television may have."""

TELEVISION_LEVEL = -15.0
"""How loud the microphone hears the television, in decibels against the local voices while
they speak — supposed, not measured: a television beside a laptop, low enough to talk over."""

HUM = -45.0
"""The room's steady noise — a fan, a television's hiss — in decibels of full scale, louder
at low pitches as such noises are; supposed, not measured."""

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
        model, speaker = {**VOICES, "television": TELEVISION_VOICE}[voice]
        kept = cache / (hashlib.sha256(f"{model}/{speaker}/{text}".encode()).hexdigest()[:24] + ".npy")
        if kept.exists():
            return np.load(kept)
        if model not in loaded:
            loaded[model] = PiperVoice.load(voices / f"{model}.onnx")
        # No chance in the voice: the same sentence is the same sound on every machine.
        steady = SynthesisConfig(speaker_id=speaker, noise_scale=0.0, noise_w_scale=0.0, length_scale=1.15)
        chunks = list(loaded[model].synthesize(text, steady))
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
        pace = len(turn["text"].split()) / (end - start)
        if len(turn["text"].split()) >= 4 and not PACE[0] <= pace <= PACE[1]:
            raise ValueError(
                f"{turn['voice']} says {turn['text']!r} at {pace:.1f} words a second: not the pace of a person"
            )
        side = meeting["voices"][turn["voice"]]["side"]
        placed.append((side, round(start * RATE), sound))
        # A word's instant is where it falls in its turn by the letters before it: near enough
        # to say which words were spoken while the other side spoke.
        words, before = [], 0
        for word in turn["text"].split():
            words.append([word, round(start + (end - start) * before / len(turn["text"]), 2)])
            before += len(word) + 1
        about = {"voice": turn["voice"], "side": side, "start": round(start, 3), "end": round(end, 3)}
        truth.append({**about, "text": turn["text"], "words": words})
        cursor = max(cursor, end)
    length = round((cursor + 1.0) * RATE)
    tracks = {side: np.zeros(length, dtype=np.float32) for side in ("local", "remote")}
    for side, at, sound in placed:
        tracks[side][at : at + len(sound)] += sound
    return tracks, truth


def television(say: Synth, length: int) -> tuple[np.ndarray, list[dict]]:
    """The television's sound for a meeting of `length` samples, at the level it is said at,
    and what it said: each sentence with its instants and its words, as the truth's turns."""
    sound, said, sentences = np.zeros(length, dtype=np.float32), [], itertools.cycle(TELEVISION)
    at = 0
    while at < length:
        text = next(sentences)
        voice = say("television", text)
        if at + len(voice) > length:
            break  # A sentence it would not finish is not said: the truth holds whole sentences.
        sound[at : at + len(voice)] = voice
        start, end = at / RATE, (at + len(voice)) / RATE
        words, before = [], 0
        for word in text.split():
            words.append([word, round(start + (end - start) * before / len(text), 2)])
            before += len(word) + 1
        said.append(
            {
                "voice": "television",
                "side": "television",
                "start": round(start, 3),
                "end": round(end, 3),
                "text": text,
                "words": words,
            }
        )
        at += len(voice) + round(0.8 * RATE)
    return sound, said


def speaking(sound: np.ndarray) -> float:
    """How loud a track is while something is said on it: its level, its silences left out."""
    return float(np.sqrt(np.mean(sound[sound != 0].astype(np.float64) ** 2)))


def hum(length: int, seed: int) -> np.ndarray:
    """The room's steady noise at `HUM`: each pitch weaker as it rises, as a fan's is."""
    spectrum = np.fft.rfft(np.random.default_rng([seed, 1]).standard_normal(length))
    spectrum /= np.sqrt(np.maximum(np.arange(len(spectrum)), 1))
    sound = np.fft.irfft(spectrum, length)
    return (sound * 10 ** (HUM / 20) / np.sqrt(np.mean(sound**2))).astype(np.float32)


def in_the_room(tracks: dict[str, np.ndarray], seed: int) -> np.ndarray:
    """What the microphone hears of a room that is not silent: the television, `TELEVISION_LEVEL`
    under the local voices, and the room's hum — another draw of it for each seed."""
    louder = 10 ** (TELEVISION_LEVEL / 20) * speaking(tracks["local"]) / speaking(tracks["television"])
    return (tracks["television"] * louder + hum(len(tracks["television"]), seed)).astype(np.float32)


def room(played: np.ndarray, name: str) -> np.ndarray:
    """What a microphone hears of what the loudspeakers play: later, weaker, ringing — and,
    pushed hard, not quite what they were sent.

    A model set on one measure, not a room: a canceller may find it easier or harder than
    the laptop it was measured on. The bench compares; it does not promise.
    """
    pushed = np.tanh(played * DRIVE[name]) / DRIVE[name]
    after = round(ECHO_DELAY * RATE)
    tail = np.arange(round(ECHO_RINGS * RATE) * 4)
    ringing = np.random.default_rng(sum(map(ord, name))).standard_normal(len(tail)) * np.exp(
        -tail / (ECHO_RINGS * RATE)
    )
    ringing *= np.sqrt((1 - ECHO_DIRECT) / ECHO_DIRECT / np.sum(ringing**2))
    heard = fftconvolve(pushed, np.concatenate([np.zeros(after), [1.0], ringing]))[: len(played)]
    return (heard * 10 ** (CONDITIONS[name] / 20) * np.sqrt(np.mean(played**2) / np.mean(heard**2))).astype(np.float32)


def condition(tracks: dict[str, np.ndarray], name: str, seed: int = 0) -> dict[str, np.ndarray]:
    """The two tracks as that way of listening gives them: the remote as played, with the
    line's own noise; the local with what it heard of it, and the microphone's own noise —
    another draw of both for each seed, so that a count is read over several and not taken
    for more exact than it is."""
    draw = np.random.default_rng([seed, sum(map(ord, name))])
    noise = draw.standard_normal(len(tracks["local"]))
    local = tracks["local"] + (10 ** (NOISE[name] / 20) * noise).astype(np.float32)
    remote = tracks["remote"] + (10 ** (LINE / 20) * draw.standard_normal(len(tracks["remote"]))).astype(np.float32)
    if CONDITIONS[name] is not None:
        local = local + room(remote, name)
    if name == "television":
        local = local + in_the_room(tracks, seed)
    return {"local": local, "remote": remote}


def write_wav(path: Path, sound: np.ndarray, rate: int) -> None:
    """One channel, 16 bits: what `session-audio/v1` holds."""
    with wave.open(str(path), "wb") as wav:
        wav.setnchannels(1)
        wav.setsampwidth(2)
        wav.setframerate(rate)
        wav.writeframes((np.clip(sound, -1.0, 1.0) * 32767).astype("<i2").tobytes())


def write_truth(folder: Path, truth: list[dict], television: list[dict]) -> None:
    """The truth: each turn of the meeting — and, when one was on, what the television said."""
    kept = {"turns": truth, **({"television": television} if television else {})}
    (folder / "truth.json").write_text(json.dumps(kept, ensure_ascii=False, indent=1), encoding="utf-8")


def write_session(folder: Path, tracks: dict[str, np.ndarray], truth: list[dict], television: list[dict] = ()) -> None:
    """A session as the programs read one today (`session-audio/v1`), and the truth beside it."""
    folder.mkdir(parents=True, exist_ok=True)
    for name, side in (("me", "local"), ("others", "remote")):
        write_wav(folder / f"{name}.wav", resample_poly(tracks[side], 16_000, RATE), 16_000)
    record = {
        "contract": "session-audio/v1",
        "started": "2026-10-14T09:00:00+02:00",
        "stopped": "2026-10-14T09:05:00+02:00",
        "sources": {"me": {"device": "bench"}, "others": {"device": "bench"}},
    }
    (folder / "session.json").write_text(json.dumps(record), encoding="utf-8")
    write_truth(folder, truth, list(television))


STARTS = {"local": 0.064, "remote": 0.021}
"""When each source's first sound reaches a recorder, in seconds after the session starts:
two devices do not open at the same instant. As seen on the capture's first recordings."""

STOPS = (0.7, 2.0)
"""Two times the remote source stops delivering, in seconds: one too short for the capture
to write zeros — the clock alone says where the sound comes back — and one long enough that
it does, up to half a second before the sound returns."""


def as_delivered(track: np.ndarray, starts: float, stops: list[tuple[float, float]]) -> tuple[np.ndarray, list[dict]]:
    """A side as a recorder holds it, and what its clock says: the track opens when its
    source does, and is shorter than the meeting by what the source did not deliver —
    `stops`, each a moment it stopped at and for how long."""
    first = round(starts * RATE)
    pieces, clock, frames, cursor = [], [], 0, first
    for at, lasted in [*sorted(stops), (len(track) / RATE, 0.0)]:
        end = round(at * RATE)
        # A point where the sound starts, then every five seconds while it lasts.
        clock += [{"sample": frames + n, "at": round((cursor + n) / RATE, 4)} for n in range(0, end - cursor, 5 * RATE)]
        pieces.append(track[cursor:end])
        frames += end - cursor
        cursor = end + round(lasted * RATE)
        if lasted >= 1.5:
            # Behind for a second and a half, the capture writes zeros, to half a second before the sound.
            zeros = round((lasted - 0.5) * RATE)
            clock.append({"silence": [frames, frames + zeros]})
            pieces.append(np.zeros(zeros, dtype=track.dtype))
            frames += zeros
    return np.concatenate(pieces), clock


def write_track(path: Path, sound: np.ndarray) -> None:
    """A track as the capture writes one: two channels holding the same sound, 24 bits, the
    header's extensible form."""
    steps = (np.clip(sound, -1.0, 1.0) * 8_388_607).astype("<i4")
    data = np.repeat(steps.view(np.uint8).reshape(-1, 4)[:, :3], 2, axis=0).tobytes()
    form = struct.pack("<HHIIHHHHI", 0xFFFE, 2, RATE, RATE * 6, 6, 24, 22, 24, 3)
    form += bytes([1, 0, 0, 0, 0, 0, 0x10, 0, 0x80, 0, 0, 0xAA, 0, 0x38, 0x9B, 0x71])
    head = b"RIFF" + struct.pack("<I", 60 + len(data)) + b"WAVEfmt " + struct.pack("<I", len(form)) + form
    path.write_bytes(head + b"data" + struct.pack("<I", len(data)) + data)


def write_as_delivered(
    folder: Path, tracks: dict[str, np.ndarray], truth: list[dict], television: list[dict] = ()
) -> None:
    """The same session as a recording keeps it since ADR-0004 (`session-audio/v2`): each
    side as a laptop's devices deliver it, and the session's clock — which here has
    something to say: the two tracks do not start together, and the remote source stops
    twice, in the two longest moments nobody on that side speaks."""
    folder.mkdir(parents=True, exist_ok=True)
    said = sorted((turn["start"], turn["end"]) for turn in truth if turn["side"] == "remote")
    quiet = sorted(((later[0] - earlier[1], earlier[1]) for earlier, later in itertools.pairwise(said)), reverse=True)
    # Each stop in the middle of its quiet moment, the longer stop in the longer moment.
    stops = [
        (after + (length - lasted) / 2, lasted) for (length, after), lasted in zip(quiet, sorted(STOPS, reverse=True))
    ]
    if any(length < lasted + 0.4 for (length, _), lasted in zip(quiet, sorted(STOPS, reverse=True))):
        raise ValueError("the meeting has no moment long enough for the remote source to stop in")
    lines = []
    for side in ("local", "remote"):
        sound, clock = as_delivered(tracks[side], STARTS[side], stops if side == "remote" else [])
        write_track(folder / f"{side}.wav", sound)
        lines += [{"track": side, **line} for line in clock]
    record = {
        "contract": "session-audio/v2",
        "started": "2026-10-14T09:00:00+02:00",
        "stopped": "2026-10-14T09:05:00+02:00",
        "tracks": {"local": {"device": "bench"}, "remote": {"device": "bench"}},
    }
    (folder / "session.json").write_text(json.dumps(record), encoding="utf-8")
    (folder / "clock.jsonl").write_text("".join(json.dumps(line) + "\n" for line in lines), encoding="utf-8")
    write_truth(folder, truth, list(television))


def main(argv: list[str]) -> int:
    voices, out = Path(argv[1]), Path(argv[2])
    draws = int(argv[3]) if len(argv) > 3 else 6
    write = {"v1": write_session, "v2": write_as_delivered}[argv[4] if len(argv) > 4 else "v1"]
    meeting = script(Path(__file__).with_name("meeting.txt"))
    say = piper(voices, out / "cache")
    tracks, truth = lay_out(meeting, say)
    tracks["television"], said = television(say, len(tracks["local"]))
    for name in CONDITIONS:
        for seed in range(draws):
            write(out / name / str(seed), condition(tracks, name, seed), truth, said if name == "television" else [])
        print(f"{name}: {len(truth)} turns, {truth[-1]['end']:.0f} s, {draws} draws of the noise, in {out / name}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
