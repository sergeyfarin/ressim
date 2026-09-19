# Architecture split execution plan

Date: 2026-09-19. Measurement base: `93c8c5c`, clean tree. **This is a plan, not a record of
completed work.** Every number in §2 is reproduced by the commands in §7; nothing here is a
baseline until a task records its own.

## Purpose, authority and completion boundary

Separate the simulation engine from the browser application far enough that (a) the engine can be
built for targets other than browser WASM, (b) the frontend can be composed into more than one
page, and (c) neither change requires rewriting the other.

This document owns the split sequence. GitHub Issues owns task status.
`COMPOSITIONAL_FLUID_EXECUTION_PLAN_2026-09-14.md` owns C13 and the compositional backlog;
where the two touch (S1, below) this plan states the dependency and does not restate C13's scope.
`FIM_MODEL_SOLVER_BOUNDARY_2026-09-18.md` owns the in-crate model/solver contract, which is a
different boundary from this one and is not affected by any task here.

Two explicit milestones:

- **FRONTEND-MODULAR-READY:** S1–S3 complete. Zero mutual value-level pairs between areas under
  `src/lib`, the candidate packages declared in the pnpm workspace, and a second page buildable
  from them without editing the first.
- **ENGINE-MULTI-TARGET-READY:** S4–S5 also complete. The crate builds and is tested for a
  non-WASM consumer, with the WASM shim behind a feature and no physics change.

Neither milestone requires a repository split, a rendering-stack change, or a new solver. A task
that cannot be completed without one of those is out of scope and should be re-planned, not
widened.

## 1. What is measured, and what is judgement

The distinction matters because the cheap conclusions here are measured and the expensive ones
are not.

**Measured** (§2, replayable via §7): the WASM coupling counts in the Rust crate; the number of
production modules importing the generated bindings; cross-area import coupling under `src/`; the
shipped artifact size; the documented grid-cell ceiling.

**Judgement, argued but not measured:** that one repository with several packages beats several
repositories (§5); that sharing a data contract with Python beats sharing a renderer (§S6); that
the four extractions in S1 are each small. The last is inferred from edge counts and the identity
of the symbols involved, not from having done them.

## 2. Current state

### 2a. The Rust core is already native-first

| Measure | Value at `93c8c5c` |
|---|---|
| Rust files under `src/lib/ressim/src/` | 122 |
| Files mentioning `wasm_bindgen` | **6** |
| Of those, occurrences inside the two `frontend.rs` shims | **92 of 98** |
| Real binding occurrences outside those shims | **3**, all in `lib.rs` |

The mentions in `fluid/units.rs`, `compositional/mod.rs` and `compositional/api.rs` are **doc
comments describing the boundary**, not code. `compositional/api.rs` already implements the
target shape deliberately — plain Rust over `serde`, natively testable, with
`compositional/frontend.rs` as a thin shell over it — and says so in its module docs. The
~840-test Rust suite runs natively under `cargo test`; the physics has never required WASM.

Residual coupling, complete:

1. `wasm-bindgen`, `js-sys`, `serde-wasm-bindgen` are unconditional dependencies.
2. `#[wasm_bindgen]` is on the `ReservoirSimulator` struct itself (`lib.rs`) and on
   `set_panic_hook`.
3. `getrandom` is pinned with `features = ["wasm_js"]`.
4. `crate-type` is already `["cdylib", "rlib"]`, so an rlib consumer works today.

### 2b. The engine/frontend boundary is one production file

`src/lib/workers/sim.worker.ts` is the **only** production module that imports
`src/lib/ressim/pkg/`. Every other importer is a `.test.ts` that drives WASM directly. Those test
files are a real constraint on S4 — see its validation — but they are not application code.

### 2c. Non-browser WASM already runs

`scripts/fim-wasm-diagnostic.mjs` executes the same generated bundle under Node. The FIM
convergence baselines in `docs/FIM_STATUS.md` were measured through it. "WASM outside the browser"
needs no new capability.

### 2d. The browser ceiling is documented policy

`SOLVER_COMPARISON_SUMMARY.md` §"Browser memory footprint" holds every convergence case to
`≤ 1200` cells, instructs that `> ~1500`-cell cases not be added without a memory-budget check,
and defers SPE10/PUNQ/Egg-scale work to the offline OPM pipeline. The browser is already the
binding constraint on what the simulator may attempt, which is the concrete reason S4 has value
beyond tidiness.

### 2e. Frontend coupling is small and named

`src/App.svelte` is 310 lines; areas under `src/lib` are already organised by concern. Measured
by `scripts/measure-module-coupling.mjs`:

