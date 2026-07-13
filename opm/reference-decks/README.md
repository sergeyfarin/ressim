# OPM FIM reference decks

These are tracked, offline fixtures for convergence comparisons between the
FIM diagnostic presets and OPM Flow. They are not product artifacts.

Each case directory contains:

- `CASE.DATA` — the Flow input deck;
- `manifest.json` — the ResSim invocation, deck checksum, input invariants,
  and the recorded Flow convergence oracle.

Validate a fixture without running a simulator:

```bash
node scripts/opm-reference-fixture-check.mjs --case gas-rate-10x10x3
```

Run the side-by-side baseline (Flow output is written outside the source tree):

```bash
scripts/opm-ressim-compare.sh --case gas-rate-10x10x3 --out-dir /tmp/opm-ressim
```

The manifest verifies the deck byte-for-byte and checks Flow's `INFOSTEP`
summary after a run. It does not prove that the two simulators use identical
well equations; it makes the hand-authored input mapping and the oracle
explicit so that divergence can be investigated reproducibly.
