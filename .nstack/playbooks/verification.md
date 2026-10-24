# Playbook — Verification

> **Trigger.** Load this playbook when you are touching: verifying a change you did not
> write, filling in a test sheet.
>
> **This project's own rules** on the subject, when it has any: `docs/project/rules/verification.md`
> — read them with this playbook. They add to it or tighten it.

---

## Your posture

You are the **verifier**. You did not write this change, and you do not fix it. For the
verifier, this playbook replaces the kernel's working loop (§2); its closing summary (§8)
still applies.

| You do | You never do |
|---|---|
| Start from the need, the acceptance criteria and the test sheet | Read the diff before running the sheet |
| Run every scenario against the running change | Fix the code, even one line |
| Bring back evidence for every result | Write "passed" without evidence |
| Report the gaps against the need or against correctness | Report a preference as a gap |

The session that wrote the change cannot verify it: start another session, with a fresh
context, or hand the sheet to a human. Sign the sheet with your session's identifier —
`Verifier: session <id>` — or, as a person, `Verifier: @<handle>`. CI compares it with the
change's authors and refuses a match.

### Your own ground

You verify on someone's machine, beside work that goes on. Leave it as you found it.

- **A copy of your own**: a working tree checked out at the head, and build outputs of your
  own. A tree another session edits, or outputs another branch built, give results that are
  not this change's.
- **The machine's shared state** — a default device, a setting, a service, data others read —
  is touched only as the scenario needs, and is put back after each run, also on failure.
  What another program holds — a user's own running copy of the product, their files — is
  never touched.
- **What you start, you stop, by its process number** — never by a name or a pattern, which
  also matches what is not yours. When you leave, nothing of yours is left running.
- **The description is fetched again just before you write it**: the author may have edited
  it since you read it, and your edit must keep theirs.

---

## The procedure

1. **Read the need** — the deliverable, its acceptance criteria, the test sheet in the
   pull request's description. Nothing else yet.
2. **Start the change** — the module's `run` verb, or the environment named in
   `.nstack/tooling-profile.md`. A module with no `run` verb — a library, a contract — has
   nothing to start: its scenarios run through its tests, and you say so in your report.
3. **Run each scenario**, in order:
   - *automated* — the module's `e2e` verb, or its `test` verb when it declares no `e2e`;
     the evidence is the CI run on the head commit, or its artefact — for a change to a
     visible surface, read the run's screenshots and logs (`.evidence/e2e/`, its `audit.json`)
     and cite what they showed: never "rendering not seen" on a scenario passed. An audit
     `not run` (exit 77) ran no scenario: each one it stood for is *not verified*, or run where
     it can run;
   - *explored* — drive the interface yourself, in a browser, an emulator or on a device,
     with the tools of the tooling profile; capture the screen at the step that proves the
     result;
   - *human only* — do not run it; check that its reason is stated and, when the
     deliverable's acceptance rests on it, that an acceptance issue asks it of a person.
4. **Record** each result in the sheet — `passed`, `failed — <what you observed>`, or
   `not verified` — with its evidence and the short hash of the commit you tested. What you
   have to add goes on the line **`Verifier's remarks:`** under the table, never in a cell: a
   scenario that passed by your own probe and not by the test it cites, a wording that claims
   more than was exercised. A remark is not a result, and the approver reads it. After a
   fix, an *automated* scenario is run again at the head and its Commit cell takes the head —
   it is never confirmed; an *explored* one keeps its commit and is confirmed in
   the `Confirmed` column with the head commit and what moved since; at criticality
   `critical`, every scenario is run again (`.nstack/handbook/05-workflow.md` §7).
5. **Only then read the diff**, to explain a failure — never to decide a result — and ask one
   question of its tests: **is the entry point a real user calls exercised the way they call
   it?** A suite that always passes what the user never passes is green on a path nobody takes.
6. **Report** the failures first — observed, expected, evidence — then any optional
   remark, marked as such.

### Mutants

To show that a test pins what it claims, change the code it claims to pin and watch it fail — a
mutant. A mutant can make the code loop forever, so it is never run unbounded:

- **The language's tool first**, rather than a loop of your own: `cargo-mutants` (Rust; `--file`
  to aim it, `--timeout` to bound it), `mutmut` (Python), Stryker (JavaScript, TypeScript, C#,
  Scala), PIT (Java).
- **By hand**, one mutant per run, each run under a time limit — `timeout 120 <the module's test
  command>`; a timeout counts as killed: the mutant changed what the code does.
- **The source restored on every exit**, not only at the end — `trap 'git checkout -- <file>'
  EXIT`, or mutate in a throwaway worktree.
- **Report the counts** — killed, survived, timed out — and each survivor with its line: a
  survivor is a test that does not pin what it claims.

---

## Evidence

| Scenario | Evidence expected |
|---|---|
| An interface | A screenshot of the state that proves the result; a video for a sequence |
| A mobile interface | The screenshot of the emulator or the device, with its width or model |
| An API, a job, data | The command and its output, or the log lines |
| Accessibility | The keyboard path followed, and the automated report when there is one |

Test data only: never a real user's data, never a secret on a screenshot (`security.md`).
Evidence lives where the whole team sees it — a CI artefact, or an image attached to the
pull request.

A user-facing change covers the eight states of an interface (`ux.md`), or says which are
out of scope. A state missing from the sheet is a gap to report.

---

## When the sheet itself is wrong

A scenario that does not match the need, or a need no scenario covers: report it. An
expected result changes only with the decider's agreement, visibly in the sheet's history.
