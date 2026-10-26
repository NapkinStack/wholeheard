#!/usr/bin/env bash
# Run on a Mac by scripts/release.sh: Wholeheard.app from the artefacts each module's release
# left — the four programs side by side in Contents/MacOS, where the shell looks for them —, its
# Info.plist and icon, signed ad hoc (an Apple chip runs nothing unsigned), zipped as macOS keeps a
# bundle whole. Usage: build.sh <artefacts> <version> <out>
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
artefacts=$1 version=$2 out=$3
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
app=$work/Wholeheard.app
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
for program in shell/wholeheard capture/wholeheard-capture transcription/wholeheard-transcription summary/wholeheard-summary; do
  install -m755 "$artefacts/$program" "$app/Contents/MacOS/${program#*/}"
done
python3 "$here/info.py" "$app" "$version"
icons=$work/Wholeheard.iconset
mkdir "$icons"
for size in 16 32 128 256 512; do
  sips -z $size $size "$artefacts/shell/dev.wholeheard.Wholeheard.png" --out "$icons/icon_${size}x${size}.png" >/dev/null
  double=$((size * 2))
  [ $double -le 512 ] && sips -z $double $double "$artefacts/shell/dev.wholeheard.Wholeheard.png" --out "$icons/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$icons" -o "$app/Contents/Resources/Wholeheard.icns"
# Each program, then the bundle: its seal covers the Info.plist and the resources added above.
for program in "$app"/Contents/MacOS/wholeheard-*; do codesign --force --sign - "$program"; done
codesign --force --sign - "$app"
ditto -c -k --keepParent "$app" "$out/Wholeheard-$version-macos-$(uname -m).zip"
