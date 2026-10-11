# AI Engineering OS

A system of rules, boundaries, contracts, documentation and deterministic guardrails that
lets **several teams and their AI agents develop in parallel** without generation speed
blowing up complexity.

---

## The problem this OS solves

Agents make writing code nearly free. Three costs do not come down:

1. **Understanding** — somebody still has to know what the system does.
2. **Verification** — somebody still has to review and validate.
3. **Coordination** — several teams have to move without blocking each other.

A generation accelerator plugged into a system without boundaries does not produce a
faster project: it produces an unreadable project faster.

This OS therefore treats **cognitive load**, **review capacity** and **inter-team
coupling** as the project's three scarce resources, and organises everything else around
preserving them.

---

## The five structuring ideas

| # | Idea | Practical consequence |
|---|------|----------------------|
| 1 | **The prompt is a transit zone, not a residence** | Every automatable rule moves down into CI and leaves the prompt |
| 2 | **The module is the unit of parallelism** | One PR = one module; two modules know each other only through a contract |
| 3 | **The oracle before the generation** | The criterion is made executable before the first line is written |
| 4 | **Convention is the default choice** | It needs no justification; every deviation does |
| 5 | **No stack is imposed** | The OS imposes a selection method, not a list of technologies |

---

## Navigation map

### What to read, depending on who you are

| You are… | Read, in order |
|-----------|---------------------|
| **An AI agent** | `AGENTS.md` (kernel) + the module's local `AGENTS.md` + the triggered playbooks |
| **A new developer** | `00-overview.md` → `02-modules.md` → `05-workflow.md` |
| **A tech lead / architect** | `00-overview.md` → `02` → `03` → `07` |
| **A product owner** | `00-overview.md` → `06-decisions.md` |
| **Whoever creates the project** | The project's `README.md`, then `09-platform.md` |

### Project contents

```
.
├── README.md                     How to use the project
├── AGENTS.md                     ← THE KERNEL: resident, loaded for every task
├── CONTRIBUTING.md, SECURITY.md
│
├── docs/
│   ├── os/                       ← this handbook
│   │   ├── README.md             you are here
│   │   ├── 00-overview.md        OS architecture, the 4 layers, glossary
│   │   ├── 01-principles.md      Non-negotiable principles and anti-patterns
│   │   ├── 02-modules.md         Boundaries, manifest, lifecycle, one PR = one module
│   │   ├── 03-contracts.md       Versioning, expand/contract, contract tests
│   │   ├── 04-ai-context.md      Context firewall, context budget, crossing over
│   │   ├── 05-workflow.md        Verification-first loop, DoR/DoD, review budget
│   │   ├── 06-decisions.md       ADR/PDR, Prior Art Gate, dated success criteria
│   │   ├── 07-governance.md      Where a rule lives, fitness functions, CI, quality gates
│   │   ├── 08-quality.md         Tests, security, reliability, data, UX, QA, UAT
│   │   ├── 09-platform.md        Standard verbs, layout, tooling, onboarding
│   │   └── 10-measurement.md     Metrics, feedback loop, decision review
│   ├── tooling-profile.md        Capabilities → the tools of the moment
│   ├── adr/, pdr/                Decisions, and their _TEMPLATE.md models
│   └── architecture/, runbooks/
│
├── playbooks/                    ← instruction modules loaded on demand
├── modules/                      The code: one unit of parallelism per folder
├── contracts/                    The only channel between modules
└── .github/                      CI, CODEOWNERS, issue and pull request templates
```

---

## How to read these documents

The documents in `docs/` are **the reference**: they explain, justify and detail. They
are **not** meant to be loaded into an agent's context.

What an agent loads is:

- `AGENTS.md` (the kernel, always);
- the `AGENTS.md` of the module concerned (always);
- one or more `playbooks/` (only when triggered).

If you need to put a rule in the kernel, re-read `07-governance.md` § "Where does this
rule live?" first. The answer is very often "in CI", not "in the prompt".

---

## Status

This OS is subject to its own rules: it evolves in small steps, its structuring changes
get an ADR, and every rule it contains is a candidate for automation. See
`10-measurement.md` § "Improving the OS".
