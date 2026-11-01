// The window: what the shell's `listed` command returns, shown as D11's first board. It
// reads nothing itself and changes nothing; it asks again whenever the window comes forward.
"use strict";

// A failure of the page's own reaches the shell's log (D29): its kind and where it was thrown —
// never its sentence, which may hold a meeting's words. A rejection says where from its stack.
function pageFailed(error, file, line, column) {
  const kind = error instanceof Error ? error.name : "";
  window.__TAURI__.core.invoke("page_failed", { kind, file: file || "", line: line || 0, column: column || 0 }).catch(() => {});
}
window.addEventListener("error", (event) => pageFailed(event.error, event.filename, event.lineno, event.colno));
window.addEventListener("unhandledrejection", (event) => {
  const at = /([^\s@()]*):(\d+):(\d+)/.exec(event.reason instanceof Error ? event.reason.stack || "" : "");
  pageFailed(event.reason, at?.[1], Number(at?.[2]), Number(at?.[3]));
});

const fill = (template, ...values) => values.reduce((text, value) => text.replace("{}", value), template);

function size(bytes, words) {
  let unit = 0;
  let value = bytes;
  // The unit is chosen on the value as it will be written: 999 999 B is "1 MB", not "1,000 kB".
  const rounded = (v, u) => { const d = v < 10 && u > 0 ? 10 : 1; return Math.round(v * d) / d; };
  while (rounded(value, unit) >= 1000 && unit < 3) { value /= 1000; unit += 1; }
  const digits = value < 10 && unit > 0 ? 1 : 0;
  const number = new Intl.NumberFormat(words.locale, { maximumFractionDigits: digits }).format(value);
  return `${number} ${words.units[unit]}`;
}

// The day a session started, on this machine's clock; none when the shell could not read it.
const dayOf = (session) => (session.instant === null ? null : new Date(session.instant * 1000));

// As the notifications say it (words::length): seconds under a minute, then minutes, then hours.
function length(seconds) {
  if (seconds < 59.5) return `${Math.round(Math.max(0, seconds))} s`;
  const minutes = Math.round(seconds / 60);
  if (minutes < 60) return `${minutes} min`;
  return `${Math.floor(minutes / 60)} h ${String(minutes % 60).padStart(2, "0")}`;
}

// Monday 00:00 of the week a day is in, on this machine's clock.
function weekOf(day) {
  const start = new Date(day.getFullYear(), day.getMonth(), day.getDate());
  start.setDate(start.getDate() - ((start.getDay() + 6) % 7));
  return start;
}

function group(day, now, words) {
  if (day === null) return words.undated;
  const thisWeek = weekOf(now);
  const lastWeek = new Date(thisWeek);
  lastWeek.setDate(lastWeek.getDate() - 7);
  if (day >= thisWeek) return words.this_week;
  if (day >= lastWeek) return words.last_week;
  const month = new Intl.DateTimeFormat(words.locale, { month: "long" }).format(day);
  if (day.getFullYear() === now.getFullYear() && day.getMonth() === now.getMonth()) return fill(words.earlier_in, month);
  const named = new Intl.DateTimeFormat(words.locale, { month: "long", year: "numeric" }).format(day);
  return named.charAt(0).toUpperCase() + named.slice(1);
}

function chip(text, kind) {
  const element = document.createElement("span");
  element.className = kind ? `chip ${kind}` : "chip";
  element.textContent = text;
  return element;
}

// A lock, drawn: its meaning is always in the words beside it (D60).
function lock() {
  const ns = "http://www.w3.org/2000/svg";
  const svg = document.createElementNS(ns, "svg");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("class", "lock");
  svg.setAttribute("aria-hidden", "true");
  const body = document.createElementNS(ns, "rect");
  for (const [key, value] of Object.entries({ x: 5, y: 11, width: 14, height: 10, rx: 2, fill: "currentColor" })) body.setAttribute(key, value);
  const shackle = document.createElementNS(ns, "path");
  for (const [key, value] of Object.entries({ d: "M8 11 V8 a4 4 0 0 1 8 0 V11", fill: "none", stroke: "currentColor", "stroke-width": 2.2 })) shackle.setAttribute(key, value);
  svg.append(body, shackle);
  return svg;
}

// Lucide's icons, inline (ISC, `ui/LUCIDE-LICENSE`): what the page draws for itself, a glyph being the
// font's to draw (D69). The name goes on what holds it, for assistive technology.
const ICONS = {
  check: ["M20 6 9 17l-5-5"],
  "chevron-left": ["m15 18-6-6 6-6"],
  x: ["M18 6 6 18", "m6 6 12 12"],
  "arrow-right": ["M5 12h14", "m12 5 7 7-7 7"],
  "maximize-2": ["M15 3h6v6", "m21 3-7 7", "m3 21 7-7", "M9 21H3v-6"],
  "picture-in-picture-2": ["M21 9V6a2 2 0 0 0-2-2H4a2 2 0 0 0-2 2v10c0 1.1.9 2 2 2h4", "M14 13h6a2 2 0 0 1 2 2v3a2 2 0 0 1-2 2h-6a2 2 0 0 1-2-2v-3a2 2 0 0 1 2-2"],
};
function icon(name) {
  const ns = "http://www.w3.org/2000/svg";
  const svg = document.createElementNS(ns, "svg");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("class", "glyph");
  svg.setAttribute("aria-hidden", "true");
  for (const d of ICONS[name]) {
    const path = document.createElementNS(ns, "path");
    path.setAttribute("d", d);
    svg.append(path);
  }
  return svg;
}

function readySaid(text) {
  const said = element("span", "ready");
  said.append(icon("check"), ` ${text}`);
  return said;
}

function lockedChip(words) {
  const said = chip(words.confidential_chip, "locked");
  said.prepend(lock());
  return said;
}

// « Réunion confidentielle » (D60): the settings' own switch, a lock before its words.
function confidentialSwitch(words, id) {
  const row = element("label", "row confidential");
  const box = element("input");
  box.type = "checkbox";
  box.id = id;
  box.setAttribute("role", "switch");
  const said = element("span", "said");
  said.append(element("span", "name", words.confidential_name), element("span", "says", words.confidential_says));
  row.append(lock(), said, box);
  return { row, box };
}

// What had left before the meeting was said confidential, and to which host.
function leftSaid(words, left) {
  const each = left.map(({ what, host }) => fill(what === "sound" ? words.confidential_left_sound : words.confidential_left_transcript, host));
  return each.length === 0 ? "" : fill(words.confidential_left, each.join(" · "));
}

// The language a meeting is spoken in (D65): the settings' own, or one of its own — its next
// transcript heard in it. Written at once; refused, said, the choice put back.
function languageRow(session, language, languages, words, settled, apple, system) {
  const row = element("div", "row language");
  const label = element("label", "name", words.language_label);
  label.htmlFor = "meeting-language";
  const pick = element("select");
  pick.id = "meeting-language";
  const add = (value, text) => {
    const option = element("option", "", text);
    option.value = value;
    pick.append(option);
  };
  // What « as in Settings » means, said: the language they name, or detected — by SpeechAnalyzer
  // never: it hears French or English, the system's when none is named (D64).
  const heard = apple ? languages.filter(([code]) => code === "fr" || code === "en") : languages;
  const named = (code) => languages.find(([known]) => known === code)?.[1] ?? code;
  const settledSaid = apple ? named(heard.some(([code]) => code === settled) ? settled : system) : settled ? named(settled) : words.language_detected_short;
  add("", `${words.language_default} — ${settledSaid}`);
  for (const [code, said] of heard) add(code, said);
  // A language of its own SpeechAnalyzer does not hear — written while Whisper heard —: kept, said as heard.
  if (language && !heard.some(([code]) => code === language)) add(language, apple ? fill(words.language_heard_as, named(language), named(system)) : named(language));
  pick.value = language ?? "";
  const said = element("p", "says");
  said.id = "language-said";
  said.setAttribute("role", "status");
  let kept = pick.value;
  pick.addEventListener("change", async () => {
    try {
      await window.__TAURI__.core.invoke("language_set", { name: nameOf(session), language: pick.value || null });
      kept = pick.value;
      said.textContent = "";
    } catch (why) {
      pick.value = kept;
      said.textContent = fill(words.language_not_saved, why);
    }
  });
  row.append(label, pick, said);
  return row;
}

// The switch on a meeting's page: the file written or removed at once; what had left said.
function confidentialCard(session, confidential, words) {
  const name = nameOf(session);
  const card = element("section", "card confidential-card");
  card.id = "confidential";
  const { row, box } = confidentialSwitch(words, "confidential-switch");
  box.checked = confidential.on;
  const left = element("p", "left");
  left.id = "confidential-left";
  const said = element("p", "says");
  said.id = "confidential-said";
  said.setAttribute("role", "status");
  const paint = (shown) => {
    card.classList.toggle("on", shown.on);
    left.textContent = leftSaid(words, shown.left);
    left.hidden = left.textContent === "";
    said.hidden = said.textContent === "";
  };
  box.addEventListener("change", async () => {
    box.disabled = true;
    try {
      const shown = await window.__TAURI__.core.invoke("confidential_set", { name, on: box.checked });
      said.textContent = "";
      paint(shown);
      // The title's chips say it at once.
      const holds = document.querySelector("#meeting .meeting-title .holds");
      holds?.querySelector(".chip.locked")?.remove();
      if (shown.on) holds?.prepend(lockedChip(words));
    } catch {
      box.checked = !box.checked;
      said.textContent = words.confidential_not_saved;
      said.hidden = false;
    }
    box.disabled = false;
    readMakings();
  });
  card.append(row, left, said);
  paint(confidential);
  return card;
}

// A session's folder name: what the window calls it, and the only date it says when its start
// cannot be read.
const nameOf = (session) => session.folder.split(/[\\/]/).pop();

function heading(session, words) {
  const day = dayOf(session);
  if (day === null) return nameOf(session);
  const date = new Intl.DateTimeFormat(words.locale, { weekday: "short", day: "numeric", month: "short" }).format(day);
  const time = new Intl.DateTimeFormat(words.locale, { hour: "2-digit", minute: "2-digit" }).format(day);
  return `${date} · ${time}`;
}

// What a meeting holds; on its page, where each step ran (`where`: the minutes' provider known).
function holdings(session, words, where) {
  const holds = document.createElement("div");
  holds.className = "holds";
  // A place is said only where it is known: `here` true or false; unknown, the chip says none.
  const ran = (text, here) => (where && here !== undefined ? `${text} · ${here ? words.ran_here : words.ran_online}` : text);
  // Said confidential (D60): the lock and its word, first — never the lock alone.
  if (session.confidential) holds.append(lockedChip(words));
  if (session.state === "recording") holds.append(chip(words.recording, "live"));
  if (session.state === "interrupted") holds.append(chip(words.interrupted, "cut"));
  if (session.minutes) {
    const made = chip(ran(words.minutes, where?.minutes_here));
    // Turned from what was being made: said by one small pop, once (D69).
    if (chipsMaking.delete(nameOf(session))) made.classList.add("turned");
    holds.append(made);
  }
  if (session.transcript) holds.append(chip(ran(words.transcript, true)));
  holds.append(session.audio > 0 ? chip(fill(words.audio, size(session.audio, words))) : chip(words.audio_freed, "gone"));
  // What is being made for it, in the background (D25): painted from `makings`.
  const making = chip("", "making");
  making.dataset.name = nameOf(session);
  // Minutes there already, made again: the chip that follows has not turned from anything.
  making.dataset.first = String(!session.minutes);
  making.hidden = true;
  holds.append(making);
  return holds;
}

// What Wholeheard makes in the background (D25): read with the header, painted where it is shown.
let makings = null;
const chipsMaking = new Set();

function minutesState(name) {
  if (makings === null) return null;
  if (makings.minutes?.name === name) return { kind: "making", ...makings.minutes };
  const waiting = makings.minutes_waiting.indexOf(name);
  if (waiting >= 0) return { kind: "waiting", before: waiting + 1 };
  if (makings.awaiting[name] !== undefined) return { kind: "awaiting", host: makings.awaiting[name] };
  // Confidential, waiting for Ollama: silent, or holding no model (D60).
  if (makings.ollama_waiting[name] !== undefined) {
    return { kind: "ollama", silent: makings.ollama_waiting[name] === "silent", busy: makings.model_fetching !== null };
  }
  if (makings.failed[name] !== undefined) return { kind: "failed", why: makings.failed[name], said: makings.failed_said[name] };
  return null;
}

function transcriptState(name) {
  if (makings === null) return null;
  if (makings.transcript?.name === name) return { kind: "making", ...makings.transcript };
  const waiting = makings.transcripts_waiting.indexOf(name);
  // Nothing being made and the model not there: the line waits for it, not for a turn.
  if (waiting >= 0 && makings.transcript === null && makings.model_needed) {
    return { kind: "model", coming: makings.model_coming, busy: makings.model_fetching !== null && !makings.model_coming };
  }
  if (waiting >= 0) return { kind: "waiting", before: waiting + 1 };
  if (makings.transcripts_failed[name] !== undefined) return { kind: "failed", why: makings.transcripts_failed[name] };
  return null;
}

// One card's state, said where its content is: what makes it and for how long, its place in
// line, the word it waits for, or why it was not made, with Try again (D25).
function paintState(said, shown, of) {
  said.hidden = shown === null;
  said.className = `making-state ${shown?.kind ?? ""}`;
  // Being made, waiting, or failed with Try again: not offered a second time.
  const offered = document.getElementById(of.offer);
  if (offered) offered.hidden = shown !== null;
  // Drawn again only when it says something else — a link or a button under the pointer or the
  // keyboard's focus stays the same one —; while it is made, only its time moves.
  const { seconds, ...same } = shown ?? {};
  const key = shown === null ? "" : JSON.stringify(same);
  const minutes = `${Math.floor((seconds ?? 0) / 60)}:${String((seconds ?? 0) % 60).padStart(2, "0")}`;
  const by = shown?.kind !== "making" ? "" : shown.host === undefined ? spoken(shown.model) : `${modelSaid(shown.model)} · ${shown.host}`;
  if (said.dataset.key === key && said.childElementCount > 0) {
    const time = said.querySelector(".by");
    if (time) time.textContent = `${by} · ${minutes}`;
    return;
  }
  said.dataset.key = key;
  if (shown === null) return said.replaceChildren();
  if (shown.kind === "making") {
    said.replaceChildren(element("span", "what", of.making), element("span", "by", `${by} · ${minutes}`), element("span", "making-bar"));
  } else if (shown.kind === "waiting") {
    said.replaceChildren(element("span", "what", fill(words.minutes_waiting, shown.before)));
  } else if (shown.kind === "model") {
    if (shown.coming) said.replaceChildren(element("span", "what", words.transcript_model_coming));
    else said.replaceChildren(element("span", "what", words.transcript_model_needed), modelFetch(shown.busy));
  } else if (shown.kind === "ollama") {
    // Silent: said where to get it, and tried again on a click; holding none: its model offered,
    // as Settings offer it — once it is there, the minutes are made.
    if (shown.silent) {
      const again = element("button", "open", words.try_again);
      again.type = "button";
      again.id = of.again;
      again.addEventListener("click", () => askAgain(of, said, again));
      said.replaceChildren(element("span", "what", words.welcome_ollama_silent), again);
    } else {
      const fetch_ = element("button", "open", fill(words.confidential_fetch, makings.ollama_recommended));
      fetch_.type = "button";
      fetch_.id = "ollama-fetch";
      fetch_.disabled = shown.busy;
      fetch_.addEventListener("click", async () => {
        fetch_.disabled = true;
        await window.__TAURI__.core.invoke("ollama_pull_offered").catch(() => {});
        readMakings();
      });
      said.replaceChildren(element("span", "what", words.confidential_ollama_empty), fetch_);
    }
  } else if (shown.kind === "awaiting") {
    const agree = element("a", "open", words.agree_open);
    agree.id = "minutes-agree";
    agree.href = `#/agree/${said.dataset.name}`;
    said.replaceChildren(element("span", "what", fill(words.minutes_awaiting, shown.host)), agree);
  } else {
    const again = element("button", "open", words.try_again);
    again.type = "button";
    again.id = of.again;
    again.addEventListener("click", () => askAgain(of, said, again));
    // What the program said, only in developer mode: the shell sends nothing else (D29).
    const detail = shown.why ? [element("code", "detail", fill(words.program_said, shown.why))] : [];
    // Its cause and gesture, when the program named one (D29): else the step's sentence alone.
    const what = shown.said ? `${of.not_made} ${shown.said}` : of.not_made;
    said.replaceChildren(element("span", "what", what), again, ...detail);
  }
}

// The tray's own fetch of the default model, offered where the window says it is missing (D36).
function modelFetch(busy = false) {
  const fetch_ = element("button", "open", fill(words.model_offer, size(makings.model_offer_bytes, words), makings.model_offer_host));
  fetch_.type = "button";
  fetch_.id = "model-fetch";
  fetch_.disabled = busy;
  fetch_.addEventListener("click", async () => {
    fetch_.disabled = true;
    await window.__TAURI__.core.invoke("model_fetch").catch(() => {});
    readMakings();
  });
  return fetch_;
}

// Each model of Settings' list: here, coming and how far, or offered — one fetch at a time (D37).
function paintOffered(list = document.getElementById("offered")) {
  if (!list || makings === null) return;
  const coming = makings.model_fetching;
  for (const item of list.children) {
    const far = coming?.file === item.dataset.offered ? Math.floor((100 * coming.held) / Math.max(coming.bytes, 1)) : null;
    const absent = item.dataset.by === "absent";
    const kind = item.dataset.here === "true" ? "here" : far !== null ? `far ${far}` : absent ? "absent" : coming ? "busy" : "offered";
    if (item.dataset.kind === kind) continue;
    item.dataset.kind = kind;
    const state = item.querySelector(".state");
    if (kind === "here") state.replaceChildren(readySaid(words.on_this_computer));
    else if (far !== null) state.replaceChildren(element("span", "says", fill(words.offered_far, far)));
    else if (absent) state.replaceChildren(element("span", "says", words.offered_needs_ollama));
    else {
      const fetch_ = element("button", "open", words.offered_fetch);
      fetch_.type = "button";
      fetch_.disabled = kind === "busy";
      fetch_.addEventListener("click", async () => {
        fetch_.disabled = true;
        const { invoke } = window.__TAURI__.core;
        const asked = item.dataset.by ? invoke("ollama_pull_offered") : invoke("model_offered_fetch", { file: item.dataset.offered });
        await asked.catch(() => {});
        readMakings();
      });
      state.replaceChildren(fetch_);
    }
  }
}

// Settings' model, not on this computer: coming, offered, or one the user named — theirs (D36).
function paintModel(state = document.getElementById("model-state")) {
  if (!state?.dataset.missing || makings === null) return;
  const busy = makings.model_fetching !== null && !makings.model_coming;
  const kind = makings.model_coming ? "coming" : makings.model_needed ? (busy ? "needed busy" : "needed") : "named";
  if (state.dataset.kind === kind) return;
  // Offered or coming, and now neither: the default model came — the page drawn as it now is,
  // once: the drawing paints the makings again, this card still in it.
  const came = kind === "named" && state.dataset.kind;
  state.dataset.kind = kind;
  if (came) return refresh();
  state.querySelector(".says").textContent = { coming: words.model_coming, named: words.model_named_missing }[kind] ?? words.model_missing;
  state.querySelector("#model-fetch")?.remove();
  // Another model coming: offered, not yet clickable — one fetch at a time (D37).
  if (kind.startsWith("needed")) state.append(modelFetch(busy));
}

