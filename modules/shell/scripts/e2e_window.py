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
    """A click on the page as it is: drawn again under the driver, the element is found afresh;
    found before it can be clicked — shown, laid out — it is clicked a moment later (#510)."""
    for _ in range(10):
        try:
            browser.find_element(By.ID, element_id).click()
            return
        except (StaleElementReferenceException, ElementNotInteractableException):
            time.sleep(0.2)
    browser.find_element(By.ID, element_id).click()


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
    """W. Settings read the list of models again on the user's click, its host shown (D40)."""
    print("W. Settings: the list of models read again on the user's click, its host shown (D40)")
    shipped = json.loads((Path(__file__).resolve().parent.parent / "registry" / "models.json").read_text())
    more = json.loads(json.dumps(shipped))
    # A model of a test's, never fetched: offered, its host shown.
    more["whisper"].append({"address": "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base-q8_0.bin",
                            "bytes": 1000, "sha256": "0" * 64, "good": "faster"})
    answers, asked = [(200, json.dumps(more)), (404, "Not Found")], []

    class List(http.server.BaseHTTPRequestHandler):
        def do_GET(self):
            asked.append(self.path)
            status, body = answers.pop(0)
            self.send_response(status)
            self.send_header("Content-Length", str(len(body.encode())))
            self.end_headers()
            self.wfile.write(body.encode())

        def log_message(self, *_):
            pass

    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), List)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    at = f"127.0.0.1:{server.server_address[1]}"
    meetings = WORK / "refreshed"
    meetings.mkdir()
    base = "return document.querySelector('[data-offered=\"ggml-base-q8_0.bin\"]')?.innerText || ''"
    said = "return document.getElementById('offered-refreshed')?.innerText || ''"
    for answered in ("a list", "a page not found"):
        window = Window(meetings, "fr_FR.UTF-8")
        window.env.update(WHOLEHEARD_REGISTRY=f"http://{at}/models.json")
        with window as browser:
            browser.find_element(By.ID, "settings-link").click()
            wait_for(browser, lambda b: b.find_elements(By.ID, "offered-refresh"))
            label = browser.find_element(By.ID, "offered-refresh").text
            if answered == "a list":
                check("opening Settings reads nothing of the list", asked == [], asked)
                check("the button says where the list is read from", label == f"Mettre à jour la liste ({at})", label)
            browser.execute_script("document.getElementById('offered-refresh').click()")
            if answered == "a list":
                listed = wait_for(browser, lambda b: b.execute_script(base), 10)
                check("read on the click, its models are offered, each with its size, its host and what it is good for",
                      listed and asked == ["/models.json"] and "huggingface.co" in browser.execute_script(base)
                      and "1 ko" in browser.execute_script(base)
                      and "ordinateur modeste" in browser.execute_script(base), (asked, browser.execute_script(base)))
            else:
                refused = wait_for(browser, lambda b: "n’a pas pu être mise à jour" in b.execute_script(said), 10)
                check("refused, it is said, and the list shipped stays offered",
                      refused and len(asked) == 2 and not browser.execute_script(base)
                      and browser.find_elements(By.CSS_SELECTOR, '[data-offered="ggml-small-q8_0.bin"]'),
                      (asked, browser.execute_script(said)))
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
        browser.find_element(By.ID, "transcription-test").click()
        tested = wait_for(browser, lambda b: b.execute_script(said) and "Enregistré" not in b.execute_script(said), 10)
        check("Tester says in one sentence what the service answered", tested, browser.execute_script(said))
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
        check("no header of the meetings while it is answered", not browser.find_element(By.CSS_SELECTOR, "header.bar").is_displayed())
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
        ring = wait_for(browser, lambda b: b.execute_script("const s = getComputedStyle(document.activeElement); return s.outlineStyle !== 'none' && parseFloat(s.outlineWidth) > 0"), 3)
        check("the focus is seen", ring)
        keys(browser, Keys.ENTER)
        listed = wait_for(browser, lambda b: b.find_element(By.ID, "list").is_displayed() and b.find_element(By.CSS_SELECTOR, "header.bar").is_displayed(), 10)
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
                away = wait_for(browser, lambda b: b.find_element(By.ID, "list").is_displayed() and asked == ["refused", "cut", "whole"][:round_ + 1], 5)
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
            told = wait_for(browser, lambda b: "Personne n’est choisi pour écrire le compte rendu" in notified.read_text(), 10)
            opened = wait_for(browser, lambda b: b.find_element(By.ID, "settings").is_displayed(), 5)
            check("connected, no service chosen after: the first meeting's minutes say none is, and open Settings",
                  told and opened and not (first / "minutes.md").exists() and "minutes_provider" not in window.file.read_text(),
                  (notified.read_text()[-300:], window.file.read_text()))
    finally:
        monitor.terminate()
        monitor.wait(timeout=10)
        for server in servers:
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
              and browser.find_element(By.CSS_SELECTOR, "#context-changed .says").text == "Le transcript et le compte rendu ont été faits avec un autre contexte."
              and browser.find_element(By.ID, "context-redo").text == "Refaire avec ce contexte", (meeting / "context.txt").read_text())
        seen(browser, "d59-4-page-changed-light")
        # Made again, slowly: the field closed meanwhile, and said.
        (WORK / "slow-hearing").touch()
        try:
            browser.find_element(By.ID, "context-redo").send_keys(Keys.ENTER)
            making = wait_for(browser, lambda b: b.find_element(By.ID, "context-text").get_attribute("readonly") is not None, 5)
            sentence = browser.find_element(By.ID, "context-said").text
            check("while it is made again, the field is closed, and says so",
                  making and sentence == "Le contexte ne change pas pendant que le transcript ou le compte rendu se fait."
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
              sentence == "Le transcript n’est pas terminé : terminez-le (Réessayer) avant de changer le contexte.", sentence)

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
        shown = wait_for(browser, lambda b: b.find_elements(By.ID, "confidential-switch"), 10)
        check("its page says it, the switch on, the lock among what it holds, nothing said to have left",
              shown and switch(browser, "confidential-switch").is_selected()
              and browser.find_element(By.CSS_SELECTOR, "#meeting .meeting-title .chip.locked").text == "Confidentielle"
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
        said = wait_for(browser, lambda b: b.execute_script(left) == "Déjà parti avant\u00a0: le transcript vers writes.example.", 5)
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
        check("in English, the list and the page say it", chips == ["Confidential"] and switch(browser, "confidential-switch").is_selected()
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
            said = wait_for(browser, lambda b: b.execute_script(left) == "Déjà parti avant\u00a0: le transcript vers writes.example.", 5)
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
        heard_here = [p.text for p in browser.find_elements(By.CSS_SELECTOR, "#meeting .made .maker")]
        check("a meeting heard here says so, by the model's name", heard_here == ["Écrit par Whisper large v3 turbo, sur cet ordinateur"], heard_here)
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
        check("the models on this computer are offered, by the names people say", names == ["Whisper large v3 turbo", "Whisper small", "OpenRouter", "OpenAI", "Un service que je choisis"], names)
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
        said = read(browser, lambda b: " ".join(p.text for p in b.find_elements(By.CSS_SELECTOR, "#minutes-where, #minutes-silent, #minutes-lacking")))
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
    list_refreshed(now)
    speech_offered(now)
    service_named(now)
    ollama_pulled(now)
    first_run(now)
    context_written(now)
    confidential_kept(now)
    return 1 if FAILED else 0


if __name__ == "__main__":
    # A scenario that breaks off says where, on the output the evidence keeps: its error stream
    # is not kept, and a run that failed once must say why.
    try:
        sys.exit(main())
    except Exception:
        traceback.print_exc(file=sys.stdout)
        sys.exit(1)
