---
goal: "Day to day, Wholeheard is the best of its kind to use: a meeting ends in one notification that opens it, named, its actions read at a glance, its minutes copied in one click, said in a French that sounds native — and the first run holds the user's hand, the announcement one click away — and what a first Store submission waits on: Windows' package made and tried, capture `critical`, the list of models signed, the listing and the privacy policy written"
status: accepted          # proposed | accepted | closed | stopped — the decider accepts
appetite_weeks: 3         # the time the decider WANTS to spend, not an estimate
start: 2026-10-29         # YYYY-MM-DD
end: 2026-11-19           # start + appetite: the first day the cycle is over — the circuit breaker
deliverables:
  - id: D44
    title: "A meeting's notifications are fewer, and a click on one opens the meeting in Wholeheard"
    module: shell
    state: ready
    acceptance:
      - "Given a recording stopped while a Wholeheard window is visible — shown and not minimized, as the toolkit reports it —, when it stops, then no notification says the stop; with no Wholeheard window visible, the stop is said as today"
      - "Given Settings set to make the minutes after the stop (`after = \"minutes\"`), when the transcript is written and the minutes follow, then no notification says the transcript, and one says the minutes; a failure of either step is always said"
      - "Given a notification that a meeting's transcript or minutes are ready, on Linux, when the user clicks it, then the full window opens on that meeting's page (`#/meeting/<name>`) — never the file manager — and its words say so: « Cliquez pour l'ouvrir. » / \"Click to open it.\"; elsewhere a notification promises no click — its words are chosen by the system"
      - "Given a meeting made with `after = \"minutes\"` and no window visible at the stop, when it is made, and nothing failed, no side fell silent and no device changed, then it gave at most three notifications: the reminder at the start, the stop, the minutes ready"
  - id: D45
    title: "A meeting's minutes are copied in one click, ready to paste where the user writes"
    module: shell
    state: ready
    acceptance:
      - "Given a meeting with minutes, when the user clicks « Copier le compte rendu » / \"Copy the minutes\", beside *Refaire…*, then the clipboard holds the minutes' body as Markdown text, without its front matter, and « Copié » / \"Copied\" is said in place (`role=status`) for a moment"
      - "Given a meeting without minutes, or minutes that do not read, when its page shows, then no copy is offered"
      - "Given the clipboard refused by the web view, when the user clicks, then one sentence says the minutes could not be copied, and nothing else changes"
  - id: D46
    title: "A meeting's actions are rows: Qui · Quoi · Pour quand"
    module: shell
    state: ready
    acceptance:
      - "Given minutes whose section 2 holds items written as `minutes/v1` says — what — who — by when, separated by ` — ` —, when the meeting's page shows them, then each is one row of three cells, who in its colour and by when in monospace, with no visible headers, as board 2 of D11 drew them — Qui, Quoi, Pour quand their names for assistive technology —, in the order the decider settles (open question)"
      - "Given an item that does not split into exactly three parts on ` — `, when it shows, then it is shown as written, never cut or guessed"
      - "Given « non précisé » / \"not specified\" in a cell, when it shows, then it is shown in the soft colour, as what the meeting did not settle"
  - id: D47
    title: "Wholeheard speaks a native, short French — and never shows English to a French user"
    module: shell
    state: ready
    acceptance:
      - "Given the wording table of the review of 2026-10-26 (`docs/project/reviews/2026-10-26-ux.md` §10), as the decider amends it at the cycle's acceptance, when Wholeheard speaks French, then each of its sentences is the one adopted, and the English sentences the review names beside them are rewritten with them"
      - "Given every French sentence the window and the notifications say, when the shell's tests run, then a test refuses one without U+202F before `? ! ;` or without U+00A0 before `:` — times (`12:21`) and addresses (`https:`) aside"
      - "Given a command the page calls that fails — a settings file, a path, a key, the system —, when it fails, then the page says it in a sentence of `words.rs`, in the user's language; the raw text goes to the log and, in developer mode, under the sentence; a test refuses a command that hands the page a text of its own (`Err(\"…\")`)"
  - id: D48
    title: "A meeting has a name: the one its minutes give it"
    module: summary
    state: ready
    acceptance:
      - "Given `contracts/minutes/v2`, a new version of `minutes/v1` (ADR-0005 keeps v1, a page of Markdown, frozen), when its page is written, then it says an optional `title` in the front matter — one line, a few words in the meeting's language, at most 80 characters, never a key or a path — in a pull request of the contracts module alone, merged before the summary writes it; the shell reads `minutes/v1` and `minutes/v2` alike, with or without a title, before the summary writes v2"
      - "Given a transcript, when the summary writes its minutes, then their front matter carries the title the provider's answer gives, where it gives one — a real model's title is a scenario of the sheet; given a model that gives none, or one longer than 80 characters or on several lines, then the minutes are written whole without it, never refused for it"
      - "Given the list of meetings and a meeting's page, when a meeting's minutes carry a title, then the row and the page's heading show the title — the date beneath it in the list (board 1), above it on the page (board 2); without minutes, or minutes made before, the date as today"
      - "Given a title that holds markup — `<img src=x onerror=alert(1)>` —, when it shows, then it is shown as text, never as markup, held by a test (`security.md`)"
      - "Given the minutes made again, when they are written, then the meeting takes the new title"
  - id: D49
    title: "The first recording holds the user's hand: each side heard checked live"
    module: shell
    state: ready
    acceptance:
      - "Given a recording started, when it records, then the small window shows, for each side — « Votre micro » / \"Your microphone\", « Le son de la réunion » / \"The meeting's sound\" —, a check that turns ✓ as soon as its levels rise above silence (`level`, `recording/v1`), then fades"
      - "Given a side that stays silent, when capture says it silent, then the small window says under its check what to do, as well as the notification it says today: for a microphone that sends nothing (`Said::Nothing`, #457), the sentence of `sends_nothing` — check the device's own mute; otherwise (`Said::Silent`), speak, or play a sound in the call"
      - "Given the small window at 380×400 while recording, with the checks in each meter's device slot, a silence's sentence, D50's line, the context open and D60's switch, when it shows in either theme, then nothing overflows or is cut — a departure from board 6 and D34, screenshots of both themes in the sheet"
  - id: D50
    title: "The reminder to announce the recording is kept, shown where the user looks, the announcement one click away"
    module: shell
    state: ready
    acceptance:
      - "Given a settings file that does not say `reminder`, or none, when Wholeheard starts, then the reminder is on (`Settings::default().reminder`), held by a test; Settings still let the user turn it off (charter, 2026-10-13)"
      - "Given a recording with the reminder on, when it records, then the small window says « Prévenez les participants que cette réunion est enregistrée. » / its English, under the timer, in the soft colour, for the recording's first fifteen seconds *(framer, to confirm: UX review 1.2)* — as well as the notification at the start, so that it shows with notifications off or held back"
      - "Given that line, when the user clicks « Copier l'annonce » / \"Copy the announcement\", then the clipboard holds a sentence ready for the call's chat, in the meeting's language when it says one in French or English (D65), else in the language of Wholeheard's window, and « Copié » is said in place; the sentence is the decider's, adopted at the cycle's acceptance (D50's section)"
      - "Given the reminder off, when a recording starts, then neither the line nor the button shows"
  - id: D54
    title: "On Windows, Wholeheard installs from one MSIX package the Microsoft Store can take, and removing it keeps the meetings"
    module: install
    state: ready
    acceptance:
      - "Given the Windows workflow on main, when it runs, then it leaves beside the zip Wholeheard-<version>-x64.msix, made with the Windows SDK's MakeAppx from an AppxManifest.xml that names Wholeheard, its version in four parts the last 0, its logos, the identity the decider reserved in Partner Center — or a placeholder the run says is one —, `runFullTrust` and the microphone, and the shell started with `--window`; the four programs inside it"
      - "Given that package signed for the run with a certificate made for it, when GitHub's Windows runner installs it (`Add-AppxPackage`), then Wholeheard's process starts from it with its window; when a later version is installed over it, then a meeting folder placed under Documents\\Wholeheard and the settings are still there; and when it is removed (`Remove-AppxPackage`), then the meeting folder is still there"
      - "Given the package, when its notes for certification are read (`modules/install/windows/certification.md`), then they say why Wholeheard needs `runFullTrust` and the microphone — recording a call's two sides —, and that its settings and downloaded models live in the package's own data, which Windows removes with it, its meetings never"
      - "Given the colleague of cycle 04's D53 on their Windows 11, when they install the package signed for the run and record a real call of five minutes from it, then the cycle records whether both tracks hold the call's speech — capture working from inside a package, PDR-0004's removal condition"
  - id: D55
    title: "capture is critical before the first build published outside the project"
    module: capture
    state: ready
    acceptance:
      - "Given capture's MANIFEST.yaml, when it is read, then its criticality is `critical`, as PDR-0002 decided for the first build published outside the project — the Store's (PDR-0004)"
      - "Given capture's manifest, when the fitness functions run, then it declares its runbook (`docs.runbook`), and the runbook says how a recording that failed is found in the logs, what a user is told, and how a release is rolled back — on Linux, the previous package installed again (`dnf downgrade`, tried once by the framer on a Fedora made for it, its date written); in the Store, which takes no lower version, the previous code submitted again under a higher one"
      - "Given the rows `critical` makes due (`.nstack/handbook/05-workflow.md` §7) — every scenario run again, UAT, rollback verified, after a release verified, E2E on critical journeys —, when the runbook is read, then it says who does each for capture and when"
  - id: D56
    title: "The list of models read again is refused unless the decider's key signed it"
    module: shell
    state: ready
    acceptance:
      - "Given `registry/models.json` and its signature `models.json.minisig` on main, made with minisign by the decider's key — the public key built into the shell, the private one never in a repository —, when the list is read again on the user's gesture, then it is offered only when its signature verifies; a list unsigned, signed by another key, or changed after it was signed is refused, and the shipped list stays, said in one sentence (ADR-0010, *Rejected alternatives*)"
      - "Given a pull request that changes the list, when its checks run, then a list whose signature does not verify against the built-in key is refused; until the decider's key is given, no list is read again, and Settings and the check say it waits on that key — the colleagues' builds lose today's reading again, said in the cycle's closure"
      - "Given ADR-0010, when D56 is merged, then the record says its option 4 adopted, `minisign-verify` declared in the shell's manifest with its exit strategy, and what a signature does not stop: an older list, signed, offered again"
      - "Given the shell's tests, when they run, then a list signed by a key made for the test is offered, and the three refusals above are held, without the decider's key"
  - id: D57
    title: "The Store's listing and Wholeheard's privacy policy are written, in French and in English"
    module: null
    state: ready
    acceptance:
      - "Given the repository, when `docs/privacy.md` is read on GitHub, then it says in French and in English what Wholeheard records, where it keeps it, what leaves the machine only when the user chose a service and to which host, that nothing is sent otherwise, and how to remove it all — its URL the one Partner Center is given"
      - "Given `docs/store/listing.md`, when it is read, then it holds the Store's description, its short features and the screenshots it needs, in French and in English, said as PDR-0001's promise — the decider reads it and says it claims nothing the product does not do"
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
  - id: D41
    title: "On Linux, a call heard on an output other than the system's default is recorded there"
    module: capture
    state: in-progress
    acceptance:
      - "Given a call — an application other than capture holding a microphone — whose sound plays on an output other than the system's default, when it is recorded, then the remote track holds what the call plays, and capture says `device` with that output's name"
      - "Given the call's sound moved to another output while recording, or back to the default, when it moves, then the remote track follows it within two seconds and the move is kept in `session.json` with when"
      - "Given no call, a call whose sound plays on the default, or on several outputs at once, when it is recorded, then the remote track records the system's default output, as D31 does"
      - "Given a graph that cannot be read or a stream that cannot be moved, when it is recorded, then the recording goes on from the default, and one warning is logged"
      - "Given capture's e2e, when a stand-in call plays a note on an output of its own that is not the default, then the remote track holds that note — on devices made for the run, nobody recorded"
  - id: D42
    title: "On a Mac with macOS 26, Apple's SpeechAnalyzer hears a meeting, chosen in Settings"
    module: transcription
    state: in-progress
    acceptance:
      - "Given a session and Apple's model of the system's language — French or English — on a Mac with macOS 26, when it is transcribed with `--engine speechanalyzer`, then `transcript/v1` is written by the step as ever — each segment on its track with its times —, its header naming the engine SpeechAnalyzer, and no line of `src/step.rs` changed to add it"
      - "Given the macOS workflow, when it runs, then it builds `wholeheard-speech` beside the other programs and transcribes back the words of a French sentence `say` speaks; where the runner offers no SpeechTranscriber or no French voice, the step says it was not run, and this criterion is met on the colleague's Mac instead"
      - "Given D38's app, when it is made, then `wholeheard-speech` is in `Contents/MacOS`, signed with the rest"
      - "Given Settings on a Mac with macOS 26 and French supported, when the user opens the choice of who hears, then SpeechAnalyzer is offered beside the Whisper models; its model not on the Mac, Settings say macOS fetches it from Apple and offer it on a click, nothing fetched before; elsewhere — Linux, an older macOS, a language unsupported — it is not offered"
      - "Given the colleague's real call of D39 on their Mac, when it is transcribed by SpeechAnalyzer, then the cycle records how long it took against the call and the colleague's reading of it — nothing of the call published —, read against the threshold: less time than the call lasted"
  - id: D53
    title: "On Windows, Wholeheard opens from one zip the forge made — and does a call recorded there hold both sides?"
    module: install
    state: in-progress
    acceptance:
      - "Given a run of the Windows workflow on main, or one started by hand, when it ends, then each program's tests pass on GitHub's Windows runner, and it leaves one artifact, Wholeheard-<version>-windows-x64.zip, holding wholeheard.exe and the three programs side by side, kept 7 days as the Mac's"
      - "Given that zip on the colleague's Windows 11, when they unpack it and open wholeheard.exe — past SmartScreen's warning on an unsigned program —, then Wholeheard's icon is in the notification area and its window opens from it, with nothing installed before it and no terminal"
      - "Given a call recorded there for at least five minutes, when it stops, then the cycle records, for each track — local, the microphone; remote, what Windows plays: whether it holds the call's speech, and what Windows asked before it recorded — read against D39's threshold: both tracks hold the call's speech as the colleague hears it at the start, the middle and the end of each, and capture's code holds no line added for Windows; below it, what was missing is recorded for the next framing. A refuted result is a delivered one"
  - id: D58
    title: "At the first start, one or two questions set Wholeheard up, said plainly"
    module: shell
    state: in-progress
    acceptance:
      - "Given a start where the setting `welcomed` is not true and the folder of meetings holds none, when Wholeheard starts — with `--window` or not, on Linux, macOS and Windows —, then the full window opens on the assistant of D58's board (`docs/project/drawings/d58-the-first-run/`) and `welcomed` is written true at once, so that it opens by itself once only — a start with the session included, the one departure from D35's « started again with the session, nothing does », once —; a start with a meeting already there, or with `welcomed` true, never shows it, and closing it midway changes nothing else"
      - "Given the first screen, nothing chosen in advance, when the user answers « Oui, souvent » or « Je ne sais pas », goes on with none, or chooses « Passer », then the setting is private: the transcription on this computer — Whisper with the model the settings name, or Apple's SpeechAnalyzer where it is chosen already — and the minutes by Ollama on this computer, its model the settings name or the recommended one; given « Rarement ou jamais » and « Un service en ligne », then the last screen's « Choisir mon service » opens Settings at the minutes, where the service, its model and key are chosen and tested as ever, its host agreed before the first sending as ever"
      - "Given the last screen, when it shows, then it says the mode — « Mode privé » or « Mode connecté » — and who transcribes, who writes the minutes and the reminder in plain words, the models by their spoken names; in private mode, the model missing, « Télécharger le modèle » fetches it as Settings do, its size and host shown, the window then going to the meetings while it is fetched, a failure or a refused fingerprint said as Settings say it — the model there, « Terminer »; Ollama not answering, it says the minutes wait for Ollama, with ollama.com; « Plus tard » leaves the model unfetched; connected and no service chosen afterwards, the first meeting's minutes say none is chosen and open Settings, as today"
      - "Given Settings, when the user chooses « Refaire le réglage », then the assistant opens, « Passer » reads « Annuler », and nothing changes until the last screen's button; confirming private switches who transcribes and who writes the minutes to this computer and nothing else — never the model file, the keys, the reminder or the folders; confirming connected opens Settings at the minutes and changes nothing by itself"
      - "Given the assistant, when it is gone through with the keyboard alone, in either theme, in French and in English, then each question is a group of radios, the focus is always seen, no emoji, nothing overflows at the full window's smallest size (420 × 400) — screenshots of both themes in the sheet —, the window left to reopen is the one it was (D35), and the window's e2e goes through every path, its other scenarios unaffected"
  - id: D63
    title: "A service that hears is chosen as the minutes' is: from a list, its transcription models offered"
    module: shell
    state: in-progress
    acceptance:
      - "Given Settings, when the user chooses who hears online, then the services the minutes offer that publish OpenAI's `audio/transcriptions` — OpenRouter, OpenAI, and Mistral if it does — are offered by name, their address filled in as the minutes' are, and « Un service que je choisis » keeps an address of their own"
      - "Given a service chosen, when Settings show it, then its transcription models are offered to choose from — OpenRouter's read from its public list, `models?output_modalities=transcription`, asked with no key and nothing of a meeting; the others' that give segments and their times named in the repository —, a model typed by hand still possible, and *Tester* says whether the one chosen gives segment times, the list not saying it; its key kept in the system's keyring as the minutes' is"
      - "Given a service and a model chosen, when the user tests it or a meeting is transcribed, then it is as D43 says — a sound of the test's own, the host agreed first, a model that gives no segment times refused and said —, with the service's own address"
      - "Given the shell's tests and the window's e2e, when they run, then a stand-in service listing its models is offered, chosen and tested, and its key never written in the settings"
  - id: D66
    title: "A recording holds the microphone the call holds, and never touches a Bluetooth headset's profile"
    module: capture
    state: ready
    acceptance:
      - "Given a call holding a microphone other than the default input, when a recording runs, then `me` records that one — its name in `session.json` and the `device` event — and, when the call changes it, follows it, the track going on; held on virtual devices by capture's e2e, a stand-in call on a source of its own"
      - "Given a source carrying `bluez5.loopback=true`, when capture records it, then its stream says its version's passive value (ADR-0015), the source never runs from it alone and no profile switch comes from it, within one known window: a default input turning into a headset during a recording is on an active stream for at most one reading of the graph, about 1 s, and a switch came 4 times in 10 measured; held by capture's e2e on a stand-in source, read in `pw-dump` — WirePlumber's decision testable on a stand-in carrying `device.id` —, and `others` is unchanged. Given no call holding a microphone, then capture records the meeting's sound and says in one sentence that the microphone waits when the default input is such a source; records the default input, as today, when it is not"
      - "Given a call that lets go of the microphone, when it leaves, then capture lets go of it too: within what WirePlumber's restore takes, it holds no HFP profile by a stream of its own and the source no longer runs — held by the same e2e, the source's state in `pw-dump`, and by the decider's trial, the card's profile"
      - "Given a transport failure during a recording — a source taken away, on the e2e's stand-in —, when it fails, then the recording continues, the track's gap is marked as `session-audio/v2` allows (`clock.jsonl`'s `silence`, `changed` with `device: null`), and one sentence says it (`recording/v1`, an optional field, additive; the shell's words in its own pull request)"
      - "Given the decider's AirPods and a real WhatsApp Web call, when they start and stop a recording during it, at least five times, then the call is never cut, no `Failure in Bluetooth audio transport` is logged within two seconds of capture opening or closing a stream, and `local.wav` holds their voice each time; an acceptance issue says what was seen"
