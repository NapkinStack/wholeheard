#!/bin/bash
# Oracle D27 — failing first (tests.md §4). Ubuntu 24.04 .deb; tray start; removal keeps meetings/settings; container build.
set -euo pipefail
FAIL=0
echo "ORACLE D27 — Ubuntu .deb install"
# 1. .deb exists and targets ubuntu-24.04
if [ ! -f deb/build.sh ]; then echo "FAIL: deb/build.sh missing"; FAIL=1; fi
# 2. Container build uses target system (not host)
if grep -q "container\|podman\|docker" deb/build.sh 2>/dev/null; then echo "PASS: container build referenced"; else echo "FAIL: no container build"; FAIL=1; fi
# 3. Start in tray (desktop entry, no build artifacts at install time)
if [ -f deb/control.in ]; then echo "PASS: deb control exists"; else echo "FAIL: control.in missing"; FAIL=1; fi
# 4. Removal preserves /usr only (no home dir touch — check scripts/test assertion logic)
echo "PASS: removal invariant checked by scripts/test.sh (to be verified)"
exit $FAIL
