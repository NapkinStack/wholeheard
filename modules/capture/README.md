# capture

Records the audio sources of a conversation into a session.

- **Owner**: NapkinStack/maintainers
- **Criticality**: standard
- **Manifest**: [MANIFEST.yaml](./MANIFEST.yaml)

```mermaid
flowchart LR
    M["The microphone"]:::src --> P["wholeheard-capture<br/>a program, no window"]:::mod
    O["What the system plays"]:::src --> P
    P -->|"session-audio/v2"| F["The session's folder<br/>session.json · local.wav · remote.wav · clock.jsonl"]:::file
    P -.->|"one line per event"| W["Whoever started it:<br/>the shell, a terminal"]:::user

    classDef src fill:#374151,color:#fff
    classDef mod fill:#065f46,color:#fff
    classDef file fill:#1e3a8a,color:#fff
    classDef user fill:#92400e,color:#fff
```

**Legend** — grey: the two sources · green: this module · blue: what it provides, the
contract · orange: who reads its events. It consumes no contract.

## Getting started

```bash
nstack bootstrap capture     # the toolchain's components and, on Linux, the sound headers
nstack check capture         # format and lint
nstack test capture          # no audio device needed, nobody recorded
nstack run capture           # lists the devices the system offers
nstack e2e capture           # records two notes on devices made for the run: nobody is recorded
```

Recording — **the people recorded know it**:

```bash
cargo run --release -- record <sessions-folder>   # until Ctrl-C, a TERM signal, or "stop" on its input
cargo run --release -- record <sessions-folder> --as v1   # the same, in the first form
```

**`nstack e2e capture`** runs the scenarios without a call and without a voice: for about
three minutes the system's default output and input are two silent devices of its own, a note
is played into each, and the program records them in both forms — stopped as a user stops it,
read while it writes, left silent, killed, and with the same sound in both sources to see that
the tracks keep one clock. The second form's notes rest away from zero and carry two unequal
channels in 24 bits, so that a track centred, mixed or cut shows; and one source stops and
comes back. It runs nothing where there is no sound server, or while an application holds a
microphone; what it saw is in `.evidence/e2e.txt`, its logs in `$NSTACK_EVIDENCE/logs`
(`.evidence/e2e/logs` when run alone), where nstack reads them after the run: a warning or an
error the manifest's `evidence.provoked` does not declare is said. A real call, a real microphone and a
headset stay a person's to try.

## What it says while it runs

One JSON object per line on its output; errors go to the error stream.

| `event` | When | Carries |
|---|---|---|
| `started` | The two files exist and grow | `folder`, `sources` |
| `unavailable` | A source could not be opened, the other is recorded: it has no file at all — with `--as v1`, it is written as silence | `source`, `reason` |
| `silent` | A source has given no sound for ten seconds | `source` |
| `heard` | A source told silent gives sound again | `source` |
| `stopped` | The files are finished and `session.json` says so | `folder`, `seconds` |

A source is named for the device the system's default stands for when the recording starts — read from PipeWire's metadata — so that a microphone other than the call's shows at once; `wholeheard-capture devices` prints it as `stands_for`. Unread, the default's own name.

A recording follows the user's devices (D31): when the default changes — another chosen, a headset
unplugged — the system moves the stream, and the track goes on in the form it opened with, the
system converting the new device's sound to it. The program says `device` with the new one's name,
or `null` when the system names none, and `session.json` keeps each change with when
(`recording/v1`, `session-audio/v2`).

The local side records the microphone the call holds (D66, ADR-0015), on Linux where PipeWire's tools
answer, read from the same reading of the graph as below:

| In the graph | `me` records | Said |
|---|---|---|
| An application other than Wholeheard holds a source, not simply the default | That source, under the track's form; of several, the one running, then the oldest stream | `device`, with its description |
| That source is a headset's virtual input (`bluez5.loopback=true`), or a filter fed by one | The same, on a passive stream (`src/passive.rs`, its `node.passive` the one the running PipeWire reads as such: `call::passive_value`): it never makes the source run, so never asks for the headset's profile; a stream linked to a source that does not run receives no form, holds no track and is not named | `device`, once it received its form |
| Nobody, and another default input | The default, as above | As above |
| Nobody, and the default is a headset's | Nothing: no stream at all, until a call takes it | `unavailable` at the start; later, `device: null`, if a device was said |

A track that waits from the start has no file until it is taken, in the form then negotiated, opening on
zeros the clock names; never taken, it has no track, as a source that could not be opened. Taken back, it
goes on in its form, the system converting; the gap is zeros the clock names. A graph that does not read
leaves what is open as it is; at the start, a default named `bluez_…`, or not named at all, waits and any
other opens. `--as v1` does not follow a call's microphone, and records none with a headset's default; its `others`
still follows the call's output. macOS and Windows: the default, as above.

`device` on `me` says what is being recorded now: a stream is named when it has a track, that is, when it
has received its form, and `session.json` holds a track gained before `device` names it. A name said is
closed by the next, when another stream takes over at once, and otherwise by `device: null`; `stopped`
closes the last. A stream that never received a form has said nothing, so it owes no `null`.

**What this does not prevent.** A default input that *becomes* a headset's during a recording is moved there
by the system, on capture's active stream, until a reading of the graph closes it: up to a second between
readings, plus what `pw-dump` takes, up to two. That can pass the 500 ms of WirePlumber's script: the profile
can switch, and a call be cut, by that one move. No fast path is built; the cost is known and named here.

The other side follows the call (D41, ADR-0011), where PipeWire's tools answer: once a second capture
reads the graph (`pw-dump`); when an application other than capture holds a microphone and all its
sound plays on one output that is not the default — earbuds a call picked itself —, the remote stream
is moved there by its `target.object` (`pw-metadata`), said `device` with that output's name and
kept in `session.json`; when the call leaves it, back to the default. No call, a call on the
default or on several outputs: the default, as above. A graph that does not read, or a stream that
cannot be moved, leaves the default, logged once (`capture.not_followed`).

What it does is logged too — each event but the levels, each source opened with its device,
rate and channels, a gap of zeros written for a mute source, a stream's error, a failure with its
detail; never a sound, nor a session's path (ADR-0007): one JSON line per step in
`$XDG_STATE_HOME/wholeheard/logs/capture.<date>.jsonl` (UTC), fourteen days kept and the folder
under 50 MB. `WHOLEHEARD_LOG_DIR`, an absolute path, puts them elsewhere; `WHOLEHEARD_LOG=debug`
asks for more; `WHOLEHEARD_TRACE` is the id of the making the shell passes.

These lines are a contract: [`recording/v1`](../../contracts/recording/v1/README.md), which the shell reads. They are the same lines in both forms, and still name the sources `me` and `others` where the second form's files are `local` and `remote`.

## Contracts

- Provided: [`session-audio/v2`](../../contracts/session-audio/v2/README.md), `experimental`
  — each source as delivered, with the session's clock: what `record` writes, and so what the
  tray records (ADR-0004)
- Provided: [`session-audio/v1`](../../contracts/session-audio/v1/README.md), `experimental`
  — the first form, two files at 16 kHz: what `record … --as v1` writes, to compare with it
- Provided: [`recording/v1`](../../contracts/recording/v1/README.md), `experimental`
  — what `record` says while it records
- Consumed: none

## Decisions

[ADR-0002](../../docs/adr/0002-a-rust-core-of-programs-and-a-web-view-shell.md): a program
of its own, in Rust, on one audio library. ADR-0015, in its own pull request: the local side, and the
`pipewire` crate for one stream.
