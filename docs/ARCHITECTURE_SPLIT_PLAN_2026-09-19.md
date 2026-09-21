# Architecture split execution plan

Date: 2026-09-19. Measurement base: `93c8c5c`, clean tree. **This is a plan, not a record of
completed work.** Every number in §2 is reproduced by the commands in §7; nothing here is a
baseline until a task records its own.

## Purpose, authority and completion boundary

Separate the simulation engine from the browser application far enough that (a) the engine can be
built for targets other than browser WASM, (b) the frontend can be composed into more than one
page, and (c) neither change requires rewriting the other.

This document owns the split sequence. GitHub Issues owns task status.
`COMPOSITIONAL_FLUID_EXECUTION_PLAN_2026-09-14.md` owns C13 and the compositional backlog. The
two plans were initially believed to touch at S1; they do not (§9a), and neither gates the other.
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

> **Qualified 2026-09-21 by S4 (§10).** The counts below are correct, and the conclusion drawn
> from them — that the *physics modules* are native-first — holds. What they do **not** show is
> that `frontend.rs` is a shim. It is 1 327 lines and 69 functions, of which only **12** touch
> `JsValue`; the rest is the model's own constructor and setters wearing a `#[wasm_bindgen]`
> attribute. Counting attribute occurrences measured decoration, not coupling.

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

~~**This task is on C13's critical path, not a detour from it.**~~ **WITHDRAWN 2026-09-19 —
see §9a.** C13's blocker is a closed black-oil type *inside* `charts/`, not the `catalog ↔ charts`
coupling. Finishing S1 does not unblock C13, and C13 needs no S-task. The two tracks are
independent.

**Validation:** `pnpm run validate:product` after each of the four; `check:cycles` green;
mutual-pair count 5 → 0 across the task.
**Exit:** `measure-module-coupling.mjs` reports zero mutual pairs. (An earlier exit criterion
required recording extraction 3's effect on C13; there is none — §9a.)

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

No issue covers this work at the time of writing. Open one issue per milestone
(FRONTEND-MODULAR-READY, ENGINE-MULTI-TARGET-READY). An earlier revision also said to link
extraction 3 to #29; §9a withdraws that — C13 consumes nothing from S1. Per repo working style,
`TODO.md` gets at most a dashboard line, not a second checkbox tracker.

## 9. Risks, alternatives and open decisions

Added 2026-09-19 after executing S0–S1.4. Everything here came from doing the work, not from
planning it.

### 9a. Correction: S1 is **not** on C13's critical path

§4 S1 claimed that extraction 3 unblocks C13's compositional curve sourcing. **That is wrong, and
the claim is withdrawn.** Checked at `6131628`:

- The `catalog ↔ charts` mutual pair lives entirely in `charts/scenarioChartModel.ts` (→ catalog)
  and `catalog/scenarios.ts` + `catalog/analyticalAdapters.ts` (→ charts).
- C13's blocker is that `charts/simulationCurves.ts` and `buildChartData` are typed against
  `DerivedRunSeries`, a **closed struct of black-oil named fields** defined in
  `charts/axisAdapters.ts`. `buildChartData` imports only a *type* from the catalog.

The blocker is therefore **inside `charts/`**, and is a type-design problem, not a cross-area
coupling problem. Finishing S1 does not unblock C13, and C13 does not need any S-task. The two
tracks are independent and may be sequenced by preference rather than by dependency.

This also revises the answer given when the split was first proposed: "add the compositional
scenario, or split first?" is a genuine either/or, not a sequencing consequence.

### 9b. S1.3 has a second design, possibly better than the planned one

The plan's remaining S1.3 threads `resolveCapabilities` and `resolveScenarioReferenceSeries`
through `App.svelte` into `ScenarioChart`. There is an alternative worth weighing before starting:

**Relocate the scenario adapter instead of injecting into it.** `scenarioChartModel.ts` maps a
catalog `Scenario` onto a `BenchmarkFamily` — both non-chart concepts — and `ScenarioChart.svelte`
is a 58-line adapter that does nothing but call it and render `ReferenceComparisonChart`. Neither
is a reusable chart primitive. Moving *both* out of `charts/` would leave `charts/` holding only
genuinely reusable rendering, and the scenario-aware pair would sit together where their catalog
dependency is legitimate. Their imports *from* `charts/` are already type-only.

Trade-off: it changes which area owns two files (a larger diff, a clearer boundary) versus adding
two props to a public component and editing the app's render path (a smaller diff, a boundary that
stays slightly wrong). Both are behaviour-preserving if done carefully. **Undecided — see §9f.**

