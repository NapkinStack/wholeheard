"""The window's end-to-end scenarios: Summit started with `--window` by tauri-driver, the page
read as WebKitGTK shows it, over WebDriver. Sessions are made here; nothing is recorded.

    python3 scripts/e2e_window.py <summit-binary> <work-folder>

Prints one line per check, `ok` or `FAIL`, and exits 1 on any failure.
"""

from __future__ import annotations

import os
import socket
import subprocess
import sys
import time
from pathlib import Path

from selenium import webdriver
from selenium.webdriver.common.by import By
from selenium.webdriver.common.keys import Keys
from selenium.webdriver.common.options import ArgOptions

BINARY, WORK = Path(sys.argv[1]), Path(sys.argv[2])
FAILED = []
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


class Window:
    """Summit with its window, its settings naming `sessions`, for as long as the `with` lasts."""

    def __init__(self, sessions: Path, language: str) -> None:
        config = WORK / f"config-{language}"
        (config / "summit").mkdir(parents=True, exist_ok=True)
        (config / "summit" / "settings.toml").write_text(f'sessions = "{sessions}"\n')
        self.env = dict(os.environ, XDG_CONFIG_HOME=str(config), XDG_DATA_HOME=str(WORK / "data"), LANGUAGE="")
        self.env.update(LANG=language, LC_ALL=language)

    def __enter__(self) -> webdriver.Remote:
        port, native = free_port(), free_port()
        self.driver = subprocess.Popen(
            ["tauri-driver", "--port", str(port), "--native-port", str(native)],
            env=self.env,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        options = ArgOptions()
        options.set_capability("browserName", "wry")
        options.set_capability("tauri:options", {"application": str(BINARY), "args": ["--window"]})
        for _ in range(50):
            try:
                self.browser = webdriver.Remote(f"http://127.0.0.1:{port}", options=options)
                break
            except Exception:
                time.sleep(0.2)
        else:
            self.driver.terminate()
            raise SystemExit("tauri-driver did not answer")
        wait_for(self.browser, lambda b: b.execute_script("return document.getElementById('meetings').textContent !== ''"))
        return self.browser

    def __exit__(self, *_) -> None:
        try:
            self.browser.quit()
        finally:
            self.driver.terminate()
            self.driver.wait(timeout=10)


def main() -> int:
    sessions = WORK / "sessions"
    sessions.mkdir(parents=True)

    print("H. the window: in the system's language, its theme, from the keyboard")
    with Window(sessions, "fr_FR.UTF-8") as browser:
        check("the section is named in French", browser.find_element(By.ID, "meetings").text == "Réunions")
        # The theme is the system's: the page's ground is that of the theme the system shows.
        dark = browser.execute_script("return matchMedia('(prefers-color-scheme: dark)').matches")
        ground = browser.execute_script("return getComputedStyle(document.body).backgroundColor")
        check(f"the ground of the system's {'dark' if dark else 'light'} theme", ground == GROUND["dark" if dark else "light"], ground)
        # Every action from the keyboard: the first stop of Tab is the section's link.
        browser.find_element(By.TAG_NAME, "body").send_keys(Keys.TAB)
        focused = browser.execute_script("return document.activeElement.id")
        check("Tab reaches the window's first action", focused == "meetings", focused)
    with Window(sessions, "en_GB.UTF-8") as browser:
        check("the section is named in English", browser.find_element(By.ID, "meetings").text == "Meetings")
    return 1 if FAILED else 0


if __name__ == "__main__":
    sys.exit(main())
