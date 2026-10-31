# Open items — 2026-09-21

Original base: `8945f75` plus the work of that session. Tracking reconciled 2026-10-05.
One place to look for "what did we decide not to do yet, and why", so that a deliberate deferral
is not mistaken later for an oversight.

Each item says what it is, why it was not done, and what would close it. Items owned by a specific
document link there rather than being restated; this file is an index of debt, not a second copy
of the reasoning.

## 1. FIM substeps differently on wasm32 than on x86-64 — **root cause and follow-up closed**

Was: the committed Buckley case A fixture took 4 substeps on wasm32 and 18 natively for its first
1-day step, with 8.28 bar between them. A first analysis traced it to a `cfg(target_arch)` backend
split (sparse LU native, dense LU wasm), but that was the trigger rather than the defect.

**Root cause (FIM-DIRECT-001):** the inactive two-phase gas unknown lost its Jacobian slope
whenever a direct solve left it at negative roundoff (`max_floor(0.0)` in
`fim/properties.rs::cell_props_generic`). Every later Jacobian in the step was then exactly
singular, and both LUs correctly refused it. Which backend's roundoff came out negative decided
which target fragmented. Fixed and pinned by two tests; native and wasm then agreed to **5.7e-14**
on every parity case, all of which are strict. Full record:
[`FIM_CROSS_TARGET_DIVERGENCE_2026-09-22.md` §9](FIM_CROSS_TARGET_DIVERGENCE_2026-09-22.md). Since
#62 (§10) they agree bit for bit: the platform libm was the last difference.

**Routing unified (2026-09-22).** The routing no longer depends on the target. Small systems try
sparse LU, then dense LU as a backup when sparse refuses, then iterative CPR. `setFimDirectBackend("dense")`
(`set_fim_direct_backend` natively and in Python) swaps the order at runtime if a
factorization-specific problem ever appears. Sparse was 5–6× faster natively and 2.3–26× in wasm,
with the same answers. Checked against OPM Flow on five new decks under 512 rows:
[`opm/reference-decks/small-direct/`](../opm/reference-decks/small-direct/README.md).
`fim/unify-linear-routing` (`f5a9577`) is superseded.

