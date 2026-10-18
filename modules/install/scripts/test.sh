#!/usr/bin/env bash
# The spec, read: every file it installs is in %files and the other way round, the desktop entry
# carries the application's id (GNOME's shortcut portal finds Summit by it), and nothing is
# installed outside /usr — so removing the package never touches a user's meetings or settings.
set -euo pipefail
here=$(cd "$(dirname "$0")/.." && pwd)
python3 - "$here/summit.spec" <<'PY'
import re, sys
spec = open(sys.argv[1], encoding="utf-8").read()
macros = {"%{_bindir}": "/usr/bin", "%{_datadir}": "/usr/share", "%{buildroot}": ""}
def expand(path):
    for macro, value in macros.items():
        path = path.replace(macro, value)
    return path
installed = {expand(m) for m in re.findall(r"^install -D\S* \S+ (\S+)$", spec, re.M)}
files = {expand(line.strip()) for line in spec.split("%files", 1)[1].splitlines() if line.strip()}
failed = 0
def check(what, held, got=""):
    global failed
    print(f"  {'ok  ' if held else 'FAIL'}  {what}" + ("" if held else f"\n        got: {got}"))
    failed |= not held
check("what is installed is what %files lists", installed == files, installed ^ files)
for program in ("summit", "summit-capture", "summit-transcription", "summit-summary"):
    check(f"{program} in /usr/bin, beside the others", f"/usr/bin/{program}" in files)
check("the desktop entry is named as the application's id", "/usr/share/applications/dev.summit.Summit.desktop" in files)
check("its icon, under the same name", "/usr/share/icons/hicolor/512x512/apps/dev.summit.Summit.png" in files)
check("nothing outside /usr: the user's meetings and settings are never the package's", all(f.startswith("/usr/") for f in files), files)
check("pw-metadata required: capture runs it, no library names it", re.search(r"^Requires:\s+pipewire-utils$", spec, re.M) is not None)
check("the tray's library required: the shell opens it at run time, no link names it",
      re.search(r"^Requires:.*\blib(ayatana-)?appindicator-gtk3\b", spec, re.M) is not None)
pairs = re.findall(r"^install -D\S* %\{artefacts\}/\S+/(\S+) %\{buildroot\}%\{_bindir\}/(\S+)$", spec, re.M)
check("each program installs its own artefact", len(pairs) == 4 and all(a == b for a, b in pairs), pairs)
scriptlets = re.findall(r"^%(pre|post|preun|postun|pretrans|posttrans|trigger\w*)\b", spec, re.M)
check("no scriptlet: installing and removing touch nothing but the package's files", not scriptlets, scriptlets)
print("test: passed" if not failed else "test: FAILED")
sys.exit(failed)
PY
