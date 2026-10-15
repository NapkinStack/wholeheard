# Installing and updating the foundation

> **This project was created by `nstack init`**, the command of the NapkinStack engineering
> framework. This page is the manual for the foundation: what to install, what to set on the
> forge, how a new version arrives. The method itself is the rest of `.nstack/handbook/`; the project's
> own front page is its `README.md`, and nothing here belongs on it.

```mermaid
flowchart LR
    I["nstack init<br/>project created and committed"]:::cmd --> G["GitHub repository<br/>checklist applied"]:::team
    G --> D["nstack doctor<br/>workstation and settings"]:::cmd
    D --> X["nstack discover idea.md<br/>then the discovery, with your agent"]:::cmd
    X --> F["Framing with your agent<br/>charter + first cycle"]:::team
    F --> M["nstack new-module<br/>first module"]:::cmd
    M --> W["Work in pull requests<br/>CI: fitness, hooks, secrets, sheet, cycle"]:::team
    W --> U["nstack update<br/>new version, reviewed branch"]:::cmd
    U --> W

    classDef cmd fill:#1f2937,color:#fff
    classDef team fill:#065f46,color:#fff
```

**Legend** — grey: an `nstack` command · green: the work of the team and its agent.
CI decides as soon as the rulesets require it: what does not pass does not merge (see
"Prerequisites").

## Prerequisites

- **Workstation**: uv and git; read access to GitHub (the NapkinStack skeleton) and to
  PyPI.
- **GitHub repository.** Workflows inform; only rulesets and push protection block. What
  each case allows:

| Setting | Public repository | Private, Free plan | Private, Team or Pro |
|---|---|---|---|
| Rulesets: PR, review, CODEOWNERS, required checks | Yes | **No: nothing blocks the merge** | Yes |
| Secret scanning and push protection | Yes | No | Paid Secret Protection option |
| Private vulnerability reporting | Yes | Does not exist | Does not exist |
| CI minutes | Unlimited | 2 000 per month | Per the plan |

`nstack doctor` reads these settings and answers in one word. **Guarded**: the forge refuses
what the checklist asks it to refuse. **Unguarded**: nothing here refuses a merge — and it
keeps apart what your plan forbids, which you can do nothing about, from what is simply not
in place yet. A project that has done everything its plan allows is reported as compliant,
and still told that it is unguarded: the checks below keep telling the truth, and none of
them stops a merge on their own.

Whatever the plan allows, the project receives one more workflow, **Commits on main**: it
fails when a commit reached `main` outside a pull request, and names it. It records; it does
not refuse — on a repository where nothing can refuse, it is the only thing that will ever
mention such a commit, and where a ruleset does refuse, it catches a bypass.

**Where the CI runs.** Every job runs on GitHub's hosted Ubuntu unless the repository
variable `NSTACK_RUNS_ON` names another runner label — a self-hosted runner, for a private
repository whose hosted minutes are spent: `gh variable set NSTACK_RUNS_ON --body
<label>`. Such a runner needs `git` on its path; the workflows install uv themselves. What a
pull request costs there or on hosted minutes: about five jobs across the workflows for a
change to one module, each billed at least a minute, plus what the module's own commands take. Never on a public repository: there, a pull request from a
fork would run its code on that machine — and hosted minutes are unlimited anyway.

## Installation

```bash
# 1. Tooling at the project's NapkinStack version, as CI installs it (prerequisite: uv)
bash .nstack/install-engine.sh

# 2. Hooks, once per clone
pre-commit install

# 3. Frame the project with your agent (nstack discover, .nstack/playbooks/framing.md), then create the first module
nstack new-module billing acme/billing standard

# 4. Check that the guardrails answer
nstack fitness
```

That version is the one in `.copier-answers.yml`; it only changes through `nstack update`,
in a reviewed pull request. A release comes from the registry; a commit, from the framework's
repository, and every run then says an unpublished framework judges it.

## Receiving a new version of NapkinStack

```bash
uv tool install "napkinstack==X.Y.Z" --with-executables-from pre-commit  # the version you want
git switch main                          # an update starts from the default branch
nstack update                            # branch nstack/update-vX.Y.Z, merged and committed
git push -u origin nstack/update-vX.Y.Z  # then the PR, validated by CI
```

**When: at a cycle's boundary** — close the cycle, take the update, then accept the next one.
Upgrading mid-cycle changes the judge while a cycle is being judged.

**Everyone else, once that pull request is merged**, installs the same version — the first
command above. Until then every `nstack` command says, before anything else, that the
workstation is behind the project.

The merge keeps the project's adaptations and never touches the modules' code. A conflict
stays marked in the file, uncommitted: the team decides, and both the hook and CI reject
any remaining marker.

## When the framework is wrong

A message that misleads, a check that misjudges, a page that contradicts a command: it is not
worked around here, it is fixed where it lives — and this project is the proof.

