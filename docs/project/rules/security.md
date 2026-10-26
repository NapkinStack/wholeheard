# Rules of this project — security

> Read with the security playbook. These rules add to what every project shares, or tighten
> it; they never repeat it. *In force since the charter was accepted, 2026-10-12.*

| # | Rule | Why | Checked by |
|---|---|---|---|
| 1 | Nothing leaves the machine unless the user chose it: no network call to a host the user did not configure, unless the user turned on the sending that names it — no telemetry the user did not turn on ([PDR-0003](../../pdr/0003-telemetry-only-if-the-user-turns-it-on.md), 2026-10-26), no update check, no model download from a host that is not shown first. | The charter's second risk, and principle 4 of the idea file ("local first"), 2026-10-12. It is the one promise Wholeheard keeps against a bot. | Nobody yet. Owed: a test that fails when a step opens a connection to a host absent from the configuration. |
| 2 | A recording, a transcript or the minutes of a real meeting never enter this repository, an issue, a pull request or a log — not as a fixture, not as evidence. Tests use audio recorded for the purpose, by people who agreed. | The other participants of a meeting did not agree to be in a repository; charter, risks, 2026-10-12. | Review. Nobody yet for the logs. |
