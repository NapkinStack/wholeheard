---
goal: "A meeting's minutes can be checked at the very second they came from, and the window feels finished: nothing fails in silence, each gesture answers at once, the microphone can be chosen at the start — with what people still have to try kept ready for them"
status: accepted          # proposed | accepted | closed | stopped — the decider accepts
appetite_weeks: 2         # the time the decider WANTS to spend, not an estimate
start: 2026-10-31         # YYYY-MM-DD
end: 2026-11-14           # start + appetite: the first day the cycle is over — the circuit breaker
deliverables:
  - id: D66
    title: "A recording holds the microphone the call holds, and never touches a Bluetooth headset's profile"
    module: capture
    state: in-progress
    acceptance:
      - "Given a call holding a microphone other than the default input, when a recording runs, then `me` records that one — its name in `session.json` and the `device` event — and, when the call changes it, follows it, the track going on; held on virtual devices by capture's e2e, a stand-in call on a source of its own"
      - "Given a source carrying `bluez5.loopback=true`, when capture records it, then its stream says its version's passive value (ADR-0015, as corrected by #574, merged after this amendment), the source never runs from it alone and no profile switch comes from it, within one known window: a default input turning into a headset during a recording stays on an active stream until a reading of the graph closes it, within a few seconds, and for as long as the graph does not read; in that window WirePlumber's decision to switch came 4 times in 10 measured, on a stand-in with no card; held by capture's e2e on a stand-in source, read in `pw-dump` — WirePlumber's decision testable on a stand-in carrying `device.id` —, and `others` is unchanged. Given no call holding a microphone, then capture records the meeting's sound and says in one sentence that the microphone waits when the default input is such a source; records the default input, as today, when it is not"
      - "Given a call that lets go of the microphone, when it leaves, then capture lets go of it too: within what WirePlumber's restore takes, it holds no HFP profile by a stream of its own and the source no longer runs — held by the same e2e, the source's state in `pw-dump`, and by the decider's trial, the card's profile"
      - "Given a transport failure during a recording — a source taken away, on the e2e's stand-in —, when it fails, then the recording continues, the track's gap is marked as `session-audio/v2` allows (`clock.jsonl`'s `silence`, `changed` with `device: null`), and `recording/v1` says it as its page says now, by `device` with `null`; a reason in the event is D73's, in *Later*; the shell's words are in their own pull request"
      - "Given the decider's AirPods and a real WhatsApp Web call, when they start and stop a recording during it, at least five times, then the call is never cut, no `Failure in Bluetooth audio transport` is logged within two seconds of capture opening or closing a stream, and `local.wav` holds their voice each time; an acceptance issue says what was seen"
  - id: D54
    title: "On Windows, Wholeheard installs from one MSIX package the Microsoft Store can take, and removing it keeps the meetings"
    module: install
    state: in-progress
    acceptance:
      - "Given the Windows workflow on main, when it runs, then it leaves beside the zip Wholeheard-<version>-x64.msix, made with the Windows SDK's MakeAppx from an AppxManifest.xml that names Wholeheard, its version in four parts the last 0, its logos, the identity the decider reserved in Partner Center — or a placeholder the run says is one —, `runFullTrust` and the microphone, and the shell started with `--window`; the four programs inside it"
      - "Given that package signed for the run with a certificate made for it, when GitHub's Windows runner installs it (`Add-AppxPackage`), then Wholeheard's process starts from it with its window; when a later version is installed over it, then a meeting folder placed under Documents\\Wholeheard and the settings are still there; and when it is removed (`Remove-AppxPackage`), then the meeting folder is still there"
      - "Given the package, when its notes for certification are read (`modules/install/windows/certification.md`), then they say why Wholeheard needs `runFullTrust` and the microphone — recording a call's two sides —, and that its settings and downloaded models live in the package's own data, which Windows removes with it, its meetings never"
      - "Given the colleague of cycle 04's D53 on their Windows 11, when they install the package signed for the run and record a real call of five minutes from it, then the cycle records whether both tracks hold the call's speech — capture working from inside a package, PDR-0004's removal condition"
  - id: D55
    title: "capture is critical before the first build published outside the project"
    module: capture
    state: in-progress
    acceptance:
      - "Given capture's MANIFEST.yaml, when it is read, then its criticality is `critical`, as PDR-0002 decided for the first build published outside the project — the Store's (PDR-0004)"
      - "Given capture's manifest, when the fitness functions run, then it declares its runbook (`docs.runbook`), and the runbook says how a recording that failed is found in the logs, what a user is told, and how a release is rolled back — on Linux, the previous package installed again (`dnf downgrade`, tried once by the framer on a Fedora made for it, its date written); in the Store, which takes no lower version, the previous code submitted again under a higher one"
      - "Given the rows `critical` makes due (`.nstack/handbook/05-workflow.md` §7) — every scenario run again, UAT, rollback verified, after a release verified, E2E on critical journeys —, when the runbook is read, then it says who does each for capture and when"
  - id: D56
    title: "The list of models read again is refused unless the decider's key signed it"
    module: shell
    state: in-progress
    acceptance:
      - "Given `registry/models.json` and its signature `models.json.minisig` on main, made with minisign by the decider's key — the public key built into the shell, the private one never in a repository —, when the list is read again on the user's gesture, then it is offered only when its signature verifies; a list unsigned, signed by another key, or changed after it was signed is refused, and the shipped list stays, said in one sentence (ADR-0010, *Rejected alternatives*)"
      - "Given a pull request that changes the list, when its checks run, then a list whose signature does not verify against the built-in key is refused; until the decider's key is given, no list is read again, and Settings and the check say it waits on that key — the colleagues' builds lose today's reading again, said in the cycle's closure"
      - "Given ADR-0010, when D56 is merged, then the record says its option 4 adopted, `minisign-verify` declared in the shell's manifest with its exit strategy, and what a signature does not stop: an older list, signed, offered again"
      - "Given the shell's tests, when they run, then a list signed by a key made for the test is offered, and the three refusals above are held, without the decider's key"
  - id: D57
    title: "The Store's listing and Wholeheard's privacy policy are written, in French and in English"
    module: null
    state: in-progress
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
  - id: D67
    title: "Nothing the user does fails in silence"
    module: shell
    state: ready
    acceptance:
      - "Given `minutes_again` or `transcript_again` on a meeting gone, `hearing_models` refused, or the sidebar's fold not kept, when each fails, then the page says it in a sentence of `words.rs`, in the user's language, the raw text in the log and in developer mode; held by the window's e2e, the command stood in to fail"
      - "Given `window.js`, when the shell's tests run, then a test refuses any `.catch` or `catch` whose handler neither says nor logs the failure of a command that can fail, whatever it returns (`() => []`, `() => null`); those that stay — `page_failed`, commands returning nothing — are named in the test, each with its reason"
      - "Given the list of meetings that cannot be read, when the full window opens, then it says so and offers to try again; given no meeting, then it says what to do first, never only a path (UX review 3.7); in either theme, at the smallest size, screenshots in the sheet"
  - id: D68
    title: "Every gesture answers at once"
    module: shell
    state: ready
    acceptance:
      - "Given *Démarrer* or *Arrêter*, its command stood in to take two seconds, when the user clicks, then the button shows its new state before the command answers, and a second click during that time starts or stops nothing; held by the window's e2e (UX review 3.3)"
      - "Given the *Refaire…* panel open, when the user chooses *Annuler* or presses Escape, then it closes, nothing changed, the focus back on *Refaire…* (UX review 4.4)"
      - "Given a setting changed, when it is written, then « Enregistré » is said in place (`role=status`) for 1.5 s, and a write refused is said as D67 says (UX review 5.2)"
  - id: D69
    title: "The window looks finished: crisp icons, checked contrast, a little motion"
    module: shell
    state: ready
    acceptance:
      - "Given `window.js` and `window.css`, when the shell's tests run, then a test refuses the glyphs used as icons (› ⤢ ✓ → ×) outside the words: each is an inline SVG, none fetched, no dependency added (UX review §8)"
      - "Given the tokens of each theme and the pairs of text and background `window.css` declares, read from the file itself, when the shell's tests run, then each reaches WCAG AA, 4.5:1, 3:1 for large text; a pair below it fails"
      - "Given the small window going between rest and recording, and a chip turning from *Compte rendu…* to *Compte rendu*, when it happens, then it moves for at most 200 ms; with `prefers-reduced-motion`, it does not move; held by the window's e2e"
      - "Given the full window, when its screenshots are taken in both themes, then the header shows one primary action and *Petite fenêtre* as an icon with its name for assistive technology, before and after shots in the sheet (UX review 3.2)"
  - id: D70
    title: "A meeting can be renamed by hand, and the name stays"
    module: shell
    state: ready
    acceptance:
      - "Given `contracts/session-audio/v2`, when its page is changed, in the contracts' own pull request, merged first, then it says an optional file holding the name the user gave, written by the shell, additive; absent reads as today"
      - "Given a meeting's page, while recording or after, when the user clicks its heading, types a name and presses Enter, then the list, the page and *Libérer de l'espace* show it at once; Escape leaves it as it was (#504)"
      - "Given a name given by hand, when the minutes are made again, then the name stays; given none, the minutes' title as D48 says; given the name emptied, the minutes' title or the date comes back"
      - "Given a name holding `<img src=x onerror=alert(1)>`, or a path, when it shows, then it is shown as text, and the meeting's folder keeps its name; held by a test (`security.md`)"
  - id: D71
    title: "After a gap in a track, a word clicked plays that word"
    module: shell
    state: ready
    acceptance:
      - "Given a made-up session whose `local.wav` has a `device: null` gap in `clock.jsonl`, zeros stopping short of the next point as `session-audio/v2` says, when `meeting::tracks` and the mix place it, then a tone at a known second on the clock is heard within 50 ms of that second, on each side of the gap; the test seen failing first, today's drift then written in the sheet (by reading today, 0.5 to 1.5 s a gap)"
      - "Given that session, when a transcript line after the gap is clicked, then the sound played is that line's within 0.2 s. If the wrong time is the transcription's, a second module, this criterion is not met: D71 stops, reports it, and it moves to a deliverable of that module"
      - "Given the tracks as recorded, when the player plays them, then their files are unchanged (ADR-0004)"
  - id: D72
    title: "*Tester* says what is wrong, not one sentence for every cause"
    module: shell
    state: ready
    acceptance:
      - "Given `--test` of the summary and of the transcription, when it fails, then it says a code its contract lists — `summarising/v1` already says `failed` with a `code` on a test, what is missing is read when built; for the transcription, a contract page first, in the contracts' own pull request, additive —, merged before the shell reads it; a code unknown is read as no code"
      - "Given *Tester*, a stand-in service for each cause, when it answers — key refused, model unknown, no segment times, quota or payment, service down, no network —, then each gets its own sentence of `words.rs` saying what to do, in the user's language; today's sentence only for no code; the raw text in the log and in developer mode"
      - "Given the e2e's check that says « and why », when it runs, then it asserts the cause's sentence for each of the above"
  - id: D74
    title: "A microphone is chosen by hand beside Auto, on Linux"
    module: capture
    state: ready
    acceptance:
      - "Given `contracts/recording/v1`, when its page is changed, in the contracts' own pull request, merged first, then the start may name a microphone, additive; absent, Auto as D66 built it — the call's microphone, else the default"
      - "Given a microphone named at the start, when capture records on virtual devices, then `me` records that one, and a call holding another does not move it; a source carrying `bluez5.loopback=true` stays passive as ADR-0015 says, the sentence saying the microphone waits; a name gone says so and falls back to Auto, the track going on; held by capture's e2e"
      - "Given the small window at rest, on Linux, when the user opens the microphone's name, then *Auto* and the system's microphones are listed — keyboard alone, either theme, inside 380×400, nothing cut, the context open —, the choice applying to the next recording and kept; on macOS and Windows no list is offered; held by the window's e2e on stand-in devices"
      - "Given a new device arriving, when it arrives, then no dialog opens"
  - id: D51
    title: "Minutes written by an online service show what the call cost, as the service reported it"
    module: summary
    state: ready
    acceptance:
      - "Given `contracts/minutes/v3`, a new version of `minutes/v2` as ADR-0005 asks, when its page is written, in the contracts' own pull request, merged first, then it says an optional `cost` — a number, at least zero, the price the provider reported for the call — with its `currency`, an ISO 4217 code, both or neither; absent when the provider reported none; the shell reads v1, v2 and v3 alike before the summary writes v3"
      - "Given a provider whose answer carries `usage.cost` — OpenRouter's, read at its documentation when it is built —, when the summary writes the minutes, then their front matter carries that cost as reported; given a provider that reports none, then no cost is written; given a `usage.cost` that is not a number at least zero, or a currency OpenRouter's documentation does not name, then none is written and one warning is logged; given a `usage.cost_details.upstream_inference_cost` neither null nor zero — the user's own key at the model's provider —, then no cost is written. No price is ever estimated, and no table of prices is kept"
      - "Given minutes that carry a cost, when their page shows, then the maker's line ends with it in the soft colour, formatted for the window's language (`Intl.NumberFormat`), in its `currency`, two significant digits: « Écrit par Claude Sonnet, via openrouter.ai · mar. 13 oct. à 12:21 · 0,012 $US »; given none, the line is as today"
      - "Given the agreement before a first sending and the small window while recording, when they show, then neither shows a price or a running count"
  - id: D75
    title: "Each decision and action opens its passage, and plays its second"
    module: summary
    state: ready
    acceptance:
      - "Before `minutes/v3` is written: given made-up transcripts of at least 30 minutes whose decisions and actions are annotated beforehand with their second, and `qwen3:8b` by Ollama — the recommended local model —, when it writes minutes three times, then the sheet records, per run, the share of times inside the annotated passage ±30 s, and whether each run finished. Read against: at least 90 % on each finished run *(framer's, no source)*. Below it, D75 stops there: no field, no link; a refuted result is a delivered one. A service's result is recorded beside it and decides nothing"
      - "Given `contracts/minutes/v3`, when its page is written, then a decision or an action may end with the time of the passage it came from, in a form the page defines, optional and additive; minutes without it read as today"
      - "Given a transcript and a stand-in provider, when the summary writes the minutes, then each time that falls inside no segment of the transcript is removed, the line kept, one warning logged; the prompt asks for times from the transcript and nothing else"
      - "Given minutes with times, when their page shows, then a line's time is a small quiet link, an action's row keeping its three cells (D46); a click brings its passage into view in the transcript and plays from that second, keyboard alone as well; D26 holds: the page does not follow what plays; held by the window's e2e"
      - "Given « Copier le compte rendu », when it copies, then the times are not in the text (D45's body, unchanged for the reader)"
  - id: D76
    title: "Spike — does Parakeet v3 hear a French meeting better than today's Whisper, on our own recordings?"
    module: transcription
    state: ready
    acceptance:
      - "Given today's default Whisper and Parakeet TDT 0.6B v3 INT8 through sherpa-onnx, in a spike outside the shipped program, the input diffed first against what the step really reads, when each hears the module's bench, the trial meeting and the decider's call kept in `temp/real-calls` — nothing of which leaves the machine or is published —, then the module's log issue records for each: word errors where a reference exists, inventions on a silent track, duplicated words, time against the recording's length, peak memory, whether its word times can write `transcript/v1`'s segments, and whether it can be told the meeting's language (D65)"
      - "Given that record, when it is read against the threshold — at least two points fewer word errors on the trial meeting *(framer's, no source)*, no more than one point worse on the bench, fewer inventions on silence, faster than the call lasted on this machine's CPU, and word times that write `transcript/v1`'s segments —, then the framer proposes an ADR to offer it in Settings beside Whisper; below it, what was missing is recorded for the next framing. A refuted result is a delivered one"
  - id: D77
    title: "The certification kit reads Windows' package before the Store does"
    module: install
    state: ready
    acceptance:
      - "Given the Windows workflow, when it runs, then `appcert.exe` tests the package and its report is kept beside it, the run naming each failed test; given a runner where the kit cannot run — it asks for an active user session —, then the run says so in one line, and the sheet records it as the answer, the gesture going to a person's Windows"
      - "Given the report, or the kit's refusal, when it is read, then `modules/install/windows/certification.md` says what stands between the package and a submission"
