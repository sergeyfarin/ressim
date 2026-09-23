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

## Known mapping gaps (measured, #21)

The water-pressure decks mirror the wasm presets closely but not exactly. Against the default
preset, two differences remain, and together they explain the whole matched-step difference.

- **Relperm.** The decks tabulate Corey in 9 SWOF knots, while the preset evaluates analytic
  Corey. Run ResSim with `--corey-table-points 9` to compare like with like: injection then
  agrees to ≤ 0.004%.
- **Oil FVF reference: fixed by #36.** `PVCDO 300 1.0 1e-5` puts Bo = 1 at 300 bar. ResSim used
  to reference `b_o` to 0 bar, 0.30% more surface oil. It now references it to the initial
  pressure, as the deck does.

With the SWOF matched, cumulative oil and injection agree to ≤ 0.005% at every matched step. See
`docs/BLACK_OIL_VALIDATION.md` "#21" and "#36".

