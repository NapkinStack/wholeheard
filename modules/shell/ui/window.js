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
  if (session.state === "recording") holds.append(chip(words.recording, "live"));
  if (session.state === "interrupted") holds.append(chip(words.interrupted, "cut"));
  if (session.minutes) holds.append(chip(ran(words.minutes, where?.minutes_here)));
  if (session.transcript) holds.append(chip(ran(words.transcript, true)));
  holds.append(session.audio > 0 ? chip(fill(words.audio, size(session.audio, words))) : chip(words.audio_freed, "gone"));
  // What is being made for it, in the background (D25): painted from `makings`.
  const making = chip("", "making");
  making.dataset.name = nameOf(session);
  making.hidden = true;
  holds.append(making);
  return holds;
}

// What Wholeheard makes in the background (D25): read with the header, painted where it is shown.
let makings = null;

function minutesState(name) {
  if (makings === null) return null;
  if (makings.minutes?.name === name) return { kind: "making", ...makings.minutes };
  const waiting = makings.minutes_waiting.indexOf(name);
  if (waiting >= 0) return { kind: "waiting", before: waiting + 1 };
  if (makings.awaiting[name] !== undefined) return { kind: "awaiting", host: makings.awaiting[name] };
  if (makings.failed[name] !== undefined) return { kind: "failed", why: makings.failed[name], said: makings.failed_said[name] };
  return null;
}

function transcriptState(name) {
  if (makings === null) return null;
  if (makings.transcript?.name === name) return { kind: "making", ...makings.transcript };
  const waiting = makings.transcripts_waiting.indexOf(name);
  // Nothing being made and the model not there: the line waits for it, not for a turn.
  if (waiting >= 0 && makings.transcript === null && makings.model_needed) return { kind: "model", coming: makings.model_coming };
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
    said.replaceChildren(element("span", "what", shown.coming ? words.transcript_model_coming : words.transcript_model_needed));
  } else if (shown.kind === "awaiting") {
    const agree = element("a", "open", words.agree_open);
    agree.id = "minutes-agree";
    agree.href = `#/agree/${said.dataset.name}`;
    said.replaceChildren(element("span", "what", fill(words.minutes_awaiting, shown.host)), agree);
  } else {
    const again = element("button", "open", words.try_again);
    again.type = "button";
    again.id = of.again;
    again.addEventListener("click", async () => {
      again.disabled = true;
      await window.__TAURI__.core.invoke(of.ask, { name: said.dataset.name }).catch(() => {});
      // Failing again for the same reason is said again: the state drawn anew.
      said.dataset.key = "";
      readMakings();
    });
    // What the program said, only in developer mode: the shell sends nothing else (D29).
    const detail = shown.why ? [element("code", "detail", fill(words.program_said, shown.why))] : [];
    // Its cause and gesture, when the program named one (D29): else the step's sentence alone.
    const what = shown.said ? `${of.not_made} ${shown.said}` : of.not_made;
    said.replaceChildren(element("span", "what", what), again, ...detail);
  }
}

function paintMakings() {
  for (const chip_ of document.querySelectorAll(".chip.making")) {
    // The minutes' state first; else the transcript's, which comes before them.
    const minutes = minutesState(chip_.dataset.name);
    const transcript = transcriptState(chip_.dataset.name);
    const state = minutes ?? transcript;
    chip_.hidden = state === null;
    if (state === null) continue;
    chip_.textContent = minutes
      ? { making: words.chip_making, waiting: words.chip_waiting, awaiting: words.chip_awaiting, failed: words.chip_failed }[state.kind]
      : { making: words.chip_transcribing, waiting: words.chip_waiting, model: words.chip_waiting, failed: words.chip_transcript_failed }[state.kind];
    chip_.classList.toggle("failed", state.kind === "failed");
  }
  const minutes = document.getElementById("minutes-state");
  if (minutes) {
    paintState(minutes, minutesState(minutes.dataset.name), {
      offer: "make-minutes", again: "minutes-again", ask: "minutes_again", making: words.minutes_making, not_made: words.minutes_not_made,
    });
  }
  const transcript = document.getElementById("transcript-state");
  if (transcript) {
    paintState(transcript, transcriptState(transcript.dataset.name), {
      offer: "make-transcript", again: "transcript-again", ask: "transcript_again", making: words.transcript_making, not_made: words.transcript_not_made,
    });
  }
}

