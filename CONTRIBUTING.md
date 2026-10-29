# Contributing

> **Outside contributions are not merged for now.** The code is public under GPL-3.0
> ([`LICENSE`](LICENSE)); it stays the project's alone until a contributor agreement exists
> ([PDR-0002](docs/pdr/0002-the-repository-opened-the-pace-kept.md), 2026-10-25). An issue — a
> defect, an idea — is welcome.

## Joining the project

Once per person, before a first deliverable. Each step says where it is said in full.

1. **Access.** The decider invites you on the forge. When you, or your team, will own a
   module, a pull request names the owner in its `MANIFEST.yaml` — and in `.github/CODEOWNERS` when a
   human approves it (step 6).
2. **The engine, at the project's version, and the hooks** (prerequisites: uv and git):

   ```bash
   bash .nstack/install-engine.sh   # once per workstation: the version .copier-answers.yml records, pre-commit with it
   pre-commit install               # once per clone
   ```

3. **Your agent's identity.** Any coding agent works: it reads `AGENTS.md` at the root and
   follows it. It works under an identity of its own — a GitHub App (`.nstack/handbook/INSTALL.md`,
   *the identity your agents work under*) — never with your credentials. Creating it and installing it on
   the repository is a person's act: yours where you may, else the decider's on your
   request; its key stays outside any repository. Each session your agent runs has a name — any
   opaque label, `alice-d30` — that goes on its commits (`Agent-Session:`) and its holds (`--session`).
   Where the project answers `approval: agent`, each agent needs an identity of its
   own (step 6).
4. **Check the workstation**: `nstack doctor` — the engine, the hooks, the project's version. The
   forge's settings stay *not verified* without a token that may read them: that part is the
   decider's.
5. **See the work**: `nstack plan` — the cycle, its ready deliverables, who holds what. Then
   *Before you start*.
6. **Know who approves your changes**: the module's owner, a human — or, where the project
   answers `approval: agent`, for a `prototype` or `standard` module, a human or another
   agent — yours or a colleague's — under an identity of its own: an identity never approves what
   it pushed. Everything else waits for a human code owner (`.nstack/handbook/07-governance.md` §7).
7. **When the method gets in your way** — a message that misleads, a check that misjudges, a
   page that contradicts a command — say it, never work around it: an issue here when it is the
   project's; when it is the framework's, an issue at its repository — `_src_path` in
   `.copier-answers.yml` — with what happened, what you expected and the exact command. That
   repository is public: describe the defect, never the project (`.nstack/handbook/INSTALL.md`,
   *When the framework is wrong*).

## Before you start

1. Pick a **ready** deliverable of the current cycle (`docs/project/cycles/`) and **take it**:
   `nstack claim D<n> --person @you --session <its session>` — or an issue, for what nobody planned: a defect, a debt. A project
   with no accepted charter has no cycle yet: pick the work, the other rules apply
   (`.nstack/handbook/project.md`).
2. Identify **the** target module — exactly one.
3. Read its `MANIFEST.yaml`, then its `AGENTS.md`.
4. `nstack bootstrap <name> && nstack check <name>`

## Working with several contributors

```mermaid
flowchart LR
    D["The decider<br/>accepts the cycle"]:::who --> R["Ready deliverables<br/>one module each"]:::work
    R -->|"nstack claim D30"| A["Alice and her agent<br/>hold D30 · module capture"]:::held
    R -->|"nstack claim D31"| B["Bob and his agent<br/>hold D31 · module export"]:::held
    A --> PA["pull requests d30-…<br/>approved by capture's owner"]:::work
    B --> PB["pull requests d31-…<br/>approved by export's owner"]:::work
    classDef who fill:#374151,color:#fff
    classDef work fill:#1e3a8a,color:#fff
    classDef held fill:#065f46,color:#fff
```

**Legend** — grey: who decides · blue: the work and its review · green: a deliverable held.

