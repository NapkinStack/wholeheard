# Spikes

Throwaway probes of the first cycle. Their product is what they record in the cycle's file
(`docs/project/cycles/`), not this code: nothing here is a module, and nothing here ships.

| Probe | Deliverable | Question |
|---|---|---|
| [`d1-capture/`](d1-capture/) | D1, first half | Can one program write the microphone and the system's sound as two files, on the three systems, with the same code? |
| [`d1-shell/`](d1-shell/) | D1, second half | Does a web-view application get a window that renders, a tray icon, and a global shortcut that fires while another window has the focus? |

## Running `d1-shell`

```bash
cd spikes/d1-shell
cargo run --release
```

A window opens and says what the system gave it. Nothing is recorded. It needs no Node: its
page is one static file.

On a Linux desktop the portal gives a global shortcut only to an application it can name.
Install the probe's desktop entry once, and remove it when the spike is over:

```bash
install -Dm644 dev.summit.d1shell.desktop ~/.local/share/applications/dev.summit.d1shell.desktop
rm ~/.local/share/applications/dev.summit.d1shell.desktop    # afterwards
```

## Running `d1-capture`

During a real call, with people who know they are recorded:

```bash
cd spikes/d1-capture
cargo run --release -- 20 /tmp --tone
```

`--tone` plays a quiet note on the default output for the whole run, so that silence in the
system's sound cannot mean that nothing was playing.

It records for 20 seconds into `/tmp/me.wav` and `/tmp/others.wav`, then says for each
whether it holds sound or silence. **Delete the two files afterwards**: no recording of a
real meeting enters this repository (`docs/project/rules/security.md`, rule 2).
