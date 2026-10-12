# Spikes

Throwaway probes of the first cycle. Their product is what they record in the cycle's file
(`docs/project/cycles/`), not this code: nothing here is a module, and nothing here ships.

| Probe | Deliverable | Question |
|---|---|---|
| [`d1-capture/`](d1-capture/) | D1, first half | Can one program write the microphone and the system's sound as two files, on the three systems, with the same code? |
| [`d1-shell/`](d1-shell/) | D1, second half | Does a web-view application get a window that renders, a tray icon, and a global shortcut that fires while another window has the focus? |
| [`d2-local-minutes/`](d2-local-minutes/) | D2 | Are all-local minutes of a French meeting worth keeping, on the decider's laptop? |

## Running `d2-local-minutes`

It needs a meeting of about an hour, in French, **recorded for the purpose with people who
agreed**, as one audio file; `ffmpeg`, `uv`, and `ollama serve` running.

```bash
uv run --with faster-whisper python3 spikes/d2-local-minutes/d2.py path/to/meeting.m4a \
    --reference path/to/human-transcript.txt   # optional: counts the words that differ
```

The same minutes from an online provider — any address that speaks the OpenAI protocol — with
or without a context the user attaches, on a transcript already made:

```bash
read -rs SUMMIT_PROVIDER_KEY && export SUMMIT_PROVIDER_KEY     # paste the key; it is written nowhere
uv run python3 spikes/d2-local-minutes/d2.py path/to/meeting.wav \
    --transcript path/to/transcript.txt --provider https://openrouter.ai/api/v1 \
    --model mistralai/mistral-medium-3.1 --context path/to/context.txt
```

**What is sent leaves the machine**: only a recording made for the purpose, or a public one.

| Found on the first run, Fedora 44 | What the probe does about it |
|---|---|
| The `whisper-cpp` package holds the library and no program | Transcribes with the Python engine `uv` fetches for the run; a `whisper-cli` on the path still serves |
| That engine's decoder breaks on its own audio library | Hands it the samples `ffmpeg` already converted |
| Its voice detector's library tries to send telemetry to its publisher | Nothing: recorded, to weigh when the product's engine is chosen |

**No meeting at hand?** The decider chose not to wait for one (2026-10-12). A public corpus
of French meetings serves instead: [SUMM-RE](https://huggingface.co/datasets/linagora/SUMM-RE),
CC BY-SA 4.0 — conversations of about twenty minutes, three or four speakers each on a track
of their own, with a human transcript to compare against. Its small example, five minutes of
a meeting that settles the date of an event, is what the probe was first run on; it lives
outside the repository, in the ignored `temp/` folder.

It downloads two models the first time and counts that as setup, transcribes, writes the
minutes, and prints the lines the cycle records. The count that decides — actions missing,
wrong or invented — is a human's, read against what was really agreed.

## Running `d1-shell`

```bash
cd spikes/d1-shell
cargo run --release
```

A window opens and says what the system gave it. Nothing is recorded. It needs no Node: its
page is one static file.

On a Linux desktop the portal gives a global shortcut only to an application it can name.
Install the probe's desktop entry once, and remove it when the spike is over:

```bash
# the entry must name the program by its full path: the desktop discards one it cannot find
sed "s|^Exec=.*|Exec=$PWD/target/release/d1-shell|" dev.summit.d1shell.desktop \
  > ~/.local/share/applications/dev.summit.d1shell.desktop
rm ~/.local/share/applications/dev.summit.d1shell.desktop    # afterwards
```

## Running `d1-capture`

During a real call, with people who know they are recorded:

```bash
cd spikes/d1-capture
cargo run --release -- 20 /tmp --tone
```

`--tone` plays a quiet note on the default output for the whole run, so that silence in the
system's sound cannot mean that nothing was playing.

It records for 20 seconds into `/tmp/me.wav` and `/tmp/others.wav`, then says for each
whether it holds sound or silence. **Delete the two files afterwards**: no recording of a
real meeting enters this repository (`docs/project/rules/security.md`, rule 2).