### 9c. S2's real cost is Svelte and Tailwind packaging, not the workspace file

`tailwind.config.ts` scans `content: ['./index.html', './src/**/*.{svelte,js,ts,jsx,tsx}']`.
As long as packages are declared **in place under `src/lib/`**, which is what S2 proposes, this is
fine. The moment any package moves outside `src/`, Tailwind stops seeing its classes and **emits no
error** — the components render unstyled. A daisyUI plugin in the same config means a consumer
needs the identical Tailwind setup, not merely the package.

Consequences for S2, which the task text does not currently carry:

- Keep packages under `src/lib/` until someone has deliberately solved the Tailwind content
  question. Do not move directories to a top-level `packages/` as a tidiness step.
- Svelte components shipped as a package are consumed as source with a `svelte` export condition,
  compiled by the consumer. A second frontend therefore inherits this Tailwind setup rather than
  being free of it. S3's "second page" should be treated as a test of exactly that, and its
  acceptance should include *visual* confirmation, not only that it builds.

### 9d. Two different ceilings are being conflated

§2d records the browser heap ceiling. There are actually two limits, and they are lifted by
different things:

| Ceiling | Value | Lifted by |
|---|---|---|
| Browser tab heap | a few hundred MB in practice; the `≤ 1200`-cell policy | **running the same WASM under Node — already works today** (§2c) |
| `wasm32` address space | 4 GiB, host-independent | only a native target (S5), or `wasm64`, which is immature |

So there is a cheaper intermediate step than S5 that the plan never named: **a Node-hosted batch
runner using the existing bundle**, which lifts the first ceiling with no new binding layer at all.
`scripts/fim-wasm-diagnostic.mjs` is already most of it. If the motivation for S5 is "cases larger
than the browser allows", that motivation is partly satisfied for free; if it is "field-scale",
only native suffices. Worth deciding which is actually wanted before building a PyO3 layer.

### 9e. S5 is probably a separate crate, not a third shim

S5 as written adds a PyO3 shim beside `frontend.rs` behind features. Stacking `wasm_bindgen` and
`pyo3` attribute macros on one struct in one crate invites feature-combination breakage and makes
`cargo test` matrices grow.

Alternative: a thin `ressim-py` crate depending on `simulator` as an **rlib**, which `crate-type`
already supports. The physics crate then needs no PyO3 at all, S4's feature gate stays a two-state
switch rather than a matrix, and the Python binding can version independently. This is likely the
better shape; recorded here rather than silently rewritten into S5.

### 9f. Reversibility, and what it costs to be wrong

| Task | Blast radius | Cost to revert |
|---|---|---|
| S1.1, S1.2, S1.4 (done) | file moves + import paths | trivial — `git revert`, no API changed |
| S1.3 remainder | a public component's props, the app render path | moderate; needs visual check, no browser gate here |
| S2 | build/packaging config | low while packages stay under `src/lib/`; **high** if files move out (§9c) |
| S3 | new entry point only | trivial — delete it |
| S4 | crate feature gating | moderate; the 14 WASM-driving scenario tests are the net |
| S5 | new crate or shim | low if a separate crate; moderate if in-crate features |

The pattern: everything up to S3 is cheap to undo, and the two genuinely expensive mistakes
available are moving packages out of `src/` (silent styling loss) and putting PyO3 in the physics
crate (feature-matrix debt).

### 9g. Adjacent risk, not part of this plan

`three` moved 0.185.1 → 0.186.0 in `adac71b`. Repo constraint calls Three.js pinned and
visualization "version-sensitive", and the only evidence the upgrade is safe is a unit suite that
does not render. `visualization/` is also the area this plan designates least portable. Not a
task here, but if a 3D rendering regression appears later, that bump is the first place to look.

## 10. Completion records

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

### S1 — leaf extractions (COMPLETE: 5 → 0 mutual pairs)

| Step | Commit | Mutual pairs | Resolved |
|---|---|---|---|
| S0 baseline | `5effac2` | 5 | — |
| S1.1 `ToggleGroup` → `primitives/` | `431b323` | **5 → 3** | `charts ↔ ui`, `ui ↔ visualization` |
| S1.2 series/volume helpers → `quantities/` | `465f971` | **3 → 2** | `charts ↔ lib-root` |
| S1.3 (prep) dead layout lookup removed | `52d5f99` | 2 | — (one of three symbols) |
| S1.4 preset contract → `presets/` | `0d67d68` | **2 → 1** | `catalog ↔ stores` |
| S1.3 scenario adapter → `scenario/` | `1507c81` | **1 → 0** | `catalog ↔ charts` |

