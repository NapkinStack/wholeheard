---
goal: "On a Mac, the colleague opens Wholeheard from one app the forge made, and records a real call with it — both sides, nothing added to the Mac"
status: accepted          # proposed | accepted | closed | stopped — the decider accepts
appetite_weeks: 1         # the time the decider WANTS to spend, not an estimate
start: 2026-10-26         # YYYY-MM-DD
end: 2026-11-02           # start + appetite: the first day the cycle is over — the circuit breaker
deliverables:
  - id: D38
    title: "On a Mac, Wholeheard opens from one app the forge made, its four programs inside it"
    module: install
    state: in-progress
    acceptance:
      - "Given a run of the macOS workflow on main, or one started by hand, when it ends, then it leaves one artifact, Wholeheard-<version>-macos-arm64.zip, holding Wholeheard.app: wholeheard and the three programs side by side in Contents/MacOS, and an Info.plist naming the application, its version, and the sentences macOS shows when it asks for the microphone and for the sound the Mac plays; the bundle signed ad hoc, `codesign --verify --deep --strict` passing on it"
      - "Given that zip on the colleague's Mac — an Apple chip, macOS later than 14.6 — when they open the app as macOS lets an unsigned one open (System Settings → Privacy & Security → Open Anyway), then Wholeheard's icon is in the menu bar and its window opens from it, with nothing installed before it and no terminal"
      - "Given a first recording started from the menu bar, when macOS asks for the microphone and for the sound the Mac plays, then it names Wholeheard and shows the Info.plist's sentences — never the Terminal"
  - id: D39
    title: "Spike — does a real call recorded on a Mac hold both sides, with nothing added to the Mac?"
    module: capture
    state: ready
    acceptance:
      - "Given D38's app on the colleague's Mac, when they record a real call of at least five minutes with it, then the cycle records, for each track — local, the microphone; remote, what the Mac plays: whether it holds the call's speech, its sample rate and channels, its gaps of zeros, and what macOS asked before it recorded"
      - "Given that record, when it is read against the threshold — both tracks hold the call's speech, as the colleague hears it at the start, the middle and the end of each, and capture's code holds no line added for macOS —, then ADR-0002's criterion is met for macOS; below it, what was missing — a permission, a device, the system's interface — is recorded for the next framing. A refuted result is a delivered one"
  - id: D27
    title: "Wholeheard installs on Ubuntu from one package, its programs built on the system they ship to"
    module: install
    state: in-progress
    acceptance:
      - "Given Ubuntu 24.04, when a user installs Wholeheard from one package file (`sudo apt install ./wholeheard_<version>_amd64.deb`), then Wholeheard is in the applications menu and starts in the tray with nothing built and no variable set, and removing it leaves the user's meetings and settings"
      - "Given a release for a system — Fedora or Ubuntu —, when the programs are built, then they are built in a container of that system, so that no package depends on the machine that made it"
  - id: D40
    title: "Settings offer the models of a list kept in the repository, read again on the user's word"
    module: shell
    state: ready
    acceptance:
      - "Given the list `modules/shell/registry/models.json` on main, when Wholeheard is built, then Settings offer the models it lists — each with its size, its host and what it is good for — and no list of Whisper models is written in the code"
      - "Given Settings open, when the user has not asked for the list up to date, then no connection is opened to the list's host, and the models offered are those Wholeheard was built with"
      - "Given the gesture that asks for the list up to date, its host shown, when the list answers and reads, then Settings offer its models in place of the shipped ones until Wholeheard quits, and one of them is fetched only on the user's click, its size and host shown, kept only whole and with its fingerprint"
      - "Given a list that does not answer, is larger than 64 KB, does not read, says a version Wholeheard does not know, or names a model's host other than huggingface.co, when the list up to date is asked for, then the shipped models stay offered and one sentence says the list could not be brought up to date"
      - "Given a pull request that changes the list, when its checks run, then a list that does not read, names a host not allowed, a size of zero or a fingerprint that is not 64 hexadecimal characters is refused"
---

# Cycle 04 — Wholeheard on macOS

> The framework's words: `.nstack/handbook/glossary.md`.