- **The work is shared by module.** Each module has an owner — `owner:` in its
  `MANIFEST.yaml`, and `.github/CODEOWNERS` where a human approves it — who answers for its changes; a deliverable belongs
  to one module. Two contributors in two modules never meet but through a contract
  (`.nstack/handbook/02-modules.md`, `03-contracts.md`).
- **A deliverable is taken before its first change**, so that nobody delivers it twice:
  `nstack claim D30 --person @you` — and an agent adds `--session <its session>`. `nstack plan`
  says who holds what; a hold nobody works on goes stale and may be taken over
  (`.nstack/handbook/project.md`, *Who holds a deliverable*).
- **Any coding agent works**, under an identity of its own (*Joining the project*); the skills
  `nstack skills` makes under `.claude/skills/` are one tool's shortcut to the same playbooks
  (`.nstack/playbooks/`).
- **Who checks whom.** The author never verifies nor approves their own work: a verifier
  runs the test sheet, then the approver of *Joining the project*, step 6, approves
  (`.nstack/handbook/05-workflow.md` §10).

## The barriers against leaks

```mermaid
flowchart LR
    C["git commit"] --> H{"1 · pre-commit hook<br/>gitleaks + checks"}
    H -->|refused| X1["Commit blocked"]
    H -->|ok| P["git push"] --> PP{"2 · GitHub push<br/>protection"}
    PP -->|refused| X2["Push blocked"]
    PP -->|ok| PR["Pull request"] --> CI{"3 · CI 'Hooks and secrets'<br/>same hooks + full history"}
    CI -->|red| X3["Merge impossible"]
    CI -->|green| M["main"]

    classDef local fill:#374151,color:#fff
    classDef server fill:#065f46,color:#fff
    classDef stop fill:#7c2d12,color:#fff
    class H local
    class PP,CI server
    class X1,X2,X3 stop
```

**Legend** — grey: on the workstation, bypassable · green: GitHub side · red: a stop.

Only barriers 1 and 2 act **before** publication. A secret caught by CI is already
public: that is an incident, and it is revoked immediately (`.nstack/playbooks/security.md`).

## While you work

- **Write the oracle first**, watch it fail, then implement
  (`.nstack/handbook/05-workflow.md` §3).
- **Minimal change.** No opportunistic refactoring: it gets its own pull request.
- **Stay in the module.** Need another one? Go through its contract. The contract is not
  enough? That is a contract change, so an expand/contract sequence
  (`.nstack/handbook/03-contracts.md` §4) — never a single pull request.
- `nstack fitness` before every commit.

## Opening the pull request

The template applies the Definition of Done. Three things not to skate over:

- **The summary** — `DONE / VERIFIED / ASSUMED / NOT VERIFIED / RISKS`.
  A summary with nothing under `ASSUMED` and `NOT VERIFIED` is almost always incomplete.
- **The test sheet** — when one is due (`.nstack/handbook/05-workflow.md` §7: a module a user sees,
  from `standard` up, and every `high` or `critical` one): scenarios written before the code,
  results filled in by a verifier who is not you, each with its evidence
  (`.nstack/playbooks/verification.md`).
- **The signals to report** — that is how the system improves
  (`.nstack/handbook/10-measurement.md`).

## Exceptions

| Label | When | Consequence |
|---|---|---|
| `cross-module` | A PR touching several modules, with a justification | Allowed, counted |
| `over-budget` | Generation, mechanical migration, mass rename | Allowed, counted |
| `out-of-cycle` | An incident, a production defect, with "Out of cycle: <reason>" | Allowed, counted |

Exceptions are **visible**, never silent. Their rate is a health indicator for the
boundaries.

## What is not negotiable

- No quality gate bypass: no `skip`, no `--no-verify`, no test disabled without an issue
  and a date.
- No secret in the repository, in any form.
- Never tick "tests passing" without having run them.