---

# Cycle 06 — The minutes you can check

> The framework's words: `.nstack/handbook/glossary.md`.

> **Framed on 2026-10-30**, on the decider's word that day: « en analysant le marché et les tendances,
> avance les cycle de maniere pro, ux/ui toujours moderne, highend, ludique fluide et optimale, code
> expert. ce que tu ne PEUX pas realiser en autonomie est reporté dans le cycle suivant. »
> **Accepted by the decider on 2026-10-31**, closing cycle 05 early, on their word that day in the
> session that proposed it: « 05 fermé, D44 à D50 acceptés ; 06 accepté, défauts du cadreur ». Each open question below is settled by its default, the decider
> amending any at will; the dates are that day's, the appetite two weeks. Cycle 05's thirteen
> deliverables left moved here that day. The criteria name symbols, never lines.

## Goal

The market, [read on 2026-10-30](../reviews/2026-10-30-market.md), shows leaders widening away from
the minutes — dictation, briefs, agents — and two pressures on the rest: notice, in court, and
Whisper behind in French on read speech, by its rival's figures. Wholeheard's own ground is narrower and stronger:
the audio stays on the machine, so a line of the minutes may be proved at its second: D75's first criterion finds out whether a local model can say which. If it refutes it, the goal is read as: the player exact after a gap, and the window finished. Beside it, what makes a tool
feel high-end is mostly the absence of small betrayals, and the verifiers of cycle 05 named them.

