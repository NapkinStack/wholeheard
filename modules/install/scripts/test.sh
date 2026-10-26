#!/usr/bin/env bash
# The spec, the .deb's recipe and the Mac's app, read: every file it installs is in %files and the other way round, the desktop entry
# carries the application's id (GNOME's shortcut portal finds Wholeheard by it), and nothing is
# installed outside /usr — so removing the package never touches a user's meetings or settings.
set -euo pipefail
here=$(cd "$(dirname "$0")/.." && pwd)
python3 - "$here/wholeheard.spec" "$here/deb/build.sh" "$here/deb/control.in" "$here/mac" <<'PY'
import json, os, plistlib, re, subprocess, sys, tempfile
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
for program in ("wholeheard", "wholeheard-capture", "wholeheard-transcription", "wholeheard-summary"):
    check(f"{program} in /usr/bin, beside the others", f"/usr/bin/{program}" in files)
check("the desktop entry is named as the application's id", "/usr/share/applications/dev.wholeheard.Wholeheard.desktop" in files)
check("its icon, under the same name", "/usr/share/icons/hicolor/512x512/apps/dev.wholeheard.Wholeheard.png" in files)
check("nothing outside /usr: the user's meetings and settings are never the package's", all(f.startswith("/usr/") for f in files), files)
check("pw-metadata required: capture runs it, no library names it", re.search(r"^Requires:\s+pipewire-utils$", spec, re.M) is not None)
check("the tray's library required: the shell opens it at run time, no link names it",
      re.search(r"^Requires:.*\blib(ayatana-)?appindicator-gtk3\b", spec, re.M) is not None)
pairs = re.findall(r"^install -D\S* %\{artefacts\}/\S+/(\S+) %\{buildroot\}%\{_bindir\}/(\S+)$", spec, re.M)
check("each program installs its own artefact", len(pairs) == 4 and all(a == b for a, b in pairs), pairs)
scriptlets = re.findall(r"^%(pre|post|preun|postun|pretrans|posttrans|trigger\w*)\b", spec, re.M)
check("no scriptlet: installing and removing touch nothing but the package's files", not scriptlets, scriptlets)
# The .deb (D27): made by deb/build.sh from the same artefacts — what it installs is judged on the
# package installed, by the e2e, against the spec's %files.
build = open(sys.argv[2], encoding="utf-8").read()
control = open(sys.argv[3], encoding="utf-8").read()
check("pw-metadata and the tray's library named in its dependencies: run and opened, never linked",
      re.search(r"pipewire-bin, libayatana-appindicator3-1 \| libappindicator3-1", build) is not None)
check("its control names the package, its version and its dependencies as computed",
      all(f"{field}: @" in control for field in ("Version", "Architecture", "Depends")) and "Package: wholeheard" in control)
scripts = [f for f in os.listdir(os.path.dirname(sys.argv[3])) if f in ("preinst", "postinst", "prerm", "postrm", "triggers")]
check("no maintainer script: installing and removing touch nothing but the package's files", not scripts, scripts)
check("the graphics card's Vulkan driver recommended by both: the transcript is made there",
      re.search(r"^Recommends:\s+mesa-vulkan-drivers$", spec, re.M) is not None and "mesa-vulkan-drivers" in control)
check("Wholeheard takes Summit's place: the rpm obsoletes it, the .deb breaks and replaces it (D28)",
      re.search(r"^Obsoletes:\s+summit < 0\.2\.0$", spec, re.M) is not None
      and "Breaks: summit (<< 0.2.0)" in control and "Replaces: summit (<< 0.2.0)" in control)
# The Mac's app (D38): made on a Mac by mac/build.sh; its Info.plist and French strings written
# here by the same mac/info.py, and read.
mac = sys.argv[4]
macbuild = open(os.path.join(mac, "build.sh"), encoding="utf-8").read()
for program in ("shell/wholeheard", "capture/wholeheard-capture", "transcription/wholeheard-transcription", "summary/wholeheard-summary",
                # Apple's SpeechAnalyzer, through transcription's Swift program (D42, ADR-0012).
                "transcription/wholeheard-speech"):
    check(f"{program.split('/')[1]} in Contents/MacOS, beside the others", program in macbuild)
each, bundle = "codesign --force --sign - \"$program\"", "codesign --force --sign - \"$app\""
check("signed ad hoc, each program then the bundle, last of all: its seal covers what is in it",
      each in macbuild and bundle in macbuild and macbuild.index(each) < macbuild.index(bundle)
      and all(macbuild.index(step) < macbuild.index(bundle) for step in ("info.py", "iconutil")))
with tempfile.TemporaryDirectory() as work:
    app = os.path.join(work, "Wholeheard.app")
    subprocess.run([sys.executable, os.path.join(mac, "info.py"), app, "1.2.3"], check=True)
    info = plistlib.load(open(os.path.join(app, "Contents", "Info.plist"), "rb"))
    check("the Info.plist names the application, its id and its version",
          info.get("CFBundleName") == "Wholeheard" and info.get("CFBundleIdentifier") == "dev.wholeheard.Wholeheard"
          and info.get("CFBundleShortVersionString") == "1.2.3", info)
    check("it opens the shell, the program that finds the others beside it", info.get("CFBundleExecutable") == "wholeheard", info)
    words = json.load(open(os.path.join(mac, "words.json"), encoding="utf-8"))
    asked = ("NSMicrophoneUsageDescription", "NSAudioCaptureUsageDescription")
    check("macOS asks for the microphone and the sound the Mac plays with Wholeheard's sentences",
          all(words["en"].get(key) and info.get(key) == words["en"][key] for key in asked), info)
    strings = open(os.path.join(app, "Contents", "Resources", "fr.lproj", "InfoPlist.strings"), encoding="utf-8").read()
    check("and in French, from the same file", all(words["fr"].get(key) and f'{key} = "{words["fr"][key]}";' in strings for key in asked) and
          info.get("CFBundleLocalizations") == ["en", "fr"], strings)
print("test: passed" if not failed else "test: FAILED")
sys.exit(failed)
PY
