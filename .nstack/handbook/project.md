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
| The pull request check | Every pull request changing a module beyond its description — manifest, `AGENTS.md`, `README.md`, `docs/`; not a framework update — **once the charter is accepted** | An accepted cycle; not past its end date; a ready or in-progress deliverable named — or the `out-of-cycle` label with its justification |

The charter changes only through a PDR (`.nstack/handbook/06-decisions.md` §4). A cycle is never
extended: past its end date, delivery work stops until the decider records a decision.
