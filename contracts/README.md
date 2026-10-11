# Contracts

The **only** channel of communication between modules, and the only point of
coordination between teams.

This folder is a **module in its own right**: it has an owner, a manifest, its own tests
and a high criticality. An orphan boundary decays.

## Structure

```
contracts/
├── MANIFEST.yaml
├── <contract-name>/
│   ├── v1/          ← executable schema, docs, examples, error cases
│   └── v2/
└── tests/           ← contract tests, run on BOTH sides
```

## Evolving a contract

**Additive** (optional field, new endpoint) → a simple pull request, contract tests
green — and, once the version is consumed or stable, the `compat` command confirming it.

**Breaking** → an expand/contract sequence in 4 pull requests, never one
(`docs/os/03-contracts.md` §4):

1. contract v2 declared, additive — v1 untouched, tests for both versions green
2. the producer serves v1 **and** v2
3. each consumer migrates at its own pace, updating its manifest
4. v1 is removed once `v1 consumers = 0`

Every deprecated version carries a **removal date**. A check fails once that date has
passed.

## The trap

An unchanged structure with a **modified semantics** is a breaking change: a field that
gains an unexpected value, a unit that changes, a nullable field that becomes always
filled. No diff tool detects it — only contract tests do.