// The model being fetched when the page was last painted: once it ends, Settings are drawn again.
let lastFetching = null;

function paintMakings() {
  // Drawn again first, never painted then drawn over: what is under the focus stays one drawing.
  const fetching = makings?.model_fetching?.file ?? null;
  const ended = lastFetching !== null && fetching === null;
  lastFetching = fetching;
  if (ended && document.getElementById("offered")) return refresh();
  paintModel();
  paintOffered();
  for (const chip_ of document.querySelectorAll(".chip.making")) {
    // The minutes' state first; else the transcript's, which comes before them.
    const minutes = minutesState(chip_.dataset.name);
    const transcript = transcriptState(chip_.dataset.name);
    const state = minutes ?? transcript;
    chip_.hidden = state === null;
    if (state === null) continue;
    if (minutes && chip_.dataset.first === "true") chipsMaking.add(chip_.dataset.name);
    chip_.textContent = minutes
      ? { making: words.chip_making, waiting: words.chip_waiting, ollama: words.chip_waiting, awaiting: words.chip_awaiting, failed: words.chip_failed }[state.kind]
      : { making: words.chip_transcribing, waiting: words.chip_waiting, model: words.chip_waiting, failed: words.chip_transcript_failed }[state.kind];
    chip_.classList.toggle("failed", state.kind === "failed");
  }
  const minutes = document.getElementById("minutes-state");
  if (minutes) {
    paintState(minutes, minutesState(minutes.dataset.name), {
      offer: "make-minutes", again: "minutes-again", ask: "minutes_again", making: words.minutes_making, not_made: words.minutes_not_made,
    });
  }
  // No line corrected while the transcript is made or waits to be (D62): its pencils hidden.
  for (const pencil of document.querySelectorAll("#meeting .lines .edit")) {
    if (pencil.parentElement.querySelector(".correcting")) continue;
    const kind = transcriptState(pencil.dataset.name)?.kind;
    // Nor while its minutes are written: they read the transcript before (#528's verification).
    pencil.hidden = (kind !== undefined && kind !== "failed") || minutesState(pencil.dataset.name)?.kind === "making";
  }
  const transcript = document.getElementById("transcript-state");
  if (transcript) {
    paintState(transcript, transcriptState(transcript.dataset.name), {
      offer: "make-transcript", again: "transcript-again", ask: "transcript_again", making: words.transcript_making, not_made: words.transcript_not_made,
    });
  }
  paintContext();
}

// *Réessayer*: fails only when the meeting is gone, said there (D67); a later failure is on this page.
async function askAgain(of, said, again) {
  again.disabled = true;
  try {
    await window.__TAURI__.core.invoke(of.ask, { name: said.dataset.name });
  } catch (why) {
    said.replaceChildren(element("span", "what", fill(words.again_refused, why)));
    return;
  }
  // Failing again for the same reason is said again: the state drawn anew.
  said.dataset.key = "";
  readMakings();
}

// « Enregistré », a moment, under what was changed (D68): the page is drawn again by what it says,
// so the sentence is put there after the drawing, by the name of what changed.
let savedFor = null;
let lastChanged = "";
const remembered = (event) => {
  const control = event.target.closest?.("input, select, textarea, button");
  if (!control) return;
  lastChanged = control.id ? `#${CSS.escape(control.id)}` : control.name ? `[name="${CSS.escape(control.name)}"][value="${CSS.escape(control.value)}"]` : "";
};
document.addEventListener("change", remembered, true);
document.addEventListener("click", remembered, true);
function savedSaid(saved) {
  clearTimeout(savedFor);
  document.getElementById("saved-said")?.remove();
  if (!saved) return;
  const savedSentence = element("p", "says saved-said", words.saved);
  savedSentence.id = "saved-said";
  savedSentence.setAttribute("role", "status");
  const changed = lastChanged ? document.querySelector(`#settings ${lastChanged}`) : null;
  const row = changed?.closest("label, .row, .field");
  if (changed) (row ?? changed).after(savedSentence);
  else document.getElementById("settings").prepend(savedSentence);
  savedSentence.scrollIntoView({ block: "nearest" });
  savedFor = setTimeout(() => savedSentence.remove(), 1500);
}

async function readMakings() {
  makings = await window.__TAURI__.core.invoke("makings").catch(() => null);
  // Only the meeting shown: its page hidden, the last one opened is no longer in front.
  const opened = document.getElementById("meeting").hidden ? null : document.getElementById("minutes-state")?.dataset.name;
  // The minutes or the transcript of the meeting shown written since its page was drawn, however
  // quickly: drawn again, to show them — unless the user has a panel open there, or is typing in
  // it, which a redraw would take from under them; then once that is done.
  const busy = document.querySelector("#meeting .redo, #meeting input:focus, #meeting textarea:focus");
  const drawn = (id) => document.getElementById(id)?.dataset.made;
  const moved = opened && makings
    && ((String(makings.made[opened] ?? 0) !== drawn("minutes-state") && minutesState(opened)?.kind !== "making")
      || (String(makings.transcripts_made[opened] ?? 0) !== drawn("transcript-state") && transcriptState(opened)?.kind !== "making"));
  if (moved && !busy) refresh();
  paintMakings();
}

// How much was said over a meeting, small, as the board draws it beside each one: one line of
// speech, both sides together, over its baseline. Its page draws who spoke, side by side.
function shapeOf([here, remote], quiet) {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("viewBox", "0 0 132 36");
  // The row says the meeting; the line only draws it: nothing for a screen reader to repeat.
  svg.setAttribute("aria-hidden", "true");
  svg.classList.add("relief", "small");
  if (quiet) svg.classList.add("quiet");
  const step = 132 / Math.max(1, here.length - 1);
  const base = document.createElementNS("http://www.w3.org/2000/svg", "line");
  for (const [name, value] of [["x1", 0], ["y1", 32], ["x2", 132], ["y2", 32]]) base.setAttribute(name, value);
  base.classList.add("base");
  const said = document.createElementNS("http://www.w3.org/2000/svg", "polyline");
  said.setAttribute("points", here.map((v, i) => `${(i * step).toFixed(1)},${(32 - Math.min(1, v + remote[i]) * 26).toFixed(1)}`).join(" "));
  said.classList.add("said");
  svg.append(base, said);
  return svg;
}

function row(session, shape, words) {
  const item = document.createElement("li");
  const link = document.createElement("a");
  link.className = "card session";
  link.href = `#/meeting/${encodeURIComponent(nameOf(session))}`;
  link.dataset.state = session.state;
  const when = document.createElement("div");
  when.className = "when";
  const title = document.createElement("span");
  title.className = "day";
  // Named by its minutes (D48): the name whole in its tooltip, the date beneath it.
  title.textContent = session.title ?? heading(session, words);
  if (session.title) title.title = session.title;
  if (session.title) when.append(element("span", "on", heading(session, words)));
  else when.append(title);
  if (session.seconds !== null) {
    const lasted = document.createElement("span");
    lasted.className = "length";
    lasted.textContent = length(session.seconds);
    when.append(lasted);
  }
  // No transcript read: a quiet line where its relief would be, so the rows keep their rhythm.
  const drawn = element("span", "shape");
  const none = new Array(24).fill(0);
  drawn.append(shapeOf(shape ?? [none, none], !shape));
  // In the sidebar (D61): the title and its relief on a line, what it holds below.
  const line = element("div", "line");
  line.append(when, drawn);
  // A name has the row's whole width, its date and relief under it.
  if (session.title) link.append(title);
  link.append(line, holdings(session, words));
  item.append(link);
  return item;
}

// The header's button, as the shell says the recording is — or as the user just asked it to be,
// until the command has answered (D68).
let recordAsked = null;
let recordRefocus = false;
function paintRecord(recording, stopping) {
  const live = recordAsked ?? recording;
  // Ending, it waits: *Arrêt…*, a click then would ask nothing.
  const ending = recordAsked === false || (recordAsked === null && stopping);
  document.getElementById("record-said").textContent = ending ? words.record_stopping : live ? words.record_stop : words.record_start;
  document.getElementById("record").classList.toggle("live", live);
  // Disabled, it loses the focus: given back with the button, for whoever stopped by the keyboard.
  const button = document.getElementById("record");
  if (ending && document.activeElement === button) recordRefocus = true;
  button.disabled = ending;
  if (!ending && recordRefocus) {
    recordRefocus = false;
    if (document.activeElement === document.body) button.focus();
  }
}

function show({ words, folder, sessions, shapes, recording, stopping, shortcut, folded, unreadable }) {
  document.documentElement.lang = words.locale;
  // The header's gesture, as the board has it: start or stop, with the shortcut beside.
  document.getElementById("free-link").textContent = words.free;
  const small = document.getElementById("small-link");
  // An icon with its name: the header's one action stays the recording's (D69, UX review 3.2).
  small.replaceChildren(icon("picture-in-picture-2"));
  small.title = words.small;
  small.setAttribute("aria-label", words.small);
  // At rest too: the small window is the user's to choose (D35).
  small.hidden = false;
  paintRecord(recording, stopping);
  const keys = document.getElementById("header-keys");
  keys.textContent = shortcut ?? "";
  keys.hidden = !shortcut;
  document.getElementById("sidebar").setAttribute("aria-label", words.meetings);
  document.getElementById("sections").setAttribute("aria-label", words.sections);
  document.getElementById("settings-link").textContent = words.settings;
  document.getElementById("pane-back").replaceChildren(icon("chevron-left"), words.back);
  // Folded as the user left it (D61), each button named with its shortcut.
  document.body.classList.toggle("folded", folded);
  const folding = /Mac/.test(navigator.platform) ? "⌘B" : "Ctrl+B";
  for (const [id, name] of [["fold", words.fold], ["unfold", words.unfold]]) {
    const button = document.getElementById(id);
    button.setAttribute("aria-label", name);
    button.title = `${name} (${folding})`;
  }
  const find = document.getElementById("find");
  find.placeholder = words.find_meetings;
  find.setAttribute("aria-label", words.find_meetings);
  find.hidden = sessions.length === 0;
  document.getElementById("none-found").textContent = words.none_found;
  const weight = document.getElementById("weight");
  weight.hidden = sessions.length === 0;
  // A folder that cannot be read is no empty list: said, and tried again (D67).
  document.getElementById("none").textContent = unreadable ? fill(words.list_unread, folder) : fill(words.none, folder);
  document.getElementById("none-do").textContent = unreadable ? words.list_unread_do : words.none_do;
  document.getElementById("empty-record").textContent = words.record_start;
  document.getElementById("empty-record").hidden = recording || unreadable;
  document.getElementById("empty-again").textContent = words.try_again;
  document.getElementById("empty-again").hidden = !unreadable;

  const audio = sessions.reduce((sum, s) => sum + s.audio, 0);
  const text = sessions.reduce((sum, s) => sum + s.text, 0);
  const heard = sessions.filter((s) => s.audio > 0 && s.seconds !== null).reduce((sum, s) => sum + s.seconds, 0);
  document.getElementById("total").textContent = size(audio + text, words);
  const taken = sessions.length === 1 ? words.weight_one : words.weight;
  document.getElementById("taken").textContent = fill(taken, sessions.length, length(heard));
  document.getElementById("audio-kept").textContent = fill(words.audio_kept, size(audio, words));
  document.getElementById("text-kept").textContent = fill(words.text_kept, size(text, words));
  const share = audio + text > 0 ? (100 * audio) / (audio + text) : 0;
  document.getElementById("bar-audio").style.width = `${share}%`;
  document.getElementById("bar-text").style.width = `${100 - share}%`;
  document.getElementById("bar").setAttribute("aria-label", fill(words.weight_described, size(audio, words), size(text, words)));

  const now = new Date();
  const groups = document.getElementById("groups");
  // Drawn anew at each reading: the row that held the focus holds it again.
  const focused = document.activeElement?.closest?.("#groups .session")?.getAttribute("href");
  groups.replaceChildren();
  let current = null;
  for (const [index, session] of sessions.entries()) {
    const name = group(dayOf(session), now, words);
    if (!current || current.name !== name) {
      const section = document.createElement("section");
      section.className = "group";
      const heading = document.createElement("h2");
      heading.textContent = name;
      const list = document.createElement("ol");
      section.append(heading, list);
      groups.append(section);
      current = { name, list };
    }
    current.list.append(row(session, shapes[index], words));
  }
  if (focused) groups.querySelector(`.session[href="${CSS.escape(focused)}"]`)?.focus({ preventScroll: true });
  sought();
}

// The sidebar's search (D61): the meetings whose row holds what is typed, the others hidden.
function sought() {
  // Accents and case aside, as the transcript's own search reads.
  const typed = folded(document.getElementById("find").value.trim());
  let any = false;
  for (const section of document.querySelectorAll("#groups .group")) {
    let some = false;
    for (const item of section.querySelectorAll("li")) {
      item.hidden = typed !== "" && !folded(item.textContent).includes(typed);
      some ||= !item.hidden;
    }
    section.hidden = !some;
    any ||= some;
  }
  document.getElementById("none-found").hidden = any || typed === "";
}

// How long the small window says the reminder, in seconds (D50).
const ANNOUNCED_FOR = 15;
// How long a side's ✓ shows before it fades, in milliseconds (D49).
const CHECK_SHOWN = 2500;

const element = (tag, className, text) => {
  const made = document.createElement(tag);
  if (className) made.className = className;
  if (text !== undefined) made.textContent = text;
  return made;
};

// A meeting's context (D59): the field, its count, and the script it is written in. Never more
// than the shell takes, counted as it counts: in characters, not in UTF-16's units.
const CONTEXT_MOST = 400;
// Latin and Cyrillic are heard whole within the 400; another script — Greek, Arabic, Chinese… —
// may be cut by Whisper or refused by a service (#489).
const otherScript = /[^\p{Script=Latin}\p{Script=Cyrillic}\p{Script=Common}\p{Script=Inherited}]/u;

function contextField(words, text, save, id) {
  const area = element("textarea", "context-text");
  area.id = id;
  area.value = text;
  const count = element("span", "context-count");
  count.id = `${id}-count`;
  // Announced as it changes, as a status is.
  count.setAttribute("role", "status");
  area.setAttribute("aria-describedby", count.id);
  const painted = () => {
    const left = CONTEXT_MOST - [...area.value].length;
    const leftSaid = fill(words.context_left[left <= 1 ? 0 : 1], left);
    const other = otherScript.test(area.value);
    count.textContent = other ? `${leftSaid} · ${words.context_script}` : `${fill(words.context_most, CONTEXT_MOST)} · ${leftSaid}`;
    count.classList.toggle("other-script", other);
  };
  // A character typed past the most is not taken; a paste or a drop too long is let in, then cut
  // to it below — refused whole, nothing of it would be taken.
  area.addEventListener("beforeinput", (event) => {
    if (/^insertFrom(Paste|Drop)/.test(event.inputType)) return;
    const data = event.data ?? (/^insert(LineBreak|Paragraph)$/.test(event.inputType) ? "\n" : null);
    if (data === null || !event.inputType.startsWith("insert")) return;
    const replaced = [...area.value.slice(area.selectionStart, area.selectionEnd)].length;
    if ([...area.value].length - replaced + [...data].length > CONTEXT_MOST) event.preventDefault();
  });
  area.addEventListener("input", () => {
    const typed = [...area.value];
    if (typed.length > CONTEXT_MOST) area.value = typed.slice(0, CONTEXT_MOST).join("");
    painted();
    save(area.value);
  });
  painted();
  return { area, count, painted };
}

// The context on a meeting's page: shown and written, closed while a making of it runs or its
// transcript is left unfinished, and made again with it when what was made had another.
function contextCard(session, { context, transcript, minutes }, words) {
  const name = nameOf(session);
  const card = element("section", "card context");
  card.id = "context";
  card.dataset.name = name;
  card.dataset.changed = String(context.changed);
  // Unfinished, nothing making it: continued with another context, it would be refused.
  card.dataset.unfinished = String(transcript.state === "present" && !transcript.complete);
  const said = element("p", "says");
  said.id = "context-said";
  said.setAttribute("role", "status");
  const changed = element("div", "context-changed");
  changed.id = "context-changed";
  const redo = element("button", "open", words.context_redo);
  redo.type = "button";
  redo.id = "context-redo";
  redo.addEventListener("click", async () => {
    redo.disabled = true;
    await window.__TAURI__.core.invoke("context_redo", { name }).catch((why) => {
      said.textContent = fill(words.not_saved, why);
    });
    await readMakings();
    redo.disabled = false;
  });
  changed.append(element("p", "says", words.context_changed[minutes.state === "present" ? 1 : 0]), redo);
  const save = async (text) => {
    // Written now: what it could not read back is no longer the last word.
    delete card.dataset.unread;
    try {
      const shown = await window.__TAURI__.core.invoke("context_saved", { name, text });
      card.dataset.changed = String(shown.changed);
      said.textContent = "";
    } catch {
      said.textContent = words.context_not_saved;
    }
    paintContext();
  };
  const { area, count } = contextField(words, context.text, save, "context-text");
  area.rows = 3;
  area.placeholder = words.context_example;
  const label = element("label", "context-label", words.context_label);
  label.htmlFor = area.id;
  // Made again from its audio: a meeting whose audio was freed is not offered it.
  card.dataset.redo = String(session.audio > 0);
  card.append(element("h2", "label", words.context_title), label, area, count, element("p", "says", words.context_why), said, changed);
  return card;
}

// The context's field as the makings stand: closed and said while one of its meeting runs.
function paintContext() {
  const card = document.getElementById("context");
  if (!card) return;
  const name = card.dataset.name;
  const making = minutesState(name)?.kind === "making" || transcriptState(name)?.kind === "making";
  const closed = making ? words.context_making : card.dataset.unfinished === "true" ? words.context_unfinished : null;
  const area = document.getElementById("context-text");
  // A making of it ended: whether what is there now was made with this context is read again.
  if (card.dataset.making === "true" && !making) {
    window.__TAURI__.core
      .invoke("context_shown", { name })
      .then((shown) => {
        card.dataset.changed = String(shown.changed);
        delete card.dataset.unread;
        paintContext();
      })
      // Its flag and its text stay as they were, and that they could not be read back is said.
      .catch((why) => {
        card.dataset.unread = fill(words.context_not_read, why);
        paintContext();
      });
  }
  card.dataset.making = String(making);
  area.readOnly = closed !== null;
  const said = document.getElementById("context-said");
  if (closed !== null) said.textContent = closed;
  else if (card.dataset.unread) said.textContent = card.dataset.unread;
  else if (said.textContent !== words.context_not_saved) said.textContent = "";
  said.hidden = said.textContent === "";
  document.getElementById("context-changed").hidden = closed !== null || card.dataset.changed !== "true" || card.dataset.redo !== "true";
}

const clock = (seconds) => {
  const whole = Math.floor(seconds);
  const parts = [Math.floor(whole / 3600), Math.floor((whole % 3600) / 60), whole % 60];
  const shown = parts[0] > 0 ? parts : parts.slice(1);
  return shown.map((part) => String(part).padStart(2, "0")).join(":");
};

