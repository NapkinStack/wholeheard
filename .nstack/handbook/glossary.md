# Glossary

The words of this framework, in plain terms: what each means, where you meet it, and an
example. A rule's code — `T5`, `K3`, `P1` — is explained in [`RULES.md`](RULES.md), one line
each. A word of your product's domain belongs to your charter, not here.

An agent speaking to a person uses these words, and explains one the first time it uses it.

---

## Who

| Term | What it means | Where you meet it | Example |
|---|---|---|---|
| **Decider** | The person who owns the project: accepts the charter, the cycles and the deliverables, and chooses at a cycle's end | `decider:` in the charter; every pull request that needs a human word | "The decider accepted D29 after trying it on their laptop." |
| **Framer** | An agent that proposes the charter, a cycle and its deliverables; it never decides | The framing playbook | "The framer proposed three deliverables; the decider kept two." |
| **Challenger** | Another agent session, or a person, that attacks a discovery before anyone builds on it | `challenger:` in a discovery | "The challenger found the market already had a free tool." |
| **Author** | The agent that writes a change — one deliverable, in one module | A pull request's commits | "The author fixed the parser and its test." |
| **Verifier** | A session or a person who is not the author and runs the change's test sheet | `Verifier:` in a pull request's description | "The verifier ran S2 again and saw it fail on the head." |
| **Approver** | The person who approves a pull request on the forge, on its evidence | The forge's *Approve* | "The approver read the sheet, then approved." |
| **Owner** | The team or person a module, or the project's rules, belong to | `owner:` in a manifest; `CODEOWNERS` | "The billing module's owner is acme/billing." |
| **Posture** | Which of the roles above someone holds for a task; the author never verifies or approves its own work | `05-workflow.md` §10 | "One agent was the framer on Monday and the author on Tuesday." |
| **Session** | One run of an agent, named by an identifier its commits carry as `Agent-Session` | A commit's trailer; `Verifier: session …` | "Agent-Session: billing-d12-parser" |

## The work

| Term | What it means | Where you meet it | Example |
|---|---|---|---|
| **Discovery** | An idea tested before anything is built: evidence, a hypothesis, a challenge, then a decision | `docs/project/discovery.md` | "The discovery of an export tool ended in kill." |
| **go · clarify · kill** | A discovery's decision: build it, ask more, or drop it | `decision:` in a discovery | "go, with a smaller first cycle." |
| **Charter** | The project's founding page: users, problem, constraints, success criteria, what is out | `docs/project/charter.md` | "The charter says: used at every meeting for two weeks." |
| **Framed · unframed** | A project with an accepted charter is framed and its work is bounded by cycles; before that it is unframed — a stage, not a fault | The first lines of `pr-check` and `plan` | "Unframed: the cycle rules say nothing yet." |
| **Cycle** | A bounded stretch of work: one goal, a few deliverables, an appetite, an end date | `docs/project/cycles/` | "Cycle 03, the window, from 17/10 to 07/11." |
| **Appetite** | How long the decider wants to spend on a cycle — a choice, not an estimate | `appetite_weeks:` in a cycle | "An appetite of three weeks." |
| **Circuit breaker** | A cycle's end date: past it, the work stops and the decider chooses to ship, reframe or stop | `end:` in a cycle; rule K2 | "The circuit breaker fell; the last deliverable was deferred." |
| **Deliverable** | Something a user can observe, with acceptance criteria, in one module | `D<n>` in a cycle | "D18: a user reads one meeting's minutes beside its transcript." |
| **proposed · ready · in-progress · accepted · deferred · dropped** | A deliverable's states: drafted, testable and waiting, being built, accepted by the decider, moved to later, abandoned | `state:` of a deliverable | "D26 is ready: its criteria can be tested." |
| **proposed · accepted · closed · stopped** | A cycle's states, and a charter's first two | `status:` of a cycle or the charter | "Cycle 02 closed with two deliverables accepted." |
| **Oracle** | The executable success criterion of a task, written and seen failing before the change | The *Oracle* section of a pull request | "The oracle: a test that a refused key says why." |
| **Test sheet** | The scenarios of a change, written before the code and run by a verifier, each with its evidence | *Test sheet* in a pull request's description | "Three scenarios, all passed on the head." |
| **Scenario** | One row of a test sheet: given, when, then — **automated**, **explored** by hand, or **human only** | A test sheet's rows | "S5, explored: the button stays reachable on a phone." |
| **Evidence** | What shows a scenario's result: a run, a log, a screenshot, with the commit it was produced on | The *Evidence* and *Commit* columns | "The CI run's trace, at e4f5a6b." |
| **Acceptance** | A person trying a deliverable themselves, and saying what they saw | An issue labelled `acceptance` | "Acceptance on Ubuntu: installed from one package." |

## Modules and contracts