---

# Cycle 05 — The best day to day

> The framework's words: `.nstack/handbook/glossary.md`.

> Framed on 2026-10-26 on the decider's word: their choice that day, « Cycle 05 + bug tout de
> suite » — a cycle 05 after cycle 04, « le meilleur au quotidien ». It starts on cycle 04's
> `end`, the first day outside it, and is accepted only once cycle 04 is closed: one accepted cycle at a time.
> Its source is the review of 2026-10-26 the decider asked for — [the market](../reviews/2026-10-26-market.md)
> and [the surface](../reviews/2026-10-26-ux.md), each by an independent session — as cycle 04's
> *Later* says. The criteria name symbols, never lines: lines move. The file:line of the sections
> below were read at commit 66b1ff7, by the reviews or the framer, and have moved since (the
> verifier of #445; this page's reviewer, 2026-10-26).
> **Accepted by the decider on 2026-10-29**, closing cycle 04 early, on their word that day: « cadre oui mais il faut commencer … il nous faut continuer le projet et le terminer, les dates sont des estimations ».
> Each open question below is settled by the framer's default, the decider amending any of them at will;
> the appetite three weeks, not two, as the question on it foresaw. From that day, cycle 04's eight
> deliverables left — each built, waiting on a person's trial — moved here (*Moved from cycle 04*).

## Goal

Cycle 03 built the window to its boards; cycle 04 brings it to the Mac. The charter's criteria
count weeks of meetings with Wholeheard instead of a bot, and the market moved: botless is now
every leader's default, and a local stack of Whisper and Ollama is a weekend project (market
review §0, §2). What is left to win on is the finish of each day's meeting. The review found
the bones good and named what keeps them from feeling high-end: a meeting has no name, its
result is hard to take away, up to four notifications a meeting, one of which opens a file
manager (UX review, verdict). The decider chose from it, on 2026-10-26: the notifications that
open the meeting in Wholeheard and are fewer; the minutes copied; the French wording; actions as
rows (Qui · Quoi · Pour quand); a meeting's title; the first run — the model card, a live audio
check, the reminder in the small window, « Copier l'annonce »; the informational cost an AI call
reports (OpenRouter's `usage.cost`, never an estimate); and the reminder kept, « Garder, mais
jamais éteint par défaut ». On 2026-10-27 they asked for the version complete enough to publish
« sur les store » and left the choice to the framer (PDR-0004): Windows' Store package, `capture`
`critical` and the list of models signed join the cycle; the cost and the live spike wait. Their standing asks hold for each: modern, high-end, playful, the
easiest, non-intrusive, fluid; no over-engineering; words chosen with care.

```mermaid
flowchart LR
    S["Start<br/>reminder · Copier l'annonce"]:::d50 --> C["Each side heard<br/>✓ live"]:::d49
    C --> T["Stop"]:::rec
    T --> N["One notification<br/>opens the meeting"]:::d44
    N --> M["The meeting, named<br/>actions as rows"]:::d48
    M --> P["Copied<br/>to mail, chat, Notion"]:::d45
    W["Wording, French first"]:::d47 -.-> S & N & M
    Z["Ready to submit<br/>Store package · capture critical<br/>list signed · listing, privacy"]:::pub
    classDef d50 fill:#92400e,color:#fff
    classDef d49 fill:#065f46,color:#fff
    classDef rec fill:#374151,color:#fff
    classDef d44 fill:#1e3a8a,color:#fff
    classDef d48 fill:#5b21b6,color:#fff
    classDef d45 fill:#0f766e,color:#fff
    classDef d47 fill:#4b5563,color:#fff
    classDef pub fill:#7c2d12,color:#fff
```

**Legend** — one meeting, left to right · amber: D50 · green: D49 · grey: what is there today ·
blue: D44 · violet: D48, with D46 on the same page · teal: D45 · slate: D47, said
everywhere · red: D54–D57, what a first submission waits on.

## Deliverables

Each is the shell's but D48, whose minutes are written by the summary, D54, install's package, and
D55 and D66, capture's, and D57, words outside the modules. Ordered by the framer — the review's value over cost (UX review §11), where a deliverable leans on
another, after it: the order to build them in. **Cut first** if the breaker nears: D47's errors, then D49's live check. D54–D57 are not cut:
they are what publishing waits on.
D44, D45, D47 and D50 all touch `words.rs` and the « Copié » status: one session at a time on them.

### D44

- Today: up to four notifications a meeting — the reminder, the stop, the transcript, the minutes
  (`words.rs:175-176, 171`, `main.rs:2188-2195`); « Transcription prête » opens the session's
  folder with `xdg-open` (`tell_opening`, `main.rs:2393-2415`), « Compte rendu prêt » opens
  nothing. The window already shows the stop and the making where the user is (D35: "what the
  user waits for is shown where they are").
- The reminder at the start stays a notification in every case: it is the charter's (C7).
- **Linux only for the click**: elsewhere the toolkit's plugin tells no click (the shell's
  `MANIFEST.yaml`, `notify-rust`; `main.rs:2417`) — a notification there is a notification alone,
  as today. The click on a Mac is *Later*.
- With D47: "session" leaves the notifications' words for "réunion" (UX review §10, `Made::Written`,
  `Said::Failed`).