> Framed on 2026-10-26 on the decider's word ("oui, cadre le build macOS"): after cycle 03, one
> week. **Accepted by the decider on 2026-10-26**, closing cycle 03 early — their choice among
> options, "Clore 03, D27 dans 04", which read "j'ajoute D27 au cycle 04 et tu acceptes 04 dans la
> même PR : D38 démarre tout de suite, fin 2026-11-02": from that day, D27 moved here. It rides
> along: Ubuntu is not this cycle's goal, and its trial is all that is left of it.

## Goal

The charter's second criterion starts the day Wholeheard installs on the colleagues' machines;
nothing installs on a Mac today. Since #416 and #417 the four programs compile there on GitHub's
runners (`cargo check`, macOS 26 arm64), free since the repository is public (PDR-0002). This
cycle turns them into an app the colleague opens, and tries the one thing a Mac might refuse:
recording what it plays.

```mermaid
flowchart LR
    W["macOS workflow<br/>GitHub's runner"]:::forge --> A["Wholeheard.app<br/>four programs, Info.plist"]:::d38
    A --> M["The colleague's Mac<br/>Open Anyway"]:::person
    M --> R["A real call recorded<br/>local · remote"]:::d39
    R --> K["Recorded against<br/>the threshold"]:::out
    classDef forge fill:#374151,color:#fff
    classDef d38 fill:#065f46,color:#fff
    classDef person fill:#1e3a8a,color:#fff
    classDef d39 fill:#92400e,color:#fff
    classDef out fill:#5b21b6,color:#fff
```

**Legend** — dark grey: the forge · green: D38 · blue: the colleague · amber: D39 · violet: what
the cycle records.

## Deliverables

### D38

- The shell already finds the other programs beside its own executable (`program()`,
  `modules/shell/src/main.rs`): side by side in `Contents/MacOS`, they are found with nothing
  changed.
- Built on GitHub's macOS runner, never here: a Mac's app cannot be made in a Linux container
  (ADR-0002: "Windows and macOS are built by the forge"). How the app is put together — Tauri's
  bundler, or install assembling the folder from each module's release as ADR-0006 does for the
  rpm — is an ADR of install's, its prior art read first; the field's convention by default, so
  that the choice does not eat the week.
