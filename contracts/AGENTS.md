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
- **The comparator promises one thing: no break goes through.** It does not promise that
  every harmless change does — what it cannot decide, it refuses, and a harmless change
  refused costs a new version, never a consumer. A schema is written the way its two
  verifications found it decides (2026-10-14), as `tests/test_compat.py`'s own is: JSON
  Schema 2020-12, said in `$schema`; one self-contained file a document, at the version's
  top, named `<document>.schema.json`; the kinds of a line in a `oneOf`, **each an object
  that requires what tells it from the others**, never leaning on a `type` or a `required`
  beside the union; a shared field by its own `$ref` to `#/$defs/`, never a `$ref` or an
  `allOf` to a whole shared object; an object left open wherever a field may be added
  later; text a field must keep on one line by what it must not contain, `not` a
  `pattern` of `[\n\r]` — never an anchored `^…$`, which lets a final line feed through
  in Python's reading; an instant's shape left to the page, as `session-audio`'s are —
  `format` is not read, and a pattern cannot be loosened in place. **A schema enters a
  version with a test that runs its own harmless changes through the comparator**: that it
  decides them is shown for that schema, not supposed of all.
- An example beside a schema is a document or lines (`examples/…json`, `…jsonl`): it does
  not say `$schema`. A file that does, anywhere but at the version's top, is refused.
- Contract tests run on **both sides**: producer and consumer.

## The main trap

Unchanged structure but **modified semantics** = a breaking change. A new enumeration
value, a change of unit, a nullable that becomes mandatory. No diff detects it; only
contract tests do.

## Before changing anything

Check the consumer matrix, generated from the manifests. A version is only removed once
`consumers = 0`.