```mermaid
flowchart LR
    M["Moved: people's gestures<br/>D66 D54–D57 D38 D39 D42<br/>D53 D27 D41 D58 D63"]:::wait
    F["The finish<br/>D67 no silent failure<br/>D68 answers at once<br/>D69 looks finished<br/>D70 renamed by hand"]:::ux
    L["What verifiers named<br/>D71 the player after a gap<br/>D72 Tester's cause"]:::fix
    C["D74 the microphone,<br/>chosen beside Auto"]:::src
    Q["Trust and quality<br/>D51 what it cost<br/>D75 a line plays its second<br/>D76 spike: Parakeet"]:::qual
    S["D77 the kit reads<br/>the package"]:::pub
    M -.->|"no one waits on them"| F
    F --> L --> C --> Q --> S
    classDef wait fill:#92400e,color:#fff
    classDef ux fill:#065f46,color:#fff
    classDef fix fill:#374151,color:#fff
    classDef src fill:#1e3a8a,color:#fff
    classDef qual fill:#5b21b6,color:#fff
    classDef pub fill:#7c2d12,color:#fff
```

**Legend** — amber: moved, built, a person's gesture left, which holds nothing else up · green: the shell's
finish · grey: follow-ups a verifier named · blue: the source chooser · violet: minutes and transcript ·
red: the Store. Left to right, the order to build them in.

