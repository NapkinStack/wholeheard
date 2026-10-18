# install

What this module is for, who owns it and how critical it is: its
[MANIFEST.yaml](./MANIFEST.yaml), the one place they are written.

```mermaid
flowchart LR
    M["shell · capture · transcription · summary<br/>each module's release"]:::mod --> D["dist/&lt;module&gt;/<br/>their programs, the desktop entry, the icon"]:::art
    D --> B["install's release<br/>rpmbuild, in a Fedora container"]:::this
    B --> R["dist/install/summit-‹version›.x86_64.rpm"]:::out
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
nstack release                     # every module, each after what it assembles
sudo dnf install ./dist/install/summit-*.x86_64.rpm
```
