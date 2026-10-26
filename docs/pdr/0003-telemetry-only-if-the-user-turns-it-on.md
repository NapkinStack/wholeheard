# PDR-0003 — Telemetry, only if the user turns it on

- **Status**: Proposed. The framer wrote it from the decider's answer of 2026-10-26: asked what
  becomes of the charter's "telemetry, even opt-in, while the repository is private" now that the
  repository is public, they chose "Possible si l'utilisateur l'accepte". Accepting it is the
  decider's.
- **Date**: 2026-10-26
- **Decision makers**: the decider, `@napkinstack-admin`
- **Modules affected**: none for now. Nothing is built by this record.
- **Amends**: the charter's *Out of scope* (the framer's line on telemetry);
  [PDR-0001](./0001-wholeheard-open-on-linux-paid-on-macos-and-windows.md)'s *Out of scope*
  ("Telemetry, still"); rule 1 of `docs/project/rules/security.md`.

> The PDR describes **what the product must do and why**, never its implementation.

---

## User problem

Today the project learns how Wholeheard is used only by asking its three users (charter,
*Success criteria*). The charter also has a second purpose, making the decider's company known
once the repository is open, and nothing measures it. Once a build reaches people outside the
project, nobody can be asked: a failure on a machine nobody knows stays unseen.

The charter keeps telemetry out "while the repository is private". The repository has been
public since 2026-10-26 (PDR-0002), so that condition no longer holds. Read literally, the line
now allows what rule 1 of `docs/project/rules/security.md` forbids. Nobody decided that.

## Goal

A user who wants to help can let Wholeheard send what it does, never what was said. A user who
does nothing sends nothing.

## Out of scope

- Building it: no telemetry is built by this record. It is framed, if ever, as a deliverable of
  its own, with its own criteria.
- Anything on by default, or turned on by an update.
- Anything of a meeting: its audio, its words, its minutes, its title, its participants. Rule 2
  of `docs/project/rules/security.md` and ADR-0007 already keep these out of every log.
- A service that is not shown to the user before anything is sent.

---

## Prior art

| Product / reference | Solution chosen | What we keep from it |
|---|---|---|
| Audacity, 2021 ([pull request #835](https://github.com/audacity/audacity/pull/835), not read again here) | Analytics proposed by its new owner, withdrawn after its users objected | For people who chose a local tool, a sending they did not ask for costs their trust |
| Homebrew ([*Anonymous Analytics*](https://docs.brew.sh/Analytics), not read again here) | On unless turned off, each event listed, a command to turn it off | Say exactly what is sent, and where. Not its default |
| Visual Studio Code ([*Telemetry*](https://code.visualstudio.com/docs/configure/telemetry), not read again here) | On unless turned off, by levels, with a command that shows what is sent | A level the user picks, and a way to see the events |

**The convention the user already knows:** large tools send unless they are told not to.
Local, privacy-minded tools send nothing. Wholeheard's users are the second kind (charter,
*Users and their problem*).

**Why depart from the large tools:** "everything stays on the machine" is the one promise
Wholeheard keeps against a bot (charter, *Risks*). Sending anything by default would break it.

---

## Options considered

| Option | What the user experiences | Cost | Chosen? |
|---|---|---|---|
| Do nothing | The charter's line allows telemetry by default, which nobody decided, and rule 1 contradicts it | 0 | |
| Out of scope, with no condition | Nothing is ever sent; the project learns from asking | One line struck | |
| **Possible, off unless the user turns it on** | Nothing sent unless they choose it; what would be sent is shown first | This record; any later build | ✓ |

## Decision

1. **Telemetry is possible, never on by default.** It is sent only after the user turned it on,
   in a place where they see what would be sent and to which host. They can turn it off at any
   time. An update never turns it on.
2. **What it may carry**: what ADR-0007 lets a log carry — events, durations, sizes, counts,
   model names, error kinds — and the system and version. **Never anything of a meeting**
   (*Out of scope*).
3. **Rule 1 of `docs/project/rules/security.md` stays, reworded**: "no telemetry" becomes "no
   telemetry the user did not turn on". A host the user did not configure is still refused,
   unless the user turned on the sending that names it.
4. **Nothing is built now.** This record lifts an exclusion; it orders nothing.

---

## Expected behaviour

**Acceptance criteria** — for the deliverable that builds it, if one is framed:

- [ ] Given a fresh install, when the user records, transcribes and summarises a meeting and
      never opens the setting, then no connection is opened to a host they did not configure
- [ ] Given the setting, when the user reads it before turning it on, then it shows the host
      and an example of what would be sent
- [ ] Given the sending turned on, when a meeting is recorded, then nothing sent holds a word,
      a title, a name or a sample of it

---

## Success criterion

> We will consider this was the right call if, **by 2027-06-30**, no user has reported anything
> sent that they did not turn on, and the project has decided whether to build it from what it
> would answer: the charter's criteria, and its second purpose.

How it is observed: the project's issues, and the colleagues asked at each cycle's closure.

If the criterion is not met: the exclusion comes back, with no condition.

---

## Removal condition

> This permission is withdrawn **the first time something is found sent that the user did not
> turn on**, whatever the cause.

---

## Impacts

- **The charter**: its *Out of scope* line "Telemetry, even opt-in, while the repository is
  private" is struck and points here, in the pull request that accepts this record.
- **PDR-0001**: its *Out of scope* "Telemetry, still" now reads through decision 1.
- **Rule 1** of `docs/project/rules/security.md`: reworded as decision 3, in the pull request
  that accepts this record.
- **Existing users**: none. Nothing changes in what Wholeheard does.
