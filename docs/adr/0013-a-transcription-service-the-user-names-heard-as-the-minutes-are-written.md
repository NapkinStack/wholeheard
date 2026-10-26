# ADR-0013 — A transcription service the user names, heard as the minutes are written

> The framework's words: `.nstack/handbook/glossary.md`.

- **Status**: Proposed — the framer's, on the decider's words of 2026-10-26: "gérer les models de
  transcription de la même manière que pour les minutes", their choice "Le même choix que les
  minutes", and on criticality "Standard comme summary". Accepting it is the decider's.
- **Date**: 2026-10-26
- **Decision makers**: the decider, `@napkinstack-admin`
- **Scope**: module `transcription`; the shell consumes it
- **Reversibility**: easy — one more engine behind ADR-0003's interface; Whisper and SpeechAnalyzer
  stay

---

```mermaid
flowchart LR
    S["Settings: who hears<br/>on this computer · a service"]:::user --> H["The shell<br/>the host agreed first, the key from the keyring"]:::mod
    H -->|"--provider · --model<br/>WHOLEHEARD_PROVIDER_KEY"| T["wholeheard-transcription<br/>the step, unchanged"]:::mod
    T -->|"each span: POST /audio/transcriptions"| P["The service the user named"]:::out
    P -->|"segments, their times"| T
    classDef user fill:#1e3a8a,color:#fff
    classDef mod fill:#065f46,color:#fff
    classDef out fill:#7c2d12,color:#fff
```

**Legend** — blue: the user's choice · green: Wholeheard · red: outside the machine, only once chosen
and agreed.

## Context

The minutes are written on this computer (Ollama) or by a service the user names at an address
that speaks OpenAI's protocol, with their model and key: the host agreed before the first
sending, the key in the system's keyring, *Tester* (cycle 03; `modules/summary`, which reads the
key from `WHOLEHEARD_PROVIDER_KEY` and writes it nowhere). The transcript is heard on this computer
only: a Whisper model, or SpeechAnalyzer on a Mac (D42). The charter drew a transcription service
from the start, "only if the user chose it". D43 asks for the minutes' way.

## Problem

How does a transcript come from a service the user names, as the minutes do, without changing
the step, and with nothing sent before the user agreed?

## Constraints

- ADR-0003: one interface; an engine more, the step unchanged.
- Security rule 1: nothing leaves the machine to a host the user did not configure; the meeting's
  sound sent only after the user agreed to that host, as for the minutes.
- `transcript/v1`: each segment with its start and end.
- The key: never written, never logged (ADR-0007), held by the system's keyring as the minutes'.

---

## Prior art

**Dominant convention of the field:** OpenAI's `POST /v1/audio/transcriptions` — a multipart
upload of the audio, `model`, `language`, and `response_format=verbose_json` for segments with
their times — spoken by most services and by local servers alike.

**References examined:**

| Reference | What it does | Applicable here? |
|---|---|---|
| OpenAI's audio API ([*Create transcription*](https://platform.openai.com/docs/api-reference/audio/createTranscription), not read again here) | `file`, `model`, `language`, `prompt`, `response_format` (`json`, `verbose_json` with `segments`, …); `whisper-1` gives segments, the newer `gpt-4o-transcribe` text only | Yes: the protocol; and a service that answers text alone must be refused or said, never timed by guess |
| Groq, Mistral and local servers (faster-whisper's OpenAI-compatible servers, LocalAI) | The same route and fields | Yes: one engine reaches them all, as the minutes' one does |
| Wholeheard's own minutes (`modules/summary`, cycle 03) | The address, the model, the key from `WHOLEHEARD_PROVIDER_KEY`, the host agreed first, *Tester* | Yes: the user's way, kept as it is |

---

## Options considered

### Option 1 — Do nothing
- On this computer alone; the charter's service never offered.

### Option 2 — An engine of transcription's that speaks the audio route
- `wholeheard-transcription <session> --provider <address> --model <name> [--language <code>]`,
  the key from `WHOLEHEARD_PROVIDER_KEY` as `summary` reads it; each span sent as one 16 kHz file,
  `response_format=verbose_json`; its `segments` the span's segments.
- Advantages: the step unchanged; the shell's way of the minutes reused whole.
- Cost of getting out: drop the engine.

### Option 3 — The shell sends the audio itself
- Drawbacks: a step's work in the shell, which holds none (ADR-0002).

## Decision

**Option 2.**

1. **The route is OpenAI's `/audio/transcriptions`**, the provider's address as the user gives it
   (`https://api.groq.com/openai/v1`, `http://localhost:8000/v1`…), with `verbose_json`.
2. **A service that answers no segments with their times is refused for the span**, said in one
   sentence: "this service gives no times: choose another model" — a transcript is never timed by
   guess.
3. **The key** is read from `WHOLEHEARD_PROVIDER_KEY`, as `summary` reads it; the shell hands it
   from the keyring under its own account, the transcription's apart from the minutes'.
4. **Nothing is sent before the user agreed to the host**, as for the minutes
   (`transcription_agreed` beside `minutes_agreed`); a host on this computer never asks.
5. **Criticality**: `transcription` holds a key, which the handbook's rule makes `critical`
   (`.nstack/handbook/05-workflow.md` §7). The decider chose, 2026-10-26: **`standard`, as
   `summary`**, which already holds one — a deviation taken knowingly: three users who agreed, no
   build published for people outside the project. It ends with `capture`'s, before the first
   build published outside the project (PDR-0002, decision 2), for both modules.

### Deviation from the convention

- **From the handbook's criticality rule**, for `transcription` and, written now, for `summary`.
  **Expected user value**: the pace kept while three people use it. **How we will observe it**:
  the day PDR-0002's event comes, both modules' manifests say `critical`.

---

## Success criterion

> We will consider this was the right call if, **by the end of cycle 05**, a session is transcribed
> by a service the user named — a local OpenAI-compatible server on the bench at least — its
> segments on their tracks and times, and the minutes' way (agreed host, key, *Tester*) is the
> same for both steps.

What we do if it is not: the engine stays behind its setting, unused, and the cause is named.

---

## Consequences

**Positive:** the charter's transcription service; Groq-class speeds where the user wants them.

**Negative and accepted debt:** a module holding a key at `standard`, as `summary` already does.

**Impacts on other modules or contracts:** the shell's Settings and keyring; `transcript/v1`
unchanged — its header names an engine and a model.

**Rule to automate:** transcription's tests against a server played on the machine — segments,
text without times refused, a key refused, no answer.

---

## Rejected alternatives

- **Timing a text-only answer by spreading it over the span**: a transcript that says when
  someone spoke must not guess it.
- **The shell sending the audio** (option 3): a step's work out of its module.
