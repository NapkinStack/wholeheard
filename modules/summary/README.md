# summary

Writes the minutes of a transcript.

- **Owner**: NapkinStack/maintainers
- **Criticality**: standard
- **Manifest**: [MANIFEST.yaml](./MANIFEST.yaml)

```mermaid
flowchart LR
    T["transcript.jsonl<br/>context.txt"]:::file --> P["summit-summary<br/>a program, no window"]:::mod
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
summit-summary <session-folder> --provider http://localhost:11434/v1 --model <name>
```

A key, when the provider wants one, is read from `SUMMIT_PROVIDER_KEY` and written nowhere.
Whatever fails — no network, a refused key, an unknown model, an answer cut short or that is
not minutes — the reason is one sentence on the error stream, the transcript is untouched and
the minutes that were there are still there.

## Getting started

The standard verbs — `bootstrap`, `check`, `test`, `run` — are declared in the `commands`
section of the [manifest](./MANIFEST.yaml), for this module's stack. What each one must do:
`.nstack/handbook/09-platform.md` §2.

## Contracts

- Provided: [`minutes/v1`](../../contracts/minutes/v1/README.md), `experimental`
- Consumed: [`transcript/v1`](../../contracts/transcript/v1/README.md)

## Decisions

See `docs/adr/`.