### D45

- Markdown text, the body only: the code already writes to `navigator.clipboard` for the
  diagnostic (UX review 4.1). Rich text beside it — what a mail pastes as headings — is not
  promised: whether the system's web view takes `text/html` on the three systems is not known
  *(assumption)*, and not tried in this cycle.
- Export, Notion, a file: *Later*, as cycle 02 deferred them (its *Later*).

### D46

- The contract already guarantees the separator: `contracts/minutes/v1`, Actions, "what, who and
  by when, in that order, separated by ` — `". Rendered in `minutes.rs` of the shell; the contract
  unchanged (UX review 4.2).
- Board 2 of D11 drew what · who · when; the market review's recommendation 3 says Qui · Quoi ·
  Pour quand; the decider chose actions as rows (cycle 04's *Later*): see *Open questions*.
- The copy of D45 keeps the items as written.

### D47

- The table of the UX review §10 is a proposal, not decided words: the decider adopts or amends
  it when they accept the cycle (*Open questions*); the market review's §7 is read with it (Compte rendu, Décisions,
  Actions, Points clés, Intervenants).
- The errors handed to the page as English text (`main.rs:414-415, 508, 916-946, 1100-1134`) go
  through an enum said by `words.rs`, as `summarising::Cause` already is (UX review 5.1).
