---
goal: "On Fedora, a user finds their meetings, reads one — its minutes beside its transcript — frees space and sets Summit, in one window, without a terminal"
status: proposed          # proposed | accepted | closed | stopped — the decider accepts
appetite_weeks: 3         # the time the decider WANTS to spend, not an estimate — framer's proposal, the decider sets it
start: 2026-10-19         # YYYY-MM-DD
end: 2026-11-09           # start + appetite: the first day the cycle is over — the circuit breaker
deliverables:
  - id: D17
    title: "A user sees their meetings and what they weigh, and opens one"
    module: shell
    state: proposed
    acceptance:
      - "Given sessions in the user's folder — of both forms, some with a transcript or minutes, one whose audio was deleted — when the user opens the window from the tray's menu, then each meeting is listed by week, newest first, with its date, its length, what it holds and what its audio weighs, and above them what all the audio weighs against the transcripts and minutes"
      - "Given a session still being recorded, or one a recording left without its end, when the list is shown, then it says so, and nothing of it is offered that would touch its files"
      - "Given the window, when it is used, then it follows the system's light or dark theme, every action in it is reached from the keyboard, and no window ever opens without a gesture of the user"
  - id: D18
    title: "A user reads one meeting: its minutes beside its transcript"
    module: shell
    state: proposed
    acceptance:
      - "Given a session with its minutes and its transcript, when the user opens it, then the minutes — highlights, actions with who and by when, key points, and which model wrote them and where — are beside the transcript, each line with its side, its voice and its time; who spoke is drawn over the length of the meeting; the transcript can be searched; and the session's folder opens from it"
      - "Given a session that holds no transcript, or no minutes, when it is opened, then what it lacks is offered, and made on the user's word as the tray makes it"
      - "Given a transcript or minutes made again with the model the user chose, when the new one is being made, then the current one stays readable until the new one is whole; then the current one goes to the system's trash; and a making that fails is said in one sentence and leaves the current one as it was"
  - id: D19
    title: "A user frees space — the audio alone, or the whole meeting — sees what it frees before, and can undo"
    module: shell
    state: proposed
    acceptance:
      - "Given the sessions, when the user chooses to free the audio alone or the whole meeting, then the heaviest are shown first, those older than thirty days can be ticked at once, and what will be freed is said before anything is done"
      - "Given the user's word, when the space is freed, then what is freed goes to the system's trash, nothing is deleted for good; a session whose audio alone was freed still lists, and its transcript and minutes still read; and a session restored from the trash is listed again as it was"
      - "Given a session being recorded or transcribed, when space is freed, then it cannot be chosen"
  - id: D20
    title: "A user sets how Summit chains its steps and which model does each one"
    module: shell
    state: proposed
    acceptance:
      - "Given the settings, when the user chooses what follows a recording — nothing, the transcript, or the transcript then the minutes — and whether the reminder to announce the recording is shown, then the next recording behaves so; out of the box the transcript follows alone and the reminder is shown; and the choices are in one configuration file that reads without Summit"
      - "Given the settings, when the user chooses the model of a step — one found on this computer, an online service with its address filled in, or their own address, model and key — then the next making uses it; a key is kept in the system's keyring, never in a file; and *Tester* says in one sentence whether that model answers"
      - "Given the settings, when the user changes the shortcut, the sessions' folder, or how long the audio is kept, then each holds from the next recording; and audio older than that goes to the system's trash, its transcript and minutes kept"
  - id: D21
    title: "A user gets the minutes from the tray and the window, and agrees before anything first leaves for an online service"
    module: shell
    state: proposed
    acceptance:
      - "Given an online provider and no agreement given yet, when minutes are about to be sent for the first time — asked for, or chained — then the user is shown exactly what leaves the machine and to which host, and nothing is sent before they agree; given the agreement, a chained making sends without asking again; changing the provider's host asks again; and the minutes can be written on this computer instead"
      - "Given minutes asked for or chained, when they are made, then the user is told when they are ready, and a failure is said in one sentence with the transcript untouched"
  - id: D22
    title: "While a recording runs, a small window shows its time and its two sources' levels, and stops it"
    module: shell
    state: proposed
    acceptance:
      - "Given a recording started from the tray or the shortcut, when the user opens the small window, then it shows the time recorded and a level for the microphone and one for the meeting, moving as sound comes, and *Arrêter* stops the recording as the tray does; it opens the full window, and it never opens without a gesture"
  - id: D23
    title: "The decider goes through one real meeting with the window"
    module: shell
    state: proposed
    acceptance:
      - "Given the window and the tray's menu on the decider's laptop, when the decider goes through one real meeting with them — record, transcript, minutes, read, free space — then the decider writes what stood in the way; the deliverable is accepted when the decider reads nothing blocking in that list"