// Who spoke, over the meeting: for each track, the share of each sixtieth of it covered by speech.
function relief(segments, total, words) {
  const bins = 60;
  const share = { local: new Array(bins).fill(0), remote: new Array(bins).fill(0) };
  const width = total / bins;
  for (const segment of segments) {
    for (let bin = Math.floor(segment.start / width); bin < bins && bin * width < segment.end; bin += 1) {
      const covered = Math.min(segment.end, (bin + 1) * width) - Math.max(segment.start, bin * width);
      share[segment.track][bin] = Math.min(1, share[segment.track][bin] + covered / width);
    }
  }
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("viewBox", "0 0 1100 88");
  svg.setAttribute("preserveAspectRatio", "none");
  svg.setAttribute("role", "img");
  svg.setAttribute("aria-label", words.who_spoke);
  svg.classList.add("relief");
  for (const [track, sign, base] of [["local", -1, 42], ["remote", 1, 46]]) {
    const line = document.createElementNS("http://www.w3.org/2000/svg", "polyline");
    line.setAttribute("points", share[track].map((v, i) => `${(i * 1100) / (bins - 1)},${base + sign * v * 36}`).join(" "));
    line.classList.add(track);
    svg.append(line);
  }
  return svg;
}

// What a meeting lacks, offered on its page and made on the user's word: once asked, said so.
function offer(card, label, id, command, session, words) {
  const button = element("button", "primary", label);
  button.type = "button";
  button.id = id;
  button.addEventListener("click", async () => {
    button.disabled = true;
    try {
      await window.__TAURI__.core.invoke(command, { name: nameOf(session) });
      // Its state is said where it is made (D25).
      button.remove();
      readMakings();
    } catch (why) {
      button.replaceWith(element("p", "note warn", fill(words.not_saved, why)));
    }
  });
  card.append(button);
}

// A panel of *Refaire…* closes on *Annuler* or Escape: nothing changed, the focus back on *Refaire…* (D68).
function closes(panel, go, button, words) {
  const cancel = element("button", "open", words.redo_cancel);
  cancel.type = "button";
  cancel.id = `${panel.id}-cancel`;
  const close = () => {
    panel.replaceWith(button);
    button.focus();
  };
  cancel.addEventListener("click", close);
  // From the panel, or from nowhere — its title clicked; a field elsewhere keeps its own Escape.
  const onEscape = (event) => {
    if (!panel.isConnected) return document.removeEventListener("keydown", onEscape);
    if (event.key !== "Escape") return;
    const at = document.activeElement;
    if (at === document.body || panel.contains(at)) close();
  };
  document.addEventListener("keydown", onEscape);
  const row = element("div", "redo-actions");
  go.replaceWith(row);
  row.append(go, cancel);
  return panel;
}

function redoPanel(choices, session, words, button) {
  const panel = element("section", "redo");
  panel.id = "redo-panel";
  panel.append(element("h3", "", words.redo_title), element("p", "says", words.redo_with));
  const options = [];
  if (choices.provider && choices.model) options.push([choices.provider, choices.model, choices.provider === choices.ollama_address]);
  for (const name of choices.ollama) {
    if (!(choices.provider === choices.ollama_address && choices.model === name)) options.push([choices.ollama_address, name, true]);
  }
  const list = element("div", "group");
  list.setAttribute("role", "radiogroup");
  options.forEach(([provider, model, here], index) => {
    const one = element("label", "choice");
    const input = element("input");
    input.type = "radio";
    input.name = "redo-with";
    input.value = String(index);
    input.checked = index === 0;
    const said = element("span", "said");
    said.append(element("span", "name", modelSaid(model)));
    one.append(input, said, element("span", "says", here ? words.on_computer : words.ran_online));
    list.append(one);
  });
  const go = element("button", "primary", words.redo_do);
  go.type = "button";
  go.id = "redo-do";
  go.disabled = options.length === 0;
  go.addEventListener("click", async () => {
    const picked = panel.querySelector("input[name=redo-with]:checked");
    const [provider, model] = options[Number(picked?.value ?? 0)] ?? [];
    go.disabled = true;
    try {
      await window.__TAURI__.core.invoke("minutes_redo", { name: nameOf(session), provider: provider ?? null, model: model ?? null });
      // Asked: the panel goes, the making is said where the minutes are (D25).
      panel.replaceWith(button);
      readMakings();
    } catch (why) {
      go.replaceWith(element("p", "note warn", fill(words.not_saved, why)));
    }
  });
  panel.append(list, element("p", "says", words.redo_note), go);
  return closes(panel, go, button, words);
}

function redoTranscriptPanel(shown, session, words, button, corrected) {
  const panel = element("section", "redo");
  panel.id = "redo-transcript-panel";
  panel.append(element("h3", "", words.redo_transcript_title), element("p", "says", words.redo_with));
  const models = [...shown.models].sort((a, b) => (b.path === shown.model) - (a.path === shown.model));
  const list = element("div", "group");
  list.setAttribute("role", "radiogroup");
  models.forEach((model, index) => {
    const one = element("label", "choice");
    const input = element("input");
    input.type = "radio";
    input.name = "redo-transcript-with";
    input.value = model.path;
    input.checked = index === 0;
    const said = element("span", "said");
    said.append(element("span", "name", spoken(model.name)));
    one.append(input, said, element("span", "says", words.on_computer));
    list.append(one);
  });
  const go = element("button", "primary", words.redo_do);
  go.type = "button";
  go.id = "redo-transcript-do";
  go.disabled = models.length === 0;
  go.addEventListener("click", async () => {
    const picked = panel.querySelector("input[name=redo-transcript-with]:checked");
    go.disabled = true;
    try {
      await window.__TAURI__.core.invoke("transcript_redo", { name: nameOf(session), model: picked.value });
      // Asked: the panel goes, the making is said where the transcript is (D25).
      panel.replaceWith(button);
      readMakings();
    } catch (why) {
      go.replaceWith(element("p", "note warn", fill(words.not_saved, why)));
    }
  });
  panel.append(list, element("p", "says", words.redo_transcript_note));
  // Made again, the transcript is another file: a person's corrections are not in it (ADR-0014).
  if (corrected) {
    const lost = element("p", "note warn", words.redo_loses_corrections);
    lost.id = "redo-loses-corrections";
    panel.append(lost);
  }
  panel.append(go);
  return closes(panel, go, button, words);
}

function minutesCard(read, words, session, transcript) {
  const card = element("article", "card minutes");
  // Its title, and Refaire… beside it: the state below them never sits under a button.
  const top = element("div", "top");
  top.append(element("h2", "label", words.minutes_title));
  card.append(top);
  // What is being made for them, or why they were not (D25).
  const state = element("p", "making-state");
  state.id = "minutes-state";
  state.dataset.name = nameOf(session);
  state.dataset.made = String(makings?.made[nameOf(session)] ?? 0);
  state.setAttribute("role", "status");
  state.hidden = true;
  card.append(state);
  if (read.state !== "present") {
    card.append(element("p", "none", read.state === "absent" ? words.no_minutes : words.unreadable));
    if (read.state === "absent" && transcript.state === "present" && transcript.complete) {
      offer(card, words.make_minutes, "make-minutes", "minutes_asked", session, words);
    }
    return card;
  }
  // Refaire… (D11's sixth board): another model, this time only; the minutes stay until the new
  // ones are whole.
  const redo = element("button", "open", words.redo);
  redo.type = "button";
  redo.id = "redo-minutes";
  redo.addEventListener("click", async () => {
    const choices = await window.__TAURI__.core.invoke("minutes_shown");
    const panel = redoPanel(choices, session, words, redo);
    redo.replaceWith(panel);
    panel.querySelector("input, button:enabled").focus();
  });
  // Copied in one click, as written, to paste where the user writes (D45): said in place, a moment.
  const copy = element("button", "open", words.copy_minutes);
  copy.type = "button";
  copy.id = "copy-minutes";
  const copied = element("span", "says copied");
  copied.id = "minutes-copied";
  copied.setAttribute("role", "status");
  let said = 0;
  copy.addEventListener("click", async () => {
    const mine = ++said;
    try {
      await navigator.clipboard.writeText(read.markdown);
      copied.textContent = words.minutes_copied;
      setTimeout(() => mine === said && (copied.textContent = ""), 2000);
    } catch {
      copied.textContent = words.minutes_not_copied;
    }
  });
  const actions = element("span", "minutes-actions");
  actions.append(copied, copy, redo);
  top.append(actions);
  const made = read.made && dayOf({ instant: Date.parse(read.made) / 1000 || null });
  // As the board says it: the day of the week, no year, and the model as people say it.
  const day = { weekday: "short", day: "numeric", month: "short" };
  const hour = { hour: "2-digit", minute: "2-digit" };
  // What is known of who wrote them, and when: what is not known is left unsaid.
  let maker = fill(words.written_by, read.model ? modelSaid(read.model) : "—");
  if (read.provider) maker += fill(words.written_via, read.provider);
  if (made) maker += ` · ${new Intl.DateTimeFormat(words.locale, day).format(made)} ${words.at} ${new Intl.DateTimeFormat(words.locale, hour).format(made)}`;
  // What the call cost, as the provider reported it (D51); the shell gives both or neither.
  if (read.cost !== null) maker += ` · ${new Intl.NumberFormat(words.locale, { style: "currency", currency: read.currency, maximumSignificantDigits: 2 }).format(read.cost)}`;
  card.append(element("p", "maker", maker));
  // The transcript corrected after they were written (D62): said, and made again only on a click.
  const corrected = transcript.state === "present" ? Math.max(0, ...transcript.segments.map((segment) => Date.parse(segment.edited ?? "") || 0)) : 0;
  if (corrected > (Date.parse(read.made ?? "") || 0)) {
    const since = element("div", "corrected-since");
    since.id = "corrected-since";
    const again = element("button", "open", words.redo_corrections);
    again.type = "button";
    again.id = "redo-corrections";
    again.addEventListener("click", async () => {
      again.disabled = true;
      await window.__TAURI__.core.invoke("minutes_redo", { name: nameOf(session), provider: null, model: null }).catch((why) => {
        since.append(element("p", "note warn", fill(words.not_saved, why)));
      });
      readMakings();
    });
    since.append(element("p", "note", words.corrected_since), again);
    card.append(since);
  }
  // The minutes as written, rendered by the shell: raw HTML in them is text there, a link its words.
  const body = element("div", "body");
  body.innerHTML = read.body;
  actionRows(body, words);
  lineTimes(body, session, transcript, words);
  card.append(body);
  return card;
}

// A line's time (D75), the shell's own mark: a small link that opens its passage and plays from that
// second. Nothing to play, it opens the passage and says that alone; nothing to open either, it
// stays quiet text: the time as said, no link that leads nowhere. Once, on the gesture: the page
// does not follow what plays (D26).
function lineTimes(body, session, transcript, words) {
  const plays = player?.name === nameOf(session);
  const passages = transcript.state === "present" && transcript.segments.length > 0;
  for (const time of body.querySelectorAll("time")) {
    const second = Number(time.dataset.second);
    // As the transcript and the player write it, past the hour too.
    time.textContent = clock(second);
    if (!plays && !passages) continue;
    const link = element("button", "line-time", clock(second));
    link.type = "button";
    link.setAttribute("aria-label", fill(plays ? words.time_listen : words.time_passage, clock(second)));
    link.title = link.getAttribute("aria-label");
    link.addEventListener("click", () => {
      const lines = document.querySelector("#meeting .lines");
      // A segment holds its second from its start cut to whole, as the contract counts it.
      const all = lines ? [...lines.children] : [];
      const line = all.findLast((one) => Math.floor(Number(one.dataset.start)) <= second) ?? all[0];
      if (line?.hidden) {
        const search = document.getElementById("search");
        search.value = "";
        search.dispatchEvent(new Event("input"));
      }
      roam(lines, line);
      // In view already, the page stays as it is; else its passage comes with what is around it.
      const box = line?.getBoundingClientRect();
      if (box && (box.top < 0 || box.bottom > innerHeight)) line.scrollIntoView({ block: "center" });
      if (player?.name === nameOf(session)) {
        document.getElementById("not-played").textContent = "";
        player.seek(second);
        player.play();
      } else {
        // No playback to mark it: marked here, the line asked for alone.
        for (const one of all) one.classList.toggle("now", one === line);
      }
    });
    time.replaceWith(link);
  }
}

// The minutes' actions as rows (D46): « what — who — by when », as `minutes/v1` writes them, shown
// Qui · Quoi · Pour quand, who in its colour, by when in monospace, the cells named for assistive
// technology alone; an item that does not split in three is shown as written, in its place. A
// line's time is none of the three (`minutes/v3`): it sits after what.
function actionRows(body, words) {
  // Every second-level title, as the shell's reader counts them.
  const titles = body.querySelectorAll("h2");
  if (titles.length < 2) return;
  let list = titles[1].nextElementSibling;
  while (list && list !== titles[2] && !["UL", "OL"].includes(list.tagName)) list = list.nextElementSibling;
  if (!list || list === titles[2]) return;
  const table = element("table", "actions");
  const head = element("thead");
  const named = element("tr");
  for (const cell of words.actions_cells) named.append(element("th", "", cell));
  head.append(named);
  const rows = element("tbody");
  const unsettled = (text) => [words.unsettled, "non précisé", "not specified"].includes(text.toLowerCase());
  for (const item of list.children) {
    const time = item.querySelector("time");
    const held = time?.parentElement;
    time?.remove();
    const parts = item.querySelector("ul, ol") ? [] : item.textContent.trim().split(" — ").map((part) => part.trim());
    const row = element("tr");
    if (parts.length === 3 && parts.every((part) => part !== "")) {
      const [what, who, when] = parts;
      for (const [text, kind] of [[who, "who"], [what, "what"], [when, "when"]]) {
        // Short, on one line; long, back to the line between words, never pushing the card wider.
        const long = kind !== "what" && text.length > 18 ? " long" : "";
        const cell = element("td", (unsettled(text) ? `${kind} unsettled` : kind) + long, text);
        if (kind === "what" && time) cell.append(" ", time);
        row.append(cell);
      }
    } else {
      const cell = element("td", "written");
      cell.colSpan = 3;
      if (time) held.append(time);
      cell.append(...item.childNodes);
      row.append(cell);
    }
    rows.append(row);
  }
  table.append(head, rows);
  list.replaceWith(table);
}

const folded = (text) => text.normalize("NFD").replace(/\p{M}/gu, "").toLowerCase();

// The one line of a transcript in the order of Tab: this one, the others reached by the arrows.
function roam(lines, line) {
  if (!line || line.tabIndex === -1 && !line.hasAttribute("role")) return;
  for (const other of lines.children) {
    if (!other.hasAttribute("role")) continue;
    other.tabIndex = other === line ? 0 : -1;
    // Its pencil follows it in the order of Tab: one line, one pencil reached by Tab (D62).
    const pencil = other.querySelector(".edit");
    if (pencil) pencil.tabIndex = other.tabIndex;
  }
}

function transcriptCard(read, words, session) {
  const card = element("aside", "card transcript");
  card.setAttribute("aria-label", words.transcript_title);
  const top = element("div", "top");
  top.append(element("h2", "label", words.transcript_title));
  card.append(top);
  // What is being made for it, or why it was not (D25).
  const state = element("p", "making-state");
  state.id = "transcript-state";
  state.dataset.name = nameOf(session);
  state.dataset.made = String(makings?.transcripts_made[nameOf(session)] ?? 0);
  state.setAttribute("role", "status");
  state.hidden = true;
  card.append(state);
  if (read.state !== "present") {
    card.append(element("p", "none", read.state === "absent" ? words.no_transcript : words.unreadable));
    if (read.state === "absent" && session.audio > 0 && session.state !== "recording") {
      offer(card, words.make_transcript, "make-transcript", "transcript_asked", session, words);
    }
    return card;
  }
  const search = element("input", "search");
  search.type = "search";
  search.id = "search";
  search.placeholder = words.search;
  search.setAttribute("aria-label", words.search);
  top.append(search);
  // Refaire…: another Whisper model of this computer, this time only.
  const redo = element("button", "open", words.redo);
  redo.type = "button";
  redo.id = "redo-transcript";
  redo.addEventListener("click", async () => {
    const shown = await window.__TAURI__.core.invoke("settings_shown");
    const panel = redoTranscriptPanel(shown, session, words, redo, read.segments.some((segment) => segment.edited));
    redo.replaceWith(panel);
    panel.querySelector("input, button:enabled").focus();
  });
  // Beside who heard it, as board 2 has it: the head keeps its search whole.
  const heard = element("div", "made");
  // A service names its host as who heard (D43): said as the minutes' maker is, never « on this
  // computer » — a host of one word (`http://transcriber/v1`) is a service too. This computer's
  // engines name themselves: whisper.cpp, SpeechAnalyzer; none named, nothing is claimed.
  const service = read.engine && !["whisper.cpp", "SpeechAnalyzer"].includes(read.engine);
  const heardSaid = service
    ? fill(words.written_by, read.model || "—") + fill(words.written_via, read.engine)
    : fill(words.heard_by, read.model ? spoken(read.model) : read.engine || "—");
  heard.append(element("p", "maker", heardSaid), redo);
  card.append(heard);
  if (!read.complete) card.append(element("p", "note", words.being_made));
  if (read.unread > 0) card.append(element("p", "note", fill(words.unread, read.unread)));
  const lines = element("ol", "lines");
  // What a correction met, said where a screen reader hears it.
  const said_ = element("p", "says");
  said_.id = "correction-said";
  said_.setAttribute("role", "status");
  card.append(said_);
  for (const segment of read.segments) {
    const line = element("li");
    line.dataset.track = segment.track;
    line.dataset.start = String(segment.start);
    // A click on a line, or Enter on it, plays the meeting from it (D26).
    const from = () => {
      if (!player || player.name !== nameOf(session)) return;
      player.seek(segment.start);
      player.play();
    };
    line.addEventListener("click", () => {
      roam(lines, line);
      from();
    });
    // By the keyboard: one line in the order of Tab, the arrows to the others; Enter or Space
    // plays from it, as a button's keys do.
    if (player?.name === nameOf(session)) {
      line.setAttribute("role", "button");
      line.tabIndex = lines.children.length === 0 ? 0 : -1;
      line.addEventListener("keydown", (event) => {
        // The next line shown: one a search hides is passed over.
        let next = { ArrowDown: line.nextElementSibling, ArrowUp: line.previousElementSibling }[event.key];
        while (next?.hidden) next = event.key === "ArrowDown" ? next.nextElementSibling : next.previousElementSibling;
        if (next) {
          event.preventDefault();
          roam(lines, next);
          next.focus();
        }
        if (event.key !== "Enter" && event.key !== " ") return;
        event.preventDefault();
        if (!event.repeat) from();
      });
    }
    const side = segment.track === "local" ? words.here : words.remote;
    const who = segment.voice === null ? side : `${side} · ${fill(words.voice, segment.voice)}`;
    const said = element("div", "said");
    const text = element("span", "text", segment.text);
    said.append(element("span", `who ${segment.track}`, who), text);
    if (segment.edited) said.append(element("span", "edited", words.corrected));
    line.append(element("span", "at", clock(segment.start)), said);
    // Corrected by whoever listens, once complete (D62): the line becomes a field; Enter or leaving
    // it keeps what is typed, Escape leaves it as it was.
    if (read.complete) {
      const pencil = element("button", "edit", "✎");
      pencil.type = "button";
      pencil.dataset.name = nameOf(session);
      pencil.setAttribute("aria-label", fill(words.correct_line, clock(segment.start)));
      pencil.title = pencil.getAttribute("aria-label");
      pencil.tabIndex = line.hasAttribute("role") ? line.tabIndex : 0;
      // Its own keys: Enter on it opens the field, never plays the line it sits in.
      pencil.addEventListener("keydown", (event) => event.stopPropagation());
      pencil.addEventListener("click", (event) => {
        event.stopPropagation();
        const field = element("input", "correcting");
        field.type = "text";
        field.value = text.textContent;
        field.setAttribute("aria-label", pencil.getAttribute("aria-label"));
        // A line break pasted is a space: a field of one line would drop it, gluing two words.
        field.addEventListener("paste", (event) => {
          const pasted = event.clipboardData?.getData("text/plain");
          if (!pasted || !/[\r\n]/.test(pasted)) return;
          event.preventDefault();
          field.setRangeText(pasted.replace(/[\r\n]+/g, " "), field.selectionStart, field.selectionEnd, "end");
        });
        let done = false;
        const leave = (kept) => {
          done = true;
          if (kept !== null) text.textContent = kept;
          field.replaceWith(text);
          pencil.hidden = false;
        };
        const keep = async () => {
          if (done) return;
          if (field.value === text.textContent) return leave(null);
          if (field.value.replace(/[\r\n]+/g, " ").trim() === "") {
            said_.textContent = words.correction_empty;
            return field.focus();
          }
          done = true;
          try {
            await window.__TAURI__.core.invoke("transcript_corrected", { name: nameOf(session), track: segment.track, start: segment.start, text: field.value });
            said_.textContent = "";
            if (!said.querySelector(".edited")) said.append(element("span", "edited", words.corrected));
            leave(field.value.replace(/[\r\n]+/g, " ").trim());
          } catch (why) {
            said_.textContent = fill(words.correction_failed, why);
            leave(null);
          }
        };
        for (const kind of ["click", "keydown"]) field.addEventListener(kind, (event) => event.stopPropagation());
        field.addEventListener("keydown", (event) => {
          if (event.key === "Enter") {
            event.preventDefault();
            keep().then(() => pencil.focus());
          } else if (event.key === "Escape") {
            event.preventDefault();
            said_.textContent = "";
            leave(null);
            pencil.focus();
          }
        });
        field.addEventListener("blur", () => keep());
        text.replaceWith(field);
        pencil.hidden = true;
        field.focus();
      });
      line.append(pencil);
    }
    lines.append(line);
  }
  const nothing = element("p", "none", words.no_match);
  nothing.hidden = true;
  search.addEventListener("input", () => {
    const wanted = folded(search.value.trim());
    let shown = 0;
    for (const line of lines.children) {
      line.hidden = wanted !== "" && !folded(line.querySelector(".text").textContent).includes(wanted);
      shown += line.hidden ? 0 : 1;
    }
    nothing.hidden = shown > 0;
    // The line in the order of Tab hidden: the first one shown takes its place.
    const stop = [...lines.children].find((line) => line.tabIndex === 0);
    if (stop?.hidden) roam(lines, [...lines.children].find((line) => !line.hidden));
  });
  card.append(lines, nothing);
  return card;
}