| | Count at `93c8c5c` |
|---|---|
| Value (runtime) edges between areas | 83 across 39 ordered pairs |
| Type-only edges between areas | 78 — **not obstacles**, erased at build time |
| **Mutual value-level pairs** | **5** ← the split metric; target 0 |

The five, with the symbol that actually causes each:

| Pair | Cause | Resolution |
|---|---|---|
| `charts ↔ ui` | `ToggleGroup` used by `ReferenceComparisonChart`; `ChartSubPanel` used by two `ui/sections` | move `ToggleGroup` to a primitives leaf |
| `ui ↔ visualization` | `ToggleGroup` again, from `3dview.svelte` | **same move resolves both pairs** |
| `catalog ↔ charts` | catalog declares layouts (`chartLayoutConfig`, `curvePropertyRegistry`), `scenarioChartModel` resolves them | extract the chart-layout contract to a leaf both depend on |
| `charts ↔ lib-root` | `analyticalParamAdapters` uses `runSeries`/`reservoirVolumes`; `benchmarkDisclosure` uses `analyticalMethodRegistry` | move the shared series/volume helpers to a leaf |
| `catalog ↔ stores` | **one** edge inverts it: `caseLibrary.ts` → `stores/phase2PresetContract.ts` | move the preset contract out of `stores` |

Note the shape: `stores → catalog` is five legitimate edges and a *single* reverse edge creates
the pair. Four of the five pairs are resolved by moving a leaf concern, not by redesigning either
side.

## 3. Instructions for the executing model

1. One task per commit, in order. A task that grows beyond its stated scope stops and is
   re-planned rather than widened.
2. No behaviour change in S1–S3. These are moves and re-exports. If a rendered page or a test
   output changes, stop and isolate the edit rather than accepting the new output.
3. `pnpm run check:cycles` and `node scripts/measure-module-coupling.mjs` run after every task in
   S1–S3. The mutual-pair count is monotonically non-increasing across the sequence; an increase
   is a failed task.
4. No new runtime dependency without stating why an existing one cannot serve.
5. Do not begin S4 before S3 is green — a backend swap while the module graph is still mutual
   forces the same untangling under a harder constraint.

## 4. Task sequence

### S0 — Record the baseline

Commit the measurement script and this plan. Record the mutual-pair count, value-edge count and
the shipped `simulator_bg.wasm` size in a completion record, so that every later task can be shown
to have reduced coupling rather than moved it.

**Validation:** `node scripts/measure-module-coupling.mjs` output pasted into the record.
**Exit:** a committed revision and an exact replay command exist for every number in §2.

### S1 — Four leaf extractions to zero the mutual pairs

Each is a separate commit, each independently revertible, in this order:

1. `ToggleGroup` → a UI primitives leaf. Resolves `charts ↔ ui` and `ui ↔ visualization`.
2. Shared series/volume helpers (`runSeries`, `reservoirVolumes`) → a leaf. Resolves
   `charts ↔ lib-root`.

   **Corrected 2026-09-19 while executing.** This task originally also called for moving
   `analyticalMethodRegistry`'s descriptor lookup off `benchmarkDisclosure`'s path, to remove the
   `lib-root → charts` edge. That would have been wrong. Breaking a mutual pair needs only one
   direction removed, and the two directions are not interchangeable: `charts → lib-root` must go,
   because a package cannot import the application that consumes it, while `lib-root → charts` is
   the *correct* direction and is what "the app depends on the charts package" looks like. Removing
   it would have cost an edit and moved the codebase away from the target shape. Only the helper
   move was performed.
3. Chart-layout contract (`chartLayoutConfig`, `curvePropertyRegistry` surface consumed by
   `catalog/scenarios.ts`) → a leaf both `catalog` and `charts` depend on. Resolves
   `catalog ↔ charts`.
4. `stores/phase2PresetContract` → a leaf outside `stores`. Resolves `catalog ↔ stores`.

**This task is on C13's critical path, not a detour from it.** C13's remaining work is chart curve
sourcing: `buildChartData` reads `DerivedRunSeries`, which is black-oil, which is why
`comp_co2_1d` sits in `WITHHELD_SCENARIOS`. Extraction 3 is the same coupling. Sequence C13's
curve sourcing immediately after it rather than before.

**Validation:** `pnpm run validate:product` after each of the four; `check:cycles` green;
mutual-pair count 5 → 0 across the task.
**Exit:** `measure-module-coupling.mjs` reports zero mutual pairs. Update #29 with extraction 3's
effect on C13.

### S2 — Declare the packages

Add a `packages:` key to the existing `pnpm-workspace.yaml` (present today, carrying only
`minimumReleaseAgeExclude`) and give `charts`, `analytical` and the new leaves their own
`package.json`. No file moves beyond what S1 already did; no publishing.

