# Spikes

Throwaway probes of the first cycle. Their product is what they record in the cycle's file
(`docs/project/cycles/`), not this code: nothing here is a module, and nothing here ships.

| Probe | Deliverable | Question |
|---|---|---|
| [`d1-capture/`](d1-capture/) | D1, first half | Can one program write the microphone and the system's sound as two files, on the three systems, with the same code? |

## Running `d1-capture`

During a real call, with people who know they are recorded:

```bash
cd spikes/d1-capture
cargo run --release -- 20 /tmp
```

It records for 20 seconds into `/tmp/me.wav` and `/tmp/others.wav`, then says for each
whether it holds sound or silence. **Delete the two files afterwards**: no recording of a
real meeting enters this repository (`docs/project/rules/security.md`, rule 2).
