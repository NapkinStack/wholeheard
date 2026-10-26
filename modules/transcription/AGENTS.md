# transcription — local instructions

> Only what is **specific to this module**.
> Never duplicate a kernel rule (`/AGENTS.md`): that is wasted context and a source of
> divergence.
>
> Indicative budget: 100 lines.

## Responsibility

Turns the audio of a session into a timestamped transcript.

## What this module does not do

- It records nothing: the audio files are `capture`'s, read as `session-audio/v1` or `v2` describes them.
- `src/tracks.rs` brings a track of `session-audio/v2` to what an engine hears, in memory: the
  track is never written to (ADR-0004). The rate is changed by a library that does only that —
  a first conversion written here mirrored a voice's high notes into it at 44,100 and 8,000
  samples a second (the first verification of this reader, 2026-10-13). Nothing else is done
  to the sound: a source's rest is left where it is, and whether a span is silence is the
  step's to measure — a reader that centred the sound failed two verifications. It places
  the track on the session's clock by every point of `clock.jsonl`; where a source delivered
  nothing — zeros the clock names, an instant it leaves empty — what is heard goes in a line
  from where the source left to where it came back, never down to zero and up again: a step
  the source did not make rings in what the engine hears. The step reads both
  forms and measures silence by the form: the first is centred, and silence is within a
  thousandth of zero, as it always was; in the second, where a span rests is taken out of the
  measure — never of the sound — by one pole at 20 Hz, as the capture does for its own watch,
  run once from the track's first sample to its last: restarted at every span, it took each
  span's first sample for its rest and heard a quiet hum at twice its size.
  One measure for both, a line from a span's first sample to its last, failed its
  verification: it changed what a first-form session gives, and was twice as strict away
  from zero.
- It says who spoke only as the two files do — `me`, `others`, and each segment's `track`, `local` or `remote`. Telling several voices of one file apart is not done here: a segment's `voice` is written `null`, its place kept (ADR-0004, rule 6).
- It fetches no model: the model file is the caller's to give. It contacts no host but the service named with `--provider` (D43, ADR-0013), sending each span there; the key comes from `WHOLEHEARD_PROVIDER_KEY`, written nowhere, never logged.
- It writes nothing beside the transcript but `derived/`: what the engine heard, made from the tracks, never the tracks themselves (ADR-0004, rule 4).

## Internal conventions nobody could guess

- `src/lib.rs` writes and takes up the transcript and knows no engine; `src/step.rs` is the step every engine shares; an engine is one file behind `step::Engine` — today `src/whisper.rs`. Another system's engine is another file, never a branch inside the step (ADR-0003).
- The step's tests play an engine that "hears" a segment wherever the samples are not zero: no model, no voice of anyone in the repository.

## Business invariants

- `transcript.jsonl` only grows, and is true up to its last line: a step killed anywhere is run again and gives, byte for byte, the transcript of a run never stopped.
- A transcript another engine, model, language or context made is never continued.
- Whatever is refused, the engine is asked nothing and no transcript is created or changed.
- `complete` is written only when the session's record says `stopped`.
- A log line holds lengths, counts, durations, the echo and failures — never a word heard, the
  context, nor a session's path beyond its folder's name (`tests/log.rs`, `tests/program.rs`).

## Known traps

- An echo canceller run on a microphone that holds no echo can only damage it: `src/echo.rs` measures first, by the canceller's own estimate, and the step uses its work only from 3 dB taken out. White noise is no voice to test it with: it keeps a fifth of a noise's energy where it keeps nine tenths of a voiced sound.

- Whisper finds words in silence: a span of pure silence is never given to an engine.
- What it still writes on a side's silence inside a span (*Sous-titrage …*) is the engine's, accepted by the decider on 2026-10-24 (#355): no list of phrases, no threshold — the levers are refuted in cycle 02's D10.
- `unwritten` measures a tenth's level with the microphone's offset: a built-in microphone resting far from zero has its rest heard again, and a first word comes out twice. Accepted by the decider on 2026-10-24 (#362); the offset-free level was tried and moved the engine's inventions ("Það er hann.") — read on the bench before trying it again.
- A guard against one defect cut another case twice: hearing a last sentence again, then not crawling on an unbroken voice. Any change to the loop of `transcribe` is tried with random sessions against one huge span, not with a few placed stretches.
- whisper.cpp trusts its model file: one cut after its first bytes crashes the program, one missing its last megabytes is used and writes nonsense. Whoever fetches a model checks it whole before this program sees it.
- Whisper starts a segment where it starts a window, not where a turn starts: one exchange in four out of order on the bench. A segment is cut where its words stop, each word timed by the model's alignment heads (`src/whisper.rs`). The engine's own word times, without the heads, leave one in five out of order: they are no fallback for the order.
- Hearing thirty seconds at a time cost eight points of words on the trial meeting: the span is long on purpose (`SPAN` in `src/main.rs`). Cut where the sound is quietest, thirty-second pieces still cost seven (cycle 02, D10): they found the turns the engine jumps over on the bench, and lost the short replies of a real meeting. A bench of read sentences cannot decide how the engine hears; the trial meeting is heard too.
- Whisper sometimes jumps from one sentence to a later one, over a whole turn whose sound is intact — beam search too, elsewhere. What it leaves unwritten at the speaker's level is heard again alone (`unwritten` in `src/whisper.rs`), with any stray word it wrote alone in the turn — kept, that word cut a name out of it — and what a stretch heard alone leaves unwritten is heard again in turn, twice deep: the engine jumps again from where the stretch starts; a television in the room sits below that level on the bench, and is not.
- whisper.cpp's `no_context` clears the text it hears after once per call: within a span, each thirty seconds is heard after the text of the thirty before, and a silent track can loop on it. What loops is heard again alone (`looped` in `src/whisper.rs`). Cutting that text out everywhere (`n_max_text_ctx`) cuts out `context.txt` with it: four points of words on the trial meeting. A stricter repetition guard of the engine (`entropy_thold` 3.0) broke the loops and wrote nonsense on clean draws, even with five samples to choose from (cycle 02, D10).

## Areas not to change without approval

- The lines of `transcript.jsonl`: they are `contracts/transcript/v1`, read by `summary`.

## Non-standard commands

- The graphics card: `cargo build --locked --release --features vulkan`, where the system has Vulkan and its headers. The standard verbs build for the processor.