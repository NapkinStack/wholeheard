# Rules of this project — security

> Read with the security playbook. These rules add to what every project shares, or tighten
> it; they never repeat it. *In force since the charter was accepted, 2026-10-12.*

| # | Rule | Why | Checked by |
|---|---|---|---|
| 1 | Nothing leaves the machine unless the user chose it: no network call to a host the user did not configure — no telemetry, no update check, no model download from a host that is not shown first. | The charter's second risk, and principle 4 of the idea file ("local first"), 2026-10-12. It is the one promise Wholeheard keeps against a bot. | Nobody yet. Owed: a test that fails when a step opens a connection to a host absent from the configuration. |
| 2 | A recording, a transcript or the minutes of a real meeting never enter this repository, an issue, a pull request or a log — not as a fixture, not as evidence. Tests use audio recorded for the purpose, by people who agreed. | The other participants of a meeting did not agree to be in a repository; charter, risks, 2026-10-12. | Review. Nobody yet for the logs. |
