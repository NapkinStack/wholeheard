# summary

Writes the minutes of a transcript.

- **Owner**: NapkinStack/maintainers
- **Criticality**: standard
- **Manifest**: [MANIFEST.yaml](./MANIFEST.yaml)

```mermaid
flowchart LR
    T["transcript.jsonl<br/>context.txt"]:::file --> P["wholeheard-summary<br/>a program, no window"]:::mod
    P -->|"one request"| V["The provider the user configured:<br/>a model on the machine, or online"]:::ext
    V --> P
    P -->|"minutes/v1, whole or not at all"| M["minutes.md"]:::file
    P -.->|"sending · written"| W["Whoever started it"]:::user

    classDef file fill:#1e3a8a,color:#fff
    classDef mod fill:#065f46,color:#fff
    classDef ext fill:#374151,color:#fff
    classDef user fill:#92400e,color:#fff
```

**Legend** — blue: files of the session's folder · green: this module · grey: the only host
it contacts · orange: who reads its events, one JSON object per line.

```bash
# The example: an online provider and a large model — the one that kept every action of the
# meeting it was tried on (cycle 01, D5). The key is read from the environment, written nowhere.
wholeheard-summary <session-folder> --provider https://openrouter.ai/api/v1 --model anthropic/claude-sonnet-5.5

# A model on the machine: nothing leaves it, no key — and, on a laptop, fewer of the actions.
wholeheard-summary <session-folder> --provider http://localhost:11434/v1 --model mistral

# Whether a model answers, before any meeting: one sentence of the program's own is sent,
# `sending` then `answered` said, or why not in one sentence. What the settings' Tester runs.
wholeheard-summary --test --provider http://localhost:11434/v1 --model mistral
```

| Tried on one meeting, with its context | Of seven actions agreed: missing | Time |
|---|---|---|
| The large model of the example | 0 | 15 s |
| A fast online model | 2 to 3 | 3 s |
| A 7B model on a laptop's processor | 4 | minutes |

**The transcript is read through its errors.** The model is told the transcript comes from a
speech recogniser and names its errors: a word heard as another, a name or a figure misheard,
a sentence written twice, the remote side written again as an echo, a reply missing. Where the
rest of the meeting makes what was meant certain, it writes that. Where a name, a figure, a
date or an owner stays uncertain, it writes it as heard and says so. A task said as "I" gets
its owner only when the meeting names them, otherwise "probably" someone, or nobody: the
minutes are less often wrong about who, and less often sure. The price, too:
a stray word can make a fact said clearly read as uncertain — "c'est nantes" misheard beside
"je garde celui de Paris" made the kept server "Paris or Nantes". The transcript itself is
never changed.

**Attach a context** — `context.txt` in the session's folder, a few lines: who was there, on
which side — in the room or remote — and who is only talked about. Without it, a person the
meeting talks about was written as the owner of an action.

On the bench's meeting, four of the engine's transcripts, three answers each from the model of
the example: told of the errors and given a context, the minutes give the support rota — said
over six turns — right 12 times in 12, against 5 in 12 without either, and no task to the wrong
person without doubt, against four. Which of the two does the most is not measured apart. What
no request recovers is the owner of a task said as "I": each track holds two people (cycle 02,
D16).

A key, when the provider wants one, is read from `WHOLEHEARD_PROVIDER_KEY` and written nowhere.
Whatever fails — no network, a refused key, no credit left with the provider, an unknown
model, an answer cut short or that is not minutes — the reason is one sentence on the error
stream, the transcript is untouched and the minutes that were there are still there.

## Getting started

The standard verbs — `bootstrap`, `check`, `test`, `run` — are declared in the `commands`
section of the [manifest](./MANIFEST.yaml), for this module's stack. What each one must do:
`.nstack/handbook/09-platform.md` §2.

## Contracts

- Provided: [`minutes/v1`](../../contracts/minutes/v1/README.md), `experimental`; [`summarising/v1`](../../contracts/summarising/v1/README.md) — how it is started and what it says, `experimental`
- Consumed: [`transcript/v1`](../../contracts/transcript/v1/README.md)

## Decisions

See `docs/adr/`.
