"""The window's end-to-end scenario: Summit started with `--window` by tauri-driver, the page
read as WebKitGTK shows it, over WebDriver. Sessions are made here; nothing is recorded.

    python3 scripts/e2e_window.py <summit-binary> <work-folder>

Prints one line per check, `ok` or `FAIL`, and exits 1 on any failure.
"""

from __future__ import annotations

import datetime as dt
import json
import os
import socket
import subprocess
import sys
import time
from pathlib import Path

from selenium import webdriver
from selenium.common.exceptions import WebDriverException
from selenium.webdriver.common.by import By
from selenium.webdriver.common.keys import Keys
from selenium.webdriver.common.options import ArgOptions
from selenium.webdriver.support.expected_conditions import staleness_of
from selenium.webdriver.support.select import Select
from selenium.webdriver.support.wait import WebDriverWait
from urllib3.exceptions import HTTPError

BINARY, WORK = Path(sys.argv[1]), Path(sys.argv[2])
FAILED = []
MONTHS_FR = "janvier février mars avril mai juin juillet août septembre octobre novembre décembre".split()
# The page's ground, as `ui/window.css` sets it in each theme.
GROUND = {"light": "rgb(242, 244, 247)", "dark": "rgb(21, 23, 28)"}


def check(what: str, held: bool, got: object = "") -> None:
    print(f"  {'ok  ' if held else 'FAIL'}  {what}" + ("" if held else f"\n        got: {got}"))
    if not held:
        FAILED.append(what)


def wait_for(browser, holds, seconds: float = 10) -> bool:
    until = time.monotonic() + seconds
    while time.monotonic() < until:
        if holds(browser):
            return True
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
        with open(folder / name, "wb") as made:
            made.truncate(size)


class Window:
    """Summit with its window, for as long as the `with` lasts."""

    def __init__(self, sessions: Path, language: str, more: str = "") -> None:
        config = WORK / f"config-{language}-{sessions.name}"
        (config / "summit").mkdir(parents=True, exist_ok=True)
        self.file = config / "summit" / "settings.toml"
        self.file.write_text(f'sessions = "{sessions}"\n{more}')
        self.env = dict(os.environ, XDG_CONFIG_HOME=str(config), XDG_DATA_HOME=str(WORK / "data"), LANGUAGE="")
        self.env.update(LANG=language, LC_ALL=language)

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
        focused = browser.execute_script("return document.activeElement.className")
        check("Tab reaches the first meeting after the two sections", "session" in focused, focused)
        browser.find_elements(By.CSS_SELECTOR, ".session")[0].click()
        shown = wait_for(browser, lambda b: b.find_element(By.ID, "meeting").is_displayed() and b.find_elements(By.CSS_SELECTOR, ".lines li"))
        check("a click on a meeting opens it", shown and not browser.find_element(By.ID, "list").is_displayed())
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
            "const s = document.querySelector('.relief'); return [s.getAttribute('aria-label'),"
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
        check("who wrote the minutes, and where", maker.startswith("Écrit par claude-sonnet, via openrouter.ai · "), maker)
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
        check("the folder can be opened from the meeting", browser.find_element(By.CSS_SELECTOR, ".open").text == "Ouvrir le dossier")
        browser.find_element(By.CSS_SELECTOR, ".back").click()
        back = wait_for(browser, lambda b: b.find_element(By.ID, "list").is_displayed())
        check("the way back is the list", back and not browser.find_element(By.ID, "meeting").is_displayed())
        browser.find_elements(By.CSS_SELECTOR, ".session")[1].click()
        wait_for(browser, lambda b: b.find_element(By.ID, "meeting").is_displayed() and b.find_elements(By.CSS_SELECTOR, ".minutes .none"))
        kept = browser.execute_script("return document.getElementById('search') && document.getElementById('search').value")
        check("another meeting opens without the last one's search", not kept, kept)
        notes = [p.text for p in browser.find_elements(By.CSS_SELECTOR, "#meeting .none") if p.is_displayed()]
        check("minutes that cannot be read are said so", notes == ["Ce fichier ne peut pas être lu par cette version de Summit."], notes)
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
        checked = browser.execute_script("return document.querySelector('input[name=after]:checked').value")
        check("out of the box, the transcript follows", checked == "transcript", checked)
        names = [o.text.split(" — ")[0] for o in browser.find_elements(By.CSS_SELECTOR, "#model option")]
        check("the models on this computer are offered", names == ["ggml-large-v3-turbo-q8_0.bin", "ggml-small.bin"], names)
        # Each change is saved, then the page is drawn again: the next acts on the new one.
        def changed(act):
            before = browser.find_element(By.ID, "reminder")
            act()
            WebDriverWait(browser, 5).until(staleness_of(before))

        changed(lambda: browser.find_element(By.CSS_SELECTOR, "input[name=after][value=nothing]").click())
        changed(lambda: Select(browser.find_element(By.ID, "keep")).select_by_value("30"))
        changed(lambda: Select(browser.find_element(By.ID, "model")).select_by_visible_text(
            next(o.text for o in browser.find_elements(By.CSS_SELECTOR, "#model option") if o.text.startswith("ggml-small"))
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
