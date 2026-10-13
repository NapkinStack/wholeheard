# Architecture decisions

The technical decisions of this project that are costly to reverse, or that constrain the
ones after them. What a record holds and how it is named:
[`.nstack/handbook/06-decisions.md`](../../.nstack/handbook/06-decisions.md) §4. Its form: [`.nstack/forms/adr.md`](../../.nstack/forms/adr.md).

| No. | Title | Status | Criterion to check on |
|---|---|---|---|
| [0001](./0001-split-summit-into-four-modules.md) | Split Summit into four modules, one per step | Accepted (2026-10-12) | End of the second cycle: a step replaced or run alone without a change elsewhere |
| [0002](./0002-a-rust-core-of-programs-and-a-web-view-shell.md) | A Rust core of programs that run alone, and a web-view shell that starts them | Accepted (2026-10-12) | End of cycle 01, 2026-10-27: D3 accepted on Fedora with no capture code written for one system; the first Windows and macOS builds record a real call with none added |
| [0003](./0003-transcribe-behind-one-interface-whisper-on-the-graphics-card.md) | Transcribe behind one interface, with a native Whisper on the graphics card for Fedora | Accepted (2026-10-13) | End of cycle 01, 2026-10-27: D4 accepted on Fedora with this engine; a second engine added without a change to the step |
| [0004](./0004-the-audio-as-recorded-is-kept-everything-else-is-derived.md) | The audio as recorded is kept; everything else is derived from it | Proposed (2026-10-13) | End of the cycle that implements it: on the bench, loudspeakers give no more words said twice than a headset and no more words wrong on the local side; a session is transcribed again by a later version without being recorded again |