---

# Cycle 03 — the window

> Copied from `.nstack/forms/cycle.md` at framing (`.nstack/playbooks/framing.md`). One accepted cycle at a
> time; no automatic extension.

## Goal

Cycle 02 kept the audio as recorded and brought the transcript to the tray. Everything else a
user does with a meeting still takes a terminal or a file manager: finding it, reading its
minutes, freeing space, choosing a model. The window was drawn and accepted in cycle 02
([D11, its drawings](../drawings/d11-the-window/README.md)); this cycle builds it, and the
accepted changes are its criteria.

```mermaid
flowchart LR
    T["The tray<br/>(built)"]:::built --> L["D17<br/>the meetings,<br/>what they weigh"]:::work
    L --> S["D18<br/>one meeting:<br/>minutes beside transcript"]:::work
    L --> F["D19<br/>freeing space,<br/>undone from the trash"]:::work
    T --> P["D20<br/>the settings,<br/>a model per step"]:::work
    P --> M["D21<br/>the minutes from the tray,<br/>the agreement first"]:::work
    T --> W["D22<br/>the small window<br/>while recording"]:::work
    S & F & M & W --> R["D23<br/>the decider's<br/>real meeting"]:::gate

    classDef built fill:#374151,color:#fff
    classDef work fill:#065f46,color:#fff
    classDef gate fill:#92400e,color:#fff
```

**Legend** — grey: built before this cycle · green: a deliverable a user sees · orange: the
decider's trial that closes the cycle.

## Deliverables

Each deliverable's screens are a board of [D11's drawings](../drawings/d11-the-window/README.md);
a board is what the decider accepted, the criteria are what binds. All are in `shell`: its web
view (ADR-0002, Tauri) shows the window, and the programs stay where they are.

### D17

Board 1. The list reads each session's `session.json` and files: both forms (`session-audio`
v1 and v2) are listed; what a session weighs is its files' size on disk.

### D18

Boards 2 and 6 (*Refaire…*). The minutes and the transcript are read from their files as their
contracts say (`minutes/v1`, `transcript/v1`); the window writes neither.

### D19

Board 3. The system's trash, never a delete: what the decider asked for undoing.

### D20

Board 4. What the shell's configuration already holds (the model, the folder, the shortcut)
moves into the window; models "on this computer" are those Summit finds installed (Whisper
models in its folder, Ollama's list).

### D21

Board 5, the second agreement. The first — the transcription model fetched on the user's word
— is built (cycle 02, D12) and is drawn in the window as it is.

### D22

Board 6, the small window. The levels come from what `capture` says while it records; if it
says nothing that carries a level today, `recording/v1` gains it as the contract's rules allow.

### D23

The trial that closes the cycle, as cycle 01's D7 had it.

## Out of scope

- The offer to record when an application takes the microphone, and recording by itself:
  drawn and accepted (board 6), not built here — it needs another application's use of the
  microphone seen on each system, and it records other people. *Later*.
- Windows and macOS.
- `export`, and `all`.
- Compressing the tracks.
- Telling the voices of one track apart.

## Later

- The offer when an application takes the microphone, and recording by itself.
- Windows and macOS, with the two colleagues.
- An installer: the day the charter's success criteria start.
- `export`, with `all`.
- Compressing the tracks without loss (cycle 02, D9: it halves their weight).

## Open questions

**Blocking:**

- **The appetite** — three weeks is the framer's proposal; the decider sets it. When the seven
  deliverables do not fit, D22 and D19 are cut first, never the end date.

**Not blocking:**

- How a window is tested end to end on Fedora: the shell's e2e drives the tray today; a window
  needs a driver of its web view (WebKitGTK's WebDriver, through `tauri-driver`). The first
  pull request of D17 shows one, or says why another way.
- The engine's remaining errors (cycle 02's closure) are read in the window as they are; the
  minutes' reading through them is D16's, accepted.

## Closure

<Written at the end: what was delivered, which success criteria moved, what was deferred,
where the next framing starts.>

**Documents brought current:** <the front page, the module map, a module's page — each with
its pull request; or "none", and why nothing a reader sees has changed>
