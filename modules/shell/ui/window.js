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

function minutesCard(read, words) {
  const card = element("article", "card minutes");
  card.append(element("h2", "label", words.minutes_title));
  if (read.state !== "present") {
    card.append(element("p", "none", read.state === "absent" ? words.no_minutes : words.unreadable));
    return card;
  }
  const made = read.made && dayOf({ instant: Date.parse(read.made) / 1000 || null });
  const when = made ? new Intl.DateTimeFormat(words.locale, { dateStyle: "medium", timeStyle: "short" }).format(made) : "—";
  card.append(element("p", "maker", fill(words.written_by, read.model || "—", read.provider || "—", when)));
  // The minutes as written, rendered by the shell: raw HTML in them is text there, a link its words.
  const body = element("div", "body");
  body.innerHTML = read.body;
  card.append(body);
  return card;
}

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
      line.hidden = wanted !== "" && !folded(line.querySelector(".text").textContent).includes(wanted);
      shown += line.hidden ? 0 : 1;
    }
    nothing.hidden = shown > 0;
  });
  card.append(lines, nothing);
  return card;
}

function showMeeting({ session, transcript, minutes }, words) {
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
  const said = transcript.state === "present" ? transcript.segments : [];
  const total = session.seconds || said.reduce((end, s) => Math.max(end, s.end), 0) || 1;
  const drawn = element("section", "card drawn");
  const legend = element("div", "legend");
  legend.append(element("span", "here", words.here_described), element("span", "remote", words.remote_described));
  drawn.append(relief(said, total, words), legend);
  const columns = element("div", "columns");
  columns.append(minutesCard(minutes, words), transcriptCard(transcript, words));
  // Shown again when the window comes forward: a search in progress in this meeting is kept.
  const searching = page.dataset.name === nameOf(session) ? (document.getElementById("search")?.value ?? "") : "";
  page.dataset.name = nameOf(session);
  page.replaceChildren(top, title, drawn, columns);
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
  const box = element("div", "minutes-step");
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
    for (const name of minutes.ollama) add(group, `ollama:${name}`, name);
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
  const where = element("p", "says");
  const field = (id, text, type, value) => {
    const wrap = element("label", "field");
    wrap.htmlFor = id;
    const input = element("input");
    input.id = id;
    input.type = type;
    input.value = value || "";
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
    await refresh();
    const said = document.getElementById("not-saved");
    said.textContent = failed ?? "";
    said.hidden = failed === null;
  };
  const draw = () => {
    fields.replaceChildren();
    const value = pick.value;
    if (value.startsWith("ollama:")) {
      where.textContent = words.on_this_computer;
      return;
    }
    if (value === "none") {
      where.textContent = "";
      return;
    }
    where.textContent = words.where_online;
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
  pick.addEventListener("change", () => {
    if (pick.value.startsWith("ollama:")) save(minutes.ollama_address, pick.value.slice("ollama:".length), null);
    draw();
  });
  draw();
  box.append(row, fields, where);
  // Tester: the choice saved, asked whether it answers — one sentence of Summit's, said first.
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
    test.append(button, tested);
    box.append(test);
  }
  return box;
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
    await refresh();
    const said = document.getElementById("not-saved");
    said.textContent = failed ?? "";
    said.hidden = failed === null;
  };
  const parts = [];
  if (locked) parts.push(element("p", "card note warn", fill(words.settings_unreadable, shown.unreadable)));
  const failed = element("p", "card note warn");
  failed.id = "not-saved";
  failed.hidden = true;
  parts.push(failed);

  const after = element("fieldset", "card group");
  after.append(element("legend", "", words.after_title));
  for (const [value, said] of [["nothing", words.after_nothing], ["transcript", words.after_transcript], ["minutes", words.after_minutes]]) {
    const one = choice("after", value, said, shown.after === value, locked);
    one.querySelector("input").addEventListener("change", () => save({ after: value }));
    after.append(one);
  }
  parts.push(after);

  const models = element("section", "card group");
  models.append(element("h2", "", words.models_title));
  const row = element("div", "row");
  const label = element("label", "name", words.transcription);
  label.htmlFor = "model";
  const pick = element("select");
  pick.id = "model";
  pick.disabled = locked;
  for (const model of shown.models) {
    const option = element("option", "", `${model.name} — ${size(model.bytes, words)}`);
    option.value = model.path;
    option.selected = model.path === shown.model;
    pick.append(option);
  }
  pick.addEventListener("change", () => save({ model: pick.value }));
  row.append(label, pick);
  models.append(row, element("p", "says", words.on_this_computer));
  models.append(minutesRow(minutes, words, locked));
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
  const keep = element("div", "row");
  const keepSaid = element("label", "said");
  keepSaid.htmlFor = "keep";
  keepSaid.append(element("span", "name", words.keep_audio[0]), element("span", "says", words.keep_audio[1]));
  const keepPick = element("select");
  keepPick.id = "keep";
  keepPick.disabled = locked;
  const days = new Set([90, 30]);
  if (shown.keep_audio !== "always") days.add(shown.keep_audio.days);
  const options = [["always", words.keep_always], ...[...days].sort((a, b) => b - a).map((n) => [String(n), fill(words.keep_days, n)])];
  for (const [value, text] of options) {
    const option = element("option", "", text);
    option.value = value;
    option.selected = shown.keep_audio === "always" ? value === "always" : value === String(shown.keep_audio.days);
    keepPick.append(option);
  }
  keepPick.addEventListener("change", () =>
    save({ keep_audio: keepPick.value === "always" ? "always" : { days: Number(keepPick.value) } }),
  );
  keep.append(keepSaid, keepPick);
  // Where the sessions go: typed, or chosen in the system's own dialog.
  const where = element("div", "row folder");
  const whereSaid = element("label", "said");
  whereSaid.htmlFor = "sessions";
  whereSaid.append(element("span", "name", words.sessions_folder[0]), element("span", "says", words.sessions_folder[1]));
  const path = element("input");
  path.id = "sessions";
  path.type = "text";
  path.value = shown.sessions;
  path.disabled = locked;
  path.spellcheck = false;
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
  where.append(whereSaid, path, choose);
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
  more.append(remind, keep, where, keys);
  parts.push(more);
  page.replaceChildren(...parts);
}

let words = null;

async function refresh() {
  const listing = await window.__TAURI__.core.invoke("listed");
  words = listing.words;
  show(listing);
  const wanted = location.hash.match(/^#\/meeting\/(.+)$/);
  const settings = location.hash === "#/settings";
  let opened = null;
  if (wanted) {
    // A meeting no longer there, or a name that is none, goes back to the list.
    try {
      opened = await window.__TAURI__.core.invoke("opened", { name: decodeURIComponent(wanted[1]) });
    } catch {
      history.replaceState(null, "", "#/");
    }
  }
  document.getElementById("list").hidden = opened !== null || settings;
  document.getElementById("meeting").hidden = opened === null;
  document.getElementById("settings").hidden = !settings;
  document.getElementById("meetings").setAttribute("aria-current", settings ? "false" : "page");
  document.getElementById("settings-link").setAttribute("aria-current", settings ? "page" : "false");
  if (opened) showMeeting(opened, words);
  if (settings) {
    const [shown, minutes] = await Promise.all([
      window.__TAURI__.core.invoke("settings_shown"),
      window.__TAURI__.core.invoke("minutes_shown"),
    ]);
    showSettings(shown, minutes, words);
  }
}

refresh();
window.addEventListener("focus", refresh);
window.addEventListener("hashchange", () => {
  refresh();
  window.scrollTo(0, 0);
});
