---
goal: "Day to day, Wholeheard is the best of its kind to use: a meeting ends in one notification that opens it, named, its actions read at a glance, its minutes copied in one click, said in a French that sounds native — and the first run holds the user's hand, the announcement one click away"
status: proposed          # proposed | accepted | closed | stopped — the decider accepts
appetite_weeks: 2         # (framer, to confirm) — the time the decider WANTS to spend, not an estimate
start: 2026-11-02         # YYYY-MM-DD
end: 2026-11-16           # start + appetite: the first day the cycle is over — the circuit breaker
deliverables:
  - id: D44
    title: "A meeting's notifications are fewer, and a click on one opens the meeting in Wholeheard"
    module: shell
    state: proposed
    acceptance:
      - "Given a recording stopped while a Wholeheard window is visible, when it stops, then no notification says the stop; with no Wholeheard window visible, the stop is said as today"
      - "Given Settings set to make the minutes after the stop (`after = \"minutes\"`), when the transcript is written and the minutes follow, then no notification says the transcript, and one says the minutes; a failure of either step is always said"
      - "Given a notification that a meeting's transcript or minutes are ready, on Linux, when the user clicks it, then the full window opens on that meeting's page (`#/meeting/<name>`) — never the file manager — and its words say so: « Cliquez pour l'ouvrir. » / \"Click to open it.\""
      - "Given a meeting made with `after = \"minutes\"` and no window visible at the stop, when it is made, then it gave at most three notifications: the reminder at the start, the stop, the minutes ready"
  - id: D45
    title: "A meeting's minutes are copied in one click, ready to paste where the user writes"
    module: shell
    state: proposed
    acceptance:
      - "Given a meeting with minutes, when the user clicks « Copier le compte rendu » / \"Copy the minutes\", beside *Refaire…*, then the clipboard holds the minutes' body as Markdown text, without its front matter, and « Copié » / \"Copied\" is said in place (`role=status`) for a moment"
      - "Given a meeting without minutes, or minutes that do not read, when its page shows, then no copy is offered"
      - "Given the clipboard refused by the web view, when the user clicks, then one sentence says the minutes could not be copied, and nothing else changes"
  - id: D46
    title: "A meeting's actions are rows: Qui · Quoi · Pour quand"
    module: shell
    state: proposed
    acceptance:
      - "Given minutes whose section 2 holds items written as `minutes/v1` says — what — who — by when, separated by ` — ` —, when the meeting's page shows them, then each is one row of three labelled cells, who in its colour and by when in monospace, as board 2 of D11 drew them, in the order the decider settles (open question)"
      - "Given an item that does not split into exactly three parts on ` — `, when it shows, then it is shown as written, never cut or guessed"
      - "Given « non précisé » / \"not specified\" in a cell, when it shows, then it is shown in the soft colour, as what the meeting did not settle"
  - id: D47
    title: "Wholeheard speaks a native, short French — and never shows English to a French user"
    module: shell
    state: proposed
    acceptance:
      - "Given the wording table of the review of 2026-10-26 (`docs/project/reviews/2026-10-26-ux.md` §10), as the decider amends it at the cycle's acceptance, when Wholeheard speaks French, then each of its sentences is the one adopted, and its English counterpart said with the same care"
      - "Given every French sentence the window and the notifications say, when the shell's tests run, then a test refuses one with a space other than U+202F before `? ! ;` or other than U+00A0 before `:`"
      - "Given a command the page calls that fails — a settings file, a path, a key, the system —, when it fails, then the page says it in a sentence of `words.rs`, in the user's language; the raw text goes to the log and, in developer mode, under the sentence; a test refuses a command that hands the page a text of its own (`Err(\"…\")`)"
  - id: D48
    title: "A meeting has a name: the one its minutes give it"
    module: summary
    state: proposed
    acceptance:
      - "Given `contracts/minutes/v1`, when its page is changed, then it says an optional `title` in the front matter — one line, a few words in the meeting's language, at most 80 characters, never a key or a path — in a pull request of the contracts module alone, merged before the summary writes it; the shell, which reads the front matter into a table of keys (`minutes.rs:49-53`), reads minutes with or without it unchanged"
      - "Given a transcript, when the summary writes its minutes, then their front matter carries a `title` that names what the meeting was about; given a model that gives none, or one longer than 80 characters or on several lines, then the minutes are written whole without it, never refused for it"
      - "Given the list of meetings and a meeting's page, when a meeting's minutes carry a title, then the row and the page's heading show the title, the date beneath it; without minutes, or minutes made before, the date as today (`window.js:72-78, 321, 865`)"
      - "Given the minutes made again, when they are written, then the meeting takes the new title"
  - id: D49
    title: "The first run holds the user's hand: the model offered before the first meeting, each side heard checked live"
    module: shell
    state: proposed
    acceptance:
      - "Given the small window at rest, the chosen Whisper model not on the machine and no meeting yet, when it shows, then board 5's card stands in place of « Aucune réunion pour l'instant. »: the model's size and host, that it is checked before it is used, « Télécharger » / \"Download\" and « Plus tard » / \"Later\"; Télécharger fetches it as Settings do (`model_fetch`); Plus tard hides the card until Wholeheard starts again"
      - "Given a recording started, when it records, then the small window shows, for each side — « Votre micro » / \"Your microphone\", « Le son de la réunion » / \"The meeting's sound\" —, a check that turns ✓ as soon as its levels rise above silence (`level`, `recording/v1`), then fades"
      - "Given a side that stays silent, when capture says it silent (`Said::Silent`, `words.rs:159-171`), then the small window says under its check what to do — speak, or play a sound in the call — as well as the notification it says today"
  - id: D50
    title: "The reminder to announce the recording is kept, shown where the user looks, the announcement one click away"
    module: shell
    state: proposed
    acceptance:
      - "Given a settings file that does not say `reminder`, or none, when Wholeheard starts, then the reminder is on (`settings.rs:106`), held by a test; Settings still let the user turn it off (charter, 2026-10-13)"
      - "Given a recording with the reminder on, when it records, then the small window says « Prévenez les participants que cette réunion est enregistrée. » / its English, under the timer, in the soft colour — as well as the notification at the start, so that it shows with notifications off or held back"
      - "Given that line, when the user clicks « Copier l'annonce » / \"Copy the announcement\", then the clipboard holds a sentence ready for the call's chat, in the language of Wholeheard's window, and « Copié » is said in place; the sentence is the decider's, adopted at the cycle's acceptance (D50's section)"
      - "Given the reminder off, when a recording starts, then neither the line nor the button shows"
  - id: D51
    title: "Minutes written by an online service show what the call cost, as the service reported it"
    module: summary
    state: proposed
    acceptance:
      - "Given `contracts/minutes/v1`, when its page is changed — in D48's pull request of the contracts module, merged first —, then it says an optional `cost`: a number, at least zero, the price the provider reported for the call, in the unit it names; absent when the provider reported none"
      - "Given a provider whose answer carries `usage.cost` — OpenRouter's, read at its documentation when it is built —, when the summary writes the minutes, then their front matter carries that cost as reported; given a provider that reports none — Ollama on this computer, another service —, then no cost is written; given a `usage.cost` that is not a number at least zero, then none is written and one warning is logged. No price is ever estimated, and no table of prices is kept"
      - "Given minutes that carry a cost, when their page shows, then the maker's line ends with it in the soft colour, formatted for the window's language (`Intl.NumberFormat`), in the unit the contract names: « Écrit par Claude Sonnet, via openrouter.ai · mar. 13 oct. à 12:21 · 0,012 $ »; given none, the line is as today (`window.js:571`)"
      - "Given the agreement before a first sending and the small window while recording, when they show, then neither shows a price or a running count"
      - "Given the summary's tests, when they run, then a stand-in provider's answer with `usage.cost`, one without and one with a cost that does not read are each held by a test; one real call to openrouter.ai is a scenario of the sheet"
  - id: D52
    title: "Spike — can a transcript followed live, on the machine alone, keep up during a call and be worth reading?"
    module: platform
    state: proposed
    acceptance:
      - "Given a measuring program of the bench — a session's two tracks heard as they would arrive, in spans of 5 s and of 10 s, by `ggml-small-q8_0` (the list's \"faster\" model) —, when it runs on the decider's Fedora while capture records a stand-in call on devices made for the run, then the cycle records, for each span: the time to hear a span against its length, the processor and memory it took, the delay from a word said to the word shown, and capture's tracks against a recording of the same call without it — gaps of zeros, blocks lost"
      - "Given SUMM-RE's passages of the bench (`bench.real`), when they are heard in those spans and scored, then the cycle records the share of words missed or misheard by the preview, side by side with the transcript of record's — the recommended model, whole — on the same passages"
      - "Given that record, when it is read against the threshold *(framer, to confirm)* — each span heard in at most half its length, capture's tracks no worse than without it, and the preview's words missed or misheard at most 15 points above the transcript of record's —, then the next framing may build the preview, its ADR first; below it, what fell short is recorded. A refuted result is a delivered one"
