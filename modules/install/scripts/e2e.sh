#!/usr/bin/env bash
# The package as a user meets it, for the system SUMMIT_TARGET names (unset: this Fedora): installed
# with dnf or apt in a container of that system that never saw Summit, each program run, removed —
# meetings and settings stay. With no argument, made from stand-in programs (no other module
# built); given a package, that one. Needs podman and the network.
set -euo pipefail
shopt -s nullglob
here=$(cd "$(dirname "$0")/.." && pwd)
target=${SUMMIT_TARGET:-fedora-$(rpm -E %fedora 2>/dev/null || echo unknown)}
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
  SUMMIT_TARGET=$target NSTACK_DIST="$work/install" bash "$here/scripts/release.sh" >/dev/null
  made=("$work"/install/*.rpm "$work"/install/*.deb)
  package=${made[0]}
fi
# What any package must install, and nothing else: the spec's %files, its macros expanded.
expected=$(sed -n '/^%files/,$p' "$here/summit.spec" | sed '1d; /^$/d; s|%{_bindir}|/usr/bin|; s|%{_datadir}|/usr/share|' | sort | tr '\n' ' ')
case $target in
  fedora-[0-9]*)
    image="registry.fedoraproject.org/fedora:${target#fedora-}"
    add='dnf install -y -q /tmp/summit.rpm'; remove='dnf remove -y -q summit'; file=/tmp/summit.rpm
    listed='rpm -ql summit' ;;
  ubuntu-[0-9]*)
    image="docker.io/library/ubuntu:${target#ubuntu-}"
    add='apt-get update -qq && DEBIAN_FRONTEND=noninteractive apt-get install -y -qq /tmp/summit.deb'
    remove='apt-get remove -y -qq summit'; file=/tmp/summit.deb
    listed='dpkg -L summit' ;;
  *) echo "e2e: no package is made for '$target'" >&2; exit 2 ;;
esac
podman run --rm --security-opt label=disable -v "$package:$file:ro" -e add="$add" -e remove="$remove" -e listed="$listed" -e expected="$expected" "$image" bash -euo pipefail -c '
failed=0
check() { if eval "$2"; then echo "  ok    $1"; else echo "  FAIL  $1"; failed=1; fi; }
mkdir -p $HOME/.config/summit $HOME/Summit/2026-10-18_10-00
echo "sessions = \"$HOME/Summit\"" > $HOME/.config/summit/settings.toml
echo "{}" > $HOME/Summit/2026-10-18_10-00/session.json
check "installed, every dependency found" "eval \"\$add\" >/tmp/add.log 2>&1 || { tail -5 /tmp/add.log; false; }"
for program in summit summit-capture summit-transcription summit-summary; do
  check "$program on the path, every library found" "command -v $program >/dev/null && ! ldd \$(command -v $program) 2>/dev/null | grep -q \"not found\""
  # A program says its usage; the shell, a window with no display here, goes as far as GTK.
  check "$program answers as itself" "{ timeout 10 $program 2>&1 || true; } | grep -qE \"^$program: usage|initialize GTK\""
done
check "the tray'"'"'s library is installed with it — the shell opens it at run time" "ldconfig -p | grep -E \"lib(ayatana-)?appindicator3\.so\.1\" >/dev/null"
check "pw-metadata is installed with it — capture runs it" "command -v pw-metadata >/dev/null"
# The files the package installed, as the system lists them: those of the spec, and nothing else.
installed=$(eval "$listed" | while read -r path; do [ -f "$path" ] && echo "$path"; done | sort | tr "\n" " ")
check "it installs the spec'"'"'s files and no other, all under /usr" "[ \"\$installed\" = \"\$expected\" ] || { echo \"        got: \$installed\"; false; }"
check "the menu sees Summit, under its id" "grep -q ^Exec=summit /usr/share/applications/dev.summit.Summit.desktop"
check "removed" "eval \"\$remove\" >/dev/null 2>&1 && ! command -v summit >/dev/null"
check "the meetings and the settings stay" "test -f $HOME/.config/summit/settings.toml && test -f $HOME/Summit/2026-10-18_10-00/session.json"
[ "$failed" = 0 ] && echo "e2e: passed" || { echo "e2e: FAILED"; exit 1; }
'
