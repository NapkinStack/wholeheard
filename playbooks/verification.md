# Playbook — Verification

> **Trigger.** Load this playbook when you are asked to verify a change you did not write,
> or to fill in the results of a test sheet.

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

---

## The procedure

1. **Read the need** — the deliverable, its acceptance criteria, the test sheet in the
   pull request's description. Nothing else yet.
2. **Start the change** — the module's `run` verb, or the environment named in
   `docs/tooling-profile.md`.
3. **Run each scenario**, in order:
   - *automated* — the module's `e2e` verb; the evidence is the CI run on the head commit,
     or its artefact;
   - *explored* — drive the interface yourself, in a browser, an emulator or on a device,
     with the tools of the tooling profile; capture the screen at the step that proves the
     result;
   - *human only* — do not run it; check that its reason is stated.
4. **Record** each result in the sheet — `passed`, `failed — <what you observed>`, or
   `not verified` — with its evidence and the short hash of the commit you tested.
5. **Only then read the diff**, to explain a failure — never to decide a result.
6. **Report** the failures first — observed, expected, evidence — then any optional
   remark, marked as such.

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
