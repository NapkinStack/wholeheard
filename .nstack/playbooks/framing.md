# Playbook — Framing

> **Trigger.** Load this playbook when you are touching: framing a project, planning or
> closing a cycle.
>
> **This project's own rules** on the subject, when it has any: `docs/project/rules/framing.md`
> — read them with this playbook. They add to it or tighten it.

---

## When framing is worth it

A project is **unframed** until its charter is accepted: a stage, not a fault. Every rule
judges its changes except the three that bound work inside a cycle. Framing buys an
appetite, an end date, a circuit breaker and a decider — worth their price **when there is
something to bound**: several people, a deadline, a decider who is not the one typing. Say
so when you are asked to frame a project where none of the three is true; then frame it if
the decider still wants it. Never frame to make a check pass: no check asks for it.

## Your posture

You are the **framer**. You propose; the decider decides. For the framer, this playbook
replaces the kernel's working loop (§2); its closing summary (§8) still applies.

| You do | You never do |
|---|---|
| Interview the decider, one subject at a time | Decide in their place, or fill a gap with an invention |
| Propose the field's convention by default, marked *(framer, to confirm)* with its source | Propose more than the need |
| List the open questions, blocking apart | Let a charter or a cycle be accepted with a blocking question open |
| Keep the first cycle small | Split into many modules on day one (`.nstack/handbook/02-modules.md` §3) |

A proposal without a source is an open question, not a proposal. The documents you write
carry the project, not the conversation: every later session reads them, and reads nothing
else of what was said.

---

## 1. Frame the project: the charter

Start from what the decider gives you. When `docs/project/discovery.md` exists, its decision
must be **go**: the charter reuses its users, problem, constraints, success signals, no-gos
and risks, carries its unanswered questions into its own, and you ask only what it leaves
open. Otherwise — an idea alone — propose a discovery first (`discovery.md`); a specification
already challenged can be framed directly. Read the source entirely, then interview, one
subject at a time, asking only what you cannot infer:

| Subject | Ask | The charter records |
|---|---|---|
| Users | Who uses it, in what situation, with what problem? | The users and their problem |
| Outcome | What must be true for the project to be finished? | Success criteria, measurable, dated — or relative to a named event, such as launch day, that a later framing dates |
| Constraints | Law, money, security, data, platforms, deadlines | The constraints |
| Risks | What could make it fail, or cause harm? | The risks, and the criticality they imply — what a failure of each module would cost, which the decider settles and `.nstack/handbook/05-workflow.md` §7 prices |
| Vocabulary | The domain's words, and what each one means | The vocabulary |
| Not this project | What it will deliberately not do | Out of scope |

A domain that moves money, handles personal data or acts on someone's behalf raises the
criticality of the modules concerned, and `security.md` applies from the first cycle — to
the code, and to the data a spike collects: who sees it, how long it is kept. Say so to the
decider.

**Before proposing a criticality, read the rule** — the last paragraph of
`.nstack/handbook/05-workflow.md` §7: a module that holds a key, places an order, moves money or
touches someone's personal data is `critical`. Propose what the rule says, with what each
value costs; a lower value is the decider's to choose, and is written as a deviation with its
reason and the event that ends it.

When no discovery came first, ask too **in which language the project writes**; the answer is
its first rule (`docs/project/rules/always.md`).

**A constraint that binds how the work is done becomes a rule of the project**, proposed
with the charter: one line of `docs/project/rules/<subject>.md`, from
`.nstack/forms/rules.md` — the subject is the playbook it completes, `always` when
it applies to every task. It names its source in the charter and how it is checked. Never
write it into a playbook or the kernel: those are every project's, and the next update
would conflict on it.

Write `docs/project/charter.md` from `.nstack/forms/charter.md`, with
`status: proposed` and the open questions at the end, then run the fitness functions.
"Finished" means the charter's goal is reached: framing stops; running what was built may go
on.

## 2. Plan a cycle

1. **Goal** — one sentence, the one outcome of this cycle.
2. **Deliverables** — a finite list, `D1`, `D2`…, each an outcome a user can observe — for a
   spike, the question it answers for the decider. Each gets acceptance criteria, written
   *given · when · then*; they become its test sheet (`verification.md`).

   **Two questions to ask of each one, before writing its criteria:**
   - *Can the team finish it alone, inside this cycle?* If it waits on someone outside the
     project — a reply, an opinion, a decision elsewhere — it is a **dependency with a date**,
     not a deliverable. Name it in the risks and give it a date; the cycle does not hold it.
   - *Is its product knowledge rather than code?* Then it is a **spike**, and its criteria say
     **what will be recorded** and **the threshold it is read against** — never that the answer
     comes out a particular way. **A refuted result is a delivered one.** "Five people were
     asked, one acted, below the threshold of two" closes the deliverable; it does not fail it.
     When its product is a **drawing** — the screens, the journey the user will see — the
     drawing is committed under `docs/project/drawings/<deliverable>-<name>/` with its date,
     and frozen: what the decider accepts in it becomes criteria of the deliverable that
     builds it, and once built, the product and its screenshots are the truth. A new round is
     a new folder, never an update.

   A criterion nobody can meet is not met. It is answered with something that looks like proof,
   and that answer is reviewed and merged like the rest.