// The meeting being listened to (D26): kept while its page is drawn again, so that a window
// coming forward never stops what plays. Stopped when another page is shown.
let player = null;

// Plays a meeting from the loopback server: its tracks as one sound, each where the session's
// clock puts it (ADR-0009). One media element, so the page keeps no clock of its own: its time
// is the session's.
function listen(name, url) {
  if (player?.name === name) return player;
  player?.stop();
  const audio = new Audio(url);
  audio.preload = "metadata";
  audio.className = "track";
  const shown = { name, audio, listeners: new Set(), failures: new Set(), ticking: 0 };
  const tell = () => {
    for (const listener of shown.listeners) listener(audio.currentTime);
  };
  const failed = (error) => {
    // A play cut short by a pause is no failure.
    if (error?.name === "AbortError") return;
    clearInterval(shown.ticking);
    audio.pause();
    for (const failure of shown.failures) failure();
  };
  shown.time = () => audio.currentTime;
  shown.playing = () => !audio.paused;
  // After a failure, the sound is asked for again before it is played.
  shown.play = () => {
    if (audio.error) audio.load();
    return audio.play().catch(failed);
  };
  shown.pause = () => audio.pause();
  // Never at its very end, where WebKitGTK's player stops answering: a tenth of a second before.
  // A seek asked while another is still going on is kept and made once that one ends: WebKitGTK's
  // player drops it otherwise, and the slider stays where it was (#369).
  shown.wanted = null;
  shown.seek = (at) => {
    shown.wanted = Math.max(0, Math.min(at, (audio.duration || at + 1) - 0.1));
    if (!audio.seeking) audio.currentTime = shown.wanted;
    tell();
  };
  audio.addEventListener("seeked", () => {
    if (shown.wanted !== null && Math.abs(audio.currentTime - shown.wanted) > 0.05) audio.currentTime = shown.wanted;
    else shown.wanted = null;
    tell();
  });
  shown.stop = () => {
    clearInterval(shown.ticking);
    audio.pause();
    audio.remove();
  };
  // The playhead and the line said, ten times a second while it plays; once more at each change.
  // A failure is read at each tick too: WebKitGTK sets a network error on a range refused
  // without the event, and its player waits for good.
  audio.addEventListener("play", () => {
    clearInterval(shown.ticking);
    shown.ticking = setInterval(() => (audio.error ? failed(audio.error) : tell()), 100);
    tell();
  });
  for (const changed of ["pause", "ended", "seeked", "loadedmetadata"]) {
    audio.addEventListener(changed, () => {
      if (changed !== "seeked" && changed !== "loadedmetadata") clearInterval(shown.ticking);
      tell();
    });
  }
  audio.addEventListener("error", failed);
  // Kept in the page, never shown: the browser plays an element it holds.
  document.body.append(audio);
  player = shown;
  return shown;
}

// The player of a meeting's page: Écouter / Pause, where the playback is, and the drawing of who
// spoke as its control — a click, or the arrows on it, put the playback there (D26).
function playerBar(session, listened, total, words, drawing) {
  if (!listened) {
    // Its audio freed while it played: nothing plays on.
    if (player?.name === nameOf(session)) {
      player.stop();
      player = null;
    }
    // Freed, said so; there and not playable, said so too — never one for the other.
    const freed = element("p", "note", session.audio > 0 ? words.not_playable : words.no_player);
    freed.id = "no-player";
    return freed;
  }
  const shown = listen(nameOf(session), listened);
  const bar = element("div", "player");
  const toggle = element("button", "open", "");
  toggle.type = "button";
  toggle.id = "play";
  const at = element("span", "at", "");
  at.id = "played";
  // A track the window could not play, said where a screen reader hears it.
  const says = element("p", "note", "");
  says.id = "not-played";
  says.setAttribute("role", "status");
  const head = element("span", "playhead");
  drawing.append(head);
  drawing.tabIndex = 0;
  drawing.setAttribute("role", "slider");
  drawing.setAttribute("aria-label", words.playback);
  drawing.setAttribute("aria-valuemin", "0");
  // The slider runs to the sound's end, once known: it may outlast the session's recorded length.
  const span = () => Math.max(total, shown.audio.duration || 0);
  const paint = (time) => {
    // Written only when it changes: playing, this runs ten times a second, and a press on words
    // written again under the pointer is no click in WebKitGTK.
    const said = shown.playing() ? words.pause : words.play;
    if (toggle.textContent !== said) toggle.textContent = said;
    at.textContent = clock(time);
    head.style.left = `${Math.min(100, (time / total) * 100)}%`;
    drawing.setAttribute("aria-valuemax", String(Number(span().toFixed(1))));
    drawing.setAttribute("aria-valuenow", String(Number(time.toFixed(1))));
    drawing.setAttribute("aria-valuetext", `${clock(time)} / ${clock(span())}`);
  };
  shown.listeners.clear();
  shown.listeners.add(paint);
  shown.failures.clear();
  shown.failures.add(() => (says.textContent = words.not_played));
  toggle.addEventListener("click", () => {
    if (shown.playing()) return shown.pause();
    says.textContent = "";
    shown.play();
  });
  drawing.addEventListener("click", (event) => {
    const box = drawing.getBoundingClientRect();
    shown.seek(((event.clientX - box.left) / box.width) * total);
  });
  // A slider's keys: five seconds back or on, the start, the end.
  drawing.addEventListener("keydown", (event) => {
    // From where the last key asked, not from where the player still is.
    const at = shown.wanted ?? shown.time();
    const to = { ArrowLeft: at - 5, ArrowDown: at - 5, ArrowRight: at + 5, ArrowUp: at + 5, PageDown: at - 30, PageUp: at + 30, Home: 0, End: span() }[
      event.key
    ];
    if (to === undefined) return;
    event.preventDefault();
    shown.seek(to);
  });
  paint(shown.time());
  bar.append(toggle, at, says);
  return bar;
}

// The space bar plays and pauses the meeting shown, wherever the focus is but in a field or on
// a control of its own; held down, it does not toggle again and again.
document.addEventListener("keydown", (event) => {
  if (event.key !== " " || !player || document.getElementById("meeting")?.hidden) return;
  if (event.target.closest("input, textarea, select, button, a, [role=button]")) return;
  event.preventDefault();
  if (event.repeat) return;
  player.playing() ? player.pause() : player.play();
});

function showMeeting({ session, transcript, minutes, developer, listen: listened, context, confidential, language, languages, meeting_language, apple, system_language }, words) {
  const page = document.getElementById("meeting");
  const top = element("div", "meeting-top");
  const open = element("button", "open", words.open_folder);
  open.type = "button";
  open.addEventListener("click", () => window.__TAURI__.core.invoke("open_folder", { name: nameOf(session) }).catch((why) => {
    open.replaceWith(element("p", "note warn", why));
  }));
  const actions = element("div", "actions");
  // In developer mode: this meeting's logs, to send to whoever looks into it — nothing of the
  // meeting is in them (D29).
  if (developer) {
    const copy = element("button", "open", words.copy_diagnostic);
    copy.type = "button";
    copy.id = "copy-diagnostic";
    // What became of it, said where a screen reader hears it.
    const copied = element("span", "says", "");
    copied.setAttribute("role", "status");
    copy.addEventListener("click", async () => {
      try {
        const lines = await window.__TAURI__.core.invoke("diagnostic", { name: nameOf(session) });
        await navigator.clipboard.writeText(lines);
        copied.textContent = lines ? words.diagnostic_copied : words.diagnostic_empty;
      } catch {
        copied.textContent = words.diagnostic_not_copied;
      }
    });
    actions.append(copied, copy);
  }
  actions.append(open);
  // The way back is the pane's, under 840 px alone (D61).
  top.append(actions);
  const title = element("section", "meeting-title");
  const named = element("div", "named");
  // Named by its minutes (D48): the date above the name.
  if (session.title) named.append(element("span", "on", heading(session, words)));
  named.append(meetingName(session, words));
  if (session.seconds !== null) named.append(element("span", "length", length(session.seconds)));
  // Where the minutes were written, from their own front matter: unread, or naming none, not said.
  const minutesHere = minutes.state === "present" && minutes.provider ? minutes.provider === "local" : undefined;
  title.append(named, holdings(session, words, { minutes_here: minutesHere }));
  const said = transcript.state === "present" ? transcript.segments : [];
  const total = session.seconds || said.reduce((end, s) => Math.max(end, s.end), 0) || 1;
  const drawn = element("section", "card drawn");
  const legend = element("div", "legend");
  legend.append(
    element("span", "here", words.here_described),
    element("span", "remote", words.remote_described),
    element("span", "range", `${clock(0)} – ${clock(total)}`),
  );
  const drawing = element("div", "drawing");
  drawing.append(relief(said, total, words));
  drawn.append(drawing, legend, playerBar(session, listened, total, words, drawing));
  const columns = element("div", "columns");
  columns.append(minutesCard(minutes, words, session, transcript), transcriptCard(transcript, words, session));
  // Shown again when the window comes forward: a search in progress in this meeting is kept.
  const searching = page.dataset.name === nameOf(session) ? (document.getElementById("search")?.value ?? "") : "";
  // A name being typed is kept the same way, with its refusal and where the caret was (D70).
  const naming = document.querySelector("#naming:not([data-saving])");
  const namingRefused = document.getElementById("naming-said")?.textContent ?? "";
  const same = page.dataset.name === nameOf(session);
  const typing = same && naming ? { value: naming.value, caret: naming.selectionStart, end: naming.selectionEnd, focused: document.activeElement === naming } : null;
  page.dataset.name = nameOf(session);
  const kept = confidentialCard(session, confidential, words);
  kept.append(languageRow(session, language, languages, words, meeting_language, apple, system_language));
  page.replaceChildren(top, title, kept, contextCard(session, { context, transcript, minutes }, words), drawn, columns);
  // The line being said, marked while it plays; the page stays where the user left it (D26, the
  // decider, 2026-10-24: following the line was intrusive).
  if (player?.name === nameOf(session)) {
    const lines = [...page.querySelectorAll(".lines li")];
    let marked = null;
    // The player it follows, not the page's: a stopped one still says its last pause, after the
    // page let it go.
    const followed = player;
    followed.listeners.add((at) => {
      const now = lines.findLast((line) => Number(line.dataset.start) <= at) ?? null;
      if (now === marked) return;
      marked?.classList.remove("now");
      now?.classList.add("now");
      marked = now;
    });
    followed.listeners.forEach((listener) => listener(followed.time()));
  }
  const search = document.getElementById("search");
  if (search && searching) {
    search.value = searching;
    search.dispatchEvent(new Event("input"));
  }
  if (typing) {
    const button = document.getElementById("rename");
    // Typed in, it takes the focus back; the user gone elsewhere, they are not brought back.
    nameField(button.parentElement, button, session, words, typing.value, namingRefused, typing.focused);
    if (typing.focused) document.getElementById("naming").setSelectionRange(typing.caret, typing.end);
  }
}

// A name refused while its field stays open, until the page is left.
let nameRefused = null;
// What the service's button last said, and whether it is still asking: Settings are drawn anew under
// it by a model come or the window coming back, the answer still to arrive (D72).
let serviceAnswer = null;

// A sentence in the pane, until the next address.
function paneSays(sentence) {
  const said = document.getElementById("pane-said");
  said.textContent = sentence;
  said.hidden = false;
}

// The meeting's heading (D70): clicked, a field. Enter or leaving it keeps what is typed, emptied
// its minutes' title or its date comes back; Escape leaves it as it was.
function meetingName(session, words) {
  const heading_ = element("h1");
  const button = element("button", "rename", session.title ?? heading(session, words));
  button.type = "button";
  button.id = "rename";
  button.title = words.rename_meeting;
  button.addEventListener("click", () => nameField(heading_, button, session, words, session.title ?? ""));
  heading_.append(button);
  return heading_;
}

function nameField(heading_, button, session, words, typed, refused = "", focused = true) {
  const field = element("input", "naming");
  field.type = "text";
  field.id = "naming";
  field.maxLength = 80;
  field.value = typed;
  field.placeholder = words.name_meeting;
  field.setAttribute("aria-label", words.name_meeting);
  const said = element("p", "says");
  said.id = "naming-said";
  said.setAttribute("role", "status");
  said.textContent = refused;
  said.hidden = refused === "";
  let done = false;
  const leave = () => {
    done = true;
    nameRefused = null;
    said.remove();
    field.replaceWith(button);
  };
  const keep = async () => {
    if (done) return;
    if (field.value.trim() === (session.title ?? "")) return leave();
    done = true;
    // Being saved, no longer being typed: a reading of the page meanwhile does not open it again.
    field.dataset.saving = "";
    // An earlier refusal is no longer the last word, nor a sentence for the next address.
    nameRefused = null;
    const asked = location.hash;
    try {
      await window.__TAURI__.core.invoke("title_saved", { name: nameOf(session), title: field.value });
      button.textContent = field.value.trim() || heading(session, words);
      leave();
      await refresh();
    } catch (why) {
      done = false;
      delete field.dataset.saving;
      // Its page left or drawn anew meanwhile: said where it stays, never to no one.
      if (!field.isConnected || location.hash !== asked) return paneSays(fill(words.name_not_saved, why));
      said.textContent = fill(words.name_not_saved, why);
      said.hidden = false;
      // The page left next, by the click that took the focus from here: the pane says it then.
      nameRefused = said.textContent;
    }
  };
  field.addEventListener("keydown", (event) => {
    if (event.key === "Enter") {
      event.preventDefault();
      keep().then(() => document.getElementById("rename")?.focus());
    } else if (event.key === "Escape") {
      event.preventDefault();
      leave();
      button.focus();
    }
  });
  // Another window come forward leaves the field where it was: only a click or a tab elsewhere keeps it.
  field.addEventListener("blur", () => {
    if (document.activeElement !== field) keep();
  });
  button.replaceWith(field);
  heading_.after(said);
  if (!focused) return;
  field.focus();
  field.select();
}

// The settings' page: each change saved at once; nothing offered over a file that cannot be read.
function choice(name, value, [label, says], checked, disabled) {
  const option = element("label", "choice");
  const input = element("input");
  input.type = "radio";
  input.name = name;
  input.value = value;
  input.checked = checked;
  input.disabled = disabled;
  const text = element("span", "said");
  text.append(element("span", "name", label), element("span", "says", says));
  option.append(input, text);
  return option;
}

// Who writes the minutes: picked from what Ollama holds, an online service, or the user's own
// address; a key goes to the system's keyring, and the page never receives one back.
let testing = false;

