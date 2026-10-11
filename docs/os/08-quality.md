# 08 — Quality and risk

> This document is the **reference**. The matching operational rules live in
> `playbooks/`, loaded on demand by an agent.

---

## 1. The common principle

Every discipline in this document follows the same rule:

> **You test, secure and instrument according to risk — never according to an arbitrary
> numeric target.**

A project at 90 % coverage where no critical business rule is tested is less safe than
one at 40 % that covers the high-impact journeys. A numeric target moves effort towards
what is easy to cover, which is to say towards what matters least.

---

## 2. Test strategy

### Priorities

In decreasing order, whatever the type of test:

```
1. business rules
2. critical journeys
3. permissions and authorisations
4. contracts
5. error handling
6. edge cases
7. regressions that already happened
8. behaviours with a high user impact
```

### Types and use

| Type | Useful for | Trap |
|---|---|---|
| Unit | Business rules, edge cases, pure computation | Testing implementation details that break at every refactor |
| Integration | Interaction with the database, the filesystem, a service | Becoming a disguised E2E, slow and fragile |
| Contract | Inter-module boundaries | Absent → the contract is only a document |
| Component | An isolated UI unit | Over-mocking: you test the mock |
| E2E | Critical journeys only | Multiplying them → a slow, flaky suite |
| Visual regression | Design system, stable components | False positives people end up ignoring |
| Accessibility | Every user surface | Check what can be automated only; the rest is tested with the keyboard |
| Performance | Declared budgets | Without an explicit budget, it measures nothing |
| Security | Input, authz, dependencies | Does not replace secure design |
| Smoke | Post-deployment | Too broad → you do not know what broke |
| UAT | Fit with the real need | Used as a substitute for technical tests |

### Flaky tests

A flaky test is an **engineering problem**, never a fact of life.

Its real cost is not the time lost re-running it: it is that it teaches the team to
ignore a CI failure. A single tolerated flaky test degrades the value of the **whole**
suite.

Handling: isolate, diagnose, fix or delete. Never "re-run until it passes". A test
disabled "temporarily" carries an issue and a date.

Tests must be as deterministic as possible: no dependency on the real clock, on execution
order, on the network, or on shared state that is not reset.

---

## 3. QA — before the code

QA starts at framing, not at delivery. For every significant feature, look systematically
for:

```
happy paths · edge cases · invalid input · permissions
unexpected states · concurrency · network errors · external errors
missing or partial data · possible regressions
accessibility problems · compatibility problems
```

**Acceptance criteria must be testable.** A criterion such as "the interface feels
smooth" is not a criterion: it is an intention. It has to be translated into an
observable behaviour, or acknowledged as unverifiable and owned as such.

It is the same requirement as the workflow's oracle (`05-workflow.md`), seen from the
product side.

---

## 4. UX/UI

For every feature exposed to a user, analyse at least:

| Dimension | Question |
|---|---|
| Journey | Does the user reach their goal without a detour? |
| Understanding | Do they know what is happening and what is expected of them? |
| Feedback | Does every action get a perceptible response? |
| Errors | Does the message say what to do, not only what failed? |
| Loading | Are the waiting states handled? |
| Empty states | Is the first use guided? |
| Responsive and mobile | Does the behaviour hold outside the development machine? |
| Accessibility | Keyboard, contrast, screen readers, touch targets |
| Consistency | Does it match the existing design system? |
| Perceived performance | The felt time, not the measured time |

> **"The interface works" is not the same as "the experience is decent".**

The Prior Art Gate applies fully here, and this is even its most profitable ground: on
interface patterns the convention is almost always the right choice, because its value
comes precisely from the fact that the user already knows it. An unrequested interface
innovation is an imposed learning cost.

---

## 5. UAT

UAT validates that the product **actually** answers the expected need.

It **does not replace** unit, integration, security or technical tests. A UAT used as a
technical safety net is the symptom of an insufficient test suite — and it arrives too
late and costs too much for that role.

It starts from the acceptance criteria of the PDR or the issue and from real user
journeys, not from free exploration of the interface.