```mermaid
flowchart LR
    D["A defect met<br/>in this project"]:::pr --> C["The framework's sources,<br/>cloned beside the project"]:::fw
    C --> T["A failing test,<br/>then the fix"]:::fw
    T --> J["This project, judged<br/>by the sources"]:::pr
    J --> P["A pull request<br/>to the framework"]:::fw
    P -.->|"while it is reviewed"| R["nstack update --ref<br/>the fix's commit"]:::pr
    P -->|"merged"| U["nstack update --ref<br/>the merged commit, then the release"]:::pr

    classDef pr fill:#065f46,color:#fff
    classDef fw fill:#1e3a8a,color:#fff
```

**Legend** — green: in this project · blue: in the framework's repository · dotted: if the
project needs the fix before its merge.

```bash
git clone <the framework's repository> ../framework    # its address: _src_path in .copier-answers.yml
uv run --project ../framework nstack fitness --root .  # this project, judged by the sources
```

Any command runs that way, with nothing installed. The rest is the framework's own page:
its `CONTRIBUTING.md` says how a fix is tested, proposed and reviewed. While it is reviewed,
`nstack update --ref <commit>` lets this project run on the fix: CI installs that commit, and
every run says an unpublished framework judged it. Once it is merged, the pin moves — to the
commit the framework's default branch received, which the squash made, or to the first release
that carries it. `nstack doctor` says when the pinned commit is on no branch any more.

**A private project stays private.** The framework's repository is public: what is reported
there describes the defect, never the project — no name, no domain, no content.

## To do once on GitHub — otherwise nothing is guaranteed

Workflows **inform**; it is the rulesets that **block**. Without this step the whole
edifice rests on goodwill, which the OS forbids (`.nstack/handbook/07-governance.md`).

Settings → Rules → Rulesets → on the main branch (private repository: Team or Pro plan):

- [ ] Pull request required: no direct push to main
- [ ] At least 1 approving review
- [ ] Code owner review required
- [ ] Stale approvals dismissed when new commits are pushed
- [ ] Required checks: `Fitness functions`, `PR scope and review budget`, `Hooks and secrets`, `Test sheet and cycle`, `Module checks`
- [ ] Bypass list empty: nobody merges around the rules, administrators included
- [ ] Main branch cannot be deleted
- [ ] Main branch history cannot be rewritten: no force push

GitHub offers a check to require only once it has run: open a first pull request, then
select them.

Settings → Advanced Security (private repository: the Secret Protection option):

- [ ] Secret Protection and push protection
- [ ] Private vulnerability reporting, public repository (the `SECURITY.md` channel)

Settings → Actions → General:

- [ ] Allowed actions: GitHub's own, plus `astral-sh/setup-uv`
- [ ] SHA-pinned actions required
- [ ] Workflow approval for every outside contributor
- [ ] Workflow token read-only; Actions neither creates nor approves pull requests

Settings → General → Default branch:

- [ ] Default branch `main`: the branch every workflow and ruleset here watches

Settings → General → Pull Requests — three that need no paid plan and no Administration
permission, so they hold on any repository. Auto-merge is armed on each pull request by its
author, when it is opened:

- [ ] Auto-merge available: a pull request armed for it merges itself once approved and green
- [ ] Squash the only merge method: one pull request, one commit on main
- [ ] Head branch deleted on merge: a merged branch is history, not a place to work

Issues → Labels:

- [ ] Labels `cross-module`, `over-budget` and `out-of-cycle` — they make the exceptions
  visible **and countable** (`.nstack/handbook/10-measurement.md` §3)

Settings → Developer settings → GitHub Apps → New GitHub App, then install it on this
repository — **the identity your agents work under** (`.nstack/handbook/07-governance.md` §7):

- [ ] Permissions: Contents, Pull requests and Issues read and write; Actions and Checks
      read; Workflows write only when agents maintain CI; never Administration
- [ ] No webhook; installable only on this account
- [ ] Its private key stored outside any repository: the workstation's secret store, or
      the CI's secrets
- [ ] The agent's session reaches no human credential

Rather than click through these pages, print the calls that close the distance and run them
yourself — the command reads the repository, writes nothing, and never plans to lower a
setting or to rewrite a ruleset:

```bash
nstack settings
```

Then check, read-only:

```bash
nstack doctor
```

`nstack doctor` exits 0 when everything is read and in place, 1 when a setting is missing,
and 2 when nothing read is missing but something could not be read.

`nstack doctor` is also where a project learns that a newer version of NapkinStack is
published: one line, with its notes and the way to take it — information, never a gap, and
the one question any command asks the registry.

`nstack doctor` reads the token from `GH_TOKEN`, otherwise `GITHUB_TOKEN`: a fine-grained
token, limited to the repository, with the **Administration: read** permission. Without a
token, the GitHub part stays "not verified", never compliant.

---

## What is in this repository

