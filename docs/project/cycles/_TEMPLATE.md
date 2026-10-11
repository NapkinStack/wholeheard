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
# ended_on: YYYY-MM-DD
---

# Cycle NN — <title>

> Copied to `NN-<slug>.md` at framing (`playbooks/framing.md`). One accepted cycle at a
> time; no automatic extension.

## Goal

<Why this outcome, now.>

## Deliverables

<Notes the front matter cannot hold: order, dependencies.>

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
