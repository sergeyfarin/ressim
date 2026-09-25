# Small-direct OPM decks (every case under 512 rows)

Every other deck under `opm/reference-decks/` has ~900+ rows. That is above the 512-row threshold
where FIM switches from a direct LU to iterative CPR, so none of them exercises the small direct
path FIM-DIRECT-001 was about. These do.

| Case | Grid | Rows | What it is |
|---|---|---|---|
| `ow-1d-96` | 96×1×1 | 292 | `wf_bl1d` geometry: BHP waterflood, M ≈ 2 |
| `ow-1d-50-adverse` | 50×1×1 | 154 | the parity matrix's adverse-mobility FIM case (μo = 20) |
| `ow-2d-12x12` | 12×12×1 | 436 | heterogeneous quarter five-spot; took 9,021 substeps before FIM-DIRECT-001 |
| `bo-1d-10` | 10×1×1 | 32 | `physics_depletion_grid_convergence_fim` at nx=10: depletion through the bubble point |
| `bo-1d-40` | 40×1×1 | 122 | the same column at nx=40 |
| `go-1d-50` | 50×1×1 | 152 | `gas_injection` base case: dead oil displaced by gas, both wells on BHP |
| `dep-pvt-correlation` | 48×1×1 | 146 | `dep_pvt` base case: constant-rate (3 Sm³/d ORAT) blowdown through the bubble point |
| `dep-pvt-lab-report` | 48×1×1 | 146 | the same with the lab-report table (2.5× the undersaturated c_o) |
| `gas-drive-20` | 20×1×1 | 60 | `three_phase_acceptance` `gas_drive`: saturated solution-gas drive, BHP producer, redissolution on (#55) |
| `spe1-10x10x3` | 10×10×3 | 900 | `spe1_acceptance` SPE1 Case 1: gravity, three layers, gas rate injector, ORAT producer with a BHP floor (#55) |

The last two are not small: SPE1 is past the forced-direct threshold. They are here for the
one-definition deck writer, which settled #55: their hand-written Flow decks disagreed with
ResSim by 3–8 %, and these decks, written from the acceptance tests' own simulators, agree to
0.1–0.8 % (see "Generated decks for the #55 gaps" below). A gravity-on case gets real `TOPS`, the
engine's surface densities and an explicit well datum, and its header says to run plain `flow`;
`compare_small_direct.py` takes the Flow options from that header.

**The decks are generated, not hand-written.** `src/lib/ressim/src/tests/opm_small_direct.rs` builds
each case as a ResSim simulator and writes its deck from that same object. SWOF, SGOF, PVDO,
PVTO and PVDG are ResSim's own relperm and PVT functions sampled onto dense nodes: 91–161
saturation points with the Corey kinks on nodes, and PVT about every 2.5 bar with the table's own
rows (the bubble point among them) on nodes. Differences in how the two simulators interpolate
between nodes are therefore second-order. Regenerate rather than edit.

A PVT table that is not a scalar input comes from a committed fixture both sides assert against:
`dep-pvt-tables.json` is what `generateBlackOilTable` makes for `dep_pvt`, and `dep_pvt.test.ts`
fails if the scenario stops shipping it.

Three of these decks are also the frontend's OPM references (`gas_injection`,
`dep_pvt_correlation`, `dep_pvt_lab_report` in `tools/opm_flow/opm_flow_tool/cases.py`), which is
why every deck requests FPR/FVIT/FGOR and writes a text summary (RUNSUM/SEPARATE). The artifact
records the deck's SHA-256, so regenerating a deck without re-running Flow fails
`opmReferenceWiring.test.ts`.

## Gate: `scripts/validate-cross-solver.sh`

```bash
bash scripts/validate-cross-solver.sh              # FIM sparse + dense + IMPES vs Flow, checked against scorecard.json
bash scripts/validate-cross-solver.sh --update     # re-baseline, on a committed tree only
bash scripts/validate-cross-solver.sh --markdown   # the tables below, generated rather than typed
bash scripts/validate-cross-solver.sh --refine 0.025 --case ow-2d-12x12   # time-refined referee
```

The script runs the three steps under "Replay" for every ResSim solver and checks the result against
[`scorecard.json`](scorecard.json). That file is written by the comparator and never edited by hand.
It fails when:

- an accuracy metric against Flow gets worse than its band: worst and final cell Δp/ΔSw/ΔSg, and
  cumulatives;
- a work metric gets worse than its band: substeps, Newton;
- a run raises a solver warning the scorecard did not record;
- sparse and dense stop agreeing;
- a run the scorecard covers produces no output.

Bands are `BANDS` in `tools/opm_flow/compare_small_direct.py`. Each is a relative allowance plus an
absolute floor, so roundoff on a near-zero baseline is not a failure.

Improvements past the band are printed. Record them with `--update` and commit the scorecard with
the change that earned them. Its `provenance` then names the commit, the Flow version and the
command. Flow output is cached per deck SHA-256, so a warm check takes about 20 s.

Before this gate, every result table below was typed into this README by hand after each fix, and
nothing noticed when a later change moved a number.

## Replay

```bash
# decks (only after changing a case)
OPM_SMALL_DECK_DIR=$PWD/opm/reference-decks/small-direct cargo test --release \
  --manifest-path src/lib/ressim/Cargo.toml opm_small_direct_write_decks -- --ignored
# ResSim: FIM once per small-system LU, then IMPES
for b in sparse dense; do OPM_SMALL_OUT=/tmp/small-direct OPM_SMALL_SOLVER=fim OPM_SMALL_BACKEND=$b cargo test --release \
  --manifest-path src/lib/ressim/Cargo.toml opm_small_direct_run_ressim -- --ignored --nocapture; done
OPM_SMALL_OUT=/tmp/small-direct OPM_SMALL_SOLVER=impes cargo test --release \
  --manifest-path src/lib/ressim/Cargo.toml opm_small_direct_run_ressim -- --ignored --nocapture
# Flow + comparison (Flow always runs with --enable-gravity=false)
python3 tools/opm_flow/compare_small_direct.py --ressim-dir /tmp/small-direct --flow-dir /tmp/small-direct-flow
```

## Results, 2026-09-22 (`flow 2026.04`, native release)

Worst cell difference against Flow over **every** report step. Cumulatives are compared at the end
and omitted when below 0.1% of the case's largest cumulative: trace volumes such as immobile
connate water or pre-breakthrough water.

| Case | Simulator | Substeps | Newton | Retries / cuts | Wall ms | max \|Δp\| bar | max \|ΔSw\| | max \|ΔSg\| | Cumulatives vs Flow |
|---|---|---|---|---|---|---|---|---|---|
| ow-1d-96 | Flow | 121 | 259 | 0 | 1115 | | | | |
| | ResSim sparse | 120 | 376 | 0 | 160 | 0.94 | 0.032 | — | FOPT 0.05%, FWPT 0.10%, FWIT 0.07% |
| | ResSim dense | 120 | 376 | 0 | 874 | 0.94 | 0.032 | — | identical |
| ow-1d-50-adverse | Flow | 13 | 33 | 0 | 526 | | | | |
| | ResSim sparse | 12 | 48 | 0 | 11 | 0.38 | 0.0042 | — | FOPT 0.08%, FWIT 0.10% |
| | ResSim dense | 12 | 48 | 0 | 24 | 0.38 | 0.0042 | — | identical |
| ow-2d-12x12 | Flow | 40 | 114 | 0 | 663 | | | | |
| | ResSim sparse | 40 | 135 | 0 | 177 | 0.75 | 0.0079 | — | FOPT 0.08%, FWPT 0.81%, FWIT 0.21% |
| | ResSim dense | 40 | 133 | 0 | 1069 | 0.75 | 0.0079 | — | same to 0.01 bar |
| bo-1d-10 | Flow | **23** | **63** | 0 | 558 | | | | |
| | ResSim sparse | **21,816** | **237,018** | 1,865 | 32,813 | 0.45 | 0.000 | 0.0052 | FOPT 0.99%, FGPT 4.23% |
| | ResSim dense | 24,296 | 264,365 | 2,034 | 28,576 | 0.53 | 0.000 | 0.0055 | FOPT 1.32%, FGPT 4.54% |
| bo-1d-40 | Flow | **24** | **57** | 0 | 565 | | | | |
| | ResSim sparse | **19,027** | **208,903** | 1,437 | 85,299 | 0.65 | 0.000 | 0.0059 | FOPT 1.04%, FGPT 2.75% |
| | ResSim dense | 19,443 | 213,102 | 1,518 | 120,269 | 0.66 | 0.000 | 0.0057 | FOPT 0.95%, FGPT 2.54% |

Reading it:

- **Oil–water: ResSim matches Flow** to under 1 bar and 0.03 Sw anywhere, at any time. The Sw
  maximum is the displacement front landing one cell apart. Cumulatives agree to 0.05–0.8%, and
  substeps match. ResSim takes 1.2–1.5× Flow's Newton iterations.
- **Sparse and dense give the same answer**: 1e-10 bar in 1-D, and 0.01 bar on 12×12, where
  roundoff crosses one adaptive threshold. Sparse is 5–6× faster natively and 2.3–26× faster in
  wasm.
- **Bubble point: the answer is right and the work is not.** The final state is within 0.5–0.7 bar
  and 0.006 Sg of Flow, but ResSim needs about 1,000× the substeps and 3,500× the Newton
  iterations Flow does. That is the open bubble-point fragmentation defect
  (`docs/OPEN_ITEMS_2026-09-21.md` §1a). Flow's first 1-day step there takes 19 Newton
  iterations and **does not cut**.

### After FIM-BUBBLE-001 (branch `fim/bubble-point-lifecycle`)

The bubble-point rows above are the fragmentation defect. With the OPM primary-variable
lifecycle (gas-free cells start on Rs, and Sg↔Rs adapts every Newton update) the oil–water rows
are unchanged and the black-oil rows become:

| Case | Simulator | Substeps | Newton | Retries | Wall ms | max \|Δp\| bar | max \|ΔSg\| | Cumulatives vs Flow |
|---|---|---|---|---|---|---|---|---|
| bo-1d-10 | Flow | 23 | 63 | 0 | 553 | | | |
| | ResSim sparse | **23** | 111 | 0 | 12 | **0.0048** | **1e-5** | FOPT 0.01%, FGPT 0.01% |
| | ResSim dense | 23 | 101 | 0 | 9 | 0.0137 | 2e-5 | FOPT 0.01%, FGPT 0.01% |
| bo-1d-40 | Flow | 24 | 57 | 0 | 557 | | | |
| | ResSim sparse | **25** | 109 | 1 | 40 | **0.063** | **9e-5** | FOPT 0.05%, FGPT 0.04% |
| | ResSim dense | 25 | 105 | 1 | 55 | 0.067 | 1e-4 | FOPT 0.05%, FGPT 0.04% |

ResSim still takes ~1.8× Flow's Newton iterations here. Substeps and answers now match.

### Stable PVT table (#11, `c225102`)

The `bo-1d` decks were regenerated after the fixture's Bo at 100 bar moved from 1.05 to 1.08. The
old table let the two-phase volume factor grow with pressure, i.e. negative total
compressibility below the bubble point ([`BLACK_OIL_VALIDATION.md` §2](../../../docs/BLACK_OIL_VALIDATION.md)).
The black-oil rows above were measured on that old table. On the regenerated decks:

| Case | Simulator | Substeps | Newton | Retries | Wall ms | max \|Δp\| bar | max \|ΔSg\| | Cumulatives vs Flow |
|---|---|---|---|---|---|---|---|---|
| bo-1d-10 | Flow | 22 | 49 | 0 | 574 | | | |
| | ResSim sparse | 22 | 85 | 0 | 9 | 0.0003 | 0.00000 | FOPT 0.00%, FGPT 0.00% |
| | ResSim dense | 22 | 87 | 0 | 9 | 0.0004 | 0.00000 | FOPT 0.00%, FGPT 0.00% |
| bo-1d-40 | Flow | 22 | 50 | 0 | 565 | | | |
| | ResSim sparse | 23 | 92 | 0 | 30 | 0.184 | 0.00022 | FOPT 0.01%, FGPT 0.04% |
| | ResSim dense | 22 | 86 | 0 | 37 | 0.0015 | 0.00000 | FOPT 0.00%, FGPT 0.00% |

On bo-1d-40, sparse takes one more substep than Flow and dense. The worst-report Δp of 0.18 bar
is that transient; the two backends agree at the end. Replay: the three commands above,
with `--case bo-1d-10` / `--case bo-1d-40`.

Wall times are single observations and include Flow's process start (~0.5 s). Flow's own
timings are in each run's `CASE.INFOSTEP`.

### Top-branch Bo above the bubble point (#38, `1b7853e`)

`bo-1d-10` / `bo-1d-40` were regenerated after #38. Their undersaturated PVTO rows come from ResSim's
`interpolate_oil`, which above the highest bubble point had extrapolated `Bo` with the scalar `c_o`
instead of following the table's own 200-bar row. Against the new Flow runs:

| Case | Simulator | Substeps | Newton | max \|Δp\| bar | max \|ΔSg\| | Cumulatives vs Flow |
|---|---|---|---|---|---|---|
| bo-1d-10 | Flow | 22 | 52 | | | |
| | ResSim sparse / dense | 22 / 22 | 84 / 84 | 0.0009 / 0.0005 | 0.00000 | FOPT 0.00%, FGPT 0.00% |
| bo-1d-40 | Flow | 23 | 54 | | | |
| | ResSim sparse / dense | 22 / 22 | 91 / 90 | 0.274 / 0.274 | 0.00033 | FOPT 0.05%, FGPT 0.12% |

The bo-1d-40 pressure maximum is a transient: Flow takes one more substep. The final averages agree
to 0.008 bar (`docs/BLACK_OIL_VALIDATION.md` §2). `go-1d-50` has no PVT table and is unchanged.

### `gas_injection` twin (#12)

`go-1d-50` is the `gas_injection` catalog scenario's base case, written by
`opm_small_direct::gas_injection_1d`: dead oil with no PVT table, and gas injected at 350 bar into a
column produced at 100 bar. The deck branch is `OIL WATER GAS` without `DISGAS`, with ResSim's own
dead-oil `PVDO` and a `PVDG` for its table-less gas, `Bg = exp(−c_g·(p − p_ref))` since #42. Before
#42 that gas had a constant `Bg = 1`, which Flow will not accept flat, so the first version of the
deck tilted it by 1e-9 per bar.

| Case | Simulator | Substeps | Newton | Retries | Wall ms | max \|Δp\| bar | max \|ΔSg\| | Cumulatives vs Flow |
|---|---|---|---|---|---|---|---|---|
| go-1d-50 | Flow | 151 | 339 | 0 | 1492 | | | |
| | ResSim sparse | 151 | 486 | 0 | 140 | 0.50 | 0.0050 | FOPT 0.01%, FGPT 0.03%, FGIT 0.02% |
| | ResSim dense | 151 | 486 | 0 | 260 | 0.50 | 0.0050 | identical to 1e-10 bar |

Measured after #42 (compressible table-less gas). Before it, with `Bg = 1`: 0.59 bar, 0.0049 Sg, the
same cumulative agreement. Gas breaks through between 170 and 180 d in both simulators. Cumulatives at 100 / 200 / 300 d are
embedded in `three_phase_gas_injection_matches_opm_flow_twin` (engine) and in
`src/lib/catalog/scenarios/gas_injection.test.ts`, which runs the shipped scenario through the
worker's setup. Replay: the three commands at the top, with `--case go-1d-50`
(`OPM_SMALL_CASE=go-1d-50` for the ResSim run).


### `dep_pvt` pair (#20, `91d3a21`)

`dep-pvt-correlation` / `dep-pvt-lab-report` are the withheld `dep_pvt` scenario's two rungs, written by
`opm_small_direct::dep_pvt_column` from `dep-pvt-tables.json`: a 48-cell column at 280 bar, 130 bar
above the bubble point, produced at 3 Sm³/d stock-tank oil (`ORAT`, BHP floor unreachable) for 300 ×
0.75 d. They are also the frontend's OPM references for `dep_pvt`.

| Case | Simulator | Substeps | Newton | Retries | Wall ms | max \|Δp\| bar | max \|ΔSg\| | Cumulatives vs Flow |
|---|---|---|---|---|---|---|---|---|
| dep-pvt-correlation | Flow | 300 | 608 | 0 | 4250 | | | |
| | ResSim sparse | 301 | 908 | 0 | 421 | 0.307 | 0.00023 | FOPT 0.00%, FGPT 1.06% |
| | ResSim dense | 301 | 908 | 0 | 606 | 0.307 | 0.00023 | identical to 1e-10 bar |
| dep-pvt-lab-report | Flow | 300 | 607 | 0 | 4253 | | | |
| | ResSim sparse | 301 | 906 | 0 | 413 | 0.221 | 0.00025 | FOPT 0.00%, FGPT 1.13% |
| | ResSim dense | 301 | 906 | 0 | 604 | 0.221 | 0.00025 | identical to 1e-10 bar |

Flow's FPR falls through 150 bar at 36.0 d and 87.75 d, the 2.4× clock ratio the scenario is built
around; average-pressure gap 15.2 bar at 6.75 d, 77.5 bar at 35.25 d, 14.6 bar at 168.75 d. No
fragmentation at the bubble point on either side. Replay: the three commands at the top, with
`--case dep-pvt-correlation` / `--case dep-pvt-lab-report` (`OPM_SMALL_CASE=…` for the ResSim run).

### IMPES joins the matrix, and a refined referee (2026-09-25, base `5c290b0`)

Until now only FIM ran on these decks. IMPES, the product solver, now runs on all eight. Numbers
come from `bash scripts/validate-cross-solver.sh --markdown`, run natively in release with
`flow 2026.04`. FIM rows reproduce the sections above exactly. Worst cell difference over every
report:

| Case | IMPES substeps | max \|Δp\| bar | max \|ΔSw\| | max \|ΔSg\| | Cumulatives vs Flow | FIM sparse, same columns |
|---|---|---|---|---|---|---|
| ow-1d-96 | 631 | 7.51 | 0.19 | — | FOPT 0.64%, FWPT 4.84%, FWIT 2.82% | 0.94 / 0.032 / — / FWPT 0.10% |
| ow-1d-50-adverse | 28 | 6.15 | 0.040 | — | FOPT 0.45%, FWIT 0.08% | 0.38 / 0.0042 / — / FOPT 0.08% |
| ow-2d-12x12 | 225 | 23.9 | 0.15 | — | FOPT 2.25%, FWPT 6.16%, FWIT 3.18% | 0.75 / 0.0079 / — / FWPT 0.81% |
| bo-1d-10 | **20** | 2.10 | 0.000 | 0.0011 | FOPT 0.20%, **FGPT 3.91%** | 0.0009 / 0 / 0 / 0.00% |
| bo-1d-40 | **20** | 2.15 | 0.000 | 0.0010 | FOPT 0.24%, **FGPT 3.97%** | 0.27 / 0 / 0.0003 / FGPT 0.12% |
| go-1d-50 | 213 | 12.6 | 0.000 | 0.074 | FOPT 0.81%, FGPT 0.57%, FGIT 0.67% | 0.50 / 0 / 0.005 / FGPT 0.03% |
| dep-pvt-correlation | 300 | 0.32 | 0.000 | 0.00026 | FOPT 0.00%, FGPT 2.10% | 0.31 / 0 / 0.0002 / FGPT 1.06% |
| dep-pvt-lab-report | 300 | 0.15 | 0.000 | 0.00028 | FOPT 0.00%, FGPT 1.59% | 0.22 / 0 / 0.0003 / FGPT 1.13% |

**At the same report step, FIM matches Flow far better than IMPES does. That is not evidence that
FIM is more accurate.** FIM and Flow share an implicit scheme and take the same steps, so they
share its time-step error. The referee is the same decks with a 10× finer report step
(`--refine`), where all three simulators are close to time-converged:

| Case (report dt) | Solver | max \|Δp\| bar | max \|ΔSw\| / \|ΔSg\| | final \|Δp\| bar | final \|ΔSw\| / \|ΔSg\| | Cumulatives vs Flow |
|---|---|---|---|---|---|---|
| ow-2d-12x12 (0.025 d) | FIM sparse | 1.18 | 0.0081 | 0.80 | 0.0081 | FWPT 0.83% |
| | IMPES | 5.51 | 0.052 | 0.32 | **0.0037** | FWPT 1.39% |
| ow-1d-96 (0.025 d) | FIM sparse | 0.75 | 0.0082 | 0.18 | 0.0082 | FWPT 0.18% |
| | IMPES | 10.7 | 0.064 | 0.23 | **0.0017** | FWPT 1.07% |
| ow-1d-50-adverse (0.025 d) | FIM sparse | 0.59 | 0.0042 | 0.34 | 0.0037 | FOPT 0.05% |
| | IMPES | 12.4 | 0.0041 | 0.23 | 0.0040 | FOPT 0.21% |
| go-1d-50 (0.2 d) | FIM sparse | 1.16 | 0.0036 | 0.010 | 0.0001 | FGPT 0.04% |
| | IMPES | 6.50 | 0.019 | 0.011 | 0.0002 | FGPT 0.04% |
| bo-1d-10 (0.5 d) | FIM sparse | 1.27 | 0.0001 | 0.0025 | 0.0000 | FGPT 0.05% |
| | IMPES | 0.21 | 0.0001 | 0.0099 | 0.0000 | FGPT 0.58% |
| bo-1d-40 (0.5 d) | FIM sparse | 1.18 | 0.0005 | 0.0054 | 0.0000 | FGPT 0.05% |
| | IMPES | 0.18 | 0.0001 | 0.0101 | 0.0000 | FGPT 0.62% |

Reading it:

- **IMPES converges to Flow under refinement.** Its final oil–water saturation gap is 0.0017–0.004,
  the same as or below FIM's. Most of the coarse-step IMPES "gap" is the implicit schemes' shared
  time error, not an IMPES defect.
- **The worst-over-reports Δp for IMPES is a start-up transient.** On ow-1d-50-adverse at 0.025 d
  it is 12.4 bar at the first report, 3.5 bar at the second, and ≤ 0.3 bar from 0.15 d onward.
  Mobilities are explicit, so the first substep's pressure solve sees the injector cell at
  connate water. This is why the scorecard gates the final state as well as the worst.
- **IMPES does not cut its step at the bubble point.** On bo-1d it takes exactly one substep per
  5-day report and ends 3.9% off Flow's FGPT. At 0.5 d the gap is 0.6%, so it falls with dt at
  roughly first order. Nothing in the IMPES step controller limits dt when a cell crosses the
  bubble point, and its saturation-change limit never trips there because Sg starts at zero.
  Tracked in [#44](https://github.com/sergeyfarin/ressim/issues/44), fixed in `58231be`: see the
  next section, which also shows that most of the 3.9% was Flow's time error, not IMPES's.
- Sparse against dense stays at 1e-10 bar on every case except bo-1d, where roundoff crosses one
  adaptive threshold and the gap is 9e-4 bar. That agreement is gated as an invariant.

The scorecard was first written on a dirty tree (provisional), then re-baselined on the committed
harness `35eb950` with `bash scripts/validate-cross-solver.sh --update` (`dirty: false`,
`flow 2026.04`). That supersedes the provisional one; the numbers did not change.

Replay: `bash scripts/validate-cross-solver.sh --markdown`, and for the referee
`--refine 0.025 --case ow-2d-12x12 --case ow-1d-96 --case ow-1d-50-adverse`, `--refine 0.2 --case go-1d-50`,
`--refine 0.5 --case bo-1d-10 --case bo-1d-40`. The refined Flow runs are slow: ow-1d-96 takes ~55 s
and go-1d-50 ~90 s.

### IMPES limits the dissolved-gas change per substep (2026-09-25, `58231be`, #44)

**Mechanism, confirmed.** Logging every IMPES trial on bo-1d-10 at base `0f9b2ce` showed that
every limit read 1.0 on every substep. Liberated gas stays under the critical saturation, so the
saturation limit measured zero change. The whole drop, from 175 bar to a 120 bar BHP, is under the
75 bar pressure limit. The first substep dropped 38 bar and took Rs from 15 to about 12.4 in one
explicit step.

**Fix.** A substep is now cut when its flash would change a cell's Rs by more than 5% of the
saturated Rs at the cell's pressure (`MAX_RS_RELATIVE_CHANGE_PER_STEP`, `impes/pressure.rs`).
The limit binds only on bo-1d. The other six decks are bit-identical and the `dep_*` scenario
tests take the same time.

**The referee changes the verdict.** At 5-day reports Flow and FIM share a ~2.2% time error in
FGPT, so the gate's "vs Flow" column mixes that error in. Cumulative gas in Sm³, from the harness
runs (`report.json` for Flow, each run's history for ResSim):

| Case | Report dt | Flow | FIM sparse | IMPES, before (`0f9b2ce`) | IMPES, after (`58231be`) |
|---|---|---|---|---|---|
| bo-1d-10 | 5 d | 82,485 | 82,484 | 85,707 (20 substeps) | 84,595 (25 substeps) |
| | 0.5 d | 84,079 | 84,119 | | 84,569 |
| | 0.1 d | 84,359 | 84,362 | | 84,459 |
| bo-1d-40 | 5 d | 78,298 | 78,207 | 81,410 (20 substeps) | 80,072 (28 substeps) |
| | 0.5 d | 79,656 | 79,697 | | 80,127 |
| | 0.1 d | 79,938 | 79,934 | | 80,038 |

Before the fix, IMPES at 5 d was 1.6–1.8% above Flow at 0.1 d. After it, IMPES is within 0.3% of
that at every report step, and its spread across report steps is at most 0.16%, against Flow's
2.1–2.3%. Its gap to Flow at 5 d is now 2.3–2.6%, which is Flow's time error, not an IMPES error. The issue's
"within ~1% of Flow at 5-day reports" criterion therefore does not apply: meeting it would mean
matching Flow's time error. The test `physics_depletion_impes_gas_production_is_independent_of_report_step`
pins the outcome that matters: 5-day and 0.5-day reports agree to 0.03% (1.35% before).

The scorecard was re-baselined on `58231be` with `--update`, which supersedes the `35eb950`
baseline for the bo-1d IMPES rows only. Against Flow at the same step, IMPES's worst Δp fell from
2.1 to 0.5 bar. Its final Δp rose from 0.017–0.025 to 0.086–0.097 bar, because Flow's 5-day final pressure
is itself 0.27–0.29 bar off Flow at 0.1 d. Against Flow at 0.1 d, IMPES's final pressure gap fell
from 0.25–0.27 to 0.18–0.20 bar.

Replay: `bash scripts/validate-cross-solver.sh --markdown --case bo-1d-10 --case bo-1d-40`, then the
same with `--refine 0.5` and `--refine 0.1` (`/tmp/ressim-cross-solver/{,refine-0.5/,refine-0.1/}report.json`).

### Generated decks for the #55 gaps (2026-09-25, flow 2026.04)

The two acceptance cases whose hand-written Flow decks disagreed with ResSim, written instead by
`opm_small_direct.rs` from `make_spe1_acceptance_sim` and `make_gas_drive_acceptance_sim`. FIM
against Flow, worst signed difference at report times (`FPR` hydrocarbon-pore-volume weighted as
Flow reports it):

| Case | Deck | p | q_o | GOR | Cum. oil |
|---|---|---|---|---|---|
| SPE1 10×10×3 | hand-written (`cases.SPE1_GAS_INJECTION`) | −3.06 % | +2.73 % | +7.13 % | — |
| | **generated** | −0.15 % | −0.53 % | −0.80 % | +0.04 % |
| `gas_drive` | hand-written (`cases.GAS_DRIVE`) | +1.59 % | +4.61 % | −6.08 % | +4.31 % |
| | **generated** | +0.55 % | +3.90 % (20 d transient; +0.76 % final) | +0.24 % | −0.09 % final |

What the hand-written decks got wrong, found by swapping their pieces into the generated deck
one group at a time and re-running Flow:

- **SPE1:** the hand deck has **no `ROCK`**, so Flow ran incompressible rock where ResSim (and
  published SPE1, 3e-6 /psi) has 4.35e-5 /bar. Alone that moves Flow by pressure +3.6 %, GOR
  −15 %. Its SPE1-published PVTO (undersaturated viscosity rising 0.51 → 0.74 cP by 621 bar,
  where ResSim's setup has one c_o and no branch data) and coarser SWOF/SGOF partly offset it.
  The three together reproduce the hand deck to 0.3 %. `EQUIL` against ResSim's uniform initial
  pressure, `TOPS`, and the well datums contribute about 0.1 %.
- **`gas_drive`:** only **SGOF**. Its Corey values are right at the nodes, but ten nodes are too
  few: between S_gc = 0.05 and 0.1286 Flow's linear interpolation overstates k_rg by up to 41 %,
  and the case starts at S_g = 0.08, inside that interval. PVT, initialization and wells change
  nothing.

The PVT table-edge conventions of #35 play no part in either.

