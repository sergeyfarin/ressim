# Chart Architecture Review — 2026-08-02

Audit of how far the chart layer actually is from "a scenario declares what it plots; the chart
layer knows nothing about reservoirs." Requested after the material-balance work kept forcing edits
to shared chart files for what should have been scenario-local decisions.

Scope: `src/lib/charts/` (12,379 lines incl. tests), `src/lib/catalog/chartLayouts.ts`,
`src/lib/catalog/chartPanels/`, and the run-model surface they read
(`benchmarkRunModel.ts`, `stores/parameterStore.svelte.ts`). Read-only: no behaviour was changed
while auditing.

---

## 1. The verdict in one paragraph

The chart layer is **presentation-agnostic but not domain-agnostic**. The rendering primitives
(`ChartSubPanel.svelte`, `UniversalChart.svelte`, `chart-helpers.ts`) are genuinely generic — one
domain word between them. Everything above those primitives is a **closed enumeration of reservoir
engineering vocabulary**: 23 panel ids, 8 x-axis modes, 14 scale presets, 22 derived-series fields
and ~45 curve keys, all declared in shared modules. A scenario cannot introduce a quantity; it can
only *select from* and *retitle* what shared code already decided to compute. What the catalog calls
"declarative" is a filter over a fixed menu, not a declaration.

The existing guard, `scenarioAgnosticArchitecture.test.ts`, enforces exactly one rule — no branching
on a scenario **key** — and that rule holds. It says nothing about branching on analytical method,
panel id, or curve key, which is where the coupling actually lives.

---

## 2. What "agnostic" was taken to mean, and what it left out

`docs/SCENARIO_CATALOG_ARCHITECTURE.md` states the intent: a scenario owns "chart-layout preset,
scenario-local panel/curve/formatting overrides, and live panel definitions", and consumers "must
not contain `if (scenario.key === ...)`". Both are true today. The gap is that the *vocabulary* a
scenario declares in is owned by the chart layer:

| Contract | Where | Members | A scenario can… |
|---|---|---|---|
| `RateChartPrimaryPanelId` | `rateChartLayoutConfig.ts:11` | 18 | pick, reorder, retitle |
| `RateChartSweepPanelId` | same, `:13` | 5 | pick |
| `RateChartXAxisMode` | same, `:1` | 8 | pick |
| `RateChartScalePreset` | same, `:51` | 14 | pick |
| `DerivedRunSeries` | `axisAdapters.ts:22` | 22 fields | read |
| `LiveDerivedSeries` | `universalChartTypes.ts` | 23 fields | read |
| curve keys | minted in `buildChartData.ts` | ~45 | filter by key |
| `AnalyticalMethod` | `catalog/scenarios.ts:50` | 8 | pick one |

Adding anything outside those lists is a shared-code change. That is the whole finding; the rest of
this document is evidence and consequences.

---

## 3. Evidence

### 3.1 The cost of one new curve family, measured

`9cf2daf` added `dep_gas_pz` and its p/z curve. Beyond the scenario file and its test, it had to
touch **nine shared frontend modules**:

```
src/lib/catalog/analyticalAdapters.ts       +45
src/lib/catalog/chartLayouts.ts             +49
src/lib/catalog/scenarios.ts                +28
src/lib/charts/analyticalMethodRegistry.ts  +78
src/lib/charts/analyticalParamAdapters.ts  +105
src/lib/charts/buildChartData.ts            +22
src/lib/charts/curvePropertyRegistry.ts      +1
src/lib/charts/panelDefs.ts                  +6
src/lib/charts/rateChartLayoutConfig.ts      +3
```

One scenario wanting one plot required edits in nine places that every other scenario also depends
on. That is the practical definition of not-agnostic, and it is why the roadmap prices a new
analytical method as "its own commit".

### 3.2 `buildChartData.ts` is a 1,259-line switchboard

It references specific panels by name **41 times** (`panels.rates`, `panels.gor`, `panels.pz`,
`panels.mbe_ooip`, `panels.pss_drawdown`, …) and mints ~45 domain curve keys inline
(`'water-cut-sim'`, `'gor-sim'`, `'p-over-z-reference'`, `'drive-gas-cap'`, …). The flow is
**emit-everything-then-filter**: the builder computes every curve it knows how to compute, and the
scenario's layout decides which survive (`ReferenceComparisonChart.svelte:404`,
`.filter((panel) => panel.visible && panel.curves.length > 0)`).