| Term | What it means | Where you meet it | Example |
|---|---|---|---|
| **Module** | A part of the system that can be understood, changed, tested and validated without the rest | `modules/<name>/` with its manifest | "The capture module records; it knows nothing of minutes." |
| **Manifest** | A module's identity card: owner, criticality, contracts provided and consumed, commands | `MANIFEST.yaml` | "The manifest says the module consumes transcript v1." |
| **Contract** | A versioned interface between two modules — the only way they talk | `contracts/<name>/v<n>/` | "recording v1: the events the recorder prints." |
| **Provides · consumes** | A module that publishes a contract version, and one that relies on it | `provides:` and `consumes:` in manifests | "The shell consumes summarising v1, which summary provides." |
| **Expand / contract** | The sequence of pull requests that changes a contract without breaking its consumers | `03-contracts.md` §4 | "Add v2, move the consumers, then remove v1." |
| **Criticality** | What a module's failure costs, as the decider settles it; it sets what each change of the module must show | `criticality:` in a manifest | "Capture is critical: a lost recording is lost for good." |
| **prototype · standard · high · critical** | The criticality values: a failure costs nothing (thrown away), a user's patience, a day of work, or a key, an order, money, personal data | `criticality:` in a manifest; `05-workflow.md` §7 | "From high, a runbook is required." |
| **User-facing** | A module a user sees; from standard up, its changes need a test sheet | `user_facing:` in a manifest | "The window is user-facing." |
| **Lifecycle** | A module's stage, from its first idea to its removal | `lifecycle:` in a manifest | "export is proposed: nothing built yet." |
| **proposed · active · maintenance · deprecated · retired** | The lifecycle values: planned, in use and changing, kept but no longer extended, being removed by a date, gone | `lifecycle:` in a manifest; `02-modules.md` §6 | "deprecated, removal_date 2027-01-31." |

## Rules and checks

| Term | What it means | Where you meet it | Example |
|---|---|---|---|
| **A rule's code** | A letter for a family and a number, naming one rule the engine applies; each is one line of `RULES.md` | `FAIL [T1]`, `WARNING [K3]` in a command's output | "T1: a change owes a test sheet and carries none." |
| **Fitness function** | An automated check that fails when the architecture drifts — a forbidden dependency, a boundary crossed | `nstack fitness`; families M, B, S, C, H | "B2 refused a read into another module's folder." |
| **Review budget** | The most a pull request may change — lines, files — so that a person can review it | `nstack pr-scope`; rule P2 | "412 lines of 400: over budget." |
| **cross-module · over-budget · out-of-cycle** | The labels that allow an exception — several modules, a large change, work outside the cycle — each with its reason, and counted | A pull request's labels | "out-of-cycle: a production incident." |
| **Guarded · unguarded** | Whether the forge can refuse a merge that breaks the rules; on some plans nothing can | `nstack doctor` | "Private on a free plan: unguarded." |
| **Checked · attested** | A change's checks ran on the forge's CI, or on a workstation and were recorded on the commit | `Checks :` in `pr-check`; `nstack attest` | "Attested on a workstation, 5 of 5." |
| **Published · unpublished** | Whether the engine judging a project is a released version or a local copy | The first line of every judging command | "Judged by an UNPUBLISHED NapkinStack — a local checkout." |
| **Sources** | A contributor's engine: a local checkout of the framework that holds the project's version, ahead of what CI installs — said by `doctor`, not a gap | `SOURCES [L1]` in `nstack doctor` | "Sources at v0.11.0-3: ahead of the project's v0.11.0." |
| **Prior art gate** | Before a structuring decision: find how the field does it, adopt that unless a user gains from another way | The *Prior art* section of an ADR or a PDR | "Copier, because the field already uses it." |

## Records and files

| Term | What it means | Where you meet it | Example |
|---|---|---|---|
| **ADR · PDR** | A record of a technical decision (architecture) or a product decision, kept for whoever comes later | `docs/adr/`, `docs/pdr/` | "A PDR says why the export was dropped." |
| **Kernel** | The short instructions every agent session reads first | `AGENTS.md` at the root | "The kernel says: never verify your own work." |
| **Playbook** | Instructions an agent loads for one kind of task | `.nstack/playbooks/` | "The verification playbook." |
| **Skill** | A playbook as the agent's tool loads it, generated from it | `.claude/skills/`, `nstack skills` | "The skills are regenerated, never edited." |
| **Handbook** | The framework's manual, kept in the project | `.nstack/handbook/` | "07-governance.md says when the forge cannot guard." |
| **Form** | A blank page to copy and fill — a charter, a cycle, an ADR | `.nstack/forms/` | "The cycle form, copied at framing." |
| **Seat** | A file whose content is the project's own; an update never writes into it | `README.md`, the decision indexes | "The README is a seat: the update leaves it." |
| **Skeleton** | The framework's files a project receives and owns — kernel, playbooks, handbook, CI, hooks, forms | `nstack init`, `nstack update` | "The update merged the skeleton's new handbook page." |
| **Engine** | The versioned tool that creates and updates projects and runs the checks | `nstack`, its version on every judging run | "nstack 0.10.0." |
| **Answers** | The project's choices recorded at creation, with the framework's version | `.copier-answers.yml` | "checks: forge." |