**Validation:** `pnpm install` resolves; `pnpm run validate:product` unchanged; the production
bundle byte size does not grow.
**Exit:** each package builds standalone and its dependents resolve through the workspace.

### S3 — Prove a second page

Build one additional entry point that composes the packages — a single scenario or case study,
deliberately minimal — without editing the existing page.

**Validation:** both pages build; `validate:product` green; no change to the existing page's
rendered output.
**Exit:** FRONTEND-MODULAR-READY. The claim "different frontends are possible" is demonstrated,
not asserted.

### S4 — Feature-gate the WASM shim

Put `wasm-bindgen`/`js-sys`/`serde-wasm-bindgen` and the two `frontend.rs` shims behind a
default-on feature. Resolve the `getrandom` `wasm_js` pin per target. No physics edits.

**Validation:** `scripts/validate-solver-coverage.sh all`, `validate-compositional.sh thermo`,
`benchmark_buckley`, and a `--no-default-features` native build. The 14 scenario `.test.ts` files
that load the bindings directly (§2b) must still pass — they are the regression net for this task,
so `pnpm run validate:product` is required, not optional.
**Exit:** the crate builds with and without the WASM shim, identical native test results.

### S5 — A non-WASM consumer

Add a PyO3 shim beside `frontend.rs`, reusing `compositional/api.rs`'s serde payload pattern.
Additive: no existing file changes meaning.

**Validation:** a native run reproduces a committed fixture already used by the Rust suite, to the
same tolerance; `uv` for the Python side per repo convention.
**Exit:** ENGINE-MULTI-TARGET-READY. Grid sizes above §2d's browser ceiling become reachable —
record what was actually run, not what is now theoretically possible.

### S6 — Notebook views (scope decision required before starting)

Two routes, and they are not equally priced:

- **Share the data contract.** Export the `runQuantities`/`DerivedRunSeries` layer as the canonical
  schema; notebooks plot it with Python-native tools. Cheap, and it makes the contract — not the
  renderer — the thing under test.
- **Share the renderer.** Wrap the S2 chart package in `anywidget` for pixel-identical views.
  Viable only after S2, and materially more expensive for a benefit that is mostly cosmetic.

**Recommendation: the first.** Do not start S6 until someone has stated which, because the two
produce different artifacts and the second cannot be reached incrementally from the first.

## 5. Repository topology — one repository, several packages

Argued, not measured. Against a multi-repository split, specifically for this project:

1. CI builds the WASM bindings and then runs the frontend suite against them in one job. That is
   what catches cross-boundary breakage. Split the repositories and this check has no home.
