# Playbook — Security

> **Trigger.** Load this playbook when the task touches: authentication, authorisation,
> secrets, personal or sensitive data, external input, file upload, network exposure,
> dependencies.
>
> Format: `situation → action → verification`. Anything not verifiable here must go to
> human review.

---

## Absolute rules

| # | Rule |
|---|---|
| S1 | **No secret in the repository.** Not in clear text, not encoded, not "temporarily", not in a test, not in a comment. |
| S2 | **All external input is hostile** until explicitly validated. |
| S3 | **Least privilege by default** — including for CI and for agents. |
| S4 | **Deny by default** in authorisation: whatever is not explicitly allowed is forbidden. |
| S5 | **Never design your own** cryptographic, password-hashing or authentication mechanism. The established convention, always (Prior Art Gate). |

If one of these rules has to be broken, this is no longer a development task: it is a
decision, and it goes through an ADR and human approval.

---

## Authentication

| Situation | Action | Verification |
|---|---|---|
| New entry point | Check it is covered by the existing mechanism, not a new one | Test: unauthenticated call → refused |
| Password storage | A slow, standard hashing algorithm, never a homemade one | Test on the algorithm used |
| Session or token | Expiry, revocation and rotation planned | Test: expired token → refused |
| Authentication error message | Never reveal whether the account exists | Test on the message content |

---

## Authorisation

This is the most frequent source of vulnerabilities, and the easiest to test.

| Situation | Action | Verification |
|---|---|---|
| New endpoint or action | Decide explicitly who is allowed | Test per role: allowed, not allowed, not authenticated |
| Access to a resource by identifier | Check ownership, not only the role | Test: user A reaches B's resource → refused |
| List or search | Filter server-side, never client-side | Test: the response contains no one else's data |
| Privilege escalation possible | Treat it as a high-risk action | Test + logging + human review |

> **The most common trap.** Hiding a button in the interface is not an authorisation.
> Every access rule exists server-side, and is tested there.

---

## Secrets

| Situation | Action | Verification |
|---|---|---|
| A secret is needed | Environment variable or vault — never the code | Automated detection in CI |
| Secret exposed, even briefly | **Rotate immediately.** Removing it from the commit is not enough | An incident, not a simple fix |
| Secret in a test | A fake value explicitly named as such | Diff review |
| Log containing a secret or a token | Mask at the source, not at display time | Test on the log content |

---

## Input and data

| Situation | Action | Verification |
|---|---|---|
| User input | Validate structure, type, bounds, format — then sanitise | Test with invalid, empty, overlong, malformed input |
| Query to a database | Parameterised queries, never concatenation | Injection test |
| Displaying supplied content | Escape at render time, appropriately for the context | XSS test |
| File upload | Type, size, extension, storage outside any executable area | Test with a hostile file |
| Personal data | Minimise, encrypt at rest when sensitive, log access | Review + data inventory |
| Application log | No personal data and no secret | Test on the log content |

---

## Dependencies and supply chain

| Situation | Action | Verification |
|---|---|---|
| New dependency | Niche filter (`docs/os/06-decisions.md` §3) + declared in the manifest | Review: undeclared dependency (to automate) |
| Reported vulnerability | Handle it by severity and real exposure, not by score alone | Automated analysis in CI |
| Unmaintained dependency | Open a *Technical debt* issue with the exit strategy | Dependency review |

---

## Exposure and abuse

| Situation | Action |
|---|---|
| Expensive public endpoint | Rate limiting, from the first version |
| Sensitive operation | Auditable logging: who, what, when, from where |
| Error returned to the client | A generic message; the detail goes to the logs |
| Internal resource | Not exposed by default; exposing it is a decision |

---

## High-risk actions

Changing permissions, accessing secrets, a change touching authentication, exposing a
new service, an irreversible migration.

**Mandatory procedure — never a silent execution:**

```
1. name the risk and its blast radius
2. describe the state before / after
3. propose the safe procedure and the rollback
4. ask for explicit confirmation
```

---

## Closing checklist

- [ ] No secret added, in any form
- [ ] Authorisation tested by role **and** by ownership
- [ ] Input validated and sanitised, with negative tests
- [ ] Logs free of secrets and personal data
- [ ] Error messages uninformative to an attacker
- [ ] New dependencies declared and filtered
- [ ] High-risk actions explicitly confirmed
- [ ] Whatever could not be verified appears under `NOT VERIFIED` in the summary
