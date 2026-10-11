# <title: what this changes, from the behaviour's point of view>

Closes #

---

## Module

`modules/<name>` — **exactly one**.

- [ ] This pull request touches a single module
- [ ] ⚠️ Cross-module — the `cross-module` label is required, justification below

<Justification when cross-module. Reminder: a contract change goes through an
expand/contract sequence, not a single PR (`docs/os/03-contracts.md`).>

## Deliverable

Deliverable: D<n>

> A deliverable of the current cycle (`docs/project/cycles/`), ready or in progress. Work
> outside the cycle — an incident, a production defect — carries the `out-of-cycle` label,
> and replaces the line above with `Out of cycle: <reason>`. A pull request that changes no
> module beyond its description — manifest, `AGENTS.md`, `README.md`, `docs/` — such as a
> framework update, is not delivery work: the line does not apply.

## What it changes

<The behaviour, not the list of files. The diff already says what changed; this section
says why.>

## Oracle

> `docs/os/05-workflow.md` §3

- [ ] The success criterion was written **before** the implementation
- [ ] It was seen **failing**, for the right reason
- [ ] It passes now

Kind of oracle: unit test · integration · contract test · fitness function · budget ·
other: <specify>

---

## Review budget

> `docs/os/05-workflow.md` §4

- Lines changed (excluding generated): <n>
- Files touched: <n>
- [ ] Within budget
- [ ] ⚠️ Over budget — label required, justification: <generation, mechanical migration,
  mass rename…>

---

## Contracts

- [ ] No contract affected
- [ ] Contract consumed — the version is declared in the manifest
- [ ] Contract changed, **additive** — contract tests green, and `nstack compat` proving it
      once the version is consumed or stable
- [ ] Contract changed, **breaking** — expand/contract step no. <1|2|3|4>, ADR linked,
  removal date set

---

## Definition of Done

> Tick what is **applicable** for the module's criticality
> (`docs/os/05-workflow.md` §7). Never tick a box without having actually run it.

- [ ] Oracle green
- [ ] Lint, format, types
- [ ] Unit tests
- [ ] Build
- [ ] Contract tests *(when there is a contract)*
- [ ] Fitness functions
- [ ] Integration tests *(per risk)*
- [ ] Security analysis
- [ ] Accessibility *(when there is UI)*
- [ ] E2E on critical journeys *(criticality high and above)*
- [ ] Observability added *(criticality high and above)*
- [ ] Rollback verified *(criticality critical)*
- [ ] Affected documentation updated
- [ ] Diff self-reviewed line by line

---

## Test sheet

> Required when the pull request changes a user-facing module, or one of criticality high
> or critical, beyond its description — manifest, `AGENTS.md`, `README.md`, `docs/`
> (`docs/os/05-workflow.md` §7). The scenarios come from the acceptance
> criteria and are written before the code; the results are filled in by a verifier who is
> not the author (`playbooks/verification.md`). Kind: automated · explored · human only —
> reason. Result: passed · failed — what was observed · not verified.

Verifier: <@handle, or session and the agent session's identifier>

| # | Given · when · then | Kind | Result | Evidence | Commit |
|---|---|---|---|---|---|
| S1 | <Given …, when …, then …> | <automated · explored · human only — reason> | <passed · failed · not verified> | <link> | <commit> |

---

## Summary

> An imposed format. The last three sections are the most important and the most often
> skated over. (`docs/os/05-workflow.md` §8)

**DONE**
<one sentence per change>

**VERIFIED**
<checks actually run, with their result>

**ASSUMED**
<assumptions taken for lack of information>

**NOT VERIFIED**
<what was not tested, and why>

**RISKS**
<possible side effects, debt introduced, follow-ups needed>

---

## Signals to report

- [ ] I had to look at another module's implementation → **a signal of a bad boundary**
  (`docs/os/02-modules.md` §9)
- [ ] A kernel or playbook rule got in my way for no good reason
- [ ] A quality gate blocked without improving quality
- [ ] A rule applied by hand would be worth automating
  → the automation backlog (`docs/os/07-governance.md` §9)
