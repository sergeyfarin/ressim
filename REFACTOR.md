# Frontend Architecture — Working Document

> Use this file to track active architecture work and design decisions. Delete when all phases are complete and docs are refreshed.

---

## Phase 1 — Case-Library Simplification

**Goal:** Replace the 4-layer "case library + editability policies" navigation with a simple **pick scenario → optionally pick sensitivity → run** flow.

**Before:** 4 navigation layers (family → group → case → sensitivity axis → variant checkboxes), provenance tracking, override counts, editability policies, 733-line `phase2PresetContract.ts`.

**After:** scenario buttons, optional sensitivity chip row, one run button.

### Step Progress

| Step | Status | Notes |
|------|--------|-------|
| 1 — `scenarios.ts` | ✅ Done | `src/lib/catalog/scenarios.ts` — single source of truth, 0 TS errors |
| 2 — `ScenarioPicker.svelte` | ✅ Done | ~150 lines vs 451 for the old `ModePanel.svelte` |
| 3 — Store wiring | ✅ Done | New state + actions; run button routes sweep vs single |
| 4 — Store simplification | ✅ Done (2026-03-17) | `buildScenarioNavigationState` removed; `evaluateAnalyticalStatus` moved to `warningPolicy.ts`; backward-compat re-exports in `phase2PresetContract.ts` |
| 5 — Wire App.svelte | ✅ Done | ScenarioPicker wired; `activeReferenceFamily` uses `activeScenarioAsFamily` |
| 6 — Simplify RunControls | ✅ Done | Removed "Advance 1 Step" + "Render Every N" |
| 7 — Delete old files | 🔲 Partial | `ModePanel.svelte` deleted. 8 files remain with active dependencies — see below |

### Step 7 — Remaining File Deletions

These files still have active production imports and cannot yet be deleted:

| File | Blocking dependency |
|------|---------------------|
| `src/lib/ui/cards/ReferenceExecutionCard.svelte` | Used by App.svelte |
| `src/lib/catalog/benchmarkCases.ts` | Used by ReferenceExecutionCard, charts |
| `src/lib/benchmarkRunModel.ts` | Used by store, charts, ReferenceResultsCard |
| `src/lib/benchmarkDisclosure.ts` | Used by ReferenceExecutionCard, ReferenceResultsCard |
| `src/lib/catalog/caseCatalog.ts` | Used throughout |
| `src/lib/catalog/caseLibrary.ts` | Used by caseCatalog.ts |
| `src/lib/catalog/presetCases.ts` | Used by caseLibrary.ts |
| `src/lib/stores/phase2PresetContract.ts` | Used by store, ScenarioPicker, modePanelTypes.ts |

**Path to deletion:** audit whether `ReferenceExecutionCard` and `benchmarkRunModel` are superseded by the S1 sweep-run model (Phase 2). If so, remove both together with their dependency chain. See `TODO.md` — Simplification Refactor — Step 7 Remainder.

---

## Phase 2 — Scenario/Sensitivity Architecture Redesign (S1)

**Goal:** Replace the current model — 18 scenarios each with one optional sensitivity dimension — with ~6 canonical scenarios each supporting multiple selectable sensitivity dimensions.

**Status:** ✅ Complete (2026-03-19). 28/28 tests pass.

### The Problem

The current `Scenario` type has `sensitivity?: ScenarioSensitivity` — a single optional slot. To study multiple independent parameters of the same base physics, a new scenario entry must be created for each. This leads to 6 separate scenario buttons for what is fundamentally one physics study (1D waterflood), and 3 separate Dietz scenarios for what are really sensitivity variants of the same depletion scenario.

The consequence: 18 scenarios in the picker, most of which are redundant entries rather than genuinely different physics.

### Design: `sensitivities[]`

