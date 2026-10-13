# shell

Puts Summit's four steps in the user's hands: a tray, a global shortcut, notifications, one window.

- **Owner**: NapkinStack/maintainers
- **Criticality**: standard
- **Manifest**: [MANIFEST.yaml](./MANIFEST.yaml)

```mermaid
flowchart TB
    U(["The user"]):::user -->|"start · stop · make the minutes"| S["shell<br/>tray · shortcut · notifications · one window"]:::mod
    S -->|"starts, stops, reads what they print"| P["summit-capture · summit-transcription<br/>summit-summary · export"]:::step
    P --> F["The session's folder"]:::file
    F -->|"read"| S
    S -->|"what happened, what failed"| U

    classDef user fill:#92400e,color:#fff
    classDef mod fill:#065f46,color:#fff
    classDef step fill:#1e3a8a,color:#fff
    classDef file fill:#374151,color:#fff
```

**Legend** — orange: whoever uses Summit · green: this module · blue: the four steps, programs
it starts and never reads the code of · grey: files, the only thing it shares with them.

**It holds no step** (ADR-0002): closed or crashed, a recording goes on, and every step still
runs from a terminal.

**Today**: an icon in the tray whose menu starts and stops a recording, the same from a
global shortcut — Super+Alt+M under a Linux desktop — and a notification for what matters:
the reminder to announce the recording, a source that is silent, the stop, a failure. No
window yet. In French where the system is, in English anywhere else.

```bash
nstack bootstrap shell && nstack bootstrap capture
(cd modules/capture && cargo build --locked --release)
SUMMIT_CAPTURE=$PWD/modules/capture/target/release/summit-capture nstack run shell
```

| On a Linux desktop | What it takes |
|---|---|
| The icon | Under GNOME, an extension that shows tray icons — *AppIndicator and KStatusNotifierItem Support*. Without it Summit runs, unseen |
| The shortcut | The desktop grants it to an installed application: copy `linux/dev.summit.Summit.desktop` into the user's `applications` folder under their data folder, then start Summit again and accept the desktop's question. Refused, Summit says so once; the menu still does the same |

**Settings** — `summit/settings.toml` in the user's configuration folder, a few lines written
by hand until the window exists: `reminder = false` to stop the reminder, `sessions = "…"`
for where sessions go. Out of the box they go to a folder `Summit` in the user's documents.

## Getting started

The standard verbs — `bootstrap`, `check`, `test`, `run` — are declared in the `commands`
section of the [manifest](./MANIFEST.yaml), for this module's stack. What each one must do:
`.nstack/handbook/09-platform.md` §2.

## Contracts

- Provided: none
- Consumed: none yet — the contracts of the files it reads are declared with the slice that reads them

## Decisions

See `docs/adr/`.
