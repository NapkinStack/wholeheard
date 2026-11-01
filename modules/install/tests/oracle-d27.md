# D27 Oracle — Ubuntu 24.04 .deb install
Module: install (standard)
Accept (from brief): Ubuntu 24.04 install from .deb; starts in tray with nothing built / no var set; removal keeps meetings/settings; programs built in container of target system (no host dependency).
Success criterion (test first, must FAIL before code):
- deb/build.sh produces .deb in throwaway Ubuntu container (WHOLEHEARD_TARGET=ubuntu-24.04); no host library linked.
- Installed package puts /usr/bin/wholeheard, /usr/bin/wholeheard-capture, /usr/bin/wholeheard-transcription, /usr/bin/wholeheard-summary; desktop entry dev.wholeheard.Wholeheard.desktop under /usr/share/applications; no file outside /usr.
- Tray starts (shell module) with nothing built (no dist/artefact required) and with no env var pre-set; start returns 0, desktop shown.
- Removal (apt remove) does not delete /home/*/.wholeheard or meetings/settings; only /usr files removed (verified by scripts/test.sh).
Fail-first: run `bash scripts/test.sh` before code change — expect FAIL on .deb build / container dependency.

## Test sheet
- Deliverable: D27 — Ubuntu 24.04 .deb install / removal, oracle verified
- Module: install (standard)
- Budget: 1 oracle script + 1 e2e call; container image ubuntu:24.04; no host build dependency
- Evidence: `bash modules/install/tests/oracle-d27.sh` (FAIL=1 when .deb missing; PASS only after container apt install/remove confirmed); `bash scripts/e2e.sh` invokes oracle for ubuntu-* targets (line 32-35)
- Scenarios: (1) .deb builds in ubuntu:24.04 container — absolute paths $here/deb/..., exit 1 if .deb absent; (2) apt install/remove inside container verifies package and home preservation; (3) e2e script calls oracle (nothing runs it — fixed)
- Verifier: session hermes-1029-13 — install/deb
- Confirmed: c03ca8a (merge origin/main); nstack check install green; CI green
