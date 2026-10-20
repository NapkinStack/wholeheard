#!/usr/bin/env bash
# The package as a user meets it, for the system WHOLEHEARD_TARGET names (SUMMIT_TARGET, its name before D28, read too) (unset: the system it runs on): installed
# with dnf or apt in a container of that system that never saw Wholeheard, each program run, removed —
# meetings and settings stay. With no argument, made from stand-in programs (no other module
# built); given a package, that one. Needs podman and the network.
set -euo pipefail
shopt -s nullglob
here=$(cd "$(dirname "$0")/.." && pwd)
# As the other modules' e2e where their system is missing: said, not failed.
command -v podman >/dev/null || { echo "e2e: NOT RUN — podman is missing here"; exit 0; }
# Unset, the system this runs on, as it names itself (fedora-44, ubuntu-24.04).
target=${WHOLEHEARD_TARGET:-${SUMMIT_TARGET:-$(. /etc/os-release && echo "$ID-$VERSION_ID")}}
package=${1:+$(realpath "$1")}
if [ -z "$package" ]; then
  work=$(mktemp -d)
  trap 'rm -rf "$work"' EXIT
  for program in shell/wholeheard capture/wholeheard-capture transcription/wholeheard-transcription summary/wholeheard-summary; do
    mkdir -p "$work/$(dirname "$program")"
    printf '#!/bin/sh\necho "%s: usage: …" >&2\nexit 2\n' "$(basename "$program")" > "$work/$program"
    chmod +x "$work/$program"
  done
  printf '[Desktop Entry]\nType=Application\nName=Wholeheard\nComment[fr]=x\nExec=wholeheard\nIcon=dev.wholeheard.Wholeheard\n' > "$work/shell/dev.wholeheard.Wholeheard.desktop"
  printf 'png' > "$work/shell/dev.wholeheard.Wholeheard.png"
  echo 0.0.0 > "$work/shell/VERSION"
  mkdir -p "$work/install"
  WHOLEHEARD_TARGET=$target NSTACK_DIST="$work/install" bash "$here/scripts/release.sh" >/dev/null
  made=("$work"/install/*.rpm "$work"/install/*.deb)
  package=${made[0]}
fi
# What any package must install, and nothing else: the spec's %files, its macros expanded.
expected=$(sed -n '/^%files/,$p' "$here/wholeheard.spec" | sed '1d; /^$/d; s|%{_bindir}|/usr/bin|; s|%{_datadir}|/usr/share|' | sort | tr '\n' ' ')
case $target in
  fedora-[0-9]*)
    image="registry.fedoraproject.org/fedora:${target#fedora-}"
    add='dnf install -y -q /tmp/wholeheard.rpm'; remove='dnf remove -y -q wholeheard'; file=/tmp/wholeheard.rpm
    listed='rpm -ql wholeheard'; scripts='rpm -q --scripts wholeheard' ;;
  ubuntu-[0-9]*)
    image="docker.io/library/ubuntu:${target#ubuntu-}"
    add='apt-get update -qq && DEBIAN_FRONTEND=noninteractive apt-get install -y -qq /tmp/wholeheard.deb'
    remove='apt-get remove -y -qq wholeheard'; file=/tmp/wholeheard.deb
    listed='dpkg -L wholeheard'
    scripts='ls /var/lib/dpkg/info/wholeheard.preinst /var/lib/dpkg/info/wholeheard.postinst /var/lib/dpkg/info/wholeheard.prerm /var/lib/dpkg/info/wholeheard.postrm' ;;
  *) echo "e2e: no package is made for '$target'" >&2; exit 2 ;;
esac
podman run --rm --security-opt label=disable -v "$package:$file:ro" -v "$here/scripts/judge-installed.sh:/judge.sh:ro" -e add="$add" -e remove="$remove" -e listed="$listed" -e scripts="$scripts" -e expected="$expected" "$image" bash -euo pipefail -c '
failed=0
check() { if eval "$2"; then echo "  ok    $1"; else echo "  FAIL  $1"; failed=1; fi; }
mkdir -p $HOME/.config/wholeheard $HOME/Wholeheard/2026-10-18_10-00
echo "sessions = \"$HOME/Wholeheard\"" > $HOME/.config/wholeheard/settings.toml
echo "{}" > $HOME/Wholeheard/2026-10-18_10-00/session.json
check "installed, every dependency found" "eval \"\$add\" >/tmp/add.log 2>&1 || { tail -5 /tmp/add.log; false; }"
for program in wholeheard wholeheard-capture wholeheard-transcription wholeheard-summary; do
  check "$program on the path, every library found" "command -v $program >/dev/null && ! ldd \$(command -v $program) 2>/dev/null | grep -q \"not found\""
  # A program says its usage; the shell, a window with no display here, goes as far as GTK.
  check "$program answers as itself" "{ timeout 10 $program 2>&1 || true; } | grep -qE \"^$program: usage|initialize GTK\""
done
check "the tray'"'"'s library is installed with it — the shell opens it at run time" "ldconfig -p | grep -E \"lib(ayatana-)?appindicator3\.so\.1\" >/dev/null"
check "pw-metadata is installed with it — capture runs it" "command -v pw-metadata >/dev/null"
check "it installs the spec'"'"'s files and the directories above them, nothing else, and runs no script" "bash /judge.sh \"\$listed\" \"\$scripts\""
check "the menu sees Wholeheard, under its id" "grep -q ^Exec=wholeheard /usr/share/applications/dev.wholeheard.Wholeheard.desktop"
check "removed" "eval \"\$remove\" >/dev/null 2>&1 && ! command -v wholeheard >/dev/null"
check "the meetings and the settings stay" "test -f $HOME/.config/wholeheard/settings.toml && test -f $HOME/Wholeheard/2026-10-18_10-00/session.json"
[ "$failed" = 0 ] && echo "e2e: passed" || { echo "e2e: FAILED"; exit 1; }
'
