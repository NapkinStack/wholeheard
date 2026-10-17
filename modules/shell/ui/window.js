// The window: what the shell's `listed` command returns, shown as D11's first board. It
// reads nothing itself and changes nothing; it asks again whenever the window comes forward.
"use strict";

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

function length(seconds) {
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

function holdings(session, words) {
  const holds = document.createElement("div");
  holds.className = "holds";
  if (session.state === "recording") holds.append(chip(words.recording, "live"));
  if (session.state === "interrupted") holds.append(chip(words.interrupted, "cut"));
  if (session.minutes) holds.append(chip(words.minutes));
  if (session.transcript) holds.append(chip(words.transcript));
  holds.append(session.audio > 0 ? chip(fill(words.audio, size(session.audio, words))) : chip(words.audio_freed, "gone"));
  return holds;
}

function row(session, words) {
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
  link.append(when, holdings(session, words));
  item.append(link);
  return item;
}

function show({ words, folder, sessions }) {
  document.documentElement.lang = words.locale;
  document.getElementById("meetings").textContent = words.meetings;
  document.getElementById("sections").setAttribute("aria-label", words.sections);
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
  for (const session of sessions) {
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
    current.list.append(row(session, words));
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

const folded = (text) => text.normalize("NFD").replace(/\p{M}/gu, "").toLowerCase();

function transcriptCard(read, words) {
  const card = element("aside", "card transcript");
  card.setAttribute("aria-label", words.transcript_title);
  const top = element("div", "top");
  top.append(element("h2", "label", words.transcript_title));
  card.append(top);
  if (read.state !== "present") {
    card.append(element("p", "none", read.state === "absent" ? words.no_transcript : words.unreadable));
    return card;
  }
  const search = element("input", "search");
  search.type = "search";
  search.id = "search";
  search.placeholder = words.search;
  search.setAttribute("aria-label", words.search);
  top.append(search);
  card.append(element("p", "maker", fill(words.heard_by, read.model || read.engine || "—")));
  if (!read.complete) card.append(element("p", "note", words.being_made));
  if (read.unread > 0) card.append(element("p", "note", fill(words.unread, read.unread)));
  const lines = element("ol", "lines");
  for (const segment of read.segments) {
    const line = element("li");
    line.dataset.track = segment.track;
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
      line.hidden = wanted !== "" && !folded(line.textContent).includes(wanted);
      shown += line.hidden ? 0 : 1;
    }
    nothing.hidden = shown > 0;
  });
  card.append(lines, nothing);
  return card;
}

function showMeeting({ session, transcript }, words) {
  const page = document.getElementById("meeting");
  const top = element("div", "meeting-top");
  const back = element("a", "back", `‹ ${words.back}`);
  back.href = "#/";
  const open = element("button", "open", words.open_folder);
  open.type = "button";
  open.addEventListener("click", () => window.__TAURI__.core.invoke("open_folder", { name: nameOf(session) }));
  top.append(back, open);
  const title = element("section", "meeting-title");
  const named = element("div", "named");
  named.append(element("h1", "", heading(session, words)));
  if (session.seconds !== null) named.append(element("span", "length", length(session.seconds)));
  title.append(named, holdings(session, words));
  const columns = element("div", "columns");
  columns.append(transcriptCard(transcript, words));
  // Shown again when the window comes forward: a search in progress is kept.
  const searching = document.getElementById("search")?.value ?? "";
  page.replaceChildren(top, title, columns);
  const search = document.getElementById("search");
  if (search && searching) {
    search.value = searching;
    search.dispatchEvent(new Event("input"));
  }
}

let words = null;

async function refresh() {
  const listing = await window.__TAURI__.core.invoke("listed");
  words = listing.words;
  show(listing);
  const wanted = location.hash.match(/^#\/meeting\/(.+)$/);
  const meeting = document.getElementById("meeting");
  document.getElementById("list").hidden = Boolean(wanted);
  meeting.hidden = !wanted;
  if (wanted) showMeeting(await window.__TAURI__.core.invoke("opened", { name: decodeURIComponent(wanted[1]) }), words);
}

refresh();
window.addEventListener("focus", refresh);
window.addEventListener("hashchange", () => {
  refresh();
  window.scrollTo(0, 0);
});
