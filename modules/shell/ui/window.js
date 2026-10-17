// The window: what the shell's `listed` command returns, shown as D11's first board. It
// reads nothing itself and changes nothing; it asks again whenever the window comes forward.
"use strict";

const fill = (template, ...values) => values.reduce((text, value) => text.replace("{}", value), template);

function size(bytes, words) {
  let unit = 0;
  while (bytes >= 1000 && unit < 3) { bytes /= 1000; unit += 1; }
  const digits = bytes < 10 && unit > 0 ? 1 : 0;
  const number = new Intl.NumberFormat(words.locale, { maximumFractionDigits: digits }).format(bytes);
  return `${number} ${words.units[unit]}`;
}

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

function row(session, words) {
  const day = new Date(session.started);
  const item = document.createElement("li");
  item.className = "card session";
  item.dataset.state = session.state;
  const when = document.createElement("div");
  when.className = "when";
  const title = document.createElement("span");
  title.className = "day";
  const date = new Intl.DateTimeFormat(words.locale, { weekday: "short", day: "numeric", month: "short" }).format(day);
  const time = new Intl.DateTimeFormat(words.locale, { hour: "2-digit", minute: "2-digit" }).format(day);
  title.textContent = `${date} · ${time}`;
  when.append(title);
  if (session.seconds !== null) {
    const lasted = document.createElement("span");
    lasted.className = "length";
    lasted.textContent = length(session.seconds);
    when.append(lasted);
  }
  const holds = document.createElement("div");
  holds.className = "holds";
  if (session.state === "recording") holds.append(chip(words.recording, "live"));
  if (session.state === "interrupted") holds.append(chip(words.interrupted, "cut"));
  if (session.minutes) holds.append(chip(words.minutes));
  if (session.transcript) holds.append(chip(words.transcript));
  holds.append(session.audio > 0 ? chip(fill(words.audio, size(session.audio, words))) : chip(words.audio_freed, "gone"));
  item.append(when, holds);
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
    const name = group(new Date(session.started), now, words);
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

async function refresh() {
  show(await window.__TAURI__.core.invoke("listed"));
}

refresh();
window.addEventListener("focus", refresh);
