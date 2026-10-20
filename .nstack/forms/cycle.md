---
goal: "<One sentence: the outcome of this cycle>"
status: proposed          # proposed | accepted | closed | stopped — the decider accepts
appetite_weeks: 2         # the time the decider WANTS to spend, not an estimate
start: 2026-01-05         # YYYY-MM-DD
end: 2026-01-19           # start + appetite: the first day the cycle is over — the circuit breaker
deliverables:
  - id: D1
    title: "<An outcome a user can observe; for a spike, the question it answers>"
    module: "<module name>"   # null for a spike run outside the code
    state: proposed       # proposed | ready (testable criteria) | in-progress | accepted | deferred | dropped
    acceptance:           # given · when · then, required from ready on: the test sheet starts here
      - "Given <context>, when <action>, then <observable result>"
# outcome: completed      # closed: completed | shipped — stopped: reframed | stopped

> The framework's words: `.nstack/handbook/glossary.md`.
# ended_on: YYYY-MM-DD
---

# Cycle NN — <title>

> Copied to `docs/project/cycles/NN-<slug>.md` at framing (`.nstack/playbooks/framing.md`). One accepted cycle at a
> time; no automatic extension.

## Goal

<Why this outcome, now.>

## Deliverables

One section per deliverable, under its identifier: a session reads the front matter and its
own deliverable's section, never the others. What binds goes here — order, dependencies, its
decisions and their links; what was tried, what it cost and what was refuted goes to the
project's log issue (`.nstack/handbook/06-decisions.md` §9).

### D1

<What binds this deliverable that the front matter cannot hold.>

## Out of scope

<Not done in this cycle, even if time remains.>

## Later

<Candidates for the next framing: what framing left out, and ideas that came up during the
cycle.>

## Open questions

**Blocking:** <none left once the cycle is accepted>

**Not blocking:**

## Closure

<Written at the end: what was delivered, which success criteria moved, what was deferred,
where the next framing starts.>

**Documents brought current:** <the front page, the module map, a module's page — each with
its pull request; or "none", and why nothing a reader sees has changed>