3. **Unknowns** — a blocking unknown becomes a *spike* deliverable, whose output is
   knowledge (`.nstack/handbook/05-workflow.md` §3). After a discovery's go, the tests the decider kept
   are the first cycle's spikes. A spike's criteria name what is recorded and the threshold it
   is read against; a refuted result is still a delivered one.
4. **Modules** — name each deliverable's module; a spike run outside the code — a sign-up
   form, interviews — has none (`module: null`). Propose a new module only against the tree of
   `.nstack/handbook/02-modules.md` §3: one module is often enough to start.
5. **Appetite** — ask the decider how many weeks they **want** to spend, not how long it
   will take. When the deliverables do not fit, cut the scope; never stretch the appetite.
6. **Out of scope and later** — out of scope is not done even if time remains; *later* holds
   the candidates for the next framing.

Write `docs/project/cycles/NN-<slug>.md` from `.nstack/forms/cycle.md`, with
`status: proposed` and `end` = `start` + the appetite, then run the fitness functions.

## 3. Hand over to the decider

Present the charter, the cycle, the open questions, what went to *later*, and what you are
unsure of. The decider amends, then sets `status: accepted` — on the charter, then on the
cycle — in a pull request they approve; accepted after `start`, the cycle's `start` and `end`
move in the same pull request. The modules accepted there are created with the engine's
module scaffolding, declared user-facing when a user sees them.

**Where the project's own rules waive approval** — an agent merging alone on an attested,
verified head, `doctor`'s G2 taken knowingly — nobody approves that pull request, and the
decider's word stands for the approval: quoted in the page it accepts (the charter, the cycle),
with its date and where it was said — a comment, a message, a meeting's note. The same holds
for a deliverable accepted below. Without that line, the acceptance is the agent's alone.

**The pull request that accepts the charter writes the project's front page from it** —
`README.md`: a diagram of who uses the product and what it talks to, how to run what exists,
and the sentence when no discovery wrote it. Until then that page holds its sentence at most;
from then on the plan check refuses an accepted charter over a form left as it came
(`.nstack/handbook/06-decisions.md` §10, `documentation.md`).

## 4. During the cycle

- **A new idea** goes to *later*. It enters the cycle only if the decider re-frames:
  swapping a deliverable out, adding one they judge fits what is left of the appetite —
  the end date does not move — or closing the cycle early. An added deliverable takes its
  criteria like the others, and its section says on whose word and on what it entered.
- **An urgent fix** outside the cycle — an incident, a production defect — goes in a pull
  request with the `out-of-cycle` label and the line `Out of cycle: <reason>`.
- **A deliverable** moves `proposed` → `ready` (testable acceptance criteria, set by the
  framer, confirmed when the decider accepts the cycle) → `in-progress` — its hold says so the
  moment it is claimed; the cycle file follows in the plan's own pull requests, never in a
  delivery's — → `accepted`: its
  test sheet run and its pull request approved, or the decider's word quoted where approval is
  waived (§3) — or, for a spike outside the code, its result
  approved by the decider: a decision record when it binds the project (*Decisions* below),
  otherwise a line in the project's log issue, which the deliverable's section links. A deliverable that several pull requests
  deliver, or that a person must try, is `accepted` when **each of its criteria** is held by a
  scenario of one of their sheets or by an answer in its acceptance issue
  (`.nstack/handbook/05-workflow.md` §7); the cycle names that issue beside the state.

## 5. The end of a cycle

**Every deliverable accepted** — write the closure: what was delivered, which success
criteria moved, what was deferred, and **which documents the cycle brought current** — the
front page and the module map first, each with its pull request; set `status: closed`,
`outcome: completed`, `ended_on`.
A success criterion the decider says moved is recorded under `success:` in the front matter, at
any closure — by its number in the charter, `met`, or `observing` with the day observation
started; the latest record is its state, and `nstack plan` counts the criteria met.
When a spike refutes what the charter rests on, present stopping the project too.

**The end date reached first** — the circuit breaker: nothing is extended. Present the
three choices, with the state of every deliverable:

| The decider chooses | `status` | `outcome` |
|---|---|---|
| Ship what is accepted | `closed` | `shipped` |
| Frame a new cycle, with a new appetite | `stopped` | `reframed` |
| Stop the project | `stopped` | `stopped` |

The closure is where the next framing starts. When `nstack plan` says the charter's goal is
reached — every success criterion met —, present it: the decider says whether the project is
finished.

## Decisions

A decision lives in a record, never in the charter, the cycle or a conversation
(`.nstack/handbook/06-decisions.md`): one register of each kind at the project's root —
`docs/pdr/` for the product, `docs/adr/` for the technique — and `Scope: module <name>` when it
concerns one module.

| When | Then |
|---|---|
| An accepted charter has to change — a constraint, a user, what the project is not | A **PDR** first, which the change cites; the charter is never rewritten in place (CI warns, C9) |
| The framing chooses a module, a stack, a contract | An **ADR**: the module's creation decision is one |
| A spike's result binds the project | A PDR or an ADR; otherwise a line in the project's log issue |
| A cycle's **closure** | Read every decision the plan names as due (C10): its criterion names this cycle, or a date its end reached. The record gains one line under its criterion, *Read at <cycle>'s closure, <date>: met \| missed \| superseded*, and nothing else; a *missed* one is the decider's to answer, by a new record or the removal condition |
