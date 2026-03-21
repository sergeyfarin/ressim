# Frontend Architecture — Working Document

> Track active architecture work and design decisions. Delete sections when complete and docs are refreshed.

---

## Completed Work

### Phase 1 — Case-Library Simplification ✅

Replaced 4-layer "case library + editability policies" navigation with **pick scenario → optionally pick sensitivity → run** flow. `scenarios.ts` + `ScenarioPicker.svelte` are now the primary scenario surface (~150 lines vs 451 for old `ModePanel.svelte`).

**Step 7 partial:** 8 legacy catalog files still have active production imports and cannot yet be deleted. See TODO.md Phase 1C.

### Phase 2 — Scenario/Sensitivity Architecture Redesign (S1) ✅

Consolidated 18 scenarios → 8 canonical scenarios with `sensitivities: SensitivityDimension[]` (multi-dimension per scenario). 28/28 tests pass. See TODO.md Completed section for details.

---

## Active Architecture: Custom Mode Redesign

### Problem Statement

Custom mode currently dumps 50+ raw parameter inputs (Geometry, Reservoir, Wells, Timestep, Analytical, SCAL, Gas) with no relationship to predefined scenarios. After S1 (8 canonical scenarios × multiple sensitivity dimensions), custom mode reads as legacy, not intentional.

Key issues:
- **No provenance**: entering custom mode loses the active scenario context
- **No clone-and-edit**: users can't start from a scenario and tweak individual params
- **Mixed systems**: custom mode still routes through old `CaseMode` enum (`'wf'`, `'dep'`, `'sim'`) and toggle-based case library, not the new scenario system
- **No persistence**: custom configurations are ephemeral; no save/load
- **No guidance**: no quick-picks for common reservoir types, no validation warnings

### Design Direction

**Per-scenario customisation (preferred over full custom mode):**

```
┌─────────────────────────────────────────────────────────┐
│  Scenario: 1D Waterflood (Buckley-Leverett)             │
│  Vary: [Mobility ●] [Corey n_o] [S_or] [Cap] [Grid]   │
│                                                          │
│  ┌─ Overrides ────────────────────────────────────────┐ │
│  │  μ_o: 1.0 → [2.5] cp  (modified)     [Reset ↺]   │ │
│  │  n_w: 2.0 → [3.0]     (modified)     [Reset ↺]   │ │
│  │  + Add parameter override...                       │ │
│  └────────────────────────────────────────────────────┘ │
│                                                          │
│  Analytical: Buckley-Leverett (updates with overrides)  │
│  [Run]  [Reset All Overrides]                           │
└─────────────────────────────────────────────────────────┘
```

- User stays within a scenario; overrides are tracked and resettable individually
- Analytical solution updates live as params change (if `affectsAnalytical`)
- No need to "switch modes" — customisation is a layer on top of the scenario
- Full custom mode remains as escape hatch for configurations that don't fit any scenario

**Full custom mode (power-user):**

```
┌─────────────────────────────────────────────────────────┐
│  Custom Configuration                                    │
│  Based on: 1D Waterflood — Mobility Ratio (cloned)      │
│                                                          │
│  ▸ Rock Properties          (3 fields modified)          │
│  ▸ Fluid PVT                (1 field modified)           │
│  ▾ Wells                                                 │
│    │  well_radius: [0.1] m   well_skin: [0]              │
│    │  Injector BHP: [250] bar                            │
│    │  Producer BHP: [100] bar                            │
│  ▸ Grid & Timestep                                       │
│  ▸ Relative Permeability                                 │
│  ▸ Gas / 3-Phase             (hidden for 2-phase)        │
│                                                          │
│  [Save Configuration ▾]  [Load ▾]  [Export JSON]         │
└─────────────────────────────────────────────────────────┘
```

- Always clones from a scenario (provenance shown)
- Domain-aware collapsible groups (not flat 50-field form)
- Modification indicators per section
- Save/load/export capabilities

### Implementation Notes

- `SimulationStore` needs per-field override tracking: `Map<string, { base: unknown, override: unknown }>`
- Scenario stays active when overrides are applied; only enters full custom mode if user explicitly requests
- `getScenarioWithVariantParams()` should accept optional overrides map
- UI: `ScenarioPicker` shows override indicator badge when active scenario has overrides
- Analytical adapters should re-evaluate when overrides affect analytical inputs

---

## Active Architecture: Analytical Adapter Contracts

### Problem

The Dietz well-location sensitivity bug pattern: scenario metadata claims `affectsAnalytical: true` (or should), but the analytical builder doesn't actually consume the relevant parameter. No automated check catches this.

### Design

