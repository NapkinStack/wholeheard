# PDR-XXXX — <short, user-oriented title>

- **Status**: Proposed | Accepted | Superseded by PDR-YYYY | Abandoned
- **Date**: YYYY-MM-DD
- **Decision makers**:
- **Modules affected**:

> The PDR describes **what the product must do and why**, never its implementation.
> The how belongs to an ADR.

---

## User problem

<Who has the problem, in what situation, and what fails today?
Describe the user, not the missing feature.
Bad: "there is no CSV export".
Good: "every month the accountants rebuild by hand a table the product already has,
which takes them half a day and introduces errors".>

## Goal

<What the user must be able to do afterwards. One sentence.>

## Out of scope

<What we deliberately do not do. The most useful section of the document: it is what
prevents gradual drift.>

---

## Prior art

> **Mandatory section.** Minimum two named references.
> (`docs/os/06-decisions.md` §2)

**How is this problem solved elsewhere?**

| Product / reference | Solution chosen | What we keep from it |
|---|---|---|
| | | |
| | | |

**The convention the user already knows:** <which pattern do they expect to find?>

**Why depart from it** *(only when departing from it)*:

<The user value that pays for the learning cost imposed. On an interface, the convention
is almost always the right choice — its value comes precisely from the user already
knowing it.>

---

## Options considered

| Option | What the user experiences | Cost | Chosen? |
|---|---|---|---|
| Do nothing | | 0 | |
| <option 1> | | | |
| <option 2> | | | |

## Decision

<What the product does, described from the user's point of view. Trade-offs owned.>

---

## Expected behaviour

**Nominal journey:**

**Edge cases and degraded states:**

**Business rules:**

**Permissions:** <who can do what>

**Acceptance criteria** *(testable — this is the task's oracle,
`docs/os/05-workflow.md` §3)*:

- [ ] Given <context>, when <action>, then <observable result>
- [ ] …

---

## Detailed functional design *(optional)*

> Fill this in only when the feature is genuinely complex: many actors, a state machine,
> a permission matrix. Otherwise, delete this section.
>
> It replaces the old FDR format (`docs/os/06-decisions.md` §4).

**Actors:**
**States and transitions:**
**Permission matrix:**
**Interactions with other modules:**

---

## Success criterion

> **Mandatory section.** Without a date nobody comes back to check, and the decision
> folder becomes a graveyard.

> We will consider this was the right call if **\<measurable observation\>** is observed
> before **\<YYYY-MM-DD\>**.

How it is observed: <metric, user feedback, usage>

If the criterion is not met: <adjust · supersede · remove>

---

## Removal condition

> **Mandatory section.** A feature with no removal condition is permanent by default,
> including when nobody uses it.

> This feature will be removed if **\<condition\>**.

---

## Impacts

- **Existing users**: <migration, communication, learning>
- **Modules and contracts**:
- **Support and documentation**:
- **Data**: <collection, retention, compliance>
