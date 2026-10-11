# Security

## Reporting a vulnerability

**Never open a public issue for a vulnerability.**

Use the forge's private reporting: on GitHub, the **Security** tab, then **Report a
vulnerability**. The report is visible only to the maintainers.

That channel only exists for a public repository. For a private one, replace this
paragraph with the team's internal channel.

Target time to first response: **7 days**.

## The project's absolute rules

| # | Rule |
|---|---|
| S1 | No secret in the repository — not in clear text, not encoded, not in a test |
| S2 | All external input is hostile until explicitly validated |
| S3 | Least privilege by default, including for CI and for agents |
| S4 | Deny by default in authorisation |
| S5 | Never a homemade cryptographic or authentication mechanism |

Operational detail: `playbooks/security.md`.

## An exposed secret

A secret exposed, even briefly, even in a commit since rewritten: **rotate it
immediately**. Removing it from history is not enough. That is an incident, not a fix.

## High-risk actions

Changing permissions, accessing secrets, exposing a new service, an irreversible
migration: never a silent execution. The risk named, the impact described, a safe
procedure and a rollback proposed, explicit confirmation asked for (`AGENTS.md` §5).
