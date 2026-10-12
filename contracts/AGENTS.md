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
- Contract tests run on **both sides**: producer and consumer.

## The main trap

Unchanged structure but **modified semantics** = a breaking change. A new enumeration
value, a change of unit, a nullable that becomes mandatory. No diff detects it; only
contract tests do.

## Before changing anything

Check the consumer matrix, generated from the manifests. A version is only removed once
`consumers = 0`.
