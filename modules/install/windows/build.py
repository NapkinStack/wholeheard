"""Run on Windows by scripts/release.sh: Wholeheard-<version>-windows-x64.zip from the artefacts
each module's release left — the four programs side by side in one folder, where the shell looks
for them (D53). No installer, no signature: the colleague unpacks it and opens wholeheard.exe; the
Store's package is msix.py's (D54).

Usage: build.py <artefacts> <version> <out>
"""
import os
import sys
import zipfile

PROGRAMS = ("shell/wholeheard.exe", "capture/wholeheard-capture.exe",
            "transcription/wholeheard-transcription.exe", "summary/wholeheard-summary.exe")

artefacts, version, out = sys.argv[1:4]
path = os.path.join(out, f"Wholeheard-{version}-windows-x64.zip")
with zipfile.ZipFile(path, "w", zipfile.ZIP_DEFLATED) as archive:
    for program in PROGRAMS:
        archive.write(os.path.join(artefacts, program), f"Wholeheard/{program.split('/')[1]}")
print(path)
