# Modules

One autonomous unit of context and of parallelism per folder.

> **Fundamental rule.** A developer or an agent must be able to understand, change, test
> and validate a module **without understanding the rest of the system**.
> If that is not true, it is not a module: it is a folder.

## Creating a module

```bash
nstack new-module billing acme/billing standard
```

The scaffolding produces the manifest, the local `AGENTS.md`, the CODEOWNERS line, and
activates the fitness functions **from the first commit** — a module created without
guardrails accumulates violations discovered too late. The standard verbs are the team's to
declare, for its stack, before the module's first file of code.

Creating a module is a decision: it goes through an **ADR** (the capability covered, the
boundary, the rejected alternatives).

## Before splitting

A module represents a **coherent capability**, not a table or a handful of endpoints. See
the decision tree in `docs/os/02-modules.md` §3.

Do not split if: the capability is not coherent, the module cannot be tested on its own,
no owner is identifiable, or its rate of change is the same as the rest with no other
explicit reason.

## What is uniform, and what is not

**Uniform**: the verbs (`bootstrap`, `check`, `test`, `run`), the file envelope, the
mandatory checks, the contract format.

**Free**: language, framework, database, internal architecture, patterns.

> You find the same **verbs**, never the same **code**.