async function readMakings() {
  makings = await window.__TAURI__.core.invoke("makings").catch(() => null);
  // Only the meeting shown: its page hidden, the last one opened is no longer in front.
  const opened = document.getElementById("meeting").hidden ? null : document.getElementById("minutes-state")?.dataset.name;
  // The minutes or the transcript of the meeting shown written since its page was drawn, however
  // quickly: drawn again, to show them — unless the user has a panel open there, or is typing in
  // it, which a redraw would take from under them; then once that is done.
  const busy = document.querySelector("#meeting .redo, #meeting input:focus");
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
  title.textContent = heading(session, words);
  when.append(title);
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
  link.append(drawn, when, holdings(session, words), element("span", "chevron", "›"));
  item.append(link);
  return item;
}

function show({ words, folder, sessions, shapes, recording, shortcut }) {
  document.documentElement.lang = words.locale;
  // The header's gesture, as the board has it: start or stop, with the shortcut beside.
  document.getElementById("free-link").textContent = words.free;
  const small = document.getElementById("small-link");
  small.textContent = words.small;
  // At rest too: the small window is the user's to choose (D35).
  small.hidden = false;
  document.getElementById("record-said").textContent = recording ? words.record_stop : words.record_start;
  document.getElementById("record").classList.toggle("live", recording);
  const keys = document.getElementById("header-keys");
  keys.textContent = shortcut ?? "";
  keys.hidden = !shortcut;
  document.getElementById("meetings").textContent = words.meetings;
  document.getElementById("sections").setAttribute("aria-label", words.sections);
  document.getElementById("settings-link").textContent = words.settings;
  const none = document.getElementById("none");
  const weight = document.getElementById("weight");
  none.hidden = sessions.length > 0;
  weight.hidden = sessions.length === 0;
  none.textContent = fill(words.none, folder);

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
}

const element = (tag, className, text) => {
  const made = document.createElement(tag);
  if (className) made.className = className;
  if (text !== undefined) made.textContent = text;
  return made;
};

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
  return panel;
}

function redoTranscriptPanel(shown, session, words, button) {
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
  panel.append(list, element("p", "says", words.redo_transcript_note), go);
  return panel;
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
    redo.replaceWith(redoPanel(choices, session, words, redo));
  });
  top.append(redo);
  const made = read.made && dayOf({ instant: Date.parse(read.made) / 1000 || null });
  // As the board says it: the day of the week, no year, and the model as people say it.
  const day = { weekday: "short", day: "numeric", month: "short" };
  const hour = { hour: "2-digit", minute: "2-digit" };
  // What is known of who wrote them, and when: what is not known is left unsaid.
  let maker = fill(words.written_by, read.model ? modelSaid(read.model) : "—");
  if (read.provider) maker += fill(words.written_via, read.provider);
  if (made) maker += ` · ${new Intl.DateTimeFormat(words.locale, day).format(made)} ${words.at} ${new Intl.DateTimeFormat(words.locale, hour).format(made)}`;
  card.append(element("p", "maker", maker));
  // The minutes as written, rendered by the shell: raw HTML in them is text there, a link its words.
  const body = element("div", "body");
  body.innerHTML = read.body;
  card.append(body);
  return card;
}

const folded = (text) => text.normalize("NFD").replace(/\p{M}/gu, "").toLowerCase();

// The one line of a transcript in the order of Tab: this one, the others reached by the arrows.
function roam(lines, line) {
  if (!line || line.tabIndex === -1 && !line.hasAttribute("role")) return;
  for (const other of lines.children) if (other.hasAttribute("role")) other.tabIndex = other === line ? 0 : -1;
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
    redo.replaceWith(redoTranscriptPanel(shown, session, words, redo));
  });
  // Beside who heard it, as board 2 has it: the head keeps its search whole.
  const heard = element("div", "made");
  heard.append(element("p", "maker", fill(words.heard_by, read.model ? spoken(read.model) : read.engine || "—")), redo);
  card.append(heard);
  if (!read.complete) card.append(element("p", "note", words.being_made));
  if (read.unread > 0) card.append(element("p", "note", fill(words.unread, read.unread)));
  const lines = element("ol", "lines");
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
    said.append(element("span", `who ${segment.track}`, who), element("span", "text", segment.text));
    line.append(element("span", "at", clock(segment.start)), said);
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
  shown.seek = (at) => {
    audio.currentTime = Math.max(0, Math.min(at, (audio.duration || at + 1) - 0.1));
    tell();
  };
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
    toggle.textContent = shown.playing() ? words.pause : words.play;
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
    const at = shown.time();
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

