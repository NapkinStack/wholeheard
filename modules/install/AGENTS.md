# install — local instructions

> Only what is **specific to this module**. Never duplicate a kernel rule (`/AGENTS.md`).

## What this module does not do

- It builds no program: each module's `release` leaves its own in `dist/<module>/`; this one
  assembles them (`assembles` in the manifest, `nstack release` runs them first).
- It decides no library: `rpmbuild` reads what each program links and requires its package.

## Internal conventions nobody could guess

- `scripts/release.sh <format>` — `rpm` today, for Fedora; the next system adds its format here
  (a `dmg` for macOS) from the same artefacts.
- The package is made in a Fedora of this system's release (a throwaway container), so that
  `rpmbuild` is the distribution's own and nothing is installed on the workstation.

## Business invariants

- Nothing outside `/usr`: removing the package never touches a user's meetings or settings
  (`scripts/test.sh`).
- The desktop entry the menu shows is `dev.summit.Summit.desktop`, the application's id: GNOME's
  shortcut portal finds Summit by it.

## Known traps

- A program run, not linked, by another (capture runs `pw-metadata`) → name its package in the
  spec's `Requires`: rpmbuild sees libraries only.

## Non-standard commands

- `e2e` needs `podman` and the network; with no argument it packs stand-in programs, given a
  package (`dist/install/summit-….rpm`) it installs that one.
