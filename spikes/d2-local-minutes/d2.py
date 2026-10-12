#!/usr/bin/env python3
"""D2 — a throwaway probe, not product code.

The question: are all-local minutes of a French meeting worth keeping, on the decider's laptop?

It takes one audio file — a meeting of about an hour, recorded for the purpose with people who
agreed — transcribes it with a local Whisper engine, asks a local model for the minutes, and
prints what the cycle asks to be recorded: the setup time, the processing time against the
meeting's length, and the minutes themselves, for a human to count the actions that are
missing, wrong or invented. Nothing leaves the machine except the two model downloads.

    uv run --with faster-whisper python3 d2.py MEETING.(wav|mp3|m4a|…) \
        [--model mistral] [--whisper large-v3-turbo] [--reference TRANSCRIPT.txt]

Needs: ffmpeg on the path, a running `ollama serve`, and a local Whisper engine — the Python
one named above, which `uv` fetches for this run, or a `whisper-cli` on the path. Fedora's
`whisper-cpp` package ships the library and no program: found on the first run, 2026-10-12.
`--reference` compares the transcript with a human one, word for word.
The audio and everything made from it stay in a folder beside the audio file: delete it
afterwards, and never commit it (docs/project/rules/security.md, rule 2).
"""

from __future__ import annotations

import argparse
import json
import re
import shutil
import subprocess
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

OLLAMA = "http://127.0.0.1:11434"
WHISPER_MODELS = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-{name}.bin"
CONTEXT = 32768  # tokens given to the model: its default would cut an hour of talk in silence
PROMPT = """Tu rédiges le compte rendu d'une réunion à partir de sa transcription.
Règles : n'écris que ce qui est dit dans la transcription ; n'invente ni nom, ni date, ni décision ;
si une information manque, écris « non précisé ».

Format, dans cet ordre :
## Points forts
(3 à 6 puces)
## Actions
(une puce par action : ce qui doit être fait — responsable — échéance)
## Points clés
(les sujets abordés, une puce chacun)

Transcription :
"""


def clock(label: str, command: list[str], **kwargs) -> float:
    start = time.monotonic()
    done = subprocess.run(command, **kwargs)
    spent = time.monotonic() - start
    if done.returncode:
        sys.exit(f"FAIL [{label}] exit code {done.returncode}: {' '.join(command[:4])} …")
    return spent


def seconds_of(audio: Path) -> float:
    out = subprocess.run(["ffprobe", "-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", str(audio)],
                         capture_output=True, text=True)
    return float(out.stdout.strip() or 0)


