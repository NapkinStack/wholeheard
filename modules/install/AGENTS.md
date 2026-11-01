# install — local instructions

## What this module does not do

- It builds no program: each module's `release` leaves its own in `dist/<module>/`; this one
  assembles them (`assembles` in the manifest, `nstack release` runs them first).
- It decides no library: `rpmbuild` reads what each program links and requires its package.

## Internal conventions nobody could guess

- `scripts/release.sh` makes the package of the system `WHOLEHEARD_TARGET` names: `fedora-<n>` an rpm
  (`wholeheard.spec`), `ubuntu-<n>.<n>` a .deb (`deb/build.sh`, `deb/control.in`); unset, the package
  of the system it runs on (`/etc/os-release`; on a Mac, `macos-<processor>`). `macos-arm64`:
  `Wholeheard.app`, zipped (`mac/build.sh`), made on a Mac — GitHub's runner, `macos-app.yml`'s job
  *the app* — never in a container. Its Info.plist and French strings: `mac/info.py`, from
  `mac/words.json`, the sentences macOS shows when it asks for the microphone and the sound the
  Mac plays.
- `windows-x64`: the zip (`windows/build.py`) and `Wholeheard-<version>-x64.msix` (`windows/msix.py`,
  from `AppxManifest.xml.in`, `identity.json` and `Assets/`), made on Windows. `windows/trial.ps1`,
  run by `windows-app.yml`, signs it for the run and installs, upgrades and removes it; `windows/certify.ps1`
  has the certification kit test it, `windows/appcert.py` says its report — read in `scripts/test.sh` from
  `tests/appcert-report.xml`, cut from a report the kit wrote on the runner.
- Each package is made in a throwaway container of its system, so that `rpmbuild`, `dpkg` and the
  libraries they read are that system's, and nothing is installed on the workstation.

## Business invariants

- Nothing outside `/usr`: removing the package never touches a user's meetings or settings
  (`scripts/test.sh`).
- The desktop entry the menu shows is `dev.wholeheard.Wholeheard.desktop`, the application's id: GNOME's
  shortcut portal finds Wholeheard by it.

## Known traps

- The Mac's app is signed ad hoc after everything is in it: a file added to the bundle after
  `codesign` breaks its seal, and macOS says a downloaded app so changed is *damaged*.
- A package's writes under `AppData` go to its own data and go with it; a test outside the package reads
  them at `%LOCALAPPDATA%\Packages\<family>\LocalCache`, never at `%APPDATA%`.
- A program run, not linked, by another (capture runs `pw-metadata`), or a library opened at run
  time (the shell's tray: appindicator) → name its package in the spec's `Requires` and in
  `deb/build.sh`'s `depends`: rpmbuild and dpkg-shlibdeps see linked libraries only.
- dpkg-shlibdeps finds a library's package only if it is installed where it runs: a new library
  linked by a program → add its runtime package to `deb/build.sh`'s `apt-get install`.

## Non-standard commands

- `e2e` needs `podman` and the network; for the system `WHOLEHEARD_TARGET` names (unset: the
  system it runs on), with no argument it packs stand-in programs, given a package (`dist/install/…`) it
  installs that one. On Ubuntu, apt fetches GTK for the tray's library: count ten minutes.
