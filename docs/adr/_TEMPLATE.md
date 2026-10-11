# ADR-XXXX — <short verb-phrase title: "Adopt X", "Split Y">

- **Status**: Proposed | Accepted | Superseded by ADR-YYYY | Abandoned
- **Date**: YYYY-MM-DD
- **Decision makers**: <team or people>
- **Scope**: project | module `<name>`
- **Reversibility**: easy | costly | irreversible

---

## Context

<What is true today and why the question arises now. Factual.
Somebody arriving in six months must understand the situation with no spoken context.>

## Problem

<The question to settle, in one or two sentences. If it cannot be put as one question,
it is probably several decisions — separate them.>

## Constraints

<What is non-negotiable and frames the choice: what exists, skills, operations, security,
budget, deadline, compliance. Distinguish real constraints from preferences.>

---

## Prior art

> **Mandatory section.** Minimum two named references. A missing section or a missing
> reference → refused in review, pending an automated check. (`docs/os/06-decisions.md` §2)

**Dominant convention of the field:** <what is the standard answer to this problem?>

**References examined:**

| Reference | What it does | Applicable here? |
|---|---|---|
| <project, company, standard> | | |
| <project, company, standard> | | |

**Why the convention is not enough** *(fill in only when departing from it)*:

<The demonstrated need that justifies the gap. "Cleaner", "more flexible", "more modern"
are not acceptable justifications.>

---

## Options considered

### Option 1 — <name>
- Description:
- Advantages:
- Drawbacks:
- Cost of setup / of maintenance / **of getting out**:

### Option 2 — <name>
<same>

### Option 3 — Do nothing
<To evaluate every time. It is often the cheapest option and rarely the worst.>

---

## Decision

<The option chosen, and **why this one** rather than the others. Not a repeat of the
description: the argument that settled it.>

### Deviation from the convention

*(Mandatory section when the decision departs from the convention identified.)*

- **Expected user value**:
- **How we will observe it**:

---

## Success criterion

*(Mandatory when building something bespoke or departing from the convention.)*

> We will consider this was the right call if **\<measurable observation\>** is observed
> before **\<YYYY-MM-DD\>**.

What we do if it is not: <correct · supersede · roll back>

---

## Consequences

**Positive:**

**Negative and accepted debt:**

**Impacts on other modules or contracts:**

**Rule to automate:** <which fitness function or check follows from this decision? If
none is possible, explain why. `docs/os/07-governance.md` §2>

---

## Rejected alternatives

<Why they were set aside. This section is often worth more than the decision itself: it
stops somebody re-proposing, a year from now, a solution already evaluated.>
