# transcription — local instructions

> Only what is **specific to this module**.
> Never duplicate a kernel rule (`/AGENTS.md`): that is wasted context and a source of
> divergence.
>
> Indicative budget: 100 lines.

## Responsibility

Turns the audio of a session into a timestamped transcript.

## What this module does not do

- It records nothing: the audio files are `capture`'s, read as `session-audio/v1` describes them.
- It says who spoke only as the two files do — `me`, `others`. Telling several voices of one file apart is not done here.
- It fetches no model and contacts no host: the model file is the caller's to give.

## Internal conventions nobody could guess

- `src/lib.rs` writes and takes up the transcript and knows no engine; `src/step.rs` is the step every engine shares; an engine is one file behind `step::Engine` — today `src/whisper.rs`. Another system's engine is another file, never a branch inside the step (ADR-0003).
- The step's tests play an engine that "hears" a segment wherever the samples are not zero: no model, no voice of anyone in the repository.

## Business invariants

- `transcript.jsonl` only grows, and is true up to its last line: a step killed anywhere is run again and gives, byte for byte, the transcript of a run never stopped.
- A transcript another engine, model, language or context made is never continued.
- Whatever is refused, the engine is asked nothing and no transcript is created or changed.
- `complete` is written only when the session's record says `stopped`.

## Known traps

- Whisper finds words in silence: a span of pure silence is never given to an engine.
- A guard against one defect cut another case twice: hearing a last sentence again, then not crawling on an unbroken voice. Any change to the loop of `transcribe` is tried with random sessions against one huge span, not with a few placed stretches.
- whisper.cpp trusts its model file: one cut after its first bytes crashes the program, one missing its last megabytes is used and writes nonsense. Whoever fetches a model checks it whole before this program sees it.
- Hearing thirty seconds at a time cost eight points of words on the trial meeting: the span is long on purpose (`SPAN` in `src/main.rs`).

## Areas not to change without approval

- The lines of `transcript.jsonl`: they are `contracts/transcript/v1`, read by `summary`.

## Non-standard commands

- The graphics card: `cargo build --locked --release --features vulkan`, where the system has Vulkan and its headers. The standard verbs build for the processor.