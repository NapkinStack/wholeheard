# Product Decision Records

An important **product** decision: a user goal, an expected behaviour, a trade-off, a
structuring business rule, a UX or business decision.

The PDR describes **what the product must do and why**, never its implementation.

- Template: [`_TEMPLATE.md`](./_TEMPLATE.md)
- Naming: `NNNN-user-oriented-title.md`

## Mandatory sections

| Section | Why |
|---|---|
| **Prior art** — ≥ 2 references | On an interface, a convention's value comes from the user already knowing it |
| **Dated success criterion** | Makes the decision falsifiable, and therefore useful |
| **Removal condition** | Without it, a feature is permanent by default, even unused |

> The **FDR** format does not exist in this OS. For genuinely complex features, the
> *Detailed functional design* section of the PDR is enough
> (`docs/os/06-decisions.md` §4).

## Index

| No. | Title | Status | Criterion to check on |
|---|---|---|---|
