# Post-Wave-4 Review — Waves 0–4 (2026-07-19)

Status: **open findings, to fix**. Ranked findings from the full-stack self-review after Wave 4
(E5 history/forecast divider, E7 pre-run artifact class, E1 per-cell permeability).
Tracker checkboxes live in `TODO.md` ("Post-Wave-4 review" section); this file is the
self-contained record with mechanisms and fix directions.

Two claims from earlier sessions are **superseded** by finding 1: Wave 1's "OPM overlays render
through the real production code path" and the 2026-07-17 review's "dead-well artifact data now
renders in the UI". Both were made from model-level tests without a browser; neither was true at
the render level.

---

## 1. BLOCKER (cross-wave): OPM artifact curves have never rendered in any chart panel

**Symptom.** The E7 demonstrator `wf_bl1d_opm` renders an empty chart. The `wf_bl1d` and
`spe1_gas_injection` OPM overlays have never been visible either.

**Mechanism.** `appendPublishedReferenceSeries` (`src/lib/charts/buildChartData.ts`) correctly adds
artifact curves to the comparison *model*, but `ReferenceComparisonChart.svelte` resolves each
panel through `resolveChartPanelDefinition` → `selectPanelEntries`
(`src/lib/charts/chartPanelSelection.ts:49`), which hard-filters entries to
`override.curveKeys ?? fallback.curveKeys`. **No `opm-*` curve key appears in any layout in
`src/lib/catalog/chartLayouts.ts`, nor in the component's panel fallbacks.** SPE1's digitized
`published-*` keys render only because that layout's author explicitly whitelisted them.

**Empirical probe** (in-repo replication of the component's exact resolution steps, 2026-07-19):

| Scenario / panel | In model | Rendered after filter |
|---|---|---|
| `wf_bl1d_opm` rates | `opm-oil-rate`, `opm-water-rate`, `opm-injection-rate` | — (empty) |
| `wf_bl1d_opm` cumulative | `opm-cum-oil`, `opm-cum-water` | — (empty) |
| `wf_bl1d_opm` diagnostics | `opm-avg-pressure` | — (empty) |
| `wf_bl1d` rates | opm overlay curves | — (empty) |
| `spe1_gas_injection` gor | `published-gor`, `opm-gor` | `published-gor` only |
| `spe1_gas_injection` oil_rate | `published-oil-rate`, `opm-oil-rate` | `published-oil-rate` only |

Because `resolvedPanels` drops panels with zero curves, `wf_bl1d_opm` shows no panels at all.
E7's `primary` (solid-styling) flag is moot until curves survive the filter.

**Fix direction (preferred).** Make the resolver always retain curves whose source is
published/artifact (`sourceType === 'opm-flow-precomputed'` or curveKey convention) — they are
*additive overlays*, not alternates the whitelist was designed to choose between. Alternative:
whitelist `opm-*` keys per layout (spe1, waterflood), but that re-creates the silent-drop trap for
every future artifact. **Either way, add a post-filter render test**: model →
`resolveChartPanelDefinition` with the scenario's real layout → assert non-empty — the entire gap
class existed because every test stopped at model content.

## 2. MAJOR (E5): divider never shows on dep_nct by default

The `fetkovich` layout opens with `xAxisMode: 'logTime'`
(`src/lib/catalog/chartLayouts.ts:165`), and `resolveHistoryDivider`
(`src/lib/charts/historyDivider.ts`) only matches `axis: 'time'` against `'time'`. The user must
manually switch the x-axis to Time to see the divider — the demonstrator doesn't demonstrate
unprompted.

**Fix subtlety.** On `logTime` the chart plots `log10(time)` values on a linear scale (see
`buildComparisonXAxisValues` in `ReferenceComparisonChart.svelte`), so the divider must draw at
`Math.log10(boundary)`, not at `boundary`. Extend `resolveHistoryDivider` to treat `logTime` as
time-family with a transformed boundary (guard `boundary > 0`) — do not merely widen the axis
match.

## 3. MINOR (docs): scenario count stale, `wf_bl1d_opm` missing from inventory

README says "13 canonical scenarios"; `docs/DOCUMENTATION_INDEX.md` says the same. Actual count is
14 after `wf_bl1d_opm`, which also has no README inventory row. Decide the demonstrator's status
first (permanent entry vs dev-only until SPE11 becomes the first real Tier-6 exhibit), then make
the docs describe the intended state.

## 4. MINOR (E5 scope): divider exists only on the comparison-chart path

`historyWindow` is threaded `ScenarioChart` → `ReferenceComparisonChart` → `ChartSubPanel`. The
live single-run `RateChart`/`UniversalChart` path does not carry it. Acceptable by design (every
scenario with a `historyWindow` renders via the comparison chart) — recorded so a future
custom-mode or live-panel use doesn't assume it exists there.

## 5. MINOR (E7 cosmetic, unverifiable headless): one-column results grid

With the 3D card hidden for pre-run scenarios, the results grid (`xl:grid-cols-2` in
`App.svelte`) renders a single child — chart in the left half, empty right half on xl screens.
Consider spanning the chart full-width when `isPrerunScenario`. Check during the `pnpm run dev`
spot-check.

## 6. MINOR (E1 scope): `permMode: 'field'` wired only through the scenario-sweep path

Verified: `runScenarioSet` → `buildBenchmarkCreatePayload` spreads params, so `fieldPermX/Y/Z`
pass through to the worker. The single-run/custom path is not wired: `parameterStore.fieldPermX/Y/Z`
default `[]` with no UI, and `applyResolvedParams` doesn't map the field arrays from scenario
params into the store — a live single-run init of a field-perm scenario would silently fall back
to uniform perms. Fine while no scenario uses `'field'`; close together with the first consuming
scenario (Tavassoli / SPE10 layer / Egg).

---

## Verified clean (no action)

- Wave 0 guardrails unchanged and green (CI impes gate, PVT constant, doc banners).
- Wave 1 parser + regenerated artifacts: physically sane post-Fix-1; 14/14 pytest.
- Wave 2 scenarios: corrected metrics (displacement efficiency for `wf_tornado`, real RF values
  for `dep_nct`, GOR panel for `dep_pvt`) all hold.
- Wave 3 archival: 0 skips in `referenceComparisonModel.test.ts`; 18 tracked skips confined to
  4 other files.
- E1 Rust setter: strictly additive (no solver/timestep/assembly change), `impes` gate exit 0,
  committed `pkg/` diff is exactly the 4 intended files.
- E7 worker gating: `initSimulator`/`runSteps`/`runScenarioSet` all guarded via
  `nav.isPrerunScenario`; no auto-run path bypasses it.
- Full gates: typecheck, lint, vitest (654 passed / 18 tracked skips), `pnpm run build`.

## Recommended fix order

1. Finding 1 (blocker) — also decides whether `wf_bl1d_opm` earns its catalog slot; land with the
   post-filter render regression test.
2. Finding 2 (E5 logTime boundary transform).
3. Findings 3–5 in one docs/UI sweep, during the overdue `pnpm run dev` visual spot-check.
4. Finding 6 rides with the first field-perm scenario.
