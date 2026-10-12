#!/usr/bin/env python3
"""D2 — a throwaway probe, not product code.

The question: are all-local minutes of a French meeting worth keeping, on the decider's laptop?

It takes one audio file — a meeting of about an hour, recorded for the purpose with people who
agreed — transcribes it with a local Whisper engine, asks a local model for the minutes, and
prints what the cycle asks to be recorded: the setup time, the processing time against the
meeting's length, and the minutes themselves, for a human to count the actions that are
missing, wrong or invented. Nothing leaves the machine except the two model downloads.

    python3 d2.py MEETING.(wav|mp3|m4a|…) [--model mistral] [--whisper large-v3-turbo]

Needs on the path: ffmpeg, whisper-cli (package whisper-cpp), and a running `ollama serve`.
The audio and everything made from it stay in a folder beside the audio file: delete it
afterwards, and never commit it (docs/project/rules/security.md, rule 2).
"""

from __future__ import annotations

import argparse
import json
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


def minutes(clock_time: float) -> str:
    return f"{int(clock_time // 60)} min {int(clock_time % 60):02d} s"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("audio", type=Path)
    parser.add_argument("--model", default="mistral", help="the local model that writes the minutes (ollama)")
    parser.add_argument("--whisper", default="large-v3-turbo", help="the Whisper model that transcribes")
    args = parser.parse_args()

    whisper = shutil.which("whisper-cli") or shutil.which("whisper-cpp")
    for name, found in (("ffmpeg", shutil.which("ffmpeg")), ("whisper-cli", whisper)):
        if not found:
            sys.exit(f"FAIL [d2] {name} is not on the path.\n      Action: sudo dnf install "
                     f"{'whisper-cpp' if name == 'whisper-cli' else name}")
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
    if not weights.is_file():
        print(f"Setup       : downloading the transcription model {args.whisper} …", flush=True)
        setup += clock("download", ["curl", "-L", "--fail", "--progress-bar", "-o", str(weights),
                                    WHISPER_MODELS.format(name=args.whisper)])
    if args.model.split(":")[0] not in have:
        print(f"Setup       : downloading the minutes model {args.model} …", flush=True)
        setup += clock("pull", ["ollama", "pull", args.model])

    print("Transcribing: this is the long step …", flush=True)
    wav = work / "audio-16k.wav"
    convert = clock("ffmpeg", ["ffmpeg", "-y", "-loglevel", "error", "-i", str(args.audio), "-ar", "16000", "-ac", "1", str(wav)])
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
    print(f"""
D2 — to paste into the cycle
  Machine            : this laptop, models {args.whisper} and {args.model}
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
