# summarising — version 1

What the program that writes the minutes says while it works, how it is given a key, and how
it is asked whether a model answers: one line of text per event, for whoever started it.

```mermaid
flowchart LR
    S["shell — or a script,<br/>a terminal"]:::consumer -->|"starts: wholeheard-summary <session-folder> --provider <address> --model <name>"| M["summary"]:::producer
    S -->|"or: wholeheard-summary --test --provider <address> --model <name>"| M
    S -.->|"its environment: WHOLEHEARD_PROVIDER_KEY, when the provider wants a key; WHOLEHEARD_TRACE, its log's id"| M
    M -->|"its output: one JSON object per line"| S
    M -.->|"its error stream: one sentence, when it fails"| S
    M -->|"sends: the request, to the provider's host"| P["The provider<br/>(OpenAI's protocol)"]:::outside
    M --> F["The session's folder<br/>(minutes)"]:::file

    classDef producer fill:#065f46,color:#fff
    classDef consumer fill:#1e3a8a,color:#fff
    classDef outside fill:#7c2d12,color:#fff
    classDef file fill:#374151,color:#fff
```

**Legend** — green: the producer · blue: whoever started it, the consumer · brown: what it
contacts, off the machine unless the address is on it · grey: what it leaves, described by
`minutes`.

**For a machine**: [`event.schema.json`](./event.schema.json) says what a line of the output
is — and is what the project's comparator judges a change to this version by (ADR-0005). The
order of the events, and what each means, are said here and nowhere else.

## How it is started

- **Minutes**: `wholeheard-summary <session-folder> --provider <address> --model <name>`, the two
  options in either order. It reads the folder's `transcript.jsonl` (`transcript/v1`) and its
  `context.txt`, when there is one, and writes the folder's `minutes.md` (`minutes/v1`).
- **A test**: `wholeheard-summary --test --provider <address> --model <name>`, the options in either
  order. It sends one sentence of its own — nothing of a meeting — and writes nothing.
- **The key**, when the provider wants one, is in its environment as `WHOLEHEARD_PROVIDER_KEY`,
  never in its arguments. It is sent to the provider's host and written nowhere: not on the
  output, not on the error stream, not in the minutes.
- **The address** speaks OpenAI's protocol (`…/v1`); it is followed nowhere else — no
  redirection, no proxy named by the environment.

## What it carries

**The output**: UTF-8, one JSON object per line, nothing else. Each has `event`:

| `event` | Other fields | Says |
|---|---|---|
| `sending` | `host`: who receives the request, as the minutes name it — `local` for an address of this machine, else the host's name or address in lower case, without its port; `model`; `characters`: how many the request holds, a whole number | Said **before** anything leaves the machine, once, and first |
| `written` | `file`: the minutes' path | The minutes are whole in the folder. **Last**, minutes only |
| `answered` | `host`; `model` | The model answered the test's sentence. **Last**, a test only |

**Failing**: the program ends in failure with one sentence on its error stream, starting
`wholeheard-summary: `, and no `written` or `answered`. Whatever fails, the transcript is untouched
and the minutes that were there are still there — nothing half-written is left behind. Before
`sending`, nothing left the machine.

**Its log**: the program logs what it does ([ADR-0007](../../../docs/adr/0007-each-program-logs-what-it-does-never-what-was-said.md)),
at `info`, under an id of its own, in the user's folder for logs (`$XDG_STATE_HOME/wholeheard/logs`,
or `~/.local/state/wholeheard/logs`), where it also removes day files older than fourteen days.
Whoever starts it may set, in its environment:

- `WHOLEHEARD_TRACE` — the making's id: letters, digits and dashes, 64 at most. Any other value
  is replaced by an id of the run's own, without a word;
- `WHOLEHEARD_LOG` — `off`, `error`, `warn`, `info`, `debug` or `trace`; any other value is
  `info`. Turned `off`, the log leaves nothing: no file, no day file removed;
- `WHOLEHEARD_LOG_DIR` — an absolute folder; a relative one is ignored. A folder that cannot be
  made or written leaves no log, and changes nothing else.

None is required. None, and no state of the log, changes a line of the output, the error stream
or what the program leaves — with one exception: a run that crosses midnight UTC while the
next day's file cannot be created has the logging library say so once on the error stream.
The log is not what the program leaves. What the log holds is not this
contract: it is read by people, never by a consumer.

## What it never does

- It never prints the key, nor the provider's answer, on its output or its error stream.
- It never prints anything but these lines on its output: a consumer may refuse a line that is
  not one — and must ignore a field or an `event` it does not know, so that a second version
  can add one.
- It never sends without saying `sending` first: what cannot be said first is not sent.

**Open before `stable`**: whether a consumer is told when the provider starts answering — a
long making, minutes on a laptop's processor, says nothing between `sending` and `written`.

## Examples

Valid — minutes made by an online provider:

```text
{"event":"sending","host":"openrouter.ai","model":"anthropic/claude-sonnet-5.5","characters":48210}
{"event":"written","file":"<session-folder>/minutes.md"}
```

Valid — a test of a model on this machine:

```text
{"event":"sending","host":"local","model":"mistral","characters":31}
{"event":"answered","host":"local","model":"mistral"}
```

To refuse: a line that is not a JSON object with an `event`.
