---
goal: "On Fedora, a meeting recorded with Summit is kept as it was heard, and its transcript is right about who spoke and when — on loudspeakers as with a headset"
status: proposed          # proposed | accepted | closed | stopped — the decider accepts
appetite_weeks: 3         # the time the decider WANTS to spend, not an estimate
start: 2026-10-14         # YYYY-MM-DD
end: 2026-11-04           # start + appetite: the first day the cycle is over — the circuit breaker
deliverables:
  - id: D8
    title: "Spike — a bench: a meeting whose every word, speaker and instant is known, played as a headset, as loudspeakers low and as loudspeakers loud, and four counts of what a transcript got wrong"
    module: platform
    state: proposed
    acceptance:
      - "Given a script written for the purpose — about five minutes in French, two sides, at least two voices on one side, turns that overlap, silences — and voices that are nobody's, when the bench renders it for a listening condition, then it yields the tracks a recording of that meeting would hold and the truth beside them: each word, its speaker, its side and its instant"
      - "Given the three conditions — a headset: no sound of one side in the other; loudspeakers low and loudspeakers loud: the remote side heard by the microphone, later and weaker, as a room does it — when each is rendered, then the cycle records how each was made and how far it is from the decider's real room, measured once by playing the bench's remote side on their loudspeakers"
      - "Given a transcript of a rendered meeting, when the bench reads it against the truth, then it prints four counts: words wrong on each side, words said twice, exchanges out of order, words given to the wrong side"
      - "Given the programs as they are on 2026-10-14, when the bench is run on the three conditions, then the cycle records the twelve counts: the line every later change is read against. A count is a record, not a target: a bad one is a delivered answer"
  - id: D9
    title: "A recording keeps each source as the system delivered it, under the name of its source, with the session's clock written beside"
    module: capture
    state: proposed
    acceptance:
      - "Given a recording on Fedora, when it is stopped, then the session holds one track per source — local, the microphone; remote, what the machine plays — each at the sample rate and with the channels its source delivered, in 24 bits, and nothing in a track was resampled, mixed, cleaned or brought to a level"
      - "Given the two tracks, when their clock is read, then it says where each track's samples are on the session's clock, how far the two drift apart over the recording, and which stretches of a track are silence the capture wrote because the source gave nothing"
      - "Given a recording killed after two minutes, or read while it is written, when its tracks are opened, then what D3 held of the first contract still holds: they read in a standard player and hold everything but the last five seconds"
      - "Given an hour of recording as delivered, when it is stopped, then the cycle records what the session weighs — as written, and compressed without loss — and the decider sets, on that measure, the limit that replaces 250 MB"
      - "Given a session recorded under the first contract, when it is read by the programs of this cycle, then it is read as it is — its transcript can still be made — or refused in one sentence that says it is of an earlier form: never read as something it is not"
  - id: D10
    title: "The transcript of a meeting says the same thing about who spoke, and in what order, whether it was taken on loudspeakers or with a headset"
    module: transcription
    state: proposed
    acceptance:
      - "Given the bench's meeting on loudspeakers, low and loud, when it is transcribed, then no more words are said twice than with a headset, and no more words of the local side are wrong than when the same tracks are transcribed as delivered — the echo gone without the speaker's own words"
      - "Given the bench's meeting in any condition, when it is transcribed, then at most one exchange in twenty is out of order, and at most one word in fifty is given to the wrong side"
      - "Given the bench's meeting with a headset, when it is transcribed, then the local track goes to the engine as it was recorded: nothing was taken out of an audio that held no echo"
      - "Given a session, when it is transcribed, then what was made on the way — the microphone without the echo, the copy the engine heard — is kept apart from the tracks, each saying what made it; deleted, they are made again and the transcript is the same"
      - "Given a transcript, when it is read, then each segment says which track it came from, and has a place for which voice within the track — empty in this cycle"
      - "Given the meeting of 2026-10-13 recorded again by the decider — on loudspeakers, then with a headset — when each is transcribed, then the decider reads both against what was said: issue to open, their word on the repetitions and on the order"
  - id: D11
    title: "Spike — the window, drawn before it is built: a session, the sessions and what they weigh, the settings, the two agreements"
    module: null
    state: proposed
    acceptance:
      - "Given the four things the window holds — the session just finished, with its minutes and its transcript; the list of sessions, each with its date, its length, what it holds and what it weighs; the settings; the two moments where the user agrees before something crosses the network — when each is drawn at the size of a laptop's screen, in a light and in a dark theme, then the decider sees the drawings and says what they change"
      - "Given the drawings of the sessions, when the decider follows on them the two ways of freeing space — the audio alone, the transcript and the minutes kept; the whole session — then each shows what it frees before it is done, and how it is undone"
      - "Given the decider's changes, when the drawings are made again, then the cycle records the ones accepted and what was refused: they are the next cycle's criteria. A drawing refused is a delivered answer"
  - id: D12
    title: "A user gets the transcript of a recording from the tray, without a terminal"
    module: shell
    state: proposed
    acceptance:
      - "Given a recording stopped from the tray, when the setting says the transcript follows — and it does, out of the box — then the transcript is made without another gesture, the user is told when it is there, and the click on that notification opens the session's folder"
      - "Given no model on the machine, when a transcript is first needed, then the user is shown the model's size and the host it comes from; nothing is fetched before they agree; the file is checked whole against its published fingerprint before any use; and a fetch that was cut is started again, never used as it is"
      - "Given a transcript that fails — no model, a session that cannot be read — when it happens, then the user is told what failed in one sentence, the recording is untouched, and asking again from the menu makes it"
      - "Given the decider's laptop, when they record one real meeting from the tray and read its transcript, then issue #67 and the one opened for this deliverable hold what they saw"
  - id: D13
    title: "Spike — why did a model on the machine give no minutes of a sixteen-minute meeting?"
    module: null
    state: proposed
    acceptance:
      - "Given the bench's meeting and one of sixteen minutes made from it, when minutes are asked of the model on the machine, then the cycle records for each: whether minutes came, in how long, and — when they did not — what the model answered instead and how much of the transcript it had been given"
      - "Given that record, when it is read against the threshold — minutes of a meeting of an hour, on the decider's laptop, in less than the meeting's length — then the result is written as met or refuted, with what would have to change: a refuted result is a delivered one"