---

# Cycle 05 — The best day to day

> The framework's words: `.nstack/handbook/glossary.md`.

> Framed on 2026-10-26 on the decider's word: their choice that day, « Cycle 05 + bug tout de
> suite » — a cycle 05 after cycle 04, « le meilleur au quotidien ». It starts the day after
> cycle 04's end, and is accepted only once cycle 04 is closed: one accepted cycle at a time.
> Its source is the review of 2026-10-26 the decider asked for — [the market](../reviews/2026-10-26-market.md)
> and [the surface](../reviews/2026-10-26-ux.md), each by an independent session — as cycle 04's
> *Later* says. Every file:line below is the review's, read at commit 66b1ff7: `main.rs` has
> moved by some fifty lines since, `window.js` by some twenty after line 1600, the content
> unchanged (the verifier of #445); `minutes.rs`, `settings.rs` and `provider.rs` were read at
> 66b1ff7 by the framer.

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
jamais éteint par défaut ». Their standing asks hold for each: modern, high-end, playful, the
easiest, non-intrusive, fluid; no over-engineering; words chosen with care.

```mermaid
flowchart LR
    S["Start<br/>reminder · Copier l'annonce"]:::d50 --> C["Each side heard<br/>✓ live"]:::d49
    C --> T["Stop"]:::rec
    T --> N["One notification<br/>opens the meeting"]:::d44
    N --> M["The meeting, named<br/>actions as rows · cost"]:::d48
    M --> P["Copied<br/>to mail, chat, Notion"]:::d45
    W["Wording, French first"]:::d47 -.-> S & N & M
    L["Spike: a transcript<br/>followed live"]:::spike -. "measured, not built" .-> T
    classDef d50 fill:#92400e,color:#fff
    classDef d49 fill:#065f46,color:#fff
    classDef rec fill:#374151,color:#fff
    classDef d44 fill:#1e3a8a,color:#fff
    classDef d48 fill:#5b21b6,color:#fff
    classDef d45 fill:#0f766e,color:#fff
    classDef d47 fill:#4b5563,color:#fff
    classDef spike fill:#7c2d12,color:#fff
```

**Legend** — one meeting, left to right · amber: D50 · green: D49 · grey: what is there today ·
blue: D44 · violet: D48, with D46 and D51 on the same page · teal: D45 · slate: D47, said
everywhere · red: D52, a measure only.

## Deliverables

Each is the shell's but D48 and D51, whose minutes are written by the summary, and D52, a measure
of the bench. Ordered by the review's value over cost (UX review §11): the order to build them in.

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
  *(assumption)*; tried during the build, kept if it holds.
- Export, Notion, a file: *Later*, as the charter has it.

### D46

- The contract already guarantees the separator: `contracts/minutes/v1`, Actions, "what, who and
  by when, in that order, separated by ` — `". Rendered in `minutes.rs` of the shell; the contract
  unchanged (UX review 4.2).
- Board 2 of D11 drew what · who · when; the decider's words say Qui · Quoi · Pour quand: see
  *Open questions*.
- The copy of D45 keeps the items as written.

### D47

- The table of the UX review §10 is a proposal, not decided words: the decider adopts or amends
  it when they accept the cycle; the market review's §7 is read with it (Compte rendu, Décisions,
  Actions, Points clés, Intervenants).