function showMeeting({ session, transcript, minutes, developer, listen: listened }, words) {
  const page = document.getElementById("meeting");
  const top = element("div", "meeting-top");
  const back = element("a", "back", `‹ ${words.back}`);
  back.href = "#/";
  const open = element("button", "open", words.open_folder);
  open.type = "button";
  open.addEventListener("click", () => window.__TAURI__.core.invoke("open_folder", { name: nameOf(session) }));
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
  top.append(back, actions);
  const title = element("section", "meeting-title");
  const named = element("div", "named");
  named.append(element("h1", "", heading(session, words)));
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
  page.dataset.name = nameOf(session);
  page.replaceChildren(top, title, drawn, columns);
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
        said = fill(words.test_failed, why);
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
  };
  const parts = [];
  if (locked) parts.push(element("p", "card note warn", fill(words.settings_unreadable, shown.unreadable)));
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
  const step = element("div", "step");
  const row = element("div", "row");
  const label = element("label", "name", words.transcription);
  label.htmlFor = "model";
  const pick = element("select");
  pick.id = "model";
  pick.disabled = locked;
  for (const model of shown.models) {
    const option = element("option", "", `${spoken(model.name)} — ${fill(words.installed, size(model.bytes, words))}`);
    option.value = model.path;
    option.selected = model.path === shown.model;
    pick.append(option);
  }
  pick.addEventListener("change", () => save({ model: pick.value }));
  row.append(label, pick);
  const here = element("p", "status here", words.on_this_computer);
  here.append(element("code", "tag", words.builtin));
  step.append(row, here);
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
  more.append(remind, offer, start, keys, where, keep);
  parts.push(more);

  // The transcription model's state: there and ready, or offered at the first transcript.
  const chosen = shown.models.find((model) => model.path === shown.model);
  const state = element("section", "card group state");
  const stateSaid = element("div", "said");
  stateSaid.append(
    element("span", "name", words.model_state),
    element("span", "says", chosen ? fill(words.model_here, size(chosen.bytes, words)) : words.model_missing),
  );
  state.append(stateSaid);
  if (chosen) state.append(element("span", "ready", `✓ ${words.model_ready}`));
  state.id = "model-state";
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
    line.append(element("span", "sign", mark === "leaves" ? "→" : "×"), said);
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
  const invoke = window.__TAURI__.core.invoke;
  const full = (where) => invoke("full_window", { page: where ?? null });
  const toFull = (id) => {
    const button = element("button", "open icon", "⤢");
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
  const start = element("button", "primary", words.record_start);
  start.type = "button";
  start.id = "small-record";
  start.addEventListener("click", () => invoke("record"));
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
  // Each level under what it is recorded from, as the system names it (D33).
  const meter = (label, id, device) => {
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
    said.append(element("span", "side", label), named);
    row.append(said, bar);
    return row;
  };
  const stop = element("button", "danger", words.small_stop);
  stop.type = "button";
  stop.id = "small-stop";
  stop.addEventListener("click", () => invoke("record"));
  const meters = element("div", "meters");
  meters.append(meter(words.small_mic, "level-me", "device-me"), meter(words.small_meeting, "level-others", "device-others"));
  const note = element("p", "says", words.small_note);
  note.hidden = true;
  live.append(top, meters, stop, note);
  live.hidden = true;
  page.replaceChildren(rest, live);

  // At rest: drawn again only when what it says changes, so that a link under the focus stays.
  const drawRest = async () => {
    const listing = await invoke("listed").catch(() => null);
    if (listing === null) return;
    await readMakings();
    const sessions = listing.sessions.filter((session) => session.state !== "recording");
    const shown = sessions[0] ?? null;
    const weight = sessions.reduce((sum, session) => sum + session.audio + session.text, 0);
    const key = JSON.stringify([listing.shortcut, shown && [nameOf(shown), shown.minutes, shown.transcript, shown.audio, shown.seconds], sessions.length, weight]);
    if (rest.dataset.key !== key) {
      rest.dataset.key = key;
      keys.textContent = listing.shortcut ? fill(words.small_keys, listing.shortcut) : words.small_no_keys;
      keys.disabled = Boolean(listing.shortcut);
      keys.onclick = listing.shortcut ? null : () => full("#/settings");
      if (shown === null) {
        last.replaceChildren(element("p", "none", words.small_none));
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
      recording = now.recording;
      rest.hidden = recording;
      live.hidden = !recording;
      if (!recording) await drawRest();
      // Opened at rest, the recording's gesture has the focus.
      if (first && !recording) start.focus();
    }
    if (!recording) return;
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
    }
    stop.disabled = !now.recording;
    note.hidden = !now.transcript_follows;
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
    const back = element("a", "back", `‹ ${words.back}`);
    back.href = "#/";
    top.append(back, element("h1", "", words.free));
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
      said.append(element("span", "name", heading(s, words)), element("span", "length", s.seconds !== null ? length(s.seconds) : ""));
      row.append(box, said, element("span", "weighs", size(weight(s), words)));
      item.append(row);
      list.append(item);
    }
    for (const s of kept) {
      const item = element("li");
      const row = element("div", "card freeable busy");
      const said = element("span", "said");
      said.append(element("span", "name", heading(s, words)), element("span", "length", words.free_busy));
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

let words = null;

// What the last save on the settings' page could not do, for the page drawn after it.
let notSaved = null;

// The latest drawing asked for: one begun before it, its answers come later, draws nothing.
let drawing = 0;

async function refresh() {
  const mine = ++drawing;
  // The small window is its own: it draws its view once, and asks for nothing else.
  if (location.hash === "#/small") {
    if (!document.body.classList.contains("small-window")) {
      const listing = await window.__TAURI__.core.invoke("listed");
      words = listing.words;
      document.documentElement.lang = words.locale;
      document.getElementById("small").hidden = false;
      for (const other of ["list", "meeting", "settings", "agree", "free"]) document.getElementById(other).hidden = true;
      showSmall(words);
    }
    return;
  }
  const listing = await window.__TAURI__.core.invoke("listed");
  if (mine !== drawing) return;
  words = listing.words;
  show(listing);
  const wanted = location.hash.match(/^#\/meeting\/(.+)$/);
  // Another page, or another meeting: what was played stops.
  if (player && player.name !== (wanted && decodeURIComponent(wanted[1]))) {
    player.stop();
    player = null;
  }
  const settings = location.hash === "#/settings";
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
    } catch {
      history.replaceState(null, "", "#/");
    }
  }
  let opened = null;
  let seen = makings;
  if (wanted) {
    // What is being made, read before the meeting: the counts its page keeps are never ahead of
    // what it shows, never behind what was made before it was read — drawn again only for what
    // was made since.
    seen = await window.__TAURI__.core.invoke("makings").catch(() => makings);
    // A meeting no longer there, or a name that is none, goes back to the list.
    try {
      opened = await window.__TAURI__.core.invoke("opened", { name: decodeURIComponent(wanted[1]) });
    } catch {
      // Gone while it played: it stops with its page.
      player?.stop();
      player = null;
      history.replaceState(null, "", "#/");
    }
  }
  if (mine !== drawing) return;
  document.getElementById("list").hidden = opened !== null || settings || agreement !== null || freeing;
  document.getElementById("agree").hidden = agreement === null;
  // Shown or emptied: never a question left hidden under another page.
  if (agreement) showAgreement(decodeURIComponent(agreeing[1]), agreement, words);
  else document.getElementById("agree").replaceChildren();
  document.getElementById("meeting").hidden = opened === null;
  document.getElementById("settings").hidden = !settings;
  document.getElementById("meetings").setAttribute("aria-current", settings || agreement ? "false" : "page");
  document.getElementById("settings-link").setAttribute("aria-current", settings ? "page" : "false");
  // Drawn with what was read before it, never with a newer reading taken meanwhile.
  if (opened) {
    makings = seen;
    showMeeting(opened, words);
  }
  paintMakings();
  if (settings) {
    const [shown, minutes] = await Promise.all([
      window.__TAURI__.core.invoke("settings_shown"),
      window.__TAURI__.core.invoke("minutes_shown"),
    ]);
    if (mine !== drawing) return;
    showSettings(shown, minutes, words);
  }
}

document.getElementById("small-link").addEventListener("click", () => window.__TAURI__.core.invoke("small_window"));
document.getElementById("record").addEventListener("click", async () => {
  await window.__TAURI__.core.invoke("record");
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
  const { recording, shortcut } = asked;
  document.getElementById("record-said").textContent = recording ? words.record_stop : words.record_start;
  document.getElementById("record").classList.toggle("live", recording);
  const keys = document.getElementById("header-keys");
  keys.textContent = shortcut ?? "";
  keys.hidden = !shortcut;
}, 1500);
refresh();
// Coming back to the window draws it anew — unless who writes the minutes is being chosen there
// and not yet saved: that choice stays.
window.addEventListener("focus", () => {
  const choosing = document.querySelector("#settings:not([hidden]) #minutes-unsaved:not([hidden])")
    || [...document.querySelectorAll("#settings:not([hidden]) .fields input")].some((input) => input.value !== input.defaultValue);
  if (!choosing) refresh();
});
window.addEventListener("hashchange", () => {
  refresh();
  window.scrollTo(0, 0);
});
