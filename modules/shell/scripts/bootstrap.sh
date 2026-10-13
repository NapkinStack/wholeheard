#!/usr/bin/env bash
# Makes the module buildable from a fresh clone: the Rust toolchain, and on Linux the headers
# of the system's web view, of GTK and of the tray. It never asks for a password: where it
# may not install, it says what to install.
set -euo pipefail

command -v cargo >/dev/null || { echo "bootstrap: install Rust first — https://rustup.rs"; exit 1; }
rustup component add rustfmt clippy >/dev/null 2>&1 || true
[ "$(uname -s)" = "Linux" ] || exit 0

if pkg-config --exists webkit2gtk-4.1 gtk+-3.0 2>/dev/null \
  && { pkg-config --exists ayatana-appindicator3-0.1 || pkg-config --exists appindicator3-0.1; } 2>/dev/null; then
  exit 0
fi
debian="libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev pkg-config"
fedora="webkit2gtk4.1-devel gtk3-devel libappindicator-gtk3-devel librsvg2-devel pkgconf-pkg-config"
if command -v apt-get >/dev/null && sudo -n true 2>/dev/null; then
  # shellcheck disable=SC2086  # a list of packages, to be split
  sudo -n apt-get update -qq && sudo -n apt-get install -y -qq $debian
elif command -v dnf >/dev/null; then
  echo "bootstrap: install the web view's, GTK's and the tray's headers — sudo dnf install $fedora"; exit 1
else
  echo "bootstrap: install the web view's, GTK's and the tray's headers — on Debian or Ubuntu: sudo apt-get install $debian"; exit 1
fi
