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
import shutil
import socket
import struct
import subprocess
import sys
import threading
import time
import traceback
import wave
from pathlib import Path

from selenium import webdriver
from selenium.common.exceptions import NoSuchWindowException, StaleElementReferenceException, TimeoutException, WebDriverException
from selenium.webdriver.common.action_chains import ActionChains
from selenium.webdriver.common.by import By
from selenium.webdriver.common.keys import Keys
from selenium.webdriver.common.options import ArgOptions
from selenium.webdriver.support.expected_conditions import staleness_of
from selenium.webdriver.support.select import Select
from selenium.webdriver.support.wait import WebDriverWait
from urllib3.exceptions import HTTPError

BINARY, WORK = Path(sys.argv[1]), Path(sys.argv[2])
# wholeheard-summary played here: it says what it was given — its arguments, the key in its
# environment. A test refuses a key at openrouter.ai and answers elsewhere; minutes are written,
# the way it was called kept beside them. Nothing leaves the machine.
SUMMARY = WORK / "wholeheard-summary"
SUMMARY.write_text(
    """#!/bin/sh
here=$(dirname "$0")
printf '%s\\n' "$*" > "$here/summary-args"
printf '%s' "${WHOLEHEARD_PROVIDER_KEY:-none}" > "$here/summary-key"
if [ "$1" != --test ]; then
  [ -e "$here/slow-minutes" ] && sleep 3
  [ -e "$here/fail-minutes" ] && { echo '{"event": "failed", "code": "answer.not_minutes"}'; echo "wholeheard-summary: the provider's answer is not minutes" >&2; exit 1; }
  [ -e "$here/fail-plainly" ] && { echo "wholeheard-summary: a failure no code names" >&2; exit 1; }
  echo '{"event": "sending", "host": "h", "model": "m", "characters": 1}'
  printf -- '---\\ncontract: minutes/v1\\nmodel: %s\\n---\\n\\n## A\\n\\n## B\\n\\n## C\\n' "$5" > "$1/minutes.md"
  echo '{"event": "written", "file": "x"}'
  exit 0
fi
case "$*" in *openrouter.ai*) echo "wholeheard-summary: the provider at openrouter.ai refused the key" >&2; exit 1;; esac
echo '{"event": "sending"}'
echo '{"event": "answered"}'
"""
)
SUMMARY.chmod(0o755)
# wholeheard-transcription played here: a whole transcript, naming the model it was given.
HEARING = WORK / "wholeheard-transcription"
HEARING.write_text(
    """#!/bin/sh
[ -e "$(dirname "$0")/fail-hearing" ] && { echo "wholeheard-transcription: the engine stopped" >&2; exit 1; }
[ -e "$(dirname "$0")/slow-hearing" ] && sleep 3
model=$(basename "$3" .bin)
printf '{"type":"header","contract":"transcript/v1","engine":"e","model":"%s","language":null,"context":null}\\n{"type":"segment","speaker":"me","track":"local","start":0.0,"end":1.0,"text":"Refait avec %s."}\\n{"type":"complete","duration":1.0}\\n' "$model" "$model" > "$1/transcript.jsonl"
"""
)
HEARING.chmod(0o755)
# wholeheard-capture played here: it says it started, then each source's level ten times a second,
# until it is told to stop. No device, nobody recorded.
RECORDER = WORK / "wholeheard-capture"
RECORDER.write_text(
    """#!/bin/bash
folder="$2/2026-10-18_12-00"; mkdir -p "$folder"
printf '{"contract":"session-audio/v2","started":"2026-10-18T12:00:00+02:00","tracks":{}}' > "$folder/session.json"
echo '{"event":"started","folder":"'"$folder"'","sources":{"me":"Mic","others":"Speaker"}}'
n=0
while ! read -t 0.1 line; do
  n=$((n + 1)); echo '{"event":"level","source":"me","level":0.'$((n % 9 + 1))'}'; echo '{"event":"level","source":"others","level":0.5}'
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
    """A click on the page as it is: drawn again under the driver, the element is found afresh."""
    for _ in range(5):
        try:
            browser.find_element(By.ID, element_id).click()
            return
        except StaleElementReferenceException:
            time.sleep(0.2)
    browser.find_element(By.ID, element_id).click()


def stop_from_small(browser) -> None:
    """Arrêter in the small window: the window goes with the recording, sometimes before the
    driver has answered the click."""
    try:
        browser.find_element(By.ID, "small-stop").click()
    except NoSuchWindowException:
        pass


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


class Window:
    """Wholeheard with its window, for as long as the `with` lasts."""

    def __init__(self, sessions: Path, language: str, more: str = "", ollama: str = "http://127.0.0.1:9", small: bool = False) -> None:
        config = WORK / f"config-{language}-{sessions.name}"
        (config / "wholeheard").mkdir(parents=True, exist_ok=True)
        self.file = config / "wholeheard" / "settings.toml"
        # The full window, as the scenarios before D35 knew it; the small one, opened first, where asked.
        self.small = small
        opened = "" if small else 'window = "full"\n'
        self.file.write_text(f'sessions = "{sessions}"\n{opened}{more}')
        self.env = dict(os.environ, XDG_CONFIG_HOME=str(config), XDG_DATA_HOME=str(WORK / "data"), XDG_STATE_HOME=str(WORK / "state"), LANGUAGE="")
        # An Ollama and a keyring of the scenario's, never the user's own.
        self.env.update(LANG=language, LC_ALL=language, WHOLEHEARD_OLLAMA=ollama, WHOLEHEARD_KEYRING="memory", WHOLEHEARD_SUMMARY=str(SUMMARY), WHOLEHEARD_TRANSCRIPTION=str(HEARING), WHOLEHEARD_CAPTURE=str(RECORDER))

    def __enter__(self) -> webdriver.Remote:
        port, native = free_port(), free_port()
        self.driver_process = subprocess.Popen(
            ["tauri-driver", "--port", str(port), "--native-port", str(native)],
            env=self.env,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        options = ArgOptions()
        options.set_capability("browserName", "wry")
        options.set_capability("tauri:options", {"application": str(BINARY), "args": ["--window"]})
        # The driver answers once it is up: until then, connections are refused or cut.
        for _ in range(50):
            try:
                self.browser = webdriver.Remote(f"http://127.0.0.1:{port}", options=options)
                break
            except (WebDriverException, HTTPError):
                time.sleep(0.2)
        else:
            self.driver_process.terminate()
            self.driver_process.wait(timeout=10)
            raise SystemExit("tauri-driver did not answer")
        # Whichever window opened: the small one at rest, or the full one's sections named.
        wait_for(self.browser, lambda b: b.find_elements(By.ID, "small-record") or b.execute_script("return document.getElementById('meetings').textContent !== ''"))
        return self.browser

    def __exit__(self, *_) -> None:
        try:
            self.browser.quit()
        finally:
            self.driver_process.terminate()
            self.driver_process.wait(timeout=10)


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
        return "Date illisible"
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
        browser.find_element(By.ID, "make-transcript").click()
        waits = "return document.getElementById('transcript-state')?.innerText || ''"
        offered = wait_for(browser, lambda b: "En attente du modèle Whisper" in b.execute_script(waits) and b.find_elements(By.CSS_SELECTOR, "#transcript-state #model-fetch"))
        check("a meeting waiting for the model offers it on its page", offered, browser.execute_script(waits))
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
        check("Settings list Whisper large v3 turbo and Whisper small, each with its size and what it is good for",
              listed and "Whisper large v3 turbo" in said[0] and "200 ko" in said[0] and "Recommandé" in said[0]
              and "Whisper small" in said[1] and "ordinateur modeste" in said[1], said)
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
        time.sleep(0.5)
        browser.execute_script("document.querySelector('[data-offered=\"ggml-small-q8_0.bin\"] button').click()")
        here = wait_for(browser, lambda b: "Sur cet ordinateur" in b.execute_script(small_row), 15)
        check("fetched, its row says it is on this computer", here and (models / "ggml-small-q8_0.bin").read_bytes() == small, browser.execute_script(small_row))
        told = wait_for(browser, lambda b: "Choisissez-le" in notified.read_text(), 5)
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
            (this, "Interrompu — jamais arrêté|Audio · 2 ko"),
            (last, "42 min|Compte rendu|Transcript|Audio · 200 Mo"),
            (months, "35 min|Transcript|Audio supprimé"),
            (earlier, "20 min|Transcript|Audio supprimé"),
            (hours_only, "10 min|Audio · 1 Mo"),
        ],
        key=lambda pair: pair[0],
        reverse=True,
    )
    expected = [(group(day.astimezone(), now), holds) for day, holds in expected]
    expected.append(("Date illisible", "Interrompu — jamais arrêté|Audio supprimé"))

    print("H. the window, in French: the meetings by week, what each holds and weighs")
    with Window(sessions, "fr_FR.UTF-8") as browser:
        check("the section is named in French", browser.find_element(By.ID, "meetings").text == "Réunions")
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
        check("Tab reaches the window's first action", focused == "meetings", focused)
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
        browser.find_element(By.TAG_NAME, "body").send_keys(Keys.TAB, Keys.TAB, Keys.TAB, Keys.TAB)
        focused = browser.execute_script("return document.activeElement.id")
        check("Tab reaches the header's gesture after the two sections and the small window's (D35)", focused == "record", focused)
        browser.find_element(By.ID, "record").send_keys(Keys.TAB)
        focused = browser.execute_script("return document.activeElement.id")
        check("then freeing space, beside the weights", focused == "free-link", focused)
        browser.find_element(By.ID, "free-link").send_keys(Keys.TAB)
        focused = browser.execute_script("return document.activeElement.className")
        check("then the first meeting", "session" in focused, focused)
        drawn = browser.execute_script("return [...document.querySelectorAll('#groups .session')].map(s => s.querySelector('.relief.small:not(.quiet) .said') !== null)")
        check("a meeting whose transcript reads shows who spoke, small, beside it", True in drawn, drawn)
        shot(browser, "1-sessions-drawn")
        browser.find_elements(By.CSS_SELECTOR, ".session")[0].click()
        shown = wait_for(browser, lambda b: b.find_element(By.ID, "meeting").is_displayed() and b.find_elements(By.CSS_SELECTOR, ".lines li"))
        check("a click on a meeting opens it", shown and not browser.find_element(By.ID, "list").is_displayed())
        shot(browser, "2-session")
        minutes = browser.find_element(By.CSS_SELECTOR, ".minutes")
        titles = [h.text for h in minutes.find_elements(By.CSS_SELECTOR, ".body h2")]
        check("the minutes' three sections, named by the page whatever the model titled them", titles == ["Points saillants", "Actions", "Points clés"], titles)
        items = [li.text for li in minutes.find_elements(By.CSS_SELECTOR, ".body li")]
        check(
            "the minutes as written: each action, each key point",
            items
            == [
                "Mettre à jour le planning — Marc — aujourd’hui",
                "Couper deux serveurs de test — Julien — avant vendredi",
                "Un serveur est gardé pour la démonstration du 20.",
            ],
            items,
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
        check("a search that finds nothing says so", none == ["Rien de ce qui a été dit ne correspond."], none)
        search.clear()
        search.send_keys("voix")
        none = [p.text for p in browser.find_elements(By.CSS_SELECTOR, ".transcript .none") if p.is_displayed()]
        check("a search looks at what was said, not at who or when", none == ["Rien de ce qui a été dit ne correspond."], none)
        check("the folder can be opened from the meeting", browser.find_element(By.CSS_SELECTOR, "#meeting .meeting-top .open").text == "Ouvrir le dossier")
        browser.find_element(By.CSS_SELECTOR, ".back").click()
        back = wait_for(browser, lambda b: b.find_element(By.ID, "list").is_displayed())
        check("the way back is the list", back and not browser.find_element(By.ID, "meeting").is_displayed())
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
        listed = wait_for(browser, lambda b: b.find_element(By.ID, "list").is_displayed())
        check("a meeting that is not there goes back to the list", listed and not browser.find_element(By.ID, "meeting").is_displayed())

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
        check("the models on this computer are offered, by the names people say", names == ["Whisper large v3 turbo", "Whisper small"], names)
        ready = browser.find_element(By.ID, "model-state").text
        check("the transcription model's state is said at the page's end", "Prêt" in ready and "sans réseau" in ready, ready)
        # Each change is saved, then the page is drawn again: the next acts on the new one.
        def changed(act):
            before = browser.find_element(By.ID, "reminder")
            act()
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
        offer_on = Select(browser.find_element(By.ID, "offer")).first_selected_option.get_attribute("value") == "offer"
        changed(lambda: Select(browser.find_element(By.ID, "offer")).select_by_value("nothing"))
        offer_off = "offer = false" in window.file.read_text()
        changed(lambda: Select(browser.find_element(By.ID, "offer")).select_by_value("offer"))
        offer_again = "offer = true" in window.file.read_text()
        changed(lambda: Select(browser.find_element(By.ID, "offer")).select_by_value("nothing"))
        # Started with the user's session (D32): on from the first start, the desktop's own entry;
        # off, then turned off by the desktop's tools, then on again, each read from the entry.
        entry = window.file.parent.parent / "autostart" / "dev.wholeheard.Wholeheard.desktop"
        switch = lambda: browser.find_element(By.ID, "autostart")
        start_on = switch().is_selected() and entry.is_file() and "Exec=wholeheard\n" in entry.read_text()
        changed(lambda: switch().click())
        start_off = not switch().is_selected() and not entry.exists() and "autostart = false" in window.file.read_text()
        entry.write_text("[Desktop Entry]\nType=Application\nExec=wholeheard\nX-GNOME-Autostart-enabled=false\n")
        browser.execute_script("location.hash = '#/'")
        wait_for(browser, lambda b: b.find_element(By.ID, "list").is_displayed(), 10)
        to_settings()
        tools_off = not switch().is_selected()
        changed(lambda: switch().click())
        start_again = switch().is_selected() and "TryExec=wholeheard\n" in entry.read_text() and "autostart = true" in window.file.read_text()
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
        offered = [o.text for o in browser.find_elements(By.CSS_SELECTOR, "#keep option")]
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
        seen = "none granted" if keys == "Pas accordé par le bureau" else "keys granted"
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
        browser.find_element(By.ID, "meetings").click()
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
        said = " ".join(p.text for p in browser.find_elements(By.CSS_SELECTOR, "#minutes-where, #minutes-silent, #minutes-lacking"))
        check("this computer's Ollama not answering: said so, here, never a model said missing",
              silent and "Sur cet ordinateur" in said and "ne répond pas" in said and not browser.find_elements(By.ID, "minutes-lacking"), said)
        window.file.write_text(kept)
        browser.execute_script("location.hash = '#/'")
        to_settings()
        # A file broken by hand is never written over: the change is said not saved.
        window.file.write_text(window.file.read_text() + "keep_audio = 0\n")
        broken = window.file.read_text()
        changed(lambda: browser.find_element(By.ID, "reminder").click())
        said = not_saved()
        check("a file that cannot be read is not written over", said.startswith("Le réglage n’a pas été enregistré") and window.file.read_text() == broken, said)
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
        answered = wait_for(browser, lambda b: b.find_element(By.ID, "minutes-tested").text == "Le modèle répond.", 10)
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
        press(browser, "minutes-test")
        refused = wait_for(browser, lambda b: b.find_element(By.ID, "minutes-tested").text.startswith("Le modèle ne répond pas"), 10)
        said = browser.find_element(By.ID, "minutes-tested").text
        check(
            "a model that does not answer is said in one sentence; the key goes by the environment, never the arguments",
            refused
            and said.endswith("the provider at openrouter.ai refused the key")
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
        check("a settings file that cannot be read is said so, and nothing can be changed", warned.startswith("Le fichier des réglages ne peut pas être lu") and locked, warned)
        check("it is left as it was", window.file.read_text() == f'sessions = "{broken}"\nkeep_audio = 0\n')
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
        check("a file corrected by hand opens the page again", open_again and not browser.find_elements(By.CSS_SELECTOR, "#settings .warn:not([hidden])")[1:])

    print("N. before a meeting first leaves for a host: what leaves, to whom; sent only on the word")
    agreeing = WORK / "agreeing"
    asked = agreeing / "2026-10-18_11-00"
    asked.mkdir(parents=True)
    (asked / "session.json").write_text('{"contract":"session-audio/v2","started":"2026-10-18T11:00:00+02:00","stopped":"2026-10-18T11:20:00+02:00","tracks":{}}')
    (asked / "transcript.jsonl").write_text(
        '{"type":"header","contract":"transcript/v1","engine":"e","model":"m","language":null,"context":null}\n'
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
        check("the meeting's page says where its transcript was made", "Transcript · sur cette machine" in chips, chips)
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
        waits = wait_for(browser, lambda b: "En attente de son tour — 1 avant" in state(b), 3)
        check("one asked while another is written waits its turn, said on its page", waits, state(browser))
        browser.execute_script("location.hash = '#/'")
        wait_for(browser, lambda b: len([c for c in b.find_elements(By.CSS_SELECTOR, ".chip.making") if c.is_displayed()]) == 2, 3)
        rowed = sorted(c.text for c in browser.find_elements(By.CSS_SELECTOR, ".chip.making") if c.is_displayed())
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
        chip_ = wait_for(browser, lambda b: "Votre accord" in [c.text for c in b.find_elements(By.CSS_SELECTOR, ".chip.making") if c.is_displayed()], 3)
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
        browser.find_elements(By.CSS_SELECTOR, "#redo-panel input[name=redo-with]")[1].click()
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
        browser.find_elements(By.CSS_SELECTOR, "#redo-panel input[name=redo-with]")[1].click()
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
        wait_for(browser, lambda b: b.find_elements(By.ID, "redo-do"))
        browser.find_elements(By.CSS_SELECTOR, "#redo-panel input[name=redo-with]")[0].click()
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
        browser.find_elements(By.CSS_SELECTOR, "#redo-panel input[name=redo-with]")[1].click()
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
        browser.find_elements(By.CSS_SELECTOR, "#redo-transcript-panel input")[1].click()
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
        unmade = wait_for(browser, lambda b: "Le transcript n’a pas été fait" in heard_state(b) and b.find_elements(By.ID, "transcript-again"), 5)
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
        browser.find_elements(By.CSS_SELECTOR, "#redo-transcript-panel input")[1].click()
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
        check("nothing follows the stop here: the small window does not say the transcript writes itself", not browser.find_element(By.CSS_SELECTOR, "#small .says").is_displayed())
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
        wait_for(browser, lambda b: b.execute_script("return document.getElementById('meetings').textContent !== ''"))
        check("the full window left is the one kept in the settings", 'window = "full"' in window.file.read_text(), window.file.read_text())
        back = browser.find_element(By.ID, "small-link")
        check("at rest too, the full window offers the small one", back.is_displayed() and back.text == "Petite fenêtre", back.text)
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
        note = browser.find_element(By.CSS_SELECTOR, "#small .says")
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
        browser.find_element(By.ID, "small-link").click()
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
        check("the section is named in English", browser.find_element(By.ID, "meetings").text == "Meetings")
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
        '{"type":"header","contract":"transcript/v1","engine":"e","model":"m","language":"fr","context":null}\n'
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
        browser.execute_script("location.hash = '#/'")
        wait_for(browser, lambda b: b.find_element(By.ID, "list").is_displayed(), 10)
        browser.execute_script("location.hash = '#/meeting/2026-10-19_10-00'")
        wait_for(browser, lambda b: b.find_elements(By.ID, "play") and b.find_element(By.ID, "meeting").is_displayed(), 10)
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
        wait_for(browser, lambda b: b.find_elements(By.ID, "play") and b.find_element(By.ID, "meeting").is_displayed(), 10)
        press(browser, "play")
        wait_for(browser, lambda b: at(b) > 0.3, 5)
        heard = played_to(window)
        check("its sound goes to the run's own output, never the speakers", bool(QUIET) and heard == {QUIET}, heard)
        browser.execute_script("location.hash = '#/'")
        stopped = wait_for(browser, lambda b: not b.execute_script("return document.querySelectorAll('audio.track').length"), 10)
        check("the list shown, nothing plays", stopped)
        # Its sound refused by the server — its track gone since the page was drawn, then a part of
        # it asked for that the player had not read: said, never silence.
        browser.execute_script("location.hash = '#/meeting/2026-10-19_06-00'")
        wait_for(browser, lambda b: b.find_elements(By.ID, "play") and b.execute_script("return document.querySelector('audio.track')?.duration || 0") > 0, 10)
        (long / "local.wav").rename(listening / "aside-long.wav")
        browser.find_element(By.CSS_SELECTOR, ".drawing").send_keys(Keys.END)
        press(browser, "play")
        said = wait_for(browser, lambda b: b.find_elements(By.ID, "not-played") and b.find_element(By.ID, "not-played").text, 10)
        check("a sound that cannot be played is said", said and "n’a pas pu être lue" in browser.find_element(By.ID, "not-played").text)
        (listening / "aside-long.wav").rename(long / "local.wav")
        # A track in a form no player reads: no sound, and it is said not playable here.
        browser.execute_script("location.hash = '#/meeting/2026-10-19_07-00'")
        unread = wait_for(browser, lambda b: b.find_elements(By.ID, "no-player") and "ne s’écoute pas ici" in b.find_element(By.ID, "no-player").text, 10)
        check("a track no player reads is said not playable here", unread)
        # The page before stays drawn until the next one is: its sentence is waited for, read in
        # the page at once — an element found then read may be the page before's, drawn again.
        no_player = "return document.getElementById('no-player')?.innerText || ''"
        browser.execute_script("location.hash = '#/meeting/2026-10-19_09-00'")
        none = wait_for(browser, lambda b: "libéré" in b.execute_script(no_player), 10)
        check("a meeting whose audio was freed has no player, and says why",
              none and not browser.find_elements(By.ID, "play")
              and not browser.execute_script("return document.querySelectorAll('audio.track').length"),
              browser.execute_script(no_player))
        browser.execute_script("location.hash = '#/meeting/2026-10-19_08-00'")
        there = wait_for(browser, lambda b: "ne s’écoute pas ici" in b.execute_script(no_player), 10)
        check("a meeting whose audio is there but not playable is not said freed",
              there and "libéré" not in browser.execute_script(no_player), browser.execute_script(no_player))
    check("the volume the system remembers for the user's Wholeheard is left as it was", remembered() == kept, remembered())
    model_offered(now)
    models_listed(now)
    return 1 if FAILED else 0


if __name__ == "__main__":
    # A scenario that breaks off says where, on the output the evidence keeps: its error stream
    # is not kept, and a run that failed once must say why.
    try:
        sys.exit(main())
    except Exception:
        traceback.print_exc(file=sys.stdout)
        sys.exit(1)
