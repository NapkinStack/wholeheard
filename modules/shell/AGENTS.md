# shell — local instructions

> Only what is **specific to this module**.
> Never duplicate a kernel rule (`/AGENTS.md`): that is wasted context and a source of
> divergence.
>
> Indicative budget: 100 lines.

## Responsibility

Puts Wholeheard's four steps in the user's hands: a tray, a global shortcut, notifications, one window.

## What this module does not do

- It records, hears and writes nothing itself: each step is its module's program, started here. A feature that needs a step's code belongs to that step.
- It decides nothing about a session's files: it reads them as their contracts describe them.

## Internal conventions nobody could guess

- Every sentence the user reads is in `src/words.rs`, in each language the interface speaks — English and French, following the system's (the decider, 2026-10-13). They are the product's content: the one place where this repository holds another language than English. A sentence written anywhere else is a defect.
- The Whisper models Settings offer are `registry/models.json`, built in (`src/registry.rs`, D40, ADR-0010): a model is added or removed there, never in the code. Each says what it is good for by a key whose sentence is in `src/words.rs`; a new key needs its sentence first. The shell's tests read the file: one that does not read, or names a host other than `registry::HOSTS`, fails them. Its first model stays this build's default (`model::DEFAULT`): a list read again that puts another first is refused, the default being what a transcript waits for and the tray fetches.
- Decided before any code (ADR-0002, cycle 01 *D1*): Tauri v2; under GNOME the global shortcut and the window's coming forward go through the desktop's portal, not the toolkit's plugin; the tray's menu holds the actions themselves, since it cannot bring a window forward there.

## Business invariants

- No window opens without a gesture of the user.
- A recording never stops because the shell did.
- Nothing crosses the network before the user was shown what and where, and agreed.
- A log line holds steps, ids, hosts, models and failures — never a word of a meeting, a key, nor
  a session's path beyond its folder's name (`tests/log.rs`, the e2e).

## Known traps

- `ashpd` is taken with its `async-io` feature, never `tokio`: with `tokio` the portal's library switches the desktop bus to that runtime, and every notification then panics inside the toolkit's own runtime ("cannot start a runtime from within a runtime").
- A notification is shown from the main thread or a plain thread, never from a task of the async runtime.
- The tray's icon is a dark tile with a light summit: a light glyph alone was invisible on the decider's desktop, whose bar is light. A tray icon is judged on a light bar, a grey one and a dark one.
- A desktop entry whose `Exec` program is not found is no entry at all for the desktop: the portal answers "App info not found" and the shortcut is refused.
- The toolkit writes `gen/` at each build: it is ignored. Files are added to a commit by name.
- A name declared twice in one function of `ui/window.js` is a syntax error that leaves the whole window blank, with nothing in the shell's own logs: `check` runs `node --check` on it where Node is there.

## Areas not to change without approval

- None yet.

## Non-standard commands

- None yet.