- The errors handed to the page as English text (`main.rs:414-415, 508, 916-946, 1100-1134`) go
  through an enum said by `words.rs`, as `summarising::Cause` already is (UX review 5.1).
- *Transcript* or *Transcription* in French: see *Open questions*.

### D48

**The title written by the minutes, rather than the review's cheap fix** *(framer, to confirm)*.
The UX review weighed two forms (3.1):

| | The cheap fix | The title of the minutes |
|---|---|---|
| What | The shell shows the first sentence of *Points saillants*, cut at about 90 characters | The summary writes an optional `title` in the front matter, an additive change of `minutes/v1` |
| Modules | shell | contracts, then summary, then shell — one pull request each |
| Gives | A sentence cut short: « La date de l'événement est fixée au jeudi 3 juin, avec le 10 en… » | A name, as every board drew it: « Lancement — calendrier, budget, recrutement » |
| Cost | S | M |

The framer proposes the title: the boards, accepted as criteria, drew names; the contract's pull
request is one D51 needs anyway, so both fields go in the same one; and a name is what a later
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
  (UX review 2.2).

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

### D51

- Today nothing records a cost: `Provider::ask` hands back the answer's text alone
  (`modules/summary/src/provider.rs:165` at 66b1ff7) and the front matter holds `made`,
  `provider`, `model`, `context` (UX review §9).