---

# Cycle 02 — The audio kept as recorded

> One accepted cycle at a time; no automatic extension.

## Goal

Cycle 01 built the three programs and put Summit in the tray. A first real meeting, taken
as most are — on a laptop's loudspeakers — gave a transcript that repeats one party under
the other's name. ADR-0004 decided what a session's audio is; this cycle builds it, and
builds first the means to measure it.

```mermaid
flowchart LR
    D8["D8 spike<br/>the bench"]:::spike --> D9["D9<br/>tracks as delivered,<br/>the clock"]:::work
    D9 --> D10["D10<br/>a transcript right about<br/>who spoke, and when"]:::work
    D8 --> D10
    D10 --> D12["D12<br/>the transcript<br/>from the tray"]:::work
    D11["D11 spike<br/>the window, drawn"]:::spike -.-> N["Cycle 03:<br/>the window, built"]:::later
    D13["D13 spike<br/>the model on<br/>the machine"]:::spike -.-> N

    classDef spike fill:#92400e,color:#fff
    classDef work fill:#065f46,color:#fff
    classDef later fill:#374151,color:#fff
```

**Legend** — orange: a spike, whose product is knowledge · green: a deliverable a user sees ·
grey: what this cycle prepares and does not build.

## Deliverables

**The order is the method.** D8 comes first and alone: nothing in the capture or the
transcription moves before the twelve counts of today exist. D9 then D10 are read against
them. D11 runs beside them from the first day: drawings cost little and decide much, and
the decider's eye is the slowest thing to schedule. D12 comes once D10 holds — a transcript
that is wrong is not worth bringing to the tray. D13 is small and fits where a day is free.

| Deliverable | Comes from | Reopens |
|---|---|---|
| D8 — the bench | ADR-0004's rule of method; cycle 01's closure: seven verifications of one step, a regression called a result | — |
| D9 — tracks as delivered, the clock | ADR-0004, rules 1 to 3 | Cycle 01's D3: its six criteria on the first contract |
| D10 — who spoke, and when | ADR-0004, rules 4 to 6; the decider's reading of 2026-10-13 | Cycle 01's D4 |
| D11 — the window, drawn | Cycle 01's D7; the decider, 2026-10-13: "moderne, ludique … UX/UI primordiale" | — |
| D12 — the transcript from the tray | Cycle 01's shell, second slice | Cycle 01's D4, first criterion |
| D13 — the model on the machine | Cycle 01, *A first real meeting* | Cycle 01's D5 |

**What a count is worth.** D10's thresholds — one exchange in twenty, one word in fifty —
are the framer's, written before the bench exists *(framer, to confirm on D8's twelve
counts)*: if a headset itself does worse, the decider restates them on that measure, as D4's
second criterion was restated in cycle 01.

**Modules.** A sixth, `platform`, for the bench: a tool the modules share and none owns
(`.nstack/handbook/09-platform.md`). No other is created: the echo-free microphone and the
engine's copy are made by `transcription`, which is what needs them; `all`, the file to
listen to, is `export`'s, in a later cycle.

## Out of scope

Not done in this cycle, even if time remains:

- Windows and macOS: no build, no trial. They come once the audio's base holds on one system.
- Building the window. It is drawn here (D11) and built next.
- The minutes from the tray, and the agreement before the first sending: with the window.
- Telling the voices of one track apart. The transcript keeps the place for it (D10); the
  bench's script has two voices on one side so that the day it is tried, it is measured.
- `export`: the open files of D6, and `all`.
- Compressing the tracks, and deleting them after a time: measured in D9, built later.
- Live transcription; the file context menu; costs; signing.

## Later

Candidates for the next framing:

- The window, built from D11's drawings: the session, the sessions and their weight, the
  two ways of freeing space, the settings, the two agreements.
- The minutes from the tray; `export`, with `all`.
- Windows and macOS, with the two colleagues.
- The voices within a track.
- A rule that frees space by itself after a time.

## Open questions

**Blocking:**

- **The appetite.** Three weeks is the framer's proposal, not the decider's wish: the
  playbook asks how long they *want* to spend. Fewer weeks means cutting — D13 first, then
  D12 — never the bench.

**Not blocking:**

- Whose voices the bench speaks with: synthesised, or read from public recordings. Neither
  is a person recorded for Summit; D8 says which, and why.
- Whether the bench's simulated room is near enough to a real one is D8's second criterion:
  if it is not, the bench is played through real loudspeakers each time, which only the
  decider's laptop can do.

## Closure

<Written at the end: what was delivered, which success criteria moved, what was deferred,
where the next framing starts.>

**Documents brought current:** <the front page, the module map, a module's page — each with
its pull request; or "none", and why nothing a reader sees has changed>