## Deliverables

Value over cost, from [the market's §6](../reviews/2026-10-30-market.md#6-recommendations-by-value-over-cost)
and the [UX review's §11](../reviews/2026-10-26-ux.md). **Cut first** if the breaker nears: D77, then
D72's transcription half, then D74. Never D67, D70 (the decider's own ask), D71, D75's first criterion or D76's record. **None of the new
waits on a person to be built; D77's kit may, and D74 costs `critical`'s rows if #568 is merged first.** D67–D69 touch
`window.js` and `words.rs`: one session at a time on them.

### Moved from cycle 05

Their criteria are copied as they stand; the state and the pull requests are in
[cycle 05's closure](05-best-of-its-kind-to-use.md#closure); their sections stay there and in
[cycle 04](04-wholeheard-on-macos.md). They moved on the decider's word above. **What needs a person** below gives each its gesture.

### What needs a person

Nothing else in this cycle waits on these. If D55 or D56 is done before the acceptance, its row goes.

| Deliverable | Who | The literal gesture |
|---|---|---|
| D55 | The decider | Once an agent says #568 is green at its head: open [#568](https://github.com/NapkinStack/wholeheard/pull/568) and press *Merge*: its `CODEOWNERS` line is theirs |
| D66, sheet | The decider | Say « S3 et S4 comme proposés », the wording in [#580](https://github.com/NapkinStack/wholeheard/pull/580)'s remark 1, or amend; then an agent merges it |
| D66, trial | The decider | On Fedora: `sudo dnf reinstall ../wholeheard-d66s/dist/install/wholeheard-*.x86_64.rpm` (#580's branch, built there); once #580 is merged, `nstack release` at the root of `main`, then the same from `./dist/install/`. AirPods connected, a WhatsApp Web call with someone. Five times, the call going on: *Démarrer*, say a sentence, *Arrêter*, noting both times to the second. After each *Arrêter*, a few seconds later: `pactl list cards \| grep 'Active Profile'`. After: `journalctl --user -b --since "-30 min" \| grep 'Failure in Bluetooth audio transport'`, its times against yours. `ls -t "$(xdg-user-dir DOCUMENTS)/Wholeheard" \| head -5`, and listen to `local.wav` in each. Answer on [#494](https://github.com/NapkinStack/wholeheard/issues/494): call cut or not, each profile, lines within two seconds or not, your voice heard or not |
| ADR-0015 | The decider | After that trial, say « ADR-0015 acceptée », or what falls. D74 builds on it |
| D56 | The decider | `sudo dnf install minisign`, then, at the root of a checkout of `main`: `minisign -G -p wholeheard-models.pub -s ~/.minisign/wholeheard-models.key`, a passphrase of your own, the private key never in a repository. Give an agent the file `wholeheard-models.pub`. Sign when asked: `minisign -Sm modules/shell/registry/models.json -s ~/.minisign/wholeheard-models.key`, and give an agent `models.json.minisig`: it commits it |
| D57 | The decider | Read [`docs/store/listing.md`](../../store/listing.md); say « l'annonce ne promet rien de faux », or which line does |
| D54, submission | The decider | In [Partner Center](https://partner.microsoft.com/dashboard), with a company's account: reserve the name Wholeheard, give an agent the identity it shows. Read the Store's licence terms beside GPL-3.0; answer the age questionnaire. Say which version submits (open questions) |
| D53, D54, D58 on Windows | The colleague on Windows 11 | [#482](https://github.com/NapkinStack/wholeheard/issues/482), updated first by an agent with the run's link. Unpack `wholeheard-windows-x64-msix`, then in PowerShell, as administrator: `Import-Certificate -FilePath .\Wholeheard-this-run.cer -CertStoreLocation Cert:\LocalMachine\TrustedPeople`, then `Add-AppxPackage .\Wholeheard-<version>-x64.msix` ([Microsoft](https://learn.microsoft.com/en-us/windows/msix/package/create-certificate-package-signing): the store; remove the certificate afterwards). The first run's assistant opens (D58). A real call of five minutes; answer there |
| D38, D39, D42, D58 on a Mac | The colleague on a Mac | [#426](https://github.com/NapkinStack/wholeheard/issues/426), updated first by an agent: open the app, the first run's assistant, ⌘B on the sidebar (D61), record a real call, answer there |
| D27 | The colleague on Ubuntu 24.04, whom the charter's three do not name: the decider says who | [#255](https://github.com/NapkinStack/wholeheard/issues/255): `sudo apt install ./wholeheard_<version>_amd64.deb`, answer there |
| D41 | The decider | [#449](https://github.com/NapkinStack/wholeheard/issues/449): a call on the earbuds, the default on the speaker |
| D63 | The decider | [#463](https://github.com/NapkinStack/wholeheard/issues/463): a real service heard, with their key |

### D67

- A verifier found it wider than D47 had named (#577): `minutes_again`, `transcript_again`,
  `hearing_models`, `settings_saved` for the fold.
- The commands that return nothing cannot fail to the page: the test names them, rather than a rewrite.

### D68

- Prior art: optimistic state on the gesture, the state corrected if the command refuses. `ux.md`'s
  states are read with it.

### D69

- Lucide's five icons (ISC, its notice kept in the repository), inline: the UX review's choice. No board: an amendment of D34's header
  only for *Petite fenêtre*, the UX review's 3.2, named here as a departure for the decider.
- The direction is [Linear's](https://linear.app/now/behind-the-latest-design-refresh): "Don't compete for
  attention you haven't earned". What the review proposed, **nothing more**: not a redesign. Playful is one
  pop and one fade, as it says.

### D70

- The decider's [#504](https://github.com/NapkinStack/wholeheard/issues/504). Kept the way a meeting's
  language is kept (D65): a file of the meeting, as #504 proposes, a `session-audio` expand. A folder is never renamed: others read it.

### D71

- Named by #579 and #580, by reading, never measured: the first thing the oracle does is measure it.
- `session-audio/v2` already says a reader places frames by clock points. No contract change.
- Whether the transcription has the same drift is not known *(assumption: its `tracks.rs` reads the
  points)*: the second criterion finds out. If it is the transcription's, it stops here and is reported: a second module.

### D72

- #577's verifier: « vérifiez son adresse, son nom et sa clé » is said to a service with no times, a
  service down, a quota. `summarising/v1` lists codes, a test's included *(assumption: read, not run)*;
  the shell does not say them, and the transcription's step has no page for them. Expand first, one pull request a module:
  contracts, each step, the shell (`.nstack/handbook/03-contracts.md`).
- Never a parse of the program's English.

### D74

- **Prior Art Gate.** The convention is [Granola's](https://docs.granola.ai/help-center/troubleshooting/transcription-issues):
  *Auto*, then a list where the transcript is. It covers the need: adopted. No device management of ours.
- At the start only: capture's input takes `stop` alone, and a change during a recording would be a new command: *Later*. Built on ADR-0015, whose only proof on a real headset is D66's trial: that gesture first if it comes in time, no formal block.
- Linux only: on macOS and Windows capture has its other adapters, and D66 left them. The list's place
  departs from D34's small window; D49's third criterion is the bar: nothing overflows.
- ADR-0015's rejections stand: a card's profile is never forced, even by hand.

### D51

- **The decider chose it on 2026-10-26**; it went to *Later* on 2026-10-27 to make room for the Store.
  Its criteria are those of [#461](https://github.com/NapkinStack/wholeheard/pull/461), brought to
  `minutes/v3`, ADR-0005 keeping each version frozen. Its section at that commit still holds. See [#503](https://github.com/NapkinStack/wholeheard/issues/503).
- It and D75 share `minutes/v3`: one contract pull request after D75's first criterion is read, merged before either; refuted, v3 holds the cost alone.

### D75

- **Prior Art Gate.** [Notion](https://www.notion.com/help/ai-meeting-notes) cites transcript lines; Granola
  points to the transcript. Those two stop at text; Otter, Fathom and tl;dv jump to a recording's moment from a transcript or a highlight, kept in their cloud. The convention is adopted, its form extended by what
  ADR-0004 already keeps, with nothing to build but a link to a player that exists. The deviation is
  paid by a named value: checking a line in one click, by ear.
- **What is not known** *(assumption)*: whether a small local model gives a time that is right. Hence the
  first criterion, read before anything is frozen (ADR-0005). `qwen3:8b` already ran past 280 s on a short transcript (#571): a run that does not finish is recorded. Below the threshold it is shown
  for none, rather than a link that lies.
- No confidence score, no checking of a claim. The review of 2026-10-26 refused them.
- A line corrected (D62) leaves earlier minutes' times as they were: they point to a time, never to a line.

### D76

- **Prior Art Gate.** [NVIDIA's own figures](https://arxiv.org/html/2509.14128v1) put Parakeet ahead of
  Whisper large-v3 on French read speech; [Meetily](https://github.com/Zackriya-Solutions/meeting-minutes/releases)
  ships it. An engine that can be swapped is already what the step holds (ADR-0003). No tuning: an
  engine's defaults are taken as they are (the decider, 2026-10-17). Whether it stays is an ADR, with its exit.
- `modules/transcription/AGENTS.md`: a bench of read sentences cannot decide; the trial meeting is heard too.
- No sound of the decider's call is sent or published. The record gives counts, a few words at most.
- Nothing enters the shipped program here: a dependency, if ever, is that ADR's (`security.md`).

### D77

- [Microsoft](https://learn.microsoft.com/en-us/windows/uwp/debug-test-perf/windows-app-certification-kit)
  ships the kit in the Windows SDK, with a command line (administrator, `appcert.exe reset` first). The page says both "an active user session" and integration where none is available: whether a
  hosted runner suits is unknown, and a refusal recorded answers the deliverable. Named by D54's verifier.

## Dependency with a date

From the day cycle 06 is accepted, as cycle 05's intervals; a date missed leaves a deliverable where it is at the end date.

- **An agent remakes the forge's builds from `main` and updates #426, #482, #255, #449 and #463 within 48 hours of the acceptance**, their dates with them (#426 and #482 say « by 2026-10-31 », their artifacts expire on 2026-11-05), and again whenever an artifact is about to expire.
- **The colleagues' trials**, and the name of D27's: 14 days after the acceptance.
- **The decider's Partner Center account and the name reserved; the minisign key**: 11 days after.
- **The decider's reading of the Store's terms beside GPL-3.0; the age rating**: 15 days after, or the end date if sooner.
- **D66's trial and ADR-0015**: before D74 is built, if it can be; at the latest the date the decider sets at the closure.

## Out of scope

As cycle 05, the market's refusals beside it (§4):

- Dictation; automatic speaker identification, voiceprints.
- An assistant during the meeting; live minutes; chat over notes across meetings; MCP.
- Team spaces, seats; a CRM, a Slack bot, an integration posting for the user.
- Public links to a meeting or its minutes — never.
- Scoring the minutes' truth; a meter, an estimate or a table of prices.
- Submitting to the Store: it waits on what *What needs a person* names.

## Later

- **Spoken notice**, as [Notion](https://www.notion.com/help/ai-meeting-notes)'s voice message: **against the charter's constraint, a PDR first, the decider's.**
- **D73, a microphone that stopped says why**: `recording/v1`'s optional reason, in no ranking of the market, and capture cannot always tell (#574, #580). **A microphone changed during a recording**, after D74.
- A trial and a licence key, a merchant of record; the Mac App Store after a spike; SpeechAnalyzer's live results (cycle 05's *Later*).
- **D52, the spike on a live transcript**, and what depends on it (cycle 05's *Later*).
- Naming the other side by hand, once, everywhere; meeting types as contexts; export to a file or Notion (market review of 2026-10-26, 7, 8, 9).
- Search highlighted, with a count; playback at 1×, 1,5×, 2× (UX review 4.6, 4.7).
- From the verifiers: a side between -60 and -40 dB says nothing, a second recording's stale first check (#575); a stereo track holding a headset's voice on one channel, at half its level; `Micro changé`, then a gap with no notification, the last word turning false (#580, remark 2); a `# ...` title refusing the answer, `Accord Q3/Q4` leaving the date (#571); the `allow_legacy` and limits mutants (#578).
- From cycle 05's *Later*: the Store's trial; macOS notarised; settings kept when the package is removed; start with Windows; a notification's click on a Mac; the cost of an online transcription; a link to its passage, now D75; a discreet indication of a new device; the tray's repair entries.
- Re-read [RM566322](https://mc.merill.net/message/RM566322) and Apple's EU position before the Store's launch.

## Open questions

**Blocking:** none — each has a default the deliverable is built with.

**Not blocking**, each *(framer, to confirm)*:

- **Closing cycle 05 now, or at 2026-11-19?** Settled: now, D44–D50 accepted on their sheets, the decider's word above.
- **The appetite**: two weeks, modest, thirteen moved and eleven new, the cuts named. Default: two; never stretched.
- **D66's fourth criterion** still names « an optional field », which #579 removed from D66 (*Later* keeps it as D73). Settled by the default: amended at the acceptance to read as `recording/v1` says now. **Its second criterion's window**: a default turning into a headset mid-recording, the switch came 4 of 10 measured. Default: taken knowingly, as already written.
- **The version submitted**: `0.2.0` has a first part the Store refuses. Default: 1.0.0 for the first submission, not before it — a version says what was tried.
- **The reminder's setting** (charter, 2026-10-13: a setting removes it): read again against [*Chamberlain v. Granola*](../reviews/2026-10-30-market.md#3-consent-and-law)? Default: unchanged, a lawyer's reading before strangers install it; changing it takes a PDR.
- **`minutes/v3`, one version for D51 and D75.** Default: yes, a page per version being ADR-0005's price.
- **D75's threshold, 90 %**: the framer's, with no source: a link that lies is what the feature cannot afford. Default: kept; amend at will.
- **D76's threshold**: the framer's, with no source, default as written. It compares only with today's model, `ggml-large-v3-turbo-q8_0`.
- **D27**: its colleague on Ubuntu is not one of the charter's three, and it moves for the fourth time. Default: no name by the date below, it goes to *Later*.
- **D74's list where**: on the microphone's line of the small window. Default: there; Settings none, for the choice is a meeting's.

## Closure

<Written at the end.>

**Documents brought current:** <at the closure>
