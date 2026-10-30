# minutes — version 2

A session's minutes are one text file a person reads as it is — highlights, actions, key
points — that says what made it, and that exists only when it is whole. Version 2 is
[version 1](../v1/README.md) and one more key, optional: the meeting's `title`. Version 1 stays
as it is, frozen (ADR-0005); a consumer reads both.

```mermaid
flowchart LR
    J["transcript.jsonl<br/>context.txt"]:::file --> S["summary"]:::producer
    S -->|"writes, whole or not at all"| M["minutes.md"]:::file
    M --> E["the shell"]:::consumer

    classDef producer fill:#065f46,color:#fff
    classDef consumer fill:#1e3a8a,color:#fff
    classDef file fill:#374151,color:#fff
```

**Legend** — green: the producer · grey: files in the session's folder · blue: the consumer.

## What it carries

`minutes.md` in the session's folder: UTF-8, Markdown, opening on a front matter.

| Front matter | Meaning |
|---|---|
| `contract` | `minutes/v2` |
| `made` | When, ISO 8601 with the local offset |
| `provider` | The host that answered, or `local` — never a key |
| `model` | The model's name, as the provider knows it |
| `context` | The text the provider received with the transcript, or `null` |
| `title` | Optional: the meeting's name, as the provider gave it — a few words in the meeting's language, on one line, at most 80 characters; never a key nor a path. Absent, the meeting has no name of its own: its date names it |

Then exactly three sections of second level, in this order, their titles in the meeting's
language:

| Section | Holds |
|---|---|
| 1. Highlights | A few sentences: what the meeting was for and what came out of it |
| 2. Actions | A list; each item says **what**, **who** and **by when**, in that order, separated by ` — ` |
| 3. Key points | A list: decisions, figures, dates, what stays open |

- **What the meeting did not settle is said**, in the meeting's language: *not specified*, or
  that it was proposed and not accepted. An empty section says that there is none.
- **Whole or absent.** The file appears under its name only once it is complete; a provider
  that fails leaves the minutes that were there, or none.
- **Made again replaces.** There is one `minutes.md`; the one before is not kept by this
  contract, nor its `title`.
- **A value is on its line.** Each key holds one line: `key: value`. A value holding anything but
  plain words — `title` always — is a JSON string in double quotes, its escapes JSON's; so a
  `title` holding `"` or `:` stays on its line, and `null` is no value, quoted or not.
- **A title out of these bounds is no title.** A reader takes a `title` that once read is blank,
  holds a line break or a control character, or runs over 80 characters — characters, not bytes —
  as absent: the minutes still read, and the date names the meeting. A producer given such a title
  writes the minutes without it. Whether a title is a key or a path, no reader can tell: that is
  the producer's to keep out.
- **A consumer reads both versions.** `minutes/v1` and `minutes/v2` alike, with or without a
  `title`; a key it does not know, it leaves aside, and a `title` in a file naming `minutes/v1` too.
  The producer writes v2 once its consumers read it.
- **Version 1 is not deprecated.** Minutes already on a user's disk stay in v1, are never rewritten
  for this, and stay readable: v1 has no removal date.

## What it never does

- It never promises that an action is right: it is a model's reading of a transcript. Who
  and by when may be wrong; the user reads them against the meeting.
- It never carries the transcript, the audio, a key, or a path of the machine.
- Its sections' titles are never a fixed text: a consumer finds a section by its place, not
  by its name.

## Examples

Valid:

```markdown
---
contract: minutes/v2
made: 2026-10-13T15:12:40+02:00
provider: local
model: mistral
context: null
title: "Date de l'événement et intervenants"
---

## Points forts

La date de l'événement est fixée au jeudi 3 juin, avec le 10 en seconde date.

## Actions

- Contacter les trois intervenants et le comité scientifique — Alexandre — 26 avril
- Pré-réserver les salles — non précisé — non précisé

## Points clés

- Réponse des intervenants attendue pour le 26 avril ; un plan B reste à trouver.
```

To refuse: a file without the front matter or naming a `contract` other than `minutes/v1` and
`minutes/v2`; fewer or more than three sections of second level.
