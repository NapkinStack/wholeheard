"""Run on Windows by scripts/release.sh, and by trial.ps1 for a later version: Wholeheard-<version>-x64.msix,
made with the Windows SDK's MakeAppx from AppxManifest.xml.in, identity.json, the four programs and
Assets/ (D54). Unsigned: the Store signs what it publishes; outside it, whoever installs it signs it.

Usage: msix.py <artefacts> <version> <out>
"""
import glob
import json
import os
import re
import shutil
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
PROGRAMS = ("shell/wholeheard.exe", "capture/wholeheard-capture.exe",
            "transcription/wholeheard-transcription.exe", "summary/wholeheard-summary.exe")


def four_parts(version):
    """A package's version: the application's three numbers, then the 0 the Store keeps for itself."""
    if not re.fullmatch(r"\d+\.\d+\.\d+", version) or any(int(part) > 65535 for part in version.split(".")):
        raise ValueError(f"not three numbers under 65536: {version}")
    return f"{version}.0"


def identity():
    with open(os.path.join(HERE, "identity.json"), encoding="utf-8") as file:
        return json.load(file)


def manifest(version, who):
    with open(os.path.join(HERE, "AppxManifest.xml.in"), encoding="utf-8") as file:
        text = file.read()
    for key, value in (("NAME", who["name"]), ("VERSION", four_parts(version)), ("PUBLISHER", who["publisher"]),
                       ("PUBLISHER_DISPLAY_NAME", who["publisher_display_name"])):
        text = text.replace(f"@{key}@", value)
    return text


def notices(version, who):
    """What this package cannot do yet, for the run to say."""
    said = []
    if who["placeholder"]:
        said.append(f"identity {who['name']} is a placeholder until Partner Center reserves the name: "
                    "this package is not submitted")
    if version.startswith("0."):
        said.append(f"version {four_parts(version)}: the Store refuses a first part 0, "
                    "the application's version must reach 1 before a submission")
    return said


def stage(artefacts, version, folder):
    """The package's content, as MakeAppx reads it."""
    os.makedirs(folder)
    for program in PROGRAMS:
        shutil.copy(os.path.join(artefacts, program), folder)
    shutil.copytree(os.path.join(HERE, "Assets"), os.path.join(folder, "Assets"))
    with open(os.path.join(folder, "AppxManifest.xml"), "w", encoding="utf-8") as file:
        file.write(manifest(version, identity()))


def makeappx():
    found = sorted(glob.glob(r"C:\Program Files (x86)\Windows Kits\10\bin\10.*\x64\makeappx.exe"))
    if not found:
        sys.exit("msix: no Windows SDK's makeappx.exe under Windows Kits\\10\\bin")
    return found[-1]


def main(artefacts, version, out):
    folder = os.path.join(out, f"msix-{version}")
    shutil.rmtree(folder, ignore_errors=True)
    stage(artefacts, version, folder)
    path = os.path.join(out, f"Wholeheard-{version}-x64.msix")
    subprocess.run([makeappx(), "pack", "/o", "/h", "SHA256", "/d", folder, "/p", path], check=True, timeout=600)
    shutil.rmtree(folder)
    for line in notices(version, identity()):
        print(f"::warning title=The Windows package::{line}")
    print(path)


if __name__ == "__main__":
    main(*sys.argv[1:4])
