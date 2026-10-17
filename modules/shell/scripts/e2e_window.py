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

    def __init__(self, sessions: Path, language: str) -> None:
        config = WORK / f"config-{language}"
        (config / "summit").mkdir(parents=True, exist_ok=True)
        (config / "summit" / "settings.toml").write_text(f'sessions = "{sessions}"\n')
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


def main() -> int:
    now = dt.datetime.now().astimezone()
    sessions = WORK / "sessions"
    # Started an hour ago, a week ago, two months ago: this week, last week, a month by name.
    hour, week, months = now - dt.timedelta(hours=1), now - dt.timedelta(days=7), now - dt.timedelta(days=61)
    session(
        sessions / "a", week, 42, {"me.wav": 100_000_000, "others.wav": 100_000_000, "transcript.jsonl": 7000, "minutes.md": 500}, "v1"
    )
    session(sessions / "b", hour, None, {"local.wav": 2000})
    session(sessions / "c", months, 35, {"transcript.jsonl": 3000})
    (sessions / "not-a-session").mkdir()

    print("H. the window, in French: the meetings by week, what each holds and weighs")
    with Window(sessions, "fr_FR.UTF-8") as browser:
        check("the section is named in French", browser.find_element(By.ID, "meetings").text == "Réunions")
        lang = browser.execute_script("return document.documentElement.lang")
        check("the page says it is in French", lang == "fr", lang)
        sections = browser.find_element(By.TAG_NAME, "nav").get_attribute("aria-label")
        check("its sections are named in French for a screen reader", sections == "Rubriques", sections)
        listed = rows(browser)
        older = f"{MONTHS_FR[months.month - 1].capitalize()} {months.year}"
        check(
            "three sessions, newest first, each under its week",
            [(group, holds) for group, _, holds in listed]
            == [
                ("Cette semaine", "Interrompu — jamais arrêté|Audio · 2 ko"),
                ("La semaine dernière", "42 min|Compte rendu|Transcript|Audio · 200 Mo"),
                (older, "35 min|Transcript|Audio supprimé"),
            ],
            listed,
        )
        total = browser.find_element(By.ID, "weight").text
        check("what they weigh, audio against the rest", total.startswith("200 Mo occupés par 3 réunions · 42 min d’audio"), total)
        check("the picture of the weights is described", "L’audio occupe 200 Mo" in (browser.find_element(By.ID, "bar").get_attribute("aria-label") or ""))
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
        came = wait_for(browser, lambda b: len(rows(b)) == 4)
        check("a new session is listed when the window comes forward", came, rows(browser))

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
