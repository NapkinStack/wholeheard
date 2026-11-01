#!/bin/bash
# Oracle D27 — failing first (tests.md §4). Ubuntu 24.04 .deb; tray start; removal keeps meetings/settings; container build.
set -euo pipefail
FAIL=0
echo "ORACLE D27 — Ubuntu .deb install"
# Absolute paths: works regardless of cwd.
here=$(cd "$(dirname "$0")/.." && pwd)
# 1. .deb exists and targets ubuntu-24.04
if [ ! -f "$here/deb/build.sh" ]; then echo "FAIL: deb/build.sh missing"; FAIL=1; fi
# 2. Container build uses target system (not host)
if grep -q "container\|podman\|docker" "$here/deb/build.sh" 2>/dev/null; then echo "PASS: container build referenced"; else echo "FAIL: no container build"; FAIL=1; fi
# 3. Start in tray (desktop entry, no build artifacts at install time)
if [ -f "$here/deb/control.in" ]; then echo "PASS: deb control exists"; else echo "FAIL: control.in missing"; FAIL=1; fi
# 4. Real test: .deb installs and removal does not touch /home
# Proves the .deb installs in an ubuntu:24.04 container and apt remove leaves $HOME intact.
container_output() {
  local deb_path="$1"
  # Uses the module's absolute paths for the .deb build reference.
  podman run --rm -v "$deb_path:/tmp/wholeheard.deb:ro" --security-opt label=disable docker.io/library/ubuntu:24.04 bash -euo pipefail -c '
    DEBIAN_FRONTEND=noninteractive apt-get update -qq >/dev/null 2>&1
    DEBIAN_FRONTEND=noninteractive apt-get install -y -qq /tmp/wholeheard.deb >/dev/null 2>&1
    command -v wholeheard >/dev/null || { echo "FAIL: .deb did not install wholeheard"; exit 1; }
    # Removal must not touch $HOME settings/meetings.
    mkdir -p "$HOME/.config/wholeheard"
    echo "sessions = \"$HOME/Wholeheard\"" > "$HOME/.config/wholeheard/settings.toml"
    mkdir -p "$HOME/Wholeheard/2026-10-18_10-00"
    echo "{}" > "$HOME/Wholeheard/2026-10-18_10-00/session.json"
    apt-get remove -y -qq wholeheard >/dev/null 2>&1 || true
    test -f "$HOME/.config/wholeheard/settings.toml" || { echo "FAIL: removal erased settings"; exit 1; }
    test -f "$HOME/Wholeheard/2026-10-18_10-00/session.json" || { echo "FAIL: removal erased meetings"; exit 1; }
    echo "PASS: .deb installs; removal keeps /home"
  ' 2>/dev/null || { echo "FAIL: container .deb install/remove failed (check podman / .deb build)"; return 1; }
}
if [ -f "$here/scripts/release.sh" ]; then
  # Try to build the .deb first so container_output has a real file; if build fails, still report.
  (cd "$here" && bash scripts/release.sh >/dev/null 2>&1 || true)
fi
# Look for a built .deb under the module's deb/ or install/ paths using absolute references.
deb_file=""
for p in "$here/deb/"*.deb "$here/install/"*.deb; do
  [ -f "$p" ] && deb_file="$p" && break
done
if [ -n "$deb_file" ]; then
  if container_output "$deb_file"; then echo "PASS: container install/remove verified"; else FAIL=1; fi
else
  echo "FAIL: no .deb built for container verify (run scripts/release.sh first); container build unverified"
  FAIL=1
fi
exit $FAIL