- OpenRouter returns `usage.cost` in every response, with no parameter — `usage: {include: true}`
  is deprecated ([OpenRouter, *Usage accounting*](https://openrouter.ai/docs/use-cases/usage-accounting),
  read by the market review on 2026-10-26). Its page says "cost in credits" without naming a
  currency: that it is US dollars is an **assumption**, read again at the source when it is built
  (*Open questions*).
- Three pull requests, one module each, in this order: contracts (the page, with D48's `title`),
  summary (reads the cost, writes it), shell (shows it). The deliverable is the summary's, as D42's
  was transcription's.
- The summary reads more of an answer it already receives: no new connection, no new host, the
  key where it is. `security.md` applies to that answer as external input — criterion 2's
  refusal of a cost that does not read.
- No estimate before the call, no running meter, no monthly total: the decider's "never an estimate", and the UX review's §9.3 total is not asked for.

### D52

- The decider's idea of 2026-10-26 (« la possibilité de faire la transcription live, le
  traitement qu'on ne peut pas faire live est celui des minutes »), framed in cycle 04's *Later*:
  a provisional preview while recording, on the machine alone, in the small window and the full
  one; the transcript of record made after the stop as today. Short spans lose words — cycle 01,
  D4: 34.5 % of the words differed heard thirty seconds at a time, 26.7 % whole — so the preview
  never replaces the transcript.
- A measure, not the product: the program lives in the bench (`modules/platform/bench`), nothing
  in the shell, capture or transcription changes. Building the preview is the next framing's, its
  ADR first, if the threshold is met.
- On the decider's Fedora, on devices made for the run while the decider is not on a call; SUMM-RE's
  passages (CC BY-SA 4.0) for the words. Nothing of a real meeting is recorded or published.
- SpeechAnalyzer's own live results on a Mac are not measured here: GitHub's runner offers no
  SpeechTranscriber (cycle 04, D42), and the colleague's hour is cycle 04's.

## Dependency with a date

- **One real call to openrouter.ai**, for D51's sheet: a key of OpenRouter's, with credit, by
  2026-11-11 *(framer, to confirm whose)*. Missed, D51's other scenarios hold and that one waits.

## Out of scope

The review's "do not build" (market review §4, §8):

- Automatic speaker identification (diarization).
- An assistant during the meeting, live minutes.
- Chat over notes, across meetings.
- Team spaces, seats.
- A CRM, a Slack bot, any integration posting for the user.
- Public links to a meeting or its minutes — never.

And for this cycle:

- The live preview built into the product: D52 measures it only.
- A notification's click on macOS and Windows.
- A price estimated before a call, a running meter, a monthly total, a table of prices.
- Export to a file or to Notion; rich text promised in the copy.

## Later

- The live preview built, on D52's record and its ADR; SpeechAnalyzer's live results on a Mac.
- Each decision and action linked to its passage, the audio played there (market review,
  recommendation 2): what Granola, which deletes the audio, cannot do.
- Naming the other side by hand, once, everywhere; meeting types as contexts (market review 7, 8).
- A notification's click on a Mac.
- A meeting renamed by hand.
- The rest of the UX review's ranked list: the full window's empty state and a caught `listed`
  failure (3.7), *Démarrer*'s instant feedback (3.3), *Refaire…*'s *Annuler* (4.4), playback
  speed (4.7), search highlighted (4.6), the tray's repair entries (6.3, departs from D34's board).

## Open questions

**Blocking:** none — each below has a default the deliverable is built with.

**Not blocking:**

- **Tokens beside the cost** (D51)? The reviews disagree: the market review shows them (« 12 480
  jetons », §6), the UX review never — jargon, the log and developer mode at most (§9). The
  framer's default: not shown, written to the log; adding them later is another additive field.
- ***Transcript* or *Transcription*** in French (D47)? Both reviews say *Transcription*: the
  French word for the text, the one every French product uses, already the step's name in
  Settings (UX review §10, market review §7). Boards 1 and 2 of D11, accepted, say *Transcript*.
  The framer's default: *Transcription*, a departure from the boards the decider settles.
- **The order of an action's cells** (D46): the decider's words, Qui · Quoi · Pour quand, or
  board 2's and the contract's, what · who · when? The framer's default: the decider's words, on
  screen; the contract's order unchanged.
- **The unit of `usage.cost`** (D51): OpenRouter's page names credits, not a currency. The
  framer's default: `$` only if OpenRouter's documentation says the credits are dollars when D51 is
  built; otherwise « crédits OpenRouter ».
- **The title or the cheap fix** (D48): the framer's proposal is the title; the cheap fix is S.
- **The announcement's sentence** (D50): the framer's, above, for the decider to adopt or amend.
- **The live check on every recording** (D49), rather than the first only.

## Closure

<Written at the end.>

**Documents brought current:** <at the closure>