The inversion matters beyond tidiness: a scenario cannot add a curve, only remove one, and the
builder must keep growing for every new quantity any scenario ever wants.

### 3.3 Semantics are recovered from string prefixes

`curvePropertyRegistry.ts` classifies curves by parsing their keys:

```ts
if (curveKey.startsWith('water-cut-')) return 'water-cut';
if (curveKey.startsWith('p-over-z-')) return 'p-over-z';
if (curveKey.startsWith('drive-'))    return 'drive-index';
```

The system needs to know what a curve *is* (to stop a panel mixing two properties), and recovers it
from the naming convention because curves carry no metadata of their own. Rename a key and the
classification silently returns `null`. This registry is the shape of the right answer — a curve
descriptor — implemented as string matching instead of as data attached to the curve.

### 3.4 One quantity, four implementations — and they have already drifted

Cumulative production is integrated from `rate × dt` in four places:

- `benchmarkRunModel.ts` (the run's own recovery/PVI)
- `charts/analyticalParamAdapters.ts` (`buildDerivedRunSeries`, the MBE)
- `charts/buildLiveDerivedSeries.ts` (live path)
- `charts/ReferenceComparisonChart.svelte:436` — **inside a Svelte component**

The same file also carries a **private `getPoreVolume`** (`:411`) that omits `cellDzPerLayer`, unlike
the shared one. Today only `spe1_gas_injection` sets per-layer thicknesses and no layout offers the
`pvp` axis, so the divergence is latent rather than live — but it is a copy that has already drifted
from its original, in a component, unnoticed.

This is the same failure mode as the OOIP defect fixed in `3aa3637`: a definition duplicated across
layers until the copies disagree. That one was three copies and cost 10–44 % errors in every
black-oil recovery curve.

### 3.5 Naming asserts a domain that no longer exists

The generic panel system is named after one scenario family: `RateChart.svelte`,
`rateChartLayoutConfig.ts`, `RateChartPanelId`, `RateChartScalePreset`, `RateChartXAxisMode`,
`DEFAULT_RATE_CHART_PANEL_ORDER`, `RateChartCurveOverride`. Half the panels it enumerates are not
rates (`pz`, `mbe_ooip`, `pss_shape_factor`, `drive_indices`, `sweep_*`). The names are load-bearing
in ~30 files, so this is not cosmetic churn — but it is the most visible symptom of a layer that was
generalised without being renamed.

### 3.6 An entire path appears to be dead

`RateChart.svelte` is never instantiated; `buildLiveDerivedSeries` has no caller outside it; and
`Scenario.liveChartPanels` — with every array in `catalog/chartPanels/*` it points at
(`gasLivePanels.ts`, `waterfloodLivePanels.ts`, `sweepLivePanels.ts`, `depletionLivePanels.ts`) — is
declared, typed, maintained, and **read by nothing**. The product renders `ScenarioChart` →
`ReferenceComparisonChart` → `buildChartData`.

`ratechart-usage.test.ts` (129 lines) actively guards the architecture of this dead path, which is
why it has stayed plausible-looking. Either a live mode regressed out of the app, or roughly 900
lines of scenario-facing chart declarations are maintained for nothing — and notably, the
`UniversalPanelDef` / `CurveConfig` shape used there is *closer to the target architecture* than the
shipped path is.

### 3.7 Method-shaped branching replaced key-shaped branching

`referenceChartConfig.ts:18` (`if (family.analyticalMethod === 'buckley-leverett')`) and
`buildChartData.ts:539` (`analyticalMethod === 'depletion' ? computeDepletionTau(...) : null`)
decide presentation from the analytical method. `analyticalMethodRegistry.ts` was built to end
exactly this ("a `(family.analyticalMethod === …)` ladder duplicated across four contexts") and
mostly did — these are the remainder. The material-balance work removed a third instance
(`analyticalMethod === 'depletion'` gating the MBE panels), which is what surfaced this review.

---

## 4. What is already right

Worth stating, because the refactor should preserve it rather than restart from zero:

- **`ChartSubPanel.svelte` and `UniversalChart.svelte` are genuinely generic.** One domain word
  across 1,128 lines. The rendering layer is not the problem.
- **`analyticalMethodRegistry.ts` is the correct pattern**, already proven in this codebase: a
  descriptor per method declaring what it produces and how it displays, with consumers asking the
  registry instead of branching. Panels and curves need the same treatment.
- **`scalePresetRegistry.ts`** separates scale policy from panels cleanly; the presets are
  domain-named but the mechanism is sound.
- **The single-property-per-panel rule** (`validateSinglePropertyPanel`) is a real design invariant
  worth keeping — it just needs a better source of truth than key prefixes.
- **`chartLayoutPatch`** already lets 8 scenarios override panel titles, order and curve sets
  without touching shared code. The extension point exists; its vocabulary is too small.

---

## 5. Target architecture

One idea: **a curve descriptor is data, owned by whoever knows the physics; the chart layer only
groups, scales and draws descriptors.**

```ts
type CurveDescriptor = {
    key: string;                 // identity, not semantics
    property: string;            // what it is — replaces prefix parsing
    label: string;
    unit: string;
    role: 'simulation' | 'analytical' | 'reference';
    axis: { scale: ScalePolicy; side?: 'left' | 'right' };
    source: (run: RunSeriesAccessor) => Array<number | null>;
};

type PanelDescriptor = {
    id: string;                  // free-form, not a union member
    title: string;
    curves: CurveDescriptor[];   // all one `property`, enforced from the descriptor
    scale: ScalePolicy;
};
```

With that, the four closed unions collapse:

- **Panel ids** become strings. `PANEL_DEFS`, `DEFAULT_RATE_CHART_PANEL_ORDER` and the 18-member
  union disappear; a scenario (or a method descriptor) supplies panels.
- **Curve keys** stop carrying semantics. `curvePropertyRegistry`'s prefix ladder is replaced by
  `descriptor.property`, and the single-property rule becomes a check over data.
- **`DerivedRunSeries`'s 22 fixed fields** become a `RunSeriesAccessor` — named quantities resolved
  on demand, computed **once** in one module, which is also where the four cumulative-integration
  copies collapse to one.
- **x-axis modes** become descriptors too (a label, a series accessor, a reference-mapping rule),
  which is what `mapReferenceTimesToXAxis` already needs and hard-codes per mode.

`buildChartData` then stops being a switchboard: it iterates descriptors, evaluates each against the
run, and hands panels to `ChartSubPanel`. Scenario-declared panels and method-declared panels merge
through one path instead of two.

---

## 6. Proposed sequence

Each step is independently shippable and leaves the product green. Sizes are rough.

**Status: steps 0-3 landed 2026-08-02**, plus the characterisation net that step 4 depends on.
Step 4 (the builder inversion) and steps 5-6 remain.

| # | Step | Why first / why here | Size |
|---|---|---|---|
| 0 | ✅ **Decide the fate of the live path** (§3.6) — delete, or restore and make it the reference implementation | Everything else doubles in cost while two chart stacks exist. This is a product decision, not an engineering one | S (decision) |
| 1 | ✅ **One run-series module.** Collapse the four cumulative/pore-volume implementations into the `reservoirVolumes.ts` + accessor pair; delete the component-local copies | Pure defect removal, no API change, and it is where the next drift will otherwise happen | M |
| 2 | ✅ **Introduce `CurveDescriptor` with a `property` field**; have `buildChartData` attach it to every curve it already emits; reimplement `curvePropertyRegistry` over descriptors, keeping the prefix ladder as a temporary fallback | Additive. Nothing moves yet, but semantics stop being parsed from strings | M |
| 3 | ✅ **Open the panel id type** to `string`, keeping the current ids as constants; move `PANEL_DEFS` entries next to the code that emits their curves | Removes the union that forces a shared edit per new panel | M |
| 4 | **Invert the builder**: panels declare their curves and pull from the accessor, instead of the builder emitting everything and the layout filtering | The actual architectural change; safe only after 1–3 | L |
| 5 | **Rename `RateChart*` → `SeriesChart*`** (or similar) across ~30 files | Cosmetic, mechanical, and best done last so it does not collide with real edits | M |
| 6 | **Extend the agnostic test** to forbid what actually leaks: no `panels.<id>` literals outside panel descriptors, no `analyticalMethod ===` outside the method registry, no domain vocabulary in `charts/*.svelte` | Locks in the result the same way the key rule locked in the last one | S |

**Order rationale.** Steps 1–3 are additive and independently valuable even if step 4 is never taken:
they remove a class of duplication defect, make curve semantics explicit, and stop the panel union
from forcing shared edits. Step 4 is the one with real regression risk and should be gated on the
scenario-level chart tests being green beforehand. Step 0 blocks nothing technically but doubles the
cost of 1–5 if left open.

---

## 7. Risks

- **`buildChartData.ts` has no direct unit test** — it is covered indirectly through
  `referenceComparisonModel.test.ts` (2,071 lines, 48 tests). Step 4 needs characterisation tests on
  the current output *before* the inversion, not after.
- **Curve keys are load-bearing in scenario files** (8 `chartLayoutPatch` blocks name them
  explicitly) and in tests that look curves up by label. Any rename is a catalog-wide edit.
- **The single-property rule is enforced against layouts at test time**; opening panel ids to strings
  must not lose that check.
- **Nothing here is physics.** No engine, solver or analytical behaviour is touched by any step, so
  the validation gate is `pnpm run validate` throughout — but scenario chart output should be
  snapshot-compared before and after step 4, since it is user-visible.

---

## 8. Salvage from the live path (recorded 2026-08-02, before deletion)

Step 0 was decided as **delete**, on condition that its good ideas are captured first. They are
better than what shipped, which is the uncomfortable part: the disconnected path is closer to the
target architecture than the live one. Recorded here so the deletion loses nothing but code.

### 8.1 Keep — a curve owns its data extraction

```ts
export type UniversalCurveDef = {
    key: string;
    label: string;
    curveType: CurveType;
    yAxisID: string;
    color: string;                                   // hex, or the sentinel 'neutral'
    getData?:   (ctx: LiveCurveContext) => Array<number | null>;
    getDataXY?: (ctx: LiveCurveContext) => Array<{ x: number; y: number | null }>;
};
```

A curve carries a closure over a context object instead of the builder knowing how to compute it.
Adding a curve is then a scenario-local edit — exactly the `source: (run) => values` the target
needs, and prior art inside this repo rather than an import from elsewhere. Adopt as-is.

### 8.2 Keep — the explicit no-fallback stance, which the live code states outright

> *"A panel declared by a scenario. Defines exactly which curves to show — no hidden fallbacks."*
> — `universalChartTypes.ts:161`

The correct principle was written down, in the path that got disconnected. The shipped builder does
the opposite: it emits every curve it knows and lets the layout subtract. Adopt the sentence as the
rule for the new panel descriptor, and keep it in the doc comment so the next reader sees the intent.

### 8.3 Keep — a per-curve x mapping escape hatch (`getDataXY`)

Some curves have their own x: a PVI-indexed analytical sweep curve, a published reference recorded
against days, a sweep series sampled on different times than the run. The live path lets *the curve*
map itself (`simSweepToXY`, `analyticalSweepToXY`, `mapPviToX` in `sweepLivePanels.ts`). The shipped
path instead centralises this in `mapReferenceTimesToXAxis` with a branch per axis mode, which is
why adding the `cumGas` axis for `dep_gas_pz` required teaching that function a new case. Per-curve
mapping generalises; adopt it alongside the plain `getData`.

### 8.4 Keep — resolve theme at build time via a sentinel

`color: 'neutral'` resolves to `ctx.neutralColor` when the curve is built, so a descriptor stays
theme-independent data. Small, and it removes the temptation to pass a theme into descriptors.

### 8.5 Keep — role-based styling

`curveType` (`'simulation' | 'analytical' | 'reference' | …`) drives dash, width and legend section
through `curveStylePolicy.ts` rather than each curve specifying its own. This half already survives
in the shipped path — `CurveType` is imported by `curveStylePolicy` and outlives the deletion.

### 8.6 Do **not** keep

- **`panelKey: RateChartPanelId`** — the panel descriptor still drew from the closed union. The whole
  point of step 3 is that this becomes a free string.
- **A panel with no title or scale** — `UniversalPanelDef` is `{ panelKey, curves }` and defers title
  and scale to `PANEL_DEFS`, so it needs a second lookup keyed on the union. The new descriptor
  should carry its own title and scale policy.
- **Domain fields on the context** — `LiveCurveContext` carries `ooipM3`, `sweep`, `sweepSimSeries`,
  `pviArr`. A context that names sweep and OOIP is not agnostic; those belong in the accessor as
  named quantities resolved on demand, not as fixed fields.
- **`yAxisID: string`** — a raw Chart.js concept in a descriptor. Express it as `side: 'left' | 'right'`
  and let the renderer map it.

### 8.7 What is deleted

`charts/RateChart.svelte`, `charts/UniversalChart.svelte`, `charts/buildUniversalChartData.ts`,
`charts/buildLiveDerivedSeries.ts`, `charts/ratechart-usage.test.ts`, the four
`catalog/chartPanels/*.ts` arrays, the `Scenario.liveChartPanels` field, and the live-only half of
`charts/universalChartTypes.ts`. `CurveType` moves to `curveStylePolicy.ts`, which is its only live
consumer. `ChartSubPanel.svelte` stays — it is used by `ReferenceComparisonChart` and two UI sections.

---

## 9. Delivery record — steps 0-3 (2026-08-02)

| Commit | Step | Effect |
|---|---|---|
| `e2d2721` | net | Characterisation of all 18 scenarios' emitted panels and curves, from a synthetic run. Step 4's safety net, and it already showed the problem: `wf_bl1d`, a waterflood, emits `gor` and `pz` panels that its layout then filters away. |
| `7653606` | 0 | Live path deleted — 1,952 lines. Its ideas recorded in §8 first. |
| `8cd69a9` | 1 | Cumulative production integrated once (`runSeries.ts`); the component's private, drifted `getPoreVolume` gone. The rectangle rule is now pinned by a test *and* explained, because it looks like a bug and is not. |
| `79786bd` | 2 | `CurveConfig.property` declared and stamped in `appendSeries` / `appendXYSeries`; classification reads an explicit table before any key parsing. |
| `04f5139` | 3 | Panel ids opened to `string`; `getPanelFallback` handles ids nobody declared. |

**Two defects the new guards found**, recorded in `TODO.md` rather than fixed in-flight:

- `dep_gas_pz`'s "Gas Rate" panel plots `oil-rate-sim` — ~0 for a dry-gas reservoir — beside OPM's
  real gas rates. `DerivedRunSeries` has **no gas-rate series at all**, so no gas scenario can plot
  its own production rate. That is a product gap, not a chart-layer one, and it is exactly the kind
  of thing the closed 22-field derived-series contract makes invisible.
- `spe1_gas_injection`'s cumulative-*oil* panel receives `opm-cum-gas`. The single-property rule was
  only ever checked against layout-declared keys, and reference curves are appended by the builder.

Both are arguments for step 4 rather than exceptions to it: a panel that declared its own curves
could not have acquired a foreign one.

---

## 10. Component-per-quantity, or quantity-as-data? (decided 2026-08-02)

Raised while fixing the gas-rate gap: since the chart layer's rules kept leaking
into components, should there be a component per plot — `GasRatePlot.svelte`,
`SaturationPlot.svelte` — that a scenario picks and feeds?

**Rejected, in favour of quantity-as-data.** Four reasons, in order of weight:

1. **The generic component already exists and is clean.** `ChartSubPanel.svelte`
   takes 13 props — title, curves, series, scales, theme, log toggle, gutters,
   x-range, history divider — and contains one domain word in 813 lines. It *is*
   "the component a scenario pulls". Nothing about a gas rate needs a different
   component; it needs different data handed to this one.
2. **The variation is data, not behaviour.** A gas-rate plot differs from a
   saturation plot in which series, what title, what unit, what scale, what
   colour. None of that is logic. Making it a component makes data differences
   cost a file.
3. **It would fork every presentation feature N ways.** Log toggling, legend
   grouping and per-case toggles, history/forecast divider, leading-outlier
   suppression, shared x-range across panels, theme response — each would need
   to be inherited or reimplemented per plot type, and would drift, exactly as
   the four cumulative-integration copies did.
4. **It puts vocabulary back into the layer that is free of it.** Groups 1-3 of
   this review's findings are about domain names in shared code. Naming
   components after reservoir quantities is the same mistake with a nicer face.

**What was missing instead**: a way to *name* a quantity. `DerivedRunSeries` was
a closed struct of 22 fields; gas rate was not one of them, so no scenario could
ask for it however it declared its panels. `runQuantities.ts` introduces
`{ id, label, unit, property, source(derived) }` and serves the rate family from
it. Adding a rate is now a registry row.

**Scenario definitions were reviewed** for the same question and left alone. They
are 100-576 lines, and the bulk is scenario-owned content that belongs there:
physics parameters, the prose the picker shows, sensitivity dimensions. Chart
declaration is a small tail (8 of 18 carry a `chartLayoutPatch`). The problem was
never that scenarios say too much — it is that they can only choose from what the
builder already emits. Splitting the files would move the symptom; the registry
plus step 4 removes it.
