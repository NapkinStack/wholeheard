#!/usr/bin/env bash
# The spec and the .deb's recipe, read: every file it installs is in %files and the other way round, the desktop entry
# carries the application's id (GNOME's shortcut portal finds Summit by it), and nothing is
# installed outside /usr — so removing the package never touches a user's meetings or settings.
set -euo pipefail
here=$(cd "$(dirname "$0")/.." && pwd)
python3 - "$here/summit.spec" "$here/deb/build.sh" "$here/deb/control.in" <<'PY'
import os, re, sys
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
# The .deb (D27): made by deb/build.sh from the same artefacts, it installs what the rpm installs.
build = open(sys.argv[2], encoding="utf-8").read()
control = open(sys.argv[3], encoding="utf-8").read()
deb = {"/usr/bin/" + p.split("/", 1)[1] for p in re.findall(r"^for program in (.+); do$", build, re.M)[0].split()}
deb |= {m.replace('"$root', "").rstrip('"') for m in re.findall(r'^install -Dm644 \S+ ("\$root/usr/\S+)$', build, re.M)}
check("the .deb installs what the rpm installs", deb == files, deb ^ files)
check("pw-metadata and the tray's library named in its dependencies: run and opened, never linked",
      re.search(r"pipewire-bin, libayatana-appindicator3-1 \| libappindicator3-1", build) is not None)
check("its control names the package, its version and its dependencies as computed",
      all(f"{field}: @" in control for field in ("Version", "Architecture", "Depends")) and "Package: summit" in control)
scripts = [f for f in os.listdir(os.path.dirname(sys.argv[3])) if f in ("preinst", "postinst", "prerm", "postrm", "triggers")]
check("no maintainer script: installing and removing touch nothing but the package's files", not scripts, scripts)
print("test: passed" if not failed else "test: FAILED")
sys.exit(failed)
PY
