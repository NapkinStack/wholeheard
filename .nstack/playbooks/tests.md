# Playbook — Tests

> **Trigger.** Load this playbook when you are touching: a non-trivial test strategy, a
> flaky test.
>
> For an ordinary task the kernel rule is enough: **write the oracle, watch it fail,
> implement**.
>
> **This project's own rules** on the subject, when it has any: `docs/project/rules/tests.md`
> — read them with this playbook. They add to it or tighten it.

---

## 1. Choosing the test level

```mermaid
flowchart TD
    A["Behaviour to verify"] --> B{"Is it a pure<br/>business rule?"}
    B -->|Yes| U["UNIT TEST<br/>fast, numerous, edge cases"]
    B -->|No| C{"Does it cross a<br/>module boundary?"}

    C -->|Yes| CT["CONTRACT TEST<br/>on both sides"]
    C -->|No| D{"Does it depend on<br/>real infrastructure?"}

    D -->|Yes| I["INTEGRATION TEST<br/>targeted, minimal scope"]
    D -->|No| E{"Is it a critical<br/>user journey?"}

    E -->|Yes| E2["E2E — only when<br/>genuinely critical"]
    E -->|No| U

    style U fill:#065f46,color:#fff
    style CT fill:#065f46,color:#fff
    style E2 fill:#7c2d12,color:#fff
```

**Legend** — green: the levels to prefer · red: the level to use sparingly.

Preference rule: **the lowest level that actually verifies the behaviour**. An E2E test
that could have been a unit test costs a hundred times more, breaks ten times more
often, and diagnoses ten times worse.

A scenario replayable end to end belongs in the module's `e2e` command, and its evidence in
`.evidence/`: CI keeps it for the test sheet (`.nstack/handbook/05-workflow.md` §7).

**What the run said.** `nstack e2e` gives the run a folder of its own, `$NSTACK_EVIDENCE`
(`<module>/.evidence/e2e/`), keeps there the command's output — a traceback included — and then
reads the log files the run wrote there:

- **Write there** the module's logs (`.log`, `.jsonl`) and screenshots; data that are not logs —
  recordings, events — elsewhere. A page's uncaught errors and console errors reach a log there:
  logged by the page itself through the application, or collected by a script the scenario
  injects (`window.onerror`, `unhandledrejection`) — some drivers expose no console.
- **The audit** reads each line's level — JSON keys, `level=`, a level word leading the line, or
  the module's `evidence.levels` expression. A warning or an error the module did not declare is
  quoted with `file:line` and the declaration that would accept it: an error fails the run, a
  warning fails from criticality `high` and is reported below. The first NapkinStack version that
  brings the audit only reports; the project's next minor version judges.
- **Declare** what the scenarios provoke on purpose, in the manifest — a pattern, its level, the
  scenario that provokes it (M12). The manifest is the one place the audit reads: a warning or
  an error a scenario provokes is declared there, even when the scenario asserts it too. A module
  that declares lines and whose run wrote no log into `$NSTACK_EVIDENCE` fails when judged — they
  were checked against nothing.
- **Not run here.** A run that cannot run on this machine — no desktop, no sound server — says
  why on its output and exits 77, the code Automake and Meson read as a skipped test: the run is
  recorded *not run here*, what it logged is shown and nothing judged, and it passes. Exit 0 means
  it ran; 77 for any other reason — some tools use it for a refused permission — reads as *not
  run*, so a run that fails exits with another code.

```yaml
evidence:
  provoked:
    - pattern: "key refused"
      level: warning
      scenario: S3
```

---

## 2. The oracle

| Requirement | Why |
|---|---|
| Written **before** the implementation | Otherwise it is written to pass, not to verify |
| Watched **failing** first | A test never seen red may test nothing |
| Fails for the **right reason** | A compilation failure is not a test failure |
| Named after the **behaviour**, not the function | `refuses_an_order_without_stock`, not `test_create_order_2` |

### When the oracle is impossible

| Cause | Response |
|---|---|
| Subjective criterion | Restate it as an observable behaviour |
| Exploratory task | Requalify it as a *spike*: the deliverable is knowledge |
| Untestable area | Make it testable first, as a separate task |
| Vague need | Back to framing — it was not *Ready* |

In every case: **do not generate while waiting.**

---

## 3. What to test first

```
1. business rules           2. critical journeys       3. permissions
4. contracts                5. errors                  6. edge cases
7. regressions that happened   8. high user impact
```

**Every regression fixed gets a test** that failed before the fix. That is the only
guarantee it will not come back quietly.

**Do not aim at a coverage percentage.** Coverage tells you what is not tested; it never
tells you that what is tested is tested well.

---

## 4. Edge cases to consider every time

```
empty · null · absent · zero · negative · very large · very long
special characters · unicode · leading and trailing whitespace
duplicate · concurrency · repeated call (idempotence)
dependency unavailable · timeout · partial response
authorisation refused · not authenticated · session expired
time zone · daylight saving · deadline date
```

---

## 5. Flaky tests

A flaky test is an **engineering problem**, never a fact of life. Its real cost: it
teaches the team to ignore a CI failure. A single tolerated flaky test degrades the
value of the whole suite.

```mermaid
flowchart TD
    A["Flaky test detected"] --> B["Isolate it: quarantine,<br/>do not delete"]
    B --> C{"Cause?"}
    C -->|"Real time"| D1["Inject the clock"]
    C -->|"Execution order"| D2["Isolate state between tests"]
    C -->|"Concurrency"| D3["Synchronise on the event,<br/>never on a duration"]
    C -->|"Network / external"| D4["Deterministic double"]
    C -->|"Shared state"| D5["Reset for every test"]
    C -->|"Undetermined"| E["The test proves nothing:<br/>rewrite it or delete it"]

    D1 --> F["Out of quarantine"]
    D2 --> F
    D3 --> F
    D4 --> F
    D5 --> F

    style E fill:#7c2d12,color:#fff
    style F fill:#065f46,color:#fff
```

**Legend** — green: back to a trustworthy suite · red: the test is not worth keeping.

**Never "re-run until it passes".** A disabled test carries an issue and a date to pick
it back up; without that, it never comes back.

Fixed waits (`sleep`) are the most widespread cause of flakiness: wait for an
**observable state**, never for a duration.

---

## 6. Doubles

| Double | Do not double |
|---|---|
| External services, network, clock, randomness | The business logic of the module under test |
| Slow or expensive dependencies | What the test is meant to verify |
| Other modules — through their **contract** | The database, in an integration test |

> **Never double another module's behaviour by guessing.** You double its contract, and
> the contract test guarantees the contract matches reality. An invented double goes
> green while production breaks.

---

## 7. Warning signals

| Signal | Reading |
|---|---|
| A test breaks at every refactor with no behaviour change | It tests the implementation, not the behaviour |
| You have to start another module to test | Bad boundary (`.nstack/handbook/02-modules.md` §9) |
| The test is longer than the code it tests | The code is probably too coupled |
| Nobody understands what this test tests | Delete it or rewrite it; it protects nothing |
| The suite takes more than 10 minutes | It will be bypassed — making it fast is a priority task |
