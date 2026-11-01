#!/usr/bin/env bash
# The spec, the .deb's recipe and the Mac's app, read: every file it installs is in %files and the other way round, the desktop entry
# carries the application's id (GNOME's shortcut portal finds Wholeheard by it), and nothing is
# installed outside /usr — so removing the package never touches a user's meetings or settings.
set -euo pipefail
here=$(cd "$(dirname "$0")/.." && pwd)
python3 - "$here/wholeheard.spec" "$here/deb/build.sh" "$here/deb/control.in" "$here/mac" <<'PY'
import json, os, plistlib, re, subprocess, sys, tempfile, zipfile
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
# Windows' zip (D53): made on Windows by windows/build.py, run here on artefacts of its names.
windows = os.path.join(os.path.dirname(mac), "windows", "build.py")
with tempfile.TemporaryDirectory() as work:
    artefacts, out = os.path.join(work, "artefacts"), os.path.join(work, "out")
    names = ("shell/wholeheard.exe", "capture/wholeheard-capture.exe",
             "transcription/wholeheard-transcription.exe", "summary/wholeheard-summary.exe")
    for name in names:
        os.makedirs(os.path.join(artefacts, os.path.dirname(name)), exist_ok=True)
        open(os.path.join(artefacts, name), "wb").write(name.encode())
    os.makedirs(out)
    subprocess.run([sys.executable, windows, artefacts, "1.2.3", out], check=True, stdout=subprocess.DEVNULL)
    made = os.listdir(out)
    check("Windows' zip is named by its version and processor", made == ["Wholeheard-1.2.3-windows-x64.zip"], made)
    inside = sorted(zipfile.ZipFile(os.path.join(out, made[0])).namelist()) if made else []
    check("the four programs side by side in one folder, where the shell looks for them",
          inside == sorted(f"Wholeheard/{name.split('/')[1]}" for name in names), inside)
# Windows' package (D54): made on Windows by windows/msix.py; its manifest and what it holds, read here.
sys.dont_write_bytecode = True
sys.path.insert(0, os.path.join(os.path.dirname(mac), "windows"))
import msix
from xml.etree import ElementTree
ns = {"m": "http://schemas.microsoft.com/appx/manifest/foundation/windows10",
      "uap": "http://schemas.microsoft.com/appx/manifest/uap/windows10",
      "rescap": "http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities"}
uap10 = "{http://schemas.microsoft.com/appx/manifest/uap/windows10/10}"
identity = msix.identity()
written = ElementTree.fromstring(msix.manifest("1.2.3", identity))
said = written.find("m:Identity", ns).attrib
check("the package's version in four parts, the last 0: the Store keeps it", said.get("Version") == "1.2.3.0", said)
for wrong in ("1.2", "1.2.3.4", "1.2.3-rc1", "70000.0.0"):
    try:
        msix.four_parts(wrong)
        check(f"a version not of three numbers under 65536 is refused: {wrong}", False)
    except ValueError:
        check(f"a version not of three numbers under 65536 is refused: {wrong}", True)
check("the identity named is the one identity.json holds, for x64",
      said.get("Name") == identity["name"] and said.get("Publisher") == identity["publisher"]
      and said.get("ProcessorArchitecture") == "x64", said)
check("Wholeheard is its name", written.findtext("m:Properties/m:DisplayName", namespaces=ns) == "Wholeheard")
placeholder = {**identity, "placeholder": True}
check("a placeholder identity is said, and that the package is not to be submitted",
      any("placeholder" in line and "not submitted" in line for line in msix.notices("1.2.3", placeholder)))
check("a reserved identity is not said to be one",
      not any("placeholder" in line for line in msix.notices("1.2.3", {**identity, "placeholder": False})))
check("a first part 0 is said: the Store refuses it", any("first part" in line for line in msix.notices("0.2.0", identity))
      and not any("first part" in line for line in msix.notices("1.2.0", identity)))