- Not signed by Apple: macOS stops it the first time. Since macOS 15, right-click, Open no longer
  lets it through; System Settings → Privacy & Security → Open Anyway does, with the Mac's
  password ([Michael Tsai, 2024-07-05](https://mjtsai.com/blog/?p=43993), read by the cycle's
  verifier). Signed ad hoc at least: an Apple chip runs nothing unsigned, and a bundle changed
  after its programs were linked is said *damaged*, which only a terminal gets past.
  Signing is PDR-0001's open question, out of this cycle.
- A new ad hoc signature on every build: macOS may ask for the microphone again with each
  artifact. That a program the app starts is asked for under the app's name, its taps of what
  plays included, is assumed, not tried: D38's third criterion says it.
- The artifact is kept 7 days on the run, the shortest a colleague needs: see *Open questions*.

**Built, each pull request verified by an independent session (2026-10-26).** The oracle first:
the macOS workflow's job *the app* runs `nstack release` on GitHub's runner and reads D38's first
criterion on the zip itself (#423) — red, on transcription's Vulkan; transcription released on a
Mac on the processor (#424) — red, on install's `/etc/os-release`; install assembles
`Wholeheard.app`, its Info.plist in English and French, signed ad hoc, zipped (#425, ADR-0006
amended) — green. `main`'s run 40264092320 keeps `wholeheard-macos-arm64` (11 MB) until
2026-11-02.

**Offered for acceptance** — its acceptance issue, with D39's:
[#426](https://github.com/NapkinStack/wholeheard/issues/426), the colleague on macOS.

| Criterion | Where it stands |
|---|---|
| One zip from the macOS workflow: the four programs in `Contents/MacOS`, an Info.plist naming the application, its version and the two sentences, signed ad hoc, `codesign --verify --deep --strict` passing | Holds — judged on every run of the job *the app*, on `main` since a8b5892 |
| Opened on the colleague's Mac through Open Anyway: the icon in the menu bar, the window, no terminal | The trial (#426) |
| macOS asks for the microphone and the sound the Mac plays naming Wholeheard, with the Info.plist's sentences | The trial (#426) |

### D27

**Moved from [cycle 03](03-the-window.md#d27) on the decider's word, 2026-10-26** (their
choice, "Clore 03, D27 dans 04"): built and verified there, its criteria unchanged but the name, Summit having
become Wholeheard (D28). What is left is its trial: a colleague on Ubuntu 24.04 installs the
package — its acceptance issue [#255](https://github.com/NapkinStack/wholeheard/issues/255). A
dependency with a date, as D39's: by 2026-10-31.

### D39

- cpal 0.18.2, which capture records with, says it records "system audio output on macOS >
  14.6" (its CHANGELOG, after 14.6, not at it): capture's code may need nothing. Not tried yet.
- Run by the colleague, on their call: nothing of it is published (rule of the project); the
  cycle records the counts, never the audio or its words.


Its trial: [#426](https://github.com/NapkinStack/wholeheard/issues/426), after D38's, on the same Mac.

### D40

**Added on the decider's word, 2026-10-26** ("c'est ok pour moi, on peut le faire maintenant,
tranche tu es autonome"), on cycle 03's *Later*: the list of D37 read from a registry kept open on
GitHub, signed — the signature deferred, below. It is not this cycle's goal; it fits what is left of the week without the colleagues,
whose trials are what D38, D39 and D27 wait on. The end date does not move.

- How: [ADR-0010](../../adr/0010-the-models-offered-are-a-list-kept-in-the-repository.md) —
  one file of the shell's, `modules/shell/registry/models.json`, built in as the shipped list
  and read again from `main` only on the user's gesture; refused, the shipped list stays.
- Who may write it: whoever may merge into `main` — NapkinStack's members, and the agent merging alone under rule 2 — with no rule of its own on the folder (`CONTRIBUTING.md`).
- D37's list does not show a model's host today, only its size (`Offered`, `modules/shell/src/main.rs`); D40's first criterion adds it, as security rule 1 asks of a download.
- Not signed: the signature comes before the first build published for people outside the
  project (ADR-0010, *Rejected alternatives*).
- The minutes' model, fetched by Ollama, stays in the code.
## Dependency with a date

- **The colleague on Ubuntu 24.04**, for D27: the trial of #255 by 2026-10-31 (the framer's
  date). Missed, D27 stays where it is at the end date.
- **The colleague on macOS**, for D38's second and third criteria and for D39: one session of
  an hour by 2026-10-31, the app ready by 2026-10-29. The cycle does not hold it: missed, the
  deliverables stay where they are at the end date.

## Out of scope

- Apple's signing and notarisation, a `.dmg`, the App Store.
- Intel Macs, macOS 14.6 and before.
- Windows.
- Anything of the window or of the settings made for macOS beyond opening.

## Later

- The transcript and the minutes of D39's call made on the Mac, the engine on its graphics card
  (Metal), and how long it takes against the call: narrowed out by the decider's appetite (one
  week, 2026-10-26). The charter's second criterion — two weeks of meetings on the Mac — needs it.
- macOS's own transcription engine (SpeechAnalyzer), against Whisper.
- A refusal of macOS's trash said by its cause: today it is said as *other* (#417).

## Open questions

**Blocking:** none.

**Not blocking:**
- Is a run's artifact a build "published for people outside the project" (PDR-0002, decision
  2), which makes capture `critical` first? On a public repository any signed-in GitHub user can download it ([GitHub Docs, *Downloading workflow artifacts*](https://docs.github.com/en/actions/how-tos/manage-workflow-runs/download-workflow-artifacts): "People who are signed into GitHub and have read access to a repository can download workflow artifacts").
  The framer's proposal: no — no release, no link given outside the project, kept 7 days; the
  first Copr, PPA or Mac download page is. ~~*(framer, to confirm)*~~ Answered by the decider on
  2026-10-26: no ("ok pour ta proposition").

## Closure

<Written at the end.>

**Documents brought current:** <at the closure>