function minutesRow(minutes, words, locked) {
  const box = element("div", "step");
  const row = element("div", "row");
  const label = element("label", "name", words.minutes_step);
  label.htmlFor = "minutes";
  const pick = element("select");
  pick.id = "minutes";
  pick.disabled = locked;
  const add = (parent, value, text) => {
    const option = element("option", "", text);
    option.value = value;
    parent.append(option);
  };
  const online = minutes.online.findIndex((service) => service.address === minutes.provider);
  let current = "none";
  if (minutes.provider === minutes.ollama_address && minutes.ollama.includes(minutes.model)) current = `ollama:${minutes.model}`;
  else if (online >= 0) current = `online:${online}`;
  else if (minutes.provider) current = "custom";
  if (current === "none") add(pick, "none", words.not_chosen);
  if (minutes.ollama.length > 0) {
    const group = element("optgroup");
    group.label = words.ollama_group;
    // The one that holds the minutes' form, said so (#261): first, as the shell lists it.
    for (const name of minutes.ollama) add(group, `ollama:${name}`, name === minutes.recommended ? fill(words.model_recommended, name) : name);
    pick.append(group);
  }
  const groupOnline = element("optgroup");
  groupOnline.label = words.online_group;
  minutes.online.forEach((service, index) => add(groupOnline, `online:${index}`, service.name));
  const groupOther = element("optgroup");
  groupOther.label = words.other_group;
  add(groupOther, "custom", words.custom);
  pick.append(groupOnline, groupOther);
  pick.value = current;
  row.append(label, pick);
  const fields = element("div", "fields");
  const where = element("p", "status");
  where.id = "minutes-where";
  const field = (id, text, type, value) => {
    const wrap = element("label", "field");
    wrap.htmlFor = id;
    const input = element("input");
    input.id = id;
    input.type = type;
    input.value = value || "";
    input.defaultValue = input.value;
    input.disabled = locked;
    wrap.append(element("span", "name", text), input);
    return [wrap, input];
  };
  // Saved or not, the page is drawn again from what the shell holds; a failure stays said.
  const save = async (provider, model, key) => {
    let failed = null;
    try {
      const notKept = await window.__TAURI__.core.invoke("minutes_saved", { provider, model, key });
      if (notKept !== null) failed = fill(words.key_not_kept, notKept);
    } catch (why) {
      failed = fill(words.not_saved, why);
    }
    notSaved = failed;
    await refresh();
    savedSaid(failed === null);
  };
  // Chosen, not yet saved: who writes them meanwhile is said — going through the list, with the
  // keyboard's arrows too, never changes who does (D25).
  const unsaved = element("p", "says");
  unsaved.id = "minutes-unsaved";
  const inUse = minutes.provider && minutes.model ? modelSaid(minutes.model) : words.not_chosen;
  const draw = () => {
    fields.replaceChildren();
    const value = pick.value;
    unsaved.hidden = value === current;
    unsaved.textContent = minutes.provider && minutes.model ? fill(words.minutes_unsaved, inUse) : words.minutes_unsaved_none;
    if (value.startsWith("ollama:")) {
      where.className = "status here";
      where.textContent = words.on_this_computer;
      const button = element("button", "open", words.save);
      button.type = "button";
      button.id = "minutes-save";
      button.disabled = locked || value === current;
      button.addEventListener("click", () => save(minutes.ollama_address, value.slice("ollama:".length), null));
      fields.append(button);
      return;
    }
    if (value === "none") {
      where.textContent = "";
      return;
    }
    let shown = minutes.provider ?? "";
    try {
      const address = new URL(value === "custom" ? minutes.provider : minutes.online[Number(value.split(":")[1])].address);
      shown = `${address.host}${address.pathname}`;
    } catch {}
    // The address saved is this machine — Ollama with a model it does not list, or another server
    // here —: said as it is, nothing leaves; and a model Ollama does not have, named.
    const here = value === "custom" && value === current && minutes.provider_local;
    where.className = here ? "status here" : "status online";
    where.replaceChildren(here ? words.on_this_computer : words.where_online, element("code", "tag", shown));
    // Ollama answered without that model: named. Not answering, or with no model at all: said so —
    // never "the model is missing" on a list that could not be read.
    if (here && minutes.provider === minutes.ollama_address && minutes.model) {
      const answered = minutes.ollama.length > 0;
      if (!answered || !minutes.ollama.includes(minutes.model)) {
        const said = element("p", "says warn", answered ? fill(words.ollama_lacks, minutes.model) : words.ollama_silent);
        said.id = answered ? "minutes-lacking" : "minutes-silent";
        fields.append(said);
      }
    }
    const custom = value === "custom";
    const [addressWrap, address] = field("minutes-address", words.address, "url", custom && current === "custom" ? minutes.provider : "");
    const [modelWrap, model] = field("minutes-model", words.model_name, "text", current === value ? minutes.model : "");
    const [keyWrap, key] = field("minutes-key", words.key, "password", "");
    key.autocomplete = "off";
    if (minutes.key_kept && current === value) key.placeholder = words.key_kept;
    const button = element("button", "open", words.save);
    button.type = "button";
    button.id = "minutes-save";
    button.disabled = locked;
    button.addEventListener("click", () => {
      const provider = custom ? address.value : minutes.online[Number(value.split(":")[1])].address;
      save(provider, model.value, key.value || null);
    });
    if (custom) fields.append(addressWrap);
    fields.append(modelWrap, keyWrap, button);
  };
  // Tester sits on the key's row, as the board has it, or on its own row when there is none.
  let place = () => {};
  pick.addEventListener("change", () => {
    draw();
    place();
  });
  draw();
  box.append(row, where, fields, unsaved);
  // Tester: the choice saved, asked whether it answers — one sentence of Wholeheard's, said first.
  // One test at a time, the page drawn again meanwhile or not.
  if (minutes.provider && minutes.model) {
    const test = element("div", "row");
    const button = element("button", "open", words.test);
    button.type = "button";
    button.id = "minutes-test";
    button.disabled = locked || testing;
    const tested = element("p", "says");
    tested.id = "minutes-tested";
    tested.setAttribute("aria-live", "polite");
    let host = minutes.provider;
    try {
      host = new URL(minutes.provider).host;
    } catch {}
    tested.textContent = testing ? words.testing : fill(words.test_sends, host);
    button.dataset.locked = String(locked);
    button.addEventListener("click", async () => {
      testing = true;
      button.disabled = true;
      tested.textContent = words.testing;
      let said;
      try {
        await window.__TAURI__.core.invoke("minutes_tested");
        said = words.test_answered;
      } catch (why) {
        said = why;
      }
      testing = false;
      // Said on the page as it is now: it may have been drawn again meanwhile.
      const now = document.getElementById("minutes-tested");
      if (now) now.textContent = said;
      const again = document.getElementById("minutes-test");
      if (again) again.disabled = again.dataset.locked === "true";
    });
    test.append(tested);
    box.append(test);
    place = () => (fields.childElementCount > 0 ? fields.append(button) : test.prepend(button));
    place();
  }
  return box;
}

// A Whisper model's file named as people say it: `ggml-large-v3-turbo-q8_0.bin` is Whisper large
// v3 turbo. A file of another kind, or outside Wholeheard's folder, keeps its own name.
// A provider's model named as people say it: `anthropic/claude-sonnet` is Claude Sonnet; its
// version and size stay as written (`mistral:7b` is Mistral 7b).
function modelSaid(model) {
  // What people say of a model: the last part of its id, without what only machines read — a
  // file's extension, a tag "latest", a date of release, a quantisation (q4_K_M, fp16, qat…).
  let name = model.split("/").filter(Boolean).pop() ?? model;
  name = name.replace(/\.(gguf|bin)$/i, "").replace(/:latest$/i, "");
  name = name.replace(/[-_:.](?:i?q\d(?:_[a-z0-9]+)*|b?fp?16|fp?32|int[48])$/i, "");
  name = name.replace(/[-_:](?:\d{8}|\d{4}-\d{2}-\d{2}|\d{2}-\d{4})$/, "");
  const machine = /^(?:i?q\d(?:_[a-z0-9]+)*|b?fp?16|fp?32|int[48]|qat|k|m|s|l)$/i;
  const words = name.split(/[-_:\s]+/).filter(Boolean);
  while (words.length > 1 && machine.test(words.at(-1))) words.pop();
  if (words.length === 0 || (words.length === 1 && machine.test(words[0]))) return model;
  const said = [];
  for (const word of words) {
    // A version written with dashes — claude-sonnet-4-5 — is said with its point.
    if (/^\d{1,2}$/.test(word) && /^\d{1,2}(\.\d{1,2})*$/.test(said.at(-1) ?? "")) said[said.length - 1] += `.${word}`;
    else said.push(word.toLowerCase() === "gpt" ? "GPT" : /^[a-z]/.test(word) ? word[0].toUpperCase() + word.slice(1) : word);
  }
  return said.join(" ");
}

function spoken(name) {
  const file = name.match(/^ggml-(.+?)(?:[-.](?:q\d+_\d+|f16|f32))?(?:\.bin)?$/i);
  return file ? `Whisper ${file[1].replace(/[-_]/g, " ")}` : name;
}