caps = written.find("m:Capabilities", ns)
check("runFullTrust and the microphone, and nothing else",
      [(c.tag.split("}")[1], c.get("Name")) for c in caps] == [("Capability", "runFullTrust"), ("DeviceCapability", "microphone")]
      and caps.find("rescap:Capability", ns) is not None, ElementTree.tostring(caps))
application = written.find("m:Applications/m:Application", ns).attrib
check("the shell started with its window, full trust",
      application.get("Executable") == "wholeheard.exe" and application.get(uap10 + "Parameters") == "--window"
      and application.get(uap10 + "RuntimeBehavior") == "packagedClassicApp" and application.get(uap10 + "TrustLevel") == "mediumIL", application)
family = written.find("m:Dependencies/m:TargetDeviceFamily", ns).attrib
check("for desktops from the build that reads uap10", family.get("Name") == "Windows.Desktop" and family.get("MinVersion") == "10.0.19041.0", family)
visual = written.find("m:Applications/m:Application/uap:VisualElements", ns).attrib
sizes = {written.findtext("m:Properties/m:Logo", namespaces=ns): 50, visual.get("Square150x150Logo"): 150, visual.get("Square44x44Logo"): 44}
with tempfile.TemporaryDirectory() as work:
    artefacts, stage = os.path.join(work, "artefacts"), os.path.join(work, "stage")
    for name in names:
        os.makedirs(os.path.join(artefacts, os.path.dirname(name)), exist_ok=True)
        open(os.path.join(artefacts, name), "wb").write(name.encode())
    msix.stage(artefacts, "1.2.3", stage)
    held = {os.path.relpath(os.path.join(folder, file), stage).replace(os.sep, "/") for folder, _, files in os.walk(stage) for file in files}
    check("the four programs, the manifest and its logos are what it holds, and no more",
          held == {name.split("/")[1] for name in names} | {"AppxManifest.xml"} | {logo.replace("\\", "/") for logo in sizes}, held)
    for logo, side in sizes.items():
        head = open(os.path.join(stage, logo.replace("\\", "/")), "rb").read(24) if logo else b""
        check(f"{logo} is a PNG of {side}x{side}", head[:8] == b"\x89PNG\r\n\x1a\n"
              and head[16:24] == side.to_bytes(4, "big") * 2, head)
certification = open(os.path.join(os.path.dirname(mac), "windows", "certification.md"), encoding="utf-8").read()
check("the notes for certification say why runFullTrust and the microphone, and what removal keeps",
      all(word in certification for word in ("runFullTrust", "microphone", "both sides", "Documents", "removes")))
# The certification kit's report (D77): written on Windows by appcert.exe, read here from a report it wrote.
import appcert
said = appcert.said(os.path.join(os.path.dirname(mac), "tests", "appcert-report.xml"))
check("the kit's verdict first: its version, the overall result, how many tests and how many failed",
      said[0] == "certification kit 10.0.26100.9457: PASS overall, 2 tests, 1 failed", said)
check("each failed test named, its number, whether the Store requires it, and what the kit saw first",
      any(line.startswith("  FAIL  Blocked executables (test 88, optional): 3 messages, the first:")
          and "CreateProcessW" in line for line in said), said)
check("a test that passed is not named", not any("App manifest" in line for line in said), said)
with tempfile.TemporaryDirectory() as work:
    broken = os.path.join(work, "report.xml")
    open(broken, "w").write("<html>not the kit's</html>")
    for path, why in ((broken, "a file that is not the kit's report"), (os.path.join(work, "none.xml"), "no file")):
        got = appcert.said(path)
        check(f"{why}: one line, saying no report was read", len(got) == 1 and got[0].startswith("certification kit: no report read"), got)
check("the notes for certification say what stands between the package and a submission, the kit's failure in it",
      "## What stands between the package and a submission" in certification and "Blocked executables" in certification)
print("test: passed" if not failed else "test: FAILED")
sys.exit(failed)
PY
