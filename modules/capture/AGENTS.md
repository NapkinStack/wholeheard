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
  for one system (ADR-0002's success criterion). `src/passive.rs` and `src/call.rs` are the
  exceptions the ADRs name (ADR-0011, ADR-0015), and the first is the only file that knows the
  `pipewire` crate.
- `src/call.rs` decides from the graph, nothing else; `Following` in `src/main.rs` applies it, polled
  by the main loop, closing before it opens. `Me` and `Step` are tested in `tests/call.rs`.

## Business invariants

- A `bluez5.loopback=true` source, or a filter fed by one, is never opened actively: `me` is
  passive on it (`call::passive_value`, by the running PipeWire's version: `in` to 1.6, `in-follow`
  from 1.7) while a call holds it, and has no stream otherwise (ADR-0015). A Bluetooth address
  reaches no log, event or session: `call::without_address`.
- The recording's loop never waits on the system: `Following` opens on a thread and polls.

- What was heard is on disk when `Track::push` returns: no buffer between a sample and the file.
- A sample once written never changes; the header's lengths are true only after a clean stop.
- First form (`record`): the two files are the same length, on the same clock — a source that
  gives nothing is zeros; and silence is zeros — what a source rests at is taken out before
  anything is written or measured.
- Second form (`record … --as v2`, `src/v2.rs`): a track is what its source delivered, to the
  sample — nothing taken out, no channel mixed, no length made equal; the rest is taken out
  of the *measure* only. Zeros written here are named on the clock before they are written.
- A log line holds events, devices, rates and failures — never a sound, nor a session's path
  beyond its folder's name (`tests/program.rs`, `scripts/e2e.sh`).

## Known traps

- A system default that becomes a headset's moves the open cpal stream there, actively, until a
  reading of the graph closes it: a second between readings, plus `pw-dump`, up to two, against the
  script's 500 ms. The switch can fire; said in the README, no fast path.
- A passive stream on a source that does not run links and negotiates nothing: no form, no track,
  until a call runs it. Never read that as a failure.
- What a desktop's graph must not be put through — a stuck session manager, a source's state with
  nothing else on it — is `scripts/private_graph.sh`, safe beside a call: a PipeWire of its own.
- `--as v1` does not follow a call's microphone: it refuses a headset's default at its start only.

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
