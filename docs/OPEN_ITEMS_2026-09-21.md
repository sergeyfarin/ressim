# Open items — 2026-09-21

Base: `8945f75` plus the work of this session. One place to look for "what did we decide not to do
yet, and why", so that a deliberate deferral is not mistaken later for an oversight.

Each item says what it is, why it was not done, and what would close it. Items owned by a specific
document link there rather than being restated; this file is an index of debt, not a second copy
of the reasoning.

## 1. FIM substeps differently on wasm32 than on x86-64 — **new, and the most important**

**Measured 2026-09-21**, `crates/ressim-py/parity/cases.json` case `buckley-fim`, on the committed
Buckley case A fixture:

| Target | Substeps recorded for the **first** 1-day step | Max pressure difference |
|---|---|---|
| wasm32 (browser bindings, under Node) | **4** | — |
| x86-64 (native bindings) | **18** | **8.28 bar** |

This is not floating-point noise amplified over a long run: it is present on the very first step,
and the trajectories then differ by whole bars. IMPES, on the same fixture and the same two
targets, agrees to **1e-12**.

**What was ruled out**, each by experiment rather than by argument:

- *Build profile* — a `--release` native build gives the same 18 substeps as debug.
- *The `wasm` feature added in S4* — a native build **with** the feature also gives 18. S4 did not
  cause this.
- *Configuration drift between the two runners* — the runners used `add_well` and
  `addWellWithId`, which select different `WellGroupingKey` variants. Binding `add_well_with_id`
  for Python so both configure identically changed nothing.

That leaves the target itself. The plausible mechanism is that the linear stack
(`faer`/`nalgebra`/`sprs`) dispatches different SIMD paths on x86-64 than on wasm32, changing
summation order and therefore rounding, and FIM's adaptive controller branches on convergence
tolerances — so a difference far below any physical tolerance flips a cut/accept decision and the
substep counts diverge. **This has not been confirmed**, and confirming it is the first step
toward closing this item.

**Why it matters beyond curiosity.** The FIM convergence baselines in `docs/FIM_STATUS.md` were
measured through `scripts/fim-wasm-diagnostic.mjs`, i.e. **wasm**. The Rust suite, including every
FIM gate, runs **natively**. Those two have been measuring materially different solver behaviour,
and nothing in the repository would have revealed it — the native/wasm parity gate did not exist
until this session, and its first fixture ran IMPES.

**To close:** confirm or refute the SIMD-dispatch mechanism; then decide whether the baselines are
per-target (and label them so) or whether the controller should be made target-stable. Until then
the two FIM cases in the parity matrix are marked non-strict and report their divergence on every
run rather than failing.

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