```typescript
// Replaces: sensitivity?: ScenarioSensitivity
// New field on Scenario:
sensitivities: SensitivityDimension[];
defaultSensitivityDimensionKey?: string;  // shown first on scenario load

type SensitivityDimension = {
    key: string;            // 'mobility' | 'corey_no' | 'sor' | 'shape_factor' | ...
    label: string;          // "Mobility Ratio", "Corey n_o", "Shape Factor", ...
    description: string;
    variants: SensitivityVariant[];   // unchanged — up to 5 variants per dimension
    defaultVariantKey: string;        // pre-selected on dimension load
    chartPresetOverride?: string;     // optional: override scenario chartPreset for this dimension
};

// SensitivityVariant unchanged:
type SensitivityVariant = {
    key: string;
    label: string;
    description: string;
    paramPatch: Record<string, unknown>;
    affectsAnalytical: boolean;
};
```

**Key rule:** sensitivities are scenario-scoped. A sensitivity dimension that applies to 1D Waterflood (e.g. Mobility Ratio) is redefined independently for Areal Sweep — even if the variants are similar. This keeps each scenario self-contained.

### Canonical Scenario Map (18 → 6)

#### Waterflood domain

**`wf_1d_waterflood` — 1D Waterflood (Buckley-Leverett)**

Consolidates: BL Case A, BL Case B, Mobility Study, Corey n_o, Residual Oil, Capillary

| Dimension key | Label | Variants (up to 5) | Affects analytical |
|--------------|-------|--------------------|--------------------|
| `mobility` | Mobility Ratio | M≈0.2 (μ_o=0.25), M≈1 (μ_o=0.5), M≈2 (μ_o=1.0), M≈5 (μ_o=2.5), M≈10 (μ_o=5.0) | Yes |
| `corey_no` | Oil Corey Exponent | n_o = 1.5, 2.0, 3.5 | Yes |
| `sor` | Residual Oil | S_or = 0.05, 0.10, 0.15, 0.20, 0.30 | Yes |
| `capillary` | Capillary Pressure | P_e = 0, 0.3, 0.8, 1.5 bar | No (analytical stays sharp) |
| `grid` | Grid Resolution | 24, 48, 96 cells | No (BL solution grid-independent) |

Default dimension: `mobility`. Default base params: BL Case A (μ_o = 1.0 cp, s_wc = 0.1, s_or = 0.1, n_w = 2, n_o = 2, nx = 96).

*Note: BL Case B parameters (μ_o = 1.4 cp, n_w = 2.2, s_wc/s_or = 0.15) become a variant within the `mobility` dimension ("M≈2.3 adverse").*

#### Sweep domain

**`sweep_areal` — Areal Sweep (2D XY, Five-Spot)**

Consolidates: Areal Sweep – Mobility, Areal Sweep – Residual

| Dimension key | Label | Variants | Affects analytical |
|--------------|-------|----------|--------------------|
| `mobility` | Mobility Ratio | M≈0.2, M≈0.5, M≈1, M≈3, M≈10 | Yes |
| `sor` | Residual Oil | S_or = 0.05, 0.10, 0.20, 0.30 | Yes |

**`sweep_vertical` — Vertical Sweep (2D XZ, Dykstra-Parsons)**

No consolidation needed (was one scenario).

| Dimension key | Label | Variants | Affects analytical |
|--------------|-------|----------|--------------------|
| `heterogeneity` | Permeability Heterogeneity | V_DP = 0 (uniform), 0.5 (moderate), 0.85 (extreme) | No |
| `mobility` | Mobility Ratio | M≈0.5, M≈1, M≈5 | Yes |

**`sweep_combined` — Combined Volumetric Sweep (3D)**

No consolidation needed (was one scenario). Represents E_vol = E_A × E_V.

| Dimension key | Label | Variants | Affects analytical |
|--------------|-------|----------|--------------------|
| `combined` | Mobility + Heterogeneity | Ideal (M=1, V_DP=0), Moderate, Poor (M=10, V_DP=0.85) | Yes |

#### Depletion domain

**`dep_dietz` — Pressure Depletion (Dietz)**

Consolidates: Dietz Center, Dietz Corner, Skin, Permeability, Compressibility

