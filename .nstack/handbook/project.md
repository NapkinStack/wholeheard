# The project's own folder — `docs/project/`

It holds what the framework cannot know about this project: who it is for, when it is finished,
and what is being delivered now. Every agent session reads the charter and the current
cycle before working (`AGENTS.md` §4).

| File | Holds | Written by | Accepted by |
|---|---|---|---|
| `discovery.md` | An idea tested: evidence, value hypothesis, challenge, decision | The framer, then the challenger (`.nstack/playbooks/discovery.md`) | The decider |
| `charter.md` | Users, problem, constraints, risks, success criteria, out of scope | The framer (`.nstack/playbooks/framing.md`) | The decider |
| `cycles/NN-<slug>.md` | One goal, a finite list of deliverables, an appetite, an end date, the closure | The framer | The decider |
| `rules/<subject>.md`, `rules/always.md` | The project's own rules, read with the playbook of the same name; absent until the project has one | Whoever meets the constraint, the decision or the incident | The foundation team |
| `.nstack/forms/` — `discovery.md`, `charter.md`, `cycle.md`, `rules.md` | The forms these files are copied from; they are the framework's and live in its folder | — | — |

```mermaid
flowchart LR
    P["proposed"]:::draft -->|"the decider accepts"| A["accepted<br/>running from start"]:::run
    A -->|"every deliverable accepted"| C["closed<br/>completed"]:::ok
    A -->|"end date reached"| B{"Circuit breaker<br/>the decider chooses"}:::stop
    B -->|"ship what is accepted"| S["closed<br/>shipped"]:::ok
    B -->|"new cycle"| R["stopped<br/>reframed"]:::stop
    B -->|"stop the project"| X["stopped<br/>stopped"]:::stop

    classDef draft fill:#374151,color:#fff
    classDef run fill:#1e3a8a,color:#fff
    classDef ok fill:#065f46,color:#fff
    classDef stop fill:#7c2d12,color:#fff
```

**Legend** — grey: a proposal · blue: the cycle in progress · green: an accepted end ·
red: the circuit breaker and what it can lead to.

## Framed, or unframed

```mermaid
flowchart LR
    U["UNFRAMED<br/>no accepted charter"]:::un -->|"the decider accepts<br/>the charter"| F["FRAMED<br/>cycles bound the work"]:::fr
    U -.-> RU["Every rule applies<br/>except the cycle's"]:::note
    F -.-> RF["Every rule applies"]:::note

    classDef un fill:#374151,color:#fff
    classDef fr fill:#1e3a8a,color:#fff
    classDef note fill:#065f46,color:#fff
```

**Legend** — grey: a project that has framed nothing yet, a stage and not a fault · blue:
a project whose charter is accepted · green: what judges a change in each.

The stage is read from this folder, never declared, and every run that judges names it. An
unframed project is held to its boundaries, its contracts, the scope of its pull requests,
the test sheet and the hooks; only the three rules that bound work inside a cycle are silent,
since there is no cycle. **Framing is worth its price when there is something to bound** —
several people, a deadline, a decider who is not the one typing. It is one-way: once the
charter is accepted, a project between two cycles is framed, and is told which cycle is
missing.

## What CI checks

| Check | When | Rules |
|---|---|---|
| The plan, a fitness function | Every push | Formats; one accepted cycle at a time, under an accepted charter; `end` = `start` + the appetite; acceptance criteria once a deliverable is ready; closures recorded; a discovery decided with its decider and date, a charter only after a go. A charter, a cycle or a deliverable still holding the template's words is refused |
| The pull request check | Every pull request changing a module beyond its description — manifest, `AGENTS.md`, `README.md`, `docs/`; not a framework update — **once the charter is accepted** | An accepted cycle; not past its end date; a ready or in-progress deliverable named — and, when someone holds it, delivered by its holder — or the `out-of-cycle` label with its justification |

## A cycle's progress

Read, never kept: `nstack plan` computes it from the accepted cycle's front matter and says it in
one line, then who holds each deliverable.

```
Cycle 03-the-window: 10 of 14 accepted · 2 in progress · 2 ready · day 5 of 21, 17 left (ends 2026-11-07)
Holds in 03-the-window: 3 held, of which 0 stale (from origin; stale after 7 days without work):
```

*Accepted* counts what the decider accepted; a `dropped` deliverable counts in nothing. From its
end date — the first day the cycle is over — the line says the circuit breaker fell and the
decider chooses, and the pull request check refuses delivery work (K2). The project's own
success — the charter's criteria — is read at the cycle's closure, not here. An agent reporting
progress gives this line, so that one figure circulates.

### On the forge

The same plan, where a person already looks: after every merge to the default branch, and once a
day — so that a new holder shows within a day —, the workflow *Plan on the forge* runs `nstack board`. Each cycle the decider accepted,
closed or stopped is a **milestone** — titled by the cycle file's heading, due on its end date,
its description carrying the line above, counts only — and each deliverable an **issue** in it: its acceptance
criteria as a checklist, its module and state, who holds it, the pull requests that name it.
An issue closes as *completed* when the deliverable is accepted, as *not planned* when it is
dropped, deferred, or left unfinished by a closed cycle.

Nothing is maintained by hand and nothing on the forge is the plan: an issue or a milestone edited
there is put back at the next run, and the run says it updated it — it cannot tell an edit by
hand from the plan moving: the plan is edited in the cycle files, in a pull request. The display
finds its own objects by a marker in their body; an issue or a milestone a person made is never
touched, and a person's milestone already holding a cycle's title is said, not overwritten. What cannot be shown — no token, a refusal, holds unreadable — is said in the
run's summary, never a failed merge. The first run on a project creates an issue per deliverable,
and the repository's watchers get a notification for each.

## Who holds a deliverable

With several contributors, a deliverable is **held** before its first change, so that two of
them never deliver it twice:

```bash
nstack claim D30 --person @alice --session <session>   # held; its first work branch d30-… made, unless one exists
nstack plan                                             # who holds what, since when; what is stale
nstack unclaim D30                                      # given back: by its holder, the decider, anyone once stale
nstack claim D30 --take-over                            # a hold nobody worked on for hold_days
```

The hold is a branch of the shared remote, `nstack/hold/<cycle>/D30`, whose commits are its
events; the remote takes the first of two claims and refuses the second. A hold's last work is
its newest event or commit on a branch `d30-…`; with none for `hold_days` (an answer of the
project, 7 by default) it is **stale**, and anyone may take it over — recorded, with whom it was
taken from; the previous work is kept. The pull request check refuses delivery work for a
deliverable someone else holds (K5): its commits name none of the hold's sessions, and no
author, committer or opener is its person; nobody holds it, it asks nothing. A holder working
from a new session names it on the hold first — `nstack claim D30 --session <it>`. A hold says who is
accountable; it proves nobody's identity.

The charter changes only through a PDR (`.nstack/handbook/06-decisions.md` §4). A cycle is never
extended: past its end date, delivery work stops until the decider records a decision.