Every step: `pnpm run validate:product` exit 0, 926 passed / 15 skipped, "No runtime import cycles
across 193 files" — identical to the S0 baseline at each commit. No behaviour change.

Value edges rose 83 → 90 across the sequence while mutual pairs fell 5 → 0. That is the intended
shape: same-area imports became edges *into leaves*, which cost nothing at packaging time, and the
edges that blocked packaging went away. Edge count is not the metric; mutual pairs are.

**S1.4 was executed before the rest of S1.3.** The pairs are independent, and S1.3 looked
materially larger at the time. Pausing to re-scope it is what surfaced the cheaper route — it
ended up the smallest change of the four. See below.
#### How `catalog ↔ charts` was actually resolved

The re-plan written at `6131628` proposed threading `resolveCapabilities` and
`resolveScenarioReferenceSeries` through `App.svelte` into `ScenarioChart`, and priced the task
above the other three because of it. **A cheaper and safer route was found and taken instead**
(§9b, decided with the maintainer): move `scenarioChartModel.ts` out of `charts/` into
`scenario/`, where the `BenchmarkFamily` it produces is already defined.

Two facts, neither visible until the file was read closely, made it clean:

- Its three exported chart-model types — `ChartCurveModel`, `ChartPanelModel`, `ChartModel` — were
  **imported by nothing**. Deleting them removed the file's only two imports from `charts/`, so
  after the move it imports nothing from `charts/` at all.
- `charts → scenario` was already entirely `import type`, so the new value edge from
  `ScenarioChart` into `scenario/` forms no pair.

No component signature changed, `App.svelte` was not touched, and `ScenarioChart` still builds its
own family — so the `isCustomMode` null-semantics hazard that ruled out passing the store's
`activeScenarioAsFamily` never arises. The threading cost the earlier re-plan priced was avoided.

Final state: value edges 90, type-only 79, **mutual pairs 0** — *"every area could be lifted into
its own package."*

**FRONTEND-MODULAR-READY is NOT declared.** That milestone also requires S2 (packages declared in
the workspace) and S3 (a second page built from them without editing the first). S1 is one of its
three preconditions, and the §9c Tailwind question is unanswered until S2 is attempted.

### S2 — packages declared (COMPLETE)

Five directories under `src/lib/` are now workspace packages — `@ressim/analytical`,
`@ressim/charts`, `@ressim/presets`, `@ressim/primitives`, `@ressim/quantities` — declared via a
`packages: ['src/lib/*']` glob added to the existing `pnpm-workspace.yaml`, each with a
`package.json`, each linked from the root. 48 files changed; every cross-package import is now
written `@ressim/<pkg>/<module>`. No file moved.

| Check | Result |
|---|---|
| `pnpm install` | exit 0, "Scope: all 6 workspace projects", five `link:` entries |
| Coupling metric | **90 value / 79 type-only / 0 mutual — identical to before packaging** |
| `pnpm run validate:product` | exit 0, 930 passed / 15 skipped (926 + 4 new gate tests) |
| `index-*.css` | **43 591 bytes, content hash `BV2-zFJz` — byte-identical to `4b30109`** |
| `index-*.js` | 547 052 bytes — identical size (hash differs; module ids changed) |

The metric being *unchanged* is the result worth reading twice. Declaring a package must not move
real coupling, and it did not — but that only became checkable after the gates were taught to
resolve package specifiers (below).

#### §9c answered: Tailwind is fine, while packages stay under `src/`

The predicted hazard was that Tailwind's `content: ['./src/**/*']` glob would stop seeing a
packaged component and silently emit no classes for it. It does not, and the evidence is stronger
than a size comparison: the built CSS has the **same content hash** before and after, and
`ToggleGroup`'s arbitrary utility `text-\[11px\]` is present in the output. Packages remain under
`src/lib/`, so the glob still covers them. §9c's warning stands for any future move *out* of
`src/`; it is not a live problem now.

