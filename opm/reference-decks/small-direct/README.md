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

**The decks are generated, not hand-written.** `src/lib/ressim/src/tests/opm_small_direct.rs` builds
each case as a ResSim simulator and writes its deck from that same object. SWOF, SGOF, PVDO,
PVTO and PVDG are ResSim's own relperm and PVT functions sampled onto dense nodes: 91–161
saturation points with the Corey kinks on nodes, and PVT every 2.5 bar. Differences in how the
two simulators interpolate between nodes are therefore second-order. Regenerate rather than edit.

## Replay

```bash
# decks (only after changing a case)
OPM_SMALL_DECK_DIR=$PWD/opm/reference-decks/small-direct cargo test --release \
  --manifest-path src/lib/ressim/Cargo.toml opm_small_direct_write_decks -- --ignored
# ResSim, once per small-system LU
for b in sparse dense; do OPM_SMALL_OUT=/tmp/small-direct OPM_SMALL_BACKEND=$b cargo test --release \
  --manifest-path src/lib/ressim/Cargo.toml opm_small_direct_run_ressim -- --ignored --nocapture; done
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

Wall times are single observations and include Flow's process start (~0.5 s). Flow's own
timings are in each run's `CASE.INFOSTEP`.
