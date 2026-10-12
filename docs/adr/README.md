# Architecture decisions

The technical decisions of this project that are costly to reverse, or that constrain the
ones after them. What a record holds and how it is named:
[`.nstack/handbook/06-decisions.md`](../../.nstack/handbook/06-decisions.md) §4. Its form: [`.nstack/forms/adr.md`](../../.nstack/forms/adr.md).

| No. | Title | Status | Criterion to check on |
|---|---|---|---|
| [0001](./0001-split-summit-into-four-modules.md) | Split Summit into four modules, one per step | Accepted (2026-10-12) | End of the second cycle: a step replaced or run alone without a change elsewhere |
| [0002](./0002-a-rust-core-of-programs-and-a-web-view-shell.md) | A Rust core of programs that run alone, and a web-view shell that starts them | Accepted (2026-10-12) | End of cycle 01, 2026-10-27: D3 accepted on Fedora with no capture code written for one system; the first Windows and macOS builds record a real call with none added |
