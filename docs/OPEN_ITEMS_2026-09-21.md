# Open items — 2026-09-21

Base: `8945f75` plus the work of this session. One place to look for "what did we decide not to do
yet, and why", so that a deliberate deferral is not mistaken later for an oversight.

Each item says what it is, why it was not done, and what would close it. Items owned by a specific
document link there rather than being restated; this file is an index of debt, not a second copy
of the reasoning.

## 1. FIM substeps differently on wasm32 than on x86-64 — **root cause fixed 2026-09-22; one follow-up open**

Was: the committed Buckley case A fixture took 4 substeps on wasm32 and 18 natively for its first
1-day step, with 8.28 bar between them. A first analysis traced it to a `cfg(target_arch)` backend
split (sparse LU native, dense LU wasm), but that was the trigger rather than the defect.

**Root cause (FIM-DIRECT-001):** the inactive two-phase gas unknown lost its Jacobian slope
whenever a direct solve left it at negative roundoff (`max_floor(0.0)` in
`fim/properties.rs::cell_props_generic`). Every later Jacobian in the step was then exactly
singular, and both LUs correctly refused it. Which backend's roundoff came out negative decided
which target fragmented. Fixed and pinned by two tests; native and wasm now agree to **5.7e-14**
on every parity case, all of which are strict. Full record:
[`FIM_CROSS_TARGET_DIVERGENCE_2026-09-22.md` §9](FIM_CROSS_TARGET_DIVERGENCE_2026-09-22.md).

**Routing unified (2026-09-22).** The routing no longer depends on the target. Small systems try
sparse LU, then dense LU as a backup when sparse refuses, then iterative CPR. `setFimDirectBackend("dense")`
(`set_fim_direct_backend` natively and in Python) swaps the order at runtime if a
factorization-specific problem ever appears. Sparse was 5–6× faster natively and 2.3–26× in wasm,
with the same answers. Checked against OPM Flow on five new decks under 512 rows:
[`opm/reference-decks/small-direct/`](../opm/reference-decks/small-direct/README.md).
`fim/unify-linear-routing` (`f5a9577`) is superseded.

**Still open:**

- *Re-baseline `FIM_STATUS.md`.* Its wasm figures for two-phase cases may now reproduce natively;
  measure before citing any of them as cross-target.

## 1a. Bubble-point fragmentation in three-phase FIM — **fixed on branch `fim/bubble-point-lifecycle` (FIM-BUBBLE-001), awaiting merge decision**

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

The Jacobian sweep found one real defect cluster, at the bubble-point boundary of a live-oil
table.

- A cell whose Rs primary sits exactly at Rs_sat(p) gets the saturated curve's derivatives:
  d/dRs = 0 and the wrong d/dp.
- Three table-edge conventions differ from OPM: the flat Rs_sat above the table, a two-way
  definition of Bo at Rs_max, and the top-knot derivative.
- Fixing any one alone either helps one grid and pins the other, or pins both.

Only Newton efficiency is at stake here: substeps already match Flow, and the fix is worth about
111 → 86 Newton against Flow's 63 on the 10-cell column. It is deferred as a bundle rather than
patched piecewise. Evidence: worklog "FIM-KINK-001"; attempt diff on
`experiment/fim-kink-j1-pvt-boundary`.

## 2. Cross-client coverage is bounded by the Python shim, not by test effort

`ressim-py` binds **29** of the engine's **85** public API functions. Ten configuration knobs the
scenario catalog relies on are unbound, among them `setThreePhaseModeEnabled`, `setGravityEnabled`,
`setPermeabilityPerLayer`, `setInitialSaturationPerLayer`, `setWellSchedule`, `setTargetWellRates`,
`setInjectedFluid` and `setRockProperties`.

So "do all clients agree across the catalog's scenarios and sensitivities?" cannot be answered
today: the native client cannot **express** 17 scenarios × 53 sensitivity variants. The parity
matrix covers what it can — two solvers, two mobility ratios, two grid sizes — which is what
found item 1.

**To close:** bind the remaining configuration surface, then drive the matrix from the catalog's
own scenario definitions rather than a hand-written case list.

## 3. Deletion candidate: `getFimStepStatsHistory`

No call site in live code; only `.archive/` prose. Ported rather than deleted in Phase 1 because
the evidence behind the original "two dead functions" claim was **wrong about the other one** —
`getLastFimStepStats` is used by `scripts/fim-wasm-diagnostic.mjs`. A removal deserves its own
deliberate change rather than being folded into a refactor.

## 4. `api::GridState` carries no schema version

Now a one-line addition, since the type is named and in one place
(`docs/ENGINE_PAYLOAD_BOUNDARY_DESIGN_2026-09-21.md` §6.2). Deferred because it is a *behaviour*
decision — what should happen to a payload that lacks one — not a refactor.

## 5. Package `exports` are still `./*` wildcards

S2 left them open because nothing had yet shown what a second page needs. S3 then showed it: three
modules out of 36 across five packages (`analytical/fractionalFlow`, `charts/ChartSubPanel.svelte`,
`primitives/ToggleGroup.svelte`). Tightening is now a decision with evidence behind it.
`src/lib/packageBoundaries.test.ts` enforces entry-by-name in the meantime, but that is a test, not
a build-level guarantee.

## 6. Three.js moved 0.185.1 → 0.186.0 with no visual verification

Bumped in `adac71b`. The repo constraint calls Three.js pinned and the visualization
"version-sensitive", and the only evidence the upgrade is safe is a suite that does not render.
`visualization/` is also the area the split plan designates least portable. If a 3D rendering
regression appears, this is the first place to look.

## 7. Four pre-existing dead-code warnings in the Rust build

`CellPrimary` and the `fim/linear` layout accessors are reported unused in **both** feature
configurations. Pre-existing on master, unrelated to the feature split; surfaced here only because
the no-default-features build made warnings worth reading. They are the layout surface the FIM
repair published for reuse, so the fix is probably to consume or retire them, not to `allow` them.

## Closed this session, for the record

- The import gates were blind to workspace package specifiers, so a cycle spanning two packages
  would have passed CI silently. Fixed and tested (`4b30109`).
- Two architecture tests asserted import *spelling*; one was a negative assertion that would have
  stopped guarding the moment `catalog` became a package (`bf80666`).
- `test:deployed` and the native binding gate were absent from PR CI. Both added, and the smoke
  step was found to hang before it landed: a background server inheriting stdout keeps the step's
  pipe open, and `kill %1` silently fails because job control is off in non-interactive shells.
