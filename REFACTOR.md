# Frontend Simplification — Working Document

> Use this file to track progress, note issues, and pick up work after session breaks.

---

## Goal

Replace the overcomplicated "case library with editability policies" paradigm with a simple **pick scenario → optionally pick sensitivity → run** flow.

**Before:** 4 navigation layers (family → group → case → sensitivity axis → variant checkboxes), provenance tracking, override counts, editability policies, 733-line phase2PresetContract.ts.

**After:** 7 scenario buttons, optional sensitivity chip row, one run button.

---

## Step Progress

| Step | Status | Notes |
|------|--------|-------|
| 1 — `scenarios.ts` | ✅ Done | `src/lib/catalog/scenarios.ts` — 0 TS errors |
| 2 — `ScenarioPicker.svelte` | ✅ Done | Replaces ModePanel.svelte (~150 lines vs 451) |
| 3 — Store wiring | ✅ Done | New state + actions in store public API; run button routes sweep vs single |
| 4 — Store simplification | 🔲 Todo | Remove ScenarioNavigationState + phase2PresetContract |
| 5 — Wire App.svelte | ✅ Done | ScenarioPicker wired, activeReferenceFamily uses activeScenarioAsFamily |
| 6 — Simplify RunControls | ✅ Done | Removed "Advance 1 Step" + "Render Every N" |
| 7 — Delete old files | 🔲 Todo | After all new wiring confirmed working |

---

## Architecture: New Model

```
src/lib/catalog/scenarios.ts       ← single source of truth for all scenarios
src/lib/ui/modes/ScenarioPicker.svelte  ← replaces ModePanel.svelte
src/lib/ui/cards/RunControls.svelte     ← simplified
src/App.svelte                     ← simplified, fewer derived vars
```

### Scenario structure

```typescript
type Scenario = {
    key: string;
    label: string;
    description: string;
    scenarioClass: 'waterflood' | 'depletion';
    params: Record<string, unknown>;   // complete, self-contained param set
    chartPreset: string;               // key into CHART_PRESETS
    sensitivity?: {
        key: string;
        label: string;
        description: string;
        variants: SensitivityVariant[];
    };
};

type SensitivityVariant = {
    key: string;
    label: string;
    description: string;
    paramPatch: Record<string, unknown>;
    affectsAnalytical: boolean;  // true = mu_o/viscosity change; false = grid refinement
};
```

### Store state (after Step 4)

```typescript
type ScenarioState =
  | { kind: 'preset'; scenarioKey: string; activeVariantKeys: string[] }
  | { kind: 'custom'; seededFrom: string | null };
```

### Run status display

- Single run: `Running — 14/50 steps`
- Sensitivity sweep: `Case 1/3 — 14/50 steps`

---

## Scenarios (7 predefined)

| Key | Label | Class | Chart Preset | Sensitivity |
|-----|-------|-------|-------------|-------------|
| `wf_bl_case_a` | BL Case A | waterflood | waterflood | Grid refinement (24/48/96 cells) — analytical unchanged |
| `wf_bl_case_b` | BL Case B | waterflood | waterflood | Grid refinement (24/48/96 cells) — analytical unchanged |
| `wf_mobility_study` | Mobility Study | waterflood | waterflood | Oil viscosity (0.5/1.0/5.0 cp) — both sim & analytical change |
| `dep_dietz_center` | Dietz Center | depletion | depletion | None |
| `dep_dietz_corner` | Dietz Corner | depletion | depletion | None |
| `dep_fetkovich` | Fetkovich Decline | depletion | fetkovich | None |
| (custom mode) | Custom | any | (from last preset) | None (deferred) |

---

## Files to Delete (Step 7)

- `src/lib/ui/modes/ModePanel.svelte`
- `src/lib/ui/cards/ReferenceExecutionCard.svelte`
- `src/lib/stores/phase2PresetContract.ts` (keep `evaluateAnalyticalStatus` — move to warningPolicy.ts or similar)
- `src/lib/catalog/presetCases.ts`
- `src/lib/catalog/benchmarkCases.ts`
- `src/lib/catalog/caseCatalog.ts`
- `src/lib/catalog/caseLibrary.ts`
- `src/lib/benchmarkDisclosure.ts`
- `src/lib/benchmarkRunModel.ts` (review — likely replaced by simpler sweep logic)

---

## Issues & Notes Found During Work

### Step 1 (scenarios.ts)

- Benchmark JSON files (`dietz_sq_center.json`, `dietz_sq_corner.json`, `fetkovich_exp.json`) only have ~10 sparse params.
  **Resolution:** Built full self-contained param sets in `scenarios.ts` using the benchmark JSON values for case-specific params (grid, BHP, permeability, producer location) and the preset case values for shared defaults (fluid props, rel-perm, compressibility).

- `wf_mobility_study` base is identical to `wf_bl_case_a` with `mu_unit` variant having empty `paramPatch: {}`.
  This is intentional — all 3 variants run as separate sim+analytical pairs for overlay comparison.

- `custom` mode is a UI/store concept, not a scenario. Not included in `SCENARIOS` array.

- Future scenarios to add (TODO comment in scenarios.ts):
  - BL Case A with capillary pressure sensitivity
  - BL Case A with timestep refinement sensitivity
  - Layered permeability heterogeneity
  - Depletion with partial perforations

---

## Deferred Items

- **Custom sensitivity sweeps** — custom mode is single-run only for now
- **Custom analytical** — custom uses whatever `analyticalSolutionMode` is set to in params
- **Run button placement** — discuss when implementing sensitivity UI (Step 3)