- *Transcript* or *Transcription* in French: see *Open questions*.

### D48

- **`minutes/v2`, not `minutes/v1`**, on the decider's word, 2026-10-30, choosing « minutes/v2 » among
  three: ADR-0005, accepted, keeps `minutes/v1` frozen — a page of Markdown, which the project's
  comparator of JSON Schemas cannot judge. The title goes in a new version: `minutes/v2` is v1 and an
  optional `title`; the shell reads v1 and v2, then the summary writes v2. #562, which gave v1's
  front matter a schema to change it, is closed.

**The title written by the minutes, rather than the review's cheap fix** *(framer, to confirm)*.
The UX review weighed two forms (3.1):

| | The cheap fix | The title of the minutes |
|---|---|---|
| What | The shell shows the first sentence of *Points saillants*, cut at about 90 characters | The summary writes an optional `title` in the front matter, an additive change of `minutes/v1` |
| Modules | shell | contracts, then summary, then shell — one pull request each |
| Gives | A sentence cut short: « La date de l'événement est fixée au jeudi 3 juin, avec le 10 en… » | A name, as every board drew it: « Lancement — calendrier, budget, recrutement » |
| Cost | S | M |

The framer proposes the title: the boards, accepted as criteria, drew names; the contract's pull
request is small and additive; and a name is what a later
search and export would share. The cheap fix stays the fallback if the decider prefers S.

