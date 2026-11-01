"""The window's end-to-end scenario: Wholeheard started with `--window` by tauri-driver, the page
read as WebKitGTK shows it, over WebDriver. Sessions are made here; nothing is recorded.

    python3 scripts/e2e_window.py <wholeheard-binary> <work-folder>

Prints one line per check, `ok` or `FAIL`, and exits 1 on any failure.
"""

from __future__ import annotations

import datetime as dt
import hashlib
import http.server
import json
import math
import os
import re
import shutil
import signal
import socket
import struct
import subprocess
import sys
import threading
import time
import traceback
import urllib.request
import wave
from pathlib import Path

from selenium import webdriver
from selenium.common.exceptions import (
    ElementNotInteractableException, NoSuchWindowException, StaleElementReferenceException, TimeoutException, WebDriverException,
)
from selenium.webdriver.common.action_chains import ActionChains
from selenium.webdriver.common.by import By
from selenium.webdriver.common.keys import Keys
from selenium.webdriver.common.options import ArgOptions
from selenium.webdriver.remote.client_config import ClientConfig
from selenium.webdriver.support.expected_conditions import staleness_of
from selenium.webdriver.support.select import Select
from selenium.webdriver.support.wait import WebDriverWait
from urllib3.exceptions import HTTPError, ReadTimeoutError

BINARY, WORK = Path(sys.argv[1]), Path(sys.argv[2])
# wholeheard-summary played here: it says what it was given — its arguments, the key in its
# environment, the context it would send (D59), read where the real one reads it. A test refuses a
# key at openrouter.ai and answers elsewhere; minutes are written, naming their context as the
# real ones do, the way it was called kept beside them. Nothing leaves the machine. Where a scenario
# stands a service in ($STANDIN, D60), a request for a provider elsewhere than this computer is sent
# to it, as the real one would send it there, and the minutes name the host that answered.
SUMMARY = WORK / "wholeheard-summary"
SUMMARY.write_text(
    """#!/bin/sh
here=$(dirname "$0")
printf '%s\\n' "$*" > "$here/summary-args"
printf '%s' "${WHOLEHEARD_PROVIDER_KEY:-none}" > "$here/summary-key"
if [ "$1" != --test ]; then
  provider=local
  case "$3" in http://127.0.0.1*|http://localhost*) ;; *) provider=$(python3 -c "$HOST" "$3"); [ -n "$STANDIN" ] && python3 -c "$SEND" "$STANDIN/write";; esac
  [ -e "$here/slow-minutes" ] && sleep 3
  [ -e "$here/fail-minutes" ] && { echo '{"event": "failed", "code": "answer.not_minutes"}'; echo "wholeheard-summary: the provider's answer is not minutes" >&2; exit 1; }
  [ -e "$here/fail-plainly" ] && { echo "wholeheard-summary: a failure no code names" >&2; exit 1; }
  context=$(python3 -c "$CONTEXT" "$1")
  printf '%s' "$context" > "$here/summary-context"
  echo '{"event": "sending", "host": "h", "model": "m", "characters": 1}'
  named=""; [ -n "$STANDIN" ] && named="provider: $provider\\n"
  printf -- "---\\ncontract: minutes/v1\\n${named}model: %s\\ncontext: %s\\n---\\n\\n## A\\n\\n## B\\n\\n## C\\n" "$5" "$context" > "$1/minutes.md"
  echo '{"event": "written", "file": "x"}'
  exit 0
fi
[ -e "$here/test-code" ] && { echo '{"event": "sending", "host": "h", "model": "m", "characters": 1}'; echo '{"event": "failed", "code": "'"$(cat "$here/test-code")"'"}'; echo; echo "an earlier line" >&2; echo "wholeheard-summary: raw words of the provider" >&2; exit 1; }
case "$*" in *openrouter.ai*) echo "wholeheard-summary: the provider at openrouter.ai refused the key" >&2; exit 1;; esac
echo '{"event": "sending"}'
echo '{"event": "answered"}'
"""
)
SUMMARY.chmod(0o755)
# wholeheard-transcription played here: a whole transcript, naming the model it was given — a
# service's by its host, as the real one names it; where a scenario stands the service in
# ($STANDIN, D60), its request is sent to it, as the real one would send the sound there.
HEARING = WORK / "wholeheard-transcription"
HEARING.write_text(
    """#!/bin/sh
printf '%s\\n' "$*" > "$(dirname "$0")/hearing-args"
# Tester (D43, D63): the key it was given kept beside; a model that gives no times refused, as the real one says it.
if [ "$1" = --test ]; then
  printf '%s' "${WHOLEHEARD_PROVIDER_KEY:-none}" > "$(dirname "$0")/hearing-key"
  [ -e "$(dirname "$0")/test-code" ] && { echo '{"event": "started"}'; echo '{"event": "failed", "code": "'"$(cat "$(dirname "$0")/test-code")"'"}'; echo; echo "an earlier line" >&2; echo "wholeheard-transcription: raw words of the service" >&2; exit 1; }
  exit 0
fi
[ -e "$(dirname "$0")/fail-hearing" ] && { echo "wholeheard-transcription: the engine stopped" >&2; exit 1; }
engine=whisper.cpp
model=$(basename "$3" .bin)
if [ "$2" = --provider ]; then
  engine=$(python3 -c "$HOST" "$3"); model=$5
  [ -n "$STANDIN" ] && python3 -c "$SEND" "$STANDIN/hear"
fi
[ -e "$(dirname "$0")/slow-hearing" ] && sleep 3
context=$(python3 -c "$CONTEXT" "$1")
printf '{"type":"header","contract":"transcript/v1","engine":"%s","model":"%s","language":null,"context":%s}\\n{"type":"segment","speaker":"me","track":"local","start":0.0,"end":1.0,"text":"Refait avec %s."}\\n{"type":"complete","duration":1.0}\\n' "$engine" "$model" "$context" "$model" > "$1/transcript.jsonl"
"""
)
HEARING.chmod(0o755)
# The context of a session's folder as both steps read it — trimmed, none when nothing is left —,
# quoted as JSON, for the two programs played above.
os.environ["CONTEXT"] = (
    "import json, pathlib, sys; f = pathlib.Path(sys.argv[1], 'context.txt'); "
    "t = f.read_text().strip() if f.exists() else ''; print(json.dumps(t or None, ensure_ascii=False))"
)
# The host of a provider's address, and a request sent to a stand-in service, for the two above.
os.environ["HOST"] = "import sys, urllib.parse; print(urllib.parse.urlsplit(sys.argv[1]).hostname)"
os.environ["SEND"] = "import sys, urllib.request; urllib.request.urlopen(urllib.request.Request(sys.argv[1], data=b'x', method='POST'), timeout=5)"

# wholeheard-capture played here: it says it started, then each source's level ten times a second,
# until it is told to stop. No device, nobody recorded.
RECORDER = WORK / "wholeheard-capture"
RECORDER.write_text(
    """#!/bin/bash
folder="$2/2026-10-18_12-00"; mkdir -p "$folder"
printf '{"contract":"session-audio/v2","started":"2026-10-18T12:00:00+02:00","tracks":{}}' > "$folder/session.json"
me='"Mic"'
# No microphone to open, when the scenario says so (D49).
if [ -e "$2/waiting-me" ]; then echo '{"event":"unavailable","source":"me","reason":"none"}'; me='"unavailable"'; fi
echo '{"event":"started","folder":"'"$folder"'","sources":{"me":'"$me"',"others":"Speaker"}}'
n=0
while ! read -t 0.1 line; do
  n=$((n + 1))
  # Levels at silence as long as the scenario says so; what capture says of a silence, when it does (D49).
  if [ -e "$2/hush" ]; then echo '{"event":"level","source":"me","level":0}'; echo '{"event":"level","source":"others","level":0}'
  else
    [ -e "$2/waiting-me" ] || echo '{"event":"level","source":"me","level":0.'$((n % 9 + 1))'}'
    echo '{"event":"level","source":"others","level":0.5}'
  fi
  if [ -e "$2/nothing-me" ]; then rm "$2/nothing-me"; echo '{"event":"silent","source":"me","nothing":true}'; fi
  if [ -e "$2/silent-others" ]; then rm "$2/silent-others"; echo '{"event":"silent","source":"others"}'; fi
  if [ -e "$2/heard-me" ]; then rm "$2/heard-me"; echo '{"event":"heard","source":"me"}'; fi
  if [ -e "$2/heard-others" ]; then rm "$2/heard-others"; echo '{"event":"heard","source":"others"}'; fi
  # A device changed, or gone, when the scenario says so (D31, D33).
  if [ -e "$2/move" ]; then rm "$2/move"; echo '{"event":"device","source":"me","device":"AirPods Pro"}'; fi
  if [ -e "$2/lose" ]; then rm "$2/lose"; echo '{"event":"device","source":"others","device":null}'; fi
done
printf '{"contract":"session-audio/v2","started":"2026-10-18T12:00:00+02:00","stopped":"2026-10-18T12:00:05+02:00","tracks":{}}' > "$folder/session.json"
echo '{"event":"stopped","folder":"'"$folder"'","seconds":5.0}'
"""
)
RECORDER.chmod(0o755)
FAILED = []
# What Tester says of each cause its program names (D72), as `src/words.rs` says it in French.
TEST_CAUSES = [
    ("provider.refused_key", "Le fournisseur a refusé la clé\u00a0: saisissez-la de nouveau dans les réglages de Wholeheard."),
    ("provider.unknown_model", "Le fournisseur ne connaît pas ce modèle ou cette adresse\u00a0: choisissez-les de nouveau dans les réglages de Wholeheard."),
    ("provider.no_times", "Ce modèle ne donne pas les temps de ce qu’il entend\u00a0: choisissez-en un qui donne ceux de chaque segment dans les réglages de Wholeheard."),
    ("provider.no_credit", "Le fournisseur a refusé faute de crédit\u00a0: ajoutez du crédit chez lui, puis réessayez."),
    ("provider.too_many", "Le fournisseur refuse pour trop de demandes, ou un quota épuisé\u00a0: patientez, ou vérifiez votre forfait chez lui."),
    ("provider.down", "Le fournisseur répond qu’il est en panne\u00a0: réessayez plus tard. Sur cet ordinateur, ce modèle est peut-être trop gros pour la mémoire\u00a0: choisissez-en un plus petit."),
    ("provider.unreachable", "Le fournisseur n’a pas pu être joint\u00a0: vérifiez qu’il tourne, ou le réseau, puis réessayez."),
]
MONTHS_FR = "janvier février mars avril mai juin juillet août septembre octobre novembre décembre".split()
# The page's ground, as `ui/window.css` sets it in each theme.
GROUND = {"light": "rgb(242, 244, 247)", "dark": "rgb(21, 23, 28)"}


def check(what: str, held: bool, got: object = "") -> None:
    print(f"  {'ok  ' if held else 'FAIL'}  {what}" + ("" if held else f"\n        got: {got}"))
    if not held:
        FAILED.append(what)


def shot(browser, name: str) -> None:
    """The page as it is, saved where WHOLEHEARD_SCREENSHOTS says: for an eye against D11's boards."""
    folder = os.environ.get("WHOLEHEARD_SCREENSHOTS")
    if folder:
        Path(folder).mkdir(parents=True, exist_ok=True)
        browser.save_screenshot(str(Path(folder) / f"{name}.png"))


def choose(browser, element_id: str, value: str) -> None:
    """A choice in a list of the page as it is: drawn again under the driver — a choice just
    saved draws it anew —, the list is found afresh."""
    for _ in range(5):
        try:
            Select(browser.find_element(By.ID, element_id)).select_by_value(value)
            return
        except StaleElementReferenceException:
            time.sleep(0.2)
    Select(browser.find_element(By.ID, element_id)).select_by_value(value)


def press(browser, element_id: str) -> None:
    """A click on the page as it is: drawn again under the driver, the element is found afresh;
    found before it can be clicked — shown, laid out — it is clicked a moment later (#510)."""
    for _ in range(10):
        try:
            browser.find_element(By.ID, element_id).click()
            return
        except (StaleElementReferenceException, ElementNotInteractableException):
            time.sleep(0.2)
    browser.find_element(By.ID, element_id).click()


def panel_choice(browser, choices: str, index: int):
    """A redo panel's choice: its models come after the panel, so they are waited for."""
    wait_for(browser, lambda b: len(b.find_elements(By.CSS_SELECTOR, choices)) > index)
    return browser.find_elements(By.CSS_SELECTOR, choices)[index]


def shot_themed(browser, name: str, theme: str) -> None:
    """A screenshot in its theme: on a dark desktop, the light one's colours put on the page."""
    css = (Path(__file__).parent.parent / "ui" / "window.css").read_text()
    light = css[css.index(":root {") + len(":root {"):css.index("}", css.index(":root {"))].replace("\n", " ").strip()
    forced = theme == "light" and browser.execute_script("return matchMedia('(prefers-color-scheme: dark)').matches")
    if forced:
        browser.execute_script("document.documentElement.style.cssText = arguments[0] + '; color-scheme: light'", light)
        time.sleep(0.3)
    shot(browser, name)
    if forced:
        browser.execute_script("document.documentElement.style.cssText = ''")


def stop_from_small(browser) -> None:
    """Arrêter in the small window: the window goes with the recording, sometimes before the
    driver has answered the click."""
    try:
        browser.find_element(By.ID, "small-stop").click()
    except NoSuchWindowException:
        pass


def read(browser, reads):
    """What the page says, as it is: drawn again under the driver, it is read afresh (#409)."""
    for _ in range(5):
        try:
            return reads(browser)
        except StaleElementReferenceException:
            time.sleep(0.2)
    return reads(browser)


def wait_for(browser, holds, seconds: float = 10) -> bool:
    until = time.monotonic() + seconds
    while time.monotonic() < until:
        # The page drawn again between finding an element and reading it: not yet, look again.
        try:
            if holds(browser):
                return True
        except StaleElementReferenceException:
            pass
        time.sleep(0.1)
    return False


def free_port() -> int:
    with socket.socket() as probe:
        probe.bind(("127.0.0.1", 0))
        return probe.getsockname()[1]


def session(folder: Path, started: dt.datetime, minutes: float | None, files: dict[str, int], form: str = "v2") -> None:
    folder.mkdir(parents=True)
    record = {"contract": f"session-audio/{form}", "started": started.isoformat(timespec="seconds")}
    if minutes is not None:
        record["stopped"] = (started + dt.timedelta(minutes=minutes)).isoformat(timespec="seconds")
    record["sources" if form == "v1" else "tracks"] = {}
    (folder / "session.json").write_text(json.dumps(record))
    for name, size in files.items():
        (folder / name).parent.mkdir(parents=True, exist_ok=True)
        with open(folder / name, "wb") as made:
            made.truncate(size)


def instances() -> list[int]:
    """The processes of the Wholeheard under test."""
    found = []
    for process in Path("/proc").iterdir():
        try:
            if process.name.isdigit() and os.readlink(process / "exe") == str(BINARY.resolve()):
                found.append(int(process.name))
        except OSError:
            pass
    return found


def tray_entries() -> tuple[str, str, list[tuple[int, str]]]:
    """The tray's menu of the Wholeheard under test, as the desktop reads it: its bus, its path, and
    each entry's number and words (D64)."""
    watcher = ["org.kde.StatusNotifierWatcher", "/StatusNotifierWatcher", "org.kde.StatusNotifierWatcher"]
    listed = subprocess.run(["busctl", "--user", "get-property", *watcher, "RegisteredStatusNotifierItems"],
                            capture_output=True, text=True).stdout
    ours = set(instances())
    for item in (word.strip('"') for word in listed.split() if "@" in word and "wholeheard" in word):
        bus, path = item.split("@", 1)
        status = subprocess.run(["busctl", "--user", "status", bus], capture_output=True, text=True).stdout
        if not any(line == f"PID={pid}" for pid in ours for line in status.splitlines()):
            continue
        layout = subprocess.run(["busctl", "--user", "--json=short", "call", bus, f"{path}/Menu", "com.canonical.dbusmenu",
                                 "GetLayout", "iias", "0", "--", "-1", "0"], capture_output=True, text=True).stdout
        entries: list[tuple[int, str]] = []

        def walk(node) -> None:
            node = node["data"] if isinstance(node, dict) and "data" in node else node
            number, props, children = node
            if "label" in props:
                entries.append((number, props["label"]["data"]))
            for child in children:
                walk(child)

        walk(json.loads(layout)["data"][1])
        return bus, path, entries
    return "", "", []


def tray_labels() -> list[str]:
    """The words of the tray's menu of the Wholeheard under test (D64)."""
    return [label for _, label in tray_entries()[2]]


def tray_click(words: str) -> bool:
    """Clicks the tray's entry whose words begin so, as the desktop does (D60)."""
    bus, path, entries = tray_entries()
    number = next((n for n, label in entries if label.startswith(words)), None)
    if number is None:
        return False
    return subprocess.run(["busctl", "--user", "call", bus, f"{path}/Menu", "com.canonical.dbusmenu", "Event", "isvu",
                           str(number), "clicked", "s", "", "0"], capture_output=True).returncode == 0

class Window:
    """Wholeheard with its window, for as long as the `with` lasts."""

    def __init__(self, sessions: Path, language: str, more: str = "", ollama: str = "http://127.0.0.1:9", small: bool = False,
                 welcomed: bool = True, args: tuple[str, ...] = ("--window",)) -> None:
        config = WORK / f"config-{language}-{sessions.name}"
        (config / "wholeheard").mkdir(parents=True, exist_ok=True)
        self.file = config / "wholeheard" / "settings.toml"
        # The full window, as the scenarios before D35 knew it; the small one, opened first, where asked.
        self.small = small
        opened = "" if small else 'window = "full"\n'
        # Set up already (D58): a scenario with no meeting yet is not met by the first run's assistant.
        greeted = "welcomed = true\n" if welcomed else ""
        self.args = list(args)
        self.file.write_text(f'sessions = "{sessions}"\n{opened}{greeted}{more}')
        self.env = dict(os.environ, XDG_CONFIG_HOME=str(config), XDG_DATA_HOME=str(WORK / "data"), XDG_STATE_HOME=str(WORK / "state"), LANGUAGE="")
        # An Ollama and a keyring of the scenario's, never the user's own.
        self.env.update(LANG=language, LC_ALL=language, WHOLEHEARD_OLLAMA=ollama, WHOLEHEARD_KEYRING="memory", WHOLEHEARD_SUMMARY=str(SUMMARY), WHOLEHEARD_TRANSCRIPTION=str(HEARING), WHOLEHEARD_CAPTURE=str(RECORDER))

    def __enter__(self) -> webdriver.Remote:
        # One Wholeheard per session: one still leaving takes this start's arguments and this one
        # leaves, its session never opened (#510). Waited for.
        until = time.monotonic() + 10
        while instances() and time.monotonic() < until:
            time.sleep(0.1)
        port, native = free_port(), free_port()
        self.driver_process = subprocess.Popen(
            ["tauri-driver", "--port", str(port), "--native-port", str(native)],
            env=self.env,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        options = ArgOptions()
        options.set_capability("browserName", "wry")
        options.set_capability("tauri:options", {"application": str(BINARY), "args": self.args})
        # The driver answers once it is up: until then, connections are refused or cut.
        # A command the driver never answers breaks off after a minute, on the line that sent it —
        # four minutes for a GET or a DELETE, which urllib3 tries again three times —, never the
        # run's twenty minutes, silent (#510).
        address = f"http://127.0.0.1:{port}"
        for _ in range(50):
            try:
                self.browser = webdriver.Remote(address, options=options, client_config=ClientConfig(address, timeout=60))
                break
            except ReadTimeoutError:
                self.driver_process.terminate()
                self.driver_process.wait(timeout=10)
                raise RuntimeError(f"tauri-driver opened no session in a minute; Wholeheard's processes: {instances()}") from None
            except (WebDriverException, HTTPError):
                time.sleep(0.2)
        else:
            self.driver_process.terminate()
            self.driver_process.wait(timeout=10)
            raise SystemExit("tauri-driver did not answer")
        # Whichever window opened: the small one at rest, or the full one's sections named.
        wait_for(self.browser, lambda b: b.find_elements(By.ID, "small-record") or b.execute_script("return document.getElementById('settings-link').textContent !== ''"))
        return self.browser

    def __exit__(self, *_) -> None:
        try:
            self.browser.quit()
        finally:
            self.driver_process.terminate()
            self.driver_process.wait(timeout=10)
            # A session that never answered leaves the Wholeheard under test behind: ended, or it takes
            # the next start's arguments and the next session never opens (#510). Only this binary's.
            for pid in instances():
                try:
                    os.kill(pid, signal.SIGTERM)
                except ProcessLookupError:
                    pass


def asking_less_motion(window: Window) -> Window:
    """A window whose desktop asks for less motion, in the scenario's own settings, never the user's:
    WebKitGTK reads `org.gnome.desktop.interface enable-animations`, from a key file with this backend
    (`gtk-3.0/settings.ini` alone is not enough: the query stays false under Wayland)."""
    settings = Path(window.env["XDG_CONFIG_HOME"]) / "glib-2.0" / "settings"
    settings.mkdir(parents=True, exist_ok=True)
    (settings / "keyfile").write_text("[org/gnome/desktop/interface]\nenable-animations=false\n")
    window.env.update(GSETTINGS_BACKEND="keyfile")
    return window


LESS_MOTION = "return matchMedia('(prefers-reduced-motion: reduce)').matches"
# From here on, whatever starts to move, in the running page.
WATCH_MOTION = ("window.__moved = []; for (const kind of ['animationstart', 'transitionrun'])"
                " document.addEventListener(kind, (e) => window.__moved.push(`${kind} ${e.animationName ?? e.propertyName} ${e.target.className || e.target.id}`), true)")
# What moved so far, and every element or pseudo-element that would move by its computed style.
MOVING = ("const moves = [...window.__moved], some = (t) => t.split(',').some((d) => parseFloat(d) > 0);"
          "for (const e of document.querySelectorAll('*')) for (const p of [null, '::before', '::after']) {"
          " const c = getComputedStyle(e, p), named = `${e.tagName}.${e.className.baseVal ?? e.className}#${e.id}${p ?? ''}`;"
          " if (c.animationName !== 'none' && some(c.animationDuration)) moves.push(`animation ${c.animationName} ${named}`);"
          " if (c.transitionProperty !== 'none' && some(c.transitionDuration)) moves.push(`transition ${c.transitionProperty} ${c.transitionDuration} ${named}`); }"
          "return moves")


# The run's own output, made by e2e.sh: the window's sound goes there, never to the speakers.
QUIET = os.environ.get("PULSE_SINK", "")
# What WirePlumber remembers of an application's sound, by its name: the user's own Wholeheard
# shares it, so a run that changes a volume there changes theirs.
REMEMBERED = Path(os.environ.get("XDG_STATE_HOME") or Path.home() / ".local" / "state") / "wireplumber" / "stream-properties"


def remembered() -> str | None:
    """The volume the system keeps for Wholeheard's sound, as its line says it."""
    if not REMEMBERED.exists():
        return None
    return next((line for line in REMEMBERED.read_text().splitlines() if line.startswith("Output/Audio:application.name:wholeheard=")), None)


def same_volume(one: str | None, other: str | None) -> bool:
    """The same volume remembered: the system writes the line's keys in no fixed order."""
    read = lambda line: line and json.loads(line.split("=", 1)[1])
    return read(one) == read(other)


def played_to(window: "Window") -> set[str]:
    """The outputs the sound of this window — its web view, under its driver — is linked to."""
    def under(pid: int) -> bool:
        while pid > 1:
            if pid == window.driver_process.pid:
                return True
            try:
                pid = int(Path(f"/proc/{pid}/stat").read_text().rsplit(")", 1)[1].split()[1])
            except (OSError, IndexError, ValueError):
                return False
        return False

    objects = json.loads(subprocess.run(["pw-dump"], capture_output=True, text=True, check=True).stdout)
    nodes = {o["id"]: (o.get("info") or {}).get("props") or {} for o in objects if o.get("type") == "PipeWire:Interface:Node"}
    streams = {i for i, props in nodes.items() if props.get("media.class") == "Stream/Output/Audio"
               and under(int(props.get("application.process.id") or 0))}
    return {nodes.get(o["info"]["input-node-id"], {}).get("node.name", "?") for o in objects
            if o.get("type") == "PipeWire:Interface:Link" and o["info"]["output-node-id"] in streams}


def rows(browser) -> list[tuple[str, str, str]]:
    """Each listed session: its group's heading, its first line, and what it holds."""
    return browser.execute_script(
        """return [...document.querySelectorAll('.group')].flatMap(g => [...g.querySelectorAll('.session')].map(s =>
             [g.querySelector('h2').textContent, s.querySelector('.day').textContent,
              [...s.querySelectorAll('.length, .chip')].filter(c => !c.hidden).map(c => c.textContent).join('|')]))"""
    )


def week_of(day: dt.datetime) -> dt.datetime:
    """Monday 00:00 of the week a day is in, on this machine's clock — as the page reckons it."""
    start = day.replace(hour=0, minute=0, second=0, microsecond=0)
    return start - dt.timedelta(days=start.weekday())


def group(day: dt.datetime | None, now: dt.datetime) -> str:
    """The heading the page puts a session under, in French — the page's own rule, here."""
    if day is None:
        return "Date inconnue"
    this_week = week_of(now)
    if day >= this_week:
        return "Cette semaine"
    if day >= this_week - dt.timedelta(days=7):
        return "La semaine dernière"
    if (day.year, day.month) == (now.year, now.month):
        return f"Plus tôt en {MONTHS_FR[day.month - 1]}"
    return f"{MONTHS_FR[day.month - 1].capitalize()} {day.year}"


def model_offered(now: dt.datetime) -> None:
    """T. The default model missing: the window offers the tray's own fetch where it says so (D36)."""
    print("T. the transcription model missing: offered in the window too, where it says so (D36)")
    served, right = WORK / "served", os.urandom(300_000)
    served.mkdir()
    (served / "model.bin").write_bytes(os.urandom(300_000))  # served first: not the published file
    asked = []

    # Served slowly — 30 kB every 0.3 s, three seconds a fetch — so that its coming is seen.
    class Slow(http.server.SimpleHTTPRequestHandler):
        def __init__(self, *args, **kwargs):
            super().__init__(*args, directory=str(served), **kwargs)

        def log_message(self, *_):
            asked.append(self.path)

        def copyfile(self, source, sink):
            while chunk := source.read(30_000):
                sink.write(chunk)
                sink.flush()
                time.sleep(0.3)

    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Slow)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    host = f"127.0.0.1:{server.server_address[1]}"
    meetings, data = WORK / "fetching", WORK / "data-fetching"
    session(meetings / "2026-10-25_10-00", now - dt.timedelta(hours=1), 5, {"local.wav": 2000})
    heard = meetings / "2026-10-25_10-00" / "transcript.jsonl"
    fetched = data / "wholeheard" / "models" / "ggml-large-v3-turbo-q8_0.bin"
    window = Window(meetings, "fr_FR.UTF-8")
    window.env.update(XDG_DATA_HOME=str(data), WHOLEHEARD_MODEL_PUBLISHED=f"http://{host}/model.bin 300000 {hashlib.sha256(right).hexdigest()}")
    said = "return document.getElementById('model-state')?.innerText || ''"
    with window as browser:
        browser.find_element(By.ID, "settings-link").click()
        offered = wait_for(browser, lambda b: b.find_elements(By.ID, "model-fetch"))
        label = browser.find_element(By.ID, "model-fetch").text if offered else ""
        check("Settings offer the missing model, with its size and host", offered and label == f"Télécharger le modèle (300 ko, {host})", label)
        check("nothing is fetched before the click", asked == [], asked)
        # Coming back to the window draws it anew, by design: the window's focus waited for first —
        # the desktop being in use, it may come late (#510).
        wait_for(browser, lambda b: b.execute_script("return document.hasFocus()"), 5)
        time.sleep(0.5)
        button = browser.find_element(By.ID, "model-fetch")
        time.sleep(3.5)
        check("in Settings too, the offer is not drawn again while nothing changes", not staleness_of(button)(browser))
        browser.find_element(By.ID, "model-fetch").click()
        coming = wait_for(browser, lambda b: "En cours de téléchargement" in b.execute_script(said) and not b.find_elements(By.ID, "model-fetch"), 3)
        check("once clicked, it is said coming, and not offered again", coming, browser.execute_script(said))
        again = wait_for(browser, lambda b: b.find_elements(By.ID, "model-fetch"), 15)
        check("a file that is not the published one: nothing kept, offered again", again and not fetched.exists() and asked != [], asked)
        (served / "model.bin").write_bytes(right)
        press(browser, "model-fetch")
        ready = wait_for(browser, lambda b: "Prêt" in b.execute_script(said), 15)
        check("fetched from Settings, the model is ready there", ready and fetched.read_bytes() == right, browser.execute_script(said))
        time.sleep(1)
        check("with none in line, no transcript is made", not heard.exists())
        # The model gone again, a meeting waits for it: offered on its page, where it waits.
        fetched.unlink()
        browser.execute_script("location.hash = '#/meeting/2026-10-25_10-00'")
        wait_for(browser, lambda b: b.find_elements(By.ID, "make-transcript"))
        press(browser, "make-transcript")
        waits = "return document.getElementById('transcript-state')?.innerText || ''"
        offered = wait_for(browser, lambda b: "En attente du modèle Whisper" in b.execute_script(waits) and b.find_elements(By.CSS_SELECTOR, "#transcript-state #model-fetch"))
        check("a meeting waiting for the model offers it on its page", offered, browser.execute_script(waits))
        # The tray's entry that fetches it follows the language chosen, at once (D64).
        speak = lambda code: browser.execute_script("return window.__TAURI__.core.invoke('settings_saved', {change: {language: arguments[0]}})", code)
        tray = lambda b, said: any(label.startswith(said) for label in tray_labels())
        offered = wait_for(browser, lambda b: tray(b, "Télécharger le modèle ("), 5)
        speak("en")
        english = wait_for(browser, lambda b: tray(b, "Download the model ("), 5)
        speak("")
        check("the tray's entry that fetches it speaks the language chosen, at once", offered and english
              and wait_for(browser, lambda b: tray(b, "Télécharger le modèle"), 5), tray_labels())
        # Offered, it stays the same button across the page's readings: a focus on it is kept.
        button = browser.find_element(By.ID, "model-fetch")
        time.sleep(3.5)
        check("the offer is not drawn again while nothing changes", not staleness_of(button)(browser))
        browser.find_element(By.ID, "model-fetch").click()
        coming = wait_for(browser, lambda b: "en cours de téléchargement" in b.execute_script(waits) and not b.find_elements(By.ID, "model-fetch"), 3)
        check("while it comes, the meeting says so and offers it no more", coming, browser.execute_script(waits))
        made = wait_for(browser, lambda b: heard.exists(), 15)
        check("fetched from the meeting, its transcript is made", made and fetched.read_bytes() == right)
    # A model the user named, not on this computer: theirs, nothing offered.
    named = Window(meetings, "fr_FR.UTF-8", f'model = "{WORK / "mine.bin"}"\n')
    named.env.update(XDG_DATA_HOME=str(WORK / "data-named"), WHOLEHEARD_MODEL_PUBLISHED=window.env["WHOLEHEARD_MODEL_PUBLISHED"])
    with named as browser:
        browser.find_element(By.ID, "settings-link").click()
        drawn = wait_for(browser, lambda b: "Pas sur cet ordinateur" in b.execute_script(said))
        time.sleep(1)
        check("a model the user named is not offered: theirs", drawn and not browser.find_elements(By.ID, "model-fetch")
              and "proposé" not in browser.execute_script(said), browser.execute_script(said))
        before = len(asked)
        browser.execute_script("window.__TAURI__.core.invoke('model_fetch')")
        time.sleep(1.5)
        check("asked all the same, nothing is fetched for it", len(asked) == before and not (WORK / "data-named" / "wholeheard" / "models").exists(), asked)
    server.shutdown()


def models_listed(now: dt.datetime) -> None:
    """U. Settings offer a short list of models to download, each said by what it is good for (D37)."""
    print("U. Settings: a short list of models to download, each said by what it is good for (D37)")
    served, turbo, small = WORK / "served-list", os.urandom(200_000), os.urandom(100_000)
    served.mkdir()
    (served / "turbo.bin").write_bytes(turbo)
    (served / "small.bin").write_bytes(os.urandom(100_000))  # served first: not the published file
    asked = []

    # Served slowly — 20 kB every 0.3 s — so that how far it is can be read.
    class Slow(http.server.SimpleHTTPRequestHandler):
        def __init__(self, *args, **kwargs):
            super().__init__(*args, directory=str(served), **kwargs)

        def log_message(self, *_):
            asked.append(self.path)

        def copyfile(self, source, sink):
            while chunk := source.read(20_000):
                sink.write(chunk)
                sink.flush()
                time.sleep(0.3)

    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Slow)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    at = f"http://127.0.0.1:{server.server_address[1]}"
    meetings, data = WORK / "listing", WORK / "data-listing"
    session(meetings / "2026-10-25_11-00", now - dt.timedelta(hours=1), 5, {"local.wav": 2000})
    heard = meetings / "2026-10-25_11-00" / "transcript.jsonl"
    models = data / "wholeheard" / "models"
    window = Window(meetings, "fr_FR.UTF-8")
    window.env.update(XDG_DATA_HOME=str(data), WHOLEHEARD_MODEL_PUBLISHED=";".join([
        f"{at}/turbo.bin 200000 {hashlib.sha256(turbo).hexdigest()}",
        f"{at}/small.bin 100000 {hashlib.sha256(small).hexdigest()}",
    ]))
    row = "return document.querySelector('[data-offered=\"{}\"]')?.innerText || ''"
    turbo_row, small_row = row.format("ggml-large-v3-turbo-q8_0.bin"), row.format("ggml-small-q8_0.bin")
    offers = "return [...document.querySelectorAll('#offered button, #model-fetch')].filter(b => !b.disabled).length"
    with window as browser:
        browser.find_element(By.ID, "settings-link").click()
        listed = wait_for(browser, lambda b: b.execute_script(turbo_row) and b.execute_script(small_row))
        said = (browser.execute_script(turbo_row), browser.execute_script(small_row))
        check("Settings list Whisper large v3 turbo and Whisper small, each with its size, its host and what it is good for",
              listed and "Whisper large v3 turbo" in said[0] and "200 ko" in said[0] and "Recommandé" in said[0]
              and "Whisper small" in said[1] and "ordinateur modeste" in said[1]
              and all(at.split("/")[2] in line for line in said), said)
        check("each not on this computer is offered", "Télécharger" in said[0] and "Télécharger" in said[1], said)
        check("nothing is fetched before a click", asked == [], asked)
        chosen = browser.execute_script("return document.getElementById('model').selectedOptions[0]?.text || ''")
        check("the model chosen and missing is said so in the choice, not shown as another", "pas encore" in chosen, chosen)
        # A meeting waiting for the model chosen, the small one coming: its offer waits too.
        browser.execute_script("location.hash = '#/meeting/2026-10-25_11-00'")
        wait_for(browser, lambda b: b.find_elements(By.ID, "make-transcript"))
        press(browser, "make-transcript")
        wait_for(browser, lambda b: b.find_elements(By.CSS_SELECTOR, "#transcript-state #model-fetch"))
        browser.execute_script("window.__TAURI__.core.invoke('model_offered_fetch', {file: 'ggml-small-q8_0.bin'})")
        waits = wait_for(browser, lambda b: b.execute_script("return document.getElementById('model-fetch')?.disabled === true"), 5)
        check("another model coming, a meeting waiting for the chosen one offers it greyed", waits)
        wait_for(browser, lambda b: b.execute_script("return document.getElementById('model-fetch')?.disabled === false"), 15)
        browser.find_element(By.ID, "settings-link").click()
        wait_for(browser, lambda b: b.execute_script(small_row) and "Télécharger" in b.execute_script(small_row))
        # Clickable once the fetch before it has let go — greyed until then, a click on it is none.
        wait_for(browser, lambda b: b.execute_script("return document.querySelector('[data-offered=\"ggml-small-q8_0.bin\"] button')?.disabled === false"), 10)
        browser.execute_script("document.querySelector('[data-offered=\"ggml-small-q8_0.bin\"] button').click()")
        far = wait_for(browser, lambda b: "%" in b.execute_script(small_row), 5)
        check("being downloaded, how far is said on its row", far, browser.execute_script(small_row))
        check("one fetch at a time: nothing else is offered meanwhile", browser.execute_script(offers) == 0, browser.execute_script(offers))
        again = wait_for(browser, lambda b: "Télécharger" in b.execute_script(small_row) and "%" not in b.execute_script(small_row), 15)
        check("a file that is not the published one: nothing kept, offered again", again and not (models / "ggml-small-q8_0.bin").exists())
        (served / "small.bin").write_bytes(small)
        notified = WORK / "notified-small"
        monitor = subprocess.Popen(
            ["dbus-monitor", "--session", "interface='org.freedesktop.Notifications',member='Notify'"],
            stdout=open(notified, "w"), stderr=subprocess.DEVNULL,
        )
        try:
            time.sleep(0.5)
            browser.execute_script("document.querySelector('[data-offered=\"ggml-small-q8_0.bin\"] button').click()")
            here = wait_for(browser, lambda b: "Sur cet ordinateur" in b.execute_script(small_row), 15)
            check("fetched, its row says it is on this computer", here and (models / "ggml-small-q8_0.bin").read_bytes() == small, browser.execute_script(small_row))
            told = wait_for(browser, lambda b: "Choisissez-le" in notified.read_text(), 5)
        finally:
            monitor.terminate()
            monitor.wait(timeout=10)
        check("its notice says to choose it, never that a transcript begins",
              told and "La transcription est en cours" not in notified.read_text() and not heard.exists(), notified.read_text()[-400:])
        names = [o.text for o in browser.find_elements(By.CSS_SELECTOR, "#model option")]
        check("it is offered among the models to choose", any(name.startswith("Whisper small") for name in names), names)
        check("the choice stays the user's", "ggml-small" not in window.file.read_text()
              and "pas encore" in browser.execute_script("return document.getElementById('model').selectedOptions[0]?.text || ''"))
        # The turbo's row and D36's offer are the same fetch.
        browser.execute_script("document.querySelector('[data-offered=\"ggml-large-v3-turbo-q8_0.bin\"] button').click()")
        both = wait_for(browser, lambda b: "%" in b.execute_script(turbo_row) and not b.find_elements(By.ID, "model-fetch"), 5)
        check("the turbo's row and the missing model's offer are one fetch", both, browser.execute_script(turbo_row))
        ready = wait_for(browser, lambda b: "Sur cet ordinateur" in b.execute_script(turbo_row) and "Prêt" in (b.execute_script("return document.getElementById('model-state')?.innerText || ''")), 20)
        check("fetched, the model chosen is ready", ready and (models / "ggml-large-v3-turbo-q8_0.bin").read_bytes() == turbo)
    server.shutdown()


def list_refreshed(now: dt.datetime) -> None:
    """W. The list of models is read again only signed by the decider's key: until it is built in, none is (D40, D56)."""
    print("W. Settings: no list of models read again until the decider's key is built in, and it is said (D40, D56)")
    # When registry::PUBLIC_KEY is set, this scenario changes: the button is back, its host shown,
    # and an unsigned list served here is refused. The signed path is the shell's tests'
    # (tests/registry_fetch.rs), with a key of their own: no window can hold a key a test made.
    asked = []

    class List(http.server.BaseHTTPRequestHandler):
        def do_GET(self):
            asked.append(self.path)
            self.send_response(404)
            self.send_header("Content-Length", "0")
            self.end_headers()

        def log_message(self, *_):
            pass

    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), List)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    at = f"127.0.0.1:{server.server_address[1]}"
    meetings = WORK / "refreshed"
    meetings.mkdir()
    said = "return document.getElementById('offered-refreshed')?.innerText || ''"
    window = Window(meetings, "fr_FR.UTF-8")
    window.env.update(WHOLEHEARD_REGISTRY=f"http://{at}/models.json")
    with window as browser:
        browser.find_element(By.ID, "settings-link").click()
        waits = wait_for(browser, lambda b: "attend la clé qui la signe" in b.execute_script(said), 10)
        check("Settings say the list waits on the key that signs it, and the models stay offered",
              waits and "restent proposés" in browser.execute_script(said)
              and browser.find_elements(By.CSS_SELECTOR, '[data-offered="ggml-small-q8_0.bin"]'),
              browser.execute_script(said))
        check("no button offers to read it again", not browser.find_elements(By.ID, "offered-refresh"))
        # Asked all the same, it is refused, and logged so, with nothing asked of anyone.
        browser.execute_script("window.__TAURI__.core.invoke('registry_refresh').then(() => { window.listRead = 'read'; }, () => { window.listRead = 'refused'; })")
        answered = wait_for(browser, lambda b: b.execute_script("return window.listRead || ''"), 10)
        check("asked all the same, it is refused, and nothing crosses the network",
              answered and browser.execute_script("return window.listRead") == "refused" and asked == [],
              (browser.execute_script("return window.listRead"), asked))
    server.shutdown()


def speech_offered(now: dt.datetime) -> None:
    """X. On a Mac, Apple's SpeechAnalyzer is offered among who hears, its model fetched on the click (D42)."""
    print("X. Settings: Apple's SpeechAnalyzer offered where this Mac offers it, its model fetched on the click (D42)")
    # A program in place of wholeheard-speech: this Mac offers it, Apple's model not here yet.
    program, asked = WORK / "wholeheard-speech", WORK / "speech-asked"
    program.write_text(f"""#!/bin/sh
echo "$@" >> '{asked}'
case "$1" in
  status) echo '{{"available":true,"installed":false,"supported":true}}' ;;
  install) sleep 1; echo '{{"installed":true}}' ;;
esac
""")
    program.chmod(0o755)
    meetings = WORK / "speech"
    meetings.mkdir()
    pick = "return [...document.querySelectorAll('#model option')].map(o => o.value + '=' + o.text).join('|')"
    window = Window(meetings, "fr_FR.UTF-8")
    window.env.update(WHOLEHEARD_SPEECH=str(program))
    with window as browser:
        browser.find_element(By.ID, "settings-link").click()
        offered = wait_for(browser, lambda b: "speechanalyzer=" in b.execute_script(pick), 10)
        check("SpeechAnalyzer is offered among who hears, its model said not on this Mac yet",
              offered and "SpeechAnalyzer d’Apple, sur ce Mac — le modèle d’Apple n’est pas encore sur ce Mac" in browser.execute_script(pick),
              browser.execute_script(pick))
        check("its model is not fetched when Settings open", asked.read_text().split() == ["status", "fr-FR"], asked.read_text())
        Select(browser.find_element(By.ID, "model")).select_by_value("speechanalyzer")
        shown = wait_for(browser, lambda b: b.find_element(By.ID, "speech-fetch").is_displayed(), 5)
        check("chosen, it is not who hears yet: the download is offered, macOS fetching it from Apple",
              shown and "engine" not in window.file.read_text() and "install" not in asked.read_text()
              and "depuis Apple" in browser.find_element(By.ID, "speech-fetch").text, window.file.read_text())
        browser.find_element(By.ID, "speech-fetch").click()
        chosen = wait_for(browser, lambda b: 'engine = "speechanalyzer"' in window.file.read_text(), 10)
        check("on the click, macOS fetches it; then SpeechAnalyzer is who hears",
              chosen and "install fr-FR" in asked.read_text(), (asked.read_text(), window.file.read_text()))
    # Where no wholeheard-speech answers — Linux, an older Mac — nothing is offered.
    window = Window(WORK / "speech", "fr_FR.UTF-8")
    window.env.update(WHOLEHEARD_SPEECH=str(WORK / "nowhere"))
    with window as browser:
        browser.find_element(By.ID, "settings-link").click()
        wait_for(browser, lambda b: b.find_elements(By.ID, "model"))
        time.sleep(1)
        check("where SpeechAnalyzer does not answer, it is not offered", "speechanalyzer" not in browser.execute_script(pick),
              browser.execute_script(pick))


def service_named(now: dt.datetime) -> None:
    """Z. Who hears chosen as the minutes' (D43): a service named, its host agreed to first, tested."""
    print("Z. Settings: a service named to hear, its host agreed to before anything is sent (D43)")
    meetings = WORK / "service"
    meetings.mkdir()
    # A meeting a service heard: its header names the service's host as the engine.
    session(meetings / "heard", dt.datetime.now().astimezone() - dt.timedelta(hours=1), 2, {"local.wav": 1000})
    (meetings / "heard" / "transcript.jsonl").write_text(
        '{"type":"header","contract":"transcript/v1","engine":"api.groq.com","model":"whisper-large-v3","language":"fr","context":null}\n'
        '{"type":"segment","speaker":"me","track":"local","voice":null,"start":0.5,"end":1.5,"text":"Bonjour."}\n{"type":"complete","duration":2.0}\n'
    )
    # OpenRouter's stand-in: it lists its transcription models, by what they do (D63).
    listed = []

    class Listing(http.server.BaseHTTPRequestHandler):
        def do_GET(self):
            listed.append(self.path)
            body = json.dumps({"data": [{"id": "openai/whisper-large-v3-turbo"}, {"id": "vendor/scribe"}]}).encode()
            self.send_response(200)
            self.send_header("content-length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def log_message(self, *_):
            pass

    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Listing)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    stand = f"http://127.0.0.1:{server.server_address[1]}/api/v1"
    window = Window(meetings, "fr_FR.UTF-8")
    window.env.update(WHOLEHEARD_HEARING=stand)
    said = "return document.getElementById('transcription-said')?.innerText || ''"
    with window as browser:
        browser.find_element(By.ID, "settings-link").click()
        wait_for(browser, lambda b: b.find_elements(By.ID, "model"))
        check("the service's fields are hidden until it is chosen", not browser.find_element(By.ID, "transcription-service").is_displayed())
        Select(browser.find_element(By.ID, "model")).select_by_value("service")
        shown = wait_for(browser, lambda b: b.find_element(By.ID, "transcription-service").is_displayed(), 5)
        check("chosen, its address, its model and its key are asked", shown and browser.find_elements(By.ID, "transcription-key"))
        # An address that is none, or a name that only starts as this computer's: said, nothing saved.
        browser.find_element(By.ID, "transcription-provider").send_keys("hears.example/v1")
        browser.find_element(By.ID, "transcription-save").click()
        bad = wait_for(browser, lambda b: "l’adresse commence par https://" in b.execute_script(said), 5)
        browser.find_element(By.ID, "transcription-provider").clear()
        browser.find_element(By.ID, "transcription-provider").send_keys("https://localhost.hears.example/v1")
        browser.find_element(By.ID, "transcription-model").send_keys("whisper-1")
        browser.find_element(By.ID, "transcription-save").click()
        elsewhere = wait_for(browser, lambda b: "acceptez d’abord" in b.execute_script(said), 5)
        check("an address that is none, or a host that only starts as this computer's, is said and not saved",
              bad and elsewhere and "transcription_provider" not in window.file.read_text(), browser.execute_script(said))
        browser.find_element(By.ID, "transcription-provider").clear()
        browser.find_element(By.ID, "transcription-model").clear()
        # A host elsewhere, not agreed to: nothing saved, the agreement asked.
        browser.find_element(By.ID, "transcription-provider").send_keys("https://hears.example/v1")
        browser.find_element(By.ID, "transcription-model").send_keys("whisper-1")
        agreement = browser.find_element(By.ID, "transcription-agreed").find_element(By.XPATH, "..").text
        browser.find_element(By.ID, "transcription-save").click()
        refused = wait_for(browser, lambda b: "acceptez d’abord" in b.execute_script(said), 5)
        check("a host elsewhere is not saved before the user agreed, the host named in the agreement",
              refused and "hears.example" in agreement and "transcription_provider" not in window.file.read_text(), (agreement, browser.execute_script(said)))
        # Agreed: saved, and who hears is that service.
        browser.find_element(By.ID, "transcription-agreed").click()
        browser.find_element(By.ID, "transcription-save").click()
        saved = wait_for(browser, lambda b: 'engine = "service"' in window.file.read_text(), 5)
        check("agreed, the service is saved as who hears, its host kept as agreed",
              saved and 'transcription_agreed = "hears.example"' in window.file.read_text() and "whisper-1" in window.file.read_text(), window.file.read_text())
        # Saved, the model no longer needed: Settings are drawn anew at the next reading of the makings,
        # perhaps between finding the button and clicking it. Clicked in the page, as it is now.
        browser.execute_script("document.getElementById('transcription-test').click()")
        tested = wait_for(browser, lambda b: b.execute_script(said).startswith("Il entend, avec les temps de chaque segment"), 10)
        check("Tester says in one sentence what the service answered: it hears, with each segment's times", tested, browser.execute_script(said))
        # The page drawn anew under a slow answer, as a model come or the window back do: nothing lost, no second ask.
        browser.execute_script("const real = window.__TAURI__.core; window.answer = null; window.asked = 0; window.__TAURI__.core = { ...real, invoke: (asked, ...rest) => { if (asked !== 'transcription_tested') return real.invoke(asked, ...rest); window.asked += 1; return new Promise((resolve) => { window.answer = () => real.invoke(asked, ...rest).then(resolve); }); } }; window.restore = () => { window.__TAURI__.core = real; };")
        browser.execute_script("window.__old = document.getElementById('transcription-said'); document.getElementById('transcription-test').click()")
        wait_for(browser, lambda b: b.execute_script("return window.answer !== null"), 5)
        browser.execute_script("refresh()")
        drawn = wait_for(browser, lambda b: b.execute_script("return !window.__old.isConnected && document.getElementById('transcription-said') !== null"), 5)
        check("drawn anew while it asks: Tester stays disabled", drawn and browser.execute_script("return document.getElementById('transcription-test').disabled"))
        browser.execute_script("window.answer()")
        kept = wait_for(browser, lambda b: b.execute_script(said).startswith("Il entend, avec les temps de chaque segment"), 10)
        check("drawn anew while it asks: the answer is said on the page as it is", kept and not browser.execute_script("return document.getElementById('transcription-test').disabled") and browser.execute_script("return window.asked") == 1, browser.execute_script(said))
        browser.execute_script("window.restore(); window.__old = document.getElementById('transcription-said'); refresh()")
        wait_for(browser, lambda b: b.execute_script("return !window.__old.isConnected && document.getElementById('transcription-said') !== null"), 5)
        check("drawn anew after it answered: the answer stays", browser.execute_script(said).startswith("Il entend, avec les temps de chaque segment"), browser.execute_script(said))
        # Who heard a meeting, said where it is read: the service's host, never « sur cet ordinateur ».
        browser.execute_script("location.hash = '#/meeting/heard'")
        makers = lambda b: [p.text for p in b.find_elements(By.CSS_SELECTOR, "#meeting .maker")]
        named = wait_for(browser, lambda b: "Écrit par whisper-large-v3, via api.groq.com" in makers(b), 5)
        check("a meeting a service heard names the service, never this computer", named and not any("sur cet ordinateur" in m for m in makers(browser)), makers(browser))
        browser.execute_script("location.hash = '#/settings'")
        note = wait_for(browser, lambda b: "reçoit le son de vos réunions" in b.find_element(By.ID, "settings").text, 5)
        check("Settings say a transcription service receives the meetings' sound", note and "L’audio ne quitte jamais" not in browser.find_element(By.ID, "settings").text)
        # Offered by name, as the minutes' services are (D63).
        offered = lambda b, id_: b.execute_script("return [...document.querySelectorAll(arguments[0])].map(o => [o.value, o.textContent])", f"#{id_} option")
        names = [said_ for value, said_ in offered(browser, "model") if value.startswith("service:")]
        check("the services that hear are offered by name, as the minutes' are: OpenRouter, OpenAI", names == ["OpenRouter", "OpenAI"], names)
        Select(browser.find_element(By.ID, "model")).select_by_value("service:0")
        models = wait_for(browser, lambda b: [s for _, s in offered(b, "transcription-models")] == ["openai/whisper-large-v3-turbo", "vendor/scribe", "Un autre modèle…"], 5)
        check("OpenRouter chosen: its transcription models listed from it, its address filled and not asked",
              models and listed == ["/api/v1/models?output_modalities=transcription"] and not browser.find_element(By.ID, "transcription-provider").is_displayed()
              and not browser.find_element(By.ID, "transcription-model").is_displayed(), (offered(browser, "transcription-models"), listed))
        Select(browser.find_element(By.ID, "transcription-models")).select_by_value("")
        typed = wait_for(browser, lambda b: b.find_element(By.ID, "transcription-model").is_displayed()
                         and b.execute_script("return document.activeElement.id") == "transcription-model", 3)
        check("« Un autre modèle… »: a model typed by hand", typed)
        Select(browser.find_element(By.ID, "transcription-models")).select_by_value("vendor/scribe")
        browser.find_element(By.ID, "transcription-key").send_keys("sk-stand-in-not-a-key")
        if not browser.find_element(By.ID, "transcription-agreed").is_selected():
            browser.find_element(By.ID, "transcription-agreed").click()
        press(browser, "transcription-save")
        saved = wait_for(browser, lambda b: f'transcription_provider = "{stand}"' in window.file.read_text(), 5)
        text = window.file.read_text()
        check("saved: OpenRouter's address, the model chosen in its list, the key never in the settings",
              saved and 'transcription_model = "vendor/scribe"' in text and "sk-stand-in" not in text, text)
        (WORK / "hearing-key").unlink(missing_ok=True)
        press(browser, "transcription-test")
        tested = wait_for(browser, lambda b: b.execute_script(said).startswith("Il entend, avec les temps de chaque segment"), 10)
        check("Tester, on the service chosen from the list, says it hears with each segment's times — the key the keyring kept given to it",
              tested and (WORK / "hearing-key").exists() and (WORK / "hearing-key").read_text() == "sk-stand-in-not-a-key", browser.execute_script(said))
        # Each cause its own sentence (D72); the raw text never, outside developer mode.
        code = WORK / "test-code"
        try:
            for cause, sentence in TEST_CAUSES:
                code.write_text(cause)
                press(browser, "transcription-test")
                held = wait_for(browser, lambda b: b.execute_script(said) == sentence, 10)
                check(f"Tester of a service, {cause}: its own sentence, what to do, no raw text", held, browser.execute_script(said))
            code.write_text("provider.of_a_later_version")
            press(browser, "transcription-test")
            unknown = wait_for(browser, lambda b: b.execute_script(said).startswith("Le modèle ne répond pas"), 10)
            check("a code unknown is read as no code: today's sentence", unknown and "vérifiez son adresse, son nom et sa clé" in browser.execute_script(said), browser.execute_script(said))
        finally:
            code.unlink(missing_ok=True)
        browser.execute_script("location.hash = '#/'")
        wait_for(browser, lambda b: b.find_element(By.ID, "list").is_displayed(), 5)
        browser.execute_script("location.hash = '#/settings'")
        kept = wait_for(browser, lambda b: b.find_elements(By.ID, "model") and b.find_element(By.ID, "model").get_attribute("value") == "service:0"
                        and b.find_element(By.ID, "transcription-models").get_attribute("value") == "vendor/scribe", 5)
        check("drawn again: OpenRouter chosen, its model in its list", kept)
        Select(browser.find_element(By.ID, "model")).select_by_value("service:1")
        named = wait_for(browser, lambda b: [s for _, s in offered(b, "transcription-models")] == ["whisper-1", "Un autre modèle…"], 5)
        check("OpenAI chosen: the model it publishes with segments, named in the repository — not its own list",
              named and len(listed) == 2, (offered(browser, "transcription-models"), listed))
        # In developer mode, the sentence and the program's last line, its name off.
        code.write_text(TEST_CAUSES[-1][0])
        try:
            before = browser.find_element(By.ID, "transcription-said")
            press(browser, "developer")
            WebDriverWait(browser, 5).until(staleness_of(before))
            press(browser, "transcription-test")
            raw = wait_for(browser, lambda b: "raw words of the service" in b.execute_script(said), 10)
            told = browser.execute_script(said)
            check("Tester of a service in developer mode: the cause, then the raw line alone",
                  raw and told.startswith(TEST_CAUSES[-1][1]) and "wholeheard-transcription:" not in told and "earlier line" not in told, told)
        finally:
            code.unlink(missing_ok=True)
    server.shutdown()


def ollama_pulled(now: dt.datetime) -> None:
    """V. The minutes' recommended model fetched by Ollama, on the user's click (D37)."""
    print("V. Settings: qwen3:8b fetched by Ollama on the user's click; without Ollama, said where to get it (D37)")
    held, pulls, ways = ["mistral:7b"], [], ["cut", "refused", "whole"]

    # An Ollama of the scenario's: its models, and a pull that is cut, then refused, then whole —
    # each said in lines, slowly, as Ollama streams it.
    class Ollama(http.server.BaseHTTPRequestHandler):
        protocol_version = "HTTP/1.0"

        def do_GET(self):
            body = json.dumps({"models": [{"name": name} for name in held]}).encode()
            self.send_response(200)
            self.send_header("content-length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def do_POST(self):
            pulls.append(json.loads(self.rfile.read(int(self.headers["content-length"]))))
            way = ways.pop(0)
            self.send_response(200)
            self.send_header("content-type", "application/x-ndjson")
            self.end_headers()
            self.wfile.write(b'{"status":"pulling manifest"}\n')
            for done in range(0, 1001, 100):
                if way == "cut" and done == 300:
                    return
                if way == "refused" and done == 300:
                    self.wfile.write(b'{"error":"pull model manifest: file does not exist"}\n')
                    return
                self.wfile.write(json.dumps({"status": "pulling a1b2", "digest": "sha256:a1b2", "total": 1000, "completed": done}).encode() + b"\n")
                self.wfile.flush()
                time.sleep(0.3)
            held.append("qwen3:8b")
            self.wfile.write(b'{"status":"success"}\n')

        def log_message(self, *_):
            pass

    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Ollama)
    threading.Thread(target=server.serve_forever, daemon=True).start()

    # Whisper small served slowly, and not as published: a fetch that comes, then is refused.
    class Slow(http.server.BaseHTTPRequestHandler):
        def do_GET(self):
            self.send_response(200)
            self.send_header("content-length", "100000")
            self.end_headers()
            for _ in range(10):
                self.wfile.write(b"x" * 10_000)
                self.wfile.flush()
                time.sleep(0.3)

        def log_message(self, *_):
            pass

    whisper = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Slow)
    threading.Thread(target=whisper.serve_forever, daemon=True).start()
    at = f"http://127.0.0.1:{whisper.server_address[1]}"
    meetings = WORK / "pulling"
    meetings.mkdir()
    window = Window(meetings, "fr_FR.UTF-8", ollama=f"http://127.0.0.1:{server.server_address[1]}")
    window.env.update(XDG_DATA_HOME=str(WORK / "data-pulling"), WHOLEHEARD_MODEL_PUBLISHED=f"{at}/turbo.bin 1 {'0' * 64};{at}/small.bin 100000 {'0' * 64}")
    row = "return document.querySelector('[data-offered=\"qwen3:8b\"]')?.innerText || ''"
    pull = "document.querySelector('[data-offered=\"qwen3:8b\"] button').click()"
    offers = "return [...document.querySelectorAll('#offered button, #model-fetch')].filter(b => !b.disabled).length"
    notified = WORK / "notified-pull"
    monitor = subprocess.Popen(
        ["dbus-monitor", "--session", "interface='org.freedesktop.Notifications',member='Notify'"],
        stdout=open(notified, "w"), stderr=subprocess.DEVNULL,
    )
    try:
        with window as browser:
            browser.find_element(By.ID, "settings-link").click()
            listed = wait_for(browser, lambda b: "Télécharger" in b.execute_script(row))
            said = browser.execute_script(row)
            check("qwen3:8b is offered for the minutes, its size and what it is good for said", listed and "qwen3:8b" in said and "5 Go" in said and "comptes rendus" in said, said)
            check("nothing is fetched before a click", pulls == [], pulls)
            # A Whisper model coming: the pull waits — its row greyed, and asked all the same, Ollama is not.
            browser.execute_script("document.querySelector('[data-offered=\"ggml-small-q8_0.bin\"] button').click()")
            wait_for(browser, lambda b: "%" in b.execute_script(row.replace("qwen3:8b", "ggml-small-q8_0.bin")), 5)
            greyed = browser.execute_script("return document.querySelector('[data-offered=\"qwen3:8b\"] button')?.disabled === true")
            browser.execute_script("window.__TAURI__.core.invoke('ollama_pull_offered')")
            time.sleep(1)
            check("a Whisper model coming, the pull waits: its row greyed, Ollama not asked", greyed and pulls == [], (greyed, pulls))
            wait_for(browser, lambda b: "Télécharger" in b.execute_script(row.replace("qwen3:8b", "ggml-small-q8_0.bin")) and "%" not in b.execute_script(row.replace("qwen3:8b", "ggml-small-q8_0.bin")), 15)
            wait_for(browser, lambda b: b.execute_script("return document.querySelector('[data-offered=\"qwen3:8b\"] button')?.disabled === false"), 5)
            for way in ("cut", "refused"):
                browser.execute_script(pull)
                far = wait_for(browser, lambda b: "%" in b.execute_script(row), 5)
                check(f"{way}: how far is said while Ollama fetches it, nothing else offered meanwhile", far and browser.execute_script(offers) == 0, browser.execute_script(row))
                again = wait_for(browser, lambda b: "Télécharger" in b.execute_script(row), 15)
                check(f"{way}: said, and offered again", again and "n’a pas été téléchargé" in notified.read_text(), notified.read_text()[-300:])
            browser.execute_script(pull)
            here = wait_for(browser, lambda b: "Sur cet ordinateur" in b.execute_script(row), 20)
            check("whole: on this computer, Ollama asked for qwen3:8b", here and pulls[-1].get("model") == "qwen3:8b", (browser.execute_script(row), pulls))
            first = browser.execute_script("return document.querySelector('#minutes optgroup option')?.textContent || ''")
            check("it is offered first among the minutes' models", "qwen3:8b" in first, first)
            check("its notice says it is ready for the minutes", wait_for(browser, lambda b: "comptes rendus" in notified.read_text(), 5), notified.read_text()[-300:])
    finally:
        monitor.terminate()
        monitor.wait(timeout=10)
        whisper.shutdown()
    # No Ollama answering: where to get it is said, and nothing offered — Wholeheard never installs it.
    with Window(meetings, "fr_FR.UTF-8") as browser:
        browser.find_element(By.ID, "settings-link").click()
        absent = wait_for(browser, lambda b: "ollama.com" in b.execute_script(row))
        check("no Ollama: the row says it is needed and where to get it, with no button",
              absent and not browser.find_elements(By.CSS_SELECTOR, '[data-offered="qwen3:8b"] button'), browser.execute_script(row))
    server.shutdown()


def first_run(now: dt.datetime) -> None:
    """Y. The first start (D58): the full window opens on one question, then the setting said plainly."""
    print("Y. the first start: one question, the setting said plainly, private by default (D58)")
    welcome = "return !document.getElementById('welcome').hidden && document.getElementById('welcome').innerText || ''"
    focused = "return [document.activeElement.tagName, document.activeElement.name || document.activeElement.id, document.activeElement.value || ''].join(':')"
    picked = "return [...document.querySelectorAll('#welcome input[type=radio]:checked')].map(i => i.value).join(',')"
    summary = "return document.getElementById('welcome-summary')?.innerText || ''"
    keys = lambda browser, *sent: ActionChains(browser).send_keys(*sent).perform()

    def started(name: str, language: str = "fr_FR.UTF-8", dark: bool = False, ollama: str = "http://127.0.0.1:9") -> "Window":
        meetings = WORK / f"welcome-{name}"
        meetings.mkdir()
        # No `window` line: the small one is the one left, as at a real first start (D35).
        window = Window(meetings, language, ollama=ollama, small=True, welcomed=False, args=())
        window.env.update(XDG_DATA_HOME=str(WORK / f"data-welcome-{name}"), WHOLEHEARD_SPEECH=str(WORK / "nowhere"))
        # The dark theme asked of GTK, whatever the desktop shows; the light one cannot be asked
        # of a dark desktop without changing the user's setting — its colours are put on the page
        # for its screenshots instead, below.
        if dark:
            window.env.update(GTK_THEME="Adwaita:dark")
        return window

    # The light theme's colours, as `ui/window.css` sets them first.
    css = (Path(__file__).parent.parent / "ui" / "window.css").read_text()
    light = css[css.index(":root {") + len(":root {"):css.index("}", css.index(":root {"))].replace("\n", " ").strip()

    def seen(browser, name: str) -> None:
        time.sleep(0.4)  # the card's arrival, 180 ms, ended
        forced = "light" in name and browser.execute_script("return matchMedia('(prefers-color-scheme: dark)').matches")
        if forced:
            browser.execute_script("document.documentElement.style.cssText = arguments[0] + '; color-scheme: light'", light)
            time.sleep(0.3)  # the choices' borders, which ease to their colour
        shot(browser, name)
        if forced:
            browser.execute_script("document.documentElement.style.cssText = ''")

    def until_question(browser) -> bool:
        return wait_for(browser, lambda b: "Vos réunions parlent-elles" in b.execute_script(welcome) or "Do your meetings" in b.execute_script(welcome), 10)

    # 1. Skipped: everything on this computer, the meetings opened.
    window = started("skip")
    with window as browser:
        shown = until_question(browser)
        check("started without --window, not welcomed, no meeting: the full window opens on the assistant",
              shown and not browser.find_elements(By.ID, "small-record"), browser.execute_script(welcome))
        said = browser.execute_script(welcome)
        check("one question, its three answers in plain words, its sentence typeset as French has it",
              "Réglons Wholeheard en une question." in said and "Vos réunions parlent-elles de sujets confidentiels ?" in said
              and all(a in said for a in ("Oui, souvent", "Rarement ou jamais", "Je ne sais pas", "Passer — tout sur cet ordinateur", "Suivant")), said)
        check("nothing is chosen for the user", browser.execute_script(picked) == "", browser.execute_script(picked))
        check("each answer is a radio of one group, named by the question",
              browser.execute_script("const g = document.querySelector('#welcome [role=radiogroup]'); return !!g && !!g.getAttribute('aria-labelledby') && g.querySelectorAll('input[type=radio][name=private]').length === 3"))
        check("no sidebar of the meetings while it is answered", not browser.find_element(By.ID, "sidebar").is_displayed())
        marked = wait_for(browser, lambda b: "welcomed = true" in window.file.read_text(), 5)
        check("welcomed is written as soon as it is shown, nothing else", marked and "minutes_provider" not in window.file.read_text(), window.file.read_text())
        browser.set_window_size(420, 400)
        time.sleep(0.5)
        wide = browser.execute_script("return document.documentElement.scrollWidth <= document.documentElement.clientWidth")
        check("at the window's smallest size nothing overflows sideways", wide)
        seen(browser, "d58-1-question-light-small")
        browser.set_window_size(1100, 800)
        time.sleep(0.3)
        card = browser.execute_script("const c = document.querySelector('#welcome .welcome'), s = getComputedStyle(c); return [c.getBoundingClientRect().width / document.documentElement.clientWidth, parseFloat(s.paddingLeft)]")
        check("at the window's opening size the card takes two thirds of it, its margin 20 px at most", 0.6 <= card[0] <= 0.7 and card[1] <= 20, card)
        seen(browser, "d58-1-question-light")
        # Passer, by the keyboard: Tab from the question reaches it after the answers.
        keys(browser, Keys.TAB)
        first = browser.execute_script(focused)
        keys(browser, Keys.TAB)
        skip = browser.execute_script(focused)
        check("Tab reaches the answers, then Passer", first.startswith("INPUT:private") and skip == "BUTTON:welcome-skip:", (first, skip))
        # Drawn once the window has the system's focus: waited for, the desktop being in use (#510).
        # Asked for, then waited for: another window of the desktop may hold the system's focus.
        browser.switch_to.window(browser.current_window_handle)
        ring = wait_for(browser, lambda b: b.execute_script("const s = getComputedStyle(document.activeElement); return s.outlineStyle !== 'none' && parseFloat(s.outlineWidth) > 0"), 10)
        check("the focus is seen", ring)
        keys(browser, Keys.ENTER)
        listed = wait_for(browser, lambda b: b.find_element(By.ID, "list").is_displayed() and b.find_element(By.ID, "sidebar").is_displayed(), 10)
        text = window.file.read_text()
        check("Passer: the minutes by Ollama on this computer, the meetings opened",
              listed and 'minutes_provider = "http://127.0.0.1:9/v1"' in text and 'minutes_model = "qwen3:8b"' in text, text)
        check("the window left is not changed: the small one stays the one reopened (D35)", "window =" not in text, text)
        check("nothing is fetched on Passer", not (WORK / "data-welcome-skip" / "wholeheard" / "models").exists())
    # Started again, welcomed: no assistant.
    window.args = ["--window"]
    with window as browser:
        time.sleep(1)
        check("welcomed, it never comes back at a start", browser.execute_script(welcome) == "" and browser.find_elements(By.ID, "small-record"))

    # 2. Private, by the keyboard: the summary, then Plus tard.
    window = started("private")
    with window as browser:
        until_question(browser)
        keys(browser, Keys.TAB, Keys.SPACE)
        check("Space chooses the answer focused", browser.execute_script(picked) == "often", browser.execute_script(picked))
        keys(browser, Keys.ARROW_DOWN)
        keys(browser, Keys.ARROW_DOWN)
        check("the arrows go through the answers", browser.execute_script(picked) == "unsure", browser.execute_script(picked))
        keys(browser, Keys.TAB, Keys.TAB, Keys.ENTER)
        shown = wait_for(browser, lambda b: "Mode privé" in b.execute_script(summary), 5)
        said = browser.execute_script(summary)
        check("« Je ne sais pas »: the private mode, who transcribes, who writes, the reminder, each said plainly",
              shown and "Vos réunions ne quittent pas cet ordinateur." in said and "Transcription" in said and "Whisper large v3 turbo" in said
              and "Compte rendu" in said and "Qwen3 8b" in said and "Rappel" in said and "ggml" not in said, said)
        check("Ollama not answering: the minutes wait for it, with ollama.com", "ollama.com" in said, said)
        check("the model missing: offered with its size and host",
              "Télécharger le modèle (874 Mo)" in said.replace(" Mo", " Mo") and "Depuis huggingface.co, vérifié avant d’être utilisé." in said, said)
        check("nothing is saved before the last screen", "minutes_provider" not in window.file.read_text(), window.file.read_text())
        seen(browser, "d58-3-summary-private-light")
        press(browser, "welcome-later")
        listed = wait_for(browser, lambda b: b.find_element(By.ID, "list").is_displayed(), 10)
        check("Plus tard: saved private, the meetings opened, the model left unfetched",
              listed and 'minutes_model = "qwen3:8b"' in window.file.read_text() and not (WORK / "data-welcome-private" / "wholeheard" / "models").exists(),
              window.file.read_text())

    # 3. Rarely, then a service: connected, chosen in Settings.
    window = started("service")
    with window as browser:
        until_question(browser)
        keys(browser, Keys.TAB, Keys.ARROW_DOWN)
        check("the arrows choose « Rarement ou jamais »", browser.execute_script(picked) == "rarely", browser.execute_script(picked))
        keys(browser, Keys.TAB, Keys.TAB, Keys.ENTER)
        asked = wait_for(browser, lambda b: "Qui écrit le compte rendu ?" in b.execute_script(welcome), 5)
        said = browser.execute_script(welcome)
        check("then who writes the minutes, its two answers, no address nor key asked",
              asked and "Cet ordinateur" in said and "Un service en ligne" in said and not browser.find_elements(By.CSS_SELECTOR, "#welcome input:not([type=radio])"), said)
        seen(browser, "d58-2-writer-light")
        keys(browser, Keys.TAB, Keys.ARROW_DOWN)
        check("« Un service en ligne » chosen by the keyboard", browser.execute_script(picked) == "service", browser.execute_script(picked))
        keys(browser, Keys.TAB, Keys.TAB, Keys.ENTER)
        shown = wait_for(browser, lambda b: "Mode connecté" in b.execute_script(summary), 5)
        said = browser.execute_script(summary)
        check("the connected mode: the minutes online, the transcription on this computer",
              shown and "la transcription reste sur cet ordinateur" in said and "Choisir mon service" in said, said)
        seen(browser, "d58-3-summary-connected-light")
        press(browser, "welcome-go")
        opened = wait_for(browser, lambda b: b.find_element(By.ID, "settings").is_displayed()
                          and b.execute_script("return document.activeElement.id") == "minutes", 10)
        check("Choisir mon service opens Settings at the minutes, nothing saved for them yet",
              opened and "minutes_provider" not in window.file.read_text(), (browser.execute_script("return document.activeElement.id"), window.file.read_text()))
        # Refaire le réglage: Annuler changes nothing; confirmed, it is saved.
        before = window.file.read_text()
        press(browser, "welcome-again")
        again = until_question(browser)
        check("Refaire le réglage opens the assistant, with Annuler", again and "Annuler" in browser.execute_script(welcome) and "Passer" not in browser.execute_script(welcome))
        press(browser, "welcome-skip")
        back = wait_for(browser, lambda b: b.find_element(By.ID, "settings").is_displayed(), 5)
        check("Annuler: back to Settings, nothing changed", back and window.file.read_text() == before, window.file.read_text())
        press(browser, "welcome-again")
        until_question(browser)
        browser.execute_script("document.querySelector('#welcome input[value=often]').click()")
        press(browser, "welcome-next")
        wait_for(browser, lambda b: "Mode privé" in b.execute_script(summary), 5)
        check("nothing changes before the last screen is confirmed", window.file.read_text() == before, window.file.read_text())
        press(browser, "welcome-later")
        wait_for(browser, lambda b: b.find_element(By.ID, "list").is_displayed(), 10)
        check("confirmed, it is saved", 'minutes_model = "qwen3:8b"' in window.file.read_text(), window.file.read_text())
        check("and the reminder left on, as it was", "reminder = false" not in window.file.read_text(), window.file.read_text())
        # Private changes who transcribes and who writes, and nothing else: from a service that
        # hears, with the reminder off, a model and a folder of the user's, confirmed private.
        lines = [line for line in window.file.read_text().splitlines()
                 if not line.startswith(("engine", "transcription_", "reminder", "model ="))]
        mine = window.file.parent / "my-model.bin"
        window.file.write_text("\n".join(lines + [
            'engine = "service"', 'transcription_provider = "http://127.0.0.1:9/v1"',
            'transcription_model = "a-model"', 'transcription_agreed = "127.0.0.1:9"',
            "reminder = false", f'model = "{mine}"']) + "\n")
        kept = [line for line in window.file.read_text().splitlines()
                if line.startswith(("transcription_", "reminder", "model =", "sessions"))]
        # Refaire, confirmed connected, from a setting that saving private would change — a service
        # that hears: Settings at the minutes, nothing changed by itself.
        press(browser, "settings-link")
        wait_for(browser, lambda b: b.find_elements(By.ID, "welcome-again") and b.find_element(By.ID, "welcome-again").is_displayed(), 10)
        press(browser, "welcome-again")
        until_question(browser)
        browser.execute_script("document.querySelector('#welcome input[value=rarely]').click()")
        press(browser, "welcome-next")
        wait_for(browser, lambda b: b.find_elements(By.CSS_SELECTOR, "#welcome input[name=writer]"), 5)
        browser.execute_script("document.querySelector('#welcome input[value=service]').click()")
        press(browser, "welcome-next")
        wait_for(browser, lambda b: "Mode connecté" in b.execute_script(summary), 5)
        staged = window.file.read_text()
        press(browser, "welcome-go")
        opened = wait_for(browser, lambda b: b.find_element(By.ID, "settings").is_displayed()
                          and b.execute_script("return document.activeElement.id") == "minutes", 10)
        check("Refaire, confirmed connected: Settings at the minutes, nothing changed by itself",
              opened and window.file.read_text() == staged, window.file.read_text())
        press(browser, "settings-link")
        wait_for(browser, lambda b: b.find_elements(By.ID, "welcome-again") and b.find_element(By.ID, "welcome-again").is_displayed(), 10)
        press(browser, "welcome-again")
        until_question(browser)
        browser.execute_script("document.querySelector('#welcome input[value=often]').click()")
        press(browser, "welcome-next")
        wait_for(browser, lambda b: "Mode privé" in b.execute_script(summary), 5)
        press(browser, "welcome-later")
        wait_for(browser, lambda b: b.find_element(By.ID, "list").is_displayed(), 10)
        after = window.file.read_text()
        check("private from a service: the engine back to this computer",
              'engine = "whisper"' in after, after)
        check("and nothing else: the service's fields, the reminder, the model and the folder as they were",
              all(line in after.splitlines() for line in kept), (kept, after))

    # 4. The dark theme, in English.
    window = started("dark", "en_GB.UTF-8", dark=True)
    with window as browser:
        until_question(browser)
        dark = browser.execute_script("return matchMedia('(prefers-color-scheme: dark)').matches")
        check("the system's dark theme is the page's", dark and browser.execute_script("return getComputedStyle(document.body).backgroundColor") == GROUND["dark"])
        said = browser.execute_script(welcome)
        check("in English", "Do your meetings discuss confidential matters?" in said and "Skip — everything on this computer" in said, said)
        seen(browser, "d58-1-question-dark")
        press(browser, "welcome-next")
        wait_for(browser, lambda b: "Private mode" in b.execute_script(summary), 5)
        check("Next with nothing chosen is private", "Private mode" in browser.execute_script(summary), browser.execute_script(summary))
        seen(browser, "d58-3-summary-private-dark")
        press(browser, "welcome-back")
        until_question(browser)
        browser.execute_script("document.querySelector('#welcome input[value=rarely]').click()")
        press(browser, "welcome-next")
        wait_for(browser, lambda b: b.find_elements(By.CSS_SELECTOR, "#welcome input[name=writer]"), 5)
        seen(browser, "d58-2-writer-dark")
        browser.execute_script("document.querySelector('#welcome input[value=service]').click()")
        press(browser, "welcome-next")
        wait_for(browser, lambda b: "Connected mode" in b.execute_script(summary), 5)
        seen(browser, "d58-3-summary-connected-dark")

    # At the window's smallest size: the view 420 px at most, nothing wider than it.
    wide = lambda b: b.execute_script("const e = document.documentElement; return window.innerWidth <= 420 && e.scrollWidth <= e.clientWidth")

    # 5. From the applications' menu, not welcomed: the assistant all the same; left midway, its
    # answers are not kept — only that it was shown.
    window = started("menu")
    window.args = ["--window"]
    with window as browser:
        shown = until_question(browser)
        check("started with --window, not welcomed, no meeting: the full window opens on the assistant",
              shown and not browser.find_elements(By.ID, "small-record") and len(browser.window_handles) == 1,
              (browser.window_handles, browser.execute_script(welcome)))
        wait_for(browser, lambda b: "welcomed = true" in window.file.read_text(), 5)
        before = window.file.read_text()
        browser.set_window_size(420, 400)
        browser.execute_script("document.querySelector('#welcome input[value=rarely]').click()")
        press(browser, "welcome-next")
        reached = wait_for(browser, lambda b: b.find_elements(By.CSS_SELECTOR, "#welcome input[name=writer]"), 5)
        time.sleep(0.5)
        check("its second question at the window's smallest size: nothing overflows sideways", reached and wide(browser))
        browser.execute_script("document.querySelector('#welcome input[value=service]').click()")
        press(browser, "welcome-next")
        reached = wait_for(browser, lambda b: "Mode connecté" in b.execute_script(summary), 5)
        time.sleep(0.5)
        check("its last screen, connected, at the smallest size: nothing overflows sideways", reached and wide(browser))
    check("left midway, nothing of its answers is written", window.file.read_text() == before, window.file.read_text())

    # 6. The model here already: Terminer, nothing to fetch.
    window = started("here")
    models = WORK / "data-welcome-here" / "wholeheard" / "models"
    models.mkdir(parents=True)
    (models / "ggml-large-v3-turbo-q8_0.bin").write_bytes(b"\0" * 16)
    with window as browser:
        until_question(browser)
        press(browser, "welcome-next")
        wait_for(browser, lambda b: "Mode privé" in b.execute_script(summary), 5)
        said = browser.execute_script(summary)
        # Read once the card has arrived (180 ms from transparent): `.text` is the text seen.
        wait_for(browser, lambda b: b.find_element(By.ID, "welcome-go").text != "", 2)
        done = browser.find_element(By.ID, "welcome-go").text
        check("the model here: Terminer, no fetch offered", done == "Terminer" and "Télécharger" not in said, (done, said))
        browser.set_window_size(420, 400)
        time.sleep(0.5)
        check("its last screen, private, at the smallest size: nothing overflows sideways", done == "Terminer" and wide(browser))
        browser.set_window_size(1100, 800)
        press(browser, "welcome-go")
        listed = wait_for(browser, lambda b: b.find_element(By.ID, "list").is_displayed(), 10)
        check("Terminer: saved private, the meetings opened, nothing fetched",
              listed and 'minutes_model = "qwen3:8b"' in window.file.read_text() and [m.name for m in models.iterdir()] == ["ggml-large-v3-turbo-q8_0.bin"],
              (window.file.read_text(), list(models.iterdir())))

    # 7. The model missing, Ollama answering without the recommended one: « Télécharger le modèle »
    # fetches it as Settings do, the window at the meetings meanwhile — refused, cut, then whole.
    right, ways, asked, posted = os.urandom(300_000), ["refused", "cut", "whole"], [], []

    class Model(http.server.BaseHTTPRequestHandler):
        protocol_version = "HTTP/1.0"

        def do_GET(self):
            way = ways.pop(0)
            asked.append(way)
            sent = os.urandom(len(right)) if way == "refused" else right
            self.send_response(200)
            self.send_header("content-length", str(len(right)))
            self.end_headers()
            for at in range(0, len(right), 30_000):
                if way == "cut" and at == 150_000:
                    return
                self.wfile.write(sent[at:at + 30_000])
                self.wfile.flush()
                time.sleep(0.3)

        def log_message(self, *_):
            pass

    class Ollama(http.server.BaseHTTPRequestHandler):
        def do_GET(self):
            body = json.dumps({"models": [{"name": "mistral:7b"}]}).encode()
            self.send_response(200)
            self.send_header("content-length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def do_POST(self):
            posted.append(self.path)
            self.send_response(500)
            self.end_headers()

        def log_message(self, *_):
            pass

    servers = [http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler) for handler in (Model, Ollama)]
    for server in servers:
        threading.Thread(target=server.serve_forever, daemon=True).start()
    host, ollama = f"127.0.0.1:{servers[0].server_address[1]}", f"http://127.0.0.1:{servers[1].server_address[1]}"
    fetched = WORK / "data-welcome-fetch" / "wholeheard" / "models" / "ggml-large-v3-turbo-q8_0.bin"
    state = "return document.getElementById('model-state')?.innerText || ''"
    notified = WORK / "notified-welcome"
    monitor = subprocess.Popen(
        ["dbus-monitor", "--session", "interface='org.freedesktop.Notifications',member='Notify'"],
        stdout=open(notified, "w"), stderr=subprocess.DEVNULL,
    )

    def last_screen(browser) -> str:
        """From Settings, Refaire le réglage gone through to its last screen, private."""
        press(browser, "settings-link")
        wait_for(browser, lambda b: b.find_elements(By.ID, "welcome-again") and b.find_element(By.ID, "welcome-again").is_displayed(), 10)
        offered = wait_for(browser, lambda b: b.find_elements(By.ID, "model-fetch"), 5)
        press(browser, "welcome-again")
        until_question(browser)
        press(browser, "welcome-next")
        wait_for(browser, lambda b: "Mode privé" in b.execute_script(summary), 5)
        return offered

    try:
        window = started("fetch", ollama=ollama)
        window.env.update(WHOLEHEARD_MODEL_PUBLISHED=f"http://{host}/model.bin {len(right)} {hashlib.sha256(right).hexdigest()}")
        with window as browser:
            until_question(browser)
            press(browser, "welcome-next")
            wait_for(browser, lambda b: "Mode privé" in b.execute_script(summary), 5)
            wait_for(browser, lambda b: b.find_element(By.ID, "welcome-go").text != "", 2)
            said, label = browser.execute_script(summary), browser.find_element(By.ID, "welcome-go").text
            check("Ollama answering without qwen3:8b: the minutes wait for it, Settings fetch it — no ollama.com",
                  "Ollama n’a pas encore qwen3:8b" in said and "ollama.com" not in said, said)
            check("the model missing: Télécharger, its size and host shown, nothing fetched before the click",
                  label == "Télécharger le modèle (300 ko)" and f"Depuis {host}, vérifié avant d’être utilisé." in said and asked == [], (label, said, asked))
            for round_, way in enumerate(("refused", "cut", "whole")):
                if round_:
                    offered = last_screen(browser)
                    check(f"before the {way} one: Settings offer it again, as after their own", offered)
                    wait_for(browser, lambda b: b.find_element(By.ID, "welcome-go").text != "", 2)
                # What is told after the click: each notice is seen twice on the bus, sent and relayed.
                mark = len(notified.read_text())
                press(browser, "welcome-go")
                away = wait_for(browser, lambda b: b.execute_script("return location.hash") in ("#/", "") and b.find_element(By.ID, "list").is_displayed() and asked == ["refused", "cut", "whole"][:round_ + 1], 5)
                check(f"{way}: the click fetches it, the window at the meetings while it comes",
                      away and not fetched.exists() and "Le modèle" not in notified.read_text()[mark:], (asked, notified.read_text()[mark:]))
                if way != "whole":
                    told = wait_for(browser, lambda b: "Le modèle n’a pas été téléchargé" in notified.read_text()[mark:], 15)
                    check(f"{way}: said as Settings say it, the model not put in its place", told and not fetched.exists(), notified.read_text()[mark:][-300:])
            text = window.file.read_text()
            check("saved private before it is fetched: the minutes by the Ollama answering, qwen3:8b",
                  f'minutes_provider = "{ollama}/v1"' in text and 'minutes_model = "qwen3:8b"' in text, text)
            press(browser, "settings-link")
            coming = wait_for(browser, lambda b: "En cours de téléchargement" in b.execute_script(state), 5)
            # Refaire le réglage answered while it comes: the model come, the page read again
            # meanwhile, the answer stays the user's.
            press(browser, "welcome-again")
            until_question(browser)
            browser.execute_script("document.querySelector('#welcome input[value=rarely]').click()")
            # Come, and read by the page — Settings, hidden, say it no longer missing —; its drawing
            # then given a second to end.
            came = wait_for(browser, lambda b: fetched.exists() and b.execute_script(
                "return document.getElementById('model-state')?.dataset.kind") == "named", 15)
            time.sleep(1)
            check("the model come while the assistant is answered: the answer chosen stays, never drawn over",
                  came and browser.execute_script(picked) == "rarely", browser.execute_script(picked))
            press(browser, "welcome-skip")
            ready = wait_for(browser, lambda b: "Prêt" in b.execute_script(state), 15)
            check("whole: said coming in Settings, then ready there, checked against its fingerprint",
                  coming and ready and fetched.read_bytes() == right and wait_for(browser, lambda b: "Modèle prêt" in notified.read_text(), 5),
                  browser.execute_script(state))
            check("Ollama asked nothing but what it holds", posted == [], posted)

        # 8. Connected, no service chosen afterwards: the first meeting's minutes say none is chosen,
        # and open Settings.
        window = started("unchosen")
        with window as browser:
            until_question(browser)
            browser.execute_script("document.querySelector('#welcome input[value=rarely]').click()")
            press(browser, "welcome-next")
            wait_for(browser, lambda b: b.find_elements(By.CSS_SELECTOR, "#welcome input[name=writer]"), 5)
            browser.execute_script("document.querySelector('#welcome input[value=service]').click()")
            press(browser, "welcome-next")
            wait_for(browser, lambda b: "Mode connecté" in b.execute_script(summary), 5)
            press(browser, "welcome-go")
            wait_for(browser, lambda b: b.find_element(By.ID, "settings").is_displayed(), 10)
            first = WORK / "welcome-unchosen" / "2026-10-28_09-00"
            session(first, now - dt.timedelta(hours=1), 3, {"local.wav": 1000})
            (first / "transcript.jsonl").write_text(
                '{"type":"header","contract":"transcript/v1","engine":"whisper.cpp","model":"m","language":null,"context":null}\n'
                '{"type":"segment","speaker":"me","track":"local","start":0.0,"end":1.0,"text":"Bonjour."}\n{"type":"complete","duration":1.0}\n'
            )
            browser.execute_script(f"location.hash = '#/meeting/{first.name}'")
            wait_for(browser, lambda b: b.find_elements(By.ID, "make-minutes"), 10)
            press(browser, "make-minutes")
            told = wait_for(browser, lambda b: "Aucun modèle choisi pour les comptes rendus" in notified.read_text(), 10)
            opened = wait_for(browser, lambda b: b.find_element(By.ID, "settings").is_displayed(), 5)
            check("connected, no service chosen after: the first meeting's minutes say none is, and open Settings",
                  told and opened and not (first / "minutes.md").exists() and "minutes_provider" not in window.file.read_text(),
                  (notified.read_text()[-300:], window.file.read_text()))
    finally:
        monitor.terminate()
        monitor.wait(timeout=10)
        for server in servers:
            server.shutdown()


def meeting_language(now: dt.datetime) -> None:
    """AC. The language meetings are spoken in, and each meeting's own (D65)."""
    print("AC. the meetings' language in Settings, a meeting's own on its page, told to the transcription (D65)")
    meetings = WORK / "speaking"
    heard = meetings / "2026-10-27_14-00"
    session(heard, now - dt.timedelta(days=1), 0.1, {"local.wav": 1000})
    model = WORK / "speaking-model.bin"
    model.write_bytes(b"\0" * 16)
    window = Window(meetings, "fr_FR.UTF-8", f'reminder = false\nmodel = "{model}"\n')
    args = WORK / "hearing-args"
    told = lambda: args.read_text() if args.exists() else ""
    with window as browser:
        press(browser, "settings-link")
        wait_for(browser, lambda b: b.find_elements(By.ID, "meetings-language"), 10)
        offered = [o.text for o in Select(browser.find_element(By.ID, "meetings-language")).options]
        label = browser.execute_script("return document.querySelector('label[for=meetings-language]')?.textContent")
        check("Settings offer the meetings' language, detected by default, French and English first, labelled",
              offered[:3] == ["Détectée par la transcription", "Français", "English"] and label == "Langue des réunions"
              and Select(browser.find_element(By.ID, "meetings-language")).first_selected_option.text == "Détectée par la transcription", (offered, label))
        choose(browser, "meetings-language", "en")
        saved = wait_for(browser, lambda b: 'meeting_language = "en"' in window.file.read_text(), 5)
        check("chosen, it is the settings' default", saved, window.file.read_text())
        browser.execute_script(f"location.hash = '#/meeting/{heard.name}'")
        wait_for(browser, lambda b: b.find_elements(By.ID, "meeting-language"), 10)
        own = Select(browser.find_element(By.ID, "meeting-language"))
        check("a meeting's page offers its own language, the settings' by default — the language they name said",
              own.first_selected_option.text == "Celle des Réglages — English" and not (heard / "language").exists(), own.first_selected_option.text)
        choose(browser, "meeting-language", "de")
        written = wait_for(browser, lambda b: (heard / "language").exists() and (heard / "language").read_text() == "de\n", 5)
        check("chosen on its page, the meeting's language file holds its code alone", written)
        args.unlink(missing_ok=True)
        press(browser, "make-transcript")
        made = wait_for(browser, lambda b: (heard / "transcript.jsonl").exists() and "--language" in told(), 15)
        check("its transcript is heard in the meeting's own language", made and "--language de" in told(), told())
        choose(browser, "meeting-language", "")
        removed = wait_for(browser, lambda b: not (heard / "language").exists(), 5)
        check("« Celle des Réglages »: the file removed", removed)
        args.unlink(missing_ok=True)
        browser.execute_script("window.__TAURI__.core.invoke('transcript_redo', {name: arguments[0], model: arguments[1]})", heard.name, str(model))
        again = wait_for(browser, lambda b: "--language" in told(), 15)
        check("made again with none of its own: heard in the settings' language", again and "--language en" in told(), told())
        browser.execute_script("location.hash = '#/settings'")
        wait_for(browser, lambda b: b.find_elements(By.ID, "meetings-language"), 10)
        choose(browser, "meetings-language", "")
        wait_for(browser, lambda b: "meeting_language" not in window.file.read_text(), 5)
        args.unlink(missing_ok=True)
        browser.execute_script("window.__TAURI__.core.invoke('transcript_redo', {name: arguments[0], model: arguments[1]})", heard.name, str(model))
        detected = wait_for(browser, lambda b: told() != "", 15)
        check("« Détectée »: no language told, the engine detects it", detected and "--language" not in told() and "meeting_language" not in window.file.read_text(), (told(), window.file.read_text()))
        refused = [browser.execute_script(
            "return window.__TAURI__.core.invoke('language_set', {name: arguments[0], language: arguments[1]}).then(() => 'kept', (why) => String(why))", heard.name, text)
            for text in ("Deutsch", "zz")]
        check("a text that is not a code of ISO 639-1 is refused, nothing written", "kept" not in refused and not (heard / "language").exists(), refused)
        browser.execute_script(f"location.hash = '#/'")
        wait_for(browser, lambda b: b.find_element(By.ID, "list").is_displayed(), 5)
        browser.execute_script(f"location.hash = '#/meeting/{heard.name}'")
        wait_for(browser, lambda b: b.find_elements(By.ID, "meeting-language"), 10)
        # By script, in one step: the page may be drawn again between finding the list and reading it.
        said = browser.execute_script("const p = document.getElementById('meeting-language'); return p.options[p.selectedIndex].text")
        check("the settings detecting it, the page says « détectée »", said == "Celle des Réglages — détectée", said)
    # SpeechAnalyzer hears (a program in its place, as X has it): it detects none, and hears French
    # and English alone — the system's when nothing names another (D64).
    program = WORK / "speaking-speech"
    program.write_text("#!/bin/sh\necho '{\"available\":true,\"installed\":true,\"supported\":true}'\n")
    program.chmod(0o755)
    window = Window(meetings, "fr_FR.UTF-8", f'reminder = false\nmodel = "{model}"\nengine = "speechanalyzer"\nmeeting_language = "de"\n')
    window.env.update(WHOLEHEARD_SPEECH=str(program))
    with window as browser:
        browser.execute_script(f"location.hash = '#/meeting/{heard.name}'")
        wait_for(browser, lambda b: b.find_elements(By.ID, "meeting-language"), 10)
        offered = [o.text for o in Select(browser.find_element(By.ID, "meeting-language")).options]
        check("SpeechAnalyzer hearing: the page says the system's language, never « détectée », and offers French and English alone",
              offered == ["Celle des Réglages — Français", "Français", "English"], offered)
    with window as browser:
        press(browser, "settings-link")
        wait_for(browser, lambda b: b.find_elements(By.ID, "meetings-language"), 10)
        chosen = browser.execute_script("const p = document.getElementById('meetings-language'); return p.selectedIndex < 0 ? '' : p.options[p.selectedIndex].text")
        check("Settings too: the meetings' language SpeechAnalyzer does not hear stays chosen, said as heard",
              chosen == "Deutsch — SpeechAnalyzer entend celle du système", chosen)
    # Its own language written while Whisper heard, one SpeechAnalyzer does not hear: said as heard.
    (heard / "language").write_text("de\n")
    with window as browser:
        browser.execute_script(f"location.hash = '#/meeting/{heard.name}'")
        wait_for(browser, lambda b: b.find_elements(By.ID, "meeting-language"), 10)
        own = Select(browser.find_element(By.ID, "meeting-language")).first_selected_option.text
        check("a meeting's own language SpeechAnalyzer does not hear: kept, said with the one it hears", own == "Deutsch — SpeechAnalyzer entend Français", own)
    (heard / "language").unlink()


def app_language(now: dt.datetime) -> None:
    """AF. The language Wholeheard speaks, chosen in Settings, the system's by default (D64)."""
    print("AF. the app's language chosen in Settings: spoken at once and after a restart, never the meetings' (D64)")
    meetings = WORK / "speaking-app"
    heard = meetings / "2026-10-28_09-00"
    session(heard, now - dt.timedelta(days=1), 0.1, {"local.wav": 1000})
    model = WORK / "speaking-app-model.bin"
    model.write_bytes(b"\0" * 16)
    # SpeechAnalyzer heard here as on a Mac with its model (as X has it): told a language, never detecting one.
    program, asked = WORK / "speaking-app-speech", WORK / "speaking-app-asked"
    program.write_text(f"""#!/bin/sh
echo "$@" >> '{asked}'
echo '{{"available":true,"installed":true,"supported":true}}'
""")
    program.chmod(0o755)
    window = Window(meetings, "fr_FR.UTF-8", f'reminder = false\nmodel = "{model}"\nengine = "speechanalyzer"\n')
    window.env.update(WHOLEHEARD_SPEECH=str(program))
    args = WORK / "hearing-args"
    told = lambda: args.read_text() if args.exists() else ""
    french = ["Ouvrir Wholeheard", "Démarrer un enregistrement", "Transcrire le dernier enregistrement",
              "Rédiger le compte rendu de la dernière réunion", "Quitter Wholeheard"]
    english = ["Open Wholeheard", "Start recording", "Transcribe the last recording", "Write the minutes of the last recording", "Quit Wholeheard"]
    sections = lambda b: b.execute_script("return [document.getElementById('settings-link').textContent, document.documentElement.lang]")
    redo = lambda b: b.execute_script("window.__TAURI__.core.invoke('transcript_redo', {name: arguments[0], model: arguments[1]})", heard.name, str(model))

    def by_settings(b, ready: str) -> str:
        """The transcript made again by whoever the settings name to hear (D59): what it was told."""
        args.unlink(missing_ok=True)
        count = notified.read_text().count(ready)
        b.execute_script("window.__TAURI__.core.invoke('context_redo', {name: arguments[0]})", heard.name)
        wait_for(b, lambda b: told() != "" and notified.read_text().count(ready) > count, 15)
        return told()

    notified = WORK / "notified-language"
    monitor = subprocess.Popen(["dbus-monitor", "--session", "interface='org.freedesktop.Notifications',member='Notify'"],
                               stdout=open(notified, "w"), stderr=subprocess.DEVNULL)
    try:
        with window as browser:
            menu = wait_for(browser, lambda b: tray_labels() == french, 10)
            check("started in French, the system's: the window and the tray's menu speak it",
                  menu and sections(browser) == ["Réglages", "fr"], (tray_labels(), sections(browser)))
            press(browser, "settings-link")
            wait_for(browser, lambda b: b.find_elements(By.ID, "app-language"), 10)
            pick = Select(browser.find_element(By.ID, "app-language"))
            label = browser.execute_script("return document.querySelector('label[for=app-language] .name')?.textContent")
            check("Settings offer the language, the system's by default, each other in its own",
                  [o.text for o in pick.options] == ["Langue du système", "Français", "English"]
                  and pick.first_selected_option.get_attribute("value") == "" and label == "Langue de Wholeheard",
                  ([o.text for o in pick.options], label))
            redo(browser)
            wait_for(browser, lambda b: "Transcription prête" in notified.read_text(), 15)
            before = by_settings(browser, "Transcription prête")
            spoke = len(asked.read_text())
            choose(browser, "app-language", "en")
            saved = wait_for(browser, lambda b: 'language = "en"' in window.file.read_text(), 5)
            named = lambda b: b.execute_script("return document.querySelector('label[for=app-language] .name')?.textContent")
            # The page drawn again once SpeechAnalyzer has said what it offers: a moment after the sidebar.
            spoken = wait_for(browser, lambda b: sections(b) == ["Settings", "en"] and named(b) == "Language of Wholeheard", 5)
            label = named(browser)
            check("English chosen: written, and the window speaks it at once — the choice keeps the focus",
                  saved and spoken and label == "Language of Wholeheard"
                  and browser.execute_script("return document.activeElement.id") == "app-language",
                  (window.file.read_text(), sections(browser), label))
            check("the tray's menu speaks it at once, with no restart", wait_for(browser, lambda b: tray_labels() == english, 5), tray_labels())
            mark = len(notified.read_text())
            after = by_settings(browser, "Transcript ready")
            check("a notification speaks it", "Transcript ready" in notified.read_text()[mark:] and "Transcription prête" not in notified.read_text()[mark:],
                  notified.read_text()[mark:][-300:])
            check("SpeechAnalyzer hears the meeting in the system's language, before and after: never the app's",
                  "--engine speechanalyzer --language fr" in before and after == before, (before, after))
            check("and is asked of the system's language alone", "status fr-FR" in asked.read_text()[spoke:] and "en-" not in asked.read_text(), asked.read_text())
            main = browser.current_window_handle
            browser.execute_script("document.getElementById('small-link').click()")
            wait_for(browser, lambda b: len(b.window_handles) == 2, 5)
            browser.switch_to.window([h for h in browser.window_handles if h != main][0])
            wait_for(browser, lambda b: b.find_elements(By.ID, "small-record"), 5)
            small = [browser.find_element(By.ID, "small-record").text, browser.execute_script("return document.documentElement.lang")]
            check("the small window speaks it", small == ["Start a recording", "en"], small)
            try:
                browser.find_element(By.ID, "small-full").click()
            except NoSuchWindowException:
                pass  # the small window goes with the click
            browser.switch_to.window(main)
            wait_for(browser, lambda b: len(b.window_handles) == 1, 5)
            # The assistant, opened again from Settings (D58): its questions in the language chosen.
            browser.execute_script("location.hash = '#/welcome/again'")
            asking = "return document.getElementById('welcome-heading')?.textContent"
            asked_in = wait_for(browser, lambda b: b.execute_script(asking) == "Do your meetings discuss confidential matters?", 5)
            check("the assistant speaks it", asked_in, browser.execute_script(asking))
            browser.execute_script("location.hash = '#/'")
        # A service hears from now on, one on this computer: agreed, no key (D43).
        window.file.write_text(window.file.read_text().replace('engine = "speechanalyzer"', 'engine = "service"')
                               + 'transcription_provider = "http://127.0.0.1:9/v1"\ntranscription_model = "hears"\n')
        with window as browser:
            menu = wait_for(browser, lambda b: tray_labels() == english, 10)
            check("started again, the system still French: it speaks the one chosen", menu and sections(browser) == ["Settings", "en"],
                  (tray_labels(), sections(browser)))
            press(browser, "settings-link")
            wait_for(browser, lambda b: b.find_elements(By.ID, "app-language"), 10)
            kept = Select(browser.find_element(By.ID, "app-language")).first_selected_option.text
            check("Settings say the one chosen", kept == "English", kept)
            before = by_settings(browser, "Transcript ready")
            choose(browser, "app-language", "")
            back = wait_for(browser, lambda b: "language" not in window.file.read_text() and sections(b) == ["Réglages", "fr"], 5)
            menu = wait_for(browser, lambda b: tray_labels() == french, 5)
            check("the system's again: the key gone, French at once in the window and the menu", back and menu,
                  (window.file.read_text(), sections(browser), tray_labels()))
            after = by_settings(browser, "Transcription prête")
            check("a service is told the same before and after: no language, the meetings' naming none",
                  "--provider http://127.0.0.1:9/v1" in before and after == before and "--language" not in after, (before, after))
            refused = browser.execute_script(
                "return window.__TAURI__.core.invoke('settings_saved', {change: {language: 'de'}}).then(() => 'kept', (why) => String(why))")
            check("a language it does not speak is refused, nothing written", refused != "kept" and "language" not in window.file.read_text(), refused)
    finally:
        monitor.terminate()
        monitor.wait(timeout=10)


def confidential_every_way(now: dt.datetime) -> None:
    """AI. A meeting said confidential (D60), its transcript made again every way the criterion names
    — from the tray, by Réessayer, by « Refaire avec ce contexte » — and its minutes from the tray:
    Whisper and Ollama here, nothing asked of the services the settings name."""
    print("AI. a confidential meeting made again from the tray, by Réessayer, by Refaire avec ce contexte: here alone (D60)")
    counted = {"hear": 0, "write": 0}

    class Service(http.server.BaseHTTPRequestHandler):
        protocol_version = "HTTP/1.0"

        def do_POST(self):
            counted[self.path.strip("/")] += 1
            self.rfile.read(int(self.headers.get("content-length") or 0))
            self.send_response(200)
            self.send_header("content-length", "0")
            self.end_headers()

        def log_message(self, *_):
            pass

    class Ollama(http.server.BaseHTTPRequestHandler):
        protocol_version = "HTTP/1.0"

        def do_GET(self):
            body = json.dumps({"models": [{"name": "qwen3:8b"}]}).encode()
            self.send_response(200)
            self.send_header("content-length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def log_message(self, *_):
            pass

    servers = [http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler) for handler in (Service, Ollama)]
    for server in servers:
        threading.Thread(target=server.serve_forever, daemon=True).start()
    standin, base = (f"http://127.0.0.1:{server.server_port}" for server in servers)
    model = WORK / "every-way-model.bin"
    model.write_bytes(b"\0" * 16)
    named = (f'reminder = false\nmodel = "{model}"\n'
             'engine = "service"\ntranscription_provider = "https://hears.example/v1"\ntranscription_model = "whisper-1"\n'
             'transcription_agreed = "hears.example"\nminutes_provider = "https://writes.example/v1"\nminutes_model = "gpt-4.1-mini"\n'
             'minutes_agreed = "writes.example"\n')
    meetings = WORK / "every-way"
    meeting = meetings / "2026-10-28_15-00"
    session(meeting, now - dt.timedelta(hours=3), 1, {"local.wav": 1000})
    (meeting / "confidential").write_bytes(b"")
    (meeting / "context.txt").write_text("Budget.")
    hearing, writing = WORK / "hearing-args", WORK / "summary-args"
    heard_here = lambda: hearing.exists() and "--provider" not in hearing.read_text()

    def header() -> dict:
        try:
            return json.loads((meeting / "transcript.jsonl").read_text().splitlines()[0])
        except (FileNotFoundError, IndexError, ValueError):
            return {}

    window = Window(meetings, "fr_FR.UTF-8", named, ollama=base)
    window.env.update(STANDIN=standin)
    try:
        with window as browser:
            wait_for(browser, lambda b: tray_labels() != [], 10)
            # From the tray: the last recording's transcript, the engine named a service.
            hearing.unlink(missing_ok=True)
            clicked = tray_click("Transcrire le dernier enregistrement")
            made = wait_for(browser, lambda b: hearing.exists() and header().get("engine") == "whisper.cpp", 15)
            check("from the tray: Whisper hears it here, the service named in the settings asked nothing",
                  clicked and made and heard_here() and counted["hear"] == 0, (hearing.read_text() if hearing.exists() else "", counted))
            # By Réessayer: a making that failed, asked again — once the tray's is over: one being
            # made is refused, as the page's button would be.
            wait_for(browser, lambda b: b.execute_script(
                "return window.__TAURI__.core.invoke('makings').then(m => m.transcript === null && m.transcripts_waiting.length === 0)"), 15)
            (WORK / "fail-hearing").touch()
            hearing.unlink(missing_ok=True)
            browser.execute_script("window.__TAURI__.core.invoke('context_redo', {name: arguments[0]})", meeting.name)
            failed = wait_for(browser, lambda b: hearing.exists() and b.execute_script(
                "return window.__TAURI__.core.invoke('makings').then(m => m.transcript === null && m.transcripts_waiting.length === 0)"), 15)
            (WORK / "fail-hearing").unlink()
            hearing.unlink(missing_ok=True)
            browser.execute_script("window.__TAURI__.core.invoke('transcript_again', {name: arguments[0]})", meeting.name)
            again = wait_for(browser, lambda b: hearing.exists(), 15)
            check("by Réessayer: here again, nothing asked of the service", failed and again and heard_here() and counted["hear"] == 0,
                  (hearing.read_text() if hearing.exists() else "", counted))
            # From the tray: the last recording's minutes.
            wait_for(browser, lambda b: b.execute_script(
                "return window.__TAURI__.core.invoke('makings').then(m => m.minutes === null && m.minutes_waiting.length === 0)"), 15)
            writing.unlink(missing_ok=True)
            clicked = tray_click("Rédiger le compte rendu de la dernière réunion")
            written = wait_for(browser, lambda b: writing.exists(), 15)
            check("its minutes from the tray: Ollama here, no agreement asked, nothing sent",
                  clicked and written and f"--provider {base}/v1" in writing.read_text() and counted == {"hear": 0, "write": 0}
                  and not browser.execute_script("return location.hash").startswith("#/agree"), (writing.read_text() if writing.exists() else "", counted))
            # By « Refaire avec ce contexte »: the transcript and the minutes, here.
            wait_for(browser, lambda b: b.execute_script(
                "return window.__TAURI__.core.invoke('makings').then(m => m.transcript === null && m.minutes === null && m.minutes_waiting.length === 0)"), 15)
            hearing.unlink(missing_ok=True)
            writing.unlink(missing_ok=True)
            browser.execute_script("window.__TAURI__.core.invoke('context_redo', {name: arguments[0]})", meeting.name)
            redone = wait_for(browser, lambda b: hearing.exists() and writing.exists(), 20)
            check("by Refaire avec ce contexte: Whisper and Ollama here, nothing asked of either service",
                  redone and heard_here() and f"--provider {base}/v1" in writing.read_text() and counted == {"hear": 0, "write": 0},
                  (hearing.read_text() if hearing.exists() else "", writing.read_text() if writing.exists() else "", counted))
    finally:
        for server in servers:
            server.shutdown()


def minutes_copied(now: dt.datetime) -> None:
    """AJ. A meeting's minutes copied in one click, ready to paste where the user writes (D45)."""
    print("AJ. the minutes copied in one click: their Markdown, titled as the page names them, « Copié » said in place (D45)")
    meetings = WORK / "copying"
    written, none_, broken = (meetings / name for name in ("2026-10-28_09-00", "2026-10-28_10-00", "2026-10-28_11-00"))
    for folder in (written, none_, broken):
        session(folder, now - dt.timedelta(days=1), 10, {"local.wav": 1000})
        (folder / "transcript.jsonl").write_text(
            '{"type":"header","contract":"transcript/v1","engine":"whisper.cpp","model":"m","language":null,"context":null}\n'
            '{"type":"segment","speaker":"me","track":"local","start":0.0,"end":1.0,"text":"Bonjour."}\n{"type":"complete","duration":1.0}\n')
    (written / "minutes.md").write_text(
        "---\ncontract: minutes/v1\nmade: 2026-10-28T10:00:00+01:00\nprovider: local\nmodel: qwen3:8b\ncontext: null\n---\n\n"
        "## 1.\n\nLe budget est validé.\n\n## 2.\n\n- Envoyer le devis — Inès — vendredi\n\n## 3.\n\n- Rien\n")
    (broken / "minutes.md").write_text("pas un compte rendu\n")
    expected = ("## Points saillants\n\nLe budget est validé.\n\n## Actions\n\n- Envoyer le devis — Inès — vendredi\n\n"
                "## Points clés\n\n- Rien\n")
    # The clipboard stood in: what the page writes is kept on the page, never the user's own clipboard.
    stand_in = ("window.__copied = []; const refuse = arguments[0];"
                "Object.defineProperty(navigator, 'clipboard', {configurable: true, value: {writeText: (text) =>"
                " refuse ? Promise.reject(new Error('refused')) : (window.__copied.push(text), Promise.resolve())}})")
    status = "return document.getElementById('minutes-copied')?.textContent ?? null"
    with Window(meetings, "fr_FR.UTF-8", "reminder = false\n") as browser:
        browser.execute_script(f"location.hash = '#/meeting/{written.name}'")
        offered = wait_for(browser, lambda b: b.find_elements(By.ID, "copy-minutes"), 10)
        beside = browser.execute_script(
            "const c = document.getElementById('copy-minutes'), r = document.getElementById('redo-minutes');"
            "return c && r && c.parentElement === r.parentElement && c.nextElementSibling === r")
        check("a meeting with minutes offers « Copier le compte rendu », beside Refaire…",
              offered and beside and browser.find_element(By.ID, "copy-minutes").text == "Copier le compte rendu", beside)
        browser.execute_script(stand_in, False)
        press(browser, "copy-minutes")
        said = wait_for(browser, lambda b: b.execute_script(status) == "Copié", 3)
        copied = browser.execute_script("return window.__copied")
        check("clicked: their Markdown copied, without the front matter, titled as the page names them; « Copié » said in place",
              said and copied == [expected] and browser.find_element(By.ID, "minutes-copied").get_attribute("role") == "status", copied)
        gone = wait_for(browser, lambda b: b.execute_script(status) == "", 4)
        check("« Copié » said for a moment", gone, browser.execute_script(status))
        browser.execute_script(stand_in, True)
        press(browser, "copy-minutes")
        refused = wait_for(browser, lambda b: b.execute_script(status) == "Le compte rendu n’a pas pu être copié.", 3)
        check("the clipboard refused: one sentence says so, the minutes as they were",
              refused and browser.find_elements(By.CSS_SELECTOR, "#meeting .minutes .body h2"), browser.execute_script(status))
        for folder, why in ((none_, "without minutes"), (broken, "with minutes that do not read")):
            browser.execute_script(f"location.hash = '#/meeting/{folder.name}'")
            wait_for(browser, lambda b: b.execute_script("return document.getElementById('meeting').dataset.name") == folder.name
                     and b.find_elements(By.CSS_SELECTOR, "#meeting .minutes .none"), 10)
            check(f"a meeting {why} offers no copy", not browser.find_elements(By.ID, "copy-minutes"))


def announced(now: dt.datetime) -> None:
    """AM. The reminder said in the small window, the announcement one click away (D50)."""
    print("AM. the reminder under the timer for fifteen seconds, its announcement copied; the reminder off, neither (D50)")
    line = "const a = document.getElementById('small-announce'); return [!!a && !a.hidden, a && getComputedStyle(a).visibility, a && a.textContent]"
    place = "return document.getElementById('small-stop').getBoundingClientRect().top"
    for theme in ("light", "dark"):
        taping = WORK / f"announcing-{theme}"
        taping.mkdir()
        window = Window(taping, "fr_FR.UTF-8", 'after = "nothing"\n', small=True)
        # The dark theme asked of GTK; the other, the desktop's own.
        if theme == "dark":
            window.env.update(GTK_THEME="Adwaita:dark")
        with window as browser:
            browser.find_element(By.ID, "small-record").click()
            wait_for(browser, lambda b: b.find_elements(By.ID, "small-stop") and b.execute_script(line)[0], 10)
            shown = browser.execute_script(line)
            check("recording, the reminder says it under the timer", shown[0] and shown[1] == "visible" and "Prévenez les participants que cette réunion est enregistrée." in shown[2], shown)
            order = browser.execute_script("const l = [...document.querySelector('.small-live').children]; return [l.indexOf(document.querySelector('.small-live .small-top')), l.indexOf(document.getElementById('small-announce')), l.indexOf(document.querySelector('.meters'))]")
            check("under the timer, above the levels", order == sorted(order) and order[1] == order[0] + 1, order)
            # With the note a transcript that follows adds, read at once: the next tick hides it again here.
            fits = browser.execute_script("document.getElementById('small-note').hidden = false; const e = document.scrollingElement; return [e.scrollHeight, e.clientHeight];")
            check("nothing to scroll with it, nor with the note", fits[0] <= fits[1], fits)
            shot(browser, f"d50-announce-{theme}")
            browser.execute_script("window.copied_for_test = null; Object.defineProperty(navigator, 'clipboard', {configurable: true, value: {writeText: async (t) => { window.copied_for_test = t; }}})")
            browser.find_element(By.ID, "small-announce-copy").click()
            copied = wait_for(browser, lambda b: b.find_element(By.ID, "small-announce-copied").text == "Copié", 3)
            text = browser.execute_script("return window.copied_for_test")
            check("Copier l’annonce: the sentence on the clipboard, « Copié » in place", copied and text == "J’enregistre cette réunion pour en faire le compte rendu. Dites-le-moi si vous préférez que je ne le fasse pas.", text)
            before = browser.execute_script(place)
            gone = wait_for(browser, lambda b: b.execute_script(line)[1] == "hidden", 20)
            check("after fifteen seconds it is gone", gone, browser.execute_script(line))
            check("Arrêter has not moved", browser.execute_script(place) == before, [before, browser.execute_script(place)])
            stop_from_small(browser)
    speaking = WORK / "announcing-en"
    speaking.mkdir()
    with Window(speaking, "fr_FR.UTF-8", 'after = "nothing"\nmeeting_language = "en"\n', small=True) as browser:
        browser.find_element(By.ID, "small-record").click()
        wait_for(browser, lambda b: b.find_elements(By.ID, "small-stop") and b.execute_script(line)[0], 10)
        # By script: a slow machine may be past the fifteen seconds, and what is judged here is the sentence.
        browser.execute_script("window.copied_for_test = null; Object.defineProperty(navigator, 'clipboard', {configurable: true, value: {writeText: async (t) => { window.copied_for_test = t; }}})")
        browser.execute_script("document.getElementById('small-announce-copy').click()")
        wait_for(browser, lambda b: b.execute_script("return window.copied_for_test"), 3)
        text = browser.execute_script("return window.copied_for_test")
        check("a meeting said in English: the announcement in English, the window in French", text == "I’m recording this meeting to write up its minutes. Tell me if you’d rather I didn’t.", text)
        # Its own language file comes before the settings', as a meeting's own word does.
        folder = next(p for p in speaking.iterdir() if (p / "session.json").exists())
        (folder / "language").write_text("fr\n")
        browser.execute_script("window.copied_for_test = null")
        time.sleep(0.5)
        browser.execute_script("document.getElementById('small-announce-copy').click()")
        wait_for(browser, lambda b: b.execute_script("return window.copied_for_test"), 3)
        text = browser.execute_script("return window.copied_for_test")
        check("the meeting's own language comes first", text.startswith("J’enregistre"), text)
        # Refused: said of the announcement, in place, nothing moved.
        before = browser.execute_script(place)
        browser.execute_script("Object.defineProperty(navigator, 'clipboard', {configurable: true, value: {writeText: () => Promise.reject(new Error('refused'))}})")
        browser.execute_script("document.getElementById('small-announce-copy').click()")
        refused = wait_for(browser, lambda b: b.find_element(By.ID, "small-announce-copied").text == "Pas copiée.", 3)
        check("a clipboard refused: said of the announcement, Arrêter where it was", refused and browser.execute_script(place) == before, browser.find_element(By.ID, "small-announce-copied").text)
        stop_from_small(browser)
    quiet = WORK / "announcing-off"
    quiet.mkdir()
    with Window(quiet, "fr_FR.UTF-8", 'after = "nothing"\nreminder = false\n', small=True) as browser:
        browser.find_element(By.ID, "small-record").click()
        wait_for(browser, lambda b: b.find_elements(By.ID, "small-stop"), 10)
        time.sleep(1.5)
        check("the reminder off: neither the line nor the button shows", not browser.execute_script(line)[0], browser.execute_script(line))
        stop_from_small(browser)


def heard_checked(now: dt.datetime) -> None:
    """AN. Each side's check, its silence's sentence, in the small window (D49)."""
    print("AN. each side checked live, ✓ then gone; a silence said under its check; a side with no device said, no check; all inside 380 × 400, Arrêter still (D49)")
    place = "return document.getElementById('small-stop').getBoundingClientRect().top"
    state = "return [...document.querySelectorAll('.meters .slot')].map(s => s.dataset.check)"
    glyph = "return [...document.querySelectorAll('.meters .check')].map(c => getComputedStyle(c.querySelector('svg')).display)"
    said = "return [...document.querySelectorAll('.check-says')].map(s => s.textContent)"
    # Nothing cut or out of the window: every line of the live view inside it and inside its own box,
    # the meters' sentences above Arrêter. The note a transcript adds forced on, closed.
    fits = ("const n = document.getElementById('small-note'); const open = document.querySelector('.small-live').classList.contains('context-open'); n.hidden = open;"
            "const e = document.scrollingElement; const stop = document.getElementById('small-stop').getBoundingClientRect();"
            "const outside = [...document.querySelectorAll('.small-live > *, .small-live .says, .small-live .side')].filter(x => { const b = x.getBoundingClientRect();"
            " return b.height > 0 && getComputedStyle(x).position !== 'absolute' && (b.right > innerWidth + 0.5 || b.bottom > innerHeight + 0.5 || x.scrollHeight > x.clientHeight + 1 && x.matches('.says, .small-announce')); }).map(x => x.className);"
            "const says = [...document.querySelectorAll('.check-says')].map(s => s.getBoundingClientRect().bottom);"
            "const line = document.getElementById('small-announce').getBoundingClientRect(); const meters = document.querySelector('.meters').getBoundingClientRect();"
            "return {scroll: [e.scrollHeight, e.clientHeight, e.scrollWidth, e.clientWidth], outside, under: says.every(b => b <= stop.top + 0.5), line: line.height === 0 || line.bottom <= meters.top + 0.5, size: [innerWidth, innerHeight]}")
    def fit(browser, what: str) -> None:
        got = browser.execute_script(fits)
        ok = got["scroll"][0] <= got["scroll"][1] and got["scroll"][2] <= got["scroll"][3] and not got["outside"] and got["under"] and got["line"]
        check(f"{what}: nothing overflows or is cut, at {got['size']}", ok, got)
    expected = {
        "fr_FR.UTF-8": ("fr", "Votre micro\u00a0: entendu", "Le son de la réunion\u00a0: entendu",
                        "Rien n’en arrive\u00a0: vérifiez qu’il n’est pas coupé sur l’appareil lui-même, et qu’il est prêt.",
                        "Rien depuis dix secondes\u00a0: faites jouer un son dans l’appel.", "Votre micro n’est pas enregistré pour l’instant."),
        "en_US.UTF-8": ("en", "Your microphone: heard", "The meeting’s sound: heard",
                        "Nothing comes from it: check it is not muted on the device itself, and that it is ready.",
                        "Nothing heard for ten seconds: play a sound in the call.", "Your microphone is not recorded for now."),
    }
    for language, (short, heard_me, heard_others, nothing, silent, waiting) in expected.items():
        for theme in ("light", "dark"):
            taping = WORK / f"checks-{short}-{theme}"
            taping.mkdir()
            (taping / "hush").touch()
            window = Window(taping, language, 'after = "nothing"\n', small=True)
            if theme == "dark":
                window.env.update(GTK_THEME="Adwaita:dark")
            with window as browser:
                browser.find_element(By.ID, "small-record").click()
                wait_for(browser, lambda b: b.find_elements(By.ID, "small-stop") and b.execute_script(state) == ["listening", "listening"], 10)
                time.sleep(0.5)
                check("levels at silence: no ✓ on either side", browser.execute_script(state) == ["listening", "listening"] and all(c == "none" for c in browser.execute_script(glyph)), browser.execute_script(glyph))
                before = browser.execute_script(place)
                fit(browser, "listening")
                (taping / "hush").unlink()
                rose = wait_for(browser, lambda b: b.execute_script(state) == ["heard", "heard"], 3)
                check("levels risen: ✓ on each side", rose and all(c != "none" for c in browser.execute_script(glyph)), browser.execute_script(glyph))
                told = browser.execute_script("return [...document.querySelectorAll('.check-said')].map(s => [s.getAttribute('role') ?? s.parentElement.getAttribute('role'), s.textContent])")
                check("✓ said to a screen reader, side by side", [t[1] for t in told] == [heard_me, heard_others] and all(t[0] == "status" for t in told), told)
                shot_themed(browser, f"d49-heard-{short}-{theme}", theme)
                # Capture's word: a silence's sentence under its check, D50's line still there, in place.
                (taping / "nothing-me").touch()
                (taping / "silent-others").touch()
                silenced = wait_for(browser, lambda b: b.execute_script(said) == [nothing, silent], 3)
                check("a silence said under its check, as the notifications say it", silenced and browser.execute_script(state) == ["nothing", "silent"], browser.execute_script(said))
                line = browser.execute_script("const a = document.getElementById('small-announce'); return !a.hidden && getComputedStyle(a).visibility")
                check("with D50's line", line == "visible", line)
                fit(browser, "two silences with D50's line")
                check("Arrêter has not moved", browser.execute_script(place) == before, [before, browser.execute_script(place)])
                # Nothing rewritten at each tick: a button or a line rewritten ten times a second loses clicks.
                browser.execute_script("window.written = 0; const seen = new MutationObserver((m) => { window.written += m.length; }); for (const e of document.querySelectorAll('.check-says, .check-said')) seen.observe(e, {subtree: true, childList: true, characterData: true})")
                time.sleep(1)
                check("what does not change is not written again", browser.execute_script("return window.written") == 0, browser.execute_script("return window.written"))
                shot_themed(browser, f"d49-silent-{short}-{theme}", theme)
                browser.find_element(By.ID, "small-context-toggle").click()
                opened = browser.execute_script(place)
                fit(browser, "two silences, the context open")
                room = browser.execute_script("return [...document.querySelectorAll('.meters .check')].map(c => getComputedStyle(c).display !== 'none' && c.getBoundingClientRect().width > 0)")
                check("the context open: each check still shown, each sentence too", room == [True, True] and browser.execute_script(said) == [nothing, silent], room)
                shot_themed(browser, f"d49-context-{short}-{theme}", theme)
                (taping / "heard-me").touch()
                back = wait_for(browser, lambda b: b.execute_script(state)[0] == "heard" and b.execute_script(said)[0] == "", 3)
                check("heard again: ✓, its sentence gone, Arrêter still", back and browser.execute_script(place) == opened, browser.execute_script(said))
                browser.find_element(By.ID, "small-context-toggle").click()
                wait_for(browser, lambda b: b.execute_script(place) == before, 2)
                check("the context closed: Arrêter where it was", browser.execute_script(place) == before, [before, browser.execute_script(place)])
                # ✓ then gone, its device's name left where it was.
                faded = wait_for(browser, lambda b: b.execute_script("const s = document.querySelector('.meters .slot'); const c = s.querySelector('.check'); return s.classList.contains('faded') && getComputedStyle(c).opacity === '0' && c.getBoundingClientRect().width === 0"), 6)
                name = browser.execute_script("const d = document.getElementById('device-me'); return [d.textContent, d.getBoundingClientRect().width > 0]")
                check("✓ fades, the device's name stays", faded and name == ["Mic", True], name)
                check("Arrêter has not moved as it faded", browser.execute_script(place) == before, browser.execute_script(place))
                stop_from_small(browser)
        # The microphone waiting (D66): said truthfully, no check.
        taping = WORK / f"checks-waiting-{short}"
        taping.mkdir()
        (taping / "waiting-me").touch()
        with Window(taping, language, 'after = "nothing"\n', small=True) as browser:
            browser.find_element(By.ID, "small-record").click()
            shown = wait_for(browser, lambda b: b.find_elements(By.ID, "small-stop") and b.execute_script(state) == ["waiting", "heard"], 10)
            check("no microphone: waiting, the other side heard", shown, browser.execute_script(state))
            mark = browser.execute_script("const c = document.querySelector('.meters .check'); return [getComputedStyle(c).display, c.textContent, getComputedStyle(c.querySelector('svg')).display]")
            check("no check for it, nor ✓", mark[0] == "none" and mark[1].strip() == "" and mark[2] == "none", mark)
            check("said truthfully under its meter", browser.execute_script(said) == [waiting, ""], browser.execute_script(said))
            fit(browser, "a side waiting")
            shot(browser, f"d49-waiting-{short}")
            stop_from_small(browser)
    # A user who asked for less motion, in a running window: a side's check and its sentence stand still.
    with asking_less_motion(Window(WORK / "checks-waiting-fr", "fr_FR.UTF-8", 'after = "nothing"\n', small=True)) as browser:
        check("less motion asked: the page's query says so", browser.execute_script(LESS_MOTION))
        browser.execute_script(WATCH_MOTION)
        browser.find_element(By.ID, "small-record").click()
        wait_for(browser, lambda b: b.find_elements(By.CSS_SELECTOR, ".check-says") and b.find_elements(By.CSS_SELECTOR, ".slot[data-check=heard] .check"), 10)
        named = browser.execute_script("return ['.slot[data-check] .check', '.check-says'].map(q => [...document.querySelectorAll(q)].map(e => { const c = getComputedStyle(e); return [c.animationName, c.transitionDuration]; }))")
        check("less motion asked: neither pulse, pop nor fade", all(found and all(said == ["none", "0s"] for said in found) for found in named), named)
        check("less motion asked: nothing in the recording moves", browser.execute_script(MOVING) == [], browser.execute_script(MOVING))
        stop_from_small(browser)


def meeting_named(now: dt.datetime) -> None:
    """AL. A meeting named by its minutes (D48): the title in the row and the page, as text."""
    print("AL. a meeting takes the title its minutes give it — the date beneath it in the list, above it on the page; as text; made again, the new one (D48)")
    meetings = WORK / "named"
    def written(folder: Path, contract: str, title: str | None) -> None:
        named = f"title: {json.dumps(title)}\n" if title is not None else ""
        (folder / "minutes.md").write_text(
            f"---\ncontract: {contract}\nmade: 2026-10-28T15:00:00+01:00\nprovider: local\nmodel: m\ncontext: null\n{named}---\n\n"
            "## 1.\n\nLe budget.\n\n## 2.\n\n- Rien\n\n## 3.\n\n- Rien\n")
    long = "Lancement du produit — calendrier, budget, recrutement, et tout ce qui reste à fixer"[:80]
    named, marked, old, without = [meetings / f"2026-10-2{i}_14-00" for i in (8, 7, 6, 5)]
    for i, folder in enumerate((named, marked, old, without)):
        session(folder, now - dt.timedelta(days=1 + i), 10, {"local.wav": 1000})
    written(named, "minutes/v2", long)
    written(marked, "minutes/v2", "<img src=x onerror=\"window.named_ran=1\">")
    written(old, "minutes/v1", None)
    rows_said = ("return [...document.querySelectorAll('#groups .session')].map(r => ["
                 "r.querySelector('.day').textContent, r.querySelector('.day').title, r.querySelector('.on')?.textContent ?? null])")
    with Window(meetings, "fr_FR.UTF-8", "reminder = false\n") as browser:
        browser.set_window_size(1100, 800)
        wait_for(browser, lambda b: len(b.find_elements(By.CSS_SELECTOR, "#groups .session")) == 4, 10)
        said = browser.execute_script(rows_said)
        check("the title in its row, whole in its tooltip, the date beneath it", said[0][0] == long and said[0][1] == long and bool(said[0][2]) and "·" in said[0][2], said)
        check("minutes made before, or none: the date as today, nothing beneath",
              all("·" in row[0] and row[2] is None for row in said[2:]), said)
        check("markup in a title is shown as text, never as markup",
              said[1][0] == "<img src=x onerror=\"window.named_ran=1\">"
              and not browser.execute_script("return document.querySelector('#groups img') || window.named_ran"), said[1])
        fits = browser.execute_script(
            "const d = document.querySelector('#groups .session .day'); const r = d.closest('.session').getBoundingClientRect();"
            "return [getComputedStyle(d).whiteSpace, getComputedStyle(d).textOverflow, d.getBoundingClientRect().height < 30, d.getBoundingClientRect().right <= r.right]")
        check("a long title on one line, cut by an ellipsis, inside its row", fits == ["nowrap", "ellipsis", True, True], fits)
        browser.execute_script("const f = document.getElementById('find'); f.value = 'recrutement'; f.dispatchEvent(new Event('input'))")
        found = wait_for(browser, lambda b: b.execute_script("return [...document.querySelectorAll('#groups li')].filter(l => !l.hidden).length") == 1, 5)
        check("the search finds a meeting by its title", found)
        browser.execute_script("const f = document.getElementById('find'); f.value = ''; f.dispatchEvent(new Event('input'))")
        head = "return [...document.querySelectorAll('#meeting .named > *')].map(e => [e.tagName, e.className, e.textContent])"
        browser.execute_script(f"location.hash = '#/meeting/{named.name}'")
        page = wait_for(browser, lambda b: b.execute_script(head)[:2:1] and b.execute_script(head)[1][0] == "H1" and b.execute_script(head)[1][2] == long, 10)
        got = browser.execute_script(head)
        check("on its page: the date above, the title its heading", page and got[0][0] != "H1" and "·" in got[0][2], got)
        shot(browser, "d48-named")
        browser.execute_script(f"location.hash = '#/meeting/{marked.name}'")
        as_text = wait_for(browser, lambda b: b.execute_script(head)[1][2].startswith("<img"), 10)
        written(marked, "minutes/v2", "W" * 80)
        browser.execute_script("location.hash = '#/'")
        browser.execute_script(f"location.hash = '#/meeting/{marked.name}'")
        wide = wait_for(browser, lambda b: b.execute_script(head)[1][2] == "W" * 80, 10)
        check("a long title with no space does not widen the page",
              wide and browser.execute_script("const e = document.scrollingElement; return e.scrollWidth <= e.clientWidth"),
              browser.execute_script("return [document.scrollingElement.scrollWidth, document.scrollingElement.clientWidth]"))
        written(marked, "minutes/v2", "<img src=x onerror=\"window.named_ran=1\">")
        check("markup in a title is text on the page too",
              as_text and not browser.execute_script("return document.querySelector('#meeting .named img') || window.named_ran"), browser.execute_script(head))
        browser.execute_script(f"location.hash = '#/meeting/{old.name}'")
        dated = wait_for(browser, lambda b: b.execute_script(head)[0][0] == "H1" and "·" in b.execute_script(head)[0][2], 10)
        check("minutes made before: the date is the heading, as today", dated, browser.execute_script(head))
        # Made again: the new title, read from the file as the list reads again.
        written(named, "minutes/v2", "Un nouveau nom")
        browser.execute_script(f"location.hash = '#/meeting/{named.name}'")
        again = wait_for(browser, lambda b: b.execute_script(head)[1][2] == "Un nouveau nom", 10)
        check("the minutes made again: the meeting takes the new title", again, browser.execute_script(head))
        retitled = wait_for(browser, lambda b: b.execute_script(rows_said)[0][0] == "Un nouveau nom", 10)
        check("its row too", retitled, browser.execute_script(rows_said)[0])


def minutes_checked(now: dt.datetime) -> None:
    """AU. Minutes of `minutes/v3`: what the call cost on the maker's line alone (D51), and a line's
    time that opens its passage and plays its second (D75); Copy without the times."""
    print("AU. minutes/v3: the cost on the maker's line, nowhere else (D51); a time opens its passage and plays its second, keyboard too; none copied (D75)")
    meetings = WORK / "checking"
    played, freed, unplayable, bare, older, unpriced, secret, dear, early, hollow = (
        meetings / f"2026-10-2{n}_09-00" for n in range(10))
    head = '{"type":"header","contract":"transcript/v1","engine":"whisper.cpp","model":"m","language":"fr","context":null}\n'
    # A long meeting: a line every two seconds, its passages far below the fold; one that starts within its second.
    said = "".join(
        json.dumps({"type": "segment", "speaker": "me", "track": "local", "voice": None, "start": n * 2 + (0.6 if n == 55 else 0.0),
                    "end": n * 2 + 1.5, "text": f"Ligne numéro {n}, inventée pour cet essai."}, ensure_ascii=False) + "\n"
        for n in range(60))
    transcript = head + said + '{"type":"complete","duration":120.0}\n'
    front = ("---\ncontract: minutes/{version}\nmade: 2026-10-13T12:21:00+02:00\nprovider: openrouter.ai\nmodel: anthropic/claude-sonnet\ncontext: null\n{more}---\n\n")
    body = ("## 1.\n\nLe budget est validé.\n\n"
            "## 2.\n\n- Mettre à jour le planning — Marc — aujourd’hui [01:40]\n- Couper un serveur — non précisé — non précisé\n- Relancer le fournisseur — Paul [01:50]\n\n"
            "## 3.\n\n- Un serveur est gardé. [01:10]\n- La salle reste à trouver [12:75]\n")
    priced = "cost: 0.01234\ncurrency: USD\n"
    for folder in (played, unplayable, freed, bare, older, unpriced, secret, dear, early, hollow):
        session(folder, now - dt.timedelta(days=2), 2, {})
    for folder in (played, unpriced, secret):
        with wave.open(str(folder / "local.wav"), "wb") as made:
            made.setnchannels(2)
            made.setsampwidth(3)
            made.setframerate(48000)
            made.writeframes(bytes(6 * 48000 * 120))
        (folder / "clock.jsonl").write_text('{"track":"local","sample":0,"at":0.0}\n')
        record = json.loads((folder / "session.json").read_text())
        record["tracks"] = {"local": {"device": "Mic"}}
        (folder / "session.json").write_text(json.dumps(record))
    # A track the window cannot play: there, and not playable.
    (unplayable / "local.wav").write_bytes(b"not a sound")
    for folder in (played, freed, unplayable, older, unpriced, secret):
        (folder / "transcript.jsonl").write_text(transcript)
    (secret / "confidential").write_bytes(b"")
    for folder in (played, freed, unplayable, bare, secret):
        (folder / "minutes.md").write_text(front.format(version="v3", more=priced) + body)
    (unpriced / "minutes.md").write_text(front.format(version="v3", more="cost: -1\ncurrency: USD\n") + body)
    (older / "minutes.md").write_text(front.format(version="v2", more=priced) + body)
    (dear / "minutes.md").write_text(front.format(version="v3", more="cost: 1e30\ncurrency: EUR\n") + body)
    # A time before the first line, that line already in view; nothing heard, and a time past the hour.
    (early / "transcript.jsonl").write_text(head + "".join(line for line in said.splitlines(True)[5:]) + '{"type":"complete","duration":120.0}\n')
    (early / "minutes.md").write_text(front.format(version="v3", more="") + "## 1.\n\nRien.\n\n## 2.\n\n- Rien\n\n## 3.\n\n- Un début [00:05]\n")
    (hollow / "transcript.jsonl").write_text(head + '{"type":"complete","duration":0.0}\n')
    (hollow / "minutes.md").write_text(front.format(version="v3", more="") + "## 1.\n\nRien.\n\n## 2.\n\n- Rien\n\n## 3.\n\n- Le long [65:00]\n")
    open_ = lambda b, folder: (b.execute_script(f"location.hash = '#/meeting/{folder.name}'"), wait_for(
        b, lambda c: c.execute_script("return document.getElementById('meeting').dataset.name") == folder.name
        and c.find_elements(By.CSS_SELECTOR, "#meeting .minutes .body h2"), 10))
    maker = "return document.querySelector('#meeting .minutes .maker').textContent"
    links = "return [...document.querySelectorAll('#meeting .minutes button.line-time')]"
    labels = ("return [...document.querySelectorAll('#meeting .minutes button.line-time')].map(l => [l.textContent, l.getAttribute('aria-label')])")
    row_text = "return [...document.querySelectorAll('#meeting .minutes table.actions tbody tr')].map(r => [...r.cells].map(c => c.textContent.trim()))"
    audio = "const a = document.querySelector('audio.track'); return a ? [a.currentTime, a.paused] : null"
    in_view = ("const l = [...document.querySelectorAll('#meeting .lines li')].find(l => l.textContent.includes(`numéro ${arguments[0]},`));"
               "const r = l.getBoundingClientRect(); return !l.hidden && r.height > 0 && r.top >= 0 && r.bottom <= innerHeight")
    marked = "return document.querySelector('#meeting .lines li.now')?.textContent ?? ''"
    # Where the page stands, whichever element of it scrolls.
    scrolled = "return [...document.querySelectorAll('*')].reduce((sum, e) => sum + e.scrollTop, 0)"
    away = "[...document.querySelectorAll('*')].forEach(e => e.scrollTop = 0)"
    stand_in = ("window.__copied = [];Object.defineProperty(navigator, 'clipboard', {configurable: true, value: {writeText: (text) =>"
                " (window.__copied.push(text), Promise.resolve())}})")
    overflows = ("const wide = [...document.querySelectorAll('#meeting .minutes, #meeting .minutes *')].filter(e => {"
                 "const r = e.getBoundingClientRect(); return !e.closest('th') && r.width > 0 && r.right > innerWidth + 0.5}).map(e => e.tagName + '.' + e.className);"
                 "const page = document.scrollingElement; return [page.scrollWidth - page.clientWidth, wide]")

    for language, short, cost, listen, passage, sections in (
        ("fr_FR.UTF-8", "fr", "0,012 $US", "Écouter depuis", "Voir le passage de", ("Points saillants", "Actions", "Points clés")),
        ("en_US.UTF-8", "en", "$0.012", "Listen from", "See the passage at", ("Highlights", "Actions", "Key points")),
    ):
        with Window(meetings, language, "reminder = false\n") as browser:
            browser.set_window_size(1100, 800)
            open_(browser, played)
            wait_for(browser, lambda b: b.find_elements(By.ID, "play"), 10)
            got = browser.execute_script(maker)
            check(f"{short}: the maker's line ends with the cost, in the window's language, its currency, two significant digits", got.endswith(f" · {cost}"), got)
            colours = browser.execute_script(
                "const m = getComputedStyle(document.querySelector('#meeting .minutes .maker')).color, p = document.createElement('p');"
                "p.style.color = 'var(--soft)'; document.body.append(p); const s = getComputedStyle(p).color; p.remove(); return [m, s]")
            check(f"{short}: in the soft colour", colours[0] == colours[1], colours)
            got = browser.execute_script(row_text)
            check(f"{short}: an action's row keeps its three cells, the time beside what, out of by when (D46)",
                  got[0] == ["Marc", "Mettre à jour le planning 01:40", "aujourd’hui"]
                  and got[1] == ["non précisé", "Couper un serveur", "non précisé"]
                  and got[2] == ["Relancer le fournisseur — Paul 01:50"], got)
            check(f"{short}: an item that does not split in three keeps its time in its place",
                  browser.execute_script("return document.querySelector('#meeting table.actions td.written button.line-time')?.textContent") == "01:50", got)
            got = browser.execute_script(labels)
            check(f"{short}: each time a link that says what it does; a time out of bounds stays the item's text",
                  got == [[at, f"{listen} {at}"] for at in ("01:40", "01:50", "01:10")]
                  and "[12:75]" in browser.find_element(By.CSS_SELECTOR, "#meeting .minutes .body").text
                  and "[01:" not in browser.find_element(By.CSS_SELECTOR, "#meeting .minutes .body").text, got)
            told = browser.execute_script("return [...document.querySelectorAll('#meeting .minutes button.line-time')].map(l => l.title === l.getAttribute('aria-label') && l.title !== '')")
            check(f"{short}: its words also said to a pointer resting on it", told == [True, True, True], told)
            quiet = browser.execute_script(
                "const l = document.querySelector('#meeting .minutes button.line-time'), s = getComputedStyle(l),"
                " b = getComputedStyle(document.querySelector('#meeting .minutes .body li')); return [parseFloat(s.fontSize) < parseFloat(b.fontSize), s.textDecorationLine, s.backgroundColor]")
            check(f"{short}: small, quiet, and a link by more than its colour", quiet[0] and quiet[1] == "underline" and quiet[2] == "rgba(0, 0, 0, 0)", quiet)
            check(f"{short}: the passages are below the fold before a click", not browser.execute_script(in_view, 50))
            browser.execute_script(links)[0].click()
            shown = wait_for(browser, lambda b: b.execute_script(in_view, 50), 3)
            playing = wait_for(browser, lambda b: (a := b.execute_script(audio)) and not a[1] and 100.0 <= a[0] < 102.0, 8)
            check(f"{short}: a click brings its passage into view and plays from that second", shown and playing, browser.execute_script(audio))
            check(f"{short}: the line being said is marked", "numéro 50," in browser.execute_script(marked) or "numéro 51," in browser.execute_script(marked), browser.execute_script(marked))
            # D26: left where the user puts it, while it plays on across lines.
            browser.execute_script(away)
            at = browser.execute_script(scrolled)
            time.sleep(2.5)
            check(f"{short}: the page does not follow what plays (D26)", browser.execute_script(scrolled) == at and not browser.execute_script(audio)[1],
                  (at, browser.execute_script(scrolled)))
            # By the keyboard alone: Enter, then Space, the focus staying on the link.
            third = browser.execute_script(links)[2]
            browser.execute_script("arguments[0].focus()", third)
            browser.switch_to.active_element.send_keys(Keys.ENTER)
            keyed = wait_for(browser, lambda b: b.execute_script(in_view, 35) and (a := b.execute_script(audio)) and 70.0 <= a[0] < 72.0 and not a[1], 8)
            check(f"{short}: Enter on a time does the same", keyed, browser.execute_script(audio))
            check(f"{short}: the focus stays on the link", browser.execute_script("return document.activeElement.textContent") == "01:10",
                  browser.execute_script("return document.activeElement.outerHTML"))
            outline = browser.execute_script("const a = document.activeElement; return [a.matches(':focus-visible'), getComputedStyle(a).outlineStyle]")
            check(f"{short}: reached by the keyboard, it shows its focus", outline == [True, "solid"], outline)
            # A failure said earlier is not left beside a sound that now plays.
            browser.execute_script("document.getElementById('not-played').textContent = 'dit plus tôt'")
            browser.execute_script(links)[1].send_keys(" ")
            spaced = wait_for(browser, lambda b: b.execute_script(in_view, 55) and (a := b.execute_script(audio)) and 110.0 <= a[0] < 112.0 and not a[1], 8)
            check(f"{short}: Space too, without pausing what plays; a line starting within that second is its passage", spaced, browser.execute_script(audio))
            check(f"{short}: what a failure said before is gone",
                  browser.execute_script("return document.getElementById('not-played').textContent") == "", browser.execute_script("return document.getElementById('not-played').textContent"))
            stop = browser.execute_script("return document.querySelector('#meeting .lines li[tabindex=\"0\"]').textContent")
            check(f"{short}: Tab goes on from its passage, the line starting at 110.6 s for a time of 110", "numéro 55," in stop, stop)
            # A search that hides the passage gives way to it.
            search = browser.find_element(By.ID, "search")
            browser.execute_script("arguments[0].scrollIntoView({block: 'nearest'})", search)
            search.send_keys("numéro 3,")
            wait_for(browser, lambda b: not b.execute_script(in_view, 50))
            browser.execute_script(links)[0].click()
            check(f"{short}: a passage a search hid is shown, the search given up",
                  wait_for(browser, lambda b: b.execute_script(in_view, 50), 3) and browser.find_element(By.ID, "search").get_attribute("value") == "",
                  browser.find_element(By.ID, "search").get_attribute("value"))
            browser.execute_script("document.querySelector('audio.track').pause()")
            # Copy: D45's body, the times out, every other bracket as written.
            browser.execute_script(stand_in)
            press(browser, "copy-minutes")
            wait_for(browser, lambda b: b.execute_script("return window.__copied.length") == 1, 3)
            expected = (f"## {sections[0]}\n\nLe budget est validé.\n\n## {sections[1]}\n\n- Mettre à jour le planning — Marc — aujourd’hui\n"
                        f"- Couper un serveur — non précisé — non précisé\n- Relancer le fournisseur — Paul\n\n## {sections[2]}\n\n- Un serveur est gardé.\n- La salle reste à trouver [12:75]\n")
            copied = browser.execute_script("return window.__copied")
            check(f"{short}: Copy leaves the times out, D45's body otherwise as it was", copied == [expected], copied)
            # Both widths, both themes: nothing overflows, nothing cut.
            for width, height in ((420, 800), (1100, 800)):
                browser.set_window_size(width, height)
                time.sleep(0.4)
                got = browser.execute_script(overflows)
                check(f"{short}: at {width} px nothing overflows", got[0] <= 0 and got[1] == [], got)
                for theme in ("light", "dark"):
                    browser.execute_script("document.querySelector('#meeting .minutes').scrollIntoView()")
                    shot_themed(browser, f"d75-{short}-{width}-{theme}", theme)

            # The audio freed: the transcript still answers; what is said never promises a sound.
            for folder, why, note in ((freed, "freed", True), (unplayable, "not playable", True)):
                open_(browser, folder)
                wait_for(browser, lambda b: b.find_elements(By.ID, "no-player") and b.find_elements(By.CSS_SELECTOR, "#meeting .lines li"), 10)
                got = browser.execute_script(labels)
                check(f"{short}: its audio {why}: a time brings its passage into view, and says that alone",
                      got == [[at, f"{passage} {at}"] for at in ("01:40", "01:50", "01:10")], got)
                browser.execute_script(away)
                browser.execute_script(links)[0].click()
                shown = wait_for(browser, lambda b: b.execute_script(in_view, 50), 3)
                check(f"{short}: its audio {why}: the passage shown and marked, nothing plays, the page's own sentence kept",
                      shown and "numéro 50," in browser.execute_script(marked) and browser.execute_script(audio) is None
                      and browser.find_element(By.ID, "no-player").is_displayed(), browser.execute_script(marked))
                browser.execute_script(links)[2].click()
                wait_for(browser, lambda b: b.execute_script(in_view, 35), 3)
                check(f"{short}: one passage marked at a time", browser.execute_script(
                    "return document.querySelectorAll('#meeting .lines li.now').length") == 1 and "numéro 35," in browser.execute_script(marked), browser.execute_script(marked))
                browser.execute_script(links)[1].click()
                check(f"{short}: its audio {why}: a time of 110 marks the line starting at 110.6 s, not the one before",
                      wait_for(browser, lambda b: "numéro 55," in b.execute_script(marked), 3), browser.execute_script(marked))
            shot_themed(browser, f"d75-{short}-freed", "dark")
            # Nothing to play, nothing to open: the time as plain text, no link that leads nowhere.
            open_(browser, bare)
            times = browser.execute_script("return [...document.querySelectorAll('#meeting .minutes .body time')].map(t => [t.textContent, t.closest('button') === null, t.tabIndex])")
            check(f"{short}: no audio, no transcript: each time is quiet text, no link", browser.execute_script(labels) == []
                  and times == [["01:40", True, -1], ["01:50", True, -1], ["01:10", True, -1]], times)
            got = browser.execute_script(row_text)
            check(f"{short}: and its row keeps its three cells", got[0] == ["Marc", "Mettre à jour le planning 01:40", "aujourd’hui"], got)
            # A transcript that holds no line: nothing to open, no link; the time as the page writes it.
            open_(browser, hollow)
            wait_for(browser, lambda b: b.find_elements(By.CSS_SELECTOR, "#meeting .minutes .body time"), 5)
            times = browser.execute_script("return [...document.querySelectorAll('#meeting .minutes .body time')].map(t => t.textContent)")
            check(f"{short}: a transcript with no line: no link, and past the hour the time reads as the transcript writes it",
                  browser.execute_script(labels) == [] and times == ["01:05:00"], times)
            # Before the first line, that line in view: it is the passage, and the page does not move.
            open_(browser, early)
            wait_for(browser, lambda b: b.find_elements(By.CSS_SELECTOR, "#meeting .lines li"), 10)
            # In view low in the pane, far from its centre: the link is out of view, so clicked by the page.
            browser.execute_script(
                "document.querySelector('#meeting .lines li').scrollIntoView({block: 'end'});"
                "const pane = [...document.querySelectorAll('*')].find(e => e.scrollTop > 0 && e.contains(document.getElementById('meeting'))); pane.scrollTop += 40")
            at = browser.execute_script(scrolled)
            check(f"{short}: the first line in view before a click", browser.execute_script(in_view, 5) and at > 0, at)
            browser.execute_script("arguments[0].click()", browser.execute_script(links)[0])
            first = wait_for(browser, lambda b: "numéro 5," in b.execute_script(marked), 3)
            time.sleep(0.5)
            check(f"{short}: a time before the first line opens that line; in view already, the page stays where it is",
                  first and browser.execute_script(scrolled) == at, (browser.execute_script(marked), at, browser.execute_script(scrolled)))
            # Older minutes: exactly as today.
            open_(browser, older)
            wait_for(browser, lambda b: b.find_elements(By.ID, "play") or b.find_elements(By.ID, "no-player"), 10)
            got = browser.execute_script(maker)
            check(f"{short}: minutes/v2 with the same keys: no cost, no time, their brackets their text",
                  not got.endswith(cost) and " · " in got and browser.execute_script(labels) == []
                  and not browser.find_elements(By.CSS_SELECTOR, "#meeting .minutes .body time")
                  and "[01:10]" in browser.find_element(By.CSS_SELECTOR, "#meeting .minutes .body").text, got)
            # A cost out of the contract's bounds: the line as it is without one, the times still links.
            open_(browser, unpriced)
            wait_for(browser, lambda b: b.find_elements(By.ID, "play"), 10)
            got = browser.execute_script(maker)
            check(f"{short}: a cost out of bounds: the line as it is without one", got.count(" · ") == 1, got)
            # Any number the contract allows stays inside the card, whatever its length.
            open_(browser, dear)
            browser.set_window_size(420, 800)
            time.sleep(0.4)
            got = browser.execute_script(overflows)
            check(f"{short}: a cost of thirty digits at 420 px: nothing overflows", got[0] <= 0 and got[1] == [], got)
            browser.set_window_size(1100, 800)
            # Said confidential: the same page; nothing here sends anything.
            open_(browser, secret)
            wait_for(browser, lambda b: b.find_elements(By.ID, "play"), 10)
            browser.execute_script(links)[0].click()
            plays = wait_for(browser, lambda b: (a := b.execute_script(audio)) and not a[1] and 100.0 <= a[0] < 102.0, 8)
            check(f"{short}: a meeting said confidential: its time plays here too, its lock still said",
                  plays and browser.execute_script("return document.getElementById('confidential-switch').checked") is True, browser.execute_script(audio))
            browser.execute_script("document.querySelector('audio.track').pause()")

    # Said confidential, services named in the settings: a time played and the minutes copied send nothing.
    counted = {"hear": 0, "write": 0}

    class Service(http.server.BaseHTTPRequestHandler):
        protocol_version = "HTTP/1.0"

        def do_POST(self):
            counted[self.path.strip("/")] += 1
            self.rfile.read(int(self.headers.get("content-length") or 0))
            self.send_response(200)
            self.send_header("content-length", "0")
            self.end_headers()

        def log_message(self, *_):
            pass

    # An Ollama that holds a model: a step started here on this page's account would run, and be seen.
    class Ollama(http.server.BaseHTTPRequestHandler):
        protocol_version = "HTTP/1.0"

        def do_GET(self):
            answer = json.dumps({"models": [{"name": "qwen3:8b"}]}).encode()
            self.send_response(200)
            self.send_header("content-length", str(len(answer)))
            self.end_headers()
            self.wfile.write(answer)

        def log_message(self, *_):
            pass

    server, here = (http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler) for handler in (Service, Ollama))
    for running in (server, here):
        threading.Thread(target=running.serve_forever, daemon=True).start()
    services = ('reminder = false\nengine = "service"\ntranscription_provider = "https://hears.example/v1"\ntranscription_model = "whisper-1"\n'
                'transcription_agreed = "hears.example"\nminutes_provider = "https://writes.example/v1"\nminutes_model = "gpt-4.1-mini"\nminutes_agreed = "writes.example"\n')
    hearing, writing = WORK / "hearing-args", WORK / "summary-args"
    hearing.unlink(missing_ok=True)
    writing.unlink(missing_ok=True)
    before = {name: (secret / name).read_bytes() for name in ("minutes.md", "transcript.jsonl")}
    try:
        window = Window(meetings, "fr_FR.UTF-8", services, ollama=f"http://127.0.0.1:{here.server_port}")
        window.env.update(STANDIN=f"http://127.0.0.1:{server.server_port}")
        with window as browser:
            browser.set_window_size(1100, 800)
            open_(browser, secret)
            wait_for(browser, lambda b: b.find_elements(By.ID, "play"), 10)
            browser.execute_script(links)[0].click()
            plays = wait_for(browser, lambda b: (a := b.execute_script(audio)) and not a[1] and 100.0 <= a[0] < 102.0, 8)
            browser.execute_script(stand_in)
            press(browser, "copy-minutes")
            copied = wait_for(browser, lambda b: b.execute_script("return window.__copied.length") == 1, 3)
            time.sleep(2)
            check("a meeting said confidential: its time plays, its minutes copy, and nothing is sent: no service asked, neither step started, its files as they were",
                  plays and copied and counted == {"hear": 0, "write": 0} and not hearing.exists() and not writing.exists()
                  and all((secret / name).read_bytes() == kept for name, kept in before.items()), (plays, copied, counted, hearing.exists(), writing.exists()))
    finally:
        server.shutdown()
        here.shutdown()

    # D51's fourth: neither the agreement nor the small window says a price.
    priced_words = ("return /0[,.]01|\\$|\\bUSD\\b|coût|\\bcost\\b|\\bprix\\b|\\bprice\\b/i.test(document.getElementById(arguments[0]).innerText)")
    online = 'reminder = false\nafter = "nothing"\nminutes_provider = "https://writes.example/v1"\nminutes_model = "gpt-4.1-mini"\n'
    with Window(meetings, "fr_FR.UTF-8", online) as browser:
        browser.execute_script(f"location.hash = '#/agree/{played.name}'")
        asked = wait_for(browser, lambda b: b.find_elements(By.ID, "agree-send"), 10)
        check("the agreement before a first sending shows no price", asked and not browser.execute_script(priced_words, "agree"),
              browser.find_element(By.ID, "agree").text)
    with Window(meetings, "fr_FR.UTF-8", online, small=True) as browser:
        check("the small window at rest shows no price, its last meetings listed",
              browser.find_elements(By.ID, "small-record") and not browser.execute_script(priced_words, "small"), browser.find_element(By.ID, "small").text)
        press(browser, "small-record")
        wait_for(browser, lambda b: b.find_elements(By.ID, "small-stop") and b.find_element(By.ID, "small-stop").is_displayed(), 10)
        check("nor while recording, no running count", not browser.execute_script(priced_words, "small"), browser.find_element(By.ID, "small").text)
        stop_from_small(browser)


def word_after_a_gap(now: dt.datetime) -> None:
    """AT. A track with a gap: a line clicked after it plays that line, by the clock's next point (D71)."""
    print("AT. after a gap in a track, a line clicked plays its own second; the track as it was (D71)")
    meetings = WORK / "gapped"
    played = meetings / "2026-10-19_10-00"
    session(played, now - dt.timedelta(hours=1), 0.2, {})
    record = json.loads((played / "session.json").read_text())
    record["tracks"] = {"local": {"device": "Mic", "changed": [{"at": 4.0, "device": None}, {"at": 7.0, "device": "Mic"}]}}
    (played / "session.json").write_text(json.dumps(record))
    # Stopped at 4 s; zeros for 2 s of the 3 the clock leaves; a note at 8 s of the clock, frame 336 000.
    with wave.open(str(played / "local.wav"), "wb") as made:
        made.setnchannels(2)
        made.setsampwidth(3)
        made.setframerate(48000)
        frames = bytearray()
        for n in range(480000):
            sample = int(0.3 * 8388607 * math.sin(2 * math.pi * 880 * n / 48000)) if 336000 <= n < 384000 else 0
            frames += struct.pack("<i", sample)[:3] * 2
        made.writeframes(bytes(frames))
    (played / "clock.jsonl").write_text(
        '{"track":"local","sample":0,"at":0.0}\n{"track":"local","silence":[192000,288000]}\n{"track":"local","sample":288000,"at":7.0}\n')
    (played / "transcript.jsonl").write_text(
        '{"type":"header","contract":"transcript/v1","engine":"whisper.cpp","model":"m","language":"fr","context":null}\n'
        '{"type":"segment","speaker":"me","track":"local","start":1.0,"end":2.0,"text":"Avant le trou."}\n'
        '{"type":"segment","speaker":"me","track":"local","start":8.0,"end":9.0,"text":"Après le trou."}\n{"type":"complete","duration":11.0}\n')
    before = hashlib.sha256((played / "local.wav").read_bytes()).hexdigest()
    with Window(meetings, "fr_FR.UTF-8", "reminder = false\n") as browser:
        browser.set_window_size(1100, 800)
        browser.execute_script(f"location.hash = '#/meeting/{played.name}'")
        wait_for(browser, lambda b: b.find_elements(By.CSS_SELECTOR, ".lines li") and b.find_elements(By.ID, "play"), 10)
        browser.find_element(By.ID, "play").click()
        wait_for(browser, lambda b: b.execute_script("const a = document.querySelector('audio.track'); return !!a && !a.paused"), 10)
        browser.find_elements(By.CSS_SELECTOR, ".lines li")[1].click()
        at = lambda b: b.execute_script("return document.querySelector('audio.track').currentTime")
        jumped = wait_for(browser, lambda b: 8.0 <= at(b) < 8.6 and b.execute_script("return document.querySelector('.lines li.now')?.dataset.start") == "8", 5)
        check("the line after the gap: played from its second, and marked", jumped, at(browser))
        url = browser.execute_script("return document.querySelector('audio.track').src")
        def loud(second: float) -> float:
            start = 44 + 2 * int(second * 48000)
            asked = urllib.request.Request(url, headers={"Range": f"bytes={start}-{start + 2 * 4800 - 1}"})
            with urllib.request.urlopen(asked, timeout=10) as answered:
                body = answered.read()
            values = struct.unpack(f"<{len(body) // 2}h", body)
            return max(abs(v) for v in values) / 32768
        check("what the player is given at that second is that line's sound", loud(8.05) > 0.2, loud(8.05))
        check("and within 0.2 s of it: silence just before, sound just after", loud(7.7) < 0.01 and loud(8.1) > 0.2, (loud(7.7), loud(8.1)))
        check("before the gap, the same place as ever: nothing at 2 s", loud(2.0) < 0.01, loud(2.0))
    check("the track is unchanged (ADR-0004)", hashlib.sha256((played / "local.wav").read_bytes()).hexdigest() == before)


def meeting_renamed(now: dt.datetime) -> None:
    """AS. A meeting renamed by hand (D70): the heading a field, the name everywhere at once, as text."""
    print("AS. a meeting's heading clicked, a name typed: the list, the page and Libérer de l'espace show it; Escape leaves it; emptied, the minutes' title or the date comes back (D70)")
    meetings = WORK / "renaming"
    with_minutes, bare = meetings / "2026-10-28_14-00", meetings / "2026-10-27_14-00"
    session(with_minutes, now - dt.timedelta(days=1), 10, {"local.wav": 1000})
    session(bare, now - dt.timedelta(days=40), 10, {"local.wav": 1000})
    def written(title: str) -> None:
        (with_minutes / "minutes.md").write_text(
            f"---\ncontract: minutes/v2\nmade: 2026-10-28T15:00:00+01:00\nprovider: local\nmodel: m\ncontext: null\ntitle: {json.dumps(title)}\n---\n\n"
            "## 1.\n\nLe budget.\n\n## 2.\n\n- Rien\n\n## 3.\n\n- Rien\n")
    written("Du compte rendu")
    given = with_minutes / "title.txt"
    head = lambda b: b.execute_script("return document.querySelector('#meeting h1')?.textContent")
    row = lambda b, folder: b.execute_script(
        "const r = [...document.querySelectorAll('#groups .session')].find(r => r.getAttribute('href').endsWith(arguments[0])); return r.querySelector('.day').textContent", folder.name)
    # A save draws the page again a moment after the heading changes: pressed and typed through what is there then.
    def opened(b) -> None:
        press(b, "rename")
        wait_for(b, lambda c: c.execute_script("const f = document.getElementById('naming'); if (f && document.activeElement === f) { f.select(); return true; } return false"), 5)
    typed = lambda b, *keys: ActionChains(b).send_keys(*keys).perform()
    def name(b, text: str, key: str) -> None:
        opened(b)
        typed(b, (text or Keys.DELETE) + key)
    with Window(meetings, "fr_FR.UTF-8", 'after = "nothing"\nreminder = false\n') as browser:
        browser.set_window_size(1100, 800)
        browser.execute_script(f"location.hash = '#/meeting/{with_minutes.name}'")
        wait_for(browser, lambda b: head(b) == "Du compte rendu", 10)
        name(browser, "Point budget Q4", Keys.ENTER)
        check("Enter: the page shows it at once", wait_for(browser, lambda b: head(b) == "Point budget Q4", 5), head(browser))
        check("the list too", wait_for(browser, lambda b: row(b, with_minutes) == "Point budget Q4", 5), row(browser, with_minutes))
        check("kept as one line in title.txt", given.read_text() == "Point budget Q4\n", given.read_text())
        check("the focus is back on the heading", browser.execute_script("return document.activeElement.id") == "rename")
        check("the folder keeps its name", with_minutes.is_dir() and (with_minutes / "session.json").is_file())
        name(browser, "Autre chose", Keys.ESCAPE)
        check("Escape: as it was", wait_for(browser, lambda b: head(b) == "Point budget Q4", 5) and given.read_text() == "Point budget Q4\n", head(browser))
        check("Escape: the focus is back on the heading", browser.execute_script("return document.activeElement.id") == "rename")
        # A reading of the page while typing, the window coming forward: what is typed stays.
        opened(browser)
        typed(browser, "Tapé en cours")
        browser.execute_async_script("const done = arguments[arguments.length - 1]; document.getElementById('meeting').dataset.at = '#/other'; refresh().then(done)")
        kept = browser.execute_script("const f = document.getElementById('naming'); return f && f.value === 'Tapé en cours' && document.activeElement === f")
        check("the page read again keeps what is typed, and the focus", kept)
        # The window losing the focus, the field keeping it: nothing is kept yet.
        browser.execute_script("document.getElementById('naming').dispatchEvent(new Event('blur'))")
        time.sleep(0.3)
        # Where the caret was, the page read again.
        browser.execute_script("document.getElementById('naming').setSelectionRange(2, 4)")
        browser.execute_async_script("const done = arguments[arguments.length - 1]; document.getElementById('meeting').dataset.at = '#/other'; refresh().then(done)")
        check("the page read again keeps the caret", browser.execute_script("const f = document.getElementById('naming'); return [f.selectionStart, f.selectionEnd]") == [2, 4])
        check("another window come forward keeps nothing yet", bool(browser.find_elements(By.ID, "naming")) and given.read_text() == "Point budget Q4\n")
        typed(browser, Keys.ESCAPE)
        # The same name, spaces around it: nothing written. Enter twice: saved once.
        stamp = given.stat().st_mtime_ns
        opened(browser)
        typed(browser, "  Point budget Q4  " + Keys.ENTER)
        wait_for(browser, lambda b: b.find_elements(By.ID, "rename"), 5)
        check("the same name with spaces round it writes nothing", given.stat().st_mtime_ns == stamp)
        browser.execute_script("const real = window.__TAURI__.core; window.__saves = 0; window.__TAURI__.core = { ...real, invoke: (asked, ...rest) => { if (asked === 'title_saved') window.__saves++; return real.invoke(asked, ...rest); } }; window.restore = () => { window.__TAURI__.core = real; };")
        opened(browser)
        browser.execute_script("const f = document.getElementById('naming'); f.value = 'Une fois'; for (let i = 0; i < 2; i++) f.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));")
        wait_for(browser, lambda b: head(b) == "Une fois", 5)
        check("Enter twice saves once", browser.execute_script("return window.__saves") == 1, browser.execute_script("return window.__saves"))
        browser.execute_script("window.restore()")
        name(browser, "Point budget Q4", Keys.ENTER)
        wait_for(browser, lambda b: head(b) == "Point budget Q4", 5)
        # A reading of the page while a save is on its way: the field is not opened again over it.
        browser.execute_script("const real = window.__TAURI__.core; window.__TAURI__.core = { ...real, invoke: (asked, ...rest) => asked === 'title_saved' ? new Promise((d) => setTimeout(d, 800)).then(() => real.invoke(asked, ...rest)) : real.invoke(asked, ...rest) }; window.restore = () => { window.__TAURI__.core = real; };")
        opened(browser)
        typed(browser, "Lent" + Keys.ENTER)
        time.sleep(0.2)
        browser.execute_script("document.getElementById('meeting').dataset.at = '#/other'; refresh()")
        wait_for(browser, lambda b: head(b) == "Lent", 5)
        check("a page read during a save: no field left open after it", not browser.find_elements(By.ID, "naming"))
        browser.execute_script("window.restore()")
        name(browser, "Point budget Q4", Keys.ENTER)
        wait_for(browser, lambda b: head(b) == "Point budget Q4", 5)
        # Made again: it stays. Emptied: the minutes' title.
        written("Refait")
        browser.execute_script("refresh()")
        time.sleep(0.5)
        check("the minutes made again: the name stays", head(browser) == "Point budget Q4", head(browser))
        check("the minutes made again leave the file alone", given.read_text() == "Point budget Q4\n")
        name(browser, "", Keys.ENTER)
        check("emptied: the minutes' title comes back", wait_for(browser, lambda b: head(b) == "Refait", 5), head(browser))
        check("emptied: no file left", not given.exists() and not (with_minutes / ".title.part").exists())
        # Unchanged: the minutes' title is not frozen into a name of the user's.
        opened(browser)
        typed(browser, Keys.ENTER)
        check("Enter on the minutes' title unchanged writes nothing", wait_for(browser, lambda b: b.find_elements(By.ID, "rename"), 5) and not given.exists())
        # Leaving the field keeps it.
        opened(browser)
        typed(browser, "En partant")
        browser.find_element(By.ID, "find").click()
        check("leaving the field keeps it", wait_for(browser, lambda b: head(b) == "En partant", 5), head(browser))
        # As text, a path no path.
        mark = "<img src=x onerror=\"window.renamed_ran=1\">"
        name(browser, mark, Keys.ENTER)
        shown = wait_for(browser, lambda b: head(b) == mark and row(b, with_minutes) == mark, 5)
        check("markup is shown as text, on the page and in the list", shown
              and not browser.execute_script("return document.querySelector('#meeting .named img') || document.querySelector('#groups img') || window.renamed_ran"), head(browser))
        browser.execute_script("location.hash = '#/free'")
        freed = wait_for(browser, lambda b: b.execute_script("return [...document.querySelectorAll('#free .freeable .name')].some(n => n.textContent === arguments[0])", mark), 10)
        check("markup is text in Libérer de l'espace too", freed and not browser.execute_script("return document.querySelector('#free img') || window.renamed_ran"))
        browser.execute_script(f"location.hash = '#/meeting/{with_minutes.name}'")
        wait_for(browser, lambda b: head(b) == mark and b.find_elements(By.ID, "rename"), 10)
        name(browser, "../../hors-du-dossier", Keys.ENTER)
        check("a path is a name: shown as text", wait_for(browser, lambda b: head(b) == "../../hors-du-dossier", 5), head(browser))
        check("a path: no folder moved, none made",
              sorted(p.name for p in meetings.iterdir()) == [bare.name, with_minutes.name] and given.read_text() == "../../hors-du-dossier\n", sorted(p.name for p in meetings.iterdir()))
        # No minutes, no name: the date; a name; emptied: the date.
        browser.execute_script(f"location.hash = '#/meeting/{bare.name}'")
        wait_for(browser, lambda b: "·" in (head(b) or ""), 10)
        dated = head(browser)
        name(browser, "W" * 90, Keys.ENTER)
        check("80 characters at most, the field says so by stopping", wait_for(browser, lambda b: head(b) == "W" * 80, 5), head(browser))
        # A heading that fits stays on one line: the button adds nothing to its width.
        name(browser, "Revue d'équipe", Keys.ENTER)
        wait_for(browser, lambda b: head(b) == "Revue d'équipe", 5)
        one = browser.execute_script("const b = document.getElementById('rename'), r = document.createRange(); r.selectNodeContents(b); return r.getClientRects().length")
        check("a short name stays on one line", one == 1, one)
        name(browser, "W" * 90, Keys.ENTER)
        wait_for(browser, lambda b: head(b) == "W" * 80, 5)
        check("the longest name does not widen the page", browser.execute_script("const e = document.scrollingElement; return e.scrollWidth <= e.clientWidth"))
        shot(browser, "d70-named")
        opened(browser)
        shot(browser, "d70-naming")
        typed(browser, "Revue d'équipe" + Keys.ENTER)
        wait_for(browser, lambda b: head(b) == "Revue d'équipe", 5)
        # Libérer de l'espace: the name, the date beside it.
        browser.execute_script("location.hash = '#/free'")
        wait_for(browser, lambda b: b.find_elements(By.CSS_SELECTOR, "#free .freeable .name"), 10)
        freed = browser.execute_script("return [...document.querySelectorAll('#free .freeable')].map(r => [r.querySelector('.name').textContent, r.querySelector('.length').textContent])")
        check("Libérer de l'espace names it, its date beside it", any(n == "Revue d'équipe" and "·" in l for n, l in freed), freed)
        browser.execute_script(f"location.hash = '#/meeting/{bare.name}'")
        wait_for(browser, lambda b: head(b) == "Revue d'équipe", 10)
        name(browser, "", Keys.ENTER)
        check("emptied, no minutes: the date comes back", wait_for(browser, lambda b: head(b) == dated, 5), head(browser))
        # Refused: said, the field still open, what was typed kept.
        browser.execute_script("const real = window.__TAURI__.core; window.__TAURI__.core = { ...real, invoke: (asked, ...rest) => asked === 'title_saved' ? Promise.reject('REFUS') : real.invoke(asked, ...rest) }; window.restore = () => { window.__TAURI__.core = real; };")
        name(browser, "Refusé", Keys.ENTER)
        said = wait_for(browser, lambda b: b.find_elements(By.ID, "naming-said") and b.find_element(By.ID, "naming-said").is_displayed(), 5)
        check("a name refused is said, in French", said and browser.find_element(By.ID, "naming-said").get_attribute("textContent") == "Le nom n’a pas été enregistré\u00a0: REFUS")
        check("refused: the field stays, what was typed in it", browser.find_element(By.ID, "naming").get_attribute("value") == "Refusé")
        # A reading of the page keeps the sentence with the field, and leaves the focus where it is.
        browser.execute_async_script("const done = arguments[arguments.length - 1]; document.getElementById('meeting').dataset.at = '#/other'; refresh().then(done)")
        check("the page read again keeps the refusal", browser.find_element(By.ID, "naming-said").is_displayed() and browser.find_element(By.ID, "naming").get_attribute("value") == "Refusé")
        typed(browser, Keys.ESCAPE)
        # Refused, the user gone elsewhere: the page read again neither brings them back nor tries again.
        opened(browser)
        typed(browser, "Refusé ailleurs")
        browser.execute_script("window.__refused = 0; const was = window.__TAURI__.core; window.__TAURI__.core = { ...was, invoke: (asked, ...rest) => { if (asked === 'title_saved') window.__refused++; return was.invoke(asked, ...rest); } };")
        browser.find_element(By.ID, "find").click()
        wait_for(browser, lambda b: b.find_element(By.ID, "naming-said").is_displayed(), 5)
        for _ in range(2):
            browser.execute_async_script("const done = arguments[arguments.length - 1]; document.getElementById('meeting').dataset.at = '#/other'; refresh().then(done)")
        time.sleep(0.2)
        check("refused, gone elsewhere: the page read again leaves the focus there", browser.execute_script("return document.activeElement.id") == "find", browser.execute_script("return document.activeElement.id"))
        check("refused, gone elsewhere: nothing tried again without a gesture", browser.execute_script("return window.__refused") == 1, browser.execute_script("return window.__refused"))
        # The address left: each way out says it, where it stays.
        press(browser, "settings-link")
        told = wait_for(browser, lambda b: b.find_element(By.ID, "pane-said").is_displayed() and "REFUS" in b.find_element(By.ID, "pane-said").get_attribute("textContent"), 5)
        check("refused, then Réglages: said in the pane", told, browser.find_element(By.ID, "pane-said").get_attribute("textContent"))
        browser.execute_script(f"location.hash = '#/meeting/{bare.name}'")
        wait_for(browser, lambda b: head(b) == dated, 10)
        # The answer coming after the page was left: said there too.
        browser.execute_script("const r = window.__TAURI__.core; window.__TAURI__.core = { ...r, invoke: (asked, ...rest) => asked === 'title_saved' ? new Promise((_, no) => setTimeout(() => no('LENT'), 500)) : r.invoke(asked, ...rest) };")
        opened(browser)
        typed(browser, "Refusé plus tard" + Keys.ENTER)
        browser.execute_script("location.hash = '#/settings'")
        told = wait_for(browser, lambda b: "LENT" in b.find_element(By.ID, "pane-said").get_attribute("textContent") and b.find_element(By.ID, "pane-said").is_displayed(), 5)
        check("refused after the page was left: said in the pane", told, browser.find_element(By.ID, "pane-said").get_attribute("textContent"))
        browser.execute_script(f"location.hash = '#/meeting/{bare.name}'")
        wait_for(browser, lambda b: head(b) == dated, 10)
        browser.execute_script("window.restore(); const real = window.__TAURI__.core; window.__TAURI__.core = { ...real, invoke: (asked, ...rest) => asked === 'title_saved' ? Promise.reject('REFUS') : real.invoke(asked, ...rest) }; window.restore = () => { window.__TAURI__.core = real; };")
        # Refused, then Escape: no sentence for the next address. Refused, then a slower try that saves: none either.
        opened(browser)
        typed(browser, "Refusé" + Keys.ENTER)
        wait_for(browser, lambda b: b.find_element(By.ID, "naming-said").is_displayed(), 5)
        typed(browser, Keys.ESCAPE)
        press(browser, "settings-link")
        wait_for(browser, lambda b: b.find_element(By.ID, "settings").is_displayed(), 5)
        check("refused, then Escape: nothing said at the next address", not browser.find_element(By.ID, "pane-said").is_displayed())
        browser.execute_script(f"location.hash = '#/meeting/{bare.name}'")
        wait_for(browser, lambda b: head(b) == dated, 10)
        opened(browser)
        typed(browser, "Refusé" + Keys.ENTER)
        wait_for(browser, lambda b: b.find_element(By.ID, "naming-said").is_displayed(), 5)
        browser.execute_script("window.restore(); const real = window.__TAURI__.core; window.__TAURI__.core = { ...real, invoke: (asked, ...rest) => asked === 'title_saved' ? new Promise((d) => setTimeout(d, 400)).then(() => real.invoke(asked, ...rest)) : real.invoke(asked, ...rest) }; window.restore = () => { window.__TAURI__.core = real; };")
        browser.execute_script("document.getElementById('naming').blur(); location.hash = '#/settings'")
        wait_for(browser, lambda b: (bare / "title.txt").exists(), 5)
        time.sleep(0.3)
        check("refused, then saved by a slower try: no sentence says otherwise", not browser.find_element(By.ID, "pane-said").is_displayed(), browser.find_element(By.ID, "pane-said").get_attribute("textContent"))
        (bare / "title.txt").unlink()
        browser.execute_script(f"location.hash = '#/meeting/{bare.name}'")
        wait_for(browser, lambda b: head(b) == dated, 10)
        browser.execute_script("window.restore(); const real = window.__TAURI__.core; window.__TAURI__.core = { ...real, invoke: (asked, ...rest) => asked === 'title_saved' ? Promise.reject('REFUS') : real.invoke(asked, ...rest) }; window.restore = () => { window.__TAURI__.core = real; };")
        # Refused while leaving for another meeting: said in the pane, never to no one.
        opened(browser)
        typed(browser, "Refusé en partant")
        browser.execute_script(f"document.getElementById('naming').blur(); location.hash = '#/meeting/{with_minutes.name}'")
        told = wait_for(browser, lambda b: b.find_element(By.ID, "pane-said").is_displayed() and "REFUS" in b.find_element(By.ID, "pane-said").get_attribute("textContent"), 5)
        check("refused on the way to another meeting: said in the pane", told, browser.find_element(By.ID, "pane-said").get_attribute("textContent"))
        browser.execute_script(f"location.hash = '#/meeting/{bare.name}'")
        wait_for(browser, lambda b: head(b) == dated, 10)
        # Refused, then the folder gone: the address is replaced, no hashchange, and it is said all the same.
        gone = meetings / "2026-10-20_14-00"
        session(gone, now - dt.timedelta(days=45), 10, {"local.wav": 1000})
        browser.execute_async_script("const done = arguments[arguments.length - 1]; refresh().then(done)")
        browser.execute_script(f"location.hash = '#/meeting/{gone.name}'")
        wait_for(browser, lambda b: b.find_elements(By.ID, "rename") and b.execute_script("return document.getElementById('meeting').dataset.name") == gone.name, 10)
        opened(browser)
        typed(browser, "Refusé" + Keys.ENTER)
        wait_for(browser, lambda b: b.find_element(By.ID, "naming-said").is_displayed(), 5)
        shutil.rmtree(gone)
        browser.execute_async_script("const done = arguments[arguments.length - 1]; refresh().then(done)")
        told = wait_for(browser, lambda b: b.find_element(By.ID, "pane-said").is_displayed() and "REFUS" in b.find_element(By.ID, "pane-said").get_attribute("textContent"), 5)
        check("refused, then its folder gone: said in the pane", told, browser.find_element(By.ID, "pane-said").get_attribute("textContent"))
        browser.execute_script(f"location.hash = '#/meeting/{bare.name}'")
        wait_for(browser, lambda b: head(b) == dated, 10)
        check("then the next address says nothing", not browser.find_element(By.ID, "pane-said").is_displayed())
        browser.execute_script("window.restore()")
        # Marks that show nothing, as a paste gives them: no name, and nothing to apologise for.
        name(browser, "Provisoire", Keys.ENTER)
        wait_for(browser, lambda b: head(b) == "Provisoire", 5)
        opened(browser)
        browser.execute_script("document.getElementById('naming').value = '\\u200b\\u2060'")
        typed(browser, Keys.ENTER)
        check("marks that show nothing: the name goes, the date back, nothing refused", wait_for(browser, lambda b: head(b) == dated, 5) and not (bare / "title.txt").exists() and not browser.find_elements(By.ID, "naming-said"), head(browser))
        opened(browser)
        typed(browser, "Refusé")
        typed(browser, Keys.ESCAPE)
        check("Escape: the sentence goes, the date as it was", not browser.find_elements(By.ID, "naming-said") and head(browser) == dated, head(browser))
        # The meeting being recorded can be named too.
        live = lambda b: "live" in (b.find_element(By.ID, "record").get_attribute("class") or "")
        recording = meetings / "2026-10-18_12-00"
        browser.find_element(By.ID, "record").click()
        wait_for(browser, lambda b: (recording / "session.json").exists() and live(b), 10)
        browser.execute_script(f"location.hash = '#/meeting/{recording.name}'")
        wait_for(browser, lambda b: b.find_elements(By.ID, "rename") and b.execute_script("return document.getElementById('meeting').dataset.name") == recording.name, 10)
        name(browser, "En direct", Keys.ENTER)
        check("while recording: named at once", wait_for(browser, lambda b: head(b) == "En direct", 5), head(browser))
        check("while recording: kept in its folder", (recording / "title.txt").read_text() == "En direct\n")
        # While it records, Libérer de l'espace names it too, busy, its date beside.
        browser.execute_script("location.hash = '#/free'")
        wait_for(browser, lambda b: b.find_elements(By.CSS_SELECTOR, "#free .freeable.busy"), 10)
        busy = browser.execute_script("return [...document.querySelectorAll('#free .freeable.busy')].map(r => [r.querySelector('.name').textContent, r.querySelector('.length').textContent])")
        check("being recorded, Libérer de l'espace names it, its date beside", bool(busy) and busy[0][0] == "En direct" and "·" in busy[0][1], busy)
        browser.execute_script(f"location.hash = '#/meeting/{recording.name}'")
        wait_for(browser, lambda b: head(b) == "En direct", 10)
        # A press within half a second is the same press.
        time.sleep(0.6)
        browser.find_element(By.ID, "record").click()
        stopped = wait_for(browser, lambda b: "stopped" in (recording / "session.json").read_text() and not live(b), 10)
        check("stopped: the name stays", stopped and (recording / "title.txt").read_text() == "En direct\n" and head(browser) == "En direct", head(browser))


def actions_rows(now: dt.datetime) -> None:
    """AK. A meeting's actions as rows: Qui · Quoi · Pour quand (D46)."""
    print("AK. the minutes' actions as rows — who in its colour, by when in monospace, an item that does not split shown as written (D46)")
    meetings = WORK / "acting"
    folder = meetings / "2026-10-28_14-00"
    session(folder, now - dt.timedelta(days=1), 10, {"local.wav": 1000})
    (folder / "transcript.jsonl").write_text(
        '{"type":"header","contract":"transcript/v1","engine":"whisper.cpp","model":"m","language":null,"context":null}\n'
        '{"type":"segment","speaker":"me","track":"local","start":0.0,"end":1.0,"text":"Bonjour."}\n{"type":"complete","duration":1.0}\n')
    (folder / "minutes.md").write_text(
        "---\ncontract: minutes/v1\nmade: 2026-10-28T15:00:00+01:00\nprovider: local\nmodel: qwen3:8b\ncontext: null\n---\n\n"
        "## 1.\n\nLe budget est validé.\n\n## 2.\n\n- Envoyer le devis — Inès — vendredi\n- Réserver la salle — non précisé — non précisé\n"
        "- Relancer le fournisseur — Paul\n- Rappeler **Jiří** — Marc — 2026-11-04\n"
        "- Valider le budget — Alexandre, Inès, Paul et le comité scientifique — avant la réunion du comité de pilotage de novembre\n\n## 3.\n\n- Rien\n")
    rows = ("return [...document.querySelectorAll('#meeting .minutes table.actions tbody tr')].map(r =>"
            " [...r.cells].map(c => [c.className, c.textContent]))")
    with Window(meetings, "fr_FR.UTF-8", "reminder = false\n") as browser:
        browser.set_window_size(1100, 800)
        browser.execute_script(f"location.hash = '#/meeting/{folder.name}'")
        wait_for(browser, lambda b: b.find_elements(By.CSS_SELECTOR, "#meeting .minutes table.actions"), 10)
        got = browser.execute_script(rows)
        check("each action one row of three cells, Qui · Quoi · Pour quand; one that does not split, as written, in its place",
              got == [[["who", "Inès"], ["what", "Envoyer le devis"], ["when", "vendredi"]],
                      [["who unsettled", "non précisé"], ["what", "Réserver la salle"], ["when unsettled", "non précisé"]],
                      [["written", "Relancer le fournisseur — Paul"]],
                      [["who", "Marc"], ["what", "Rappeler Jiří"], ["when", "2026-11-04"]],
                      [["who long", "Alexandre, Inès, Paul et le comité scientifique"], ["what", "Valider le budget"],
                       ["when long", "avant la réunion du comité de pilotage de novembre"]]], got)
        heads = browser.execute_script(
            "const th = [...document.querySelectorAll('#meeting table.actions thead th')];"
            "return [th.map(t => t.textContent), th.every(t => { const r = t.getBoundingClientRect(); return r.width <= 1 && r.height <= 1; })]")
        check("no visible headers: Qui, Quoi, Pour quand their names for assistive technology", heads == [["Qui", "Quoi", "Pour quand"], True], heads)
        looks = browser.execute_script(
            "const of = (v) => { const p = document.createElement('span'); p.style.color = `var(${v})`; document.querySelector('#meeting').append(p);"
            " const c = getComputedStyle(p).color; p.remove(); return c; };"
            "const s = (q) => getComputedStyle(document.querySelector(q));"
            "return [s('#meeting table.actions td.who:not(.unsettled)').color === of('--accent'), s('#meeting table.actions td.when').fontFamily,"
            " s('#meeting table.actions td.who.unsettled').color === of('--soft')]")
        check("who in its colour, by when in monospace, « non précisé » in the soft colour",
              looks[0] and "monospace" in looks[1] and looks[2], looks)
        # Long names and dates come back to the line: nothing overflows the card, at 1100 nor at 640 px.
        wide = "const b = document.querySelector('#meeting .minutes .body'); return [b.scrollWidth, b.clientWidth]"
        held = browser.execute_script(wide)
        browser.set_window_size(420, 800)
        narrow = wait_for(browser, lambda b: b.execute_script("return innerWidth") <= 420, 5) and browser.execute_script(wide)
        check("a long who or by when comes back to the line: the table never overflows the minutes, at 1100 nor 420 px",
              held[0] <= held[1] and narrow and narrow[0] <= narrow[1], (held, narrow))
        browser.set_window_size(1100, 800)
        wait_for(browser, lambda b: b.execute_script("return innerWidth") > 1000, 5)
        time.sleep(0.5)
        browser.execute_script("document.querySelector('#meeting .minutes').scrollIntoView({block: 'start'})")
        time.sleep(0.3)
        shot(browser, "d46-actions-rows")


def meeting_told_once(now: dt.datetime) -> None:
    """AL. A meeting's notifications are fewer, and a click on one opens the meeting (D44)."""
    print("AL. a window seen: no stop told, the transcript not told before its minutes; their notification's click opens the meeting (D44)")
    meetings = WORK / "telling"
    meetings.mkdir()
    model = WORK / "telling-model.bin"
    model.write_bytes(b"\0" * 16)
    settings = (f'after = "minutes"\nreminder = false\nmodel = "{model}"\n'
                'minutes_provider = "http://127.0.0.1:9/v1"\nminutes_model = "mistral"\n')
    meeting = meetings / "2026-10-18_12-00"
    notified, answered = WORK / "notified-telling", WORK / "answered-telling"
    monitors = [subprocess.Popen(["dbus-monitor", "--session", rule], stdout=open(out, "w"), stderr=subprocess.DEVNULL)
                for rule, out in (("interface='org.freedesktop.Notifications',member='Notify'", notified),
                                  ("type='method_return',sender='org.freedesktop.Notifications'", answered))]
    live = lambda b: "live" in (b.find_element(By.ID, "record").get_attribute("class") or "")
    try:
        time.sleep(0.5)
        with Window(meetings, "fr_FR.UTF-8", settings) as browser:
            browser.find_element(By.ID, "record").click()
            wait_for(browser, lambda b: (meeting / "session.json").exists() and live(b), 10)
            # The button says it at the click now (D68); a press within half a second is the same press.
            time.sleep(0.6)
            browser.find_element(By.ID, "record").click()
            made = wait_for(browser, lambda b: "Compte rendu prêt" in notified.read_text(), 20)
            told = notified.read_text()
            check("a window seen: the stop is not told, nor the transcript before its minutes — their notification alone",
                  made and "Enregistrement arrêté" not in told and "Transcription prête" not in told, told[-600:])
            check("its words say the click opens it", "Cliquez pour l’ouvrir." in told, told[-400:])
            # Away from the meeting, then the notification clicked as the desktop clicks it.
            browser.execute_script("location.hash = '#/settings'")
            wait_for(browser, lambda b: b.find_element(By.ID, "settings").is_displayed(), 5)
            number = re.findall(r"^ +uint32 (\d+)", answered.read_text(), re.M)[-1]
            subprocess.run(["gdbus", "emit", "--session", "--object-path", "/org/freedesktop/Notifications", "--signal",
                            "org.freedesktop.Notifications.ActionInvoked", f"uint32 {number}", "'default'"], capture_output=True)
            opened = wait_for(browser, lambda b: b.execute_script("return location.hash") == f"#/meeting/{meeting.name}"
                              and b.find_element(By.ID, "meeting").is_displayed(), 10)
            check("clicked: the full window opens on that meeting's page, never the file manager", opened,
                  browser.execute_script("return location.hash"))
            # Its transcript made again (Refaire…): ready, its click opens the meeting too.
            (meeting / "local.wav").write_bytes(b"\0" * 1000)
            mark = len(notified.read_text())
            browser.execute_script("window.__TAURI__.core.invoke('transcript_redo', {name: arguments[0], model: arguments[1]})",
                                   meeting.name, str(model))
            again = wait_for(browser, lambda b: "Transcription prête" in notified.read_text()[mark:], 15)
            browser.execute_script("location.hash = '#/settings'")
            wait_for(browser, lambda b: b.find_element(By.ID, "settings").is_displayed(), 5)
            number = re.findall(r"^ +uint32 (\d+)", answered.read_text(), re.M)[-1]
            subprocess.run(["gdbus", "emit", "--session", "--object-path", "/org/freedesktop/Notifications", "--signal",
                            "org.freedesktop.Notifications.ActionInvoked", f"uint32 {number}", "'default'"], capture_output=True)
            reopened = wait_for(browser, lambda b: b.execute_script("return location.hash") == f"#/meeting/{meeting.name}", 10)
            check("a transcript made again says the click opens it, and the click does",
                  again and "Cliquez pour l’ouvrir." in notified.read_text()[mark:] and reopened, browser.execute_script("return location.hash"))
    finally:
        for monitor in monitors:
            monitor.terminate()
            monitor.wait(timeout=10)


def two_panes(now: dt.datetime) -> None:
    """AD. The window in two panes: the meetings in a sidebar, the one chosen beside them (D61)."""
    print("AD. the window in two panes: the sidebar, the meeting chosen beside it, as wide as the window (D61)")
    meetings = WORK / "panes"
    names = [f"2026-10-{day:02d}_10-00" for day in range(10, 24)]
    for index, name in enumerate(names):
        session(meetings / name, now - dt.timedelta(days=len(names) - index), 30, {"local.wav": 1000})
    newest = meetings / names[-1]
    lines = [
        {"type": "header", "contract": "transcript/v1", "engine": "whisper.cpp", "model": "ggml-large-v3-turbo-q8_0", "language": "fr", "context": None},
        *({"type": "segment", "speaker": "me", "track": "local", "voice": None, "start": 2.0 * i, "end": 2.0 * i + 1.5,
           "text": "Une phrase assez longue pour aller à la ligne dans une colonne de lecture, ni plus ni moins."} for i in range(1, 6)),
        {"type": "complete", "duration": 1800.0},
    ]
    (newest / "transcript.jsonl").write_text("".join(json.dumps(line, ensure_ascii=False) + "\n" for line in lines))
    (newest / "minutes.md").write_text(
        "---\ncontract: minutes/v1\nmade: 2026-10-23T11:00:00+02:00\nprovider: openrouter.ai\nmodel: claude-sonnet\ncontext: null\n---\n\n"
        "## 1.\n\n" + "Un compte rendu assez long pour que son texte aille à la ligne bien avant le bord de sa carte. " * 6 + "\n\n"
        "## 2.\n\n- Mettre à jour le planning — Marc — aujourd’hui\n\n## 3.\n\n- Un serveur est gardé pour la démonstration.\n"
    )
    css = (Path(__file__).parent.parent / "ui" / "window.css").read_text()
    light = css[css.index(":root {") + len(":root {"):css.index("}", css.index(":root {"))].replace("\n", " ").strip()
    box = lambda b, selector: b.execute_script(
        "const e = document.querySelector(arguments[0]); if (!e || !e.offsetParent && getComputedStyle(e).position !== 'fixed') return null;"
        "const r = e.getBoundingClientRect(); return [r.left, r.top, r.right, r.bottom];", selector)
    current = lambda b: b.execute_script("return [...document.querySelectorAll('#groups [aria-current=page]')].map(a => a.getAttribute('href'))")
    shown = lambda b: b.execute_script("const m = document.getElementById('meeting'); return m.hidden ? null : m.dataset.name")
    sideways = lambda b: b.execute_script("return document.documentElement.scrollWidth > window.innerWidth")

    def seen(browser, name: str) -> None:
        """A screenshot in the theme its name says: the light one put on the page over a dark desktop."""
        forced = "light" in name and browser.execute_script("return matchMedia('(prefers-color-scheme: dark)').matches")
        if forced:
            browser.execute_script("document.documentElement.style.cssText = arguments[0] + '; color-scheme: light'", light)
        time.sleep(0.3)
        shot(browser, name)
        if forced:
            browser.execute_script("document.documentElement.style.cssText = ''")

    with Window(meetings, "fr_FR.UTF-8") as browser:
        browser.set_window_size(1100, 800)
        wait_for(browser, lambda b: shown(b) == names[-1], 10)
        side, pane = box(browser, "#sidebar"), box(browser, "#pane")
        check("840 px and more: the sidebar on the left, 280 px, the pane beside it",
              side is not None and pane is not None and abs(side[2] - side[0] - 280) <= 1 and side[2] <= pane[0] + 1, (side, pane))
        order = [box(browser, s) for s in ("#record", "#find", "#groups", "#weight", "#settings-link")]
        check("the sidebar holds, top to bottom: the recording, the search, the meetings, the space they take, Réglages",
              None not in order and all(order[i][1] < order[i + 1][1] for i in range(4))
              and all(side[0] <= o[0] and o[2] <= side[2] + 1 for o in order), order)
        check("« Faire de la place » beside the space they take", browser.find_element(By.ID, "free-link").is_displayed()
              and box(browser, "#free-link")[1] >= box(browser, "#weight")[1] - 1)
        check("the header's two tabs are gone", not browser.find_elements(By.ID, "meetings") and not browser.find_elements(By.CSS_SELECTOR, "header.bar"))
        check("the pane shows the last meeting, marked in the list", shown(browser) == names[-1]
              and current(browser) == [f"#/meeting/{names[-1]}"], (shown(browser), current(browser)))
        said = browser.execute_script("return [...document.querySelectorAll('#groups .session')].map(r => ["
                                      "r.querySelector('.day')?.textContent, r.querySelector('.length')?.textContent, r.querySelectorAll('.holds .chip').length])")
        check("each meeting in the list says its title and date, its length and what it holds",
              len(said) == len(names) and all(day and day not in names and " · " in day and any(c.isdigit() for c in day.split(" · ")[0])
                                               and length == "30 min" and chips >= 1 for day, length, chips in said), said[:3])
        browser.execute_script("document.getElementById('list').scrollTop = 100000")
        scrolled = browser.execute_script("return document.getElementById('list').scrollTop")
        browser.find_elements(By.CSS_SELECTOR, "#groups .session")[-1].click()
        came = wait_for(browser, lambda b: shown(b) == names[0], 10)
        kept = browser.execute_script("return document.getElementById('list').scrollTop")
        check("a meeting clicked is shown in the pane, the list kept where it was", came and scrolled > 0 and abs(kept - scrolled) <= 1, (shown(browser), scrolled, kept))
        check("the meeting shown alone marked `aria-current`", current(browser) == [f"#/meeting/{names[0]}"], current(browser))
        browser.find_elements(By.CSS_SELECTOR, "#groups .session")[-1].send_keys(Keys.ARROW_UP)
        moved = wait_for(browser, lambda b: shown(b) == names[1], 10)
        focus = browser.execute_script("return document.activeElement.getAttribute('href')")
        check("↑ on the list shows the meeting above, the focus on its row", moved and focus == f"#/meeting/{names[1]}", (shown(browser), focus))
        browser.find_element(By.CSS_SELECTOR, f"#groups a[href='#/meeting/{names[1]}']").send_keys(Keys.ARROW_DOWN)
        back = wait_for(browser, lambda b: shown(b) == names[0], 10)
        check("↓ the one below", back and browser.execute_script("return document.activeElement.getAttribute('href')") == f"#/meeting/{names[0]}")
        press(browser, "settings-link")
        opened = wait_for(browser, lambda b: b.find_element(By.ID, "settings").is_displayed(), 10)
        width = box(browser, "#settings")
        check("Réglages open in the pane, the sidebar kept, 720 px at most, marked in its place of the list",
              opened and browser.find_element(By.ID, "sidebar").is_displayed() and width and width[2] - width[0] <= 720.5
              and browser.find_element(By.ID, "settings-link").get_attribute("aria-current") == "page" and current(browser) == [],
              (width, current(browser)))
        press(browser, "free-link")
        freeing = wait_for(browser, lambda b: b.find_element(By.ID, "free").is_displayed(), 10)
        check("freeing space opens in the pane, the sidebar kept", freeing and browser.find_element(By.ID, "sidebar").is_displayed())
        browser.execute_script(f"location.hash = '#/meeting/{names[-1]}'")
        wait_for(browser, lambda b: shown(b) == names[-1] and b.find_elements(By.CSS_SELECTOR, ".transcript .lines li"), 10)
        minutes, transcript, drawn = box(browser, ".minutes"), box(browser, ".transcript"), box(browser, "#meeting .drawn")
        check("under 1000 px of pane: the minutes, then the transcript below", minutes and transcript and transcript[1] >= minutes[3], (minutes, transcript))
        browser.set_window_size(1920, 1080)
        wide = wait_for(browser, lambda b: (box(b, ".transcript") or [0, -1])[1] == (box(b, ".minutes") or [0, -2])[1], 5)
        minutes, transcript, drawn, pane = box(browser, ".minutes"), box(browser, ".transcript"), box(browser, "#meeting .drawn"), box(browser, "#pane")
        check("1000 px and more of pane: the minutes and the transcript side by side, together as wide as the audio",
              wide and abs(minutes[0] - drawn[0]) <= 1 and abs(transcript[2] - drawn[2]) <= 1, (minutes, transcript, drawn))
        width = browser.execute_script("return window.innerWidth")
        check("the pane as wide as the window leaves it, no width of its own", abs(pane[2] - width) <= 1 and abs(pane[0] - 280) <= 1, (pane, width))
        reading = browser.execute_script(
            "const t = document.querySelector('.minutes .body'); if (!t) return [0, 0, document.querySelector('.minutes').outerHTML.slice(0, 400)];"
            "return [t.getBoundingClientRect().width, document.querySelector('.minutes').getBoundingClientRect().width, getComputedStyle(t).maxWidth,"
            " getComputedStyle(document.querySelector('.transcript .lines')).maxWidth]")
        # 75ch, as WebKit computes it: in pixels, the width of 75 zeros of the text's font.
        zeros = browser.execute_script(
            "const s = document.createElement('span'); s.textContent = '0'.repeat(75); document.querySelector('.minutes .body').append(s);"
            "s.style.whiteSpace = 'nowrap'; const w = s.getBoundingClientRect().width; s.remove(); return w")
        check("their text 75 characters at most, inside a wider card",
              reading[2].endswith("px") and abs(float(reading[2][:-2]) - zeros) <= 1 and reading[0] <= zeros + 1 and reading[0] < reading[1] - 60
              and reading[3] == reading[2], (reading, zeros))
        # Seen as a person sees it: the list at its top, the pointer away from it.
        browser.execute_script("document.getElementById('list').scrollTop = 0")
        ActionChains(browser).move_to_element(browser.find_element(By.CSS_SELECTOR, "#meeting .drawn")).perform()
        seen(browser, "d61-1920-light")
        seen(browser, "d61-1920-dark")
        browser.set_window_size(420, 400)
        browser.execute_script("location.hash = '#/'")
        alone = wait_for(browser, lambda b: b.find_element(By.ID, "sidebar").is_displayed() and not b.find_element(By.ID, "pane").is_displayed(), 10)
        check("under 840 px: the list alone, nothing overflowing sideways", alone and not sideways(browser))
        seen(browser, "d61-420-list-light")
        seen(browser, "d61-420-list-dark")
        browser.find_elements(By.CSS_SELECTOR, "#groups .session")[0].click()
        page = wait_for(browser, lambda b: b.find_element(By.ID, "pane").is_displayed() and not b.find_element(By.ID, "sidebar").is_displayed(), 10)
        check("then the meeting alone, with a way back, nothing overflowing sideways",
              page and browser.find_element(By.CSS_SELECTOR, "#pane .back").is_displayed() and not sideways(browser))
        seen(browser, "d61-420-meeting-light")
        seen(browser, "d61-420-meeting-dark")
        browser.find_element(By.CSS_SELECTOR, "#pane .back").click()
        again = wait_for(browser, lambda b: b.find_element(By.ID, "sidebar").is_displayed() and not b.find_element(By.ID, "pane").is_displayed(), 10)
        check("the way back is the list again", again)
        browser.set_window_size(1100, 800)
        find = browser.find_element(By.ID, "find")
        find.send_keys("zzz")
        nothing = wait_for(browser, lambda b: not [r for r in b.find_elements(By.CSS_SELECTOR, "#groups .session") if r.is_displayed()]
                           and b.find_element(By.ID, "none-found").is_displayed(), 5)
        check("the search: a word no meeting holds, none shown, said", nothing, browser.find_element(By.ID, "none-found").text)
        find.send_keys(Keys.CONTROL, "a")
        find.send_keys(Keys.BACKSPACE)
        oldest = browser.find_elements(By.CSS_SELECTOR, "#groups .session .day")[-1].get_attribute("textContent")
        find.send_keys(oldest)
        one = wait_for(browser, lambda b: [r.get_attribute("href") for r in b.find_elements(By.CSS_SELECTOR, "#groups .session") if r.is_displayed()]
                       == [b.find_elements(By.CSS_SELECTOR, "#groups .session")[-1].get_attribute("href")], 5)
        check("a word one holds, that one alone", one and not browser.find_element(By.ID, "none-found").is_displayed())
        find.send_keys(Keys.CONTROL, "a")
        find.send_keys(Keys.BACKSPACE)
        browser.execute_script("document.activeElement.blur()")
        browser.find_element(By.TAG_NAME, "body").send_keys(Keys.TAB)
        inside = browser.execute_script("return document.getElementById('sidebar').contains(document.activeElement)")
        check("the keyboard: the sidebar before the pane", inside, browser.execute_script("return document.activeElement.outerHTML.slice(0, 80)"))

    # In English, gone through with the keyboard alone: the sidebar first, the focus always seen.
    with Window(meetings, "en_GB.UTF-8") as browser:
        browser.set_window_size(1100, 800)
        wait_for(browser, lambda b: shown(b) == names[-1] and b.find_elements(By.CSS_SELECTOR, ".transcript .lines li"), 10)
        english = [browser.find_element(By.ID, i).text for i in ("record-said", "settings-link", "free-link")]
        browser.execute_script("document.activeElement.blur()")
        stops = []
        for _ in range(45):
            ActionChains(browser).send_keys(Keys.TAB).perform()
            # Seen: its outline or its shadow — or its `.choice`'s, for a radio — differs from the same
            # element's at rest, measured blurred; an outline at rest, or a shadow it always has, is no focus.
            stops.append(browser.execute_script(
                "const e = document.activeElement, n = e.closest('.choice') ?? e;"
                "const look = () => { const s = getComputedStyle(n); return [s.outlineStyle, s.outlineWidth, s.outlineColor, s.boxShadow].join('|'); };"
                "const focused = look(); e.blur(); const rest = look(); e.focus({focusVisible: true});"
                "const s = getComputedStyle(n), drawn = (s.outlineStyle !== 'none' && parseFloat(s.outlineWidth) > 0) || s.boxShadow !== 'none';"
                "return [document.getElementById('sidebar').contains(e) ? 'side' : document.getElementById('pane').contains(e) ? 'pane' : 'other',"
                " drawn && focused !== rest && document.activeElement === e, e.id || e.className || e.tagName]"))
        places = [place for place, _, _ in stops]
        unseen = [name for _, seen_, name in stops if not seen_]
        # The pane's stops in one run: none of the sidebar's between them — past the last, the page starts again.
        last = len(places) - 1 - places[::-1].index("pane") if "pane" in places else 0
        check("in English, the keyboard reaches the sidebar first, then the pane's stops in one run", english == ["Start a recording", "Settings", "Free space"]
              and places[0] == "side" and "pane" in places and set(places[places.index("pane"):last + 1]) == {"pane"} and "other" not in places,
              (english, places))
        check("the focus is always seen", unseen == [], unseen)

    empty = WORK / "panes-none"
    empty.mkdir()
    with Window(empty, "en_US.UTF-8") as browser:
        said = wait_for(browser, lambda b: b.find_element(By.ID, "none").is_displayed(), 10)
        check("no meeting yet: the pane says what to do, and offers the recording", said and browser.execute_script(
            "return document.getElementById('pane').contains(document.getElementById('none'))")
              and browser.find_element(By.ID, "none-do").text == "Start a recording: its button, its shortcut, or the tray’s icon."
              and browser.find_element(By.ID, "empty-record").text == "Start a recording", browser.find_element(By.ID, "none").text)
        browser.set_window_size(420, 400)
        narrow = wait_for(browser, lambda b: b.find_element(By.ID, "sidebar").is_displayed() and b.find_element(By.ID, "none").is_displayed(), 5)
        check("under 840 px too: the sidebar, then what to do", narrow and not browser.execute_script(
            "return document.documentElement.scrollWidth > window.innerWidth"))


def sidebar_folded(now: dt.datetime) -> None:
    """AE. The sidebar folded by the user, and brought back; the window opened again as it was left (D61)."""
    print("AE. the sidebar folded by its button or Ctrl+B outside a text field, kept by the settings (D61)")
    meetings = WORK / "folding"
    for day in range(3):
        session(meetings / f"2026-10-2{day}_10-00", now - dt.timedelta(days=3 - day), 20, {"local.wav": 1000})
    shown = lambda b, id_: b.find_element(By.ID, id_).is_displayed()
    focused = lambda b: b.execute_script("return document.activeElement.id")
    ctrl_b = lambda b: ActionChains(b).key_down(Keys.CONTROL).send_keys("b").key_up(Keys.CONTROL).perform()
    window = Window(meetings, "fr_FR.UTF-8", "reminder = false\n")
    with window as browser:
        browser.set_window_size(1100, 800)
        wait_for(browser, lambda b: shown(b, "meeting"), 10)
        fold = browser.find_element(By.ID, "fold")
        check("the sidebar's button to fold it, named with its shortcut", shown(browser, "fold") and not shown(browser, "unfold")
              and fold.get_attribute("aria-label") == "Replier la barre latérale" and fold.get_attribute("title") == "Replier la barre latérale (Ctrl+B)",
              (fold.get_attribute("aria-label"), fold.get_attribute("title")))
        press(browser, "fold")
        folded = wait_for(browser, lambda b: not shown(b, "sidebar"), 5)
        pane = browser.execute_script("const r = document.getElementById('pane').getBoundingClientRect(); return [r.left, r.right, window.innerWidth]")
        check("folded: the pane takes the whole width", folded and pane[0] == 0 and abs(pane[1] - pane[2]) <= 1, pane)
        check("a button brings it back, the focus on it", shown(browser, "unfold") and focused(browser) == "unfold", focused(browser))
        saved = wait_for(browser, lambda b: "folded = true" in window.file.read_text(), 5)
        check("the settings keep it folded", saved, window.file.read_text())
        press(browser, "unfold")
        back = wait_for(browser, lambda b: shown(b, "sidebar") and not shown(b, "unfold"), 5)
        check("brought back by its button, the focus on the one that folds it", back and focused(browser) == "fold", focused(browser))
        check("and the settings keep it open", wait_for(browser, lambda b: "folded = false" in window.file.read_text(), 5), window.file.read_text())
        browser.execute_script("document.activeElement.blur()")
        ctrl_b(browser)
        check("Ctrl+B outside a text field folds it", wait_for(browser, lambda b: not shown(b, "sidebar"), 5))
        ctrl_b(browser)
        check("Ctrl+B again brings it back", wait_for(browser, lambda b: shown(b, "sidebar"), 5))
        browser.find_element(By.ID, "find").click()
        ctrl_b(browser)
        time.sleep(0.5)
        check("Ctrl+B in a text field leaves it as it is", shown(browser, "sidebar"))
        browser.execute_script("document.activeElement.blur()")
        ctrl_b(browser)
        wait_for(browser, lambda b: "folded = true" in window.file.read_text(), 5)
        browser.set_window_size(420, 400)
        browser.execute_script("location.hash = '#/'")
        narrow = wait_for(browser, lambda b: shown(b, "sidebar") and not shown(b, "pane"), 5)
        check("under 840 px, folded or not, the list is shown alone, with no button to fold it",
              narrow and not shown(browser, "fold") and not shown(browser, "unfold"))
    again = Window(meetings, "fr_FR.UTF-8", "reminder = false\nfolded = true\n")
    with again as browser:
        browser.set_window_size(1100, 800)
        opened = wait_for(browser, lambda b: shown(b, "meeting") and not shown(b, "sidebar") and shown(b, "unfold"), 10)
        check("the window opens again as it was left: folded", opened)


def transcript_corrected(now: dt.datetime) -> None:
    """AB. A line of a transcript corrected by whoever listens to it (D62, ADR-0014)."""
    print("AB. a transcript's line corrected by whoever listens, the minutes offered again (D62)")
    keys = lambda browser, *sent: ActionChains(browser).send_keys(*sent).perform()
    focused = lambda b: b.execute_script("return document.activeElement.id || document.activeElement.className")

    def when(text) -> dt.datetime:
        """A time as the file says it, or long ago when it says none, or none that reads."""
        try:
            return dt.datetime.fromisoformat(text)
        except (TypeError, ValueError):
            return dt.datetime(1970, 1, 1, tzinfo=dt.timezone.utc)

    meetings = WORK / "correcting"
    heard = meetings / "2026-10-27_09-00"
    session(heard, now - dt.timedelta(days=1), 0.1, {})
    record = json.loads((heard / "session.json").read_text())
    record["tracks"] = {"local": {"device": "Mic"}}
    (heard / "session.json").write_text(json.dumps(record))
    with wave.open(str(heard / "local.wav"), "wb") as made:
        made.setnchannels(1)
        made.setsampwidth(2)
        made.setframerate(48000)
        made.writeframes(bytes(2 * 48000 * 3))
    (heard / "clock.jsonl").write_text('{"track":"local","sample":0,"at":0.0}\n')
    before = (
        '{"type":"header","contract":"transcript/v1","engine":"whisper.cpp","model":"m","language":null,"context":null}\n'
        '{"type":"segment","speaker":"me","track":"local","voice":null,"start":0.4,"end":1.2,"text":"Bonjour à tous, on commence."}\n'
        '{"type":"segment","speaker":"others","track":"remote","voice":null,"start":1.5,"end":2.5,"text":"Le budget de Jiri est validé."}\n'
        '{"type":"complete","duration":3.0}\n'
    )
    (heard / "transcript.jsonl").write_text(before)
    minutes = ("---\ncontract: minutes/v1\nmade: 2026-10-28T10:00:00+01:00\nprovider: local\nmodel: qwen3:8b\ncontext: null\n---\n\n"
               "## 1.\n\nLe budget est validé.\n\n## 2.\n\n- Rien\n\n## 3.\n\n- Rien\n")
    (heard / "minutes.md").write_text(minutes)
    # Not complete: nothing to correct yet.
    unfinished = meetings / "2026-10-27_11-00"
    session(unfinished, now - dt.timedelta(days=1), 0.1, {"local.wav": 1000})
    (unfinished / "transcript.jsonl").write_text(before.replace('{"type":"complete","duration":3.0}\n', ""))
    model = WORK / "correcting-model.bin"
    model.write_bytes(b"\0" * 16)
    settings = f'reminder = false\nmodel = "{model}"\nminutes_provider = "http://127.0.0.1:9/v1"\nminutes_model = "mistral"\n'
    window = Window(meetings, "fr_FR.UTF-8", settings)
    pencils = "return [...document.querySelectorAll('#meeting .lines .edit')].filter(p => !p.hidden).length"
    with window as browser:
        browser.execute_script(f"location.hash = '#/meeting/{unfinished.name}'")
        wait_for(browser, lambda b: b.find_elements(By.CSS_SELECTOR, "#meeting .lines li"), 10)
        refused = browser.execute_script(
            "return window.__TAURI__.core.invoke('transcript_corrected', {name: arguments[0], track: 'local', start: 0.4, text: 'x'}).then(() => 'kept', (why) => String(why))",
            unfinished.name)
        check("a transcript not complete offers no line to correct, and a correction asked is refused, the file as it was",
              browser.execute_script(pencils) == 0 and refused == "sa transcription n’est pas complète\u00a0: refaites-la d’abord." and (unfinished / "transcript.jsonl").read_text() == before.replace('{"type":"complete","duration":3.0}\n', ""),
              (browser.execute_script(pencils), refused))
        browser.execute_script(f"location.hash = '#/meeting/{heard.name}'")
        wait_for(browser, lambda b: b.find_elements(By.CSS_SELECTOR, "#meeting .lines .edit"), 10)
        check("complete, each line has its pencil, named by the line's time", browser.execute_script(pencils) == 2
              and browser.find_elements(By.CSS_SELECTOR, "#meeting .lines .edit")[1].get_attribute("aria-label") == "Corriger la ligne de 00:01")
        check("before any correction, the minutes say nothing of one", not browser.find_elements(By.ID, "corrected-since"))
        # By the keyboard alone: Tab from the first line reaches its pencil; Enter opens the field.
        first = browser.find_element(By.CSS_SELECTOR, "#meeting .lines li")
        browser.execute_script("arguments[0].focus()", first)
        keys(browser, Keys.TAB)
        on_pencil = browser.execute_script("return document.activeElement.classList.contains('edit')")
        keys(browser, Keys.ENTER)
        field = wait_for(browser, lambda b: focused(b) == "correcting", 2)
        check("Tab from a line reaches its pencil, Enter opens the line as a field, focused, holding its text",
              on_pencil and field and browser.execute_script("return document.activeElement.value") == "Bonjour à tous, on commence.")
        # Escape: as it was.
        keys(browser, Keys.END, " encore", Keys.ESCAPE)
        back = wait_for(browser, lambda b: b.execute_script("return document.activeElement.classList.contains('edit')"), 2)
        check("Escape leaves the line as it was, the focus back on its pencil, the file untouched",
              back and (heard / "transcript.jsonl").read_text() == before
              and browser.find_element(By.CSS_SELECTOR, "#meeting .lines .text").text == "Bonjour à tous, on commence.")
        # Emptied: refused, said.
        keys(browser, Keys.ENTER)
        wait_for(browser, lambda b: focused(b) == "correcting", 2)
        browser.execute_script("document.activeElement.select()")
        keys(browser, Keys.DELETE, Keys.ENTER)
        said = wait_for(browser, lambda b: "ne reste pas vide" in b.find_element(By.ID, "correction-said").text, 2)
        check("a line left empty is refused, said, the field kept", said and focused(browser) == "correcting" and (heard / "transcript.jsonl").read_text() == before)
        keys(browser, "Bonjour à toutes, on commence — Γιώργος 🙂", Keys.ENTER)
        kept = wait_for(browser, lambda b: "toutes" in (heard / "transcript.jsonl").read_text(), 3)
        after = (heard / "transcript.jsonl").read_text().splitlines()
        changed = [i for i, (a, b) in enumerate(zip(before.splitlines(), after)) if a != b]
        line = json.loads(after[1]) if len(after) > 1 else {}
        check("Enter keeps it: the file differs by that line alone, its text as typed — any script —, edited when",
              kept and len(after) == 4 and changed == [1] and line.get("text") == "Bonjour à toutes, on commence — Γιώργος 🙂"
              and when(line.get("edited")) > now - dt.timedelta(minutes=30)
              and {k: v for k, v in line.items() if k not in ("text", "edited")} == {k: v for k, v in json.loads(before.splitlines()[1]).items() if k != "text"},
              after)
        # Against the contract's own schema, its formats checked: `edited` a date-time.
        import jsonschema
        schema = json.loads((Path(__file__).resolve().parents[3] / "contracts" / "transcript" / "v1" / "line.schema.json").read_text())
        validator = jsonschema.Draft202012Validator(schema, format_checker=jsonschema.Draft202012Validator.FORMAT_CHECKER)
        invalid = [str(error.message) for kept_line in after for error in validator.iter_errors(json.loads(kept_line))]
        check("the file corrected is valid against transcript/v1's schema, its `edited` a date-time", invalid == [], invalid)
        shown = browser.find_element(By.CSS_SELECTOR, "#meeting .lines li").text
        check("the page shows it, marked « corrigé », the focus back on its pencil",
              "Bonjour à toutes" in shown and "corrigé" in shown and browser.execute_script("return document.activeElement.classList.contains('edit')"), shown)
        # Left by the pointer — a click elsewhere —: what is typed is kept, as Enter keeps it.
        browser.find_elements(By.CSS_SELECTOR, "#meeting .lines .edit")[1].click()
        wait_for(browser, lambda b: focused(b) == "correcting", 2)
        keys(browser, Keys.END, " en novembre")
        browser.find_element(By.CSS_SELECTOR, "#meeting h1, #meeting .meeting-top").click()
        left = wait_for(browser, lambda b: "validé. en novembre" in (heard / "transcript.jsonl").read_text(), 3)
        check("leaving the field keeps what is typed", left, (heard / "transcript.jsonl").read_text().splitlines()[2:3])
        # Pasted with its line break and spaces around: one line, its ends trimmed.
        browser.find_elements(By.CSS_SELECTOR, "#meeting .lines .edit")[1].click()
        wait_for(browser, lambda b: focused(b) == "correcting", 2)
        browser.execute_script("const f = document.activeElement; f.select(); const d = new DataTransfer(); d.setData('text/plain', arguments[0]);"
                               "f.dispatchEvent(new ClipboardEvent('paste', {clipboardData: d, bubbles: true, cancelable: true}))", "  Le budget de Jiri\nest validé.  ")
        keys(browser, Keys.ENTER)
        pasted = wait_for(browser, lambda b: '"text":"Le budget de Jiri est validé."' in (heard / "transcript.jsonl").read_text().replace('": "', '":"'), 3)
        check("a line break pasted becomes a space, the spaces at its ends dropped", pasted, (heard / "transcript.jsonl").read_text().splitlines()[2:3])
        # Drawn again: the minutes, older than the correction, say so and offer to be made again.
        browser.execute_script("location.hash = '#/'")
        wait_for(browser, lambda b: b.find_element(By.ID, "list").is_displayed(), 5)
        browser.execute_script(f"location.hash = '#/meeting/{heard.name}'")
        offered = wait_for(browser, lambda b: b.find_elements(By.ID, "corrected-since"), 10)
        check("the minutes written before it say the transcript was corrected since, and offer « Refaire avec les corrections »",
              offered and "corrigée après ce compte rendu" in browser.find_element(By.ID, "corrected-since").text
              and browser.find_element(By.ID, "redo-corrections").text == "Refaire avec les corrections")
        time.sleep(2)
        check("nothing is made again without that click", (heard / "minutes.md").read_text() == minutes)
        # Refaire… of a corrected transcript says its corrections will not be kept.
        press(browser, "redo-transcript")
        warned = wait_for(browser, lambda b: b.find_elements(By.ID, "redo-loses-corrections"), 5)
        check("Refaire… of a corrected transcript says, before it starts, that its corrections will not be kept",
              warned and "ne seront pas gardées" in browser.find_element(By.ID, "redo-loses-corrections").text)
        browser.execute_script(f"location.hash = '#/'")
        wait_for(browser, lambda b: b.find_element(By.ID, "list").is_displayed(), 5)
        browser.execute_script(f"location.hash = '#/meeting/{heard.name}'")
        wait_for(browser, lambda b: b.find_elements(By.ID, "redo-corrections"), 10)
        # While they are written, no line is corrected: they read the transcript before.
        (WORK / "slow-minutes").touch()
        try:
            press(browser, "redo-corrections")
            writing = wait_for(browser, lambda b: b.execute_script(
                "return window.__TAURI__.core.invoke('makings').then(m => m.minutes?.name === arguments[0])", heard.name), 5)
            asked = browser.execute_script(
                "return window.__TAURI__.core.invoke('transcript_corrected', {name: arguments[0], track: 'remote', start: 1.5, text: 'x'}).then(() => 'kept', (why) => String(why))",
                heard.name)
            hidden = wait_for(browser, lambda b: b.execute_script(pencils) == 0, 3)
            check("while the minutes are written, no line is corrected: refused, its pencils hidden",
                  writing and "en cours d’enregistrement ou de préparation" in asked and hidden and "Le budget de Jiri" in (heard / "transcript.jsonl").read_text(), (writing, asked))
        finally:
            (WORK / "slow-minutes").unlink()
        remade = wait_for(browser, lambda b: (heard / "minutes.md").read_text() != minutes, 10)
        check("« Refaire avec les corrections »: the minutes made again by whoever the settings name, from the corrected transcript",
              remade and "model: mistral" in (heard / "minutes.md").read_text(), (heard / "minutes.md").read_text()[:200])
        # Asked while the transcript is made again: refused — one writer at a time.
        (WORK / "slow-hearing").touch()
        try:
            # Refused while the minutes asked just before are still made: asked again until taken.
            wait_for(browser, lambda b: b.execute_script(
                "return window.__TAURI__.core.invoke('context_redo', {name: arguments[0]}).then(() => true, () => false)", heard.name), 10)
            # Made, or in line to be: what the shell says it is making.
            wait_for(browser, lambda b: b.execute_script(
                "return window.__TAURI__.core.invoke('makings').then(m => m.transcript?.name === arguments[0] || m.transcripts_waiting.includes(arguments[0]))",
                heard.name), 5)
            refused = browser.execute_script(
                "return window.__TAURI__.core.invoke('transcript_corrected', {name: arguments[0], track: 'local', start: 0.4, text: 'x'}).then(() => 'kept', (why) => String(why))",
                heard.name)
        finally:
            (WORK / "slow-hearing").unlink()
        check("a correction asked while the transcript is made again is refused", "en cours d’enregistrement ou de préparation" in refused, refused)
        # Its end waited for: the transcript set aside meanwhile, then the new one in its place.
        def remade() -> bool:
            try:
                return "Refait avec" in (heard / "transcript.jsonl").read_text()
            except FileNotFoundError:
                return False
        wait_for(browser, lambda b: remade(), 10)


def corrections_gated(now: dt.datetime) -> None:
    """AG. « Refaire avec les corrections » through Refaire…'s own gate (D62): a service not agreed to
    asks first, in the pane (D61); a meeting said confidential is made here (D60)."""
    print("AG. the minutes made again with the corrections: the agreement asked in the pane, a confidential meeting made here (D62, D61)")
    ollama = {"held": ["qwen3:8b"]}

    class Ollama(http.server.BaseHTTPRequestHandler):
        protocol_version = "HTTP/1.0"

        def do_GET(self):
            body = json.dumps({"models": [{"name": name} for name in ollama["held"]]}).encode()
            self.send_response(200)
            self.send_header("content-length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def log_message(self, *_):
            pass

    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Ollama)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    base = f"http://127.0.0.1:{server.server_port}"
    meetings = WORK / "gated"
    minutes = ("---\ncontract: minutes/v1\nmade: 2026-10-28T10:00:00+01:00\nprovider: writes.example\nmodel: gpt-4.1-mini\ncontext: null\n---\n\n"
               "## 1.\n\nLe budget est validé.\n\n## 2.\n\n- Rien\n\n## 3.\n\n- Rien\n")
    folders = []
    for name in ("2026-10-26_09-00", "2026-10-26_11-00"):
        folder = meetings / name
        session(folder, now - dt.timedelta(days=3), 0.1, {"local.wav": 1000})
        (folder / "transcript.jsonl").write_text(
            '{"type":"header","contract":"transcript/v1","engine":"whisper.cpp","model":"m","language":null,"context":null}\n'
            '{"type":"segment","speaker":"me","track":"local","voice":null,"start":0.4,"end":1.2,"text":"Le budget est validé.","edited":"2026-10-29T09:00:00+01:00"}\n'
            '{"type":"complete","duration":1.0}\n')
        (folder / "minutes.md").write_text(minutes)
        folders.append(folder)
    opened, secret = folders
    (secret / "confidential").write_bytes(b"")
    online = 'reminder = false\nminutes_provider = "https://writes.example/v1"\nminutes_model = "gpt-4.1-mini"\n'
    front = lambda folder, key: next((line.split(": ", 1)[1] for line in (folder / "minutes.md").read_text().splitlines()
                                      if line.startswith(f"{key}: ")), None)
    args = WORK / "summary-args"
    try:
        with Window(meetings, "fr_FR.UTF-8", online, ollama=base) as browser:
            browser.set_window_size(1100, 800)
            args.unlink(missing_ok=True)
            browser.execute_script(f"location.hash = '#/meeting/{opened.name}'")
            wait_for(browser, lambda b: b.find_elements(By.ID, "redo-corrections"), 10)
            press(browser, "redo-corrections")
            asked = wait_for(browser, lambda b: b.execute_script("return location.hash") == f"#/agree/{opened.name}"
                             and b.find_elements(By.ID, "agree-send"), 10)
            time.sleep(2)
            check("a service not agreed to: « Refaire avec les corrections » asks first, as Refaire… does — the step not started, two seconds on",
                  asked and not args.exists() and (opened / "minutes.md").read_text() == minutes, browser.execute_script("return location.hash"))
            check("the agreement opens in the pane, the sidebar kept beside it (D61)", browser.execute_script(
                "return document.getElementById('pane').contains(document.getElementById('agree'))")
                  and browser.find_element(By.ID, "agree").is_displayed() and browser.find_element(By.ID, "sidebar").is_displayed())
            browser.execute_script(f"location.hash = '#/meeting/{secret.name}'")
            wait_for(browser, lambda b: b.find_elements(By.ID, "redo-corrections"), 10)
            press(browser, "redo-corrections")
            # Made by Ollama on this computer, its model named in the minutes (the played step names no provider then).
            made = wait_for(browser, lambda b: front(secret, "model") == "qwen3:8b", 20)
            check("a meeting said confidential: made here, by Ollama, no agreement asked",
                  made and browser.execute_script("return location.hash") == f"#/meeting/{secret.name}"
                  and args.exists() and f"--provider {base}/v1" in args.read_text(), (front(secret, "model"), args.read_text() if args.exists() else ""))
    finally:
        server.shutdown()


def context_written(now: dt.datetime) -> None:
    """O. A meeting's context (D59): written during a recording, by the keyboard, heard by the
    transcript and the minutes; changed after, made again with it; the field closed meanwhile."""
    print("O. a meeting's context: written while recording, read by the transcript and the minutes, made again with it (D59)")
    keys = lambda browser, *sent: ActionChains(browser).send_keys(*sent).perform()
    focused = lambda b: b.execute_script("return document.activeElement.id")
    fits = lambda b: b.execute_script("const e = document.scrollingElement; return [e.scrollHeight, e.clientHeight];")
    count = lambda b, id_: b.execute_script("return document.getElementById(arguments[0]).textContent", f"{id_}-count")
    model = WORK / "context-model.bin"
    model.write_bytes(b"\0" * 16)
    settings = (f'after = "minutes"\nreminder = false\nmodel = "{model}"\n'
                'minutes_provider = "http://127.0.0.1:9/v1"\nminutes_model = "mistral"\n')
    css = (Path(__file__).parent.parent / "ui" / "window.css").read_text()
    light = css[css.index(":root {") + len(":root {"):css.index("}", css.index(":root {"))].replace("\n", " ").strip()

    def seen(browser, name: str) -> None:
        """A screenshot in the theme its name says: the light one put on the page over a dark desktop."""
        forced = "light" in name and browser.execute_script("return matchMedia('(prefers-color-scheme: dark)').matches")
        if forced:
            browser.execute_script("document.documentElement.style.cssText = arguments[0] + '; color-scheme: light'", light)
            time.sleep(0.3)
        shot(browser, name)
        if forced:
            browser.execute_script("document.documentElement.style.cssText = ''")

    def recording(name: str, language: str, dark: bool = False) -> tuple["Window", Path]:
        meetings = WORK / f"context-{name}"
        meetings.mkdir()
        window = Window(meetings, language, settings, small=True)
        if dark:
            window.env.update(GTK_THEME="Adwaita:dark")
        return window, meetings / "2026-10-18_12-00"

    def to_field(browser) -> bool:
        """Tab from wherever the focus is to the context's disclosure — never past it onto Arrêter's Enter."""
        for _ in range(6):
            if focused(browser) == "small-context-toggle":
                return True
            keys(browser, Keys.TAB)
        return focused(browser) == "small-context-toggle"

    said = "Budget du T4 avec Inès Durand, Jiří Novák et l’équipe achats."
    window, meeting = recording("fr", "fr_FR.UTF-8")
    with window as browser:
        # Started by the keyboard: the recording's gesture has the focus at rest — once the page has it.
        wait_for(browser, lambda b: focused(b) == "small-record", 10)
        keys(browser, Keys.ENTER)
        started = wait_for(browser, lambda b: b.find_elements(By.ID, "small-stop") and b.find_element(By.ID, "small-stop").is_displayed(), 10)
        toggle = browser.find_element(By.ID, "small-context-toggle")
        check("recording, the small window offers the context, closed", started and toggle.text.startswith("Ajouter le contexte")
              and toggle.get_attribute("aria-expanded") == "false" and not browser.find_element(By.ID, "small-context").is_displayed(), toggle.text)
        time.sleep(0.4)
        seen(browser, "d59-1-small-closed-light")
        check("closed, it holds in its window", fits(browser)[0] <= fits(browser)[1], fits(browser))
        reached = to_field(browser)
        keys(browser, Keys.ENTER)
        opened = wait_for(browser, lambda b: focused(b) == "small-context", 2)
        area = browser.find_element(By.ID, "small-context")
        check("reached by Tab and opened by Enter, the field takes the focus, named by its label",
              reached and opened and area.get_attribute("aria-label") == "De quoi parle la réunion ? Qui y participe ?"
              and area.get_attribute("placeholder") == area.get_attribute("aria-label"), (reached, focused(browser), area.get_attribute("aria-label")))
        check("open, the note on the transcript makes room for it", not browser.find_element(By.ID, "small-note").is_displayed())
        check("its count is said where a screen reader hears it, before a word is typed",
              browser.find_element(By.ID, "small-context-count").get_attribute("role") == "status"
              and area.get_attribute("aria-describedby") == "small-context-count"
              and count(browser, "small-context") == "400 caractères au plus · 400 restants", count(browser, "small-context"))
        keys(browser, said)
        left = 400 - len(said)
        counted = wait_for(browser, lambda b: count(b, "small-context") == f"400 caractères au plus · {left} restants", 2)
        check("the count follows each character typed", counted, count(browser, "small-context"))
        kept = wait_for(browser, lambda b: (meeting / "context.txt").exists() and (meeting / "context.txt").read_text() == said, 3)
        check("what is typed is the meeting's context.txt, whole, as typed", kept, (meeting / "context.txt").read_text() if (meeting / "context.txt").exists() else None)
        check("nothing is left beside it", sorted(p.name for p in meeting.iterdir()) == ["context.txt", "session.json"], sorted(p.name for p in meeting.iterdir()))
        time.sleep(0.3)
        check("open and written, it holds in its window — 380 × 400", fits(browser)[0] <= fits(browser)[1]
              and browser.execute_script("return [innerWidth, innerHeight]") == [380, 400], (fits(browser), browser.execute_script("return [innerWidth, innerHeight]")))
        seen(browser, "d59-2-small-open-light")
        # Another script: said in the count, the window still whole.
        keys(browser, " Γιώργος Παπαδόπουλος.")
        other = wait_for(browser, lambda b: "Dans cette écriture" in count(b, "small-context"), 2)
        time.sleep(0.3)
        check("a Greek name is said: Whisper may keep only its end, a service may refuse it", other
              and count(browser, "small-context").startswith(f"{left - len(' Γιώργος Παπαδόπουλος.')} restants"), count(browser, "small-context"))
        check("said, it still holds in its window", fits(browser)[0] <= fits(browser)[1], fits(browser))
        seen(browser, "d59-3-small-script-light")
        for _ in range(len(" Γιώργος Παπαδόπουλος.")):
            keys(browser, Keys.BACKSPACE)
        back = wait_for(browser, lambda b: (meeting / "context.txt").read_text() == said and "Dans cette écriture" not in count(b, "small-context"), 3)
        check("taken out, the context is as it was, the script no longer said", back, (meeting / "context.txt").read_text())
        keys(browser, Keys.ESCAPE)
        closed = wait_for(browser, lambda b: focused(b) == "small-context-toggle", 2)
        toggle = browser.find_element(By.ID, "small-context-toggle")
        check("Escape closes it, the focus back on its disclosure, which says it holds one",
              closed and toggle.text.startswith("Modifier le contexte") and toggle.get_attribute("aria-expanded") == "false", toggle.text)
        stop_from_small(browser)
        heard = wait_for(browser, lambda b: (meeting / "minutes.md").exists(), 15)
        header = json.loads((meeting / "transcript.jsonl").read_text().splitlines()[0]) if (meeting / "transcript.jsonl").exists() else {}
        check("the transcript made at the stop heard it", header.get("context") == said, header)
        asked = (WORK / "summary-context").read_text() if (WORK / "summary-context").exists() else None
        check("the minutes' request carried it", heard and asked is not None and json.loads(asked) == said, asked)

    # The meeting's page: the context shown, changed, made again with it — from its audio, which the
    # stand-in capture does not write: a track of its own, numbers only.
    (meeting / "local.wav").write_bytes(b"\0" * 1000)
    window = Window(meeting.parent, "fr_FR.UTF-8", settings)
    with window as browser:
        browser.execute_script(f"location.hash = '#/meeting/{meeting.name}'")
        shown = wait_for(browser, lambda b: b.find_elements(By.ID, "context-text"), 10)
        area = browser.find_element(By.ID, "context-text")
        label = browser.execute_script("return document.querySelector('label[for=context-text]')?.textContent")
        check("a meeting's page shows its context, labelled", shown and area.get_attribute("value") == said
              and label == "De quoi parle la réunion ? Qui y participe ?", (area.get_attribute("value"), label))
        check("made with it, nothing is offered to make again", not browser.find_element(By.ID, "context-changed").is_displayed())
        more = " Siobhán O’Neill (juridique)."
        area.click()
        keys(browser, Keys.END, more)
        changed = wait_for(browser, lambda b: b.find_element(By.ID, "context-changed").is_displayed(), 3)
        check("changed after its transcript and minutes, Refaire avec ce contexte is offered, said why",
              changed and (meeting / "context.txt").read_text() == said + more
              and browser.find_element(By.CSS_SELECTOR, "#context-changed .says").text == "La transcription et le compte rendu ont été faits avec un autre contexte."
              and browser.find_element(By.ID, "context-redo").text == "Refaire avec ce contexte", (meeting / "context.txt").read_text())
        seen(browser, "d59-4-page-changed-light")
        # Made again, slowly: the field closed meanwhile, and said.
        (WORK / "slow-hearing").touch()
        try:
            browser.find_element(By.ID, "context-redo").send_keys(Keys.ENTER)
            making = wait_for(browser, lambda b: b.find_element(By.ID, "context-text").get_attribute("readonly") is not None, 5)
            sentence = browser.find_element(By.ID, "context-said").text
            check("while it is made again, the field is closed, and says so",
                  making and sentence == "Le contexte ne change pas pendant que la transcription ou le compte rendu se fait."
                  and not browser.find_element(By.ID, "context-changed").is_displayed(), sentence)
            seen(browser, "d59-5-page-closed-light")
            browser.find_element(By.ID, "context-text").send_keys(" X")
            check("closed, nothing typed is taken", (meeting / "context.txt").read_text() == said + more)
            # The shell itself refuses, not only the page.
            refused = browser.execute_async_script(
                "const done = arguments[arguments.length - 1], name = arguments[0];"
                "Promise.all([window.__TAURI__.core.invoke('context_saved', {name, text: 'X'}).then(() => 'saved', () => 'refused'),"
                "window.__TAURI__.core.invoke('context_redo', {name}).then(() => 'redone', () => 'refused')]).then(done)",
                meeting.name,
            )
            check("and the shell refuses a context written or made again meanwhile",
                  refused == ["refused", "refused"] and (meeting / "context.txt").read_text() == said + more, refused)
        finally:
            (WORK / "slow-hearing").unlink()
        remade = wait_for(browser, lambda b: json.loads((meeting / "minutes.md").read_text().split("context: ", 1)[1].split("\n", 1)[0]) == said + more, 20)
        header = json.loads((meeting / "transcript.jsonl").read_text().splitlines()[0])
        check("made again: the transcript, then the minutes, with the context as it is now",
              remade and header.get("context") == said + more and json.loads((WORK / "summary-context").read_text()) == said + more, header)
        check("the transcript set aside is not left beside the new one", not list(meeting.glob("transcript-before-*")), sorted(p.name for p in meeting.iterdir()))
        reopened = wait_for(browser, lambda b: b.find_element(By.ID, "context-text").get_attribute("readonly") is None
                            and not b.find_element(By.ID, "context-changed").is_displayed(), 10)
        check("made, the field opens again, nothing left to make again", reopened)
        # Made again and failing: the transcript and the minutes there put back, never one of another context.
        before = (meeting / "transcript.jsonl").read_text(), (meeting / "minutes.md").read_text()
        browser.find_element(By.ID, "context-text").send_keys(Keys.END, " Et Lena.")
        wait_for(browser, lambda b: b.find_element(By.ID, "context-changed").is_displayed(), 3)
        (WORK / "fail-hearing").touch()
        try:
            press(browser, "context-redo")
            failed = wait_for(browser, lambda b: b.find_elements(By.ID, "transcript-again"), 10)
        finally:
            (WORK / "fail-hearing").unlink()
        check("failing, the transcript and the minutes made with the earlier context are put back",
              failed and ((meeting / "transcript.jsonl").read_text(), (meeting / "minutes.md").read_text()) == before
              and not list(meeting.glob("transcript-before-*")), sorted(p.name for p in meeting.iterdir()))
        check("and Refaire avec ce contexte is offered again", wait_for(browser, lambda b: b.find_element(By.ID, "context-changed").is_displayed(), 5))
        # Pasted past the most: cut to it, as the shell counts — in characters. As WebKit pastes: a
        # beforeinput that may refuse it, then the text in and an input.
        browser.execute_script(
            "const a = document.getElementById('context-text'), data = '🙂'.repeat(10) + 'é'.repeat(440);"
            "const asked = new InputEvent('beforeinput', {inputType: 'insertFromPaste', data, cancelable: true});"
            "if (a.dispatchEvent(asked)) { a.value = data; a.dispatchEvent(new InputEvent('input', {inputType: 'insertFromPaste', data})) }"
        )
        cut = wait_for(browser, lambda b: len((meeting / "context.txt").read_text()) == 400, 3)
        check("a paste past 400 characters is cut to 400, in the field and the file",
              cut and len(browser.find_element(By.ID, "context-text").get_attribute("value")) == 400
              and count(browser, "context-text") == "400 caractères au plus · 0 restant", count(browser, "context-text"))
        full_ = (meeting / "context.txt").read_text()
        browser.find_element(By.ID, "context-text").send_keys(Keys.END, "x")
        browser.find_element(By.ID, "context-text").send_keys(Keys.CONTROL, Keys.HOME)
        browser.find_element(By.ID, "context-text").send_keys("x")
        time.sleep(0.5)
        check("at 400, a character typed is not taken, at the end or at the start — nothing of the rest pushed out",
              (meeting / "context.txt").read_text() == full_ == browser.find_element(By.ID, "context-text").get_attribute("value")
              and len(full_) == 400 and full_.startswith("🙂"), len(full_))

    # A transcript left unfinished: the field closed, said so.
    unfinished = WORK / "context-unfinished"
    session(unfinished / "half", now - dt.timedelta(hours=2), 3, {"local.wav": 1000})
    (unfinished / "half" / "transcript.jsonl").write_text(
        '{"type":"header","contract":"transcript/v1","engine":"whisper.cpp","model":"m","language":null,"context":null}\n'
        '{"type":"segment","speaker":"me","track":"local","start":0.0,"end":1.0,"text":"Bonjour."}\n'
    )
    with Window(unfinished, "fr_FR.UTF-8", settings) as browser:
        browser.execute_script("location.hash = '#/meeting/half'")
        closed = wait_for(browser, lambda b: b.find_elements(By.ID, "context-text") and b.find_element(By.ID, "context-text").get_attribute("readonly") is not None, 10)
        # Read as written: WebDriver's text makes a no-break space a plain one.
        sentence = browser.execute_script("return document.getElementById('context-said').textContent") if closed else ""
        check("a transcript left unfinished closes the field, said with its way out",
              sentence == "La transcription n’est pas terminée : terminez-la (Réessayer) avant de changer le contexte.", sentence)

    # In English, in the dark theme: the small window and a meeting's page.
    window, meeting = recording("en", "en_GB.UTF-8", dark=True)
    with window as browser:
        wait_for(browser, lambda b: focused(b) == "small-record", 10)
        keys(browser, Keys.ENTER)
        wait_for(browser, lambda b: b.find_elements(By.ID, "small-stop") and b.find_element(By.ID, "small-stop").is_displayed(), 10)
        toggle = browser.find_element(By.ID, "small-context-toggle")
        check("in English, the context is offered", toggle.text.startswith("Add the context"), toggle.text)
        time.sleep(0.4)
        seen(browser, "d59-1-small-closed-dark")
        to_field(browser)
        keys(browser, Keys.ENTER)
        wait_for(browser, lambda b: focused(b) == "small-context", 2)
        english = "Q4 budget with Inès Durand and Jiří Novák."
        keys(browser, english)
        counted = wait_for(browser, lambda b: count(b, "small-context") == f"400 characters at most · {400 - len(english)} left", 2)
        kept = wait_for(browser, lambda b: (meeting / "context.txt").exists() and (meeting / "context.txt").read_text() == english, 3)
        time.sleep(0.3)
        check("in English, counted, kept, and holding in its window", counted and kept and fits(browser)[0] <= fits(browser)[1],
              (count(browser, "small-context"), fits(browser)))
        seen(browser, "d59-2-small-open-dark")
        keys(browser, " 王小明")
        wait_for(browser, lambda b: "In this script" in count(b, "small-context"), 2)
        time.sleep(0.3)
        check("a Chinese name is said, in English, the window whole", "In this script" in count(browser, "small-context")
              and fits(browser)[0] <= fits(browser)[1], (count(browser, "small-context"), fits(browser)))
        seen(browser, "d59-3-small-script-dark")
        stop_from_small(browser)
        wait_for(browser, lambda b: (meeting / "minutes.md").exists(), 15)
    (meeting / "local.wav").write_bytes(b"\0" * 1000)
    window = Window(meeting.parent, "en_GB.UTF-8", settings)
    window.env.update(GTK_THEME="Adwaita:dark")
    with window as browser:
        browser.execute_script(f"location.hash = '#/meeting/{meeting.name}'")
        wait_for(browser, lambda b: b.find_elements(By.ID, "context-text"), 10)
        browser.find_element(By.ID, "context-text").send_keys(Keys.END, " And Lena.")
        changed = wait_for(browser, lambda b: b.find_element(By.ID, "context-changed").is_displayed(), 3)
        check("in English, a meeting's page offers Redo with this context", changed
              and browser.find_element(By.ID, "context-redo").text == "Redo with this context", browser.find_element(By.ID, "context-redo").text)
        seen(browser, "d59-4-page-changed-dark")


def context_gaps(now: dt.datetime) -> None:
    """AH. What D59's criteria said and O did not run: the full window's page of the meeting being
    recorded, a Cyrillic name and an emoji typed, nothing of it leaving in private mode, « Refaire
    avec ce contexte » by the engine the settings name, the minutes set aside put back when they
    fail, nothing overflowing sideways at 380 × 400."""
    print("AH. a meeting's context: on the page being recorded, kept here in private mode, made again by the settings' engine (D59)")
    keys = lambda browser, *sent: ActionChains(browser).send_keys(*sent).perform()
    model = WORK / "context-gaps-model.bin"
    model.write_bytes(b"\0" * 16)
    here = f'after = "minutes"\nreminder = false\nmodel = "{model}"\nminutes_provider = "http://127.0.0.1:9/v1"\nminutes_model = "mistral"\n'
    hearing, writing = WORK / "hearing-args", WORK / "summary-args"
    meetings = WORK / "context-gaps"
    meetings.mkdir()
    meeting = meetings / "2026-10-18_12-00"
    typed = "Иван Петров и Ольга 🙂"
    hearing.unlink(missing_ok=True)
    writing.unlink(missing_ok=True)
    with Window(meetings, "fr_FR.UTF-8", here) as browser:
        live = lambda b: "live" in (b.find_element(By.ID, "record").get_attribute("class") or "")
        browser.find_element(By.ID, "record").click()
        wait_for(browser, lambda b: (meeting / "session.json").exists() and live(b), 10)
        browser.execute_script(f"location.hash = '#/meeting/{meeting.name}'")
        open_ = wait_for(browser, lambda b: b.find_elements(By.ID, "context-text")
                         and b.find_element(By.ID, "context-text").get_attribute("readonly") is None, 10)
        browser.find_element(By.ID, "context-text").click()
        keys(browser, typed)
        kept = wait_for(browser, lambda b: (meeting / "context.txt").exists() and (meeting / "context.txt").read_text() == typed, 3)
        check("the full window's page of the meeting being recorded offers its context: a Cyrillic name and an emoji kept whole",
              open_ and kept, (meeting / "context.txt").read_text() if (meeting / "context.txt").exists() else None)
        # The button says it at the click now (D68); a press within half a second is the same press.
        time.sleep(0.6)
        browser.find_element(By.ID, "record").click()
        stopped = wait_for(browser, lambda b: "stopped" in (meeting / "session.json").read_text() and not live(b), 10)
        made = wait_for(browser, lambda b: (meeting / "minutes.md").exists() and writing.exists(), 20)
        header = json.loads((meeting / "transcript.jsonl").read_text().splitlines()[0]) if (meeting / "transcript.jsonl").exists() else {}
        check("in private mode the context goes to Whisper and Ollama on this computer alone",
              made and header.get("context") == typed and "--provider" not in hearing.read_text()
              and "--provider http://127.0.0.1:9/v1" in writing.read_text(),
              (stopped, sorted(p.name for p in meeting.iterdir()), hearing.read_text() if hearing.exists() else "", writing.read_text() if writing.exists() else ""))
        if not made:
            return
    # A service the settings name hears from now on: « Refaire avec ce contexte » by it, the minutes failing.
    (meeting / "local.wav").write_bytes(b"\0" * 1000)
    service = here + 'engine = "service"\ntranscription_provider = "http://127.0.0.1:9/v1"\ntranscription_model = "hears"\n'
    with Window(meetings, "fr_FR.UTF-8", service) as browser:
        browser.execute_script(f"location.hash = '#/meeting/{meeting.name}'")
        wait_for(browser, lambda b: b.find_elements(By.ID, "context-text"), 10)
        minutes = (meeting / "minutes.md").read_text()
        browser.find_element(By.ID, "context-text").send_keys(Keys.END, " Et Lena.")
        wait_for(browser, lambda b: b.find_element(By.ID, "context-changed").is_displayed(), 3)
        hearing.unlink(missing_ok=True)
        writing.unlink(missing_ok=True)
        (WORK / "fail-minutes").touch()
        try:
            press(browser, "context-redo")
            def context_heard() -> object:
                """The context the transcript's header names — none while it is set aside and made again."""
                try:
                    return json.loads((meeting / "transcript.jsonl").read_text().splitlines()[0]).get("context")
                except (FileNotFoundError, IndexError, ValueError):
                    return None

            heard = wait_for(browser, lambda b: hearing.exists() and context_heard() == typed + " Et Lena.", 15)
            # Asked of the step, which fails: then nothing is being written, and nothing left set aside.
            failed = wait_for(browser, lambda b: writing.exists() and b.execute_script(
                "return window.__TAURI__.core.invoke('makings').then(m => m.minutes === null && m.minutes_waiting.length === 0)") and not list(meeting.glob("minutes-before-*")), 15)
        finally:
            (WORK / "fail-minutes").unlink()
        check("« Refaire avec ce contexte » hears by the engine the settings name: the service",
              heard and "--provider http://127.0.0.1:9/v1" in hearing.read_text() and "--model hears" in hearing.read_text(), hearing.read_text() if hearing.exists() else "")
        check("its minutes failing, the ones set aside are put back", failed and (meeting / "minutes.md").read_text() == minutes,
              sorted(p.name for p in meeting.iterdir()))
    # The small window at 380 × 400, the field open and full of a long name: nothing sideways.
    small = WORK / "context-gaps-small"
    small.mkdir()
    with Window(small, "fr_FR.UTF-8", here, small=True) as browser:
        wait_for(browser, lambda b: b.find_elements(By.ID, "small-record"), 10)
        browser.find_element(By.ID, "small-record").click()
        wait_for(browser, lambda b: b.find_elements(By.ID, "small-stop") and b.find_element(By.ID, "small-stop").is_displayed(), 10)
        browser.find_element(By.ID, "small-context-toggle").click()
        wait_for(browser, lambda b: b.find_element(By.ID, "small-context").is_displayed(), 2)
        browser.find_element(By.ID, "small-context").send_keys("Константинопольская-Переяславская " * 3)
        time.sleep(0.5)
        wide = browser.execute_script("const e = document.scrollingElement; return [e.scrollWidth, e.clientWidth, innerWidth, innerHeight]")
        check("at 380 × 400, the field open and a long name typed, nothing overflows sideways", wide[0] <= wide[1] and wide[2:] == [380, 400], wide)
        stop_from_small(browser)


def confidential_kept(now: dt.datetime) -> None:
    """AA. A meeting said confidential (D60): with a service named in the settings to hear and to
    write, nothing is asked of it — a stand-in service counts its requests —; the meeting is made
    here, by Whisper and Ollama; turned on while a service makes it, after it wrote, or while its
    agreement is asked, made again here and what had left said; turned off, the settings choose
    again."""
    print("AA. a meeting said confidential: nothing sent to the services the settings name, made here; turned off, the settings choose again (D60)")
    counted = {"hear": 0, "write": 0}
    ollama = {"held": ["mistral:7b", "qwen3:8b"], "silent": False}

    # The services the settings name, stood in: each request the steps send them is counted.
    class Service(http.server.BaseHTTPRequestHandler):
        protocol_version = "HTTP/1.0"

        def do_POST(self):
            counted[self.path.strip("/")] += 1
            self.rfile.read(int(self.headers.get("content-length") or 0))
            self.send_response(200)
            self.send_header("content-length", "0")
            self.end_headers()

        def log_message(self, *_):
            pass

    # Ollama on this computer: its models; silent when the scenario says so; a pull that is whole.
    class Ollama(http.server.BaseHTTPRequestHandler):
        protocol_version = "HTTP/1.0"

        def do_GET(self):
            body = json.dumps({"models": [{"name": name} for name in ollama["held"]]}).encode()
            self.send_response(500 if ollama["silent"] else 200)
            self.send_header("content-length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def do_POST(self):
            self.rfile.read(int(self.headers.get("content-length") or 0))
            ollama["held"].append("qwen3:8b")
            self.send_response(200)
            self.send_header("content-type", "application/x-ndjson")
            self.end_headers()
            self.wfile.write(b'{"status":"success"}\n')

        def log_message(self, *_):
            pass

    servers = [http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler) for handler in (Service, Ollama)]
    for server in servers:
        threading.Thread(target=server.serve_forever, daemon=True).start()
    standin, base = (f"http://127.0.0.1:{server.server_port}" for server in servers)
    model = WORK / "confidential-model.bin"
    model.write_bytes(b"\0" * 16)
    named = (f'after = "minutes"\nreminder = false\nmodel = "{model}"\n'
             'engine = "service"\ntranscription_provider = "https://hears.example/v1"\ntranscription_model = "whisper-1"\n'
             'transcription_agreed = "hears.example"\nminutes_provider = "https://writes.example/v1"\nminutes_model = "gpt-4.1-mini"\n')
    agreed = named + 'minutes_agreed = "writes.example"\n'
    fits = lambda b: b.execute_script("const e = document.scrollingElement; return [e.scrollHeight, e.clientHeight];")
    front = lambda folder, key: next((line.split(": ", 1)[1] for line in (folder / "minutes.md").read_text().splitlines()
                                      if line.startswith(f"{key}: ")), None) if (folder / "minutes.md").exists() else None
    engine = lambda folder: json.loads((folder / "transcript.jsonl").read_text().splitlines()[0]).get("engine") \
        if (folder / "transcript.jsonl").exists() and (folder / "transcript.jsonl").read_text().strip() else None
    names = lambda folder: sorted(p.name for p in folder.iterdir())
    switch = lambda b, id_: b.find_element(By.ID, id_)
    focused = lambda b: b.execute_script("return document.activeElement.id")
    left = "return document.getElementById('confidential-left')?.hidden ? '' : (document.getElementById('confidential-left')?.innerText || '')"
    css = (Path(__file__).parent.parent / "ui" / "window.css").read_text()
    light = css[css.index(":root {") + len(":root {"):css.index("}", css.index(":root {"))].replace("\n", " ").strip()

    def seen(browser, name: str) -> None:
        """A screenshot in the theme its name says: the light one put on the page over a dark desktop."""
        forced = "light" in name and browser.execute_script("return matchMedia('(prefers-color-scheme: dark)').matches")
        if forced:
            browser.execute_script("document.documentElement.style.cssText = arguments[0] + '; color-scheme: light'", light)
            time.sleep(0.3)
        shot(browser, name)
        if forced:
            browser.execute_script("document.documentElement.style.cssText = ''")

    def window(folder: Path, language: str, more: str, small: bool = False, dark: bool = False) -> "Window":
        made = Window(folder, language, more, ollama=base, small=small)
        made.env.update(STANDIN=standin)
        if dark:
            made.env.update(GTK_THEME="Adwaita:dark")
        return made

    # While recording, in the small window: said confidential by the keyboard, the file written.
    meetings = WORK / "confidential"
    meetings.mkdir()
    meeting = meetings / "2026-10-18_12-00"
    with window(meetings, "fr_FR.UTF-8", agreed, small=True) as browser:
        # Started by the keyboard, once the page has the focus.
        wait_for(browser, lambda b: focused(b) == "small-record", 10)
        ActionChains(browser).send_keys(Keys.ENTER).perform()
        started = wait_for(browser, lambda b: b.find_elements(By.ID, "small-stop") and b.find_element(By.ID, "small-stop").is_displayed(), 10)
        row = browser.find_element(By.CSS_SELECTOR, "#small .confidential")
        check("recording, the small window offers « Réunion confidentielle — rien ne quitte cet ordinateur », off",
              started and "Réunion confidentielle" in row.text and "Rien ne quitte cet ordinateur" in row.text
              and switch(browser, "small-confidential").get_attribute("role") == "switch"
              and not switch(browser, "small-confidential").is_selected(), row.text)
        check("the lock before its words is drawn, never read aloud",
              browser.execute_script("return document.querySelector('#small .confidential svg.lock')?.getAttribute('aria-hidden')") == "true")
        time.sleep(0.4)
        seen(browser, "d60-1-small-off-light")
        wait_for(browser, lambda b: (meeting / "session.json").exists(), 3)
        switch(browser, "small-confidential").send_keys(Keys.SPACE)
        marked = wait_for(browser, lambda b: (meeting / "confidential").exists(), 3)
        check("turned on by the keyboard, the meeting's file confidential is written, empty, nothing beside it",
              marked and (meeting / "confidential").read_bytes() == b"" and names(meeting) == ["confidential", "session.json"], names(meeting))
        time.sleep(0.3)
        check("on, the window holds whole — 380 × 400", fits(browser)[0] <= fits(browser)[1]
              and browser.execute_script("return [innerWidth, innerHeight]") == [380, 400], fits(browser))
        seen(browser, "d60-2-small-on-light")
        press(browser, "small-context-toggle")
        time.sleep(0.4)
        check("its context opened too, the window still holds whole", fits(browser)[0] <= fits(browser)[1], fits(browser))
        seen(browser, "d60-3-small-on-context-light")
        stop_from_small(browser)
        made = wait_for(browser, lambda b: front(meeting, "provider") == "local", 20)
        check("stopped: the transcript made by Whisper with the settings' model, the minutes by Ollama's recommended model",
              made and engine(meeting) == "whisper.cpp" and front(meeting, "model") == "qwen3:8b"
              and f"--provider {base}/v1" in (WORK / "summary-args").read_text(), (engine(meeting), front(meeting, "model")))
        check("the services the settings name count no request", counted == {"hear": 0, "write": 0}, counted)

    # In the full window: the list's lock; turned off, the settings choose again; on, made again here.
    with window(meetings, "fr_FR.UTF-8", agreed) as browser:
        chips = browser.execute_script("return [...document.querySelectorAll('.session .chip.locked')].map(c => [c.textContent, !!c.querySelector('svg.lock')])")
        check("the list shows the meeting with a lock and its word", chips == [["Confidentielle", True]], chips)
        seen(browser, "d60-4-list-light")
        browser.execute_script(f"location.hash = '#/meeting/{meeting.name}'")
        # The last meeting, drawn beside the list already: its page may be drawn again under the driver.
        shown = wait_for(browser, lambda b: b.find_elements(By.ID, "confidential-switch"), 10)
        check("its page says it, the switch on, the lock among what it holds, nothing said to have left",
              shown and read(browser, lambda b: switch(b, "confidential-switch").is_selected()
                             and b.find_element(By.CSS_SELECTOR, "#meeting .meeting-title .chip.locked").text == "Confidentielle")
              and browser.execute_script(left) == "")
        press(browser, "confidential-switch")
        off = wait_for(browser, lambda b: not (meeting / "confidential").exists(), 3)
        check("turned off, the file is removed", off and "confidential" not in names(meeting), names(meeting))
        (meeting / "minutes.md").unlink()
        browser.execute_script("location.hash = '#/'")
        browser.execute_script(f"location.hash = '#/meeting/{meeting.name}'")
        wait_for(browser, lambda b: b.find_elements(By.ID, "make-minutes"), 10)
        press(browser, "make-minutes")
        sent = wait_for(browser, lambda b: front(meeting, "provider") == "writes.example", 10)
        check("turned off, the settings choose again: the service named writes them", sent and counted["write"] == 1, counted)
        press(browser, "confidential-switch")
        again = wait_for(browser, lambda b: front(meeting, "provider") == "local", 10)
        said = wait_for(browser, lambda b: b.execute_script(left) == "Déjà parti avant\u00a0: la transcription vers writes.example.", 5)
        check("turned on after a service wrote them: written again here, nothing more sent",
              again and front(meeting, "model") == "qwen3:8b" and counted["write"] == 1, (front(meeting, "provider"), counted))
        check("and the page says what had left, to which host", said, browser.execute_script(left))
        seen(browser, "d60-5-page-left-light")

    # Turned on while a service hears it: ended, heard here, the minutes after it here too.
    heard = WORK / "confidential-heard"
    session(heard / "live", now - dt.timedelta(hours=1), 3, {"local.wav": 1000})
    (WORK / "slow-hearing").touch()
    try:
        with window(heard, "fr_FR.UTF-8", agreed) as browser:
            browser.execute_script("location.hash = '#/meeting/live'")
            wait_for(browser, lambda b: b.find_elements(By.ID, "make-transcript"), 10)
            press(browser, "make-transcript")
            asked = wait_for(browser, lambda b: counted["hear"] == 1, 10)
            press(browser, "confidential-switch")
            remade = wait_for(browser, lambda b: front(heard / "live", "provider") == "local", 25)
            check("turned on while a service hears it: ended, heard here by Whisper, the minutes written here",
                  asked and remade and engine(heard / "live") == "whisper.cpp" and counted == {"hear": 1, "write": 1}, (engine(heard / "live"), counted))
            said = wait_for(browser, lambda b: b.execute_script(left) == "Déjà parti avant\u00a0: le son vers hears.example.", 5)
            check("the page says the sound had left, to which host", said, browser.execute_script(left))
            check("its end is no failure said", not browser.find_elements(By.ID, "transcript-again"))
    finally:
        (WORK / "slow-hearing").unlink()

    # Its agreement asked: no longer, made here.
    asking = WORK / "confidential-asked"
    session(asking / "asked", now - dt.timedelta(hours=2), 3, {"local.wav": 1000, "transcript.jsonl": 0})
    (asking / "asked" / "transcript.jsonl").write_text(
        '{"type":"header","contract":"transcript/v1","engine":"whisper.cpp","model":"m","language":null,"context":null}\n'
        '{"type":"segment","speaker":"me","track":"local","start":0.0,"end":1.0,"text":"Bonjour."}\n{"type":"complete","duration":1.0}\n'
    )
    with window(asking, "fr_FR.UTF-8", named) as browser:
        browser.execute_script("location.hash = '#/meeting/asked'")
        wait_for(browser, lambda b: b.find_elements(By.ID, "make-minutes"), 10)
        press(browser, "make-minutes")
        agreeing = wait_for(browser, lambda b: b.execute_script("return location.hash") == "#/agree/asked", 5)
        browser.execute_script("location.hash = '#/meeting/asked'")
        waits = wait_for(browser, lambda b: b.find_elements(By.ID, "minutes-agree"), 10)
        check("a service not agreed to: the agreement is asked, nothing sent", agreeing and waits and counted["write"] == 1, counted)
        press(browser, "confidential-switch")
        here = wait_for(browser, lambda b: front(asking / "asked", "provider") == "local", 10)
        check("turned on while it is asked: no longer asked, written here, nothing sent",
              here and not browser.find_elements(By.ID, "minutes-agree") and counted["write"] == 1, counted)
        browser.execute_script("location.hash = '#/agree/asked'")
        back = wait_for(browser, lambda b: b.execute_script("return location.hash") == "#/", 5)
        check("its agreement page is no longer shown", back, browser.execute_script("return location.hash"))

    # Ollama silent, then holding no model: the minutes wait, said, its model offered.
    waiting = WORK / "confidential-waiting"
    session(waiting / "wait", now - dt.timedelta(hours=3), 3, {"local.wav": 1000, "transcript.jsonl": 0})
    (waiting / "wait" / "transcript.jsonl").write_text((asking / "asked" / "transcript.jsonl").read_text())
    (waiting / "wait" / "confidential").touch()
    ollama.update(held=[], silent=True)
    state = "return document.getElementById('minutes-state')?.innerText || ''"
    with window(waiting, "fr_FR.UTF-8", agreed) as browser:
        browser.execute_script("location.hash = '#/meeting/wait'")
        wait_for(browser, lambda b: b.find_elements(By.ID, "make-minutes"), 10)
        press(browser, "make-minutes")
        silent = wait_for(browser, lambda b: "Ollama ne répond pas sur cet ordinateur" in b.execute_script(state), 10)
        check("Ollama silent: the minutes wait, said where to get it, nothing asked nor sent",
              silent and browser.find_elements(By.ID, "minutes-again") and counted["write"] == 1
              and not (waiting / "wait" / "minutes.md").exists(), browser.execute_script(state))
        ollama["silent"] = False
        press(browser, "minutes-again")
        empty = wait_for(browser, lambda b: "Ollama n’a aucun modèle sur cet ordinateur" in b.execute_script(state), 10)
        offered = browser.find_elements(By.ID, "ollama-fetch")
        check("holding no model: said, its recommended model offered", empty and offered and offered[0].text == "Télécharger qwen3:8b",
              browser.execute_script(state))
        seen(browser, "d60-6-ollama-empty-light")
        press(browser, "ollama-fetch")
        written = wait_for(browser, lambda b: front(waiting / "wait", "model") == "qwen3:8b", 15)
        check("fetched, the minutes waiting are written by it, here", written and front(waiting / "wait", "provider") == "local"
              and counted["write"] == 1, counted)

    # In English, in the dark theme.
    english = WORK / "confidential-en"
    english.mkdir()
    with window(english, "en_GB.UTF-8", agreed, small=True, dark=True) as browser:
        wait_for(browser, lambda b: focused(b) == "small-record", 10)
        ActionChains(browser).send_keys(Keys.ENTER).perform()
        wait_for(browser, lambda b: b.find_elements(By.ID, "small-stop") and b.find_element(By.ID, "small-stop").is_displayed(), 10)
        wait_for(browser, lambda b: (english / "2026-10-18_12-00" / "session.json").exists(), 3)
        press(browser, "small-confidential")
        marked = wait_for(browser, lambda b: (english / "2026-10-18_12-00" / "confidential").exists(), 3)
        row = browser.find_element(By.CSS_SELECTOR, "#small .confidential").text
        time.sleep(0.3)
        check("in English, « Confidential meeting — nothing leaves this computer », on, the window whole",
              marked and "Confidential meeting" in row and "Nothing leaves this computer" in row and fits(browser)[0] <= fits(browser)[1], row)
        seen(browser, "d60-2-small-on-dark")
        stop_from_small(browser)
        wait_for(browser, lambda b: front(english / "2026-10-18_12-00", "provider") == "local", 20)
    with window(english, "en_GB.UTF-8", agreed, dark=True) as browser:
        chips = browser.execute_script("return [...document.querySelectorAll('.session .chip.locked')].map(c => c.textContent)")
        seen(browser, "d60-4-list-dark")
        browser.execute_script("location.hash = '#/meeting/2026-10-18_12-00'")
        wait_for(browser, lambda b: b.find_elements(By.ID, "confidential-switch"), 10)
        check("in English, the list and the page say it", chips == ["Confidential"] and read(browser, lambda b: switch(b, "confidential-switch").is_selected())
              and counted == {"hear": 1, "write": 1}, (chips, counted))
        seen(browser, "d60-5-page-dark")

    # Turned on while a service writes the minutes: that making ended, written again here.
    writing = WORK / "confidential-writing"
    session(writing / "sent", now - dt.timedelta(hours=3), 3, {"local.wav": 1000, "transcript.jsonl": 0})
    (writing / "sent" / "transcript.jsonl").write_text((asking / "asked" / "transcript.jsonl").read_text())
    (WORK / "slow-minutes").touch()
    try:
        with window(writing, "fr_FR.UTF-8", agreed) as browser:
            browser.execute_script("location.hash = '#/meeting/sent'")
            wait_for(browser, lambda b: b.find_elements(By.ID, "make-minutes"), 10)
            press(browser, "make-minutes")
            asked = wait_for(browser, lambda b: counted["write"] == 2, 10)
            press(browser, "confidential-switch")
            here = wait_for(browser, lambda b: front(writing / "sent", "provider") == "local", 25)
            check("turned on while a service writes the minutes: ended, written here, nothing more sent",
                  asked and here and counted == {"hear": 1, "write": 2}, (front(writing / "sent", "provider"), counted))
            said = wait_for(browser, lambda b: b.execute_script(left) == "Déjà parti avant\u00a0: la transcription vers writes.example.", 5)
            check("the page says the transcript had left, to which host", said, browser.execute_script(left))
            check("its end is no failure said", not browser.find_elements(By.ID, "minutes-again"))
    finally:
        (WORK / "slow-minutes").unlink()

    # A service named to hear, the default Whisper model not there: said confidential, the meeting
    # waits for the model, offered as Settings offer it, and nothing is sent.
    unmodelled = WORK / "confidential-no-model"
    session(unmodelled / "kept", now - dt.timedelta(hours=4), 3, {"local.wav": 1000})
    (unmodelled / "kept" / "confidential").touch()
    bare = window(unmodelled, "fr_FR.UTF-8", agreed.replace(f'model = "{model}"\n', ""))
    # Its own data folder: no default model fetched there by an earlier scenario.
    bare.env.update(XDG_DATA_HOME=str(WORK / "data-confidential"))
    with bare as browser:
        browser.execute_script("location.hash = '#/meeting/kept'")
        wait_for(browser, lambda b: b.find_elements(By.ID, "make-transcript"), 10)
        press(browser, "make-transcript")
        waits = "return document.getElementById('transcript-state')?.innerText || ''"
        offered = wait_for(browser, lambda b: "En attente du modèle Whisper" in b.execute_script(waits)
                           and b.find_elements(By.CSS_SELECTOR, "#transcript-state #model-fetch"), 10)
        check("the Whisper model missing: said and offered as Settings offer it, the service named asked nothing",
              offered and counted == {"hear": 1, "write": 2}, (browser.execute_script(waits), counted, names(unmodelled / "kept")))
    for server in servers:
        server.shutdown()


def nothing_silent(now: dt.datetime) -> None:
    """AP. What the user does and fails says so; a folder that cannot be read is no empty list (D67)."""
    print("AP. a command that fails says it, in the user's language; a folder unread is said and tried again (D67)")
    meetings = WORK / "silence"
    session(meetings / "2026-10-20_10-00", now - dt.timedelta(days=3), 20, {"local.wav": 1000})
    shown = lambda b, id_: b.find_element(By.ID, id_).is_displayed()
    text = lambda b, id_: b.find_element(By.ID, id_).get_attribute("textContent")
    # `core` itself is frozen, the object holding it not: a copy with the command stood in.
    refuse = "const real = window.__TAURI__.core, refused = arguments[0]; window.__TAURI__.core = { ...real, invoke: (asked, ...rest) => asked === refused ? Promise.reject('REFUS') : real.invoke(asked, ...rest) }; window.restore = () => { window.__TAURI__.core = real; };"
    with Window(meetings, "fr_FR.UTF-8", "reminder = false\n") as browser:
        browser.set_window_size(1100, 800)
        wait_for(browser, lambda b: shown(b, "meeting"), 10)
        # The real commands on a meeting gone: what the user reads is the shell's own sentence.
        for ask in ("minutes_again", "transcript_again"):
            said = browser.execute_async_script(
                "const done = arguments[arguments.length - 1], said = document.createElement('p'), again = document.createElement('button');"
                "said.dataset.name = 'gone'; askAgain({ ask: arguments[0] }, said, again).then(() => done(said.textContent));", ask)
            check(f"{ask} on a meeting gone says so, in French, never the command's raw text",
                  said.startswith("Impossible de recommencer") and "n’est plus dans le dossier" in said, said)
        browser.execute_script(refuse, "agreement_shown")
        browser.execute_script("location.hash = '#/agree/2026-10-20_10-00'")
        refused = wait_for(browser, lambda b: shown(b, "pane-said"), 5)
        check("an agreement refused: said in the pane, back at the list", refused
              and text(browser, "pane-said") == "Cette réunion ne peut pas être envoyée\u00a0: REFUS", text(browser, "pane-said"))
        browser.execute_script("window.restore(); location.hash = '#/settings'")
        check("another address: the sentence goes", wait_for(browser, lambda b: not shown(b, "pane-said"), 5))
        browser.execute_script("location.hash = '#/'")
        browser.execute_script(refuse, "settings_saved")
        press(browser, "fold")
        folded = wait_for(browser, lambda b: shown(b, "pane-said"), 5)
        check("the sidebar's fold not kept: said in the pane, the sidebar still as asked",
              folded and text(browser, "pane-said") == "La barre latérale reviendra comme avant au prochain démarrage\u00a0: REFUS"
              and not shown(browser, "sidebar"), text(browser, "pane-said"))
        shot(browser, "d67-pane-said")
        browser.execute_script("window.restore()")
        press(browser, "unfold")
        check("kept at last: the sentence goes", wait_for(browser, lambda b: not shown(b, "pane-said"), 5))
        browser.execute_script(refuse, "hearing_models")
        browser.execute_script("location.hash = '#/settings'")
        wait_for(browser, lambda b: b.find_elements(By.ID, "model"), 10)
        # Settings drawn before, drawn anew: the list taken once that drawing is done.
        time.sleep(1)
        Select(browser.find_element(By.ID, "model")).select_by_value("service:0")
        refused = wait_for(browser, lambda b: shown(b, "transcription-models-refused"), 5)
        check("a service's models refused: said, and the model can still be typed",
              refused and text(browser, "transcription-models-refused") == "Les modèles de ce service n’ont pas pu être lus, vous pouvez en saisir un\u00a0: REFUS"
              and shown(browser, "transcription-model"), text(browser, "transcription-models-refused"))
        browser.execute_script("window.restore()")
        browser.execute_script("location.hash = '#/'")
        # The folder refused, then given back: said, and read again on the click.
        meetings.chmod(0)
        try:
            browser.execute_script("window.dispatchEvent(new Event('focus'))")
            said = wait_for(browser, lambda b: shown(b, "empty-again"), 10)
            check("the meetings' folder unread: said, no recording offered as if it were empty, a way to try again",
                  said and text(browser, "none") == f"Les réunions de {meetings} ne peuvent pas être lues."
                  and text(browser, "none-do").startswith("Vérifiez que le disque")
                  and text(browser, "empty-again") == "Réessayer" and not shown(browser, "empty-record"), text(browser, "none"))
            browser.set_window_size(420, 400)
            browser.execute_script("document.getElementById('empty').scrollIntoView()")
            for theme in ("light", "dark"):
                shot_themed(browser, f"d67-unread-{theme}", theme)
            press(browser, "empty-again")
            time.sleep(0.3)
            check("tried again, still refused: still said", shown(browser, "empty-again"))
        finally:
            meetings.chmod(0o755)
        browser.set_window_size(1100, 800)
        browser.execute_script("document.getElementById('empty-again').click()")
        check("tried again, given back: the meetings are there", wait_for(browser, lambda b: b.find_elements(By.CSS_SELECTOR, "#groups .session") and not shown(b, "empty"), 10))
    meetings.chmod(0)
    try:
        with Window(meetings, "fr_FR.UTF-8", "reminder = false\n", small=True) as browser:
            said = wait_for(browser, lambda b: "ne peuvent pas être lues" in b.execute_script("return document.getElementById('small').textContent"), 10)
            check("the small window says the meetings cannot be read, not that there are none", said)
    finally:
        meetings.chmod(0o755)
    # A meeting's folder locked while it is recorded: the mark refused for real, the switch on says so.
    recorded = WORK / "silence-marked"
    recorded.mkdir()
    with Window(recorded, "fr_FR.UTF-8", 'after = "nothing"\nreminder = false\n', small=True) as browser:
        press(browser, "small-record")
        made = wait_for(browser, lambda b: list(recorded.glob("*/session.json")) and shown(b, "small-confidential"), 15)
        folder = next(recorded.glob("*/session.json")).parent if made else recorded
        folder.chmod(0o555)
        click = "document.getElementById('small-confidential').click()"
        top = "return document.getElementById('small-stop').getBoundingClientRect().top"
        try:
            before = browser.execute_script(top)
            browser.execute_script(click)
            said = wait_for(browser, lambda b: shown(b, "small-said"), 8)
            check("said, Arrêter stays where it is", browser.execute_script(top) == before)
            check("confidential refused in the small window: said, its lock off, never promising what is not so",
                  said and "n’a pas pu être marquée confidentielle" in text(browser, "small-said")
                  and "on" not in browser.execute_script("return document.getElementById('small-confidential').closest('.confidential').className").split(),
                  text(browser, "small-said"))
        finally:
            folder.chmod(0o755)
        check("given back, it is kept at the next try and the sentence goes",
              wait_for(browser, lambda b: not shown(b, "small-said") and (folder / "confidential").exists(), 8), sorted(p.name for p in folder.glob("*")))
        browser.execute_script(click)
        wait_for(browser, lambda b: not (folder / "confidential").exists(), 5)
        folder.chmod(0o555)
        try:
            browser.execute_script(click)
            wait_for(browser, lambda b: shown(b, "small-said"), 8)
        finally:
            folder.chmod(0o755)
        # A press within half a second of the last is taken for the same one.
        time.sleep(0.6)
        press(browser, "small-stop")
        check("stopped before the next try: tried once more, and kept",
              wait_for(browser, lambda b: shown(b, "small-record") and (folder / "confidential").exists() and not shown(b, "small-said"), 10))
        # The clock stood in: the next recording is the same folder, its mark taken away.
        (folder / "confidential").unlink()
        time.sleep(2)
        press(browser, "small-record")
        wait_for(browser, lambda b: b.find_element(By.ID, "small-stop").is_displayed(), 15)
        time.sleep(0.6)
        folder.chmod(0o555)
        try:
            browser.execute_script(click)
            wait_for(browser, lambda b: shown(b, "small-said"), 8)
            time.sleep(0.6)
            press(browser, "small-stop")
            at_rest = wait_for(browser, lambda b: shown(b, "small-record") and "n’est pas confidentielle" in text(b, "small-said"), 10)
            first = browser.execute_script("return document.getElementById('small').firstElementChild.id")
            check("stopped with the mark refused: said at rest as it is, first of all, and nothing marked",
                  at_rest and first == "small-said" and not (folder / "confidential").exists(), (text(browser, "small-said"), first))
        finally:
            folder.chmod(0o755)
        time.sleep(2)
        press(browser, "small-record")
        check("the next recording starts clean", wait_for(browser, lambda b: b.find_element(By.ID, "small-stop").is_displayed() and not shown(b, "small-said"), 15))
        time.sleep(0.6)
        press(browser, "small-stop")
    for language, raw in (("fr_FR.UTF-8", False), ("en_US.UTF-8", True)):
        with Window(meetings, language, "reminder = false\n" + ("developer = true\n" if raw else ""), ) as browser:
            told = browser.execute_async_script(
                "const done = arguments[arguments.length - 1]; window.__TAURI__.core.invoke('minutes_again', { name: 'gone' }).then(() => done(''), (why) => done(String(why)));")
            check(f"the raw text only in developer mode ({'on' if raw else 'off'})", ("gone is no session" in told) == raw, told)
    # What the page reads and cannot: said where the eye is, in the user's language; asked by no gesture
    # or not, never only a line in the log (D67).
    for language, not_opened, not_read, unread, gone in (
        ("fr_FR.UTF-8", "Cette réunion n’a pas pu être ouverte\u00a0: ", "Le contexte n’a pas pu être relu\u00a0: ", "Les réunions ne peuvent pas être lues", "n’est plus dans le dossier"),
        ("en_US.UTF-8", "This meeting could not be opened: ", "The context could not be read again: ", "The meetings cannot be read", "is no longer in the meetings"),
    ):
        with Window(meetings, language, "reminder = false\n") as browser:
            browser.set_window_size(1100, 800)
            wait_for(browser, lambda b: shown(b, "meeting") and b.find_elements(By.ID, "context"), 10)
            # A making stood in, then over, the context not read back: its text stays, the sentence comes.
            browser.execute_script(
                "const real = window.__TAURI__.core, name = arguments[0]; window.making = true;"
                "window.__TAURI__.core = { ...real, invoke: async (asked, ...rest) => {"
                " if (asked === 'context_shown') throw 'REFUS';"
                " const answer = await real.invoke(asked, ...rest);"
                " return asked === 'makings' && window.making ? { ...answer, minutes: { name, model: 'm', provider: 'p' } } : answer; } };"
                "window.restore = () => { window.__TAURI__.core = real; };", "2026-10-20_10-00")
            wait_for(browser, lambda b: b.execute_script("return document.getElementById('context').dataset.making === 'true'"), 8)
            browser.execute_script("window.making = false")
            said = wait_for(browser, lambda b: text(b, "context-said") == not_read + "REFUS" and shown(b, "context-said"), 8)
            check(f"a context not read back after a making: said ({language})", said, text(browser, "context-said"))
            browser.execute_script("window.restore()")
            browser.execute_script(refuse, "opened")
            browser.execute_script("location.hash = '#/meeting/2026-10-20_10-00'")
            said = wait_for(browser, lambda b: shown(b, "pane-said") and text(b, "pane-said") == not_opened + "REFUS", 5)
            check(f"a meeting that cannot be opened: said in the pane, back at the list ({language})",
                  said and browser.execute_script("return location.hash") == "#/" and not shown(browser, "meeting"), text(browser, "pane-said"))
            browser.execute_script("window.restore(); location.hash = '#/settings'")
            check("another address: the sentence goes", wait_for(browser, lambda b: not shown(b, "pane-said"), 5))
            # The real command, on a meeting gone: the shell's own sentence, not the stand-in's.
            browser.execute_script("location.hash = '#/meeting/gone'")
            said = wait_for(browser, lambda b: shown(b, "pane-said") and text(b, "pane-said").startswith(not_opened), 5)
            check(f"a meeting gone: said, with what the shell tells of it ({language})", said and gone in text(browser, "pane-said"), text(browser, "pane-said"))
            # Refused late, the user gone to another address meanwhile: that address stays, nothing said.
            browser.execute_script("location.hash = '#/settings'")
            wait_for(browser, lambda b: not shown(b, "pane-said") and b.find_elements(By.ID, "model"), 5)
            browser.execute_script("const real = window.__TAURI__.core; window.late = 0; window.__TAURI__.core = { ...real, invoke: (asked, ...rest) => asked === 'opened' ? new Promise((_, no) => setTimeout(() => { window.late += 1; no('REFUS'); }, 300)) : real.invoke(asked, ...rest) }; window.restore = () => { window.__TAURI__.core = real; }; location.hash = '#/meeting/2026-10-20_10-00'; setTimeout(() => { location.hash = '#/settings'; }, 50);")
            wait_for(browser, lambda b: b.execute_script("return window.late") >= 1, 5)
            time.sleep(0.2)
            check(f"refused late, another address since: it stays, nothing said ({language})",
                  browser.execute_script("return location.hash") == "#/settings" and not shown(browser, "pane-said"), browser.execute_script("return location.hash"))
            browser.execute_script("window.restore()")
        with Window(recorded, language, 'after = "nothing"\nreminder = false\n', small=True) as browser:
            wait_for(browser, lambda b: b.find_elements(By.ID, "small-read"), 8)
            browser.execute_script(refuse, "listed")
            said = wait_for(browser, lambda b: unread in b.execute_script("return document.querySelector('.small-rest').textContent") and not b.find_elements(By.ID, "small-read"), 6)
            check(f"the small window's list not read: said, nothing stale offered ({language})", said)
            browser.execute_script("window.restore()")
            check("read again: the last meeting is back", wait_for(browser, lambda b: b.find_elements(By.ID, "small-read") and unread not in b.execute_script("return document.querySelector('.small-rest').textContent"), 6))
            browser.execute_script(refuse, "context_shown")
            time.sleep(1.1)
            press(browser, "small-record")
            said = wait_for(browser, lambda b: shown(b, "small-said") and text(b, "small-said") == not_read + "REFUS", 10)
            check(f"a recording's context not read: said under the switch, Arrêter stays ({language})", said, text(browser, "small-said"))
            browser.execute_script("window.restore()")
            time.sleep(0.6)
            press(browser, "small-stop")
            wait_for(browser, lambda b: shown(b, "small-record"), 10)
    empty = WORK / "silence-none"
    empty.mkdir()
    with Window(empty, "fr_FR.UTF-8", "reminder = false\n") as browser:
        browser.set_window_size(420, 400)
        said = wait_for(browser, lambda b: shown(b, "none"), 10)
        check("no meeting: what to do first, never only a path", said and text(browser, "none-do").startswith("Démarrez un enregistrement") and shown(browser, "empty-record"))
        browser.execute_script("document.getElementById('empty').scrollIntoView()")
        for theme in ("light", "dark"):
            shot_themed(browser, f"d67-none-{theme}", theme)


def gestures_answer(now: dt.datetime) -> None:
    """AQ. Each gesture answers at once, its command stood in to take two seconds (D68)."""
    print("AQ. Démarrer and Arrêter answer before their command, a second click asks nothing; Refaire… closes; a setting says Enregistré (D68)")
    meetings = WORK / "answering"
    written = meetings / "2026-10-28_09-00"
    session(written, now - dt.timedelta(days=1), 10, {"local.wav": 1000})
    (written / "transcript.jsonl").write_text(
        '{"type":"header","contract":"transcript/v1","engine":"whisper.cpp","model":"m","language":null,"context":null}\n'
        '{"type":"segment","speaker":"me","track":"local","start":0.0,"end":1.0,"text":"Bonjour."}\n{"type":"complete","duration":1.0}\n')
    (written / "minutes.md").write_text(
        "---\ncontract: minutes/v1\nmade: 2026-10-28T10:00:00+01:00\nprovider: local\nmodel: qwen3:8b\ncontext: null\n---\n\n"
        "## 1.\n\nLe budget est validé.\n\n## 2.\n\n- Rien\n\n## 3.\n\n- Rien\n")
    slow = ("const real = window.__TAURI__.core; window.__asked = 0; window.__TAURI__.core = { ...real, invoke: (asked, ...rest) => asked === 'record'"
            " ? (window.__asked++, new Promise((done) => setTimeout(done, 2000)).then(() => real.invoke(asked, ...rest))) : real.invoke(asked, ...rest) };")
    shown = lambda b, id_: b.find_element(By.ID, id_).is_displayed()
    said = lambda b: b.find_element(By.ID, "record-said").get_attribute("textContent")
    running = lambda b: b.execute_async_script("const d = arguments[arguments.length - 1]; window.__TAURI__.core.invoke('recording_now').then((n) => d(n.recording))")
    with Window(meetings, "fr_FR.UTF-8", 'after = "nothing"\nreminder = false\n') as browser:
        browser.set_window_size(1100, 800)
        browser.execute_script(f"location.hash = '#/meeting/{written.name}'")
        wait_for(browser, lambda b: b.find_elements(By.ID, "redo-minutes"), 10)
        # Refaire…: Annuler, then Escape, each giving the focus back, nothing changed.
        before = (written / "minutes.md").read_text()
        for way in ("cancel", "escape"):
            press(browser, "redo-minutes")
            wait_for(browser, lambda b: b.find_elements(By.ID, "redo-panel"), 5)
            if way == "cancel":
                press(browser, "redo-panel-cancel")
            else:
                browser.switch_to.active_element.send_keys(Keys.ESCAPE)
            closed = wait_for(browser, lambda b: not b.find_elements(By.ID, "redo-panel"), 3)
            check(f"Refaire… closed by {way}: the focus back on it, nothing changed", closed
                  and browser.execute_script("return document.activeElement.id") == "redo-minutes" and (written / "minutes.md").read_text() == before,
                  (closed, browser.execute_script("return document.activeElement.outerHTML.slice(0, 80)")))
        press(browser, "redo-minutes")
        wait_for(browser, lambda b: b.find_elements(By.ID, "redo-panel"), 5)
        browser.execute_script("document.activeElement.blur()")
        ActionChains(browser).send_keys(Keys.ESCAPE).perform()
        check("Escape with the focus nowhere closes it too", wait_for(browser, lambda b: not b.find_elements(By.ID, "redo-panel"), 3))
        press(browser, "redo-transcript")
        wait_for(browser, lambda b: b.find_elements(By.ID, "redo-transcript-panel"), 5)
        browser.switch_to.active_element.send_keys(Keys.ESCAPE)
        check("the transcript's too", wait_for(browser, lambda b: not b.find_elements(By.ID, "redo-transcript-panel"), 3)
              and browser.execute_script("return document.activeElement.id") == "redo-transcript")
        # The setting's word, a moment.
        browser.execute_script("location.hash = '#/settings'")
        wait_for(browser, lambda b: b.find_elements(By.ID, "reminder"), 10)
        time.sleep(0.5)
        browser.execute_script("document.getElementById('reminder').click()")
        here = lambda b: b.find_elements(By.ID, "saved-said")
        told = wait_for(browser, here, 3)
        check("a setting written says Enregistré, under it", told and here(browser)[0].text == "Enregistré"
              and here(browser)[0].get_attribute("role") == "status"
              and browser.execute_script("const s = document.getElementById('saved-said'), r = document.getElementById('reminder').closest('label, .row, .field'); return r.nextElementSibling === s"))
        time.sleep(1)
        check("still said after a second", here(browser))
        check("gone at a second and a half", wait_for(browser, lambda b: not here(b), 1.5))
        refuse = "const real = window.__TAURI__.core; window.__TAURI__.core = { ...real, invoke: (asked, ...rest) => asked === 'settings_saved' ? Promise.reject('REFUS') : real.invoke(asked, ...rest) }; window.restore = () => { window.__TAURI__.core = real; };"
        browser.execute_script(refuse)
        browser.execute_script("document.getElementById('reminder').click()")
        refused = wait_for(browser, lambda b: b.find_element(By.ID, "not-saved").is_displayed(), 5)
        check("a write refused: said, no Enregistré", refused and not here(browser), browser.find_element(By.ID, "not-saved").text)
        browser.execute_script("window.restore()")
        # A radio with no id, far down at the smallest size: said under it, in view.
        time.sleep(0.5)
        browser.set_window_size(420, 400)
        browser.execute_script("document.querySelector('input[name=after][value=nothing]').scrollIntoView(); document.querySelectorAll('input[name=after]')[1].click()")
        wait_for(browser, here, 3)
        check("said under a radio with no id, in view", here(browser) and browser.execute_script(
            "const r = document.getElementById('saved-said').getBoundingClientRect(); return r.top >= 0 && r.bottom <= innerHeight && document.getElementById('saved-said').previousElementSibling.querySelector('input[name=after]') !== null"))
        wait_for(browser, lambda b: not here(b), 3)
        # A button in no row: said beside it, the page not sent to its top.
        browser.execute_script("const p = document.getElementById('minutes'); p.scrollIntoView({block: 'center'}); p.value = 'custom'; p.dispatchEvent(new Event('change', {bubbles: true}))")
        wait_for(browser, lambda b: b.find_elements(By.ID, "minutes-address"), 5)
        browser.find_element(By.ID, "minutes-address").send_keys("http://127.0.0.1:9/v1")
        browser.find_element(By.ID, "minutes-model").send_keys("mistral")
        browser.execute_script("document.getElementById('minutes-save').click()")
        wait_for(browser, here, 5)
        check("the minutes' Enregistrer: said beside its button, in view", here(browser) and browser.execute_script(
            "const s = document.getElementById('saved-said'), r = s.getBoundingClientRect(); return s.previousElementSibling?.id === 'minutes-save' && r.top >= 0 && r.top + r.height / 2 <= innerHeight"),
              browser.execute_script("const s = document.getElementById('saved-said'); return [s?.previousElementSibling?.id, s?.getBoundingClientRect().top]"))
        browser.set_window_size(1100, 800)
        browser.execute_script("location.hash = '#/'")
        time.sleep(0.5)
        # The real command, a double click: the second is the same press, and the button says so at once.
        browser.execute_script("const r = document.getElementById('record'); r.click(); setTimeout(() => r.click(), 200);")
        time.sleep(1)
        check("a double click: recording, and the button says it, not the stop that was not made",
              running(browser) and said(browser) == "Arrêter l’enregistrement", said(browser))
        time.sleep(0.6)
        browser.execute_script("document.getElementById('record').click()")
        wait_for(browser, lambda b: not running(b), 10)
        time.sleep(0.6)
        # A press that does nothing, as a start that failed: the button comes back as soon as it is answered.
        browser.execute_script("const real = window.__TAURI__.core; window.__TAURI__.core = { ...real, invoke: (asked, ...rest) => asked === 'record' ? Promise.resolve() : real.invoke(asked, ...rest) }; window.restore = () => { window.__TAURI__.core = real; };")
        browser.execute_script("document.getElementById('record').click()")
        check("a start that did nothing: the button comes back", wait_for(browser, lambda b: said(b) == "Démarrer un enregistrement", 1), said(browser))
        browser.execute_script("window.restore()")
        # Its command two seconds long: the header answers first.
        browser.execute_script(slow)
        browser.execute_script("const r = document.getElementById('record'); r.click(); r.click();")
        check("Démarrer: the button says its new state before the command answers, a second click asks nothing",
              said(browser) == "Arrêter l’enregistrement" and browser.execute_script("return window.__asked") == 1, said(browser))
        check("the recording did start, once", wait_for(browser, running, 8))
        time.sleep(1.5)
        browser.execute_script("const r = document.getElementById('record'); r.click(); r.click();")
        check("Arrêter: the same, the button waiting", said(browser) == "Arrêt…" and not browser.find_element(By.ID, "record").is_enabled()
              and browser.execute_script("return window.__asked") == 2, said(browser))
        check("the recording did stop, once", wait_for(browser, lambda b: not running(b), 15))
        check("ended, Démarrer again", wait_for(browser, lambda b: said(b) == "Démarrer un enregistrement" and b.find_element(By.ID, "record").is_enabled(), 5), said(browser))
    with Window(meetings, "fr_FR.UTF-8", 'after = "nothing"\nreminder = false\n', small=True) as browser:
        browser.execute_script(slow)
        browser.execute_script("const r = document.getElementById('small-record'); r.click(); r.click();")
        start = browser.find_element(By.ID, "small-record")
        check("the small window's Démarrer: at once, disabled", start.text == "Démarrage…" and not start.is_enabled()
              and browser.execute_script("return window.__asked") == 1, start.text)
        wait_for(browser, lambda b: b.find_element(By.ID, "small-stop").is_displayed(), 10)
        time.sleep(1)
        browser.execute_script("const r = document.getElementById('small-stop'); r.click(); r.click();")
        stop = browser.find_element(By.ID, "small-stop")
        check("its Arrêter: the same", stop.text == "Arrêt…" and not stop.is_enabled() and browser.execute_script("return window.__asked") == 2, (stop.text, stop.is_enabled(), browser.execute_script("return window.__asked")))
        back = wait_for(browser, lambda b: b.find_element(By.ID, "small-record").is_displayed() and b.find_element(By.ID, "small-record").is_enabled(), 15)
        check("at rest again, its Démarrer back", back and browser.find_element(By.ID, "small-record").text == "Démarrer un enregistrement")
    with Window(meetings, "fr_FR.UTF-8", 'after = "nothing"\nreminder = false\n', small=True) as browser:
        # A start the shell lets go later: true when first read, false after, no recording seen.
        browser.execute_script("const real = window.__TAURI__.core; let read = 0; window.__TAURI__.core = { ...real, invoke: (asked, ...rest) => asked === 'record' ? Promise.resolve() : asked === 'header' ? Promise.resolve({ recording: read++ === 0, stopping: false, shortcut: null }) : real.invoke(asked, ...rest) }; window.restore = () => { window.__TAURI__.core = real; };")
        browser.execute_script("document.getElementById('small-record').click()")
        late = wait_for(browser, lambda b: b.find_element(By.ID, "small-record").is_enabled() and b.find_element(By.ID, "small-record").text == "Démarrer un enregistrement", 3)
        check("a start refused late: its Démarrer comes back", late, browser.find_element(By.ID, "small-record").text)
        browser.execute_script("window.restore()")
        time.sleep(0.6)
        # Arrêter within the double press: it was not made, so the button is not stuck.
        browser.execute_script("document.getElementById('small-record').click()")
        wait_for(browser, lambda b: b.find_element(By.ID, "small-stop").is_displayed(), 10)
        browser.execute_script("document.getElementById('small-stop').click()")
        again = wait_for(browser, lambda b: b.find_element(By.ID, "small-stop").is_enabled() and b.find_element(By.ID, "small-stop").text == "Arrêter", 3)
        if again:
            check("a stop taken for the same press: Arrêter comes back", True)
            time.sleep(0.6)
            press(browser, "small-stop")
        else:
            check("a stop past the double press: stopped", wait_for(browser, lambda b: b.find_element(By.ID, "small-record").is_displayed(), 10))


def window_looks(now: dt.datetime) -> None:
    """AR. The window looks finished: Petite fenêtre an icon with its name, one primary action, little motion (D69)."""
    print("AR. one primary action, Petite fenêtre an icon named for assistive technology; 200 ms at most between states (D69)")
    meetings = WORK / "looking"
    made = meetings / "2026-10-28_09-00"
    session(made, now - dt.timedelta(days=1), 10, {"local.wav": 1000})
    with Window(meetings, "fr_FR.UTF-8", 'after = "nothing"\nreminder = false\n') as browser:
        browser.set_window_size(1100, 800)
        wait_for(browser, lambda b: b.find_element(By.ID, "meeting").is_displayed(), 10)
        small = browser.find_element(By.ID, "small-link")
        check("Petite fenêtre: an icon, named for assistive technology and to the pointer", small.text == "" and small.get_attribute("aria-label") == "Petite fenêtre"
              and small.get_attribute("title") == "Petite fenêtre" and small.find_elements(By.CSS_SELECTOR, "svg"), small.text)
        # Drawn, and seen: stroked in the text's colour, the size of the text; a rule of another name once hid it.
        drawn = browser.execute_script("const v = document.querySelector('#small-link svg'), c = getComputedStyle(v), p = getComputedStyle(v.parentElement); return [c.stroke === p.color, c.fill, parseFloat(c.width) === parseFloat(p.fontSize), parseFloat(c.height) === parseFloat(p.fontSize)]")
        check("the icon is stroked, the size of its text", drawn == [True, "none", True, True], drawn)
        primaries = browser.execute_script("return [...document.querySelectorAll('#sidebar .primary')].filter(b => b.offsetParent).length")
        check("the header shows one primary action", primaries == 1, primaries)
        for theme in ("light", "dark"):
            shot_themed(browser, f"d69-full-{theme}", theme)
        # A chip turning: the making stood in, then the minutes there.
        browser.execute_script("const real = window.__TAURI__.core, name = arguments[0]; window.__TAURI__.core = { ...real, invoke: async (asked, ...rest) => { const answer = await real.invoke(asked, ...rest); return asked === 'makings' ? { ...answer, minutes: { name, model: 'm', provider: 'p' } } : answer; } }; window.restore = () => { window.__TAURI__.core = real; };", made.name)
        wait_for(browser, lambda b: b.execute_script("return [...document.querySelectorAll('.chip.making')].some(c => !c.hidden)"), 8)
        (made / "minutes.md").write_text(
            "---\ncontract: minutes/v1\nmade: 2026-10-28T10:00:00+01:00\nprovider: local\nmodel: m\ncontext: null\n---\n\n"
            "## 1.\n\nLe budget.\n\n## 2.\n\n- Rien\n\n## 3.\n\n- Rien\n")
        browser.execute_script("window.restore(); window.dispatchEvent(new Event('focus'))")
        turned = wait_for(browser, lambda b: b.find_elements(By.CSS_SELECTOR, ".chip.turned"), 5)
        duration = browser.execute_script("const c = document.querySelector('.chip.turned'); return c ? parseFloat(getComputedStyle(c).animationDuration) : null")
        check("a chip turned: one small move, 200 ms at most", turned and duration is not None and 0 < duration <= 0.2, duration)
        once = browser.execute_script("const c = getComputedStyle(document.querySelector('.chip.turned')); return [c.animationIterationCount, c.animationDelay]")
        check("once, with no wait", once == ["1", "0s"], once)
        browser.execute_script("window.dispatchEvent(new Event('focus'))")
        time.sleep(0.8)
        check("once: drawn again, it stands still", not browser.find_elements(By.CSS_SELECTOR, ".chip.turned"))
        # Made again, the minutes there: nothing has turned, however often it is drawn.
        browser.execute_script("const real = window.__TAURI__.core, name = arguments[0]; window.__TAURI__.core = { ...real, invoke: async (asked, ...rest) => { const answer = await real.invoke(asked, ...rest); return asked === 'makings' ? { ...answer, minutes: { name, model: 'm', provider: 'p' } } : answer; } }; window.restore = () => { window.__TAURI__.core = real; };", made.name)
        wait_for(browser, lambda b: b.execute_script("return [...document.querySelectorAll('.chip.making')].some(c => !c.hidden)"), 8)
        seen = False
        for _ in range(3):
            browser.execute_script("window.dispatchEvent(new Event('focus'))")
            time.sleep(0.5)
            seen = seen or bool(browser.find_elements(By.CSS_SELECTOR, ".chip.turned"))
        browser.execute_script("window.restore()")
        check("minutes made again: no chip turned", not seen)
    with Window(meetings, "fr_FR.UTF-8", 'after = "nothing"\nreminder = false\n', small=True) as browser:
        moves = "return [...document.querySelectorAll('.small-rest, .small-live')].map(e => parseFloat(getComputedStyle(e).animationDuration))"
        lengths = browser.execute_script(moves)
        check("between the small window's states: 200 ms at most", lengths and all(0 < length <= 0.2 for length in lengths), lengths)
        # The page's own policy refuses what it may not load: a mark drawn from a data: address was blank (#597).
        browser.execute_script("window.__refused = []; document.addEventListener('securitypolicyviolation', (e) => window.__refused.push(e.violatedDirective + ' ' + e.blockedURI))")
        press(browser, "small-record")
        wait_for(browser, lambda b: b.find_element(By.ID, "small-stop").is_displayed(), 10)
        heard = wait_for(browser, lambda b: b.execute_script("return [...document.querySelectorAll('.slot[data-check=heard] .check svg')].some(v => getComputedStyle(v).display !== 'none' && v.getBoundingClientRect().width >= 10)"), 8)
        check("a side heard: its check drawn on its green, its size", heard)
        shot(browser, "d69-heard")
        check("nothing refused by the page's policy", browser.execute_script("return window.__refused.length") == 0, browser.execute_script("return window.__refused"))
        time.sleep(0.6)
        press(browser, "small-stop")
        full = browser.find_element(By.ID, "small-full")
        check("its way to the full window: an icon, named", full.text == "" and full.find_elements(By.CSS_SELECTOR, "svg") and full.get_attribute("aria-label"), full.text)
        check("no less motion asked: the page's query says so", not browser.execute_script(LESS_MOTION))
    # Less motion asked, in a running window: rest to recording to rest, and a chip turning, stand still.
    with asking_less_motion(Window(meetings, "fr_FR.UTF-8", 'after = "nothing"\nreminder = false\n', small=True)) as browser:
        check("less motion asked: the page's query says so", browser.execute_script(LESS_MOTION))
        browser.execute_script(WATCH_MOTION)
        check("at rest, nothing moves", browser.execute_script(MOVING) == [], browser.execute_script(MOVING))
        press(browser, "small-record")
        wait_for(browser, lambda b: b.find_element(By.ID, "small-stop").is_displayed(), 10)
        time.sleep(0.6)
        check("the small window recording: nothing moved, nothing can", browser.execute_script(MOVING) == [], browser.execute_script(MOVING))
        press(browser, "small-stop")
        wait_for(browser, lambda b: b.find_element(By.ID, "small-record").is_displayed(), 10)
        check("back at rest: nothing moved, nothing can", browser.execute_script(MOVING) == [], browser.execute_script(MOVING))
    (made / "minutes.md").unlink()
    with asking_less_motion(Window(meetings, "fr_FR.UTF-8", 'after = "nothing"\nreminder = false\n')) as browser:
        browser.set_window_size(1100, 800)
        wait_for(browser, lambda b: b.find_element(By.ID, "meeting").is_displayed(), 10)
        check("less motion asked: the page's query says so", browser.execute_script(LESS_MOTION))
        browser.execute_script(WATCH_MOTION)
        browser.execute_script("const real = window.__TAURI__.core, name = arguments[0]; window.__TAURI__.core = { ...real, invoke: async (asked, ...rest) => { const answer = await real.invoke(asked, ...rest); return asked === 'makings' ? { ...answer, minutes: { name, model: 'm', provider: 'p' } } : answer; } }; window.restore = () => { window.__TAURI__.core = real; };", made.name)
        wait_for(browser, lambda b: b.execute_script("return [...document.querySelectorAll('.chip.making')].some(c => !c.hidden)"), 8)
        (made / "minutes.md").write_text(
            "---\ncontract: minutes/v1\nmade: 2026-10-28T10:00:00+01:00\nprovider: local\nmodel: m\ncontext: null\n---\n\n"
            "## 1.\n\nLe budget.\n\n## 2.\n\n- Rien\n\n## 3.\n\n- Rien\n")
        browser.execute_script("window.restore(); window.dispatchEvent(new Event('focus'))")
        turned = wait_for(browser, lambda b: b.find_elements(By.CSS_SELECTOR, ".chip.turned"), 5)
        said = browser.execute_script("const c = document.querySelector('.chip.turned'); return c ? getComputedStyle(c).animationName : null")
        check("a chip turned: it does not move", turned and said == "none", said)
        check("the full window: nothing moved, nothing can", browser.execute_script(MOVING) == [], browser.execute_script(MOVING))


def main() -> int:
    now = dt.datetime.now().astimezone()
    sessions = WORK / "sessions"
    # Sessions this week, last week, earlier this month when there is room for one, two months
    # ago; one whose start is written with an offset of hours alone; one whose start is no instant.
    this, last, months = now - dt.timedelta(seconds=30), now - dt.timedelta(days=7), now - dt.timedelta(days=61)
    earlier = (week_of(now) - dt.timedelta(days=8)).replace(hour=12)
    hours_only = (now - dt.timedelta(days=3)).astimezone(dt.timezone(dt.timedelta(hours=2)))
    session(
        sessions / "a", last, 42, {"me.wav": 100_000_000, "others.wav": 100_000_000, "transcript.jsonl": 7000, "minutes.md": 500}, "v1"
    )
    session(sessions / "b", this, None, {"local.wav": 2000})
    session(sessions / "c", months, 35, {"transcript.jsonl": 3000})
    session(sessions / "e", earlier, 20, {"transcript.jsonl": 1000})
    session(sessions / "f", hours_only, 10, {"local.wav": 999_999})
    record = json.loads((sessions / "f" / "session.json").read_text())
    for key in ("started", "stopped"):
        record[key] = record[key].replace("+02:00", "+02")
    (sessions / "f" / "session.json").write_text(json.dumps(record))
    (sessions / "g").mkdir(parents=True)
    (sessions / "g" / "session.json").write_text(json.dumps({"contract": "session-audio/v2", "started": "yesterday", "tracks": {}}))
    (sessions / "not-a-session").mkdir()
    expected = sorted(
        [
            (this, "Interrompu|Audio · 2 ko"),
            (last, "42 min|Compte rendu|Transcription|Audio · 200 Mo"),
            (months, "35 min|Transcription|Audio supprimé"),
            (earlier, "20 min|Transcription|Audio supprimé"),
            (hours_only, "10 min|Audio · 1 Mo"),
        ],
        key=lambda pair: pair[0],
        reverse=True,
    )
    expected = [(group(day.astimezone(), now), holds) for day, holds in expected]
    expected.append(("Date inconnue", "Interrompu|Audio supprimé"))

    print("H. the window, in French: the meetings by week, what each holds and weighs")
    with Window(sessions, "fr_FR.UTF-8") as browser:
        check("the sidebar is named in French", browser.find_element(By.ID, "sidebar").get_attribute("aria-label") == "Réunions")
        lang = browser.execute_script("return document.documentElement.lang")
        check("the page says it is in French", lang == "fr", lang)
        sections = browser.find_element(By.TAG_NAME, "nav").get_attribute("aria-label")
        check("its sections are named in French for a screen reader", sections == "Rubriques", sections)
        listed = rows(browser)
        shot(browser, "1-sessions")
        granted = browser.find_element(By.ID, "header-keys").text
        check("the shortcut is said as a person reads it, never in GTK's notation (#312)",
              "<" not in granted and "Press" not in granted, granted)
        # The header's gesture, as the board has it — never pressed here: no recorder is played.
        said = browser.find_element(By.ID, "record").text
        check("the header offers to start a recording", said == "Démarrer un enregistrement", said)
        drawn = browser.execute_script(
            "return [...document.querySelectorAll('#groups .session')].map(s => s.querySelector('.relief.small:not(.quiet) .said') !== null)"
        )
        check("a transcript that cannot be read draws no one", drawn and True not in drawn, drawn)
        # A meeting with its audio and no transcript offers one — never asked here: no engine is played.
        browser.execute_script("location.hash = '#/meeting/f'")
        offered = wait_for(browser, lambda b: b.find_elements(By.ID, "make-transcript"), 5)
        check("a meeting without its transcript offers it on its page", offered)
        browser.execute_script("location.hash = '#/'")
        wait_for(browser, lambda b: b.find_element(By.ID, "list").is_displayed(), 5)
        check(
            "every session, newest first, each under its week; the one with no readable day last",
            [(heading, holds) for heading, _, holds in listed] == expected,
            listed,
        )
        check("a session with no readable day is named by its folder", listed[-1][1] == "g", listed[-1])
        total = browser.find_element(By.ID, "weight").text
        check("what they weigh, audio against the rest", total.startswith("201 Mo occupés par 6 réunions · 52 min d’audio"), total)
        check("the picture of the weights is described", "L’audio occupe 201 Mo" in (browser.find_element(By.ID, "bar").get_attribute("aria-label") or ""))
        # The theme is the system's: the page's ground is that of the theme the system shows.
        dark = browser.execute_script("return matchMedia('(prefers-color-scheme: dark)').matches")
        ground = browser.execute_script("return getComputedStyle(document.body).backgroundColor")
        check(f"the ground of the system's {'dark' if dark else 'light'} theme", ground == GROUND["dark" if dark else "light"], ground)
        # Every action from the keyboard: the first stop of Tab is the section's link.
        browser.find_element(By.TAG_NAME, "body").send_keys(Keys.TAB)
        focused = browser.execute_script("return document.activeElement.id")
        check("Tab reaches the window's first action, in the sidebar: the button that folds it (D61)", focused == "fold", focused)
        # A session recorded meanwhile is shown when the window comes forward again.
        session(sessions / "d", now - dt.timedelta(minutes=5), 3, {"local.wav": 1000, "remote.wav": 1000})
        browser.execute_script("window.dispatchEvent(new Event('focus'))")
        came = wait_for(browser, lambda b: len(rows(b)) == 7)
        check("a new session is listed when the window comes forward", came, rows(browser))

    print("J. one meeting: its minutes beside its transcript, searched")
    meetings = WORK / "meetings"
    session(meetings / "m", now - dt.timedelta(hours=3), 16, {"local.wav": 4000, "remote.wav": 4000})
    lines = [
        {"type": "header", "contract": "transcript/v1", "engine": "whisper.cpp", "model": "ggml-large-v3-turbo-q8_0", "language": "fr", "context": None},
        {"type": "segment", "speaker": "others", "track": "remote", "voice": None, "start": 8.0, "end": 11.5, "text": "Bonjour Claire, on peut commencer."},
        {"type": "segment", "speaker": "me", "track": "local", "voice": "1", "start": 1.0, "end": 7.2, "text": "Bonjour à tous, merci d’être là."},
        {"type": "segment", "speaker": "me", "track": "local", "voice": "2", "start": 15.0, "end": 19.0, "text": "La version de test sera prête le 12 novembre."},
        {"type": "complete", "duration": 960.0},
    ]
    (meetings / "m" / "transcript.jsonl").write_text("".join(json.dumps(line, ensure_ascii=False) + "\n" for line in lines))
    (meetings / "m" / "minutes.md").write_text(
        "---\ncontract: minutes/v1\nmade: 2026-10-13T12:21:00+02:00\nprovider: openrouter.ai\nmodel: claude-sonnet\ncontext: null\n---\n\n"
        # Numbered, as a small local model writes them (#261): the page names them.
        "## 1.\n\nL’ouverture est fixée au lundi 16 novembre.\n\n"
        "## 2.\n\n- Mettre à jour le planning — Marc — aujourd’hui\n- Couper deux serveurs de test — Julien — avant vendredi\n\n"
        "## 3.\n\n- Un serveur est gardé pour la démonstration du 20.\n"
    )
    session(meetings / "n", now - dt.timedelta(days=1), 5, {"local.wav": 100})
    (meetings / "n" / "minutes.md").write_text("Not minutes.\n")
    (meetings / "n" / "transcript.jsonl").write_text(
        json.dumps(lines[0]) + "\n" + json.dumps({"type": "segment", "speaker": "me", "track": "local", "voice": None, "start": 1.0, "end": 2.0, "text": "Une autre réunion."}) + "\n"
    )
    with Window(meetings, "fr_FR.UTF-8") as browser:
        browser.find_element(By.TAG_NAME, "body").send_keys(Keys.TAB, Keys.TAB)
        focused = browser.execute_script("return document.activeElement.id")
        check("Tab reaches the sidebar's gesture after the button that folds it (D61)", focused == "record", focused)
        browser.find_element(By.ID, "record").send_keys(Keys.TAB)
        focused = browser.execute_script("return document.activeElement.id")
        check("then the search of the meetings", focused == "find", focused)
        browser.find_element(By.ID, "find").send_keys(Keys.TAB)
        focused = browser.execute_script("return document.activeElement.className")
        check("then the first meeting", "session" in focused, focused)
        browser.execute_script("document.querySelector('#groups li:last-child .session').focus()")
        browser.switch_to.active_element.send_keys(Keys.TAB)
        focused = browser.execute_script("return document.activeElement.id")
        check("after the meetings, freeing space, beside the weights", focused == "free-link", focused)
        drawn = browser.execute_script("return [...document.querySelectorAll('#groups .session')].map(s => s.querySelector('.relief.small:not(.quiet) .said') !== null)")
        check("a meeting whose transcript reads shows who spoke, small, beside it", True in drawn, drawn)
        shot(browser, "1-sessions-drawn")
        browser.find_elements(By.CSS_SELECTOR, ".session")[0].click()
        shown = wait_for(browser, lambda b: b.find_element(By.ID, "meeting").is_displayed() and b.find_elements(By.CSS_SELECTOR, ".lines li"))
        check("a click on a meeting opens it, marked in the list (D61)", shown
              and browser.find_elements(By.CSS_SELECTOR, ".session")[0].get_attribute("aria-current") == "page")
        heard_here = [p.text for p in browser.find_elements(By.CSS_SELECTOR, "#meeting .made .maker")]
        check("a meeting heard here says so, by the model's name", heard_here == ["Transcrit par Whisper large v3 turbo, sur cet ordinateur"], heard_here)
        shot(browser, "2-session")
        minutes = browser.find_element(By.CSS_SELECTOR, ".minutes")
        titles = [h.text for h in minutes.find_elements(By.CSS_SELECTOR, ".body h2")]
        check("the minutes' three sections, named by the page whatever the model titled them", titles == ["Points saillants", "Actions", "Points clés"], titles)
        items = [li.text for li in minutes.find_elements(By.CSS_SELECTOR, ".body li")]
        actions = browser.execute_script("return [...document.querySelectorAll('#meeting .minutes table.actions tbody tr')].map(r => [...r.cells].map(c => c.textContent))")
        check(
            "the minutes as written: each action a row, Qui · Quoi · Pour quand (D46), each key point",
            actions == [["Marc", "Mettre à jour le planning", "aujourd’hui"], ["Julien", "Couper deux serveurs de test", "avant vendredi"]]
            and items == ["Un serveur est gardé pour la démonstration du 20."],
            (actions, items),
        )
        drawn = browser.execute_script(
            "const s = document.querySelector('#meeting .relief'); return [s.getAttribute('aria-label'),"
            " ...['local', 'remote'].map(t => s.querySelector('.' + t).getAttribute('points').split(' ').map(p => +p.split(',')[1]))]"
        )
        label, here, remote = drawn
        # Speech in the first sixtieth of the meeting, on each side: here rises above the middle, remote falls below.
        check(
            "who spoke is drawn over the meeting, here above and remote below",
            label.startswith("Qui a parlé") and len(here) == len(remote) == 60 and here[0] < 42 and remote[0] > 46 and here[-1] == 42,
            drawn,
        )
        maker = minutes.find_element(By.CSS_SELECTOR, ".maker").text
        check(
            "who wrote the minutes, and where, as the board says it — the model as people say it, the day without its year",
            maker.startswith("Écrit par Claude Sonnet, via openrouter.ai · ") and "2026" not in maker and "," not in maker.split(" · ")[-1] and " à " in maker.split(" · ")[-1],
            maker,
        )
        named = browser.execute_script(
            "return ['anthropic/claude-sonnet-4-5', 'mistral:7b-instruct-q4_K_M', 'llama3.1:8b-fp16', 'org/model/', 'gpt-4o-mini',"
            " 'claude-sonnet-4-5-20250929', 'openai/gpt-4o-2024-08-06', 'llama3.1:latest', 'model-q4_K_M.gguf', 'q4_K_M', 'qwen3:14b-q8_0'].map(modelSaid)"
        )
        check(
            "a model as people say it: a version with its point, no date, tag, file or quantisation, a name even after a last slash",
            named == ["Claude Sonnet 4.5", "Mistral 7b Instruct", "Llama3.1 8b", "Model", "GPT 4o Mini", "Claude Sonnet 4.5", "GPT 4o", "Llama3.1", "Model", "q4_K_M", "Qwen3 14b"],
            named,
        )
        said = browser.execute_script(
            "return [...document.querySelectorAll('.lines li')].map(li => [li.querySelector('.at'), li.querySelector('.who'), li.querySelector('.text')].map(c => c.textContent).join(' | '))"
        )
        check(
            "the transcript in the meeting's order, each line with its side and voice",
            said
            == [
                "00:01 | Ici · voix 1 | Bonjour à tous, merci d’être là.",
                "00:08 | À distance | Bonjour Claire, on peut commencer.",
                "00:15 | Ici · voix 2 | La version de test sera prête le 12 novembre.",
            ],
            said,
        )
        search = browser.find_element(By.ID, "search")
        search.send_keys("ETRE")
        visible = browser.execute_script(
            "return [...document.querySelectorAll('.lines li')].filter(li => !li.hidden).map(li => li.querySelector('.text').textContent)"
        )
        check("a search finds what was said, whatever its accents and case", visible == ["Bonjour à tous, merci d’être là."], visible)
        search.clear()
        search.send_keys("zzz")
        none = [p.text for p in browser.find_elements(By.CSS_SELECTOR, ".transcript .none") if p.is_displayed()]
        check("a search that finds nothing says so", none == ["Aucun passage ne correspond."], none)
        search.clear()
        search.send_keys("voix")
        none = [p.text for p in browser.find_elements(By.CSS_SELECTOR, ".transcript .none") if p.is_displayed()]
        check("a search looks at what was said, not at who or when", none == ["Aucun passage ne correspond."], none)
        check("the folder can be opened from the meeting", browser.find_element(By.CSS_SELECTOR, "#meeting .meeting-top .open").text == "Ouvrir le dossier")
        browser.find_elements(By.CSS_SELECTOR, ".session")[1].click()
        wait_for(browser, lambda b: b.find_element(By.ID, "meeting").is_displayed() and b.find_elements(By.CSS_SELECTOR, ".minutes .none"))
        kept = browser.execute_script("return document.getElementById('search') && document.getElementById('search').value")
        check("another meeting opens without the last one's search", not kept, kept)
        notes = [p.text for p in browser.find_elements(By.CSS_SELECTOR, "#meeting .none") if p.is_displayed()]
        check("minutes that cannot be read are said so", notes == ["Ce fichier ne peut pas être lu par cette version de Wholeheard."], notes)
        chips = browser.find_element(By.CSS_SELECTOR, "#meeting .holds").text
        check("minutes that cannot be read claim no place", "Compte rendu" in chips and "Compte rendu ·" not in chips, chips)
        being = [p.text for p in browser.find_elements(By.CSS_SELECTOR, ".transcript .note")]
        check("a transcript without its end says it is being written", being == ["En cours d’écriture — ce qui est là est vrai quand même."], being)

        browser.execute_script("location.hash = '#/meeting/' + encodeURIComponent('gone')")
        listed = wait_for(browser, lambda b: b.execute_script("return location.hash") == "#/")
        drawn = wait_for(browser, lambda b: b.execute_script("return document.getElementById('meeting').dataset.name") == "m", 5)
        check("a meeting that is not there goes back to the list, the last meeting beside it (D61)", listed and drawn,
              browser.execute_script("return document.getElementById('meeting').dataset.name"))

    print("K. the settings: each choice saved at once, the user's file kept")
    models = WORK / "data" / "wholeheard" / "models"
    models.mkdir(parents=True, exist_ok=True)
    for name in ("ggml-large-v3-turbo-q8_0.bin", "ggml-small.bin"):
        (models / name).write_bytes(b"x" * 1000)
    chosen = WORK / "chosen"
    chosen.mkdir()
    window = Window(chosen, "fr_FR.UTF-8", "# mine\n")
    with window as browser:
        browser.find_element(By.ID, "settings-link").click()
        wait_for(browser, lambda b: b.find_element(By.ID, "settings").is_displayed() and b.find_elements(By.ID, "model"))
        check("the settings open from the header", browser.find_element(By.ID, "settings-link").text == "Réglages")
        shot(browser, "4-settings")
        browser.execute_script("window.scrollTo(0, document.body.scrollHeight)")
        shot(browser, "4-settings-end")
        browser.execute_script("window.scrollTo(0, 0)")
        checked = browser.execute_script("return document.querySelector('input[name=after]:checked').value")
        check("out of the box, the transcript follows", checked == "transcript", checked)
        names = [o.text.split(" — ")[0] for o in browser.find_elements(By.CSS_SELECTOR, "#model option")]
        check("the models on this computer are offered, by the names people say", names == ["Whisper large v3 turbo", "Whisper small", "OpenRouter", "OpenAI", "Un service que je choisis"], names)
        ready = browser.find_element(By.ID, "model-state").text
        check("the transcription model's state is said at the page's end", "Prêt" in ready and "sans réseau" in ready, ready)
        # Each change is saved, then the page is drawn again: the next acts on the new one.
        # The page drawn again under the driver — the window coming back to the front draws it anew,
        # the desktop being in use —: the gesture made again on the new one.
        def changed(act):
            for _ in range(5):
                before = browser.find_element(By.ID, "reminder")
                try:
                    act()
                    break
                except StaleElementReferenceException:
                    time.sleep(0.2)
            WebDriverWait(browser, 5).until(staleness_of(before))

        # Back to Settings from another page: its last drawing stays in the hidden page until the
        # new one replaces it — waited for, so that nothing is clicked or read on the old one.
        def to_settings():
            old = browser.find_elements(By.ID, "reminder")
            browser.execute_script("location.hash = '#/settings'")
            if old:
                WebDriverWait(browser, 5).until(staleness_of(old[0]))
            wait_for(browser, lambda b: b.find_elements(By.ID, "developer"))

        # A refusal is said on the page drawn after the save: waited for, never read too soon.
        def not_saved():
            wait_for(browser, lambda b: b.find_elements(By.ID, "not-saved") and b.find_element(By.ID, "not-saved").text, 5)
            return browser.find_element(By.ID, "not-saved").text

        changed(lambda: browser.find_element(By.CSS_SELECTOR, "input[name=after][value=nothing]").click())
        changed(lambda: Select(browser.find_element(By.ID, "keep")).select_by_value("30"))
        changed(lambda: Select(browser.find_element(By.ID, "model")).select_by_visible_text(
            next(o.text for o in browser.find_elements(By.CSS_SELECTOR, "#model option") if o.text.startswith("Whisper small"))
        ))
        # The user changes the file by hand while Wholeheard runs: the next save keeps it.
        window.file.write_text(window.file.read_text().replace("keep_audio = 30", "keep_audio = 90 # by hand"))
        changed(lambda: browser.find_element(By.ID, "reminder").click())
        changed(lambda: browser.find_element(By.ID, "developer").click())
        # When an application takes the microphone (D30, board 4): offer to record by default;
        # nothing chosen here, then the offer again, each written.
        offer_on = read(browser, lambda b: Select(b.find_element(By.ID, "offer")).first_selected_option.get_attribute("value")) == "offer"
        changed(lambda: Select(browser.find_element(By.ID, "offer")).select_by_value("nothing"))
        offer_off = "offer = false" in window.file.read_text()
        changed(lambda: Select(browser.find_element(By.ID, "offer")).select_by_value("offer"))
        offer_again = "offer = true" in window.file.read_text()
        changed(lambda: Select(browser.find_element(By.ID, "offer")).select_by_value("nothing"))
        # Started with the user's session (D32): on from the first start, the desktop's own entry;
        # off, then turned off by the desktop's tools, then on again, each read from the entry.
        entry = window.file.parent.parent / "autostart" / "dev.wholeheard.Wholeheard.desktop"
        switch = lambda: browser.find_element(By.ID, "autostart")
        start_on = read(browser, lambda b: switch().is_selected()) and entry.is_file() and "Exec=wholeheard\n" in entry.read_text()
        changed(lambda: switch().click())
        start_off = not read(browser, lambda b: switch().is_selected()) and not entry.exists() and "autostart = false" in window.file.read_text()
        entry.write_text("[Desktop Entry]\nType=Application\nExec=wholeheard\nX-GNOME-Autostart-enabled=false\n")
        browser.execute_script("location.hash = '#/'")
        wait_for(browser, lambda b: b.find_element(By.ID, "list").is_displayed(), 10)
        to_settings()
        tools_off = not read(browser, lambda b: switch().is_selected())
        changed(lambda: switch().click())
        start_again = read(browser, lambda b: switch().is_selected()) and "TryExec=wholeheard\n" in entry.read_text() and "autostart = true" in window.file.read_text()
        check("the session starts Wholeheard from its first start; turned off here or by the desktop's tools, on again",
              start_on and start_off and tools_off and start_again, (start_on, start_off, tools_off, start_again))
        text = window.file.read_text()
        check(
            "each choice is in the settings file, the user's lines kept, a change by hand too",
            'after = "nothing"' in text
            and "reminder = false" in text
            and "developer = true" in text
            and offer_on and offer_off and offer_again and "offer = false" in text
            and "keep_audio = 90 # by hand" in text
            and f'model = "{models / "ggml-small.bin"}"' in text
            and text.startswith(f'sessions = "{chosen}"\n')
            and "# mine" in text,
            text,
        )
        # On, the developer mode offers the logs' level and their folder (D29).
        check("the developer mode offers the logs' level and their folder",
              browser.find_elements(By.ID, "log-level") and browser.find_elements(By.ID, "open-logs"))
        changed(lambda: Select(browser.find_element(By.ID, "log-level")).select_by_value("debug"))
        check("the logs' level chosen is in the file", 'log = "debug"' in window.file.read_text(), window.file.read_text())
        changed(lambda: Select(browser.find_element(By.ID, "log-level")).select_by_value("info"))
        check("the usual level chosen again is in the file", 'log = "info"' in window.file.read_text(), window.file.read_text())
        changed(lambda: browser.find_element(By.ID, "developer").click())
        check("the developer mode turned off again is said so in the file, and hides its rows",
              "developer = false" in window.file.read_text() and not browser.find_elements(By.ID, "log-level"), window.file.read_text())
        failed = browser.find_element(By.ID, "not-saved").is_displayed()
        check("nothing said failed", not failed)
        # Any number of days, when the list has not the one wanted.
        choose(browser, "keep", "other")
        typed = browser.find_element(By.ID, "keep-other")
        changed(lambda: (typed.send_keys("45"), typed.send_keys(Keys.ENTER)))
        kept = window.file.read_text()
        offered = read(browser, lambda b: [o.text for o in b.find_elements(By.CSS_SELECTOR, "#keep option")])
        check("any number of days can be typed, and is offered after", "keep_audio = 45" in kept and "45 jours" in offered, (kept, offered))
        choose(browser, "keep", "other")
        typed_days = browser.find_element(By.ID, "keep-other")
        typed_days.send_keys("0", Keys.ENTER)
        refused = wait_for(browser, lambda b: b.find_element(By.ID, "not-saved").is_displayed(), 5)
        check(
            "a number that is no number of days is said, nothing saved",
            refused and "0 n’est pas un nombre de jours" in browser.find_element(By.ID, "not-saved").text and "keep_audio = 45" in window.file.read_text(),
            browser.find_element(By.ID, "not-saved").text,
        )
        # The shortcut: the keys the desktop granted, or that it granted none; its keyboard
        # settings offered where they can be opened — never opened here.
        keys = browser.find_element(By.ID, "shortcut").text
        offered = bool(browser.find_elements(By.ID, "shortcut-settings"))
        gnome = "gnome" in os.environ.get("XDG_CURRENT_DESKTOP", "").lower().split(":")
        seen = "none granted" if keys == "Aucun raccourci défini" else "keys granted"
        check(
            f"the shortcut is shown as the desktop granted it ({seen} here), and where it changes",
            keys != "" and offered == (gnome and shutil.which("gnome-control-center") is not None),
            f"{keys!r}, settings offered: {offered}",
        )
        # The sessions' folder: typed, made when it is not there, and what the list reads.
        elsewhere = WORK / "elsewhere" / "Wholeheard"
        spoken = browser.find_element(By.ID, "sessions-shown").get_attribute("textContent")
        # Outside the home folder, from the root: / tmp / … / chosen.
        check("the sessions' folder is shown as people say it", spoken == "/ " + " / ".join(chosen.parts[1:]), spoken)

        def folder_field():
            # A click on the folder as said makes it a path to type.
            browser.find_element(By.ID, "sessions-shown").click()
            return browser.find_element(By.ID, "sessions")

        typed = lambda field, text: (browser.execute_script("arguments[0].select()", field), field.send_keys(text, Keys.ENTER))  # noqa: E731
        folder = folder_field()
        check("a click makes it the path, to type", folder.get_attribute("value") == str(chosen), folder.get_attribute("value"))
        changed(lambda: typed(folder, str(elsewhere)))
        text = window.file.read_text()
        check("a folder typed is made, and in the settings file", elsewhere.is_dir() and f'sessions = "{elsewhere}"' in text, text)
        folder = folder_field()
        changed(lambda: typed(folder, "Wholeheard"))
        said = not_saved()
        check("a folder that is not a whole path is said, and nothing changes", said.startswith("Le réglage n’a pas été enregistré") and window.file.read_text() == text, said)
        browser.execute_script("location.hash = '#/'")
        listed = wait_for(browser, lambda b: str(elsewhere) in b.find_element(By.ID, "none").text, 5)
        check("the list reads the folder chosen", listed, browser.find_element(By.ID, "none").text)
        browser.find_element(By.ID, "settings-link").click()
        wait_for(browser, lambda b: b.find_elements(By.ID, "reminder"))
        # This computer's Ollama saved, and not answering (here, nothing listens on its port): said
        # so, never "it has no such model" from a list that could not be read.
        kept = window.file.read_text()
        window.file.write_text(kept + 'minutes_provider = "http://127.0.0.1:9/v1"\nminutes_model = "mistral:7b"\n')
        browser.execute_script("location.hash = '#/'")
        browser.execute_script("location.hash = '#/settings'")
        silent = wait_for(browser, lambda b: b.find_elements(By.ID, "minutes-silent"), 5)
        said = read(browser, lambda b: " ".join(p.text for p in b.find_elements(By.CSS_SELECTOR, "#minutes-where, #minutes-silent, #minutes-lacking")))
        check("this computer's Ollama not answering: said so, here, never a model said missing",
              silent and "Sur cet ordinateur" in said and "ne répond pas" in said and not browser.find_elements(By.ID, "minutes-lacking"), said)
        window.file.write_text(kept)
        browser.execute_script("location.hash = '#/'")
        to_settings()
        # A file broken by hand is never written over: the change is said not saved.
        # Developer mode off, the sentences alone, read whole; on, the raw text under each, without
        # the file's path (D47).
        held = window.file.read_text()
        lines = [line for line in held.splitlines() if not line.startswith("keep_audio")] + ["keep_audio = 0"]
        broken = "\n".join(lines) + "\n"
        refused = "Le réglage n’a pas été enregistré\u00a0: le fichier des réglages ne peut pas être lu\u00a0: corrigez-le, puis réessayez.".replace("\xa0", " ")  # WebDriver's `.text` reads U+00A0 as a space
        locked_said = "Le fichier des réglages ne peut pas être lu\u00a0: Wholeheard ne l’écrit pas tant qu’il n’est pas corrigé.".replace("\xa0", " ")  # WebDriver's `.text` reads U+00A0 as a space
        unreadable = lambda: browser.find_element(By.ID, "settings-unreadable").text  # noqa: E731
        window.file.write_text(broken)
        changed(lambda: browser.find_element(By.ID, "reminder").click())
        said = not_saved()
        check("a file that cannot be read is not written over", said == refused and window.file.read_text() == broken, said)
        check("developer mode off, a settings file that cannot be read is said in a sentence, no raw text", unreadable() == locked_said, unreadable())
        window.file.write_text(held)
        browser.execute_script("window.dispatchEvent(new Event('focus'))")
        wait_for(browser, lambda b: not b.find_element(By.ID, "reminder").get_attribute("disabled"), 5)
        changed(lambda: browser.find_element(By.ID, "developer").click())
        check("developer mode on again, to read a failure", "developer = true" in window.file.read_text(), window.file.read_text())
        held = window.file.read_text()
        lines = [line for line in held.splitlines() if not line.startswith("keep_audio")] + ["keep_audio = 0"]
        window.file.write_text("\n".join(lines) + "\n")
        raw = f'settings.toml, line {len(lines)}: keep_audio is "always" or a number of days'
        changed(lambda: browser.find_element(By.ID, "reminder").click())
        said = not_saved()
        check("developer mode on, the raw text is under the sentence of the command that failed, without its path", said == f"{refused}\n{raw}", said)
        check("developer mode on, the raw text is under the settings file's sentence too", unreadable() == f"{locked_said}\n{raw}", unreadable())
    # Who writes the minutes: an Ollama made here, an online service, an address of the user's. A
    # key goes to the scenario's own keyring, which lives as long as the program.
    class Tags(http.server.BaseHTTPRequestHandler):
        def do_GET(self):
            body = json.dumps({"models": [{"name": "qwen3:14b"}, {"name": "mistral:7b"}]}).encode()
            self.send_response(200)
            self.send_header("content-length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def log_message(self, *_):
            pass

    tags = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Tags)
    threading.Thread(target=tags.serve_forever, daemon=True).start()
    ollama = f"http://127.0.0.1:{tags.server_address[1]}"
    writers = WORK / "writers"
    writers.mkdir()
    window = Window(writers, "fr_FR.UTF-8", ollama=ollama)
    with window as browser:
        browser.find_element(By.ID, "settings-link").click()
        wait_for(browser, lambda b: b.find_elements(By.ID, "minutes"))
        offered = browser.execute_script("return [...document.querySelectorAll('#minutes option')].map(o => o.textContent)")
        check(
            "who can write the minutes: Ollama's models, the online services, one's own",
            offered == ["Pas encore choisi", "mistral:7b", "qwen3:14b", "OpenRouter", "OpenAI", "Mistral", "Personnalisé…"],
            offered,
        )
        # Going through the list never saves: what is chosen is saved on the word (D25).
        before = browser.find_element(By.ID, "minutes")
        Select(before).select_by_visible_text("qwen3:14b")
        Select(before).select_by_visible_text("mistral:7b")
        time.sleep(1)
        unsaid = browser.find_element(By.ID, "minutes-unsaved")
        shot(browser, "8-minutes-unsaved")
        check(
            "a model chosen in the list is not saved by itself; who still writes them is said",
            "minutes_" not in window.file.read_text() and unsaid.is_displayed() and unsaid.text == "Pas encore enregistré — aucun compte rendu n’est fait tant que ce n’est pas le cas.",
            (window.file.read_text(), unsaid.text),
        )
        browser.find_element(By.ID, "minutes-save").click()
        WebDriverWait(browser, 5).until(staleness_of(before))
        local = wait_for(browser, lambda b: "mistral:7b" in window.file.read_text(), 5)
        check(
            "a model of Ollama is saved on the word, nothing left unsaid",
            local and f'minutes_provider = "{ollama}/v1"' in window.file.read_text() and not browser.find_element(By.ID, "minutes-unsaved").is_displayed(),
            window.file.read_text(),
        )
        # This computer's Ollama with a model it does not have: said as here — nothing leaves —,
        # and the model it lacks named; never "online, the transcript's text is sent".
        saved = window.file.read_text()
        window.file.write_text(saved.replace('minutes_model = "mistral:7b"', 'minutes_model = "absent:latest"'))
        browser.execute_script("location.hash = '#/'")
        browser.execute_script("location.hash = '#/settings'")
        lacking = wait_for(browser, lambda b: b.find_elements(By.ID, "minutes-lacking"), 5)
        said = " ".join(p.text for p in browser.find_elements(By.CSS_SELECTOR, "#minutes-where, #minutes-lacking"))
        check(
            "this computer's Ollama with a model it lacks: said here, nothing leaves, the model named",
            lacking and "Sur cet ordinateur" in said and "En ligne" not in said and "absent:latest" in said,
            said,
        )
        window.file.write_text(saved)
        # The page drawn anew from the file just written: its last drawing waited out, so that the
        # old Tester, hidden, is never the one pressed (as #302 found of the agreement page).
        browser.execute_script("location.hash = '#/'")
        to_settings()
        wait_for(browser, lambda b: b.find_elements(By.ID, "minutes-test"), 5)
        # Tester asks wholeheard-summary about the choice saved: no key for Ollama here.
        press(browser, "minutes-test")
        answered = wait_for(browser, lambda b: b.find_element(By.ID, "minutes-tested").text == "Ça marche : le modèle répond.", 10)
        check(
            "Tester says the model answers, asking wholeheard-summary with the choice saved",
            answered
            and (WORK / "summary-args").read_text().split() == ["--test", "--provider", f"{ollama}/v1", "--model", "mistral:7b"]
            and (WORK / "summary-key").read_text() == "none",
            browser.find_element(By.ID, "minutes-tested").text,
        )
        Select(browser.find_element(By.ID, "minutes")).select_by_visible_text("OpenRouter")
        key = browser.find_element(By.ID, "minutes-key")
        check("a key is typed where it is not shown, and never filled in", key.get_attribute("type") == "password" and key.get_attribute("value") == "")
        browser.find_element(By.ID, "minutes-model").send_keys("anthropic/claude-sonnet")
        key.send_keys("sk-scenario")
        before = browser.find_element(By.ID, "minutes")
        browser.find_element(By.ID, "minutes-save").click()
        # Saved, the page is drawn again: what follows acts on the new one.
        WebDriverWait(browser, 5).until(staleness_of(before))
        online = wait_for(browser, lambda b: "openrouter.ai" in window.file.read_text(), 5)
        text = window.file.read_text()
        check(
            "an online service, with its address and the model named",
            online and 'minutes_provider = "https://openrouter.ai/api/v1"' in text and 'minutes_model = "anthropic/claude-sonnet"' in text,
            text,
        )
        kept = browser.find_element(By.ID, "minutes-key")
        browser.execute_script("arguments[0].scrollIntoView({block: 'center'})", kept)
        shot(browser, "4-settings-online")
        check(
            "the key is kept, said so, and never shown back nor written in the settings",
            kept.get_attribute("value") == "" and kept.get_attribute("placeholder").startswith("Une clé est gardée") and "sk-scenario" not in text,
            kept.get_attribute("placeholder"),
        )
        # Each cause its own sentence (D72).
        code = WORK / "test-code"
        try:
            for cause, sentence in TEST_CAUSES:
                code.write_text(cause)
                press(browser, "minutes-test")
                held = wait_for(browser, lambda b: b.find_element(By.ID, "minutes-tested").get_attribute("textContent") == sentence, 10)
                check(f"Tester of the minutes, {cause}: its own sentence, what to do, no raw text", held, browser.find_element(By.ID, "minutes-tested").get_attribute("textContent"))
            # In developer mode, the sentence and the program's last line, its name off.
            changed(lambda: browser.find_element(By.ID, "developer").click())
            press(browser, "minutes-test")
            raw = wait_for(browser, lambda b: "raw words of the provider" in b.find_element(By.ID, "minutes-tested").get_attribute("textContent"), 10)
            told = browser.find_element(By.ID, "minutes-tested").get_attribute("textContent")
            check("Tester in developer mode: the cause, then the raw line alone", raw and told.startswith(TEST_CAUSES[-1][1]) and "wholeheard-summary:" not in told and "earlier line" not in told, told)
            changed(lambda: browser.find_element(By.ID, "developer").click())
            text = window.file.read_text()
        finally:
            code.unlink(missing_ok=True)
        press(browser, "minutes-test")
        refused = wait_for(browser, lambda b: b.find_element(By.ID, "minutes-tested").text.startswith("Le modèle ne répond pas"), 10)
        said = browser.find_element(By.ID, "minutes-tested").text
        check(
            "a model that does not answer is said in one sentence; the key goes by the environment, never the arguments",
            refused
            and said.endswith("vérifiez son adresse, son nom et sa clé, puis réessayez.")
            and "openrouter.ai" not in said
            and (WORK / "summary-key").read_text() == "sk-scenario"
            and "sk-scenario" not in (WORK / "summary-args").read_text(),
            said,
        )
        Select(browser.find_element(By.ID, "minutes")).select_by_visible_text("Personnalisé…")
        browser.find_element(By.ID, "minutes-address").send_keys("not an address")
        browser.find_element(By.ID, "minutes-model").send_keys("m")
        before = browser.find_element(By.ID, "minutes")
        browser.find_element(By.ID, "minutes-save").click()
        # Refused, the page is drawn again too, the refusal said on the new one.
        WebDriverWait(browser, 5).until(staleness_of(before))
        said = wait_for(browser, lambda b: b.find_element(By.ID, "not-saved").is_displayed(), 5)
        check("an address that is none is said, and nothing changes", said and window.file.read_text() == text, browser.find_element(By.ID, "not-saved").text)
    broken = WORK / "broken"
    broken.mkdir()
    # No window named: a file that cannot be read opens the full one, where it is said (D35).
    window = Window(broken, "fr_FR.UTF-8", "keep_audio = 0\n", small=True)
    with window as browser:
        check("a settings file that cannot be read opens the full window, not the small one", not browser.find_elements(By.ID, "small-record") and browser.find_element(By.ID, "settings-link").is_displayed())
        browser.find_element(By.ID, "settings-link").click()
        wait_for(browser, lambda b: b.find_elements(By.CSS_SELECTOR, "#settings .warn"))
        warned = browser.find_element(By.CSS_SELECTOR, "#settings .warn").text
        locked = browser.execute_script("return [...document.querySelectorAll('#settings input, #settings select')].every(e => e.disabled)")
        check("a settings file that cannot be read is said so, and nothing can be changed", warned == "Le fichier des réglages ne peut pas être lu : Wholeheard ne l’écrit pas tant qu’il n’est pas corrigé." and locked, warned)
        check("it is left as it was", window.file.read_text() == f'sessions = "{broken}"\nwelcomed = true\nkeep_audio = 0\n')
        # The shell itself refuses, not only the page.
        refused = browser.execute_async_script(
            "const done = arguments[0]; window.__TAURI__.core.invoke('settings_saved', {change: {reminder: false}})"
            ".then(() => done('saved'), why => done('refused: ' + why))"
        )
        check("the shell refuses a change while the file cannot be read", refused.startswith("refused"), refused)
        # Corrected by hand, the page opens again when the window comes forward.
        window.file.write_text(f'sessions = "{broken}"\nkeep_audio = 7\n')
        browser.execute_script("window.dispatchEvent(new Event('focus'))")
        open_again = wait_for(browser, lambda b: not b.find_element(By.ID, "reminder").get_attribute("disabled"), 5)
        check("a file corrected by hand opens the page again", open_again and not browser.find_elements(By.CSS_SELECTOR, "#settings .warn:not([hidden])")[1:],
              browser.execute_script("return [document.getElementById('reminder')?.disabled, [...document.querySelectorAll('#settings .warn:not([hidden])')].map(e => e.textContent), [...document.querySelectorAll('#settings:not([hidden]) .fields input')].filter(i => i.value !== i.defaultValue).map(i => i.id)]"))

    print("N. before a meeting first leaves for a host: what leaves, to whom; sent only on the word")
    agreeing = WORK / "agreeing"
    asked = agreeing / "2026-10-18_11-00"
    asked.mkdir(parents=True)
    (asked / "session.json").write_text('{"contract":"session-audio/v2","started":"2026-10-18T11:00:00+02:00","stopped":"2026-10-18T11:20:00+02:00","tracks":{}}')
    (asked / "transcript.jsonl").write_text(
        '{"type":"header","contract":"transcript/v1","engine":"whisper.cpp","model":"m","language":null,"context":null}\n'
        '{"type":"segment","speaker":"others","track":"remote","start":0.0,"end":2.0,"text":"Le budget est validé."}\n'
        '{"type":"segment","speaker":"me","track":"local","start":2.0,"end":4.0,"text":"Je préviens Alexandre."}\n'
        '{"type":"complete","duration":1200.0}\n'
    )
    (asked / "context.txt").write_text("Réunion budget, trois personnes.\n")
    # A second meeting, its minutes asked while the first one's are written (D25).
    other = agreeing / "2026-10-18_14-00"
    other.mkdir()
    for name in ("session.json", "transcript.jsonl"):
        (other / name).write_text((asked / name).read_text().replace("T11:", "T14:"))
    (WORK / "summary-args").unlink(missing_ok=True)
    online = 'minutes_provider = "https://openrouter.ai/api/v1"\nminutes_model = "a-model"\n'
    window = Window(agreeing, "fr_FR.UTF-8", online, ollama=ollama)
    with window as browser:
        # To the agreement page: its last drawing stays in the hidden page until the new one
        # replaces it — waited for, so that nothing is clicked on the old one (as #302 found).
        def to_agree(ready):
            # Read while the page replaces it — after `#/` — the drawing cannot be held: read
            # again, as `press` does (#309).
            for _ in range(10):
                try:
                    old = browser.find_elements(By.CSS_SELECTOR, "#agree > *")
                    break
                except StaleElementReferenceException:
                    time.sleep(0.1)
            else:
                old = browser.find_elements(By.CSS_SELECTOR, "#agree > *")
            browser.execute_script("location.hash = '#/agree/2026-10-18_11-00'")
            if old:
                WebDriverWait(browser, 5).until(staleness_of(old[0]))
            wait_for(browser, lambda b: b.find_elements(By.ID, ready))

        to_agree("agree-send")
        shot(browser, "agreement")
        page = browser.find_element(By.ID, "agree").text
        check(
            "what leaves is said — the transcript, its length and words, the context —, what stays, to whom",
            "Le texte de cette réunion va quitter votre ordinateur" in page
            and "20 minutes, environ 7 mots" in page and "Environ 4 mots" in page
            and "Reste ici : l’audio" in page and "openrouter.ai" in page and "a-model" in page,
            page,
        )
        check("nothing has left before the word", not (WORK / "summary-args").exists())
        check("not asking again is the user's tick, unticked", not browser.find_element(By.ID, "agree-remember").is_selected())
        # The provider changed in the file while the page asks: the word was not for that one.
        window.file.write_text(window.file.read_text().replace("https://openrouter.ai/api/v1", "https://api.mistral.ai/v1"))
        press(browser, "agree-send")
        refused = wait_for(browser, lambda b: any(said.is_displayed() for said in b.find_elements(By.ID, "not-saved")), 5)
        check(
            "the word is for the host shown: chosen otherwise meanwhile, nothing is sent",
            refused and not (WORK / "summary-args").exists() and "minutes_agreed" not in window.file.read_text(),
            [said.text for said in browser.find_elements(By.ID, "not-saved")],
        )
        window.file.write_text(window.file.read_text().replace("https://api.mistral.ai/v1", "https://openrouter.ai/api/v1"))
        browser.execute_script("location.hash = '#/'")
        to_agree("agree-send")
        press(browser, "agree-send")
        sent = wait_for(browser, lambda b: (asked / "minutes.md").exists(), 10)
        check(
            "on the word, this meeting is sent — once: the host is not kept",
            sent and "minutes_agreed" not in window.file.read_text()
            and (WORK / "summary-args").read_text().split() == [str(asked), "--provider", "https://openrouter.ai/api/v1", "--model", "a-model"],
            window.file.read_text(),
        )
        check("then the meeting is shown", wait_for(browser, lambda b: b.execute_script("return location.hash") == "#/meeting/2026-10-18_11-00", 5))
        # Ticked: this meeting and the next ones go to that service without asking.
        (asked / "minutes.md").unlink()
        to_agree("agree-remember")
        press(browser, "agree-remember")
        press(browser, "agree-send")
        kept = wait_for(browser, lambda b: (asked / "minutes.md").exists(), 10)
        check("ticked, the host is kept for the next ones", kept and 'minutes_agreed = "openrouter.ai"' in window.file.read_text(), window.file.read_text())
        # On this computer instead: nothing goes to the host, nothing is agreed.
        (asked / "minutes.md").unlink()
        (WORK / "summary-args").unlink()
        window.file.write_text(window.file.read_text().replace('minutes_agreed = "openrouter.ai"\n', ""))
        to_agree("agree-local-open")
        browser.find_element(By.ID, "agree-local-open").click()
        Select(browser.find_element(By.ID, "agree-local")).select_by_visible_text("qwen3:14b")
        browser.find_element(By.ID, "agree-local-write").click()
        here = wait_for(browser, lambda b: (asked / "minutes.md").exists(), 10)
        text = window.file.read_text()
        check(
            "or written on this computer, chosen for the next ones too, nothing agreed",
            here and f'minutes_provider = "{ollama}/v1"' in text and 'minutes_model = "qwen3:14b"' in text
            and "minutes_agreed" not in text
            and (WORK / "summary-args").read_text().split()[1:] == ["--provider", f"{ollama}/v1", "--model", "qwen3:14b"],
            text,
        )
        # A meeting without its minutes offers them on its page, made on the word — once the ones
        # before are ended: while they are made, it is not offered.
        idle = lambda b: b.execute_async_script("window.__TAURI__.core.invoke('makings').then(m => arguments[arguments.length - 1](m.minutes === null))")
        wait_for(browser, idle, 10)
        (asked / "minutes.md").unlink()
        browser.execute_script("location.hash = '#/'")
        browser.execute_script("location.hash = '#/meeting/2026-10-18_11-00'")
        # Offered once what was being made for it is read as ended.
        wait_for(browser, lambda b: b.find_elements(By.ID, "make-minutes") and b.find_element(By.ID, "make-minutes").is_displayed())
        chips = browser.find_element(By.CSS_SELECTOR, "#meeting .holds").text
        check("the meeting's page says where its transcript was made", "Transcription · sur cet ordinateur" in chips, chips)
        (WORK / "slow-minutes").touch()
        browser.find_element(By.ID, "make-minutes").click()
        state = lambda b: b.find_element(By.ID, "minutes-state").text if b.find_elements(By.ID, "minutes-state") else ""
        shown = wait_for(browser, lambda b: "Rédaction du compte rendu" in state(b), 3)
        said = state(browser)
        check("while they are written, the page says so — what, by which model, for how long", shown and "Qwen3 14b" in said and "0:0" in said, said)
        # Asked meanwhile for another meeting: it waits its turn, said on its page and in the list.
        browser.execute_script("location.hash = '#/meeting/2026-10-18_14-00'")
        wait_for(browser, lambda b: b.find_elements(By.ID, "make-minutes"))
        browser.find_element(By.ID, "make-minutes").click()
        waits = wait_for(browser, lambda b: "En file d’attente · 1 avant" in state(b), 3)
        check("one asked while another is written waits its turn, said on its page", waits, state(browser))
        browser.execute_script("location.hash = '#/'")
        wait_for(browser, lambda b: len([c for c in b.find_elements(By.CSS_SELECTOR, "#groups .chip.making") if c.is_displayed()]) == 2, 3)
        rowed = sorted(c.text for c in browser.find_elements(By.CSS_SELECTOR, "#groups .chip.making") if c.is_displayed())
        check("the list says it too, on each meeting's row", rowed == ["Compte rendu…", "En attente"], rowed)
        # While its minutes are written, the meeting is busy: shown, never offered to be freed.
        browser.execute_script("location.hash = '#/free'")
        held = wait_for(browser, lambda b: [r.text for r in b.find_elements(By.CSS_SELECTOR, "#free .freeable.busy")], 3)
        busy = [r.text for r in browser.find_elements(By.CSS_SELECTOR, "#free .freeable.busy")]
        check("while its minutes are written, the meeting is busy on the page that frees space", held and "compte rendu" in " ".join(busy), busy)
        # Elsewhere when they are written — the settings, being changed —: that page is left alone.
        browser.execute_script("location.hash = '#/settings'")
        wait_for(browser, lambda b: b.find_elements(By.ID, "minutes"), 5)
        browser.execute_script("window.__kept = document.getElementById('settings').firstElementChild")
        made = wait_for(browser, lambda b: (asked / "minutes.md").exists(), 10)
        check("what a meeting lacks is offered, and made on the word", made)
        time.sleep(2)
        check("written while another page is shown, that page is not drawn again under the user",
              browser.execute_script("return document.getElementById('settings').firstElementChild === window.__kept && location.hash === '#/settings'"))
        # The one waiting, now being written, shown: written, its page shows them by itself.
        browser.execute_script("location.hash = '#/meeting/2026-10-18_14-00'")
        turn = wait_for(browser, lambda b: (other / "minutes.md").exists(), 10)
        (WORK / "slow-minutes").unlink()
        check("then the one waiting is made in its turn, never dropped", turn)
        drawn = wait_for(browser, lambda b: b.find_elements(By.ID, "redo-minutes") and not b.find_element(By.ID, "minutes-state").is_displayed(), 5)
        check("written, the page shows them by itself, the state gone", drawn,
              (state(browser), bool(browser.find_elements(By.ID, "redo-minutes")), browser.execute_async_script("window.__TAURI__.core.invoke('makings').then(arguments[arguments.length - 1])")))
        # The line never waits on an agreement: set aside, said on its page, the next one made.
        invoke = lambda command, args: browser.execute_script("window.__TAURI__.core.invoke(arguments[0], arguments[1])", command, args)
        makings = lambda: browser.execute_async_script("window.__TAURI__.core.invoke('makings').then(arguments[arguments.length - 1])")
        settled = window.file.read_text()
        unchosen = "\n".join(l for l in settled.splitlines() if not l.startswith(("minutes_provider", "minutes_model", "minutes_agreed"))) + "\n"
        (WORK / "slow-minutes").touch()
        invoke("minutes_redo", {"name": "2026-10-18_11-00", "provider": None, "model": None})
        wait_for(browser, lambda b: (makings()["minutes"] or {}).get("name") == "2026-10-18_11-00", 3)
        invoke("minutes_asked", {"name": "2026-10-18_11-00"})
        time.sleep(1)
        check("asked again for the one being written, without a Refaire…: not made a second time", makings()["minutes_waiting"] == [], makings())
        window.file.write_text(unchosen + 'minutes_provider = "https://openrouter.ai/api/v1"\nminutes_model = "a-model"\n')
        invoke("minutes_redo", {"name": "2026-10-18_11-00", "provider": None, "model": None})
        line = wait_for(browser, lambda b: makings()["minutes_waiting"] == ["2026-10-18_11-00"], 3)
        check("a Refaire… with the settings' model while being written: it waits its turn, never dropped", line, makings())
        invoke("minutes_redo", {"name": "2026-10-18_14-00", "provider": f"{ollama}/v1", "model": "mistral:7b"})
        browser.execute_script("location.hash = '#/meeting/2026-10-18_11-00'")
        (WORK / "slow-minutes").unlink()
        passed = wait_for(browser, lambda b: makings()["minutes"] is None and not makings()["minutes_waiting"], 15)
        aside_ = wait_for(browser, lambda b: "En attente de votre accord pour l’envoyer à openrouter.ai" in state(b) and b.find_elements(By.ID, "minutes-agree"), 5)
        check(
            "one needing the user's agreement is set aside, said on its page with the way to give it; the next one made",
            passed and aside_ and (WORK / "summary-args").read_text().split()[-1] == "mistral:7b"
            and browser.execute_script("return location.hash") == "#/meeting/2026-10-18_11-00",
            (makings(), state(browser), (WORK / "summary-args").read_text()),
        )
        browser.execute_script("location.hash = '#/'")
        chip_ = wait_for(browser, lambda b: "Accord requis" in [c.text for c in b.find_elements(By.CSS_SELECTOR, "#groups .chip.making") if c.is_displayed()], 3)
        check("the list says it, on the meeting's row", chip_)
        browser.execute_script("location.hash = '#/meeting/2026-10-18_11-00'")
        wait_for(browser, lambda b: b.find_elements(By.ID, "minutes-agree"), 5)
        press(browser, "minutes-agree")
        wait_for(browser, lambda b: b.find_elements(By.ID, "agree-not"))
        press(browser, "agree-not")
        kept_ = wait_for(browser, lambda b: makings()["awaiting"] == {"2026-10-18_11-00": "openrouter.ai"}, 3)
        check("not now: still waiting for the word, nothing sent", kept_ and (WORK / "summary-args").read_text().split()[-1] == "mistral:7b", makings())
        # The settings changed to this computer meanwhile: looked at again, made, no word needed.
        invoke("minutes_saved", {"provider": f"{ollama}/v1", "model": "qwen3:14b", "key": None})
        again_ = wait_for(browser, lambda b: (WORK / "summary-args").read_text().split()[-1] == "qwen3:14b" and not makings()["awaiting"] and makings()["minutes"] is None, 10)
        check("set aside, then the settings changed: looked at again and made", again_, (makings(), (WORK / "summary-args").read_text()))
        # Taken from the line with no one chosen any more: said on its page, no window over the user.
        browser.execute_script("location.hash = '#/meeting/2026-10-18_11-00'")
        (WORK / "slow-minutes").touch()
        invoke("minutes_redo", {"name": "2026-10-18_14-00", "provider": f"{ollama}/v1", "model": "mistral:7b"})
        wait_for(browser, lambda b: makings()["minutes"] is not None, 3)
        window.file.write_text(unchosen)
        invoke("minutes_asked", {"name": "2026-10-18_11-00"})
        wait_for(browser, lambda b: makings()["minutes_waiting"] == ["2026-10-18_11-00"], 3)
        (WORK / "slow-minutes").unlink()
        unsaid = wait_for(browser, lambda b: "2026-10-18_11-00" in makings()["failed"], 15)
        time.sleep(1)
        check("taken from the line with no one chosen: said on its page, no settings window opened over the user",
              unsaid and browser.execute_script("return location.hash") == "#/meeting/2026-10-18_11-00", (makings(), browser.execute_script("return location.hash")))
        window.file.write_text(settled)
        # The trash emptied of these, as the user would: the next check counts its own.
        for made_before in (WORK / "data" / "Trash" / "files").glob("minutes-before-*.md"):
            made_before.unlink()
            (WORK / "data" / "Trash" / "info" / f"{made_before.name}.trashinfo").unlink(missing_ok=True)
        # Refaire…: another model, this time only; the minutes there go to the trash once replaced.
        browser.execute_script("location.hash = '#/'")
        browser.execute_script("location.hash = '#/meeting/2026-10-18_11-00'")
        wait_for(browser, lambda b: b.find_elements(By.ID, "redo-minutes"))
        press(browser, "redo-minutes")
        wait_for(browser, lambda b: b.find_elements(By.ID, "redo-do"))
        browser.execute_script("document.getElementById('redo-panel').scrollIntoView({block: 'center'})")
        shot(browser, "6-redo")
        offered = browser.execute_script("return [...document.querySelectorAll('#redo-panel .choice')].map(c => c.textContent)")
        check(
            "Refaire offers the model chosen first, then the others on this computer",
            offered == ["Qwen3 14bsur cet ordinateur", "Mistral 7bsur cet ordinateur"],
            offered,
        )
        before = (asked / "minutes.md").read_text()
        panel_choice(browser, "#redo-panel input[name=redo-with]", 1).click()
        browser.find_element(By.ID, "redo-do").click()
        def aside(folder):
            return sorted(folder.glob("minutes-before-*.md")) if folder.exists() else []

        trash_files = WORK / "data" / "Trash" / "files"
        redone = wait_for(browser, lambda b: aside(trash_files), 10)
        check(
            "made again with the model chosen, this time only; the minutes there in the trash",
            redone
            and (WORK / "summary-args").read_text().split()[-1] == "mistral:7b"
            and aside(trash_files)[0].read_text() == before
            and 'minutes_model = "qwen3:14b"' in window.file.read_text()
            and not aside(asked),
            ((WORK / "summary-args").read_text(), aside(trash_files), aside(trash_files) and aside(trash_files)[0].read_text()[:60], before[:60], aside(asked)),
        )
        # Made again and failing — on this computer, then online through the agreement: the minutes
        # there stay, and the notice says a redo, not a first making.
        (WORK / "fail-minutes").touch()
        heard = WORK / "notified-redo"
        monitor = subprocess.Popen(
            ["dbus-monitor", "--session", "interface='org.freedesktop.Notifications',member='Notify'"],
            stdout=open(heard, "w"), stderr=subprocess.DEVNULL,
        )
        time.sleep(0.5)
        before = (asked / "minutes.md").read_text()
        browser.execute_script("location.hash = '#/'")
        browser.execute_script("location.hash = '#/meeting/2026-10-18_11-00'")
        wait_for(browser, lambda b: b.find_elements(By.ID, "redo-minutes"))
        press(browser, "redo-minutes")
        wait_for(browser, lambda b: b.find_elements(By.ID, "redo-do"))
        panel_choice(browser, "#redo-panel input[name=redo-with]", 1).click()
        browser.find_element(By.ID, "redo-do").click()
        told = wait_for(browser, lambda b: "pas été refait" in heard.read_text(), 10)
        check("minutes made again that fail: the ones there stay, and the notice says a redo", told and (asked / "minutes.md").read_text() == before, heard.read_text()[-400:])
        failed = wait_for(browser, lambda b: "n’a pas été fait" in state(b) and b.find_elements(By.ID, "minutes-again"), 5)
        check("the failure said on the meeting's page too, in plain words, with a way to try again — never what the program said",
              failed and "not minutes" not in state(browser) and "not minutes" not in heard.read_text(), (state(browser), heard.read_text()[-400:]))
        # Its cause named by the program, its gesture said, on the page and in the notice (D29).
        check("the cause the program named is said with its gesture, on the page and in the notice",
              "écrit de compte rendu" in state(browser) and "écrit de compte rendu" in heard.read_text(),
              (state(browser), heard.read_text()[-400:]))
        # The developer mode shows what the program said, under the sentence; never in a notice (D29).
        to_settings()
        press(browser, "developer")
        wait_for(browser, lambda b: "developer = true" in window.file.read_text(), 5)
        browser.execute_script("location.hash = '#/meeting/2026-10-18_11-00'")
        detailed = wait_for(browser, lambda b: "the provider's answer is not minutes" in state(b) and "n’a pas été fait" in state(b), 5)
        check("in developer mode, what the program said is under the sentence", detailed, state(browser))
        # And the meeting's diagnostic: its makings' lines, every program's — nothing of a path of the run.
        offered = wait_for(browser, lambda b: b.find_elements(By.ID, "copy-diagnostic"), 5)
        lines = browser.execute_async_script(
            "window.__TAURI__.core.invoke('diagnostic', {name: '2026-10-18_11-00'}).then(arguments[arguments.length - 1])"
        )
        check("in developer mode, the meeting offers its diagnostic: its failed minutes, no path of the run",
              offered and '"shell.minutes_failed"' in lines and str(WORK) not in lines, lines[-600:])
        check("and each track's level, after the logs: one line a track, numbers only",
              lines.count('"type":"levels"') == 2 and lines.rstrip().endswith("}") and '"track":"remote"' in lines.splitlines()[-1], lines[-400:])
        to_settings()
        press(browser, "developer")
        wait_for(browser, lambda b: "developer = false" in window.file.read_text(), 5)
        settled = window.file.read_text()
        window.file.write_text(
            "\n".join(line for line in settled.splitlines() if not line.startswith(("minutes_provider", "minutes_model", "minutes_agreed")))
            + '\nminutes_provider = "https://openrouter.ai/api/v1"\nminutes_model = "a-model"\n'
        )
        # This time a failure no code names: the earlier cause is not said again (D29).
        (WORK / "fail-minutes").unlink()
        (WORK / "fail-plainly").touch()
        browser.execute_script("location.hash = '#/'")
        browser.execute_script("location.hash = '#/meeting/2026-10-18_11-00'")
        wait_for(browser, lambda b: b.find_elements(By.ID, "redo-minutes"))
        press(browser, "redo-minutes")
        panel_choice(browser, "#redo-panel input[name=redo-with]", 0).click()
        time.sleep(1)
        since = len(heard.read_text())
        browser.find_element(By.ID, "redo-do").click()
        wait_for(browser, lambda b: b.find_elements(By.ID, "agree-send"))
        press(browser, "agree-send")
        # Only what is said after this gesture: a notice may show its title more than once.
        told = wait_for(browser, lambda b: "pas été refait" in heard.read_text()[since:], 10)
        said = heard.read_text()[since:]
        monitor.terminate()
        (WORK / "fail-plainly").unlink()
        check("asked through the agreement, still a redo: its failure said as one", told and "pas été fait" not in said and (asked / "minutes.md").read_text() == before, said[-400:])
        browser.execute_script("location.hash = '#/'")
        browser.execute_script("location.hash = '#/meeting/2026-10-18_11-00'")
        plain = wait_for(browser, lambda b: "n’a pas été fait" in state(b), 5)
        check("a later failure that names no cause does not show the earlier one's",
              plain and "écrit de compte rendu" not in state(browser) and "écrit de compte rendu" not in said, (state(browser), said[-300:]))
        time.sleep(3)
        left = sorted(p.name for p in asked.glob("minutes*"))
        check("nothing else is made after, no copy left beside", (asked / "minutes.md").read_text() == before and left == ["minutes.md"], (left, (asked / "minutes.md").read_text()[:80], (WORK / "summary-args").read_text()))
        window.file.write_text(settled)
        # A trash that refuses: the minutes replaced stay beside the new ones, never deleted.
        trash = WORK / "data" / "Trash"
        trash.rename(WORK / "data" / "Trash-aside")
        trash.write_text("")
        before = (asked / "minutes.md").read_text()
        browser.execute_script("location.hash = '#/'")
        browser.execute_script("location.hash = '#/meeting/2026-10-18_11-00'")
        wait_for(browser, lambda b: b.find_elements(By.ID, "redo-minutes"))
        press(browser, "redo-minutes")
        wait_for(browser, lambda b: b.find_elements(By.ID, "redo-do"))
        browser.find_element(By.ID, "redo-do").click()
        # The new minutes written — they name their model —, then a moment for what follows them.
        written = wait_for(browser, lambda b: "model: qwen3:14b" in (asked / "minutes.md").read_text(), 10)
        time.sleep(2)
        kept = [path.read_text() for path in aside(asked)]
        check("a trash that refuses keeps the minutes replaced beside the new ones", written and kept == [before], kept)
        # Made again while the trash still refuses: the copy kept is never written over.
        second = (asked / "minutes.md").read_text()
        browser.execute_script("location.hash = '#/'")
        browser.execute_script("location.hash = '#/meeting/2026-10-18_11-00'")
        wait_for(browser, lambda b: b.find_elements(By.ID, "redo-minutes"))
        press(browser, "redo-minutes")
        wait_for(browser, lambda b: b.find_elements(By.ID, "redo-do"))
        panel_choice(browser, "#redo-panel input[name=redo-with]", 1).click()
        browser.find_element(By.ID, "redo-do").click()
        written = wait_for(browser, lambda b: "model: mistral:7b" in (asked / "minutes.md").read_text(), 10)
        time.sleep(2)
        kept = sorted(path.read_text() for path in aside(asked))
        check("made again, the copies kept are all there, none written over", written and kept == sorted([before, second]), kept)
        for path in aside(asked):
            path.unlink()
        # Minutes that cannot be set aside are not made over: nothing runs, they stay as they are.
        (asked / "minutes.md").chmod(0o000)
        (WORK / "summary-args").unlink()
        browser.execute_script("location.hash = '#/'")
        browser.execute_script("location.hash = '#/meeting/2026-10-18_11-00'")
        wait_for(browser, lambda b: b.find_elements(By.ID, "redo-minutes") or b.find_elements(By.ID, "make-minutes"))
        browser.execute_script("window.__TAURI__.core.invoke('minutes_redo', {name: '2026-10-18_11-00', provider: null, model: null})")
        time.sleep(3)
        (asked / "minutes.md").chmod(0o644)
        check(
            "minutes that cannot be set aside are not made over",
            not (WORK / "summary-args").exists() and "model: mistral:7b" in (asked / "minutes.md").read_text() and not aside(asked),
            (WORK / "summary-args").exists(),
        )
        trash.unlink()
        (WORK / "data" / "Trash-aside").rename(trash)
        # Refaire… on the transcript: another model of this computer; the transcript there in the trash.
        heard = (asked / "transcript.jsonl").read_text()
        browser.execute_script("location.hash = '#/'")
        browser.execute_script("location.hash = '#/meeting/2026-10-18_11-00'")
        wait_for(browser, lambda b: b.find_elements(By.ID, "redo-transcript"))
        press(browser, "redo-transcript")
        wait_for(browser, lambda b: b.find_elements(By.ID, "redo-transcript-do"))
        offered = browser.execute_script("return [...document.querySelectorAll('#redo-transcript-panel .choice .name')].map(c => c.textContent)")
        check("Refaire offers the Whisper models of this computer, the one chosen first", offered == ["Whisper large v3 turbo", "Whisper small"], offered)
        panel_choice(browser, "#redo-transcript-panel input", 1).click()
        browser.find_element(By.ID, "redo-transcript-do").click()
        redone = wait_for(browser, lambda b: sorted(trash_files.glob("transcript-before-*.jsonl")), 10)
        trashed = sorted(trash_files.glob("transcript-before-*.jsonl"))
        check(
            "the transcript made again with the model picked; the one there in the trash",
            redone
            and "Refait avec ggml-small." in (asked / "transcript.jsonl").read_text()
            and [path.read_text() for path in trashed] == [heard]
            and not sorted(asked.glob("transcript-before-*")),
            (asked / "transcript.jsonl").read_text(),
        )
        # A new transcript that fails: the one there comes back, nothing left aside.
        heard = (asked / "transcript.jsonl").read_text()
        (WORK / "fail-hearing").write_text("")
        browser.execute_script("location.hash = '#/'")
        browser.execute_script("location.hash = '#/meeting/2026-10-18_11-00'")
        wait_for(browser, lambda b: b.find_elements(By.ID, "redo-transcript"))
        press(browser, "redo-transcript")
        wait_for(browser, lambda b: b.find_elements(By.ID, "redo-transcript-do"))
        browser.find_element(By.ID, "redo-transcript-do").click()
        time.sleep(3)
        (WORK / "fail-hearing").unlink()
        check(
            "a new transcript that fails gives the one there back, nothing left aside",
            (asked / "transcript.jsonl").read_text() == heard and not sorted(asked.glob("transcript-before-*")),
            (asked / "transcript.jsonl").read_text(),
        )
        heard_state = lambda b: b.find_element(By.ID, "transcript-state").text if b.find_elements(By.ID, "transcript-state") else ""
        unmade = wait_for(browser, lambda b: "La transcription n’a pas été faite" in heard_state(b) and b.find_elements(By.ID, "transcript-again"), 5)
        shot(browser, "9-transcript-failed")
        check("what is not known of who wrote the minutes is left unsaid (#312)",
              "— · —" not in browser.find_element(By.ID, "meeting").text and "via —" not in browser.find_element(By.ID, "meeting").text,
              [p.text for p in browser.find_elements(By.CSS_SELECTOR, "#meeting .maker")])
        check("a transcript not made says so on its card in plain words, with a way to try again", unmade and "engine stopped" not in heard_state(browser), heard_state(browser))
        # Tried again as it was asked: made again with the model the Refaire… picked, not resumed.
        # Made again — the one there set aside, then in the trash —, never resumed from it.
        trashed_before = len(sorted(trash_files.glob("transcript-before-*.jsonl")))
        press(browser, "transcript-again")
        tried = wait_for(browser, lambda b: len(sorted(trash_files.glob("transcript-before-*.jsonl"))) == trashed_before + 1, 10)
        check("Try again after a Refaire… makes it again with the model it picked", tried and "Refait avec ggml-large" in (asked / "transcript.jsonl").read_text(), (asked / "transcript.jsonl").read_text() if (asked / "transcript.jsonl").exists() else "")
        back_ = wait_for(browser, lambda b: b.find_elements(By.ID, "redo-transcript"), 5)
        check("Refaire… is offered again once asked, the panel gone", back_)
        # Being made: said on its card, by which model, for how long; a Refaire… meanwhile waits its
        # turn — the one being made too —, then is made, and the card shows it by itself.
        (WORK / "slow-hearing").touch()
        browser.execute_script("location.hash = '#/'")
        browser.execute_script("location.hash = '#/meeting/2026-10-18_11-00'")
        wait_for(browser, lambda b: b.find_elements(By.ID, "redo-transcript"))
        press(browser, "redo-transcript")
        wait_for(browser, lambda b: b.find_elements(By.ID, "redo-transcript-do"))
        paths = [i.get_attribute("value") for i in browser.find_elements(By.CSS_SELECTOR, "#redo-transcript-panel input")]
        panel_choice(browser, "#redo-transcript-panel input", 1).click()
        browser.find_element(By.ID, "redo-transcript-do").click()
        making_ = wait_for(browser, lambda b: "Transcription en cours" in heard_state(b), 3)
        shot(browser, "7-transcript-making")
        check("while it is made, its card says so — what, by which model, for how long",
              making_ and "Whisper small · 0:0" in heard_state(browser) and not browser.find_elements(By.ID, "transcript-again"), heard_state(browser))
        browser.execute_script("window.__TAURI__.core.invoke('transcript_redo', {name: '2026-10-18_11-00', model: arguments[0]})", paths[0])
        lined = lambda b: b.execute_async_script("window.__TAURI__.core.invoke('makings').then(m => arguments[arguments.length - 1](m.transcripts_waiting))")
        queued = wait_for(browser, lambda b: lined(b) == ["2026-10-18_11-00"], 3)
        check("a Refaire… while one is made waits its turn, never refused", queued, lined(browser))
        (WORK / "slow-hearing").unlink()
        said_ = lambda: (asked / "transcript.jsonl").read_text() if (asked / "transcript.jsonl").exists() else ""
        turned = wait_for(browser, lambda b: "Refait avec ggml-large" in said_() and "Refait avec ggml-large" in b.find_element(By.ID, "meeting").text and not b.find_element(By.ID, "transcript-state").is_displayed(), 15)
        check("then made in its turn, and the card shows it by itself", turned, (said_(), heard_state(browser)))
    tags.shutdown()

    print("P. freeing space: the audio alone or the whole meeting, the heaviest first, to the trash")
    roomy = WORK / "roomy"
    now = dt.datetime.now().astimezone()
    session(roomy / "light", now - dt.timedelta(days=40), 10, {"local.wav": 1_000_000, "transcript.jsonl": 100})
    session(roomy / "heavy", now - dt.timedelta(days=2), 60, {"local.wav": 5_000_000, "derived/local.wav": 1_000_000, "transcript.jsonl": 100})
    session(roomy / "old", now - dt.timedelta(days=60), 30, {"local.wav": 3_000_000, "minutes.md": 50})
    with Window(roomy, "fr_FR.UTF-8") as browser:
        offered = browser.find_element(By.ID, "free-link").text
        check("the weights offer to free space", offered == "Faire de la place", offered)
        browser.find_element(By.ID, "free-link").click()
        wait_for(browser, lambda b: b.find_elements(By.ID, "free-do"))
        shot(browser, "3-free")
        weighs = [w.text for w in browser.find_elements(By.CSS_SELECTOR, "#free .weighs")]
        check("the heaviest first, by their audio", weighs == ["6 Mo", "3 Mo", "1 Mo"], weighs)
        check("nothing is freed before a choice", not browser.find_element(By.ID, "free-do").is_enabled())
        browser.find_element(By.ID, "free-older").click()
        check("the page drawn again, the focus stays where the keyboard left it", browser.execute_script("return document.activeElement.id") == "free-older")
        browser.find_element(By.CSS_SELECTOR, "#free .freeable input[value=heavy]").click()
        browser.find_element(By.CSS_SELECTOR, "#free .freeable input[value=heavy]").click()
        check("on a meeting's box too", browser.execute_script("return document.activeElement.value") == "heavy", browser.execute_script("return document.activeElement.outerHTML"))
        ticked = browser.execute_script("return [...document.querySelectorAll('#free .freeable input:checked')].map(i => i.value)")
        check("those older than 30 days are ticked at once", sorted(ticked) == ["light", "old"], ticked)
        said = browser.find_element(By.CSS_SELECTOR, "#free .free-total").text
        check("what will be freed is said before anything is done", "2 réunions" in said and "corbeille" in said, said)
        browser.find_element(By.ID, "free-do").click()
        gone = wait_for(browser, lambda b: not (roomy / "old" / "local.wav").exists() and not (roomy / "light" / "local.wav").exists(), 10)
        trash = WORK / "data" / "Trash" / "files"
        check(
            "their audio alone goes to the trash; transcript and minutes stay",
            gone and (roomy / "light" / "transcript.jsonl").exists() and (roomy / "old" / "minutes.md").exists()
            and (roomy / "heavy" / "local.wav").exists() and len(list(trash.glob("local.wav*"))) >= 2,
            sorted(p.name for p in trash.glob("*")),
        )
        # The whole meeting: its folder, to the trash.
        # The page is drawn again after each gesture: each is found afresh.
        time.sleep(1)
        browser.execute_script("document.querySelector('#free input[name=free-what][value=true]').click()")
        wait_for(browser, lambda b: b.find_elements(By.CSS_SELECTOR, "#free .freeable input[value=heavy]"))
        browser.execute_script("document.querySelector('#free .freeable input[value=heavy]').click()")
        button = browser.find_element(By.ID, "free-do")
        check("the whole meeting is said as such", button.text == "Mettre ces réunions à la corbeille", button.text)
        button.click()
        whole = wait_for(browser, lambda b: not (roomy / "heavy").exists(), 10)
        check("the whole meeting's folder goes to the trash", whole and (trash / "heavy").is_dir(), sorted(p.name for p in trash.glob("*")))
        browser.execute_script("location.hash = '#/'")
        listed = wait_for(browser, lambda b: len(b.find_elements(By.CSS_SELECTOR, "#groups .session")) == 2, 5)
        check("a meeting whose audio was freed is still listed; the whole one is not", listed)

    print("Q. the small window (D35): opened first, at rest; recording in it; back to rest at the stop; one window at a time")
    taping = WORK / "taping"
    taping.mkdir()
    with (window := Window(taping, "fr_FR.UTF-8", 'after = "nothing"\nreminder = false\n', small=True)) as browser:
        small = browser.current_window_handle
        start = browser.find_element(By.ID, "small-record")
        check("opened from the menu the first time, the small window, at rest, alone", start.text == "Démarrer un enregistrement" and browser.window_handles == [small], (start.text, browser.window_handles))
        check("the recording's gesture has the focus", browser.execute_script("return document.activeElement.id") == "small-record")
        reached = []
        for _ in range(4):
            browser.switch_to.active_element.send_keys(Keys.TAB)
            reached.append(browser.execute_script("return document.activeElement.id"))
        check("Tab reaches each of its controls, then comes back round", "small-full" in reached and "small-record" in reached, reached)
        check("no meeting yet: said, the recording offered alone", browser.find_element(By.CSS_SELECTOR, "#small .none").text == "Aucune réunion pour l’instant." and not browser.find_elements(By.CSS_SELECTOR, "#small .last"))
        hint = browser.find_element(By.ID, "small-keys").text
        # The desktop's grant is the desktop's: the hint says the keys, or that none were granted and where to choose.
        check("the shortcut is said under the gesture, as the desktop granted it — or where to choose one", hint.endswith("de n’importe où") or "Réglages" in hint, hint)
        fits = browser.execute_script("const e = document.scrollingElement; return [e.scrollHeight, e.clientHeight];")
        check("at rest it holds in its window, nothing to scroll", fits[0] <= fits[1], fits)
        shot(browser, "6-small-rest")
        start.click()
        wait_for(browser, lambda b: b.find_elements(By.ID, "small-stop"))
        time.sleep(1.5)
        check("Démarrer un enregistrement records in the same window", browser.window_handles == [small], browser.window_handles)
        tall = browser.execute_script("return window.innerHeight")
        shot(browser, "6-small")
        fits = browser.execute_script("const e = document.scrollingElement; return [e.scrollHeight, e.clientHeight];")
        check("it holds in its window, nothing to scroll", fits[0] <= fits[1], fits)
        moving = {browser.find_element(By.ID, "level-me").get_attribute("style") for _ in range(6) if time.sleep(0.15) is None}
        clock = browser.find_element(By.CSS_SELECTOR, "#small .time").text
        check("it shows the time recorded and the levels, moving as sound comes", clock.startswith("00:00:0") and clock != "00:00:00" and len(moving) > 1, (clock, moving))
        check("the meeting's level too", "50%" in browser.find_element(By.ID, "level-others").get_attribute("style"))
        # What each side is recorded from, named under its level, and followed (D33).
        # Read from the page: WebKitWebDriver leaves this line out of an element's text, though drawn (shot 6-small-devices).
        named = lambda b: tuple(b.execute_script("return ['device-me', 'device-others'].map(id => document.getElementById(id).textContent)"))
        check("it names the microphone and the output recorded", named(browser) == ("Mic", "Speaker"), named(browser))
        (taping / "move").touch()
        moved = wait_for(browser, lambda b: named(b) == ("AirPods Pro", "Speaker"), 1.5)
        (taping / "lose").touch()
        lost = wait_for(browser, lambda b: named(b) == ("AirPods Pro", ""), 1.5)
        check("a device changed is named within a second; a side with none names none", moved and lost, named(browser))
        shot(browser, "6-small-devices")
        check("nothing follows the stop here: the small window does not say the transcript writes itself", not browser.find_element(By.ID, "small-note").is_displayed())
        browser.find_element(By.ID, "small-stop").click()
        rest = wait_for(browser, lambda b: b.find_elements(By.ID, "small-record") and b.find_element(By.ID, "small-record").is_displayed(), 5)
        stopped = '"stopped"' in (taping / "2026-10-18_12-00" / "session.json").read_text()
        check("Arrêter stops the recording, and the small window comes back to rest — it does not close", rest and stopped and browser.window_handles == [small], browser.window_handles)
        last = wait_for(browser, lambda b: b.find_elements(By.CSS_SELECTOR, "#small .last"), 5)
        check("the meeting just recorded is its last", last and browser.find_element(By.CSS_SELECTOR, "#small .last .what").text != "")
        check("at rest and recording, the window keeps its size: nothing jumps", browser.execute_script("return window.innerHeight") == tall, (tall, browser.execute_script("return window.innerHeight")))
        shot(browser, "6-small-after")
        try:
            browser.find_element(By.ID, "small-full").click()
        except NoSuchWindowException:
            pass  # the small window goes with the click, sometimes before the driver has answered it
        full = wait_for(browser, lambda b: len(b.window_handles) == 1 and b.window_handles != [small], 5)
        check("Toute la fenêtre replaces the small one: one window", full, browser.window_handles)
        browser.switch_to.window(browser.window_handles[0])
        wait_for(browser, lambda b: b.execute_script("return document.getElementById('settings-link').textContent !== ''"))
        check("the full window left is the one kept in the settings", 'window = "full"' in window.file.read_text(), window.file.read_text())
        back = browser.find_element(By.ID, "small-link")
        check("at rest too, the full window offers the small one", back.is_displayed() and back.get_attribute("aria-label") == "Petite fenêtre", back.get_attribute("aria-label"))
        main = browser.current_window_handle
        back.click()
        smaller = wait_for(browser, lambda b: len(b.window_handles) == 2, 5)
        browser.switch_to.window([h for h in browser.window_handles if h != main][0])
        shown = wait_for(browser, lambda b: b.find_elements(By.ID, "small-record"), 5)
        browser.switch_to.window(main)
        hidden = browser.execute_script("return document.visibilityState")
        check("Petite fenêtre replaces the full one, hidden behind it", smaller and shown and hidden == "hidden", (browser.window_handles, hidden))
        check("the small window left is the one kept in the settings", 'window = "small"' in window.file.read_text(), window.file.read_text())
    taping_en = WORK / "taping-en"
    taping_en.mkdir()
    with Window(taping_en, "en_GB.UTF-8", "reminder = false\n") as browser:
        main = browser.current_window_handle
        browser.find_element(By.ID, "record").click()
        wait_for(browser, lambda b: b.find_element(By.ID, "record").get_attribute("class").find("live") >= 0, 5)
        check("a recording started from the full window opens no other window", browser.window_handles == [main], browser.window_handles)
        browser.find_element(By.ID, "small-link").click()
        wait_for(browser, lambda b: len(b.window_handles) == 2, 5)
        browser.switch_to.window([h for h in browser.window_handles if h != main][0])
        wait_for(browser, lambda b: b.find_elements(By.ID, "small-stop"))
        time.sleep(1)
        fits = browser.execute_script("const e = document.scrollingElement; return [e.scrollHeight, e.clientHeight];")
        said = browser.find_element(By.CSS_SELECTOR, "#small .said .side").text
        check("in English too, it holds in its window", said == "Your microphone" and fits[0] <= fits[1], (said, fits))
        starts = browser.execute_script("return [...document.querySelectorAll('#small .track')].map(t => t.getBoundingClientRect().left)")
        check("the two meters start at the same place", len(starts) == 2 and starts[0] == starts[1], starts)
        note = browser.find_element(By.ID, "small-note")
        check("the transcript follows the stop: the small window says so", note.is_displayed() and "transcript" in note.text, note.text)
        browser.find_element(By.ID, "small-stop").click()
        rest = wait_for(browser, lambda b: b.find_elements(By.ID, "small-record") and b.find_element(By.ID, "small-record").is_displayed(), 5)
        check("opened from the full window during a recording, it too comes back to rest, in English", rest and browser.find_element(By.ID, "small-record").text == "Start a recording")
        # Measured once its last meeting is drawn.
        wait_for(browser, lambda b: b.find_elements(By.ID, "small-read"), 5)
        time.sleep(0.5)
        fits = browser.execute_script("const e = document.scrollingElement; return [e.scrollHeight, e.clientHeight];")
        check("at rest, in English, with its last meeting, it holds in its window", fits[0] <= fits[1], fits)
        try:
            browser.find_element(By.ID, "small-read").click()
        except NoSuchWindowException:
            pass  # the small window goes with the click
        browser.switch_to.window(main)
        opened = wait_for(browser, lambda b: b.execute_script("return location.hash").startswith("#/meeting/") and len(b.window_handles) == 1, 5)
        check("its last meeting opens in the full window, in place of the small one", opened, (browser.execute_script("return location.hash"), browser.window_handles))
        # Clicked by the page itself: the driver's click, a window having just gone, was left unanswered
        # for a minute twice on a desktop in use — the windows' count below is what is checked.
        browser.execute_script("document.getElementById('small-link').click()")
        wait_for(browser, lambda b: len(b.window_handles) == 2, 5)
        browser.switch_to.window([h for h in browser.window_handles if h != main][0])
        wait_for(browser, lambda b: b.find_elements(By.ID, "small-all"), 5)
        try:
            browser.find_element(By.ID, "small-all").click()
        except NoSuchWindowException:
            pass
        browser.switch_to.window(main)
        listed = wait_for(browser, lambda b: b.execute_script("return location.hash") in ("#/", "") and b.find_element(By.ID, "list").is_displayed() and len(b.window_handles) == 1, 5)
        check("All meetings opens the full window's list", listed, browser.execute_script("return location.hash"))

    print("I. the window, in English; and a folder with no session")
    empty = WORK / "empty"
    empty.mkdir()
    with Window(empty, "en_GB.UTF-8") as browser:
        said = browser.find_element(By.ID, "none").text
        check("an empty folder is said, with its path", said == f"No meeting in {empty} yet.", said)
        check("the sidebar is named in English", browser.find_element(By.ID, "sidebar").get_attribute("aria-label") == "Meetings")
        lang = browser.execute_script("return document.documentElement.lang")
        check("the page says it is in English", lang == "en", lang)
        sections = browser.find_element(By.TAG_NAME, "nav").get_attribute("aria-label")
        check("its sections are named in English for a screen reader", sections == "Sections", sections)
        check("no weight is shown for nothing", not browser.find_element(By.ID, "weight").is_displayed())
        # One meeting is one, not "1 meetings".
        session(empty / "one", now - dt.timedelta(hours=2), 10, {"local.wav": 3000})
        browser.execute_script("window.dispatchEvent(new Event('focus'))")
        wait_for(browser, lambda b: b.find_element(By.ID, "weight").is_displayed())
        one = browser.find_element(By.ID, "taken").text
        check("one meeting is said in the singular", one == "taken by 1 meeting · 10 min of audio", one)
        # A failure of the page's own, thrown and rejected: logged by the shell (declared in the
        # manifest), its kind and where — never its sentence.
        browser.execute_script(
            "class ScenarioI extends Error { name = 'ScenarioI' }"
            " setTimeout(() => { throw new ScenarioI('Réunion avec Paul-Scenario') });"
            " setTimeout(() => Promise.reject(new ScenarioI('Réunion avec Paul-Scenario')))")
        logs = WORK / "state" / "wholeheard" / "logs"
        logged = lambda: "".join(p.read_text() for p in logs.glob("shell.*.jsonl")).count('"shell.page_failed"')
        wait_for(browser, lambda b: logged() >= 2, 10)
        text = "".join(p.read_text() for p in logs.glob("shell.*.jsonl"))
        check("the page's failures reach the log, never their sentence", logged() >= 2 and "Paul-Scenario" not in text)
        # What the notification plugin's own script asks at each page's start is answered: refused,
        # it failed every page, a rejection nobody catches.
        asked = browser.execute_async_script(
            "const done = arguments[arguments.length - 1];"
            " window.__TAURI_INTERNALS__.invoke('plugin:notification|is_permission_granted')"
            ".then(() => done('answered'), (why) => done('refused: ' + why))")
        check("the notification plugin's question at a page's start is answered", asked == "answered", asked)

    print("R. a meeting listened to, its words following (D26) — played to the run's own output, nothing reaches the speakers")
    kept = remembered()
    listening = WORK / "listening"
    played = listening / "2026-10-19_10-00"
    session(played, now - dt.timedelta(hours=1), 0.1, {})
    record = json.loads((played / "session.json").read_text())
    record["tracks"] = {"local": {"device": "Mic"}, "remote": {"device": "Speaker"}}
    (played / "session.json").write_text(json.dumps(record))
    for name, hertz in (("local", 440), ("remote", 880)):
        with wave.open(str(played / f"{name}.wav"), "wb") as made:
            made.setnchannels(2)
            made.setsampwidth(3)
            made.setframerate(48000)
            frames = bytearray()
            for n in range(48000 * 6):
                sample = int(0.2 * 8388607 * math.sin(2 * math.pi * hertz * n / 48000))
                frames += struct.pack("<i", sample)[:3] * 2
            made.writeframes(bytes(frames))
    # The remote track's frame 0 is a second and a half into the session: it ends after the local one.
    (played / "clock.jsonl").write_text('{"track":"local","sample":0,"at":0.0}\n{"track":"remote","sample":0,"at":1.5}\n')
    heard = [(0.4, "local", "Bonjour à tous."), (2.5, "remote", "Bonjour, je vous entends."), (4.2, "local", "Commençons.")]
    (played / "transcript.jsonl").write_text(
        '{"type":"header","contract":"transcript/v1","engine":"whisper.cpp","model":"m","language":"fr","context":null}\n'
        + "".join(json.dumps({"type": "segment", "speaker": "me" if track == "local" else "others", "track": track, "voice": None,
                              "start": start, "end": start + 1.5, "text": text}, ensure_ascii=False) + "\n" for start, track, text in heard)
        + '{"type":"complete","duration":6.0}\n'
    )
    freed = listening / "2026-10-19_09-00"
    session(freed, now - dt.timedelta(hours=2), 0.1, {})
    shutil.copy(played / "transcript.jsonl", freed / "transcript.jsonl")
    # Audio there, and no track this version plays: not said freed.
    unplayable = listening / "2026-10-19_08-00"
    session(unplayable, now - dt.timedelta(hours=3), 0.1, {"local.wav": 4000})
    shutil.copy(played / "transcript.jsonl", unplayable / "transcript.jsonl")
    # A meeting of two minutes, one track of silence: more than a player reads ahead of itself.
    long = listening / "2026-10-19_06-00"
    session(long, now - dt.timedelta(hours=5), 2, {})
    record = json.loads((long / "session.json").read_text())
    record["tracks"] = {"local": {"device": "Mic"}}
    (long / "session.json").write_text(json.dumps(record))
    with wave.open(str(long / "local.wav"), "wb") as made:
        made.setnchannels(1)
        made.setsampwidth(2)
        made.setframerate(48000)
        made.writeframes(bytes(2 * 48000 * 120))
    (long / "clock.jsonl").write_text('{"track":"local","sample":0,"at":0.0}\n')
    shutil.copy(played / "transcript.jsonl", long / "transcript.jsonl")
    # Tracks a WAV header names, in a form no player reads: listed, and refused by the window.
    unreadable = listening / "2026-10-19_07-00"
    session(unreadable, now - dt.timedelta(hours=4), 0.1, {})
    record = json.loads((unreadable / "session.json").read_text())
    record["tracks"] = {"local": {"device": "Mic"}}
    (unreadable / "session.json").write_text(json.dumps(record))
    noise = bytes(range(256)) * 16
    fmt = struct.pack("<HHIIHH", 0x9999, 2, 48000, 48000 * 6, 6, 24)
    (unreadable / "local.wav").write_bytes(b"RIFF" + struct.pack("<I", 4 + 24 + 8 + len(noise)) + b"WAVE"
                                           + b"fmt " + struct.pack("<I", len(fmt)) + fmt + b"data" + struct.pack("<I", len(noise)) + noise)
    (unreadable / "clock.jsonl").write_text('{"track":"local","sample":0,"at":0.0}\n')
    shutil.copy(played / "transcript.jsonl", unreadable / "transcript.jsonl")
    with (window := Window(listening, "fr_FR.UTF-8")) as browser:
        browser.execute_script("location.hash = '#/meeting/2026-10-19_10-00'")
        ready = wait_for(browser, lambda b: b.find_elements(By.ID, "play") and len(b.execute_script("return document.querySelectorAll('audio.track')")) == 1, 10)
        check("a meeting with its audio has a player: its tracks as one sound (ADR-0009)", ready)
        # The session's time is the one sound's, while it plays.
        at = lambda b: b.execute_script("const a = document.querySelector('audio.track'); return a && !a.paused ? a.currentTime : 0")
        playing = lambda b: b.execute_script("const a = document.querySelector('audio.track'); return !!a && !a.paused")
        # As long as the last track ends: the remote one, 1.5 s in, for 6 s.
        lasts = wait_for(browser, lambda b: b.execute_script("return document.querySelector('audio.track')?.duration || 0") > 0, 10)
        length = browser.execute_script("return document.querySelector('audio.track').duration || 0")
        check("the sound lasts to the end of the last track", lasts and abs(length - 7.5) < 0.05, length)
        # The lines below the window's edge, the page at its top: playing moves nothing (the decider, 2026-10-24).
        browser.execute_script("document.querySelector('#meeting .columns').style.marginTop = '2000px'; window.scrollTo(0, 0)")
        press(browser, "play")
        moving = wait_for(browser, lambda b: at(b) > 1.9, 10)
        check("Écouter plays the meeting", moving and playing(browser))
        marked = wait_for(browser, lambda b: b.execute_script("return document.querySelector('.lines li.now')?.dataset.start") == "0.4", 5)
        check("the line being said is marked", marked, browser.execute_script("return document.querySelector('.lines li.now')?.textContent"))
        still = browser.execute_script("return [window.scrollY, document.querySelector('.lines li.now').getBoundingClientRect().top > window.innerHeight]")
        check("the page stays where the user left it: the line marked is not brought into view", still == [0, True], still)
        browser.execute_script("document.querySelector('#meeting .columns').style.marginTop = ''")
        shot(browser, "10-playing")
        lasting = browser.find_element(By.CSS_SELECTOR, "#meeting .length").text
        check("a meeting under a minute says its seconds (#312)", lasting == "6 s", lasting)
        browser.find_element(By.ID, "search").send_keys(" ")
        check("a space typed in a field is the field's", not browser.execute_script("return document.querySelector('audio.track').paused"))
        browser.find_elements(By.CSS_SELECTOR, ".lines li")[2].click()
        jumped = wait_for(browser, lambda b: abs(at(b) - 4.2) < 0.6
                          and b.execute_script("return document.querySelector('.lines li.now')?.dataset.start") == "4.2", 5)
        check("a click on a line plays from it, and marks it", jumped, at(browser))
        width = browser.execute_script("return document.querySelector('.drawing').getBoundingClientRect().width")
        ActionChains(browser).move_to_element_with_offset(browser.find_element(By.CSS_SELECTOR, ".drawing"), int(-width / 2 + width * 0.4), 0).click().perform()
        sought = wait_for(browser, lambda b: abs(at(b) - 2.4) < 0.6, 5)
        check("a click on who spoke moves the playback there", sought, at(browser))
        browser.find_element(By.TAG_NAME, "body").send_keys(Keys.SPACE)
        paused = wait_for(browser, lambda b: b.execute_script("return [...document.querySelectorAll('audio.track')].every(a => a.paused)"), 5)
        check("the space bar pauses", paused and browser.find_element(By.ID, "play").text == "Écouter", browser.find_element(By.ID, "play").text)
        # Pause clicked with the mouse, held as a hand holds it: playing, the button's words are
        # read ten times a second, and a press on words written again is no click in WebKitGTK.
        browser.find_element(By.TAG_NAME, "body").send_keys(Keys.SPACE)
        wait_for(browser, playing, 5)
        ActionChains(browser).move_to_element(browser.find_element(By.ID, "play")).click_and_hold().pause(0.3).release().perform()
        paused = wait_for(browser, lambda b: not playing(b), 3)
        check("Pause clicked with the mouse, held 0.3 s, pauses", paused and browser.find_element(By.ID, "play").text == "Écouter",
              browser.find_element(By.ID, "play").text)
        # By the keyboard alone: the drawing is a slider, a line plays from it on Enter.
        drawing = browser.find_element(By.CSS_SELECTOR, ".drawing")
        slider = lambda b: b.find_element(By.CSS_SELECTOR, ".drawing").get_attribute("aria-valuenow")
        drawing.send_keys(Keys.HOME)
        drawing.send_keys(Keys.ARROW_RIGHT)
        moved = wait_for(browser, lambda b: slider(b) == "5", 5)
        check("the arrows move the playback on the drawing, a slider", moved and drawing.get_attribute("role") == "slider"
              and drawing.get_attribute("tabindex") == "0", slider(browser))
        drawing.send_keys(Keys.ARROW_LEFT)
        back = wait_for(browser, lambda b: slider(b) == "0", 5)
        # To the end of the sound — 7.5 s, past the session's 6 — a tenth before it.
        maximum = lambda b: float(b.find_element(By.CSS_SELECTOR, ".drawing").get_attribute("aria-valuemax"))
        drawing.send_keys(Keys.END)
        ended_at = wait_for(browser, lambda b: abs(float(slider(b)) - 7.4) < 0.05, 5)
        check("back, and to the end of the sound", back and ended_at and maximum(browser) == 7.5, (slider(browser), maximum(browser)))
        drawing.send_keys(Keys.PAGE_DOWN)
        down = wait_for(browser, lambda b: slider(b) == "0", 5)
        drawing.send_keys(Keys.ARROW_UP)
        up = wait_for(browser, lambda b: slider(b) == "5", 5)
        drawing.send_keys(Keys.ARROW_DOWN)
        check("up and down, page up and page down, as a slider's keys", down and up and wait_for(browser, lambda b: slider(b) == "0", 5), slider(browser))
        stops = lambda b: b.execute_script("return [...document.querySelectorAll('.lines li')].map(li => li.tabIndex)")
        # The line clicked last is the one in Tab's order.
        check("one line in the order of Tab, the one clicked last", stops(browser) == [-1, -1, 0], stops(browser))
        first = browser.find_elements(By.CSS_SELECTOR, ".lines li")[0]
        first.send_keys(Keys.ARROW_DOWN)
        line = browser.find_elements(By.CSS_SELECTOR, ".lines li")[1]
        focused = browser.execute_script("return document.activeElement === arguments[0]", line)
        check("the arrow moves the line in the order of Tab", stops(browser) == [-1, 0, -1], stops(browser))
        line.send_keys(Keys.ENTER)
        entered = wait_for(browser, lambda b: abs(at(b) - 2.5) < 0.6, 5)
        check("down to a line, Enter plays from it", focused and entered and line.get_attribute("role") == "button", at(browser))
        # A search that hides that line: the first line shown takes its place in Tab's order.
        browser.find_element(By.ID, "search").send_keys("Commençons")
        sought = wait_for(browser, lambda b: stops(b) == [-1, -1, 0], 5)
        check("a search hiding the line in Tab's order gives the first shown its place", sought, stops(browser))
        browser.find_element(By.ID, "search").send_keys(Keys.CONTROL, "a")
        browser.find_element(By.ID, "search").send_keys(Keys.DELETE)
        # Space held down: the first press alone counts.
        browser.execute_script("document.body.dispatchEvent(new KeyboardEvent('keydown', {key: ' ', repeat: true, bubbles: true}))")
        check("Space held does not toggle again", not browser.execute_script("return document.querySelector('audio.track').paused"))
        # The page drawn again while it plays: the same player goes on.
        drawn_before = browser.find_element(By.CSS_SELECTOR, ".drawing")
        browser.execute_script("window.dispatchEvent(new Event('focus'))")
        try:
            WebDriverWait(browser, 5).until(staleness_of(drawn_before))
        except TimeoutException:
            check("the page is drawn again", False)
        count = browser.execute_script("return document.querySelectorAll('audio.track').length")
        check("drawn again, it plays on", count == 1 and playing(browser) and at(browser) > 2.5, (count, at(browser)))
        # From the start, through the end of the shorter track, to the end of the last.
        press(browser, "play")
        wait_for(browser, lambda b: browser.find_element(By.ID, "play").text == "Écouter", 5)
        browser.find_element(By.CSS_SELECTOR, ".drawing").send_keys(Keys.HOME)
        press(browser, "play")
        past = wait_for(browser, lambda b: at(b) > 6.3, 12)
        check("from the start, past the end of the shorter track, it plays on", past and playing(browser), at(browser))
        ended = wait_for(browser, lambda b: browser.find_element(By.ID, "play").text == "Écouter", 5)
        check("at the end of the last, the meeting stops", ended and not playing(browser))
        press(browser, "play")
        anew = wait_for(browser, lambda b: 0 < at(b) < 1.4, 5)
        check("played again, it starts over", anew, at(browser))
        # Paused at once, the sound not yet loaded: no failure is said.
        browser.execute_script("location.hash = '#/settings'")
        wait_for(browser, lambda b: b.find_element(By.ID, "settings").is_displayed(), 10)
        browser.execute_script("location.hash = '#/meeting/2026-10-19_10-00'")
        wait_for(browser, lambda b: b.find_elements(By.ID, "play") and b.find_element(By.ID, "meeting").is_displayed()
                 and b.execute_script("return document.getElementById('meeting').dataset.name") == "2026-10-19_10-00", 10)
        browser.execute_script("document.getElementById('play').click(); document.getElementById('play').click()")
        time.sleep(1)
        check("a play paused at once is no failure", not browser.find_element(By.ID, "not-played").text and not playing(browser))
        # Its audio freed while it plays: it stops, and says so.
        for name in ("local", "remote"):
            (played / f"{name}.wav").rename(listening / f"aside-{name}.wav")
        browser.execute_script("window.dispatchEvent(new Event('focus'))")
        freed = wait_for(browser, lambda b: not b.execute_script("return document.querySelectorAll('audio.track').length")
                         and b.find_elements(By.ID, "no-player"), 10)
        check("its audio freed while it plays, it stops", freed)
        for name in ("local", "remote"):
            (listening / f"aside-{name}.wav").rename(played / f"{name}.wav")
        browser.execute_script("location.hash = '#/'")
        wait_for(browser, lambda b: b.find_element(By.ID, "list").is_displayed(), 10)
        browser.execute_script("location.hash = '#/meeting/2026-10-19_10-00'")
        wait_for(browser, lambda b: b.find_elements(By.ID, "play") and b.find_element(By.ID, "meeting").is_displayed()
                 and b.execute_script("return document.getElementById('meeting').dataset.name") == "2026-10-19_10-00", 10)
        press(browser, "play")
        wait_for(browser, lambda b: at(b) > 0.3, 5)
        heard = played_to(window)
        check("its sound goes to the run's own output, never the speakers", bool(QUIET) and heard == {QUIET}, heard)
        # The list beside it, the meeting still shown, plays on (D61); the list alone, under 840 px, stops it.
        browser.set_window_size(420, 400)
        wait_for(browser, lambda b: b.execute_script("return matchMedia('(max-width: 839px)').matches"), 5)
        browser.execute_script("location.hash = '#/'")
        stopped = wait_for(browser, lambda b: not b.execute_script("return document.querySelectorAll('audio.track').length"), 10)
        check("the list shown alone, nothing plays", stopped)
        browser.set_window_size(1100, 800)
        # Its sound refused by the server — its track gone since the page was drawn, then a part of
        # it asked for that the player had not read: said, never silence.
        browser.execute_script("location.hash = '#/meeting/2026-10-19_06-00'")
        wait_for(browser, lambda b: b.find_elements(By.ID, "play") and b.execute_script("return document.querySelector('audio.track')?.duration || 0") > 0, 10)
        (long / "local.wav").rename(listening / "aside-long.wav")
        browser.find_element(By.CSS_SELECTOR, ".drawing").send_keys(Keys.END)
        press(browser, "play")
        said = wait_for(browser, lambda b: b.find_elements(By.ID, "not-played") and b.find_element(By.ID, "not-played").text, 10)
        check("a sound that cannot be played is said", said and "Lecture impossible ici" in browser.find_element(By.ID, "not-played").text)
        (listening / "aside-long.wav").rename(long / "local.wav")
        # A track in a form no player reads: no sound, and it is said not playable here.
        browser.execute_script("location.hash = '#/meeting/2026-10-19_07-00'")
        unread = wait_for(browser, lambda b: b.find_elements(By.ID, "no-player") and "ne peut pas lire cet audio" in b.find_element(By.ID, "no-player").text, 10)
        check("a track no player reads is said not playable here", unread)
        # The page before stays drawn until the next one is: its sentence is waited for, read in
        # the page at once — an element found then read may be the page before's, drawn again.
        no_player = "return document.getElementById('no-player')?.innerText || ''"
        browser.execute_script("location.hash = '#/meeting/2026-10-19_09-00'")
        none = wait_for(browser, lambda b: "supprimé pour faire de la place" in b.execute_script(no_player), 10)
        check("a meeting whose audio was freed has no player, and says why",
              none and not browser.find_elements(By.ID, "play")
              and not browser.execute_script("return document.querySelectorAll('audio.track').length"),
              browser.execute_script(no_player))
        browser.execute_script("location.hash = '#/meeting/2026-10-19_08-00'")
        there = wait_for(browser, lambda b: "ne peut pas lire cet audio" in b.execute_script(no_player), 10)
        check("a meeting whose audio is there but not playable is not said freed",
              there and "supprimé pour faire de la place" not in browser.execute_script(no_player), browser.execute_script(no_player))
    check("the volume the system remembers for the user's Wholeheard is left as it was", same_volume(remembered(), kept), [remembered(), kept])
    model_offered(now)
    models_listed(now)
    list_refreshed(now)
    speech_offered(now)
    service_named(now)
    ollama_pulled(now)
    first_run(now)
    context_written(now)
    confidential_kept(now)
    meeting_language(now)
    transcript_corrected(now)
    corrections_gated(now)
    context_gaps(now)
    two_panes(now)
    sidebar_folded(now)
    app_language(now)
    confidential_every_way(now)
    meeting_told_once(now)
    minutes_copied(now)
    nothing_silent(now)
    gestures_answer(now)
    window_looks(now)
    actions_rows(now)
    announced(now)
    heard_checked(now)
    meeting_named(now)
    word_after_a_gap(now)
    minutes_checked(now)
    meeting_renamed(now)
    return 1 if FAILED else 0


if __name__ == "__main__":
    # A scenario that breaks off says where, on the output the evidence keeps: its error stream
    # is not kept, and a run that failed once must say why.
    try:
        sys.exit(main())
    except Exception:
        traceback.print_exc(file=sys.stdout)
        sys.exit(1)
