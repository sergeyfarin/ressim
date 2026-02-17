# Phase 1 — Foundations & Correctness: Walkthrough

## Changes Made

### 1.1 Saturation-Weighted Compressibility
**File:** [lib.rs](file:///home/reken/Repos/ressim/src/lib/ressim/src/lib.rs#L695-L700)

Total compressibility was computed once as `c_t = c_o + c_w + c_r` — uniform across the grid and independent of saturation. Now computed per cell as:

```
c_t = ϕ · (c_o · S_o + c_w · S_w) + c_r
```

This correctly weights fluid compressibilities by their respective phase saturations, producing physically accurate accumulation terms in the pressure equation.

---

### 1.2 Transmissibility Constant Audit
**Files:** [lib.rs](file:///home/reken/Repos/ressim/src/lib/ressim/src/lib.rs#L519-L524) (transmissibility) and [lib.rs](file:///home/reken/Repos/ressim/src/lib/ressim/src/lib.rs#L424-L428) (well PI)

Changed from `0.001127` (bbl/day/psi) → `8.527e-5` (m³/day/bar) for consistency with the metric unit system.

Derivation: `1 mD = 9.8692e-16 m², 1 cP = 1e-3 Pa·s, 1 bar = 1e5 Pa, 1 day = 86400 s` → `Factor = 9.8692e-16 × 86400 / (1e-3 × 1e5) = 8.527e-5`

Both transmissibility calculation and Peaceman well PI use the same constant.

---

### 1.3 Material-Balance Error Tracking
**Files:** [lib.rs](file:///home/reken/Repos/ressim/src/lib/ressim/src/lib.rs#L34-L42) (struct), [lib.rs](file:///home/reken/Repos/ressim/src/lib/ressim/src/lib.rs#L931-L945) (computation)

- Added `material_balance_error_m3` to `TimePointRates` struct
- Added cumulative injection/production tracking fields
- Each timestep computes: `|net_volume_from_wells − actual_grid_change|`
- Serialized to frontend via existing `rateHistory` pathway

---

### 1.4 PCG Solver Convergence Warning
**Files:**
- [lib.rs](file:///home/reken/Repos/ressim/src/lib/ressim/src/lib.rs#L1232-L1262) — `PcgResult` struct, solver returns `(solution, converged, iterations)`
- [lib.rs](file:///home/reken/Repos/ressim/src/lib/ressim/src/lib.rs#L535-L545) — `step()` captures warnings
- [lib.rs](file:///home/reken/Repos/ressim/src/lib/ressim/src/lib.rs#L958-L962) — `getLastSolverWarning()` WASM API
- [sim.worker.js](file:///home/reken/Repos/ressim/src/lib/sim.worker.js#L42) — passes warning in state payload
- [App.svelte](file:///home/reken/Repos/ressim/src/App.svelte#L104) — captures `solverWarning` from worker state
- [DynamicControlsPanel.svelte](file:///home/reken/Repos/ressim/src/lib/ui/DynamicControlsPanel.svelte#L65-L67) — displays ⚠ warning next to simulation status

## Verification

| Check | Result |
|-------|--------|
| `cargo test` (14 tests) | ✅ All pass |
| `npm run build:wasm` | ✅ Compiles cleanly |
| `npm run build` (production + benchmarks) | ✅ Builds successfully |
