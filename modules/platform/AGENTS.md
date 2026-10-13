# platform — local instructions

> Only what is **specific to this module**.
> Never duplicate a kernel rule (`/AGENTS.md`): that is wasted context and a source of
> divergence.
>
> Indicative budget: 100 lines.

## Responsibility

Measures what Summit's steps produce against a meeting whose every word is known.

## What this module does not do

- It is not a step of Summit and ships to no user: it measures the steps.
- It never imports a step's code: it runs the programs as a user does, and reads their files.

## Internal conventions nobody could guess

- `bench/meeting.txt` is the truth's only source: the tracks and `truth.json` are rendered from it, never edited.
- The script is in French and its voices are synthetic: test material, like the product's own sentences the one other place this repository is not in English.

## Business invariants

- A meeting rendered twice is the same bytes: a count that moves between two runs is the programs' doing.
- Nobody's voice enters the bench: synthetic voices, or none.

## Known traps

- The first counts could be won by muting the microphone while the remote side speaks: the script had almost no speech over speech, and no count saw a local word lost. Whatever is added to the bench is first tried against the cheapest way of cheating it.
- A synthesiser left to chance renders another meeting each time: the voices are steady, and a render from an empty cache is the same bytes.
- One run is not a measure: which turns an engine drops moves with a noise nobody hears. A count is a spread over several draws.
- A word alone proves no echo: `le` is said by everyone. Borrowed words are counted in runs of two.

## Areas not to change without approval

- `bench/meeting.txt`: every count recorded in a cycle was made on it. A change to the script makes every earlier count incomparable: it is said in the cycle, with the counts run again.

## Non-standard commands

- The voices are fetched once, outside the repository (see the front page): about 270 MB.