function showSettings(shown, minutes, words) {
  const page = document.getElementById("settings");
  const locked = shown.unreadable !== null;
  // Saved or not, the page is drawn again from what the shell holds; a failure stays said.
  const save = async (change) => {
    let failed = null;
    try {
      await window.__TAURI__.core.invoke("settings_saved", { change });
    } catch (why) {
      failed = fill(words.not_saved, why);
    }
    notSaved = failed;
    await refresh();
    savedSaid(failed === null);
  };
  const parts = [];
  if (locked) {
    const unreadable = element("p", "card note warn", shown.unreadable);
    unreadable.id = "settings-unreadable";
    parts.push(unreadable);
  }
  const failed = element("p", "card note warn");
  failed.id = "not-saved";
  // What the last save could not do, said on the page drawn after it — once.
  failed.textContent = notSaved ?? "";
  failed.hidden = notSaved === null;
  notSaved = null;
  parts.push(failed);

  const after = element("section", "card group");
  after.setAttribute("role", "radiogroup");
  const afterTitle = element("h2", "", words.after_title);
  afterTitle.id = "after-title";
  after.setAttribute("aria-labelledby", "after-title");
  after.append(afterTitle);
  for (const [value, said] of [["nothing", words.after_nothing], ["transcript", words.after_transcript], ["minutes", words.after_minutes]]) {
    const one = choice("after", value, said, shown.after === value, locked);
    one.querySelector("input").addEventListener("change", () => save({ after: value }));
    after.append(one);
  }
  parts.push(after);

  const models = element("section", "card group");
  models.append(element("h2", "", words.models_title), element("p", "says", words.models_intro));
  // The first run's assistant again, the current setting chosen (D58).
  const redo = element("div", "row");
  const redoSaid = element("span", "said");
  redoSaid.append(element("span", "name", words.welcome_again[0]), element("span", "says", words.welcome_again[1]));
  const redoButton = element("button", "open", words.welcome_again[0]);
  redoButton.type = "button";
  redoButton.id = "welcome-again";
  redoButton.disabled = locked;
  redoButton.addEventListener("click", () => (location.hash = "#/welcome/again"));
  redo.append(redoSaid, redoButton);
  models.append(redo);
  const step = element("div", "step");
  const row = element("div", "row");
  const label = element("label", "name", words.transcription);
  label.htmlFor = "model";
  const pick = element("select");
  pick.id = "model";
  pick.disabled = locked;
  // The model chosen, not here yet: said so in the choice, never shown as another (D37).
  if (!shown.models.some((model) => model.path === shown.model)) {
    const missing = element("option", "", fill(words.model_chosen_missing, spoken(shown.model.split("/").pop())));
    missing.disabled = true;
    missing.selected = true;
    pick.append(missing);
  }
  for (const model of shown.models) {
    const option = element("option", "", `${spoken(model.name)} — ${fill(words.installed, size(model.bytes, words))}`);
    option.value = model.path;
    option.selected = model.path === shown.model;
    pick.append(option);
  }
  // Apple's SpeechAnalyzer among who hears, where this Mac offers it (D42): asked away from the
  // page; its model fetched by macOS on the click alone, then it is who hears.
  const speech = element("div", "speech");
  speech.id = "speech";
  const speechSaid = element("p", "says");
  speechSaid.setAttribute("role", "status");
  const fetchApple = element("button", "open", words.speech_fetch);
  fetchApple.type = "button";
  fetchApple.id = "speech-fetch";
  fetchApple.hidden = true;
  fetchApple.addEventListener("click", async () => {
    fetchApple.disabled = true;
    speechSaid.textContent = words.speech_fetching;
    const fetched = await window.__TAURI__.core.invoke("speech_installed").then(() => true, () => false);
    // Chosen only while it is still the user's choice: Settings may have been drawn again meanwhile.
    if (fetched && document.getElementById("model")?.value === "speechanalyzer") return save({ engine: "speechanalyzer" });
    if (fetched) return;
    fetchApple.disabled = false;
    speechSaid.textContent = words.speech_failed;
    // Not fetched: who hears is still the Whisper model, and the choice says so.
    if (shown.models.some((model) => model.path === shown.model)) pick.value = shown.model;
  });
  speech.append(fetchApple, speechSaid);
  let appleHere = false;
  window.__TAURI__.core.invoke("speech_shown").then((status) => {
    if (!status?.offered) return;
    appleHere = status.installed;
    const option = element("option", "", status.installed ? words.speech_option : `${words.speech_option} — ${words.speech_missing}`);
    option.value = "speechanalyzer";
    option.selected = shown.engine === "speechanalyzer";
    pick.append(option);
  }).catch(() => {});
  // A service that hears, as the minutes' (D43): its fields under the choice, nothing sent before
  // the user agreed to its host; a host on this computer needs no agreement.
  const groupService = element("optgroup");
  groupService.label = words.service_group;
  // The services that hear, offered by name as the minutes' are (D63); any other, an address of the user's.
  const presetOf = (text) => shown.hearing.findIndex((hearing) => hearing.address === text);
  const preset = shown.engine === "service" ? presetOf(shown.transcription_provider) : -1;
  shown.hearing.forEach((hearing, index) => {
    const option = element("option", "", hearing.name);
    option.value = `service:${index}`;
    option.selected = preset === index;
    groupService.append(option);
  });
  const serviceOption = element("option", "", words.service_option);
  serviceOption.value = "service";
  serviceOption.selected = shown.engine === "service" && preset < 0;
  groupService.append(serviceOption);
  pick.append(groupService);
  const service = element("div", "fields");
  service.id = "transcription-service";
  service.hidden = shown.engine !== "service";
  const serviceField = (id, text, type, value) => {
    const wrap = element("label", "field");
    wrap.htmlFor = id;
    const input = element("input");
    input.id = id;
    input.type = type;
    input.value = value || "";
    input.defaultValue = input.value;
    input.disabled = locked;
    wrap.append(element("span", "name", text), input);
    service.append(wrap);
    return input;
  };
  const address = serviceField("transcription-provider", words.service_address, "url", shown.transcription_provider);
  const serviceModel = serviceField("transcription-model", words.service_model, "text", shown.transcription_model);
  const serviceKey = serviceField("transcription-key", words.service_key, "password", "");
  // A service offered by name: its models to choose from, another typed (D63).
  const serviceModels = element("select");
  serviceModels.id = "transcription-models";
  serviceModels.disabled = locked;
  const modelsWrap = element("label", "field");
  modelsWrap.htmlFor = serviceModels.id;
  modelsWrap.append(element("span", "name", words.service_model), serviceModels);
  serviceModel.parentElement.before(modelsWrap);
  const modelsRefused = element("p", "note warn");
  modelsRefused.id = "transcription-models-refused";
  modelsRefused.setAttribute("role", "status");
  modelsRefused.hidden = true;
  modelsWrap.after(modelsRefused);
  const showModel = () => {
    const named = pick.value.startsWith("service:");
    modelsWrap.hidden = !named;
    serviceModel.parentElement.hidden = named && serviceModels.value !== "";
    if (named && serviceModels.value !== "") serviceModel.value = serviceModels.value;
  };
  const offerModels = async (index) => {
    // Refused: an empty list, and said; the model can still be typed.
    let listed = [];
    let refused = "";
    try {
      listed = await window.__TAURI__.core.invoke("hearing_models", { index });
    } catch (why) {
      refused = fill(words.hearing_models_refused, why);
    }
    // Another chosen meanwhile: its own list comes.
    if (pick.value !== `service:${index}`) return;
    modelsRefused.textContent = refused;
    modelsRefused.hidden = refused === "";
    // The model saved for this service, if it is the one saved; another service starts from its list.
    const typed = index === preset ? (shown.transcription_model ?? "").trim() : "";
    serviceModel.value = typed;
    serviceModels.replaceChildren(...listed.map((name) => {
      const option = element("option", "", name);
      option.value = name;
      return option;
    }));
    const other = element("option", "", words.service_other_model);
    other.value = "";
    serviceModels.append(other);
    serviceModels.value = listed.includes(typed) ? typed : typed ? "" : listed[0] ?? "";
    showModel();
  };
  serviceModels.addEventListener("change", () => {
    if (serviceModels.value === "") serviceModel.value = "";
    showModel();
    if (serviceModels.value === "") serviceModel.focus();
  });
  const agree = element("label", "row");
  const agreeBox = element("input");
  agreeBox.type = "checkbox";
  agreeBox.id = "transcription-agreed";
  // A box ticked is no text being typed: the page still comes back when the window does.
  agreeBox.defaultValue = agreeBox.value;
  agreeBox.disabled = locked;
  const agreeSaid = element("span", "says");
  const hostOf = (text) => { try { return new URL(text).host; } catch { return ""; } };
  const sayAgree = () => (agreeSaid.textContent = fill(words.service_agree, hostOf(address.value) || "—"));
  agreeBox.checked = !!shown.transcription_agreed && shown.transcription_agreed === hostOf(address.value);
  address.addEventListener("input", () => { sayAgree(); agreeBox.checked = false; });
  sayAgree();
  // A service offered by name fills its address; one of the user's shows the field. The agreement
  // ticked only for the host already agreed to.
  const chooseService = () => {
    const index = pick.value.startsWith("service:") ? Number(pick.value.split(":")[1]) : -1;
    if (index >= 0) address.value = shown.hearing[index].address;
    else if (presetOf(address.value) >= 0) address.value = preset < 0 ? shown.transcription_provider || "" : "";
    address.parentElement.hidden = index >= 0;
    agreeBox.checked = !!shown.transcription_agreed && shown.transcription_agreed === hostOf(address.value);
    sayAgree();
    if (index < 0) return showModel();
    // Its list coming: nothing of another service's shown meanwhile, the list said when it comes.
    serviceModels.replaceChildren();
    serviceModel.value = index === preset ? shown.transcription_model ?? "" : "";
    modelsWrap.hidden = false;
    serviceModel.parentElement.hidden = true;
    offerModels(index);
  };
  agree.append(agreeBox, agreeSaid);
  const serviceSaid = element("p", "says");
  serviceSaid.id = "transcription-said";
  serviceSaid.setAttribute("role", "status");
  const saveService = element("button", "primary", words.service_save);
  saveService.type = "button";
  saveService.id = "transcription-save";
  saveService.disabled = locked;
  // This computer's own host needs no agreement: nothing of the meeting leaves it.
  // The whole host, its port aside: `localhost.example` or `127.example` is elsewhere. The shell
  // judges again (`providers::local`); this only spares the agreement where none is needed.
  const isHere = (host) => /^(localhost|127(\.\d{1,3}){3}|\[::1\])(:\d+)?$/i.test(host);
  saveService.addEventListener("click", async () => {
    const { invoke } = window.__TAURI__.core;
    if (!/^https?:$/.test((() => { try { return new URL(address.value.trim()).protocol; } catch { return ""; } })()) || !hostOf(address.value.trim())) {
      serviceSaid.textContent = words.service_address_bad;
      return address.focus();
    }
    if (!serviceModel.value.trim()) {
      serviceSaid.textContent = words.service_model_named;
      return serviceModel.focus();
    }
    if (!agreeBox.checked && !isHere(hostOf(address.value))) {
      serviceSaid.textContent = fill(words.service_agree_first, hostOf(address.value) || "—");
      return agreeBox.focus();
    }
    const saved = await invoke("transcription_saved", {
      provider: address.value, model: serviceModel.value, key: serviceKey.value || null, agreed: agreeBox.checked,
    }).then((notKept) => (notKept === null ? fill(words.service_saved, hostOf(address.value)) : fill(words.key_not_kept, notKept)), (why) => fill(words.not_saved, why));
    // A Tester still asking stays: its answer comes after this, the button its own.
    serviceAnswer ??= { said: "", asking: false };
    serviceAnswer.said = saved;
    serviceSaid.textContent = saved;
  });
  const testService = element("button", "open", words.service_test);
  testService.type = "button";
  testService.id = "transcription-test";
  testService.disabled = locked || serviceAnswer?.asking === true;
  serviceSaid.textContent = serviceAnswer?.said ?? "";
  testService.addEventListener("click", async () => {
    const mine = (serviceAnswer = { said: "", asking: true });
    testService.disabled = true;
    serviceSaid.textContent = "";
    // The service asked, as saved: an answer for one no longer saved is no answer for what is.
    const asking = async () => {
      const { transcription_provider, transcription_model } = await window.__TAURI__.core.invoke("settings_shown");
      return JSON.stringify([transcription_provider, transcription_model]);
    };
    try {
      const asked = await asking();
      let answer;
      try {
        await window.__TAURI__.core.invoke("transcription_tested");
        answer = words.service_tested;
      } catch (why) {
        answer = why;
      }
      if ((await asking()) === asked) mine.said = answer;
    } catch (why) {
      mine.said = why;
    }
    mine.asking = false;
    // Said on the page as it is now; another address or another service: nothing to say any more.
    if (serviceAnswer !== mine) return;
    const said = document.getElementById("transcription-said");
    if (said) said.textContent = mine.said;
    const button = document.getElementById("transcription-test");
    if (button) button.disabled = locked;
  });
  service.append(agree, saveService, testService, serviceSaid);
  if (pick.value.startsWith("service")) chooseService();
  pick.addEventListener("change", () => {
    serviceAnswer = null;
    serviceSaid.textContent = "";
    testService.disabled = locked;
    service.hidden = !pick.value.startsWith("service");
    if (pick.value.startsWith("service")) {
      chooseService();
      return (pick.value === "service" ? address : serviceModels).focus();
    }
    if (pick.value !== "speechanalyzer") return save({ model: pick.value });
    if (appleHere) return save({ engine: "speechanalyzer" });
    // Not chosen until its model is on the Mac: the gesture comes first, never the download.
    fetchApple.hidden = false;
    fetchApple.focus();
  });
  row.append(label, pick);
  const here = element("p", "status here", words.on_this_computer);
  here.append(element("code", "tag", words.builtin));
  // The language meetings are spoken in, unless one says its own (D65): detected by default.
  const spokenRow = element("div", "row");
  const spokenLabel = element("label", "name", words.meetings_language);
  spokenLabel.htmlFor = "meetings-language";
  const spokenPick = element("select");
  spokenPick.id = "meetings-language";
  spokenPick.disabled = locked;
  const spokenAdd = (value, text) => {
    const option = element("option", "", text);
    option.value = value;
    spokenPick.append(option);
  };
  // SpeechAnalyzer hears French and English and detects none: those two, or the system's (D64).
  const apple = shown.engine === "speechanalyzer";
  const heard = shown.languages.filter(([code]) => !apple || code === "fr" || code === "en");
  spokenAdd("", apple ? words.language_as_system : words.language_detected);
  for (const [code, said] of heard) spokenAdd(code, said);
  // One it does not hear, chosen while Whisper heard: kept, said as heard (D65).
  const spokenChosen = shown.meeting_language;
  const spokenName = shown.languages.find(([code]) => code === spokenChosen)?.[1] ?? spokenChosen;
  if (spokenChosen && !heard.some(([code]) => code === spokenChosen)) {
    spokenAdd(spokenChosen, apple ? fill(words.language_heard_as, spokenName, words.language_as_system.toLowerCase()) : spokenName);
  }
  spokenPick.value = shown.meeting_language ?? "";
  spokenPick.addEventListener("change", () => save({ meeting_language: spokenPick.value }));
  spokenRow.append(spokenLabel, spokenPick);
  step.append(row, speech, service, here, spokenRow);
  // The short list of models to download, each said by what it is good for (D37).
  const offered = element("ul", "offered");
  offered.id = "offered";
  offered.setAttribute("aria-label", words.offered_title);
  for (const model of shown.offered) {
    const item = element("li", "row");
    item.dataset.offered = model.file;
    item.dataset.here = String(model.here);
    const said_ = element("div", "said");
    // Its host shown before anything is fetched from it (security rule 1, D40).
    said_.append(element("span", "name", `${spoken(model.file)} · ${size(model.bytes, words)}, ${model.host}`));
    if (model.good) said_.append(element("span", "says", model.good));
    item.append(said_, element("span", "state"));
    offered.append(item);
  }
  // The minutes' recommended model, fetched by Ollama — never Ollama itself (D37).
  const pulled = element("li", "row");
  pulled.dataset.offered = minutes.recommended;
  pulled.dataset.here = String(minutes.ollama.includes(minutes.recommended));
  pulled.dataset.by = minutes.ollama_answers ? "ollama" : "absent";
  const pulledSaid = element("div", "said");
  pulledSaid.append(element("span", "name", `${minutes.recommended} · ${words.offered_ollama_size}`), element("span", "says", words.offered_ollama_good));
  pulled.append(pulledSaid, element("span", "state"));
  offered.append(pulled);
  // The list read again from where it is kept, only on this click, its host shown (D40): read,
  // the page is drawn with it; refused, the list above stays, and it is said. Until the key that
  // signs it is built in, no click is offered, and that is said (D56).
  const refreshed = element("p", "says");
  refreshed.id = "offered-refreshed";
  refreshed.setAttribute("role", "status");
  const refresh_ = element("button", "open", fill(words.offered_refresh, shown.registry_host));
  refresh_.type = "button";
  refresh_.id = "offered-refresh";
  refresh_.addEventListener("click", async () => {
    refresh_.disabled = true;
    refreshed.textContent = "";
    const read = await window.__TAURI__.core.invoke("registry_refresh").then(() => true, () => false);
    if (read) return refresh();
    refresh_.disabled = false;
    refreshed.textContent = words.offered_refresh_failed;
  });
  if (shown.registry_waits) refreshed.textContent = words.offered_refresh_waits;
  step.append(element("h3", "", words.offered_title), offered, ...(shown.registry_waits ? [] : [refresh_]), refreshed);
  paintOffered(offered);
  models.append(step, minutesRow(minutes, words, locked), element("p", "says", words.models_note));
  parts.push(models);

  const more = element("section", "card group");
  const remind = element("label", "row");
  const box = element("input");
  box.type = "checkbox";
  box.id = "reminder";
  box.setAttribute("role", "switch");
  box.checked = shown.reminder;
  box.disabled = locked;
  box.addEventListener("change", () => save({ reminder: box.checked }));
  const remindSaid = element("span", "said");
  remindSaid.append(element("span", "name", words.reminder[0]), element("span", "says", words.reminder[1]));
  remind.append(remindSaid, box);
  // When another application takes the microphone (D30), as board 4 has it: offer to record,
  // or do nothing.
  const offer = element("div", "row");
  const offerSaid = element("label", "said");
  offerSaid.htmlFor = "offer";
  offerSaid.append(element("span", "name", words.offer[0]), element("span", "says", words.offer[1]));
  const offerPick = element("select");
  offerPick.id = "offer";
  offerPick.disabled = locked;
  for (const [value, text] of [["offer", words.offer_choices[0]], ["nothing", words.offer_choices[1]]]) {
    const option = element("option", "", text);
    option.value = value;
    offerPick.append(option);
  }
  offerPick.value = shown.offer ? "offer" : "nothing";
  offerPick.addEventListener("change", () => save({ offer: offerPick.value === "offer" }));
  offer.append(offerSaid, offerPick);
  // Started with the user's session (D32): the desktop's own entry, turned on or off here.
  const start = element("label", "row");
  const startBox = element("input");
  startBox.type = "checkbox";
  startBox.id = "autostart";
  startBox.setAttribute("role", "switch");
  startBox.checked = shown.autostart;
  startBox.disabled = locked;
  startBox.addEventListener("change", () => save({ autostart: startBox.checked }));
  const startSaid = element("span", "said");
  startSaid.append(element("span", "name", words.autostart[0]), element("span", "says", words.autostart[1]));
  start.append(startSaid, startBox);
  const keep = element("div", "row");
  const keepSaid = element("label", "said");
  keepSaid.htmlFor = "keep";
  keepSaid.append(element("span", "name", words.keep_audio[0]), element("span", "says", words.keep_audio[1]));
  const keepPick = element("select");
  keepPick.id = "keep";
  keepPick.disabled = locked;
  const days = new Set([7, 30, 90, 365]);
  if (shown.keep_audio !== "always") days.add(shown.keep_audio.days);
  const options = [
    ["always", words.keep_always],
    ...[...days].sort((a, b) => a - b).map((n) => [String(n), fill(words.keep_days, n)]),
    ["other", words.keep_other],
  ];
  for (const [value, text] of options) {
    const option = element("option", "", text);
    option.value = value;
    option.selected = shown.keep_audio === "always" ? value === "always" : value === String(shown.keep_audio.days);
    keepPick.append(option);
  }
  // Any number of days the file allows, typed when the list has not the one wanted.
  const other = element("input");
  other.id = "keep-other";
  other.type = "number";
  other.min = "1";
  other.max = "36500";
  other.hidden = true;
  other.disabled = locked;
  other.setAttribute("aria-label", words.keep_other);
  other.addEventListener("change", () => {
    const days = Number(other.value);
    if (other.value !== "" && Number.isInteger(days) && days >= 1 && days <= 36500) return save({ keep_audio: { days } });
    const said = document.getElementById("not-saved");
    said.textContent = fill(words.not_saved, fill(words.keep_not_days, other.value || "—"));
    said.hidden = false;
  });
  other.addEventListener("keydown", (event) => {
    if (event.key === "Enter") other.blur();
  });
  keepPick.addEventListener("change", () => {
    if (keepPick.value === "other") {
      other.hidden = false;
      other.focus();
      return;
    }
    save({ keep_audio: keepPick.value === "always" ? "always" : { days: Number(keepPick.value) } });
  });
  const keepPicks = element("div", "picks");
  keepPicks.append(keepPick, other);
  keep.append(keepSaid, keepPicks);
  // Where the sessions go: typed, or chosen in the system's own dialog.
  const where = element("div", "row folder");
  const whereSaid = element("label", "said");
  whereSaid.htmlFor = "sessions";
  whereSaid.append(element("span", "name", words.sessions_folder[0]), element("span", "says", words.sessions_folder[1]));
  const path = element("input");
  path.id = "sessions";
  path.type = "text";
  path.value = shown.sessions_said;
  path.disabled = locked;
  path.spellcheck = false;
  path.hidden = true;
  // Shown as the board says it — Dossier personnel / Wholeheard —; a click makes it a path to type.
  const said = shown.sessions_said.replace(/^~(?=\/|$)/, words.home).split("/").filter(Boolean).join(" / ");
  const folderShown = element("button", "path", shown.sessions_said.startsWith("/") ? `/ ${said}` : said);
  folderShown.type = "button";
  whereSaid.id = "sessions-label";
  folderShown.setAttribute("aria-labelledby", "sessions-label sessions-shown");
  folderShown.id = "sessions-shown";
  folderShown.disabled = locked;
  folderShown.addEventListener("click", () => {
    folderShown.hidden = true;
    path.hidden = false;
    path.focus();
    path.select();
  });
  path.addEventListener("change", () => save({ sessions: path.value.trim() }));
  // Enter says the folder is typed: the field lets go, which is its change.
  path.addEventListener("keydown", (event) => {
    if (event.key === "Enter") path.blur();
  });
  const choose = element("button", "open", words.choose);
  choose.type = "button";
  choose.id = "choose-sessions";
  choose.disabled = locked;
  choose.addEventListener("click", async () => {
    const chosen = await window.__TAURI__.core.invoke("choose_folder");
    if (chosen !== null) save({ sessions: chosen });
  });
  where.append(whereSaid, folderShown, path, choose);
  // The shortcut: the desktop's to grant and to change; the page says which keys, and where.
  const keys = element("div", "row");
  const keysSaid = element("span", "said");
  keysSaid.append(element("span", "name", words.shortcut[0]), element("span", "says", `${words.shortcut[1]} ${words.shortcut_where}`));
  const granted = element("kbd", "keys", shown.shortcut ?? words.shortcut_none);
  granted.id = "shortcut";
  keys.append(keysSaid, granted);
  if (shown.shortcut_settings) {
    const open = element("button", "open", words.shortcut_open);
    open.type = "button";
    open.id = "shortcut-settings";
    open.addEventListener("click", () =>
      window.__TAURI__.core.invoke("shortcut_settings").catch((why) => {
        keysSaid.lastChild.textContent = why;
      }),
    );
    keys.append(open);
  }
  // The language Wholeheard speaks (D64): the system's by default, each other named in its own.
  const speaks = element("div", "row");
  const speaksSaid = element("label", "said");
  speaksSaid.htmlFor = "app-language";
  speaksSaid.append(element("span", "name", words.app_language[0]), element("span", "says", words.app_language[1]));
  const speaksPick = element("select");
  speaksPick.id = "app-language";
  speaksPick.disabled = locked;
  for (const [value, text] of [["", words.app_language_system], ...shown.languages.filter(([code]) => code === "fr" || code === "en")]) {
    const option = element("option", "", text);
    option.value = value;
    speaksPick.append(option);
  }
  speaksPick.value = shown.language ?? "";
  speaksPick.addEventListener("change", () => {
    // Drawn again in the language chosen: the choice keeps the focus.
    focusLanguage = true;
    save({ language: speaksPick.value });
  });
  speaks.append(speaksSaid, speaksPick);
  more.append(speaks, remind, offer, start, keys, where, keep);
  parts.push(more);

  // The transcription model's state: there and ready; else said, and offered, by paintModel.
  const chosen = shown.models.find((model) => model.path === shown.model);
  const state = element("section", "card group state");
  const stateSaid = element("div", "said");
  stateSaid.append(
    element("span", "name", words.model_state),
    element("span", "says", chosen ? fill(words.model_here, size(chosen.bytes, words)) : ""),
  );
  state.append(stateSaid);
  if (chosen) state.append(readySaid(words.model_ready));
  else state.dataset.missing = "true";
  state.id = "model-state";
  paintModel(state);
  parts.push(state);

  // Last, for whoever looks into a failure: the developer mode (D29).
  const advanced = element("section", "card group");
  advanced.append(element("h2", "", words.advanced_title));
  const developer = element("label", "row");
  const developerBox = element("input");
  developerBox.type = "checkbox";
  developerBox.id = "developer";
  developerBox.setAttribute("role", "switch");
  developerBox.checked = shown.developer;
  developerBox.disabled = locked;
  developerBox.addEventListener("change", () => save({ developer: developerBox.checked }));
  const developerSaid = element("span", "said");
  developerSaid.append(element("span", "name", words.developer[0]), element("span", "says", words.developer[1]));
  developer.append(developerSaid, developerBox);
  advanced.append(developer);
  if (shown.developer) {
    // The logs' level, read at Wholeheard's next start by the shell and its programs.
    const level = element("div", "row");
    const levelSaid = element("label", "said");
    levelSaid.htmlFor = "log-level";
    levelSaid.append(element("span", "name", words.log_level[0]), element("span", "says", words.log_level[1]));
    const levelPick = element("select");
    levelPick.id = "log-level";
    levelPick.disabled = locked;
    for (const [value, text] of [["info", words.log_info], ["debug", words.log_debug]]) {
      const option = element("option", "", text);
      option.value = value;
      option.selected = shown.log === value;
      levelPick.append(option);
    }
    levelPick.addEventListener("change", () => save({ log: levelPick.value }));
    level.append(levelSaid, levelPick);
    const logs = element("div", "row");
    const logsSaid = element("span", "said");
    logsSaid.setAttribute("role", "status");
    logsSaid.append(element("span", "name", words.logs[0]), element("span", "says", words.logs[1]));
    const openLogs = element("button", "open", words.open_logs);
    openLogs.type = "button";
    openLogs.id = "open-logs";
    openLogs.addEventListener("click", () =>
      window.__TAURI__.core.invoke("open_logs").catch(() => {
        logsSaid.lastChild.textContent = words.logs_not_opened;
      }),
    );
    logs.append(logsSaid, openLogs);
    advanced.append(level, logs);
  }
  parts.push(advanced);
  page.replaceChildren(...parts);
  if (focusLanguage) {
    focusLanguage = false;
    document.getElementById("app-language")?.focus();
  }
  // From the assistant's « Choisir mon service »: the minutes' choice, in view and focused.
  if (focusMinutes) {
    focusMinutes = false;
    const minutesPick = document.getElementById("minutes");
    minutesPick?.scrollIntoView({ block: "center" });
    minutesPick?.focus();
  }
}

// Before a meeting first leaves for a host (D11's fifth board): what leaves, what stays, to
// whom; the user's word — once, or for this service —; or the minutes written on this computer.
// Nothing here is sent before a click.
function showAgreement(name, agreement, words) {
  const page = document.getElementById("agree");
  const card = element("section", "card agreement");
  card.append(element("p", "eyebrow", words.agree_eyebrow), element("h1", "", words.agree_title), element("p", "says", words.agree_lead));
  const goes = element("ul", "goes");
  const item = (mark, [title, says], ...values) => {
    const line = element("li", mark);
    const said = element("div", "");
    said.append(element("strong", "", title), element("span", "says", fill(says, ...values)));
    const sign = element("span", "sign");
    sign.append(icon(mark === "leaves" ? "arrow-right" : "x"));
    line.append(sign, said);
    return line;
  };
  goes.append(item("leaves", words.agree_leaves, Math.max(1, Math.round(agreement.seconds / 60)), agreement.words.toLocaleString(document.documentElement.lang)));
  if (agreement.context_words !== null) goes.append(item("leaves", words.agree_context, agreement.context_words.toLocaleString(document.documentElement.lang)));
  goes.append(item("stays", words.agree_stays));
  const to = element("dl", "recipient");
  to.append(element("dt", "", words.agree_to), element("dd", "", agreement.host), element("dt", "", words.agree_model), element("dd", "", agreement.model));
  const remember = element("label", "remember");
  const tick = element("input");
  tick.type = "checkbox";
  tick.id = "agree-remember";
  remember.append(tick, element("span", "", words.agree_remember));
  const failed = element("p", "note warn");
  failed.id = "not-saved";
  failed.hidden = true;
  const act = async (command, args) => {
    try {
      await window.__TAURI__.core.invoke(command, { name, ...args });
      location.hash = `#/meeting/${encodeURIComponent(name)}`;
    } catch (why) {
      failed.textContent = fill(words.not_saved, why);
      failed.hidden = false;
    }
  };
  const choices = element("div", "choices");
  const send = element("button", "primary", words.agree_send);
  send.type = "button";
  send.id = "agree-send";
  send.addEventListener("click", () => act("agreed", { host: agreement.host, remember: tick.checked }));
  const not = element("a", "open", words.agree_not);
  not.href = "#/";
  not.id = "agree-not";
  choices.append(send, not);
  card.append(goes, to, remember, choices, failed);
  if (agreement.ollama.length > 0) {
    const instead = element("button", "link", words.agree_local);
    instead.type = "button";
    instead.id = "agree-local-open";
    const here = element("div", "choices");
    here.hidden = true;
    const pick = element("select");
    pick.id = "agree-local";
    for (const model of agreement.ollama) {
      const option = element("option", "", model);
      option.value = model;
      pick.append(option);
    }
    const write = element("button", "open", words.agree_local_write);
    write.type = "button";
    write.id = "agree-local-write";
    write.addEventListener("click", () => act("local_instead", { model: pick.value }));
    here.append(pick, write);
    instead.addEventListener("click", () => {
      here.hidden = false;
      pick.focus();
    });
    card.append(instead, here);
  }
  page.replaceChildren(card);
  send.focus();
}

