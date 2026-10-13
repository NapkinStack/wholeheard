# capture — local instructions

> Only what is **specific to this module**.
> Never duplicate a kernel rule (`/AGENTS.md`): that is wasted context and a source of
> divergence.
>
> Indicative budget: 100 lines.

## Responsibility

Records the audio sources of a conversation into a session.

## What this module does not do

- No tray, no shortcut, no window, no reminder to announce the recording: the `shell` module.
- No transcript and no conversion of a file from elsewhere: `transcription`.
- It never reads or writes `context.txt`.

## Internal conventions nobody could guess

- `src/lib.rs` knows no audio device and holds everything the contract promises of a file;
  `src/main.rs` holds the devices, the clock and the signals. A promise of the contract is
  tested in `tests/session_audio.rs`, on sound made there.
- `src/v2.rs` is the same for `session-audio/v2` — tracks as delivered and the clock
  (ADR-0004) — tested in `tests/session_audio_v2.rs`. The program writes it on `--as v2`.
- A point of the clock is the instant the system handed a piece of sound over, not the one
  it says the sound was captured at: measured on 2026-10-13, that one moved by whole periods
  of 21 ms from a piece to the next, and was wrong for the first.
- An output is opened as an input: what it delivers is what it plays.
- The host is chosen by what the system offers, never by the system's name: no code written
  for one system (ADR-0002's success criterion).

## Business invariants

- What was heard is on disk when `Track::push` returns: no buffer between a sample and the file.
- A sample once written never changes; the header's lengths are true only after a clean stop.
- First form (`record`): the two files are the same length, on the same clock — a source that
  gives nothing is zeros; and silence is zeros — what a source rests at is taken out before
  anything is written or measured.
- Second form (`record … --as v2`, `src/v2.rs`): a track is what its source delivered, to the
  sample — nothing taken out, no channel mixed, no length made equal; the rest is taken out
  of the *measure* only. Zeros written here are named on the clock before they are written.

## Known traps

- A microphone rested at a third of full scale and opened on a click at full scale (cycle 01,
  D1): measured from zero, a silent room is sound. Tested.
- First form: the channels of a source are averaged — a source that speaks on one channel of
  two is written at half its level, on one of six at a sixth (the verifier's measure,
  2026-10-12). Not handled there; the second form keeps every channel.
- An output that plays nothing delivers zeros with no error; one that moved elsewhere may
  deliver nothing at all. Both end as the `silent` event.

## Areas not to change without approval

- The format of the files and of `session.json`: it is `contracts/session-audio/v1`, and `v2`
  with `--as v2`; the events said while recording: `contracts/recording/v1`.

## Non-standard commands

None.
