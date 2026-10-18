#!/usr/bin/env bash
# The package as a user meets it: `dnf install`ed in a Fedora that never saw Summit, each program
# run, `dnf remove`d — meetings and settings stay. With no argument, made from stand-in programs
# (no other module built); given a package, that one. Needs podman and the network.
set -euo pipefail
here=$(cd "$(dirname "$0")/.." && pwd)
package=${1:+$(realpath "$1")}
if [ -z "$package" ]; then
  work=$(mktemp -d)
  trap 'rm -rf "$work"' EXIT
  for program in shell/summit capture/summit-capture transcription/summit-transcription summary/summit-summary; do
    mkdir -p "$work/$(dirname "$program")"
    printf '#!/bin/sh\necho "%s: usage: …" >&2\nexit 2\n' "$(basename "$program")" > "$work/$program"
    chmod +x "$work/$program"
  done
  printf '[Desktop Entry]\nType=Application\nName=Summit\nComment[fr]=x\nExec=summit\nIcon=dev.summit.Summit\n' > "$work/shell/dev.summit.Summit.desktop"
  printf 'png' > "$work/shell/dev.summit.Summit.png"
  echo 0.0.0 > "$work/shell/VERSION"
  mkdir -p "$work/install"
  NSTACK_DIST="$work/install" bash "$here/scripts/release.sh" rpm >/dev/null
  package=$(ls "$work"/install/*.rpm)
fi
release=$(rpm -E %fedora)
podman run --rm --security-opt label=disable -v "$package:/tmp/summit.rpm:ro" "registry.fedoraproject.org/fedora:$release" bash -euo pipefail -c '
failed=0
check() { if eval "$2"; then echo "  ok    $1"; else echo "  FAIL  $1"; failed=1; fi; }
mkdir -p $HOME/.config/summit $HOME/Summit/2026-10-18_10-00
echo "sessions = \"$HOME/Summit\"" > $HOME/.config/summit/settings.toml
echo "{}" > $HOME/Summit/2026-10-18_10-00/session.json
check "dnf installs it, every dependency found" "dnf install -y -q /tmp/summit.rpm >/tmp/dnf.log 2>&1 || { tail -5 /tmp/dnf.log; false; }"
for program in summit summit-capture summit-transcription summit-summary; do
  check "$program on the path, every library found" "command -v $program >/dev/null && ! ldd \$(command -v $program) 2>/dev/null | grep -q \"not found\""
  # A program says its usage; the shell, a window with no display here, goes as far as GTK.
  check "$program answers as itself" "{ timeout 10 $program 2>&1 || true; } | grep -qE \"^$program: usage|initialize GTK\""
done
check "the tray'"'"'s library is installed with it — the shell opens it at run time" "ldconfig -p | grep -qE \"lib(ayatana-)?appindicator3\.so\.1\""
check "the menu sees Summit, under its id" "grep -q ^Exec=summit /usr/share/applications/dev.summit.Summit.desktop"
check "dnf removes it" "dnf remove -y -q summit >/dev/null 2>&1 && ! command -v summit >/dev/null"
check "the meetings and the settings stay" "test -f $HOME/.config/summit/settings.toml && test -f $HOME/Summit/2026-10-18_10-00/session.json"
[ "$failed" = 0 ] && echo "e2e: passed" || { echo "e2e: FAILED"; exit 1; }
'