The test sheet (`05-workflow.md` §7) is the everyday form of acceptance: the acceptance
criteria turned into scenarios, run by someone other than the author, each with its
evidence. UAT remains for `critical` modules, on top of it.

---

## 6. Security — secure by design

An approach through design, not through a final inspection. Depending on the risk,
consider:

```
authentication · authorisation · least privilege
secrets · input validation · sensitive data · confidentiality
logging · audit · dependencies · supply chain
network exposure · injections · file handling
rate limiting · isolation · backups · disaster recovery
```

**Absolute rules:**

- **No secret in the repository.** Ever. Detection is automated, and a leak triggers a
  rotation, not just a removal from the commit.
- **All external input is hostile** until validated.
- **Least privilege by default**, including for agents and for CI.
- The checks that can be automated are **built into the delivery cycle**, not run
  occasionally.

The operational detail is in `playbooks/security.md`, loaded as soon as a task touches
authentication, authorisation, secrets, personal data or external input.

---

## 7. Reliability and operations

A module destined for production must be **operable**. Depending on its criticality:

| Capability | standard | high | critical |
|---|---|---|---|
| Structured logs | ✔ | ✔ | ✔ |
| Explicit error handling | ✔ | ✔ | ✔ |
| Health checks | ✔ | ✔ | ✔ |
| Metrics | — | ✔ | ✔ |
| Traces | — | as needed | ✔ |
| Alerts | — | ✔ | ✔ |
| Explicit timeouts | ✔ | ✔ | ✔ |
| Controlled retries | case by case | ✔ | ✔ |
| Idempotence | case by case | ✔ | ✔ |
| SLI / SLO | — | as needed | ✔ |
| Runbook | — | ✔ | ✔ |
| Tested rollback | — | ✔ | ✔ |
| Controlled degradation | — | as needed | ✔ |

> **Never add automatic retries without analysing the side effects.** A retry on a
> non-idempotent operation duplicates. A retry without backoff turns a local incident
> into a general outage. A retry that masks an error prevents it being detected.

Detail: `playbooks/operations.md`.

---

## 8. Data

**Each domain owns its data.** No implicitly shared data, no direct access to another
module's database — it is the hardest form of coupling to undo, because it is invisible
in the code.

Every significant change to data or to a schema takes into account:

```
compatibility (do the old and new versions of the code coexist?)
migration (how? how long? blocking?)
existing data (what happens to non-conforming rows?)
rollback or recovery strategy
performance during the migration
integrity · security · observability
```

> **A data migration is a production change, not a code change.** It falls under
> high-risk actions (kernel §5): the risk named, the impact described, a safe procedure
> proposed, confirmation asked for.

The schema evolves through the same expand/contract logic as contracts
(`03-contracts.md`): add, let both coexist, migrate, remove. Never rename in place.

Detail: `playbooks/data-migration.md`.

---

## 9. Dependencies

Before adding a significant dependency, apply the **niche filter**
(`06-decisions.md` §3): adoption, maintenance, licence and security, **exit strategy**.

Two symmetrical mistakes, each as expensive as the other:

| Mistake | Example | Cost |
|---|---|---|
| A dependency for something trivial | A library for three lines of code | Attack surface, supply chain, maintenance |
| Reimplementing the non-trivial | Cryptography, date parsing, authentication | Subtle bugs, vulnerabilities, wasted time |

The dividing line: **is this problem subtle?** Formatting a string is not. Time zones,
cryptography, parsing and authentication are — you take the established convention,
always.

Every significant dependency is declared in the module's manifest, with its exit
strategy.

---

## 10. Architectural changes

Any change that significantly increases coupling, the number of dependencies, the attack
surface, operational complexity, cognitive load, criticality or the cost of migration
must be **explicitly assessed** — an ADR, with the rejected alternatives.

Two rules:

**Architecture evolves in small steps.** Each evolution leaves the system at least as
coherent as before. A big leap that leaves the system incoherent "until we finish" never
quite finishes.

**Avoid permanent intermediate states.** When a new path replaces an old one, the removal
of the old one is planned from the start, with a date and an owner — otherwise both paths
coexist indefinitely, and nobody knows which is authoritative any more.
