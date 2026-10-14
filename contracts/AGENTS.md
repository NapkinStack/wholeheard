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
  there. A schema is never removed from a version, and a version that holds none cannot
  change at all: its first schema comes first.
- A schema is written so that the comparator can decide (its first verification, 2026-10-14):
  JSON Schema 2020-12, one self-contained file a document, at the version's top, named
  `<document>.schema.json`; `$ref` only to its own `#/$defs/`; **each kind of line, in a
  union, says `"type": "object"` and requires what tells it from the others** — a union
  that leans on a `type` or a `required` beside it is refused on any change; an object left
  open (`additionalProperties` unsaid) wherever a field may be added later, closed where
  none ever may; a date as a `pattern`, right the first time — a `format` is not read, and a
  pattern cannot be changed in place. What it cannot decide, it refuses: a harmless change
  refused costs a new version, never a consumer.
- Contract tests run on **both sides**: producer and consumer.

## The main trap

Unchanged structure but **modified semantics** = a breaking change. A new enumeration
value, a change of unit, a nullable that becomes mandatory. No diff detects it; only
contract tests do.

## Before changing anything

Check the consumer matrix, generated from the manifests. A version is only removed once
`consumers = 0`.
