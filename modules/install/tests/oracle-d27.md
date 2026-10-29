# D27 Oracle — Ubuntu 24.04 .deb install
Module: install (standard)
Accept (from brief): Ubuntu 24.04 install from .deb; starts in tray with nothing built / no var set; removal keeps meetings/settings; programs built in container of target system (no host dependency).
Success criterion (test first, must FAIL before code):
- deb/build.sh produces .deb in throwaway Ubuntu container (WHOLEHEARD_TARGET=ubuntu-24.04); no host library linked.
- Installed package puts /usr/bin/wholeheard, /usr/bin/wholeheard-capture, /usr/bin/wholeheard-transcription, /usr/bin/wholeheard-summary; desktop entry dev.wholeheard.Wholeheard.desktop under /usr/share/applications; no file outside /usr.
- Tray starts (shell module) with nothing built (no dist/artefact required) and with no env var pre-set; start returns 0, desktop shown.
- Removal (apt remove) does not delete /home/*/.wholeheard or meetings/settings; only /usr files removed (verified by scripts/test.sh).
Fail-first: run `bash scripts/test.sh` before code change — expect FAIL on .deb build / container dependency.