| Dimension key | Label | Variants | Affects analytical |
|--------------|-------|----------|--------------------|
| `shape_factor` | Well Location | Center (C_A≈30.88), Corner (C_A≈0.56) | Yes |
| `skin` | Skin Factor | s = −2, 0, +5 | Yes |
| `permeability` | Permeability | k = 5, 20, 100 mD | Yes |
| `compressibility` | Compressibility | c_o = 5×10⁻⁶, 1×10⁻⁵, 5×10⁻⁵ bar⁻¹ | Yes |

Default dimension: `shape_factor`. Default base params: Dietz Center (producerI = 10, producerJ = 10, 21×21×1 grid).

**`dep_fetkovich` — Rate Decline (Fetkovich)**

No consolidation needed. Could extend with:

| Dimension key | Label | Variants | Affects analytical |
|--------------|-------|----------|--------------------|
| `skin` | Skin Factor | s = −2, 0, +5 | Yes |
| `permeability` | Permeability | k = 5, 20, 100 mD | Yes |

#### Gas domain (future)

**`gas_injection` — Gas Injection** — promote from experimental once physics bugs fixed.
**`gas_solution_drive` — Solution Gas Drive** — same condition.

### Store State After S1

```typescript
// Added to store:
activeSensitivityDimensionKey: string | null = $state(null);

// Updated actions:
selectScenario(scenarioKey: string): void
  // sets activeScenarioKey
  // sets activeSensitivityDimensionKey = scenario.defaultSensitivityDimensionKey ?? sensitivities[0]?.key ?? null
  // sets activeVariantKeys = [dimension.defaultVariantKey]

selectSensitivityDimension(dimensionKey: string): void
  // sets activeSensitivityDimensionKey
  // resets activeVariantKeys to [dimension.defaultVariantKey]

// Updated helper:
getScenarioWithVariantParams(scenarioKey, dimensionKey, variantKey?): Record<string, unknown>
  // merges: scenario.params + dimension variant's paramPatch
```

### UI Layout After S1

```
┌─────────────────────────────────────────────────────────┐
│  [Waterflood]  [Sweep]  [Depletion]  [Gas]              │  ← domain tabs
├─────────────────────────────────────────────────────────┤
│  Scenario: [1D Waterflood ●]  [Areal Sweep]  [Vertical] │
├─────────────────────────────────────────────────────────┤
│  Vary:  [Mobility ●]  [Corey n_o]  [S_or]  [Cap]  [Grid]│  ← dimension selector
│                                                          │  (hidden if only 1 dimension)
│  Mobility Ratio:                                         │
│  [M≈0.2] [M≈1] [M≈2 ✓] [M≈5] [M≈10]                   │  ← variant chips
│                                                          │
│  ✓ Analytical solution updates with each variant         │
└─────────────────────────────────────────────────────────┘
```

### Migration Notes

- Existing scenario keys (`wf_bl_case_a`, `dep_dietz_center`, etc.) should be preserved as aliases in `getScenario()` during transition to avoid breaking any persisted state or tests.
- The `scenarioClass` field on `Scenario` maps to domain tab: `'waterflood'` → Waterflood + Sweep, `'depletion'` → Depletion, `'3phase'` → Gas.
- Tests that count scenarios or variants will need updating. Update counts explicitly; do not use snapshot-style "N scenarios expected" without documenting the intent.

---

## Phase 3 — Candidate Cleanup After Review

These items were identified during the 2026-03-19 implementation review and are good candidates once Step 7 / F4 pressure drops.

### A. Extract Output Selection View Model

`App.svelte` currently owns parallel derived value groups for:

- selected reference result
- output profile inputs
- 3D output inputs
- analytical helper inputs

This creates avoidable duplication and makes it easier for charts, 3D output, and analytical adapters to drift.

**Suggested direction:** create one typed output-selection helper or store-facing view model that returns the active run/result payload shared by charts, 3D, and analytical components.

### B. Add Typed Analytical Adapter Contracts

The current depletion well-location mismatch exists because scenario metadata can claim `affectsAnalytical: true` while the analytical builder does not consume the required parameters.

**Suggested direction:**

- Add small adapter builders per analytical family.
- Add tests that fail when a sensitivity dimension marked `affectsAnalytical: true` does not affect any consumed analytical input.
- Keep scenario metadata and analytical plumbing coupled by tests instead of convention.