| Path | Role | Who reads it |
|---|---|---|
| `AGENTS.md` | **The kernel**: the rules loaded for every task | Agents, every time |
| `modules/` | The code — one unit of parallelism per folder | Humans and agents, scoped |
| `contracts/` | The **only** channel between modules. Created by the first contract, not before | Producers and consumers |
| `docs/project/` | **The discovery, the charter and the cycles**: who it is for, what is delivered now. Born with the first of them | Everyone, agents first |
| `.nstack/` | **The framework's one folder.** Everything in it arrives by update; nothing outside it does, but the kernel and what the forge reads at the root | The foundation team |
| `.nstack/playbooks/` | Rules loaded **by trigger** (security, data, UX…) | Agents, when triggered |
| `.nstack/forms/` | The forms a command or a playbook copies: charter, cycle, decision, rules | Agents and commands |
| `.nstack/handbook/` | **The handbook**: the why behind every rule | Humans, once |
| `docs/adr/`, `docs/pdr/` | The decisions taken, with their success criterion | Everyone, on demand |
| `.copier-answers.yml` | The NapkinStack version and the creation answers | `nstack update`, the only thing that changes it |
| `.github/` | What makes the rules non-bypassable | GitHub |

> **`.nstack/handbook/` is never loaded into an agent's context.** It is the human reference. An
> agent loads: the kernel, the module's `AGENTS.md`, a playbook when triggered. That is
> the whole difference between a handbook and an operating system.

---

## The commands

```bash
nstack --help        # lists the commands
nstack fitness       # manifests + boundaries + skills + plan + hygiene — before every commit
nstack discover <idea-file>   # starts a discovery, for your agent
nstack plan          # the discovery, the charter and the cycles (docs/project/)
nstack skills        # generates the Claude Code skills from the playbooks
nstack doctor        # the workstation and the forge's settings, read-only
nstack settings      # the calls that would bring the forge's settings to the checklist
nstack new-module <name> <owner> <criticality> [--user-facing]   # --user-facing: a user sees it, which decides a test sheet
nstack new-contract <name>   # a contract, and the first time the module that holds them all
nstack check [module]   # the module's commands.check; default: every module
nstack test [module]    # commands.test, likewise
nstack bootstrap [module]   # commands.bootstrap, when declared
nstack run <module>     # local start, when commands.run is declared
nstack e2e [module]     # end-to-end scenarios, when commands.e2e is declared
nstack clean [module]   # removes a module's build outputs, when commands.clean is declared
nstack pr-check --body-file <file>   # test sheet and cycle, as CI reads them from the pull request
nstack modules --changed-since <base>   # the modules with a file changed, as CI lists them
nstack modules --changed-since <base> --with-contract-sides   # ... and both sides of a changed contract
nstack compat [module] --base <base>    # a contract version consumed or stable: the merged proof
# check, test and run of a module: the `commands` section of its MANIFEST.yaml
```

Every module exposes the **same verbs**, whatever its technology. That is what lets
someone move between modules without relearning anything (`.nstack/handbook/09-platform.md` §2).

---

## Claude Code skills

The playbooks are exposed as skills, so that loading them on demand becomes the runtime's
job rather than a kernel instruction.

```bash
nstack skills           # generates .claude/skills/ from .nstack/playbooks/
nstack skills --check   # checks they are in sync (runs in CI)
```

**`.nstack/playbooks/` stays the source of truth.** The skills are generated, gitignored, and
never edited by hand — you change the playbook, then regenerate. A playbook with no entry
in `.nstack/skills.yaml` fails CI; a generated skill that has drifted fails
`nstack skills --check` locally.

This indirection has a reason: a tool is an **adapter**, never a foundation
(`.nstack/tooling-profile.md`). If the rules existed only as skills, the OS would stop
working with any other agent — and the trigger table of kernel §4 is there for exactly
that case.

Enforcement, on the other hand, **never moves down into a plugin**. An agent-side hook is
fast feedback, not a guarantee: the blocking checks stay in CI
(`.nstack/handbook/07-governance.md` §4).

---

## The three rules to remember

1. **One PR = one module.** A change touching two goes through an expand/contract
   sequence on the contract (`.nstack/handbook/03-contracts.md` §4).
2. **The oracle before the code.** The success criterion is executable and red before the
   first generated line (`.nstack/handbook/05-workflow.md` §3).
3. **Convention is the default choice.** It needs no justification; every deviation is
   justified by an observable user value (`.nstack/handbook/06-decisions.md` §2).

---

## What is not provided — and why

No stack: no language, no application framework, no database. The OS imposes a **selection
method** and a decision trail, not a list of technologies (`.nstack/handbook/06-decisions.md` §7).

A new module therefore declares no command: it is green while it holds only its
description, and `nstack fitness` asks for `check` and `test` with its first file of code.
Wiring them, and `run`, onto your own tooling is your job. The **names** never change, the
**content** is yours.

## What is left to wire after the installation

- [ ] The `check` and `test` commands of every module, in its `MANIFEST.yaml`
- [ ] The contract format chosen, and the contract tests and the comparator behind
      `commands.compat` that go with it
- [ ] The review budget values (400 lines / 15 files are a starting point)
- [ ] Re-read the `description` fields in `.nstack/skills.yaml` — they are what triggers
      the skills, and they must speak your domain's vocabulary
- [ ] The `e2e` command of every user-facing module, and the tools that drive a browser or
      an emulator, in `.nstack/tooling-profile.md`