- Additive (`.nstack/handbook/03-contracts.md` §3): no consumer changes to keep working — the
  shell reads the front matter into a table of keys (`minutes.rs:49-53` at 66b1ff7) and `export`
  consumes no contract (its `MANIFEST.yaml`). The contract's page changes in its own pull request,
  never in the consumer's (§7), as `recording/v1`'s `device` did before capture wrote it (#439,
  #441).
- A meeting without minutes keeps its date: renaming it by hand is *Later*.
- Whether the small models Ollama offers write a usable title is not known *(assumption)*: the
  minutes are written without one rather than refused (criterion 2).

### D49

- Its first criterion — the model offered before the first meeting, board 5's card — went to
  cycle 04's D58 on 2026-10-27: the assistant's last screen offers the model.

- Board 5 of D11 drew the card, « Summit a besoin d'un modèle… » with *Télécharger le modèle* /
  *Plus tard*; today the 874 MB model is offered only after the first recording, through a
  notification (UX review 1.1, `window.js:1600-1602`). The card reuses `model_fetch` and
  `modelFetch()`.
- The live check rides on what capture already says — `level` about ten times a second for each
  side, `silent` after ten seconds — so capture and its contract are unchanged. It shows on every
  recording, not the first only *(framer, to confirm)*: no state to keep, and a device changed
  since is caught too.
- On a Mac, a tap of what plays that lacks its permission records silence with no error, and no
  interface says whether it was granted (market review §5, [Apple forum 771864](https://developer.apple.com/forums/thread/771864)):
  the check is how the colleague sees it.
- The small window keeps its fixed size (D35); the check takes the room left under *Arrêter*
  (UX review 2.2: about 40 % free). With D50's line it departs from board 6 and D34, which drew the
  time, the levels and *Arrêter* alone: named here, held by criterion 3's screenshots, no new board — with D59's field and D60's switch since (*Read again after cycle 04*).

### D50

- The decider's word, 2026-10-26: « Garder, mais jamais éteint par défaut ». The reminder is on
  by default today (`settings.rs:106`) and a setting removes it (charter, open questions,
  2026-10-13): nothing of the charter changes, so no PDR — D50 pins the default with a test and
  shows the reminder where notifications cannot hide it (macOS asks for them; GNOME's *Ne pas
  déranger* holds them back: UX review 1.2, `words.rs:98-106`, `main.rs:1824`).
- « Copier l'annonce »: Notion's consent step, Apple's audible notice and Meet's notice set the
  bar, and *Chamberlain v. Granola* (filed 2026-07-30) is about botless notes with the notice off
  by default (market review §0, §2, recommendation 6). The sentence, the framer's proposal for the
  decider to adopt or amend:
  - FR: « J'enregistre cette réunion pour en faire le compte rendu. Dites-le-moi si vous préférez
    que je ne le fasse pas. »
  - EN: "I'm recording this meeting to write up its minutes. Tell me if you'd rather I didn't."
- Posting it into the call's chat itself is an integration: out of scope.

### D54

**Added on the decider's word, 2026-10-27** (« il nous faut la version complete pour commencer la
publication sur les store … tu tranches »), after [PDR-0004](../../pdr/0004-windows-through-the-microsoft-store.md):
Windows through the Microsoft Store, an MSIX that Microsoft signs. D51 and D52 went to *Later*
to make room, the appetite unchanged — the framer's cut, the decider's to reverse.

- The zip of cycle 04's D53 holds the four programs; the package holds the same, with its
  manifest. The Store signs what it publishes; outside it, the package installs only signed, so
  the run signs it with a certificate of its own, thrown away after.
- Where Wholeheard keeps things on Windows, read when it is built: the meetings go to the
  user's Documents (the shell's default), which a package's removal leaves; its settings and logs
  go to the application data, which Windows redirects into the package's own and removes with it
  ([Advanced Installer, *MSIX file redirection*](https://www.advancedinstaller.com/msix-files-redirection.html),
  PDR-0004's risk). The deb keeps both (D27); keeping the settings too is *Later*.
- The identity in the manifest is the one Partner Center gives once the name is reserved: until
  then, a placeholder the run says is one, and the package is not submitted.
- On a hosted runner no device sounds: the package's run places a meeting folder, it records
  nothing; the recording from inside a package is the colleague's criterion.
- GPL-3.0 beside the Store's own licence terms, which Partner Center lets the publisher replace:
  the decider's to read before the first submission (*Dependency with a date*).

### D55

- PDR-0002, decision 2: `capture` becomes `critical` before the first build published for people
  outside the project. The Store's package is that build (PDR-0004, *Impacts*). The decider settled
  it then; this deliverable carries it out before D54's package is submitted.
- What `critical` costs, from `.nstack/handbook/05-workflow.md` §7: every scenario run again at
  the head, a runbook. The module's code does not change.

### D56

- ADR-0010 deferred the signature to "before the first build published for people outside the
  project": the list read again would then be trusted by strangers' machines. The Store's package
  is that build.
- minisign: the established tool for signing a file with a key pair, its format read by the
  `minisign-verify` crate in Rust *(framer, to confirm when built: the crate's version and
  maintenance)*. The private key is the decider's, made with `minisign -G` and kept outside any
  repository; each change to the list waits on their signing gesture — ADR-0010's accepted cost.
- A key made for the tests holds the refusals; the decider's public key is written into the shell
  the day they give it (*Dependency with a date*).

### D57

- Microsoft's policies ask an app that handles personal data for a privacy policy, its URL given
  in Partner Center *(framer, from memory of the Store Policies, 10.5.1: read again when it is
  written)*: Wholeheard records meetings. A page of the repository, read on GitHub, is its home —
  no site of the project's.
- The listing's words say what the README and PDR-0001 say; its screenshots are the window's, as
  the e2e takes them. No module: words only.

### D66

**Added on the decider's word, 2026-10-30**, « corriger de manière fiable et stable, durable », after
the decider's call of that day on the laptop's muted microphone and #494. A defect that became a
deliverable, for what its fix decides: [ADR-0015](../../adr/0015-the-local-side-records-the-microphone-the-call-holds-passively.md),
proposed, holds the evidence and the options; the recommended build is the `pipewire` crate, for this
stream, on Linux.

- **Built in order**, in pull requests of one module each: the unknowns of ADR-0015 tried first —
  a passive stream with no call above all; `recording/v1`'s optional field, alone; capture; the shell's
  sentence. macOS and Windows: the same product contract, other adapters: **not in this deliverable**.
- **The sentences**, *(framer, to confirm)*: « Votre micro attend l'appel : le son de la réunion est
  enregistré, le micro le sera dès qu'un appel le prend. » / "Your microphone is waiting for the call: the
  meeting's sound is recorded, and the microphone will be as soon as a call takes it." — a gap: « Le micro
  a coupé un instant ; l'enregistrement continue. » / "The microphone cut out for a moment; the
  recording goes on." Today's `device: null` says the computer has no microphone left: false here.
- **Not built** (ADR-0015, *Rejected alternatives*): the user's configuration written or the autoswitch
  off, a card's profile forced, `stream.monitor=true` set to hide from the script, heuristics per headset,
  a modal on each device.
- **If the appetite does not hold**: it does not move, and D66 comes before the cuts named above, a
  recording that cuts a call being worse than any of them. Inside D66, the gap's sentence goes first —
  the gap stays marked in the track — then the following of a microphone that changes mid-call; never
  the second criterion.
- **The criteria that need a person**: the fifth is the decider's; what a machine holds without Bluetooth
  is the graph, never WirePlumber's script at work, which has no card to act on there.

### Moved from cycle 04

D27, D38, D39, D41, D42, D53, D58 and D63 moved here on 2026-10-29, when the decider closed cycle 04.
Each is built and verified; what is left of each is a person's trial, and each section of
[cycle 04](04-wholeheard-on-macos.md) still holds its framing, its pull requests and its departures.

| Deliverable | What is left | Who, where |
|---|---|---|
| D38, D39, D42 | The forge's app opened, a real call recorded, SpeechAnalyzer heard | The colleague on a Mac, [#426](https://github.com/NapkinStack/wholeheard/issues/426) — and D61's ⌘B there |
| D53 | The zip opened, a real call recorded | The colleague on Windows, [#482](https://github.com/NapkinStack/wholeheard/issues/482) |
| D27 | The package installed on Ubuntu 24.04 | The colleague on Ubuntu, [#255](https://github.com/NapkinStack/wholeheard/issues/255) |
| D41 | The decider's call of #436 recorded again on another output (ADR-0011) | The decider |
| D58 | Its first criterion on macOS and Windows | The colleagues, in the trials above |
| D63 | A real service heard, the decider's key | The decider, [#463](https://github.com/NapkinStack/wholeheard/issues/463) |

## Dependency with a date

- **The colleagues' trials** of *Moved from cycle 04* — Mac, Windows, Ubuntu — by 2026-11-12, the
  forge's builds kept until 2026-11-05 and made again from `main` on asking. Missed, those
  deliverables stay where they are at the end date. D54's fourth criterion — a call recorded from the
  package — needs D53's colleague too.

- **The decider's Partner Center account** (a company's, PDR-0004) and **the name Wholeheard
  reserved** in it, for D54's identity: by 2026-11-09. Missed, the package is made with its
  placeholder and is not submitted.
- **The decider's reading of the Store's licence terms beside GPL-3.0**, and **the age rating**
  (Partner Center's IARC questionnaire): both theirs, before the first submission, by 2026-11-13.
- **The decider's minisign key**, for D56: `minisign -G`, its public key handed over, by
  2026-11-09. Missed, the shell holds the refusals with the tests' key and offers no list read
  again until it is given.

## Out of scope

The review's "do not build" (market review §4, §8):

- Automatic speaker identification (diarization).
- An assistant during the meeting, live minutes.
- Chat over notes, across meetings.
- Team spaces, seats.
- A CRM, a Slack bot, any integration posting for the user.
- Public links to a meeting or its minutes — never.

And for this cycle:

- The live preview built into the product.
- A notification's click on macOS and Windows.
- A price estimated before a call, a running meter, a monthly total, a table of prices.
- Export to a file or to Notion; rich text promised in the copy.

## Later

- **D51, the cost an AI call reports, and D52, the spike on a live transcript** — framed for this
  cycle on 2026-10-26 (their criteria in this page's history, at #461), moved here on 2026-10-27 to
  make room for publishing (D54–D56).
- The Store's trial on Windows: Wholeheard reads the Store's licence and records no new meeting
  once it ends (PDR-0004, decision 1) — after D54, tried through Partner Center's package flight.
- macOS notarised — Apple's developer programme, the decider's —, a trial and a licence key, a
  merchant of record (PDR-0001); the Mac App Store after a spike on the sandbox (PDR-0004).
- Settings and models kept when Windows' package is removed, as the deb keeps them.
- Starting with Windows: the package's `desktop:StartupTask` — the shell's start with the
  session is XDG's only (`modules/shell/src/autostart.rs`).

- The cost of an online transcription (D43), with the minutes' (D51).
- The live preview built, on D52's record and its ADR; SpeechAnalyzer's live results on a Mac.
- Each decision and action linked to its passage, the audio played there (market review,
  recommendation 2): what Granola, which deletes the audio, cannot do.
- Naming the other side by hand, once, everywhere; meeting types as contexts (market review 7, 8).
- A notification's click on a Mac.
- A meeting renamed by hand — the decider's [#504](https://github.com/NapkinStack/wholeheard/issues/504), a title given while recording or on its page.
- The source chosen by hand beside *Auto*, as [Granola's](https://docs.granola.ai/help-center/troubleshooting/transcription-issues) (after D66).
- A discreet indication when a new device arrives — never a dialog, as Discord's is criticised *(the decider's reading, no source read)*.
- The rest of the UX review's ranked list: the full window's empty state and a caught `listed`
  failure (3.7), *Démarrer*'s instant feedback (3.3), *Refaire…*'s *Annuler* (4.4), playback
  speed (4.7), search highlighted (4.6), the tray's repair entries (6.3, departs from D34's board).

## Read again after cycle 04

*(framer, 2026-10-29)* Cycle 04 built D58–D65 since this page was written: the first run's two
questions, a meeting's context and « Réunion confidentielle », the window in two panes, a line
corrected, a service that hears chosen by name, the app's language, each meeting's language. Read
against them, deliverable by deliverable:

| Deliverable | What cycle 04 changed for it | The framer's proposal |
|---|---|---|
| D44 | A meeting opens in the pane beside the list (D61): `#/meeting/<name>` still names it; the notifications speak the language chosen (D64) | Unchanged |
| D45 | A corrected line puts « Refaire avec les corrections » in a block of its own under the minutes' maker (D62); *Refaire…* stays in the minutes' head | « Copier le compte rendu » beside *Refaire…*; minutes older than a correction copied as they are — the block above says so |
| D46 | The minutes and the transcript side by side from 1000 px of pane (D61); D11's board 2, which its criterion cites, drew the page D61 replaced | The rows read in the minutes' column of D61's pane; its criterion cites D61's board for the layout, D11's for the cells |
| D47 | Settings choose the app's language (D64): « a French user » is one whose Wholeheard speaks French. The step's own English sentences still reach a French page — *Tester*'s refusal « … gives no times: choose another model » (#545), a service's refusal —: the shell passes the step's stderr as it is, and no contract gives a step's codes for its failures (`summary`'s events name a `code`, `transcription`'s do not). Cycle 04's new French sentences are not in the review's table §10 | Criterion 3 holds the shell's own texts; a step's sentence said in French needs its code first — a change of the transcription's contract, expand first, outside this deliverable: named in its sheet, not built here. Cycle 04's sentences read against the rules of criterion 2 |
| D48 | [#504](https://github.com/NapkinStack/wholeheard/issues/504) (the decider, 2026-10-28) asks for a title given by the user too. The list is now a sidebar of 280 px (D61), the page's head D61's | D48 kept, the minutes' title, shown on one line in the sidebar — cut with « … », whole in its tooltip and on the page —; the user's own title stays in *Later*, #504 linked there. Minutes made again and failing keep the earlier title, as they keep the earlier minutes (D59's departure) |
| D49, D50 | The small window is full while recording (D59's section): the meters, the context's disclosure and its field, D60's switch, all inside 380 × 400 — the context open already hides the devices' names and the switch's words (`window.css`). D50's announcement is copied in the window's language, while a meeting may say its own (D65) | No board: D49's ✓ takes each meter's device slot, its silence sentence under the meter; D49's third criterion holds the window with the context open, a silence said and D50's line together. A board only if that criterion fails. The announcement copied in the meeting's language when it says one, else the window's |
| D54 | The Windows zip waits on the colleague's trial (D53, [#482](https://github.com/NapkinStack/wholeheard/issues/482)) | A dependency: D54's fourth criterion — a call recorded from the package — needs D53's colleague and their Windows 11 |
| D55 | #494, a call cut on Bluetooth, is open in capture | capture `critical` with #494 in its runbook's known failures |
| D56 | Nothing | Unchanged |
| D57 | What leaves the machine grew: a meeting's context (D59), its language (D65), OpenRouter to transcribe (D63); a confidential meeting sends nothing (D60) | The privacy policy says each |

Also found, outside the cycle: [#494](https://github.com/NapkinStack/wholeheard/issues/494), a recording
started during a call cut it — the AirPods' Bluetooth transport failed as capture opened its streams;
the two leads its comment names (cpal's one `node.group` for both streams, the virtual Bluetooth
input's autoswitch) are not tried yet. It is a defect: a pull request labelled `out-of-cycle`, with
the line `Out of cycle: <reason>` (`framing.md` §4). The `node.group` lead is read with `pw-dump`
on virtual devices; the transport's failure itself needs the decider's AirPods and a test call.
*(2026-10-30: the autoswitch lead read and tried, the `node.group` lead not isolated: [D66](#d66) and ADR-0015 replace this paragraph's way.)*

## Open questions

**Blocking:** none — each below has a default the deliverable is built with.

**Settled at the acceptance, 2026-10-29**, by the framer's default on the decider's word (« cadre oui mais il faut commencer »), each the decider's to amend: a private audience for the first submission; three weeks; *Transcription*; Qui · Quoi · Pour quand; the review's table adopted; fifteen seconds; the title; the framer's announcement; the live check on every recording; D49 and D50 without a board (*Read again*); #504 in *Later*.

**Not blocking:**

- **The first submission before the Store's licence is read** (*Later*): a full-trust app's trial
  is not enforced by Windows, the app reads it. Submitted to a private audience — the colleagues —
  until it is, or the licence brought into this cycle? The framer's default: a private audience;
  no copy sold before the trial is read.
- **The appetite**: D51 and D52, two of the lighter, went out; D54–D57, heavier, came in and are
  not cut — two weeks may not hold them. The decider's to weigh at the acceptance: three weeks, or
  D49's live check and D47's errors cut first.

- ***Transcript* or *Transcription*** in French (D47)? Both reviews say *Transcription*: the
  French word for the text, the one every French product uses, already the step's name in
  Settings (UX review §10, market review §7). Boards 1 and 2 of D11, accepted, say *Transcript*.
  The framer's default: *Transcription*, a departure from the boards the decider settles.
- **The order of an action's cells** (D46): the market review's Qui · Quoi · Pour quand
  (recommendation 3), or board 2's and the contract's, what · who · when? The framer's
  default: Qui · Quoi · Pour quand on screen, without visible headers; the contract's order
  unchanged.
- **The wording table of the UX review §10** (D47): adopted as it is, or amended line by line?
  The framer's default: adopted, with *Transcription* below.
- **The reminder's line for fifteen seconds** (D50), as the UX review proposed (1.2), or the whole
  recording? The framer's default: fifteen seconds, the notification beside it.
- **The title or the cheap fix** (D48): the framer's proposal is the title; the cheap fix is S.
- **The announcement's sentence** (D50): the framer's, above, for the decider to adopt or amend.
- **The live check on every recording** (D49), rather than the first only.
- **D49's and D50's board** (*Read again after cycle 04*): drawn first, or built into the small
  window as it is? The framer's default, after #547's challenge: no board — D49's third criterion
  holds the full window, a board only if it fails.
- **#504, a title given by the user** (*Read again after cycle 04*): in this cycle beside D48, or in
  *Later*? The framer's default: *Later* — D48 already names every meeting that has minutes, and
  the appetite is the open question above.

## Closure

<Written at the end.>

**Documents brought current:** <at the closure>
