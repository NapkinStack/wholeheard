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
  return holds;
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
      button.replaceWith(element("p", "note", words.asked));
    } catch (why) {
      button.replaceWith(element("p", "note warn", fill(words.not_saved, why)));
    }
  });
  card.append(button);
}

function redoPanel(choices, session, words) {
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
      go.replaceWith(element("p", "note", words.asked));
    } catch (why) {
      go.replaceWith(element("p", "note warn", fill(words.not_saved, why)));
    }
  });
  panel.append(list, element("p", "says", words.redo_note), go);
  return panel;
}

function minutesCard(read, words, session, transcript) {
  const card = element("article", "card minutes");
  card.append(element("h2", "label", words.minutes_title));
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
    redo.replaceWith(redoPanel(choices, session, words));
  });
  card.querySelector("h2").after(redo);
  const made = read.made && dayOf({ instant: Date.parse(read.made) / 1000 || null });
  // As the board says it: the day of the week, no year, and the model as people say it.
  const options = { weekday: "short", day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" };
  const when = made ? new Intl.DateTimeFormat(words.locale, options).format(made) : "—";
  card.append(element("p", "maker", fill(words.written_by, read.model ? modelSaid(read.model) : "—", read.provider || "—", when)));
  // The minutes as written, rendered by the shell: raw HTML in them is text there, a link its words.
  const body = element("div", "body");
  body.innerHTML = read.body;
  card.append(body);
  return card;
}

const folded = (text) => text.normalize("NFD").replace(/\p{M}/gu, "").toLowerCase();

function transcriptCard(read, words, session) {
  const card = element("aside", "card transcript");
  card.setAttribute("aria-label", words.transcript_title);
  const top = element("div", "top");
  top.append(element("h2", "label", words.transcript_title));
  card.append(top);
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
  card.append(element("p", "maker", fill(words.heard_by, read.model ? spoken(read.model) : read.engine || "—")));
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
  drawn.append(relief(said, total, words), legend);
  const columns = element("div", "columns");
  columns.append(minutesCard(minutes, words, session, transcript), transcriptCard(transcript, words, session));
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
  const where = element("p", "status");
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
      where.className = "status here";
      where.textContent = words.on_this_computer;
      return;
    }
    if (value === "none") {
      where.textContent = "";
      return;
    }
    where.className = "status online";
    let shown = minutes.provider ?? "";
    try {
      const address = new URL(value === "custom" ? minutes.provider : minutes.online[Number(value.split(":")[1])].address);
      shown = `${address.host}${address.pathname}`;
    } catch {}
    where.replaceChildren(words.where_online, element("code", "tag", shown));
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
    if (pick.value.startsWith("ollama:")) save(minutes.ollama_address, pick.value.slice("ollama:".length), null);
    draw();
    place();
  });
  draw();
  box.append(row, where, fields);
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
    test.append(tested);
    box.append(test);
    place = () => (fields.childElementCount > 0 ? fields.append(button) : test.prepend(button));
    place();
  }
  return box;
}

// A Whisper model's file named as people say it: `ggml-large-v3-turbo-q8_0.bin` is Whisper large
// v3 turbo. A file of another kind, or outside Summit's folder, keeps its own name.
// A provider's model named as people say it: `anthropic/claude-sonnet` is Claude Sonnet; its
// version and size stay as written (`mistral:7b` is Mistral 7b).
function modelSaid(model) {
  const name = model.split("/").pop();
  return name
    .split(/[-_:\s]+/)
    .filter(Boolean)
    .map((word) => (word.toLowerCase() === "gpt" ? "GPT" : /^[a-z]/.test(word) ? word[0].toUpperCase() + word.slice(1) : word))
    .join(" ");
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
  // Shown as the board says it — Dossier personnel / Summit —; a click makes it a path to type.
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
  more.append(remind, keys, where, keep);
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

let words = null;

async function refresh() {
  const listing = await window.__TAURI__.core.invoke("listed");
  words = listing.words;
  show(listing);
  const wanted = location.hash.match(/^#\/meeting\/(.+)$/);
  const settings = location.hash === "#/settings";
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
  if (wanted) {
    // A meeting no longer there, or a name that is none, goes back to the list.
    try {
      opened = await window.__TAURI__.core.invoke("opened", { name: decodeURIComponent(wanted[1]) });
    } catch {
      history.replaceState(null, "", "#/");
    }
  }
  document.getElementById("list").hidden = opened !== null || settings || agreement !== null;
  document.getElementById("agree").hidden = agreement === null;
  // Shown or emptied: never a question left hidden under another page.
  if (agreement) showAgreement(decodeURIComponent(agreeing[1]), agreement, words);
  else document.getElementById("agree").replaceChildren();
  document.getElementById("meeting").hidden = opened === null;
  document.getElementById("settings").hidden = !settings;
  document.getElementById("meetings").setAttribute("aria-current", settings || agreement ? "false" : "page");
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
  const { recording, shortcut } = asked;
  document.getElementById("record-said").textContent = recording ? words.record_stop : words.record_start;
  document.getElementById("record").classList.toggle("live", recording);
  const keys = document.getElementById("header-keys");
  keys.textContent = shortcut ?? "";
  keys.hidden = !shortcut;
}, 1500);
refresh();
window.addEventListener("focus", refresh);
window.addEventListener("hashchange", () => {
  refresh();
  window.scrollTo(0, 0);
});
