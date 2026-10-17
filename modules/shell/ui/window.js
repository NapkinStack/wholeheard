// The window: what the shell's `listed` command returns, shown as D11's first board. It reads
// nothing itself and changes nothing; it asks again whenever the window comes forward.
"use strict";

function show({ words }) {
  document.documentElement.lang = words.locale;
  document.getElementById("meetings").textContent = words.meetings;
  document.getElementById("sections").setAttribute("aria-label", words.sections);
}

async function refresh() {
  show(await window.__TAURI__.core.invoke("listed"));
}

refresh();
window.addEventListener("focus", refresh);
