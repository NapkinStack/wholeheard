"""Writes Wholeheard.app's Info.plist and its French strings into <app>/Contents, for <version>.

What macOS shows when it asks for the microphone and for the sound the Mac plays is the
product's words (rule 1): English in the Info.plist, French in fr.lproj, both from words.json.
"""
import json
import os
import plistlib
import sys

app, version = sys.argv[1], sys.argv[2]
words = json.load(open(os.path.join(os.path.dirname(__file__), "words.json"), encoding="utf-8"))
contents = os.path.join(app, "Contents")
os.makedirs(os.path.join(contents, "Resources", "fr.lproj"), exist_ok=True)
info = {
    "CFBundleName": "Wholeheard",
    "CFBundleInfoDictionaryVersion": "6.0",
    "CFBundleDisplayName": "Wholeheard",
    "CFBundleIdentifier": "dev.wholeheard.Wholeheard",
    "CFBundleExecutable": "wholeheard",
    "CFBundlePackageType": "APPL",
    "CFBundleShortVersionString": version,
    "CFBundleVersion": version,
    "CFBundleIconFile": "Wholeheard",
    "CFBundleDevelopmentRegion": "en",
    "CFBundleLocalizations": sorted(words),
    "NSHighResolutionCapable": True,
    **words["en"],
}
with open(os.path.join(contents, "Info.plist"), "wb") as out:
    plistlib.dump(info, out)


def quoted(text):
    return '"' + text.replace("\\", "\\\\").replace('"', '\\"') + '"'


with open(os.path.join(contents, "Resources", "fr.lproj", "InfoPlist.strings"), "w", encoding="utf-8") as out:
    for key, text in words["fr"].items():
        out.write(f"{key} = {quoted(text)};\n")
