# summary — local instructions

> Only what is **specific to this module**.
> Never duplicate a kernel rule (`/AGENTS.md`): that is wasted context and a source of
> divergence.
>
> Indicative budget: 100 lines.

## Responsibility

Writes the minutes of a transcript.

## What this module does not do

- It makes no transcript and reads no audio: `transcription`.
- It shows nothing and asks nothing of the user — the consent before a first sending online
  is the `shell`'s, which reads the `sending` event.
- It keeps no key: `SUMMIT_PROVIDER_KEY` is read from the environment and written nowhere.

## Internal conventions nobody could guess

- `src/lib.rs` knows no network; `src/provider.rs` holds the one connection; `src/main.rs` the
  program. `tests/program.rs` plays a provider on this machine: no test calls a real one.
- The request is written in English and asks for the meeting's language.
- An answer is minutes when it holds exactly three sections of second level, none empty.

## Business invariants

- `minutes.md` appears under its name only once it is whole; a failure leaves what was there.
- One host is contacted, the one configured: a redirection is refused, never followed, and no
  proxy the environment names is used. The host named is the host the request goes to.
- The minutes name the host that answered, or `local` — never a key, a path or a full address.
- A key the provider sends back in its answer is never written; nothing leaves before `sending`.
- What the meeting did not settle is asked to be said, never guessed.

## Known traps

- A model that opens with a sentence before the sections is refused, by design.
- Reading a format by hand: an answer was once read line by line, an address split by hand —
  each cost several rounds of verification. An answer goes through a CommonMark reader, an
  address through the HTTP client's own.
- The key is looked for in an answer as it is written: sent back in another case or encoded,
  it is not caught. A key that is a common word turns honest minutes away.

## Areas not to change without approval

- The format of `minutes.md`: it is `contracts/minutes/v1`.
- The rules of the request in `request()`: they are D5's third criterion.

## Non-standard commands

None.