// The small window (D35): at rest — the recording's gesture and the shortcut, the last meeting and
// where it stands, what the meetings weigh, the way to the full window —, or while recording
// (D11's sixth board): the time, the two levels, the way to stop. The recording asked ten times a
// second; at rest, the meetings as often as the full window asks.
function showSmall(words) {
  const page = document.getElementById("small");
  document.body.classList.add("small-window");
  const invoke = (...asked) => window.__TAURI__.core.invoke(...asked);
  const full = (where) => invoke("full_window", { page: where ?? null });
  const toFull = (id) => {
    const button = element("button", "open icon");
    button.append(icon("maximize-2"));
    button.type = "button";
    button.id = id;
    button.title = words.small_full;
    button.setAttribute("aria-label", words.small_full);
    button.addEventListener("click", () => full());
    return button;
  };

  const rest = element("div", "small-rest");
  const restTop = element("div", "small-top");
  restTop.append(element("span", "small-brand", "Wholeheard"), toFull("small-full"));
  // Its new state at once, disabled until the recording's own says otherwise (D68). Answered, a press
  // the shell took for the same one, or a start that failed, gives the button back.
  let unanswered = false;
  const asking = async (button, said, wanted, back) => {
    button.disabled = true;
    button.textContent = said;
    unanswered = true;
    await window.__TAURI__.core.invoke("record");
    unanswered = false;
    const header = await window.__TAURI__.core.invoke("header");
    if (header.recording !== wanted) {
      button.disabled = false;
      button.textContent = back;
    }
  };
  const start = element("button", "primary", words.record_start);
  start.type = "button";
  start.id = "small-record";
  start.addEventListener("click", () => asking(start, words.record_starting, true, words.record_start));
  const keys = element("button", "hint");
  keys.type = "button";
  keys.id = "small-keys";
  const last = element("div", "small-last");
  const foot = element("div", "small-foot");
  rest.append(restTop, start, keys, last, foot);

  const live = element("div", "small-live");
  const time = element("span", "time", "00:00:00");
  const top = element("div", "small-top");
  top.append(element("span", "live-dot"), time, toFull("small-full-live"));
  // Each level under what it is recorded from, as the system names it (D33); before that name, a
  // check that turns ✓ when the side is heard, then fades; under it, what a silence asks (D49).
  const checks = [];
  const meter = (label, id, device, says) => {
    const row = element("div", "meter");
    const bar = element("span", "track");
    bar.setAttribute("role", "meter");
    bar.setAttribute("aria-label", label);
    bar.setAttribute("aria-valuemin", "0");
    bar.setAttribute("aria-valuemax", "100");
    const fill_ = element("span", "fill");
    fill_.id = id;
    bar.append(fill_);
    const said = element("span", "said");
    const named = element("span", "device");
    named.id = device;
    const mark = element("span", "check");
    mark.append(icon("check"));
    const status = element("span", "check-said");
    status.setAttribute("role", "status");
    mark.append(status);
    const slot = element("span", "slot");
    slot.append(mark, named);
    said.append(element("span", "side", label), slot);
    const sentence = element("p", "says check-says");
    sentence.setAttribute("role", "status");
    row.append(said, bar, sentence);
    checks.push({ slot, status, sentence, says, heard: fill(words.small_heard, label), since: 0 });
    return row;
  };
  // The reminder where the user looks, the recording's first seconds, its announcement one click
  // away (D50). Gone, its room stays: Arrêter never moves.
  const announce = element("div", "small-announce");
  announce.id = "small-announce";
  // Not offered before the first reading gives its sentence: a click would copy nothing.
  announce.hidden = true;
  const announced = element("button", "link", words.copy_announcement);
  announced.type = "button";
  announced.id = "small-announce-copy";
  const said_copied = element("span", "says copied");
  said_copied.id = "small-announce-copied";
  said_copied.setAttribute("role", "status");
  announce.append(element("span", "says", words.small_announce), announced, said_copied);
  let announcement = null;
  announced.addEventListener("click", async () => {
    try {
      await navigator.clipboard.writeText(announcement);
      said_copied.textContent = words.minutes_copied;
    } catch {
      said_copied.textContent = words.announcement_not_copied;
    }
    setTimeout(() => (said_copied.textContent = ""), 2000);
  });
  const stop = element("button", "danger", words.small_stop);
  stop.type = "button";
  stop.id = "small-stop";
  stop.addEventListener("click", () => asking(stop, words.record_stopping, false, words.small_stop));
  const meters = element("div", "meters");
  meters.append(
    meter(words.small_mic, "level-me", "device-me", { silent: words.small_silent_me, nothing: words.small_nothing, waiting: words.small_waiting_me }),
    meter(words.small_meeting, "level-others", "device-others", { silent: words.small_silent_others, waiting: words.small_waiting_others }),
  );
  const note = element("p", "says", words.small_note);
  note.id = "small-note";
  note.hidden = true;
  // The meeting's context (D59): closed by default, opened in place of the note — Arrêter never
  // moves. Kept as it is typed, once the meeting's folder holds its session.
  const meeting = { name: null, saved: null, typed: null, refused: null };
  // What could not be kept: it outlasts the recording (D67).
  const unkept = new Map();
  const unkeptSaid = element("p", "note warn small-said");
  unkeptSaid.id = "small-said";
  unkeptSaid.setAttribute("role", "status");
  unkeptSaid.hidden = true;
  const smallSaid = (what, sentence) => {
    if (sentence === null) unkept.delete(what);
    else unkept.set(what, sentence);
    unkeptSaid.textContent = [...unkept.values()].join("\n");
    unkeptSaid.hidden = unkept.size === 0;
  };
  const keep = async (last = false) => {
    const { name, typed } = meeting;
    if (name === null || typed === null || typed === meeting.saved) return;
    // Refused: tried again when it changes, and when the recording stops; each try is a line in the log.
    if (typed === meeting.refused && !last) return;
    try {
      await invoke("context_saved", { name, text: typed });
      meeting.saved = typed;
      smallSaid("context", null);
    } catch {
      meeting.refused = typed;
      smallSaid("context", words.context_not_saved);
    }
  };
  const field = contextField(words, "", (text) => {
    meeting.typed = text;
    keep();
  }, "small-context");
  field.area.rows = 2;
  field.area.placeholder = words.context_label;
  field.area.setAttribute("aria-label", words.context_label);
  const opened = element("div", "small-context");
  opened.append(field.area, field.count);
  opened.hidden = true;
  const disclose = element("button", "open disclose");
  disclose.type = "button";
  disclose.id = "small-context-toggle";
  disclose.setAttribute("aria-controls", "small-context");
  const toggled = element("span", "", words.context_add);
  disclose.append(toggled, element("span", "chevron", "▾"));
  const unfold = (open) => {
    opened.hidden = !open;
    disclose.setAttribute("aria-expanded", String(open));
    toggled.textContent = open ? words.context_open : field.area.value.trim() ? words.context_edit : words.context_add;
    disclose.lastChild.textContent = open ? "▴" : "▾";
    note.hidden = open || note.dataset.follows !== "true";
    // Open, the window is full: the switch keeps its name, its words said to a screen reader (D60).
    live.classList.toggle("context-open", open);
  };
  unfold(false);
  disclose.addEventListener("click", () => {
    unfold(opened.hidden);
    if (!opened.hidden) field.area.focus();
  });
  field.area.addEventListener("keydown", (event) => {
    if (event.key !== "Escape") return;
    unfold(false);
    disclose.focus();
  });
  // Said confidential (D60): under Arrêter, above the context; kept as it is turned, once the
  // meeting's folder holds its session.
  const secret = confidentialSwitch(words, "small-confidential");
  const mark = { wanted: null, refused: 0 };
  const marked = async () => {
    if (meeting.name === null || mark.wanted === null) return;
    const wanted = mark.wanted;
    try {
      await invoke("confidential_set", { name: meeting.name, on: wanted });
      if (mark.wanted === wanted) mark.wanted = null;
      smallSaid("mark", null);
      secret.row.classList.toggle("on", secret.box.checked);
    } catch {
      // Said, its lock off: on with nothing kept would promise what is not so. Asked again every
      // five seconds, not every tick: each try is a line in the log.
      mark.refused = Date.now();
      secret.row.classList.remove("on");
      smallSaid("mark", words.confidential_not_saved);
    }
  };
  secret.box.addEventListener("change", () => {
    mark.wanted = secret.box.checked;
    mark.refused = 0;
    secret.row.classList.toggle("on", secret.box.checked);
    marked();
  });
  live.append(top, announce, meters, stop, secret.row, disclose, opened, note);
  live.hidden = true;
  page.replaceChildren(rest, live, unkeptSaid);

  // At rest: drawn again only when what it says changes, so that a link under the focus stays.
  const drawRest = async () => {
    // Not read: said as a folder unread is, nothing stale offered; drawn anew once it reads.
    const listing = await invoke("listed").catch(() => {
      rest.dataset.key = "";
      last.replaceChildren(element("p", "none", words.small_unread));
      foot.replaceChildren();
      return null;
    });
    if (listing === null) return;
    await readMakings();
    const sessions = listing.sessions.filter((session) => session.state !== "recording");
    const shown = sessions[0] ?? null;
    const weight = sessions.reduce((sum, session) => sum + session.audio + session.text, 0);
    const key = JSON.stringify([listing.unreadable, listing.shortcut, shown && [nameOf(shown), shown.minutes, shown.transcript, shown.audio, shown.seconds], sessions.length, weight]);
    if (rest.dataset.key !== key) {
      rest.dataset.key = key;
      keys.textContent = listing.shortcut ? fill(words.small_keys, listing.shortcut) : words.small_no_keys;
      keys.disabled = Boolean(listing.shortcut);
      keys.onclick = listing.shortcut ? null : () => full("#/settings");
      if (shown === null) {
        last.replaceChildren(element("p", "none", listing.unreadable ? words.small_unread : words.small_none));
        foot.replaceChildren();
      } else {
        const card = element("div", "last");
        const when = shown.seconds === null ? heading(shown, words) : `${heading(shown, words)} · ${length(shown.seconds)}`;
        const open = element("button", "open link", shown.minutes ? words.small_read : words.small_open);
        open.type = "button";
        open.id = "small-read";
        open.addEventListener("click", () => full(`#/meeting/${encodeURIComponent(nameOf(shown))}`));
        card.append(element("span", "label", words.small_last), element("span", "what", when), holdings(shown, words), open);
        last.replaceChildren(card);
        const count = fill(sessions.length === 1 ? words.small_count_one : words.small_count, sessions.length, size(weight, words));
        const all = element("button", "open link", words.small_all);
        all.type = "button";
        all.id = "small-all";
        all.addEventListener("click", () => full("#/"));
        foot.replaceChildren(element("span", "count", count), all);
      }
    }
    paintMakings();
  };

  let recording = null;
  const tick = async () => {
    const now = await invoke("recording_now").catch(() => null);
    if (now === null) return;
    if (now.recording !== recording) {
      const first = recording === null;
      // What was typed last, kept before the field is emptied for the next recording.
      if (recording) {
        await keep(true);
        await marked();
        // Refused to the end: at rest no switch is left, and what remains is said as it is.
        smallSaid("mark", mark.wanted === true ? words.small_not_confidential : null);
      }
      // A new recording starts clean; one that ended keeps saying what it could not keep.
      if (now.recording) {
        smallSaid("context", null);
        smallSaid("mark", null);
      }
      // Under the switch while recording, *Arrêter* staying where it is; first of all at rest.
      if (now.recording) secret.row.after(unkeptSaid);
      else page.prepend(unkeptSaid);
      recording = now.recording;
      start.disabled = false;
      start.textContent = words.record_start;
      stop.disabled = false;
      stop.textContent = words.small_stop;
      // Each recording its own context: nothing of the one before.
      Object.assign(meeting, { name: null, saved: null, typed: null, refused: null });
      mark.wanted = null;
      secret.box.checked = false;
      secret.row.classList.remove("on");
      field.area.value = "";
      field.painted();
      unfold(false);
      rest.hidden = recording;
      live.hidden = !recording;
      if (!recording) await drawRest();
      // Opened at rest, the recording's gesture has the focus.
      if (first && !recording) start.focus();
    }
    if (!recording) {
      // A start that failed late: the shell has let it go, and no recording will come to give it back.
      if (start.disabled && !unanswered && !(await window.__TAURI__.core.invoke("header")).recording) {
        start.disabled = false;
        start.textContent = words.record_start;
      }
      return;
    }
    const whole = Math.floor(now.seconds);
    time.textContent = [Math.floor(whole / 3600), Math.floor((whole % 3600) / 60), whole % 60].map((n) => String(n).padStart(2, "0")).join(":");
    for (const [index, id] of ["level-me", "level-others"].entries()) {
      const percent = Math.round(now.levels[index] * 100);
      const fill_ = document.getElementById(id);
      fill_.style.width = `${percent}%`;
      fill_.parentElement.setAttribute("aria-valuenow", String(percent));
    }
    for (const [index, id] of ["device-me", "device-others"].entries()) {
      const named = document.getElementById(id);
      named.textContent = now.devices[index] ?? "";
      named.title = now.devices[index] ?? "";
      // Its name hidden while the context is open: said on its meter still (D60).
      named.closest(".meter").title = now.devices[index] ?? "";
    }
    // Written when it changes alone: a tick that rewrites text under a click loses it in WebKitGTK.
    for (const [index, check] of checks.entries()) {
      const state = now.checks[index];
      if (check.slot.dataset.check !== state) {
        check.slot.dataset.check = state;
        check.since = performance.now();
        check.status.textContent = state === "heard" ? check.heard : "";
        check.sentence.textContent = check.says[state] ?? "";
      }
      check.slot.classList.toggle("faded", state === "heard" && performance.now() - check.since >= CHECK_SHOWN);
    }
    announcement = now.announcement;
    announce.hidden = announcement === null;
    announce.classList.toggle("gone", now.seconds >= ANNOUNCED_FOR);
    stop.disabled ||= !now.recording;
    note.dataset.follows = String(now.transcript_follows);
    note.hidden = !now.transcript_follows || !opened.hidden;
    // The meeting's folder there: what it holds already is shown, what was typed before is kept.
    if (now.meeting !== meeting.name) {
      const another = meeting.name !== null;
      meeting.name = now.meeting;
      if (another) meeting.typed = null;
      if (now.meeting !== null) {
        const there = await invoke("context_shown", { name: now.meeting }).catch((why) => {
          smallSaid("context-read", fill(words.context_not_read, why));
          return null;
        });
        if (there !== null) smallSaid("context-read", null);
        meeting.saved = there?.text ?? "";
        if (meeting.typed === null && meeting.name === now.meeting) {
          field.area.value = meeting.saved;
          field.painted();
          if (opened.hidden) unfold(false);
        }
      }
    }
    keep();
    // What the folder says, unless a turn of the switch is still to be kept.
    if (mark.wanted === null && now.meeting !== null && secret.box.checked !== now.confidential) {
      secret.box.checked = now.confidential;
      secret.row.classList.toggle("on", now.confidential);
    }
    if (Date.now() - mark.refused >= 5000) marked();
  };
  setInterval(tick, 100);
  setInterval(() => recording === false && drawRest(), 1500);
  tick();
}

// Freeing space (D11's third board): the audio alone or the whole meeting, the heaviest first,
// what is freed said before; everything to the system's trash, on the user's word.
function showFree(listing, busy, words) {
  const page = document.getElementById("free");
  let whole = page.dataset.whole === "true";
  const ticked = new Set(JSON.parse(page.dataset.ticked || "[]"));
  const draw = () => {
    page.dataset.whole = String(whole);
    page.dataset.ticked = JSON.stringify([...ticked]);
    const weight = (s) => (whole ? s.audio + s.text : s.audio);
    const free = listing.sessions.filter((s) => weight(s) > 0 && !busy.includes(nameOf(s)));
    const kept = listing.sessions.filter((s) => busy.includes(nameOf(s)));
    free.sort((a, b) => weight(b) - weight(a));
    for (const name of [...ticked]) if (!free.some((s) => nameOf(s) === name)) ticked.delete(name);
    // As the board heads it: the way back, and the page's name.
    const top = element("div", "page-top");
    top.append(element("h1", "", words.free));
    const parts = [top, element("p", "eyebrow", words.free_what)];
    const ways = element("div", "ways");
    ways.setAttribute("role", "radiogroup");
    for (const [value, said] of [[false, words.free_audio], [true, words.free_whole]]) {
      const one = choice("free-what", String(value), said, whole === value, false);
      one.querySelector("input").addEventListener("change", () => {
        whole = value;
        draw();
      });
      ways.append(one);
    }
    parts.push(ways);
    const head = element("div", "row");
    const older = element("button", "open", words.free_older);
    older.type = "button";
    older.id = "free-older";
    older.addEventListener("click", () => {
      const month = Date.now() / 1000 - 30 * 86400;
      for (const s of free) if (s.instant !== null && s.instant < month) ticked.add(nameOf(s));
      draw();
    });
    head.append(element("p", "eyebrow", words.free_heaviest), older);
    parts.push(head);
    const list = element("ol", "free-list");
    for (const s of free) {
      const item = element("li");
      const row = element("label", "card freeable");
      const box = element("input");
      box.type = "checkbox";
      box.name = "freeable";
      box.value = nameOf(s);
      box.checked = ticked.has(nameOf(s));
      box.addEventListener("change", () => {
        if (box.checked) ticked.add(nameOf(s));
        else ticked.delete(nameOf(s));
        draw();
      });
      const said = element("span", "said");
      // Named (D70): the date stays beside the length, what tells two of one name apart.
      const lasted = s.seconds !== null ? length(s.seconds) : "";
      said.append(element("span", "name", s.title ?? heading(s, words)), element("span", "length", s.title ? `${heading(s, words)} · ${lasted}` : lasted));
      row.append(box, said, element("span", "weighs", size(weight(s), words)));
      item.append(row);
      list.append(item);
    }
    for (const s of kept) {
      const item = element("li");
      const row = element("div", "card freeable busy");
      const said = element("span", "said");
      said.append(element("span", "name", s.title ?? heading(s, words)), element("span", "length", s.title ? `${heading(s, words)} · ${words.free_busy}` : words.free_busy));
      row.append(said);
      item.append(row);
      list.append(item);
    }
    parts.push(list);
    const chosen = free.filter((s) => ticked.has(nameOf(s)));
    const freed = chosen.reduce((sum, s) => sum + weight(s), 0);
    const total = element("section", "card free-total");
    const sum = element("div", "said");
    const named = element("div", "total");
    named.append(element("strong", "", size(freed, words)), element("span", "", chosen.length === 1 ? words.free_one : fill(words.free_total, chosen.length)));
    sum.append(named, element("p", "says", words.free_note));
    if (!whole) sum.append(element("p", "says", words.free_audio_note));
    const go = element("button", "danger", whole ? words.free_do_whole : words.free_do_audio);
    go.type = "button";
    go.id = "free-do";
    go.disabled = chosen.length === 0;
    const said = element("p", "note");
    said.id = "free-said";
    said.setAttribute("aria-live", "polite");
    go.addEventListener("click", async () => {
      go.disabled = true;
      try {
        await window.__TAURI__.core.invoke("freed", { names: chosen.map(nameOf), whole });
        ticked.clear();
        page.dataset.ticked = "[]";
        page.dataset.said = words.free_done;
      } catch (why) {
        page.dataset.said = fill(words.not_saved, why);
      }
      await refresh();
    });
    total.append(sum, go);
    said.textContent = page.dataset.said ?? "";
    delete page.dataset.said;
    parts.push(total, said);
    // Each gesture draws the page again: the focus goes back where the keyboard left it.
    const focused = page.contains(document.activeElement) ? document.activeElement : null;
    const refocus = focused && (focused.id ? `#${CSS.escape(focused.id)}` : focused.name ? `input[name="${CSS.escape(focused.name)}"][value="${CSS.escape(focused.value)}"]` : null);
    page.replaceChildren(...parts);
    if (refocus) page.querySelector(refocus)?.focus();
  };
  draw();
}

// The first run's assistant (D58): one question — two when the minutes may go online —, then the
// setting said plainly. Private unless the user says otherwise; from Settings (`#/welcome/again`)
// the current setting chosen, and nothing saved before the last screen's confirmation.
// Settings open at the minutes once, after « Choisir mon service ».
let focusMinutes = false;
// The language just chosen in Settings (D64): focused again once the page is drawn in it.
let focusLanguage = false;

