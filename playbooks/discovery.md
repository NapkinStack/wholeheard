# Playbook — Discovery

> **Trigger.** Load this playbook when a project starts from an idea — a few lines, notes,
> a wish — before any charter: to test the idea, compare it with what exists, challenge it,
> and reach a decision.

---

## Your posture

You run stages 1 to 4 as the **framer**, stage 5 only as the **challenger** — never both in
the same session. The decider decides, at stage 6. For the framer and the challenger, this
playbook replaces the kernel's working loop (§2); its closing summary (§8) still applies.

| You do | You never do |
|---|---|
| Framer: ask one question at a time, only what you cannot infer | Agree to please; soften an objection |
| Give every market fact its source | State a fact you cannot source — write it as an assumption |
| Keep the document to what could change the decision | Fill a canvas, size a market, invent personas |
| End each round with a decision | Open a new direction before the round is decided |

The engine's `discover` command created `docs/project/discovery.md`: one section per stage,
each ending with its open questions. When the decider cannot answer now, write the question
there — most decision-changing first — and go on; never answer it in their place.

---

## 1. Intake

Read the idea file entirely. Restate the idea in one paragraph — for whom, where, which
problem, what the product does — and ask the decider to correct it. Record the decider in
the front matter. Until the decider confirms the paragraph, mark it *unconfirmed*: the round
cannot be decided while it is.

## 2. Research

With the research tools of `docs/tooling-profile.md` — or, when that row is empty, the tools
at hand, named in the document — look for:

- what users do today instead, including nothing;
- the direct and indirect alternatives, their positioning and their price;
- the business models of the field;
- the legal and regulatory constraints, for the countries concerned.

Each finding goes in the evidence table, with what it changes for the idea and its source.
Prefer the primary source; mark figures a company reports about itself, and claims seen only
in a search excerpt. A claim without a source goes to *Assumptions*. Stop when each of the
four points has at least one sourced finding, or an explicit "not found".

## 3. Define

With the decider: the users; their problem — quoted from users, saying who and how many,
since words reported by the decider alone are an assumption; the constraints — who runs it,
budget, platforms, regulation; and the success signals — what will be observed, and by when,
if the idea is right. If this stage changes the users or the model, revisit stage 2.

## 4. Shape

- **The value hypothesis** — why a user would switch from what they do today, in one or two
  sentences, stated so that it can be proven wrong. Not a list of features.
- **A short press release**, dated launch day — a placeholder if none is set: the user, the
  problem, the product, one quote, labelled as imagined. Record whether it convinces the
  decider; if it does not, that is a finding.
- **No-gos** — what the product deliberately will not do: the decider's, and any the framer
  proposes, marked *proposed*.

## 5. Challenge — in another session

A session that did not write stages 1 to 4, or a human; it starts once section 1 is
confirmed — otherwise it names the reading it challenges. The challenger does not interview
the decider: its questions go into its objections. It reads the idea file and the document,
then:

1. **Pre-mortem** — the product launched a year ago and failed: the plausible reasons, each
   tagged with its risk.
2. **The four risks** — *value*: will they use it, or buy it? *usability*: can they use it?
   *feasibility*: can it be built with the means at hand? *viability*: does it hold for the
   business — cost, law, operations? For each, the riskiest assumption and the cheapest test
   that could refute it: what is done, for how long, and the result that refutes it — a
   threshold proposed, for the decider to set.
3. **Counter-evidence** — what contradicts the value hypothesis or the riskiest assumptions:
   failed products, closed competitors, regulatory refusals. Research as in stage 2 — same
   tools, same marking, the same stopping rule; an unsourced claim is marked *assumption*.
4. **Flaws in stages 1 to 4** — each citing the line it objects to.

These are the objections, written in section 5 with the challenger's name and date. Never
rewrite stages 1 to 4.

## 6. Decide

A framer session presents section 5 unchanged, then the riskiest assumptions and the value
hypothesis. The decider chooses:

| Decision | What follows |
|---|---|
| **go** | The framing writes the charter from this document (`framing.md`); the tests the decider keeps become spike deliverables of the first cycle |
| **clarify** | A new round on the open questions, and on the cheap tests the decider picks before any charter; `round` goes up, section 5 is kept under its round, and the changed sections are challenged again |
| **kill** | The document stays, with its reasons — the cheapest outcome of all |

Section 6 records the reasons, and for a go, why each objection does not block it. Record
`decision` and `decided_on` in the front matter, then run the fitness functions.

---

## Bounds

A round is one or two working sessions. A second `clarify` is the decider's explicit choice,
recorded in section 6 — never a default.