```typescript
// Per analytical family, define consumed inputs:
type AnalyticalContract = {
    scenarioClass: string;              // 'waterflood' | 'depletion' | 'sweep'
    consumedParams: string[];           // params that change analytical output
    builder: (params: Record<string, unknown>) => AnalyticalResult;
};

// Contract test:
// For each scenario with sensitivities:
//   For each dimension with affectsAnalytical: true:
//     For each variant:
//       Assert: builder(base + patch) !== builder(base)
//       i.e., the analytical output actually changes
```

This prevents silent decoupling between metadata and implementation.

---

## Active Architecture: Black-Oil PVT Extension

### Current State

All fluid properties are constant scalars in `FluidProperties` (Rust struct):
- `mu_o`, `mu_w`, `mu_g` — viscosities (cP)
- `rho_o`, `rho_w`, `rho_g` — densities (kg/m³)
- `c_o`, `c_w`, `c_g` — compressibilities (1/bar)

No pressure dependence, no dissolved gas, no formation volume factors.

### Target State (Phase 4)

```rust
enum PvtModel {
    Constant(ConstantPvt),          // Current behavior
    BlackOil(BlackOilPvt),          // New: pressure-dependent
}

struct BlackOilPvt {
    bubble_point: f64,              // P_b (bar)
    // Correlation-based (Standing, Vazquez-Beggs, etc.)
    api_gravity: f64,
    gas_specific_gravity: f64,
    temperature: f64,               // reservoir temperature (°C)
    // Or tabular:
    pvt_table: Option<PvtTable>,    // user-supplied Bo, Rs, μ vs P
}

struct PvtTable {
    pressure: Vec<f64>,
    bo: Vec<f64>,                   // oil FVF
    rs: Vec<f64>,                   // solution GOR
    mu_o: Vec<f64>,                 // oil viscosity
    bg: Vec<f64>,                   // gas FVF
    mu_g: Vec<f64>,                 // gas viscosity
}
```

### Impact on Simulator

1. **Accumulation term**: `(V_p / dt) × [∂(S_o/Bo)/∂P + ∂(S_g/Bg)/∂P + ∂(S_w/Bw)/∂P] × ΔP`
2. **Transmissibility**: mobility uses `μ(P)` from PVT lookup
3. **Phase volumes**: `V_o = V_p × S_o / Bo(P)`, `V_g = V_p × S_g / Bg(P)`
4. **Gas liberation**: when `P < P_b`, compute `Rs(P)` and increase `S_g` by liberated gas volume
5. **Well model**: GOR at producer = `Rs(P_well) + (k_rg/μ_g) / (k_ro/μ_o) × (Bg/Bo)`

### Migration Strategy

- Keep `PvtModel::Constant` as default — zero regression risk for existing scenarios
- `BlackOilPvt` activated only when user sets `bubble_point > 0` or selects a volatile-oil scenario
- All existing tests continue to use constant PVT
- New test suite for black-oil: SPE1 and SPE3 comparative solutions

---

## Canonical Scenario Map (Current: 8 Scenarios)

| Domain | Key | Sensitivity Dimensions | Analytical |
|--------|-----|------------------------|------------|
| Waterflood | `wf_bl1d` | Mobility, Corey n_o, S_or, Capillary, Grid | Buckley-Leverett |
| Sweep | `sweep_areal` | Mobility, Areal heterogeneity, S_or | Craig (1971) |
| Sweep | `sweep_vertical` | V_DP heterogeneity, Mobility | Dykstra-Parsons (1950) |
| Sweep | `sweep_combined` | Combined mobility × heterogeneity | Craig × DP × BL |
| Depletion | `dep_pss` | Well location, Skin, Permeability, Compressibility | Dietz (1965) |
| Depletion | `dep_decline` | Skin, Permeability | Fetkovich exponential |
| Gas | `gas_injection` | *None yet* | *Planned: gas-oil BL* |
| Gas | `gas_drive` | *None yet* | *Planned: immiscible gas depletion* |

### Planned Additions (Phase 4)

| Domain | Key | Description | Analytical |
|--------|-----|-------------|------------|
| Volatile Oil | `vo_depletion` | Undersaturated oil depletion below P_b | Arps hyperbolic + MB |
| Gas Cap | `gc_expansion` | Primary gas cap over oil column | Schilthuis MB |
| Gas Cap | `gc_secondary` | Secondary gas cap from solution gas | MB-predicted gas liberation |
| Gas | `gas_depletion` | Dry gas volumetric depletion | p/z analysis |

---

## Store State Summary

```typescript
// Scenario state (S1, complete):
activeScenarioKey: string | null;
activeSensitivityDimensionKey: string | null;
activeVariantKeys: string[];
isCustomMode: boolean;

// Planned additions (custom mode redesign):
parameterOverrides: Map<string, unknown>;     // per-field overrides within active scenario
customConfigName: string | null;               // name for save/load
customConfigProvenance: string | null;         // scenario key this was cloned from
```
