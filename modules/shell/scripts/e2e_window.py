"""The window's end-to-end scenario: Summit started with `--window` by tauri-driver, the page
read as WebKitGTK shows it, over WebDriver. Sessions are made here; nothing is recorded.

    python3 scripts/e2e_window.py <summit-binary> <work-folder>

Prints one line per check, `ok` or `FAIL`, and exits 1 on any failure.
"""

from __future__ import annotations

import datetime as dt
import http.server
import json
import os
import shutil
import socket
import subprocess
import sys
import threading
import time
from pathlib import Path

from selenium import webdriver
from selenium.common.exceptions import NoSuchWindowException, StaleElementReferenceException, WebDriverException
from selenium.webdriver.common.by import By
from selenium.webdriver.common.keys import Keys
from selenium.webdriver.common.options import ArgOptions
from selenium.webdriver.support.expected_conditions import staleness_of
from selenium.webdriver.support.select import Select
from selenium.webdriver.support.wait import WebDriverWait
from urllib3.exceptions import HTTPError

BINARY, WORK = Path(sys.argv[1]), Path(sys.argv[2])
# summit-summary played here: it says what it was given — its arguments, the key in its
# environment. A test refuses a key at openrouter.ai and answers elsewhere; minutes are written,
# the way it was called kept beside them. Nothing leaves the machine.
SUMMARY = WORK / "summit-summary"
SUMMARY.write_text(
    """#!/bin/sh
here=$(dirname "$0")
printf '%s\\n' "$*" > "$here/summary-args"
printf '%s' "${SUMMIT_PROVIDER_KEY:-none}" > "$here/summary-key"
if [ "$1" != --test ]; then
  [ -e "$here/slow-minutes" ] && sleep 3
  [ -e "$here/fail-minutes" ] && { echo "summit-summary: the provider's answer is not minutes" >&2; exit 1; }
  echo '{"event": "sending", "host": "h", "model": "m", "characters": 1}'
  printf -- '---\\ncontract: minutes/v1\\nmodel: %s\\n---\\n\\n## A\\n\\n## B\\n\\n## C\\n' "$5" > "$1/minutes.md"
  echo '{"event": "written", "file": "x"}'
  exit 0
fi
case "$*" in *openrouter.ai*) echo "summit-summary: the provider at openrouter.ai refused the key" >&2; exit 1;; esac
echo '{"event": "sending"}'
echo '{"event": "answered"}'
"""
)
SUMMARY.chmod(0o755)
# summit-transcription played here: a whole transcript, naming the model it was given.
HEARING = WORK / "summit-transcription"
HEARING.write_text(
    """#!/bin/sh
[ -e "$(dirname "$0")/fail-hearing" ] && { echo "summit-transcription: the engine stopped" >&2; exit 1; }
model=$(basename "$3" .bin)
printf '{"type":"header","contract":"transcript/v1","engine":"e","model":"%s","language":null,"context":null}\\n{"type":"segment","speaker":"me","track":"local","start":0.0,"end":1.0,"text":"Refait avec %s."}\\n{"type":"complete","duration":1.0}\\n' "$model" "$model" > "$1/transcript.jsonl"
"""
)
HEARING.chmod(0o755)
# summit-capture played here: it says it started, then each source's level ten times a second,
# until it is told to stop. No device, nobody recorded.
RECORDER = WORK / "summit-capture"
RECORDER.write_text(
    """#!/bin/bash
folder="$2/2026-10-18_12-00"; mkdir -p "$folder"
printf '{"contract":"session-audio/v2","started":"2026-10-18T12:00:00+02:00","tracks":{}}' > "$folder/session.json"
echo '{"event":"started","folder":"'"$folder"'","sources":{"me":"Mic","others":"Speaker"}}'
n=0
while ! read -t 0.1 line; do
  n=$((n + 1)); echo '{"event":"level","source":"me","level":0.'$((n % 9 + 1))'}'; echo '{"event":"level","source":"others","level":0.5}'
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
    """The page as it is, saved where SUMMIT_SCREENSHOTS says: for an eye against D11's boards."""
    folder = os.environ.get("SUMMIT_SCREENSHOTS")
    if folder:
        Path(folder).mkdir(parents=True, exist_ok=True)
        browser.save_screenshot(str(Path(folder) / f"{name}.png"))


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
    """Summit with its window, for as long as the `with` lasts."""

    def __init__(self, sessions: Path, language: str, more: str = "", ollama: str = "http://127.0.0.1:9") -> None:
        config = WORK / f"config-{language}-{sessions.name}"
        (config / "summit").mkdir(parents=True, exist_ok=True)
        self.file = config / "summit" / "settings.toml"
        self.file.write_text(f'sessions = "{sessions}"\n{more}')
        self.env = dict(os.environ, XDG_CONFIG_HOME=str(config), XDG_DATA_HOME=str(WORK / "data"), LANGUAGE="")
        # An Ollama and a keyring of the scenario's, never the user's own.
        self.env.update(LANG=language, LC_ALL=language, SUMMIT_OLLAMA=ollama, SUMMIT_KEYRING="memory", SUMMIT_SUMMARY=str(SUMMARY), SUMMIT_TRANSCRIPTION=str(HEARING), SUMMIT_CAPTURE=str(RECORDER))

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
        wait_for(self.browser, lambda b: b.execute_script("return document.getElementById('meetings').textContent !== ''"))
        return self.browser

    def __exit__(self, *_) -> None:
        try:
            self.browser.quit()
        finally:
            self.driver_process.terminate()
            self.driver_process.wait(timeout=10)


def rows(browser) -> list[tuple[str, str, str]]:
    """Each listed session: its group's heading, its first line, and what it holds."""
    return browser.execute_script(
        """return [...document.querySelectorAll('.group')].flatMap(g => [...g.querySelectorAll('.session')].map(s =>
             [g.querySelector('h2').textContent, s.querySelector('.day').textContent,
              [...s.querySelectorAll('.length, .chip')].map(c => c.textContent).join('|')]))"""
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
        "## Points saillants\n\nL’ouverture est fixée au lundi 16 novembre.\n\n"
        "## Actions\n\n- Mettre à jour le planning — Marc — aujourd’hui\n- Couper deux serveurs de test — Julien — avant vendredi\n\n"
        "## Points clés\n\n- Un serveur est gardé pour la démonstration du 20.\n"
    )
    session(meetings / "n", now - dt.timedelta(days=1), 5, {"local.wav": 100})
    (meetings / "n" / "minutes.md").write_text("Not minutes.\n")
    (meetings / "n" / "transcript.jsonl").write_text(
        json.dumps(lines[0]) + "\n" + json.dumps({"type": "segment", "speaker": "me", "track": "local", "voice": None, "start": 1.0, "end": 2.0, "text": "Une autre réunion."}) + "\n"
    )
    with Window(meetings, "fr_FR.UTF-8") as browser:
        browser.find_element(By.TAG_NAME, "body").send_keys(Keys.TAB, Keys.TAB, Keys.TAB)
        focused = browser.execute_script("return document.activeElement.id")
        check("Tab reaches the header's gesture after the two sections", focused == "record", focused)
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
        check("the minutes' three sections, by their own titles", titles == ["Points saillants", "Actions", "Points clés"], titles)
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
        check("minutes that cannot be read are said so", notes == ["Ce fichier ne peut pas être lu par cette version de Summit."], notes)
        chips = browser.find_element(By.CSS_SELECTOR, "#meeting .holds").text
        check("minutes that cannot be read claim no place", "Compte rendu" in chips and "Compte rendu ·" not in chips, chips)
        being = [p.text for p in browser.find_elements(By.CSS_SELECTOR, ".transcript .note")]
        check("a transcript without its end says it is being written", being == ["En cours d’écriture — ce qui est là est vrai quand même."], being)

        browser.execute_script("location.hash = '#/meeting/' + encodeURIComponent('gone')")
        listed = wait_for(browser, lambda b: b.find_element(By.ID, "list").is_displayed())
        check("a meeting that is not there goes back to the list", listed and not browser.find_element(By.ID, "meeting").is_displayed())

    print("K. the settings: each choice saved at once, the user's file kept")
    models = WORK / "data" / "summit" / "models"
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

        changed(lambda: browser.find_element(By.CSS_SELECTOR, "input[name=after][value=nothing]").click())
        changed(lambda: Select(browser.find_element(By.ID, "keep")).select_by_value("30"))
        changed(lambda: Select(browser.find_element(By.ID, "model")).select_by_visible_text(
            next(o.text for o in browser.find_elements(By.CSS_SELECTOR, "#model option") if o.text.startswith("Whisper small"))
        ))
        # The user changes the file by hand while Summit runs: the next save keeps it.
        window.file.write_text(window.file.read_text().replace("keep_audio = 30", "keep_audio = 90 # by hand"))
        changed(lambda: browser.find_element(By.ID, "reminder").click())
        text = window.file.read_text()
        check(
            "each choice is in the settings file, the user's lines kept, a change by hand too",
            'after = "nothing"' in text
            and "reminder = false" in text
            and "keep_audio = 90 # by hand" in text
            and f'model = "{models / "ggml-small.bin"}"' in text
            and text.startswith(f'sessions = "{chosen}"\n')
            and "# mine" in text,
            text,
        )
        failed = browser.find_element(By.ID, "not-saved").is_displayed()
        check("nothing said failed", not failed)
        # Any number of days, when the list has not the one wanted.
        Select(browser.find_element(By.ID, "keep")).select_by_value("other")
        typed = browser.find_element(By.ID, "keep-other")
        changed(lambda: (typed.send_keys("45"), typed.send_keys(Keys.ENTER)))
        kept = window.file.read_text()
        offered = [o.text for o in browser.find_elements(By.CSS_SELECTOR, "#keep option")]
        check("any number of days can be typed, and is offered after", "keep_audio = 45" in kept and "45 jours" in offered, (kept, offered))
        Select(browser.find_element(By.ID, "keep")).select_by_value("other")
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
        elsewhere = WORK / "elsewhere" / "Summit"
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
        changed(lambda: typed(folder, "Summit"))
        said = browser.find_element(By.ID, "not-saved").text
        check("a folder that is not a whole path is said, and nothing changes", said.startswith("Le réglage n’a pas été enregistré") and window.file.read_text() == text, said)
        browser.find_element(By.ID, "meetings").click()
        listed = wait_for(browser, lambda b: str(elsewhere) in b.find_element(By.ID, "none").text, 5)
        check("the list reads the folder chosen", listed, browser.find_element(By.ID, "none").text)
        browser.find_element(By.ID, "settings-link").click()
        wait_for(browser, lambda b: b.find_elements(By.ID, "reminder"))
        # A file broken by hand is never written over: the change is said not saved.
        window.file.write_text(window.file.read_text() + "keep_audio = 0\n")
        broken = window.file.read_text()
        changed(lambda: browser.find_element(By.ID, "reminder").click())
        said = browser.find_element(By.ID, "not-saved").text
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
        before = browser.find_element(By.ID, "minutes")
        Select(before).select_by_visible_text("mistral:7b")
        WebDriverWait(browser, 5).until(staleness_of(before))
        local = wait_for(browser, lambda b: "mistral:7b" in window.file.read_text(), 5)
        check(
            "a model of Ollama is chosen at once",
            local and f'minutes_provider = "{ollama}/v1"' in window.file.read_text(),
            window.file.read_text(),
        )
        # Tester asks summit-summary about the choice saved: no key for Ollama here.
        browser.find_element(By.ID, "minutes-test").click()
        answered = wait_for(browser, lambda b: b.find_element(By.ID, "minutes-tested").text == "Le modèle répond.", 10)
        check(
            "Tester says the model answers, asking summit-summary with the choice saved",
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
        browser.find_element(By.ID, "minutes-test").click()
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
    window = Window(broken, "fr_FR.UTF-8", "keep_audio = 0\n")
    with window as browser:
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
    (WORK / "summary-args").unlink(missing_ok=True)
    online = 'minutes_provider = "https://openrouter.ai/api/v1"\nminutes_model = "a-model"\n'
    window = Window(agreeing, "fr_FR.UTF-8", online, ollama=ollama)
    with window as browser:
        browser.execute_script("location.hash = '#/agree/2026-10-18_11-00'")
        wait_for(browser, lambda b: b.find_elements(By.ID, "agree-send"))
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
        browser.execute_script("location.hash = '#/agree/2026-10-18_11-00'")
        wait_for(browser, lambda b: b.find_elements(By.ID, "agree-send"))
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
        browser.execute_script("location.hash = '#/agree/2026-10-18_11-00'")
        wait_for(browser, lambda b: b.find_elements(By.ID, "agree-remember"))
        press(browser, "agree-remember")
        press(browser, "agree-send")
        kept = wait_for(browser, lambda b: (asked / "minutes.md").exists(), 10)
        check("ticked, the host is kept for the next ones", kept and 'minutes_agreed = "openrouter.ai"' in window.file.read_text(), window.file.read_text())
        # On this computer instead: nothing goes to the host, nothing is agreed.
        (asked / "minutes.md").unlink()
        (WORK / "summary-args").unlink()
        window.file.write_text(window.file.read_text().replace('minutes_agreed = "openrouter.ai"\n', ""))
        browser.execute_script("location.hash = '#/agree/2026-10-18_11-00'")
        wait_for(browser, lambda b: b.find_elements(By.ID, "agree-local-open"))
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
        # A meeting without its minutes offers them on its page, made on the word.
        (asked / "minutes.md").unlink()
        browser.execute_script("location.hash = '#/'")
        browser.execute_script("location.hash = '#/meeting/2026-10-18_11-00'")
        wait_for(browser, lambda b: b.find_elements(By.ID, "make-minutes"))
        chips = browser.find_element(By.CSS_SELECTOR, "#meeting .holds").text
        check("the meeting's page says where its transcript was made", "Transcript · sur cette machine" in chips, chips)
        (WORK / "slow-minutes").touch()
        browser.find_element(By.ID, "make-minutes").click()
        said = browser.find_element(By.CSS_SELECTOR, "#meeting .minutes").text
        # While its minutes are written, the meeting is busy: shown, never offered to be freed.
        browser.execute_script("location.hash = '#/free'")
        held = wait_for(browser, lambda b: [r.text for r in b.find_elements(By.CSS_SELECTOR, "#free .freeable.busy")], 3)
        busy = [r.text for r in browser.find_elements(By.CSS_SELECTOR, "#free .freeable.busy")]
        check("while its minutes are written, the meeting is busy on the page that frees space", held and "compte rendu" in " ".join(busy), busy)
        browser.execute_script("location.hash = '#/meeting/2026-10-18_11-00'")
        made = wait_for(browser, lambda b: (asked / "minutes.md").exists(), 10)
        (WORK / "slow-minutes").unlink()
        check("what a meeting lacks is offered, and made on the word", made and "Demandé" in said, said)
        # Refaire…: another model, this time only; the minutes there go to the trash once replaced.
        browser.execute_script("location.hash = '#/'")
        browser.execute_script("location.hash = '#/meeting/2026-10-18_11-00'")
        wait_for(browser, lambda b: b.find_elements(By.ID, "redo-minutes"))
        browser.find_element(By.ID, "redo-minutes").click()
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
            (WORK / "summary-args").read_text(),
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
        browser.find_element(By.ID, "redo-minutes").click()
        wait_for(browser, lambda b: b.find_elements(By.ID, "redo-do"))
        browser.find_elements(By.CSS_SELECTOR, "#redo-panel input[name=redo-with]")[1].click()
        browser.find_element(By.ID, "redo-do").click()
        told = wait_for(browser, lambda b: "pas été refait" in heard.read_text(), 10)
        check("minutes made again that fail: the ones there stay, and the notice says a redo", told and (asked / "minutes.md").read_text() == before, heard.read_text()[-400:])
        settled = window.file.read_text()
        window.file.write_text(
            "\n".join(line for line in settled.splitlines() if not line.startswith(("minutes_provider", "minutes_model", "minutes_agreed")))
            + '\nminutes_provider = "https://openrouter.ai/api/v1"\nminutes_model = "a-model"\n'
        )
        browser.execute_script("location.hash = '#/'")
        browser.execute_script("location.hash = '#/meeting/2026-10-18_11-00'")
        wait_for(browser, lambda b: b.find_elements(By.ID, "redo-minutes"))
        browser.find_element(By.ID, "redo-minutes").click()
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
        (WORK / "fail-minutes").unlink()
        check("asked through the agreement, still a redo: its failure said as one", told and "pas été fait" not in said and (asked / "minutes.md").read_text() == before, said[-400:])
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
        browser.find_element(By.ID, "redo-minutes").click()
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
        browser.find_element(By.ID, "redo-minutes").click()
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
        browser.find_element(By.ID, "redo-transcript").click()
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
        browser.find_element(By.ID, "redo-transcript").click()
        wait_for(browser, lambda b: b.find_elements(By.ID, "redo-transcript-do"))
        browser.find_element(By.ID, "redo-transcript-do").click()
        time.sleep(3)
        (WORK / "fail-hearing").unlink()
        check(
            "a new transcript that fails gives the one there back, nothing left aside",
            (asked / "transcript.jsonl").read_text() == heard and not sorted(asked.glob("transcript-before-*")),
            (asked / "transcript.jsonl").read_text(),
        )
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

    print("Q. while recording, the small window: the time, the two levels, stop — opened on a gesture only")
    taping = WORK / "taping"
    taping.mkdir()
    with Window(taping, "fr_FR.UTF-8", 'after = "nothing"\nreminder = false\n') as browser:
        main = browser.current_window_handle
        browser.find_element(By.ID, "record").click()
        offered = wait_for(browser, lambda b: b.find_element(By.ID, "small-link").is_displayed(), 5)
        check("while recording, the header offers the small window — none opened by itself", offered and browser.window_handles == [main], browser.window_handles)
        browser.find_element(By.ID, "small-link").click()
        opened = wait_for(browser, lambda b: len(b.window_handles) == 2, 5)
        check("the small window opens on the gesture", opened, browser.window_handles)
        browser.switch_to.window([h for h in browser.window_handles if h != main][0])
        wait_for(browser, lambda b: b.find_elements(By.ID, "small-stop"))
        time.sleep(1.5)
        shot(browser, "6-small")
        fits = browser.execute_script("const e = document.scrollingElement; return [e.scrollHeight, e.clientHeight];")
        check("it holds in its window, nothing to scroll", fits[0] <= fits[1], fits)
        moving = {browser.find_element(By.ID, "level-me").get_attribute("style") for _ in range(6) if time.sleep(0.15) is None}
        clock = browser.find_element(By.CSS_SELECTOR, "#small .time").text
        check("it shows the time recorded and the levels, moving as sound comes", clock.startswith("00:00:0") and clock != "00:00:00" and len(moving) > 1, (clock, moving))
        check("the meeting's level too", "50%" in browser.find_element(By.ID, "level-others").get_attribute("style"))
        check("nothing follows the stop here: the small window does not say the transcript writes itself", not browser.find_element(By.CSS_SELECTOR, "#small .says").is_displayed())
        stop_from_small(browser)
        browser.switch_to.window(main)
        closed = wait_for(browser, lambda b: len(b.window_handles) == 1, 5)
        stopped = '"stopped"' in (taping / "2026-10-18_12-00" / "session.json").read_text()
        check("Arrêter stops the recording, and the small window goes", closed and stopped, browser.window_handles)
        # The header's offer is hidden by a poll: a click after the stop opens nothing.
        browser.execute_script('window.__TAURI__.core.invoke("small_window")')
        time.sleep(1)
        check("once stopped, nothing opens it again", len(browser.window_handles) == 1, browser.window_handles)
    taping_en = WORK / "taping-en"
    taping_en.mkdir()
    with Window(taping_en, "en_GB.UTF-8", "reminder = false\n") as browser:
        main = browser.current_window_handle
        browser.find_element(By.ID, "record").click()
        wait_for(browser, lambda b: b.find_element(By.ID, "small-link").is_displayed(), 5)
        browser.find_element(By.ID, "small-link").click()
        wait_for(browser, lambda b: len(b.window_handles) == 2, 5)
        browser.switch_to.window([h for h in browser.window_handles if h != main][0])
        wait_for(browser, lambda b: b.find_elements(By.ID, "small-stop"))
        time.sleep(1)
        fits = browser.execute_script("const e = document.scrollingElement; return [e.scrollHeight, e.clientHeight];")
        said = browser.find_element(By.CSS_SELECTOR, "#small .said").text
        check("in English too, it holds in its window", said == "Your microphone" and fits[0] <= fits[1], (said, fits))
        starts = browser.execute_script("return [...document.querySelectorAll('#small .track')].map(t => t.getBoundingClientRect().left)")
        check("the two meters start at the same place", len(starts) == 2 and starts[0] == starts[1], starts)
        note = browser.find_element(By.CSS_SELECTOR, "#small .says")
        check("the transcript follows the stop: the small window says so", note.is_displayed() and "transcript" in note.text, note.text)
        stop_from_small(browser)
        browser.switch_to.window(main)
        wait_for(browser, lambda b: len(b.window_handles) == 1, 5)

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
    return 1 if FAILED else 0


if __name__ == "__main__":
    sys.exit(main())
