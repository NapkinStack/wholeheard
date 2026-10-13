# shell — local instructions

> Only what is **specific to this module**.
> Never duplicate a kernel rule (`/AGENTS.md`): that is wasted context and a source of
> divergence.
>
> Indicative budget: 100 lines.

## Responsibility

Puts Summit's four steps in the user's hands: a tray, a global shortcut, notifications, one window.

## What this module does not do

- It records, hears and writes nothing itself: each step is its module's program, started here. A feature that needs a step's code belongs to that step.
- It decides nothing about a session's files: it reads them as their contracts describe them.

## Internal conventions nobody could guess

- Every sentence the user reads is in `src/words.rs`, in each language the interface speaks — English and French, following the system's (the decider, 2026-10-13). They are the product's content: the one place where this repository holds another language than English. A sentence written anywhere else is a defect.
- Decided before any code (ADR-0002, cycle 01 *D1*): Tauri v2; under GNOME the global shortcut and the window's coming forward go through the desktop's portal, not the toolkit's plugin; the tray's menu holds the actions themselves, since it cannot bring a window forward there.

## Business invariants

- No window opens without a gesture of the user.
- A recording never stops because the shell did.
- Nothing crosses the network before the user was shown what and where, and agreed.

## Known traps

- None yet.

## Areas not to change without approval

- None yet.

## Non-standard commands

- None yet.