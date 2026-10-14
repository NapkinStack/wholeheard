# contracts — local instructions

## Responsibility

Defines and versions the interface contracts between modules.

## Invariants

- A contract **never** contains a producer's internal structure.
- Every deprecated version carries a `removal_date`. Without a date, the pull request is
  refused.
- A breaking change goes through **4 pull requests** (expand/contract), never one.
- A version a module consumes, or marked stable, changes only with the merged `compat`
  command's approval (V1).
- What the comparator compares is a version's `*.schema.json` files (`scripts/compat.py`,
  ADR-0005): everything a producer may write under the changed schema must be something
  the merged one allowed. **A version's pages are not compared**: a promise made in prose
  only is held by review, and by nothing else — say it in a schema when it can be said
  there. A schema is never removed from a version.
- Contract tests run on **both sides**: producer and consumer.

## The main trap

Unchanged structure but **modified semantics** = a breaking change. A new enumeration
value, a change of unit, a nullable that becomes mandatory. No diff detects it; only
contract tests do.

## Before changing anything

Check the consumer matrix, generated from the manifests. A version is only removed once
`consumers = 0`.