def ollama(path: str, body: dict | None = None, timeout: int = 3600) -> dict:
    request = urllib.request.Request(OLLAMA + path, data=json.dumps(body).encode() if body is not None else None,
                                     headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(request, timeout=timeout) as response:
        return json.load(response)


def samples_of(wav: Path):
    """The converted file as numbers. The engine's own decoder broke on its audio library, 2026-10-12."""
    import wave

    import numpy

    with wave.open(str(wav), "rb") as audio:  # 16 kHz, one channel, 16 bits: what ffmpeg wrote
        return numpy.frombuffer(audio.readframes(audio.getnframes()), dtype=numpy.int16).astype(numpy.float32) / 32768


def words_of(text: str) -> list[str]:
    """Words only, lower case: timestamps, speakers' numbers and punctuation are not speech."""
    text = re.sub(r"\[\d\d:\d\d\](\s*\d+:)?", " ", text).lower().replace("_", " ").replace("+", " ")
    return re.findall(r"[a-zàâäçéèêëîïôöùûüÿœæ0-9]+", text)


def word_errors(reference: str, heard: str) -> float:
    """Word error rate, in per cent: the edits that turn what was heard into the reference."""
    ref, hyp = words_of(reference), words_of(heard)
    row = list(range(len(hyp) + 1))
    for i, a in enumerate(ref, 1):
        previous, row[0] = row[0], i
        for j, b in enumerate(hyp, 1):
            previous, row[j] = row[j], min(row[j] + 1, row[j - 1] + 1, previous + (a != b))
    return 100 * row[-1] / max(len(ref), 1)


def minutes(clock_time: float) -> str:
    return f"{int(clock_time // 60)} min {int(clock_time % 60):02d} s"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("audio", type=Path)
    parser.add_argument("--model", default="mistral", help="the local model that writes the minutes (ollama)")
    parser.add_argument("--whisper", default="large-v3-turbo", help="the Whisper model that transcribes")
    parser.add_argument("--reference", type=Path, help="a human transcript of the same audio, to compare with")
    args = parser.parse_args()

    whisper = shutil.which("whisper-cli") or shutil.which("whisper-cpp")
    try:
        import faster_whisper  # noqa: F401 — present when run through `uv run --with faster-whisper`
        engine = "faster-whisper"
    except ImportError:
        engine = "whisper-cli" if whisper else None
    if not shutil.which("ffmpeg"):
        sys.exit("FAIL [d2] ffmpeg is not on the path.\n      Action: sudo dnf install ffmpeg")
    if engine is None:
        sys.exit("FAIL [d2] no local Whisper engine: neither the Python one nor a whisper-cli program.\n"
                 "      Action: run this script as  uv run --with faster-whisper python3 "
                 f"{Path(__file__).name} …  — the distribution's whisper-cpp package holds the library only.")
    try:
        have = {model["name"].split(":")[0] for model in ollama("/api/tags", timeout=5).get("models", [])}
    except (urllib.error.URLError, OSError):
        sys.exit("FAIL [d2] no local model server answers.\n      Action: run `ollama serve` in another terminal.")
    if not args.audio.is_file():
        sys.exit(f"FAIL [d2] {args.audio} is not a file.")

    work = args.audio.resolve().parent / f"{args.audio.stem}-d2"
    work.mkdir(exist_ok=True)
    length = seconds_of(args.audio)
    setup = 0.0

    weights = work.parent / f"ggml-{args.whisper}.bin"
    if engine == "whisper-cli" and not weights.is_file():
        print(f"Setup       : downloading the transcription model {args.whisper} …", flush=True)
        setup += clock("download", ["curl", "-L", "--fail", "--progress-bar", "-o", str(weights),
                                    WHISPER_MODELS.format(name=args.whisper)])
    if args.model.split(":")[0] not in have:
        print(f"Setup       : downloading the minutes model {args.model} …", flush=True)
        setup += clock("pull", ["ollama", "pull", args.model])

    print("Transcribing: this is the long step …", flush=True)
    wav = work / "audio-16k.wav"
    convert = clock("ffmpeg", ["ffmpeg", "-y", "-loglevel", "error", "-i", str(args.audio), "-ar", "16000", "-ac", "1", str(wav)])
    if engine == "faster-whisper":
        from faster_whisper import WhisperModel

        start = time.monotonic()
        model = WhisperModel(args.whisper, device="cpu", compute_type="int8")  # fetched the first time
        loaded = time.monotonic() - start
        start = time.monotonic()
        segments, _ = model.transcribe(samples_of(wav), language="fr", vad_filter=True)
        lines = [f"[{int(s.start // 60):02d}:{int(s.start % 60):02d}] {s.text.strip()}" for s in segments]
        transcribe = time.monotonic() - start
        setup += loaded  # the first load includes the download; later ones are seconds
        (work / "transcript.txt").write_text("\n".join(lines), encoding="utf-8")
    else:
        transcribe = clock("whisper", [whisper, "-m", str(weights), "-f", str(wav), "-l", "fr", "-otxt",
                                       "-of", str(work / "transcript"), "-np"], stdout=subprocess.DEVNULL)
    transcript = (work / "transcript.txt").read_text(encoding="utf-8").strip()

    print("Summarising : …", flush=True)
    start = time.monotonic()
    answer = ollama("/api/generate", {"model": args.model, "prompt": PROMPT + transcript, "stream": False,
                                      "options": {"num_ctx": CONTEXT, "temperature": 0.2}})
    summarise = time.monotonic() - start
    (work / "minutes.md").write_text(answer.get("response", ""), encoding="utf-8")

    read = answer.get("prompt_eval_count", 0)
    words = len(transcript.split())
    # A French word is a little more than one token; far fewer tokens read than words written
    # means the model saw only the beginning — the silent failure the idea file's R7 forbids.
    cut = read and read < words
    processing = convert + transcribe + summarise
    accuracy = "no reference given"
    if args.reference and args.reference.is_file():
        accuracy = f"{word_errors(args.reference.read_text(encoding='utf-8'), transcript):.0f} % of the words differ (substituted, missing or added)"
    print(f"""
D2 — to paste into the cycle
  Machine            : this laptop, engine {engine}, models {args.whisper} and {args.model}
  Against a human    : {accuracy}
  Meeting            : {minutes(length)} of audio, {words} words transcribed
  Setup (downloads)  : {minutes(setup) if setup else "nothing to download this time"}   threshold: under 15 min, packages included
  Transcription      : {minutes(convert + transcribe)}
  Minutes            : {minutes(summarise)}
  Processing, total  : {minutes(processing)} = {processing / length * 100 if length else 0:.0f} % of the meeting's length   threshold: under 50 %
  Read by the model  : {read} tokens for {words} words{"  ← THE TRANSCRIPT WAS CUT: the minutes cover its beginning only" if cut else ""}
  Minutes written to : {work / "minutes.md"}
  Transcript         : {work / "transcript.txt"}

Left to a human, and it is the criterion that matters: read the minutes against what was
really agreed in the meeting, and count the actions that are missing, wrong or invented.
Threshold: at most one in five. Then delete {work} and {weights.name} if you no longer need it.""")


if __name__ == "__main__":
    main()