**Follow-up closed 2026-09-25:** the `FIM_STATUS.md` long-horizon table was re-measured on
`5c29e0e` and reproduces to within one substep. It is recorded in
[`BENCHMARKS.md` §7](BENCHMARKS.md#7-fim-convergence-wasm), next to the native/wasm parity result.

## 1a. Bubble-point fragmentation in three-phase FIM — **fixed (FIM-BUBBLE-001, `3fdefa4`, merged)**

The first 5-day step of the depletion column crossed the bubble point in ~21,800 substeps;
Flow takes 4. A finite-difference Jacobian audit at the stuck state found every mismatch in an
Sg column, with AD equal to the forward difference and the backward difference zero. Every
cell was tagged Saturated with Sg = ±1e-18, on the Sg = 0 relperm kink.

The fix has two halves that only work together:

- **A.** Gas-free cells start on Rs, as OPM's `assignNaive` does.
- **B.** The validated Y2b3 per-update Sg↔Rs lifecycle is now on by default on every target.

Each alone is worse (58,874 and 27,959 substeps).

Result against Flow: 23 substeps for the 100 days (Flow 23). Worst pressure 0.005 bar, Sg 1e-5,
cumulatives 0.01%. SPE1 unchanged.

**The trade:** the wasm gas-injection control `gas-rate 10x10x3` keeps its substeps and answers
(3e-7) but takes 118 → 129 Newton iterations over 24 steps, mostly in step 1 (9 → 12; Flow 7).
That cost belongs to B's OPM-faithful in-Newton switching, and the registry already recorded it
for Y2b3. Evidence: worklog "FIM-BUBBLE-001".

## 1b. Bubble-point PVT boundary conventions (FIM-KINK-001 J1) — **open, coupled**

Owned by [#35](https://github.com/sergeyfarin/ressim/issues/35). The remaining issue is the
saturated derivative path selected when an undersaturated Rs primary sits exactly at Rs_sat(p).
The former flat-Rs convention above the last table row is fixed (`1867ec0` / `fb662a8`,
FIM-KINK-002), including the redissolving SPE1 Case 2 physics discrepancy. The generated-deck
dep-pvt gas gap was separately fixed through #55's SGOF sampling; it is not evidence for J1.

The remaining Newton-efficiency gain is not measured. The old “111 → 86 against Flow 63”
estimate predates later fixes and must not drive implementation. Read the current issue,
registry and worklog before replaying the coupled convention matrix.

## 2. Cross-client coverage: catalog-driven since #64; compositional and FIM toggles stay unbound

Resolved for the black-oil catalog by [#64](https://github.com/sergeyfarin/ressim/issues/64).
`ressim-py` now binds every setter the worker's `configureReservoirSimulator` can call, and
`scripts/validate-native-binding.sh` replays every black-oil catalog run (185 runs, 135 distinct
engine setups) through both bindings from the worker's own recorded calls, requiring bit
identity. A catalog configuration the shim cannot express now fails that gate by name. The
matrix, its bounds and the current coverage are in
[`crates/ressim-py/parity/CONFIGURATION_COVERAGE.md`](../crates/ressim-py/parity/CONFIGURATION_COVERAGE.md).

Deliberately not done:

- **`CompositionalSimulator` is not bound.** `comp_co2_1d` (7 runs) is the only catalog scenario
  on it, and `scripts/validate-compositional.sh` owns its native check. Binding a second engine
  class is its own piece of work, with no consumer asking for it yet.
- **The eight FIM experiment toggles are not bound** (`setFimOpmAlignedNonlinear`,
  `setFimNestedWellSolve`, `setFimTrueFgmres`, `setFimFlowLifecycle`, `setFimFlowResvInjector`,
  `setFimOpmEndpointRelperm`, `setFimCoreyTablePoints`, `setFimOpmWaterHeavySwof`). No catalog
  case or worker path sets them; bind one when a native experiment needs it.
- **Catalog runs replay a prefix**, two steps of at most 0.1 day, not their full horizon. Parity
  is a property of the binding; the full horizons are the scenario tests' and the cross-solver
  gate's.

## 3. Deletion candidate: `getFimStepStatsHistory`

API decision owned by [#24](https://github.com/sergeyfarin/ressim/issues/24).

No call site in live code; only `.archive/` prose. Ported rather than deleted in Phase 1 because
the evidence behind the original "two dead functions" claim was **wrong about the other one** —
`getLastFimStepStats` is used by `scripts/fim-wasm-diagnostic.mjs`. A removal deserves its own
deliberate change rather than being folded into a refactor.

## 4. `api::GridState` carries no schema version

Compatibility policy owned by [#65](https://github.com/sergeyfarin/ressim/issues/65).

Now a one-line addition, since the type is named and in one place
(`docs/ENGINE_PAYLOAD_BOUNDARY_DESIGN_2026-09-21.md` §6.2). Deferred because it is a *behaviour*
decision — what should happen to a payload that lacks one — not a refactor.

## 5. Package `exports` are still `./*` wildcards

Public-surface audit owned by [#66](https://github.com/sergeyfarin/ressim/issues/66).

S2 left them open because nothing had yet shown what a second page needs. S3 then showed it: three
modules out of 36 across five packages (`analytical/fractionalFlow`, `charts/ChartSubPanel.svelte`,
`primitives/ToggleGroup.svelte`). Tightening is now a decision with evidence behind it.
`src/lib/packageBoundaries.test.ts` enforces entry-by-name in the meantime, but that is a test, not
a build-level guarantee.

## 6. Three.js dependency updates need current visual verification (#24)

Originally bumped to 0.186.0 in `adac71b`; `package.json` now pins 0.186.1 at `d4bbdd7`.
The built-app visual check is owned by [#24](https://github.com/sergeyfarin/ressim/issues/24). The repo constraint calls Three.js pinned and the visualization
"version-sensitive", and the only evidence the upgrade is safe is a suite that does not render.
`visualization/` is also the area the split plan designates least portable. If a 3D rendering
regression appears, this is the first place to look.

## 7. Rust build warnings — **closed**

`b2acb85` gated test-only items, retired unused layout helpers and renamed EOS fixture fields;
`7b26905` replaced the deprecated EPSILON constant. The release build for the #58 replay on
clean `d4bbdd7` emitted no Rust warnings. The original feature-split warning note is historical.

## 8. IMPES depletion fixture ran with a 2 bar pressure cap (#11) — **closed by #37 (`30bde0d`)**

The #11 fix left IMPES needing a pressure cap of a few bar through the bubble point, because it
booked storage errors as oil. #37 made three-phase IMPES conservative, and the fixture is back on
the 75 bar default, 0.04 bar from Flow.

**Two-phase follow-up repaired by [#63](https://github.com/sergeyfarin/ressim/issues/63).**
Water and oil are transported as surface components, with saturation recovered against
pressure-dependent pore volume and FVF. The old residual-oil closure is removed. Independent
inventory gates cover compressible depletion, closed redistribution and all small-direct IMPES
cases. Baseline impact, matched Flow comparisons and timestep refinement are recorded in
`BLACK_OIL_VALIDATION.md` §4b. The combined layered sweep uses FIM for interactive runtime;
conservative IMPES remains a mandatory native reference path. [#71](https://github.com/sergeyfarin/ressim/issues/71)
profiles and reduces its component-volume correction cost; evidence and validation are in
[IMPES_VOLUME_CLOSURE_PROFILE.md](IMPES_VOLUME_CLOSURE_PROFILE.md). The FIM default is retained:
changing that product policy requires its own browser validation and is outside this optimization.

## 9. Cross-solver harness: remaining harness work (2026-09-25, #22)

`scripts/validate-cross-solver.sh` gates FIM sparse, FIM dense and IMPES against OPM Flow on the
committed small-direct case inventory. It has expanded beyond the initial eight decks.

**Closed 2026-10-31 (#22): compositional cases share the record and scorecard.**
`scripts/validate-compositional.sh reference` writes each committed trajectory fixture's ResSim
counterpart as `<case>.comp.json` — the same `<case>.<run>.json` shape, with run id `comp` and
overall composition `z` — and grades it with `tools/opm_flow/compare_small_direct.py` against the
same `scorecard.json`. The fixture remains the oracle. `--write-scorecard` merges the cases an
invocation measured and keeps the rest, because the black-oil and compositional gates update
different families. There is no second comparator.

**Closed 2026-10-31 (#22):** the scorecard stores each FIM run's `newton_ratio` against Flow, and
`bash scripts/validate-cross-solver.sh --grid-refine` runs the uniform `bo-1d` column at 10/20/40
cells through FIM sparse and IMPES, reporting Richardson order and contraction with no external
simulator. `bo-1d-20` is the middle rung in the normal deck/scorecard inventory.

These remain outside it; #22 owns the remaining harness work:

- **The ≥ 900-row decks and the wasm presets** (`opm/reference-decks/{gas-rate,water-*}`). These
  are hand-mapped decks with known mapping gaps (that README's "#21"), driven through
  `fim-wasm-diagnostic.mjs`. Their convergence numbers still live in `SOLVER_COMPARISON_SUMMARY.md`
  and `FIM_STATUS.md` as typed tables. Close by generating those decks from the simulator the way
  `opm_small_direct.rs` does, then adding them as cases.
- **The wasm target.** The harness runs natively. Native/wasm parity is
  `validate-native-binding.sh`'s job: ~1e-13 since FIM-DIRECT-001, bit-identical since #62.
- **A second independent simulator** is wired in since #54 phase 3: JutulDarcy runs the
  small-direct decks and `gas_drive` in `benchmarks.sh`'s full tier (`BENCHMARKS.md` §9). What it
  does not cover, deliberately: SPE1 (it ignores `DRSDT`, so it runs Case 2 physics), cumulatives
  (its summary integrates end-of-step rates over report intervals), and compositional cases on
  that common scorecard. #51 separately adds a JutulDarcy compositional dew-point admission
  fixture; see [SPE3 validation](SPE3_VALIDATION.md).
- **SPE3 / SPE5.** Full-case execution remains #52, after the remaining engine enablers
  (#45–#51). Component capacity and SPE5's fluid (#45), sparse linear routes (#49), and SPE3's
  characterization, staged separation and dew-point admission (#51) are implemented. The latter
  validates a small 1D depletion case, not the full SPE3 reservoir.
- **CI.** CI has no Flow. A committed Flow-output cache would let CI run the check, but it would
  cost several MB of reference fields in the repository. Not worth it while the gate takes 20 s
  locally.

## 10. IMPES dissolved-gas limit is a constant, not a stability parameter (2026-09-25, #44)

IMPES now cuts a substep whose flash would change any cell's Rs by more than 5% of the saturated
Rs at the cell's pressure (`MAX_RS_RELATIVE_CHANGE_PER_STEP` in `impes/pressure.rs`). The other
three limits (saturation, pressure, well rate) are set through `setStabilityParams`. This one was
left out of it on purpose: a fourth argument changes the worker payload, the Python binding and
every caller, and no scenario or user needs to tune it. It binds only on bo-1d among the eight
small-direct decks and leaves the `dep_*` scenario tests unchanged. Close by adding it to
`setStabilityParams` if a scenario ever needs a looser or tighter value.

## 11. JutulDarcy and Flow disagree on dep-pvt's free-gas rate; ResSim agrees with Flow (2026-09-26, #55)

On the generated `dep-pvt-*` decks, JutulDarcy 0.3.7's final gas rate is −2.3 % and −4.5 % from
Flow's (−1.7 % and −1.9 % once #60 corrected the fluid to its stated bubble point). FIM is −0.22 % and −0.01 % from Flow. Denser SGOF and a 10× finer report step both leave the
difference in place. JutulDarcy's producer pressure is already 0.07 bar off Flow's before any free
gas flows, which points at how the two read the PVT tables. It was not pursued because it is a
difference between two reference simulators on a deck where ResSim matches one of them, and
isolating it means working inside JutulDarcy. Evidence:
[`small-direct/README.md`](../opm/reference-decks/small-direct/README.md), "Saturation tables:
nodes above S_gc, and a table's own knots". Close it if a JutulDarcy upgrade or a third simulator
settles which convention is right, or if a ResSim change starts to depend on it.

## Closed this session, for the record

- The import gates were blind to workspace package specifiers, so a cycle spanning two packages
  would have passed CI silently. Fixed and tested (`4b30109`).
- Two architecture tests asserted import *spelling*; one was a negative assertion that would have
  stopped guarding the moment `catalog` became a package (`bf80666`).
- `test:deployed` and the native binding gate were absent from PR CI. Both added, and the smoke
  step was found to hang before it landed: a background server inheriting stdout keeps the step's
  pipe open, and `kill %1` silently fails because job control is off in non-interactive shells.