2. `src/lib/ressim/pkg/` is generated and never committed (#30) precisely so it cannot go stale
   against `src/lib/ressim/src/`. A separate repository reintroduces that staleness across a
   publish boundary, which is the problem #30 removed.
3. Every engine change would become a two-repository dance with a version bump between, and the
   evidence discipline this project runs on — a change linked to its gate and its issue — would
   have to be reconstructed by hand across repositories.

A multi-package single repository delivers independent modules, multiple frontends and separate
pages with none of that cost. Revisit only if an external consumer needs the engine on a release
cadence the application cannot follow.

Incidental finding, not a task: `.git` is ~228 MB against 571 tracked files, dominated by
historical `public/cases/*.json` blobs (largest 19 MB). Irrelevant to the split; relevant if
anyone proposes a fresh repository for unrelated reasons.

## 6. What this plan does not establish

- It does not claim the four S1 extractions are trivial — only that each moves a named leaf and
  that the edge counts are small.
- It makes no claim about solver performance, convergence or physics. No task here may change a
  residual, a tolerance or a trajectory.
- It does not resolve C13, the compositional chart sourcing, or issue #22's black-oil orchestration
  extraction. S1 unblocks the first; the third is a different boundary entirely.
- The 3D visualization is the least portable area: Three.js is pinned and version-sensitive by
  repo constraint. Expect it to stay web-only and extract it last, or not at all.

## 7. Replay commands

```bash
# §2e — cross-area coupling and the mutual-pair metric
node scripts/measure-module-coupling.mjs

# §2a — WASM coupling in the crate
grep -rlc wasm_bindgen src/lib/ressim/src/ --include=*.rs
find src/lib/ressim/src -name '*.rs' | wc -l

# §2b — production importers of the generated bindings
grep -rn "ressim/pkg" src/ --include=*.ts --include=*.svelte | grep -v '\.test\.'

# gates used throughout
pnpm run check:cycles
pnpm run validate:product
bash scripts/validate-solver-coverage.sh all
bash scripts/validate-compositional.sh thermo
```

## 8. Issue mapping

No issue covers this work at the time of writing. Before S1, open one issue per milestone
(FRONTEND-MODULAR-READY, ENGINE-MULTI-TARGET-READY) and link S1's extraction 3 to #29, since C13
consumes it. Per repo working style, `TODO.md` gets at most a dashboard line — not a second
checkbox tracker.

## 9. Completion records

### S0 — baseline recorded (COMPLETE)

Measured on `adac71b`, clean tree. The plan's §2 numbers were first taken at `93c8c5c`; `adac71b`
is the dependency update that landed on top of it (including `three` 0.185.1 → 0.186.0), and the
metric is **unchanged across it**, which is itself the first useful datum: dependency movement does
not move module coupling.

| Quantity | Value | Replay |
|---|---|---|
| Mutual value-level pairs | **5** | `node scripts/measure-module-coupling.mjs` |
| Value edges between areas | 83 across 39 ordered pairs | same |
| Type-only edges between areas | 78 across 31 ordered pairs | same |
| Shipped `simulator_bg.wasm` | **1 399 032 bytes** | `RESSIM_FORCE_WASM_BUILD=1 bash scripts/build-wasm.sh` |
| Gate state at baseline | `pnpm run validate:product` exit 0, 926 passed / 15 skipped | `pnpm run validate:product` |

The five mutual pairs and the symbol responsible for each are tabulated in §2e. The WASM size is
recorded because S4 changes how the artifact is built and must be shown not to grow it.

`pnpm-workspace.yaml` exists at this revision but still declares only `minimumReleaseAgeExclude`,
with no `packages:` key — S2's precondition is unchanged by `adac71b`.

**Exit satisfied:** every §2 number has a committed revision and an exact replay command.

### S1 — leaf extractions (PARTIAL: 4 of 5 pairs resolved, `catalog ↔ charts` open)

| Step | Commit | Mutual pairs | Resolved |
|---|---|---|---|
| S0 baseline | `5effac2` | 5 | — |
| S1.1 `ToggleGroup` → `primitives/` | `431b323` | **5 → 3** | `charts ↔ ui`, `ui ↔ visualization` |
| S1.2 series/volume helpers → `quantities/` | `465f971` | **3 → 2** | `charts ↔ lib-root` |
| S1.3 (prep) dead layout lookup removed | `52d5f99` | 2 | — (one of three symbols) |
| S1.4 preset contract → `presets/` | `0d67d68` | **2 → 1** | `catalog ↔ stores` |

Every step: `pnpm run validate:product` exit 0, 926 passed / 15 skipped, "No runtime import cycles
across 193 files" — identical to the S0 baseline at each commit. No behaviour change.

Value edges rose 83 → 89 across the sequence while mutual pairs fell 5 → 1. That is the intended
shape: same-area imports became edges *into leaves*, which cost nothing at packaging time, and the
edges that blocked packaging went away. Edge count is not the metric; mutual pairs are.

**S1.4 was executed before the rest of S1.3.** The pairs are independent, and S1.3 turned out to be
materially larger than the other three — see below.

#### Re-plan: what `catalog ↔ charts` still needs

`charts → catalog` is now exactly two symbols, both in `charts/scenarioChartModel.ts`:
`resolveCapabilities` and `resolveScenarioReferenceSeries`. The other direction
(`catalog → charts`, three edges) is the **correct** direction and stays, for the reason recorded
in S1.2.

`buildScenarioComparisonFamily` must stop fetching those two and receive them. Its callers are
`stores/navigationStore` (which already imports the catalog, so it is free) and
`charts/ScenarioChart.svelte` (which must not). `ScenarioChart` is rendered in exactly one place,
`App.svelte`, so the values have to be threaded: store → `App.svelte` → `ScenarioChart` →
`buildScenarioComparisonFamily`.

**One shortcut is ruled out, and this is why the task is not a five-minute move.** It looks as
though `App.svelte` could simply pass the store's existing `activeScenarioAsFamily` and delete the
component's own call. It cannot: that derivation returns `null` when `isCustomMode` is set, while
`ScenarioChart`'s own build returns a family whenever `scenario` is non-null. Substituting one for
the other changes what renders in custom mode, which S1 forbids. The two resolved values must be
injected individually so the component's null semantics are untouched.

**Cost:** two new props on a public component, a changed helper signature, and an edit to the
application's render path — against no browser-level check in this environment (UI verification is
Playwright e2e, per repo practice). That is why it is its own pass rather than the tail of a
batch, and why it should be validated by more than the unit suite.