**The plan's own S2 criterion was wrong and is corrected here.** It required that "the production
bundle byte size does not grow". For the Tailwind hazard, growth is not the failure mode —
**shrinkage is**: classes silently dropped make the CSS smaller, and a "did not grow" check passes
happily while the page renders unstyled. The right check is byte-identity of the stylesheet, or an
assertion that specific utilities survived. Both were done.

#### Two gaps found while executing, both fixed

1. **The import gates were blind to package specifiers** (`4b30109`). `check-import-cycles.mjs` is
   a CI gate and resolved only relative paths, so converting the first module dropped measured
   coupling from 90/79 to 88/78 with nothing decoupled — and, far worse, a value-level cycle
   spanning two packages would have passed the gate in silence. Both scripts now resolve
   `@ressim/<pkg>/<path>`, and `scripts/import-gates.test.ts` pins it: four tests, run against the
   real scripts as subprocesses, asserting the *bad* case is detected and that external packages
   are still ignored. Verified by mutation.
2. **Two architecture tests asserted import spelling rather than which module** (`bf80666`). One
   broke on the rename; the other — a *negative* assertion that App must not import the catalog
   directly — did not break, and that is the point: pinned to one spelling, it would have stopped
   guarding the moment `catalog` became a package, with nothing failing.

#### Limitation this task does **not** remove

A package name is not encapsulation. The `exports` maps are `./*` wildcards over source, so any
internal module is reachable, and nothing in the toolchain prevents a file from writing
`../charts/buildChartData` and tunnelling straight past the boundary. `src/lib/packageBoundaries.test.ts`
now enforces the entry rule — it discovers packages by walking for `package.json`, so packages
added later are covered without editing it — but that is a test, not a build-level guarantee.
Tightening `exports` to explicit entry points is a later, separate decision; doing it now would
mean choosing each package's public surface before S3 has shown what a second page actually needs.

**FRONTEND-MODULAR-READY still needs S3**: a second page built from these packages without editing
the first.

### S3 — a second page (COMPLETE) — **FRONTEND-MODULAR-READY declared**

`fractional-flow.html` + `src/pages/fractionalFlow/` render Buckley–Leverett fractional flow with
the Welge tangent, composed from three packages and **nothing else**:

| Package | Used for |
|---|---|
| `@ressim/analytical/fractionalFlow` | `fractionalFlow`, `computeWelgeMetrics` |
| `@ressim/charts/ChartSubPanel.svelte` | the plot |
| `@ressim/primitives/ToggleGroup.svelte` | the viscosity-ratio control |

No store, no scenario catalog, no worker, no WASM, no `App.svelte`. `index.html` and `App.svelte`
were not edited; the only shared file touched is `vite.config.ts`, which gains a two-entry
`rollupOptions.input` — build configuration, not the page.

**Verified in a real browser**, not merely built (`tests/deployed/second-page.spec.ts`): the
analytical package computes a shock front inside its physical bounds, the chart package draws a
sized canvas when handed data by something other than the application, the toggle recomputes
through to the chart, and there are no runtime errors or failed requests.

That spec also carries §9c's visual check, which a byte count cannot do: it asserts the computed
`font-family` of a `font-mono` element actually resolves to a monospace stack. A Tailwind
content-glob miss produces an unstyled page and **no error anywhere**, so it has to be asserted
against rendered style.

#### The existing page's chunking changed — checked, not assumed

Adding a second entry made Vite re-split shared code. This is worth recording because the numbers
look alarming in isolation:

