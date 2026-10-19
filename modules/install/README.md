# install

What this module is for, who owns it and how critical it is: its
[MANIFEST.yaml](./MANIFEST.yaml), the one place they are written.

```mermaid
flowchart LR
    M["shell · capture · transcription · summary<br/>each module's release"]:::mod --> D["dist/&lt;module&gt;/<br/>their programs, the desktop entry, the icon"]:::art
    D --> B["install's release<br/>rpmbuild or dpkg, in a container<br/>of the system SUMMIT_TARGET names"]:::this
    B --> R["dist/install/<br/>summit-‹version›.x86_64.rpm · summit_‹version›_amd64.deb"]:::out
    classDef mod fill:#374151,color:#fff
    classDef art fill:#92400e,color:#fff
    classDef this fill:#065f46,color:#fff
    classDef out fill:#1e3a8a,color:#fff
```

**Legend** — grey: the modules whose programs it packs · amber: their release artefacts ·
green: this module · blue: what a user installs. It provides and consumes no contract; why it
exists: ADR-0006.

## Getting started

Needs `podman`. From the repository's root:

```bash
nstack release                                    # for this Fedora: every module, then the rpm
sudo dnf install ./dist/install/summit-*.x86_64.rpm

SUMMIT_TARGET=ubuntu-24.04 nstack release         # each program built in an Ubuntu container, then the .deb
sudo apt install ./dist/install/summit_*_amd64.deb
```

`SUMMIT_TARGET` names the system the package is for — `fedora-44`, `ubuntu-24.04` — for every
module at once: each program is built in a container of that system, so that it links what that
system has (D27).