async function showWelcome(again, words) {
  const page = document.getElementById("welcome");
  const { invoke } = window.__TAURI__.core;
  const [shown, minutes] = await Promise.all([invoke("settings_shown"), invoke("minutes_shown")]);
  // Apple's SpeechAnalyzer, asked aside: only the summary needs it.
  const speech = invoke("speech_shown").catch(() => null);
  const local = !minutes.provider || minutes.provider_local;
  // From Settings, the setting as it stands; at the first run, nothing chosen for the user.
  const state = { step: "question", private: again && minutes.provider ? (local ? "often" : "rarely") : null, writer: again && minutes.provider && !local ? "service" : null };
  const connected = () => state.private === "rarely" && state.writer === "service";
  const ollamaModel = minutes.provider === minutes.ollama_address && minutes.model ? minutes.model : minutes.recommended;
  const engine = shown.engine === "speechanalyzer" ? "speechanalyzer" : "whisper";
  const modelFile = shown.model.split(/[\\/]/).pop();
  const modelHere = shown.models.some((model) => model.path === shown.model);
  // The model the tray fetches is the list's first, the default: one the user named is theirs.
  const fetchable = !modelHere && shown.offered[0]?.file === modelFile ? shown.offered[0] : null;

  const card = (id) => {
    const made = element("section", "card welcome");
    made.id = id;
    const top = element("div", "welcome-top");
    top.append(document.querySelector("#sidebar .brand").cloneNode(true));
    made.append(top);
    return [made, top];
  };
  const heading = (text) => {
    const h1 = element("h1", "", text);
    h1.id = "welcome-heading";
    h1.tabIndex = -1;
    return h1;
  };
  const button = (id, className, text, act) => {
    const made = element("button", className, text);
    made.type = "button";
    made.id = id;
    made.addEventListener("click", act);
    return made;
  };
  const group = (name, labelled, options, chosen) => {
    const radios = element("div", "welcome-choices");
    radios.setAttribute("role", "radiogroup");
    radios.setAttribute("aria-labelledby", labelled);
    for (const [value, said] of options) {
      const one = choice(name, value, said, chosen === value, false);
      one.querySelector("input").addEventListener("change", () => (state[name] = value));
      radios.append(one);
    }
    return radios;
  };
  const said = element("p", "says welcome-said");
  said.id = "welcome-said";
  said.setAttribute("role", "status");
  const go = (step) => {
    state.step = step;
    draw();
  };
  // Confirmed: what the summary said, saved with Settings' own commands; refused, said, nothing more.
  const save = async () => {
    try {
      if (!connected() && (minutes.provider !== minutes.ollama_address || minutes.model !== ollamaModel)) {
        await invoke("minutes_saved", { provider: minutes.ollama_address, model: ollamaModel, key: null });
      }
      // Private: the transcription comes back to this computer, the engine alone. Connected
      // changes nothing by itself: the service is chosen in Settings.
      if (!connected() && shown.engine !== engine) await invoke("settings_saved", { change: { engine } });
      return true;
    } catch (why) {
      said.textContent = fill(words.not_saved, why);
      return false;
    }
  };
  const done = (hash = "#/") => {
    location.hash = hash;
  };

  const question = () => {
    const [made] = card("welcome-question");
    const examples = element("p", "says", words.welcome_examples);
    examples.id = "welcome-examples";
    const radios = group("private", "welcome-heading", [["often", words.welcome_often], ["rarely", words.welcome_rarely], ["unsure", words.welcome_unsure]], state.private);
    const actions = element("div", "welcome-actions");
    actions.append(
      again
        ? button("welcome-skip", "link", words.welcome_cancel, () => done("#/settings"))
        : button("welcome-skip", "link", words.welcome_skip, async () => {
          state.private = "often";
          if (await save()) done();
        }),
      // Nothing chosen is the most discreet: private.
      button("welcome-next", "primary", words.welcome_next, () => go(state.private === "rarely" ? "writer" : "summary")),
    );
    radios.setAttribute("aria-describedby", "welcome-examples");
    made.append(element("p", "says welcome-eyebrow", words.welcome_title), heading(words.welcome_question), examples, radios, actions, said);
    return made;
  };

  const writer = () => {
    const [made] = card("welcome-writer");
    const title = heading(words.welcome_writer);
    const radios = group("writer", "welcome-heading", [["here", words.welcome_here], ["service", words.welcome_service]], state.writer);
    const actions = element("div", "welcome-actions");
    actions.append(button("welcome-back", "link", words.welcome_back, () => go("question")), button("welcome-next", "primary", words.welcome_next, () => go("summary")));
    made.append(title, radios, actions, said);
    return made;
  };

  const summary = async () => {
    const status = await speech;
    const [made, top] = card("welcome-summary");
    top.append(button("welcome-back", "link", words.welcome_back, () => go(state.private === "rarely" ? "writer" : "question")));
    const online = connected();
    const [name, line] = online ? words.welcome_connected : words.welcome_private;
    const mode = element("div", `welcome-mode ${online ? "connected" : "private"}`);
    mode.append(heading(name), element("p", "", line));
    const facts = element("dl", "welcome-facts");
    const fact = (term, main, secondary) => {
      const value = element("dd");
      value.append(element("span", "name", main));
      if (secondary) value.append(element("span", "says", secondary));
      facts.append(element("dt", "", term), value);
    };
    const [transcription, minutesRow_, reminder] = words.welcome_rows;
    // Connected leaves who hears as it is: a service already chosen in Settings is said as one.
    let hearer = shown.transcription_provider ?? "";
    try {
      hearer = new URL(hearer).host;
    } catch {}
    if (online && shown.engine === "service") fact(transcription, words.service_option, hearer);
    else fact(transcription, words.welcome_on_this_computer, engine === "speechanalyzer" ? words.speech_option : spoken(modelFile));
    let host = "";
    try {
      host = new URL(minutes.provider).host;
    } catch {}
    // Online, chosen already (from Settings): its host; else the services Settings offer.
    const offered = new Intl.ListFormat(words.locale, { type: "disjunction" }).format(minutes.online.map((service) => service.name));
    if (online) fact(minutesRow_, local ? words.welcome_online : words.welcome_online_chosen, local ? offered : host);
    else fact(minutesRow_, words.welcome_on_this_computer, fill(words.welcome_ollama_by, modelSaid(ollamaModel)));
    fact(reminder, shown.reminder ? words.welcome_reminder_on : words.welcome_reminder_off);
    made.append(mode, facts);
    // The minutes wait for Ollama: said, with where it comes from — Wholeheard never installs it.
    if (!online && !minutes.ollama_answers) made.append(element("p", "says welcome-wait", words.welcome_ollama_silent));
    else if (!online && !minutes.ollama.includes(ollamaModel)) made.append(element("p", "says welcome-wait", fill(words.welcome_ollama_lacks, ollamaModel)));
    const actions = element("div", "welcome-actions");
    const later = button("welcome-later", "link", words.welcome_later, async () => {
      if (await save()) done();
    });
    let primary;
    let from = null;
    if (online) {
      primary = button("welcome-go", "primary", words.welcome_choose, async () => {
        if (!(await save())) return;
        focusMinutes = true;
        done("#/settings");
      });
    } else if (engine === "speechanalyzer" && status?.offered && !status.installed) {
      // Apple's model, fetched by macOS on this click, as Settings fetch it (D42).
      primary = button("welcome-go", "primary", words.speech_fetch, async () => {
        if (!(await save())) return;
        invoke("speech_installed").catch((why) => pageFailed(why));
        done();
      });
    } else if (engine === "whisper" && fetchable) {
      // The tray's own fetch, its size and host shown first (D36); how far is said where Settings
      // and the tray say it.
      primary = button("welcome-go", "primary", fill(words.welcome_fetch, size(fetchable.bytes, words)), async () => {
        if (!(await save())) return;
        await invoke("model_fetch").catch(() => {});
        done();
      });
      from = element("p", "says welcome-from", fill(words.welcome_fetch_from, fetchable.host));
      from.id = "welcome-host";
    } else {
      primary = button("welcome-go", "primary", words.welcome_done, async () => {
        if (await save()) done();
      });
    }
    actions.append(later, primary);
    made.append(actions);
    if (from) made.append(from);
    made.append(said);
    return made;
  };

  const draw = async () => {
    said.textContent = "";
    const made = state.step === "question" ? question() : state.step === "writer" ? writer() : await summary();
    page.replaceChildren(made);
    made.querySelector("#welcome-heading").focus();
  };
  await draw();
}

let words = null;

// What the last save on the settings' page could not do, for the page drawn after it.
let notSaved = null;

// The latest drawing asked for: one begun before it, its answers come later, draws nothing.
let drawing = 0;

// Under 840 px, one pane at a time (D61): the list's address is the list alone.
const narrow = matchMedia("(max-width: 839px)");
narrow.addEventListener("change", () => refresh());

// What the meeting's page was drawn from, to know whether it changed since.
let drawnMeeting = null;

// `again`: the list's address drawn once more after a page gone — never more than once.
async function refresh(again = false) {
  const mine = ++drawing;
  // The small window is its own: it draws its view once, and asks for nothing else.
  if (location.hash === "#/small") {
    if (!document.body.classList.contains("small-window")) {
      const listing = await window.__TAURI__.core.invoke("listed");
      words = listing.words;
      document.documentElement.lang = words.locale;
      document.getElementById("small").hidden = false;
      document.getElementById("frame").hidden = true;
      showSmall(words);
    }
    return;
  }
  const listing = await window.__TAURI__.core.invoke("listed");
  if (mine !== drawing) return;
  words = listing.words;
  show(listing);
  const wanted = location.hash.match(/^#\/meeting\/(.+)$/);
  // The list's address: the pane shows the last meeting beside it (D61); under 840 px, the list alone.
  const listed = location.hash === "" || location.hash === "#/";
  document.body.dataset.at = listed ? "list" : "page";
  const beside = listed && !narrow.matches && listing.sessions.length > 0;
  const name = wanted ? decodeURIComponent(wanted[1]) : beside ? nameOf(listing.sessions[0]) : null;
  // Another page, or another meeting: what was played stops — and the list alone under 840 px.
  if (player && player.name !== name) {
    player.stop();
    player = null;
  }
  const settings = location.hash === "#/settings";
  // The first run's assistant, alone in the window: no header, no other page (D58).
  const welcoming = /^#\/welcome(\/again)?$/.test(location.hash);
  document.getElementById("frame").hidden = welcoming;
  const welcome = document.getElementById("welcome");
  welcome.hidden = !welcoming;
  if (welcoming) {
    for (const other of ["meeting", "settings", "agree", "free"]) document.getElementById(other).hidden = true;
    // Drawn once while it is shown: the page read again meanwhile — a model come, a fetch ended —
    // never takes the answers from under the user.
    if (welcome.dataset.shown === location.hash) return;
    welcome.dataset.shown = location.hash;
    return showWelcome(location.hash === "#/welcome/again", words);
  }
  welcome.replaceChildren();
  delete welcome.dataset.shown;
  const freeing = location.hash === "#/free";
  document.getElementById("free").hidden = !freeing;
  if (freeing) {
    const busy = await window.__TAURI__.core.invoke("busy_meetings");
    if (mine !== drawing) return;
    showFree(listing, busy, words);
  }
  else document.getElementById("free").replaceChildren();
  const agreeing = location.hash.match(/^#\/agree\/(.+)$/);
  let agreement = null;
  if (agreeing) {
    try {
      agreement = await window.__TAURI__.core.invoke("agreement_shown", { name: decodeURIComponent(agreeing[1]) });
    } catch (why) {
      // Said in the pane: the list drawn again keeps it, until the next address.
      const said = document.getElementById("pane-said");
      said.textContent = fill(words.agreement_refused, why);
      said.hidden = false;
      // No hashchange follows a replaced address: the list's address is drawn here (D61).
      history.replaceState(null, "", "#/");
      if (!again) return refresh(true);
    }
  }
  let opened = null;
  let seen = makings;
  // Settings too: their models' states are drawn with the page, not at the next reading (D37).
  if (name || settings) {
    // What is being made, read before the meeting: the counts its page keeps are never ahead of
    // what it shows, never behind what was made before it was read — drawn again only for what
    // was made since.
    seen = await window.__TAURI__.core.invoke("makings").catch(() => makings);
  }
  if (name) {
    // A meeting no longer there, or a name that is none, goes back to the list.
    try {
      opened = await window.__TAURI__.core.invoke("opened", { name });
    } catch (why) {
      // A reading since then answers for its own address.
      if (mine !== drawing) return;
      // Gone while it played: it stops with its page.
      player?.stop();
      player = null;
      history.replaceState(null, "", "#/");
      // No hashchange follows a replaced address: said now, a name refused on that page first.
      paneSays(nameRefused ?? fill(words.meeting_not_opened, why));
      nameRefused = null;
      if (!again) return refresh(true);
    }
  }
  if (mine !== drawing) return;
  document.getElementById("empty").hidden = !listed || listing.sessions.length > 0;
  // No meeting yet: under 840 px, the sidebar and what to do, one under the other.
  document.body.classList.toggle("no-meeting", listing.sessions.length === 0);
  document.getElementById("agree").hidden = agreement === null;
  // Shown or emptied: never a question left hidden under another page.
  if (agreement) showAgreement(decodeURIComponent(agreeing[1]), agreement, words);
  else document.getElementById("agree").replaceChildren();
  document.getElementById("meeting").hidden = opened === null;
  // Not shown, not kept: a page hidden under another is never found in its place.
  if (opened === null) {
    document.getElementById("meeting").replaceChildren();
    delete document.getElementById("meeting").dataset.name;
  }
  document.getElementById("settings").hidden = !settings;
  document.getElementById("settings-link").setAttribute("aria-current", settings ? "page" : "false");
  // The meeting shown, marked in the list.
  const shownAt = opened ? `#/meeting/${encodeURIComponent(name)}` : null;
  for (const link of document.querySelectorAll("#groups .session")) {
    if (link.getAttribute("href") === shownAt) link.setAttribute("aria-current", "page");
    else link.removeAttribute("aria-current");
  }
  // Drawn with what was read before it, never with a newer reading taken meanwhile.
  // The meeting shown beside the list, then reached at its own address — its row clicked, or
  // the small window's *Lire* —, read again and unchanged: the same page, not drawn again under
  // the user (D61). Anything changed meanwhile, or any other reading, draws it anew.
  const page = document.getElementById("meeting");
  const address = listed ? "#/" : location.hash;
  const fresh = opened ? JSON.stringify(opened) : null;
  const kept = opened && page.dataset.name === name && page.dataset.at === "#/" && address !== "#/" && fresh === drawnMeeting;
  if (opened && !kept) {
    makings = seen;
    showMeeting(opened, words);
    drawnMeeting = fresh;
  }
  if (opened) page.dataset.at = address;
  else delete page.dataset.at;
  paintMakings();
  if (settings) {
    const [shown, minutes] = await Promise.all([
      window.__TAURI__.core.invoke("settings_shown"),
      window.__TAURI__.core.invoke("minutes_shown"),
    ]);
    if (mine !== drawing) return;
    makings = seen;
    showSettings(shown, minutes, words);
  }
}

document.getElementById("small-link").addEventListener("click", () => window.__TAURI__.core.invoke("small_window"));
document.getElementById("find").addEventListener("input", sought);
// The sidebar folded, or brought back (D61): kept in the settings; the focus follows a button gone.
async function fold(folded) {
  document.body.classList.toggle("folded", folded);
  const gone = document.getElementById(folded ? "sidebar" : "unfold");
  if (gone.contains(document.activeElement)) document.getElementById(folded ? "unfold" : "fold").focus();
  // Not kept: said; the sidebar stays as asked until the next start.
  const said = document.getElementById("pane-said");
  try {
    await window.__TAURI__.core.invoke("settings_saved", { change: { folded } });
    said.hidden = true;
  } catch (why) {
    said.textContent = fill(words.fold_not_kept, why);
    said.hidden = false;
  }
}
document.getElementById("fold").addEventListener("click", () => fold(true));
document.getElementById("unfold").addEventListener("click", () => fold(false));
// Ctrl+B, ⌘B on a Mac, outside a text field: the convention of sidebars that fold.
document.addEventListener("keydown", (event) => {
  // The key where B is, whatever the layout writes there; held down, it folds once.
  if (!(event.ctrlKey || event.metaKey) || event.altKey || event.shiftKey || event.code !== "KeyB" || event.repeat) return;
  const typing = event.target.closest?.("textarea, select, [contenteditable], input:not([type=checkbox], [type=radio], [type=button])");
  if (typing || narrow.matches || document.getElementById("frame").hidden || document.body.classList.contains("small-window")) return;
  event.preventDefault();
  fold(!document.body.classList.contains("folded"));
});
// ↑ and ↓ on the list: the meeting above or below shown in the pane, the focus on its row (D61).
document.getElementById("groups").addEventListener("keydown", (event) => {
  if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
  const rows = [...document.querySelectorAll("#groups li:not([hidden]) .session")];
  const at = rows.indexOf(event.target.closest(".session"));
  const next = rows[at + (event.key === "ArrowDown" ? 1 : -1)];
  if (at < 0 || !next) return;
  event.preventDefault();
  next.focus();
  location.hash = next.getAttribute("href");
});
// The empty pane's own gesture (D61's board 5): the sidebar's, offered where the eye is.
document.getElementById("empty-record").addEventListener("click", () => document.getElementById("record").click());
document.getElementById("empty-again").addEventListener("click", () => refresh());
// Its new state at once, the command answering after it; a second click meanwhile asks nothing.
// Answered, the shell's word: a press it took for the same one, or a start that failed, shows (D68).
document.getElementById("record").addEventListener("click", async () => {
  if (recordAsked !== null) return;
  recordAsked = !document.getElementById("record").classList.contains("live");
  paintRecord(recordAsked, false);
  try {
    await window.__TAURI__.core.invoke("record");
  } finally {
    recordAsked = null;
  }
  const { recording, stopping } = await window.__TAURI__.core.invoke("header");
  paintRecord(recording, stopping);
  // The recording says it started, or stopped, a moment later: then the header follows.
  setTimeout(refresh, 1200);
});
// The header follows a recording started or stopped elsewhere — the tray, the shortcut — while
// the window is shown: asked lightly, every second and a half.
setInterval(async () => {
  if (document.hidden || words === null) return;
  const asked = await window.__TAURI__.core.invoke("header").catch(() => null);
  if (asked === null) return;
  readMakings();
  const { recording, stopping, shortcut } = asked;
  paintRecord(recording, stopping);
  const keys = document.getElementById("header-keys");
  keys.textContent = shortcut ?? "";
  keys.hidden = !shortcut;
}, 1500);
refresh();
// Coming back to the window draws it anew — unless who writes the minutes is being chosen there
// and not yet saved: that choice stays.
window.addEventListener("focus", () => {
  // The assistant too: its answers are not drawn over.
  const choosing = !document.getElementById("welcome").hidden
    || document.querySelector("#settings:not([hidden]) #minutes-unsaved:not([hidden])")
    || [...document.querySelectorAll("#settings:not([hidden]) .fields input")].some((input) => input.value !== input.defaultValue);
  if (!choosing) refresh();
});
window.addEventListener("hashchange", () => {
  document.getElementById("pane-said").hidden = true;
  serviceAnswer = null;
  if (nameRefused) paneSays(nameRefused);
  nameRefused = null;
  // Another address: the assistant, shown again, is drawn anew — even if the drawing that left it was cut short.
  delete document.getElementById("welcome").dataset.shown;
  refresh();
  window.scrollTo(0, 0);
});