| Asset | Before S3 | After S3 |
|---|---|---|
| `index-*.css` | 43 591 B | 1 010 B |
| `chart-helpers-*.css` (new, shared) | — | 43 050 B |
| Total CSS | ~44 081 B | 44 559 B (+478 B — the new page's own utilities) |

The bulk of the stylesheet moved into a chunk shared by both entries, and **both pages link it
directly from `<head>`**, so the application still gets its full CSS on first paint rather than
behind a lazy import. Confirmed by running the existing `public-site.spec.ts` against the
two-entry build: green. The plan's "no change to the existing page's rendered output" holds; its
*chunking* did change, which is expected for a multi-page build and is not the same claim.

#### What S3 tells S2's deferred decision

S2 left `exports` as `./*` wildcards because nothing had yet shown what a second page needs. It
now has. The entry points actually consumed are `analytical/fractionalFlow`,
`charts/ChartSubPanel.svelte` and `primitives/ToggleGroup.svelte` — three modules out of 36 across
the five packages. `ChartSubPanel`'s surface (`curves`, `seriesData`, `scaleConfigs`, `theme`)
needed no scenario, store or catalog, and it registers Chart.js itself, so the reusable core of
`charts` is genuinely reusable. Tightening `exports` to declared entry points is now a decision
with evidence behind it rather than a guess.

#### Gap logged, not closed

`pnpm run test:deployed` is **not in CI** (`.github/workflows/pr-tests.yml` ends at `pnpm run
build`). Both Playwright specs — the pre-existing one and the new one — therefore run only when
someone runs them. The new spec is the only check that would catch a Tailwind content-glob
regression, so it is the one most worth having in CI; that needs a preview server in the workflow
and is its own change.

---

**FRONTEND-MODULAR-READY is declared**, at this commit. All three preconditions hold: zero mutual
value-level pairs (S1), packages declared in the workspace (S2), and a second page built from them
without editing the first, verified in a browser (S3).

Remaining in this plan: S4 (feature-gate the WASM shim) and S5 (a non-WASM consumer), which
together are ENGINE-MULTI-TARGET-READY, and S6, which §9d/§S6 say should not start before someone
chooses between sharing the data contract and sharing the renderer.

### S4 — the WASM shim behind a feature (COMPLETE)

`wasm`, default-on, gating `wasm-bindgen`, `js-sys` and `serde-wasm-bindgen`. `getrandom`'s
`wasm_js` backend is now scoped to `cfg(target_arch = "wasm32")` rather than requested on every
target. No physics edited.

| Check | Result |
|---|---|
| Shipped `simulator_bg.wasm` | **1 399 032 bytes — byte-identical to the S0 baseline** |
| Generated `simulator.d.ts` | **identical** |
| `validate-solver-coverage.sh all` | 38 gates, exit 0 |
| `validate-compositional.sh thermo` | 28 gates, exit 0 |
| `validate:product` | exit 0, 932 passed / 15 skipped |
| `cargo build --no-default-features` | exit 0, **0 warnings** (default build also 0) |
| `cargo tree` binding crates | **10 with default features, 0 without** |
| Representative gates, both ways | `benchmark_buckley` 3/3, `comp_flash_` 13/13, `fim::tests::repair_lifecycle` 5/5, `impes::tests::transport::` 4/4 — identical |

#### The task's premise was wrong, and the feature is what proved it

S4 as written said to put "the two `frontend.rs` shims" behind the feature. Doing exactly that
produced a library that compiled without the feature and **could not be used**: every
`ReservoirSimulator::new` call failed to resolve, because the constructor lives in `frontend.rs`.
The crate built; nothing could construct it. Had the task stopped at its stated instruction, S4
would have been green and worthless, and S5 would have discovered it instead.

The cause is §2a's measurement. "92 of 98 `wasm_bindgen` occurrences are inside the two
`frontend.rs` shims" counted **attributes**, and an attribute is decoration. Of 69 functions in
`frontend.rs`, only **12** actually touch `JsValue`, `js_sys` or `serde_wasm_bindgen`. The other
57 — the constructor, every setter, the stepping API — are plain Rust that happened to live in a
file named `frontend`.

So the gate moved from the module to the attributes: `mod frontend` stays unconditional, its 62
`#[wasm_bindgen…]` attributes became `#[cfg_attr(feature = "wasm", …)]`, and the 12 binding-bound
functions plus the imports only they use are `#[cfg(feature = "wasm")]`. `compositional/frontend.rs`
**is** a genuine shim — `compositional/api.rs` is its native counterpart by design — so it stays
gated as a whole, which is the contrast that makes the black-oil case legible.

#### What this hands S5

57 of 69 API functions are now reachable natively, with `wasm-bindgen` absent from the dependency
graph. That is the surface a `ressim-py` crate (§9e) would bind. The 12 that are not reachable are
the JS-payload boundary — `get_grid_state`, `set_pvt_table`, `load_state` and similar — and each
would need a serde-native counterpart rather than a binding. That is a concrete, enumerated list
rather than an unknown, which is the useful output of this task.

#### Also fixed

`fim/assembly.rs` imported `CellPrimary` unconditionally while only a `#[cfg(test)]` helper names
it, so every non-test `cargo build` warned. Pre-existing on master, unrelated to S4, found because
the no-default-features build made warnings worth reading. Both configurations now build clean.

**ENGINE-MULTI-TARGET-READY is not declared:** it also requires S5, a non-WASM consumer that is
built and tested.
