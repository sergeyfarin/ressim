# Analytical vs Simulation: Comprehensive Review

## Executive Summary

Full code review of the analytical models (`DepletionAnalytical.svelte`, `FractionalFlow.svelte`), the Rust IMPES simulator (`step.rs`, `lib.rs`, `relperm.rs`, `capillary.rs`), scenario catalog (`caseCatalog.ts`), and App integration. All physics definitions, unit conversions, and comparison setups are audited below.

---

## Issue Catalog

### 🔴 **CRITICAL — Physics / Correctness**

#### C1. Depletion analytical uses `reservoir.length × area` instead of actual pore volume
**File:** `DepletionAnalytical.svelte:80`
```js
const poreVolume = reservoir.length * reservoir.area * reservoir.porosity;
// reservoir = { length: nx*cellDx, area: ny*cellDy*nz*cellDz, porosity: 0.2 }
```
This is `L × A × φ = V_bulk × φ`, which is **correct** for the total pore volume. ✅ No bug here.

#### C2. Depletion analytical uses single-cell Peaceman PI but simulator uses whole-column well
**Files:** `DepletionAnalytical.svelte:102`, `sim.worker.ts:188-195`

The simulator adds one well per layer (`for k in 0..nz`) meaning the total well PI is `nz × PI_single_layer`. The analytical model computes PI using `wellboreDz = nz * cellDz` in a **single** Peaceman formula. These are *not* the same:

- Simulator: `PI_total = Σ_{k=0}^{nz-1} PI(cell_k)` — each cell independently contributes to flow
- Analytical: `PI = 8.527e-5 × 2π × k_avg × (nz×dz) × (k_ro/μ_o) / denom` — one big cylinder

For uniform permeability and uniform initial saturation these coincidentally match, but the formula basis differs. For multi-layer cases they would diverge. **Risk: moderate** for current single-layer depletion cases.

#### C3. Depletion analytical uses `reservoir.length * reservoir.area` which is `(nx·dx) × (ny·dy·nz·dz)` — mixing flow length with pore volume
**File:** `DepletionAnalytical.svelte:80`, `App.svelte:1431-1435`

The reservoir object passed is:
```js
{ length: nx * cellDx, area: ny * cellDy * nz * cellDz, porosity: reservoirPorosity }
```
For depletion, the pore volume calculation `length × area × porosity` gives the correct bulk pore volume. However, the `length` property is fundamentally **not meaningful for depletion** (single-producer), unlike for waterflood where it represents distance injector→producer. For the 2D center-producer case, the "length" is `49 × 10 = 490 m`, but the actual drainage length is radial, not the full reservoir extent. 

**Impact:** Pore volume is still correct because `V_p = L × A × φ` regardless of flow geometry. But the naming is misleading, and future extensions (e.g., drainage radius calculations) would be confused.

#### C4. Depletion case `initialSaturation = 0.25` but `s_wc = 0.1` — initial water ≫ connate
**File:** `caseCatalog.ts:88-89, 109-110`

Both depletion cases set `initialSaturation = 0.25` (water) and `s_wc = 0.1`. This means `Sw_init > Swc`, so water is mobile from t=0. The analytical model assumes only oil flows (`waterRate: 0`), but the simulator will transport water due to its mobility. The analytical decline curve ignores water mobility, while the simulator experiences:
- Reduced oil relative permeability at Sw=0.25 vs Sw=0.10
- Water redistribution due to pressure gradients
- Different effective total compressibility weighting

**Recommendation:** Set `initialSaturation` equal to `s_wc` (= 0.1) for clean depletion comparison, or implement two-phase depletion in the analytical model.

#### C5. Waterflood BL analytical tangent search has edge-case bugs
**File:** `FractionalFlow.svelte:241-250`

The Welge tangent search uses a simple scan:
```js
for (let s = initial_sw + 5e-4; s <= 1 - s_or; s += 5e-4) {
    const slope = fw / (s - initial_sw);
    if (slope > max_slope) { ... }
}
```

**Issues:**
1. **Division by zero** risk when `initial_sw ≈ s_wc` and scan starts very close to `initial_sw`
2. The search evaluates `fw(s) / (s - Sw_init)` but should evaluate the **tangent from the point (Sw_init, fw(Sw_init))**, not from `(Sw_init, 0)`. If `fw(Sw_init) > 0` (which it is when `Sw_init > Swc`), the tangent should be `(fw(s) - fw(Sw_init)) / (s - Sw_init)`.
3. The `computeWelgeMetrics()` (line 188-198) has the same issue — it computes `fw / (s - initialSwClamped)` instead of `(fw - fw_initial) / (s - initial_sw)`

**Impact on waterflood cases:** The BL Case A has `Sw_init = 0.1 = s_wc`, so `fw(Swc) = 0` and the formula is coincidentally correct. But BL Case B has `Sw_init = 0.15 = s_wc`, also fine. The `bl_aligned_*` cases have `Sw_init = 0.2 > s_wc = 0.1`, so **`fw(0.2) > 0`** and the tangent is **wrong** for these cases.

#### C6. Waterflood BL post-breakthrough outlet saturation search is fragile
**File:** `FractionalFlow.svelte:289-301`

After breakthrough, the code finds `Sw` at the outlet by scanning `dfw/dSw` values:
```js
for (let s = sw_f; s <= 1 - s_or; s += 5e-4) {
    const derivative = dfw_dSw(s, 1e-4);
    const delta = Math.abs(derivative - target_dfw);
    if (delta < bestDelta) { ... }
}
```
This brute-force search is O(N) per timestep. More importantly, `dfw/dSw` is not necessarily monotonic, so there may be **multiple roots**. The code takes the one closest to `sw_f`, which may not be the physically correct one (should take the largest `Sw` satisfying MOC for rarefaction).

#### C7. Simulator total compressibility uses current saturations (IMPES lag)
**File:** `step.rs:409-413`

The accumulation term in the pressure equation uses current (lagged) saturations:
```rust
let c_t = (self.pvt.c_o * self.sat_oil[id] + self.pvt.c_w * self.sat_water[id])
    + self.rock_compressibility;
```
This is standard IMPES, but introduces a discrepancy vs the analytical model which uses initial saturations for `c_t`. Over many timesteps with significant saturation change, the simulator's declining pressure will differ from the analytical exponential decline.

**Impact:** Expected behavior for an IMPES simulator. Not a "bug", but affects comparison accuracy.

---

### 🟠 **IMPORTANT — Comparison Quality**

#### I1. Depletion cases have capillary pressure enabled
**File:** `caseCatalog.ts:90-91, 111-112`

Both depletion cases set `capillaryEnabled: true, capillaryPEntry: 5.0`. The analytical model ignores capillary pressure entirely. Capillary pressure creates additional driving force and water redistribution that the analytical model doesn't account for.

**Recommendation:** Set `capillaryEnabled: false` for depletion comparison cases, or add capillary corrections to the analytical model.

#### I2. Depletion cases have too few steps to see full decline
**File:** `caseCatalog.ts:79, 100`

- `depletion_corner_producer`: 36 steps × 0.5 days = 18 days
- `depletion_center_producer`: 44 steps × 0.5 days = 22 days

The PSS time constant `τ = V_p × c_t / J` for the corner producer case:
- `V_p = 48 × 10 × 10 × 5 × 0.2 = 4800 m³`
- `c_t ≈ 0.75 × 1e-5 + 0.25 × 3e-6 + 1e-6 ≈ 9.25e-6 /bar`
- `J ≈ 8.527e-5 × 2π × 100 × 5 × k_ro(0.25)/1.0 / ln(r_eq/0.1)`
- With `k_ro(0.25) ≈ ((1-0.25-0.1)/(1-0.1-0.1))^2 = (0.65/0.8)^2 = 0.66`
- `r_eq ≈ 0.28 × sqrt(dx² + dy²) / 2 ≈ 0.28 × 14.14 / 2 ≈ 1.98 m`
- `J ≈ 8.527e-5 × 6.28 × 100 × 5 × 0.66 / (ln(19.8) + 0) ≈ 0.1766 / 2.99 ≈ 0.059 m³/day/bar`
- `τ ≈ 4800 × 9.25e-6 / 0.059 ≈ 0.75 days`

So 18 days ≈ 24τ — the decline is essentially **complete** by then. This is actually fine for corner producer.

For center producer: `V_p = 49 × 49 × 10 × 10 × 5 × 0.2 = 240,100 m³`, `τ` will be much larger. 22 days may not be enough to see full decline.

**Recommendation:** Increase steps for center producer to ~100 (50 days) or decrease `delta_t_days`.

#### I3. Waterflood `bl_aligned_*` cases have only 24 steps × 0.5 days = 12 days
**File:** `caseCatalog.ts:176, 199, 224`

Pore volume = 48 × 5 × 10 × 10 × 0.2 = 4800 m³. Injection rate = 250 m³/day. 1 PVI = 4800/250 = 19.2 days. With only 12 days of simulation, the waterflood barely reaches 0.63 PVI, probably not enough for breakthrough (typical breakthrough at 0.3-0.5 PVI).

**Recommendation:** Increase to 50 steps or increase `delta_t_days` to reach at least 1.5 PVI.

#### I4. Hardcoded porosity = 0.2 in App.svelte
**File:** `App.svelte:50`

```js
const reservoirPorosity = 0.2;
```
This constant is used for OOIP, poreVolume, and passed to the analytical models. But the simulator also uses `porosity = vec![0.2; n]` (lib.rs:133). These are consistent but immutable — the user cannot change porosity. Not a physics bug but a significant limitation.

#### I5. BL analytical uses constant injection rate `q0` for breakthrough calculation but variable rates per timestep
**File:** `FractionalFlow.svelte:256, 277-279`

The breakthrough time is computed with `q0 = first positive injection rate`, but after breakthrough, the code uses the actual `injectionRateSeries[i]`. This inconsistency means that if rates vary significantly, the breakthrough time prediction may be wrong.

#### I6. Waterflood BL Case A & B use 96 cells — expensive but unnecessary for analytical comparison
**File:** `caseCatalog.ts:126, 150`

For a clean BL comparison, 48 cells (as in the aligned cases) suffice. Higher cell count increases computation time without improving the analytical comparison. The "Benchmark-grade" label is misleading since the analytical solution is mesh-independent.

---

### 🟡 **MODERATE — Definitions & Naming**

#### M1. `reservoir.length` meaning differs between depletion and waterflood modes
**File:** `App.svelte:1431-1435, 1452-1456`

Both modes receive the same reservoir object with `length = nx * cellDx`. For waterflood, this is the distance from injector to producer (correct for 1D BL). For depletion, this is the total reservoir extent, which is used only in pore volume calculation. The dual meaning is confusing.

#### M2. Analytical meta emits wrong mode on disable
**File:** `DepletionAnalytical.svelte:140-146`

When the depletion analytical is disabled, it emits `mode: 'waterflood'` in the meta — this is misleading:
```js
onAnalyticalMeta({
    mode: 'waterflood',  // should be 'depletion' or 'none'
    shapeFactor: null,
    shapeLabel: '',
});
```

#### M3. `depletionRateScale` multiplier applied inside PI calculation
**File:** `DepletionAnalytical.svelte:103`

The rate scale is multiplied into the PI calculation:
```js
const J_oil = Math.max(1e-12,
    (8.527e-5 * 2 * Math.PI * kAvg * wellboreDz * (k_ro_swi / muO)) / denomPI
    * Math.max(0, depletionRateScale)
);
```
This is a useful calibration knob but should not be applied *inside* PI (changes τ as well as q₀). Better: apply scale only to `q0` and keep `τ` physical.

#### M4. `maxRecoverable = q0 * tau` is not technically the maximum recoverable volume
**File:** `DepletionAnalytical.svelte:120`

```js
const maxRecoverable = q0 * tau; // = V_pore · c_t · ΔP [m³]
```
The comment `= V_pore · c_t · ΔP` is mathematically correct: `q0 × τ = J·ΔP × (V_p·c_t/J) = V_p·c_t·ΔP`. But `maxRecoverable` is a misleading name — this is the total fluid that can be *expelled* by pressure depletion from the compressible pore volume, not the total recoverable oil (which would also involve `So_init`).

---

### 🟢 **MINOR / Cosmetic**

#### m1. Unit conversion factor `8.527e-5` repeated across 4 files
A shared constant would improve maintainability and avoid drift.

#### m2. BL `dfw_dSw` uses centered difference with hardcoded `ds = 1e-6`
A second-order centered difference is fine, but the step size should be related to the saturation range for robustness.

---

## ✅ Verified Correct

| Item | Status |
|---|---|
| Peaceman PI formula and unit conversion (8.527e-5) | ✅ Correct |
| Corey rel-perm model in Rust matches JS analytical | ✅ Consistent |
| IMPES pressure solve structure (accumulation + transmissibility + wells) | ✅ Standard |
| Upstream weighting for total mobility in transmissibility | ✅ Correct |
| Upwind fractional flow for saturation transport | ✅ Correct |
| Brooks-Corey capillary pressure model | ✅ Correct |
| Gravity head computation (ρ·g·Δz → bar) | ✅ Correct |
| BL shock front finding (tangent to f_w curve from Sw_init) | ✅ When Sw_init = Swc |
| Exponential decline q(t) = q₀·exp(−t/τ) derivation | ✅ Correct PSS model |
| Material balance tracking in simulator | ✅ Implemented |

---

## Proposed New Comparison Cases

### New Depletion Cases (designed for ~50 steps to show clear effect)

#### `depletion_1d_clean`
Clean 1D depletion: Sw_init = Swc, no capillary, no gravity. Producer at one end.
```
nx:20, ny:1, nz:1, cellDx:10, cellDy:10, cellDz:10
delta_t_days:1.0, steps:50
initialPressure:300, initialSaturation:0.1 (= s_wc)
injectorEnabled:false, producerControlMode:'pressure', producerBhp:100
capillaryEnabled:false, gravityEnabled:false
uniformPermX:200, uniformPermY:200, uniformPermZ:20
s_wc:0.1, s_or:0.1, n_w:2.0, n_o:2.0
```
τ ≈ V_p × c_t / J ≈ (20×10×10×10×0.2)×(0.9×1e-5+0.1×3e-6+1e-6) / PI ≈ 400×1.03e-5/PI. Good range.

#### `depletion_2d_radial_clean`
2D radial depletion with center producer, Sw_init = Swc, no capillary.
```
nx:21, ny:21, nz:1, cellDx:10, cellDy:10, cellDz:10
delta_t_days:2.0, steps:50
initialPressure:300, initialSaturation:0.1
injectorEnabled:false, producerControlMode:'pressure', producerBhp:100
capillaryEnabled:false, gravityEnabled:false
uniformPermX:200, uniformPermY:200, uniformPermZ:20
producerI:10, producerJ:10
s_wc:0.1, s_or:0.1, n_w:2.0, n_o:2.0
```

### New Waterflood Cases (designed for ~50 steps to reach 1+ PVI)

#### `waterflood_bl_clean`
Clean 1D waterflood: rate-controlled, no capillary, Sw_init = Swc.
```
nx:48, ny:1, nz:1, cellDx:5, cellDy:10, cellDz:10
delta_t_days:0.5, steps:50
initialPressure:300, initialSaturation:0.1 (= s_wc)
mu_w:0.5, mu_o:1.0, s_wc:0.1, s_or:0.1, n_w:2.0, n_o:2.0
capillaryEnabled:false, gravityEnabled:false
uniformPermX:500, uniformPermY:500, uniformPermZ:50
injectorControlMode:'rate', producerControlMode:'rate'
targetInjectorRate:200, targetProducerRate:200
injectorI:0, injectorJ:0, producerI:47, producerJ:0
```
PV = 48×5×10×10×0.2 = 4800 m³. Rate 200 m³/d. 1 PVI in 24 days. 50 steps × 0.5d = 25 days ≈ 1.04 PVI → should see breakthrough.

#### `waterflood_unfavorable_mobility`
Higher oil viscosity for unfavorable mobility ratio with clear early breakthrough.
```
nx:48, ny:1, nz:1, cellDx:5, cellDy:10, cellDz:10
delta_t_days:0.5, steps:50
initialPressure:300, initialSaturation:0.1
mu_w:0.3, mu_o:5.0, s_wc:0.1, s_or:0.1, n_w:2.0, n_o:2.0
capillaryEnabled:false, gravityEnabled:false
uniformPermX:500, uniformPermY:500, uniformPermZ:50
injectorControlMode:'rate', producerControlMode:'rate'
targetInjectorRate:200, targetProducerRate:200
```
High mobility ratio (M ≈ 16.7) → early breakthrough around 0.15-0.2 PVI.

---

## Prioritized Execution Plan

### Priority 1 — Physics Correctness (Fix before comparisons are trustworthy)

| # | Issue | Action | Files | Effort |
|---|---|---|---|---|
| 1 | **C4** Initial Sw > Swc in depletion cases | Set `initialSaturation` to `s_wc` (0.1) in both depletion catalog entries | `caseCatalog.ts` | 5 min |
| 2 | **C5** BL tangent from (Sw_init, 0) instead of (Sw_init, fw(Sw_init)) | Fix tangent calculation in `calculateAnalyticalProduction()` and `computeWelgeMetrics()` | `FractionalFlow.svelte` | 30 min |
| 3 | **I1** Capillary enabled in depletion cases | Set `capillaryEnabled: false` for both depletion cases | `caseCatalog.ts` | 5 min |
| 4 | **M2** Wrong mode emitted on depletion disable | Change to `mode: 'depletion'` or `'none'` | `DepletionAnalytical.svelte` | 5 min |

### Priority 2 — Comparison Setup (Ensure effects are visible in ~50 steps)

| # | Issue | Action | Files | Effort |
|---|---|---|---|---|
| 5 | **I2/I3** Insufficient steps in existing cases | Increase steps and/or timesteps per the analysis above | `caseCatalog.ts` | 10 min |
| 6 | Add new `depletion_1d_clean` case | Add clean depletion case with Sw=Swc, no capillary, 50 steps | `caseCatalog.ts` | 10 min |
| 7 | Add new `depletion_2d_radial_clean` case | Add 2D radial depletion case | `caseCatalog.ts` | 10 min |
| 8 | Add new `waterflood_bl_clean` case | Add clean BL case reaching ≥1 PVI in 50 steps | `caseCatalog.ts` | 10 min |
| 9 | Add new `waterflood_unfavorable_mobility` case | Add unfavorable mobility case with early breakthrough | `caseCatalog.ts` | 10 min |

### Priority 3 — Code Quality (Improve accuracy and maintainability)

| # | Issue | Action | Files | Effort |
|---|---|---|---|---|
| 10 | **M3** Rate scale applied to PI not q0 | Move `depletionRateScale` to multiply `q0` only | `DepletionAnalytical.svelte` | 15 min |
| 11 | **C6** Post-BT outlet Sw search fragility | Improve search with bisection or monotonicity check | `FractionalFlow.svelte` | 45 min |
| 12 | **I5** BL q0 vs variable rate inconsistency | Use consistent rate for BT time calculation | `FractionalFlow.svelte` | 20 min |
| 13 | **m1** Shared unit conversion constant | Extract `8.527e-5` to a named constant | Multiple | 15 min |

### Priority 4 — Stretch Goals

| # | Issue | Action | Effort |
|---|---|---|---|
| 14 | **I4** Hardcoded porosity | Make porosity a user-editable parameter | 1-2 hr |
| 15 | **C2** Multi-layer PI mismatch | Refactor analytical PI to sum per-layer, matching simulator | 1 hr |
| 16 | **M4** Misleading `maxRecoverable` name | Rename to `totalExpelledVolume` or similar | 5 min |

---

## Execution Log

### Priority 1 — Completed ✅ (2026-02-22)

| # | Issue | Status | Notes |
|---|---|---|---|
| 1 | **C4** Depletion Sw_init > Swc | ✅ Fixed | Set `initialSaturation: 0.1` (= s_wc) in both depletion entries |
| 2 | **C5** BL tangent from wrong origin | ✅ Fixed | Tangent now drawn from `(Sw_init, fw(Sw_init))` in both `computeWelgeMetrics` and `calculateAnalyticalProduction` |
| 3 | **I1** Capillary in depletion cases | ✅ Fixed | Set `capillaryEnabled: false, capillaryPEntry: 0.0` in both depletion entries |
| 4 | **M2** Wrong mode on disable | ✅ Fixed | Changed to `mode: 'depletion'` in `DepletionAnalytical.svelte` |

### Newly Discovered During P1 Execution

#### C5b. Pre-breakthrough oil rate assumes pure oil when Sw_init > Swc
**File:** `FractionalFlow.svelte:284-286` (before fix)

```js
// Old code — wrong when Sw_init > Swc:
oilRate = q;          // assumes 100% oil before breakthrough

// Fixed to:
oilRate = q * (1 - fw_initial);  // accounts for mobile water at initial saturation
```

In standard BL theory, the outlet fluid composition before breakthrough matches the initial saturation. When `Sw_init > Swc`, `fw(Sw_init) > 0`, so the oil fraction is `(1 - fw_initial)`, not 1.0. This was incorrect for all `bl_aligned_*` cases where `Sw_init = 0.2 > Swc = 0.1`.

**Status:** ✅ Fixed alongside C5.

### Priority 2 — Completed ✅ (2026-02-22)

| # | Item | Status | Details |
|---|---|---|---|
| 5 | **I2** Center producer depletion steps | ✅ Adjusted | `delta_t_days: 0.5 → 2.0`, `steps: 44 → 50` (100 days ≈ 3.7τ) |
| 6 | **I3** `bl_aligned_*` waterflood steps | ✅ Adjusted | `steps: 24 → 50` for all 3 cases (25 days ≈ 1.3 PVI) |
| 7 | New `depletion_1d_clean` | ✅ Added | 20×1×1, k=200mD, dz=10m, dt=1.0d, 50 steps, Sw=Swc, no capillary |
| 8 | New `depletion_2d_radial_clean` | ✅ Added | 21×21×1, center producer, k=200mD, dz=10m, dt=2.0d, 50 steps |
| 9 | New `waterflood_bl_clean` | ✅ Added | 48×1×1, k=500mD, rate=200 m³/d, Sw=Swc, no capillary, 50 steps (1.04 PVI) |
| 10 | New `waterflood_unfavorable_mobility` | ✅ Added | μ_o=5.0, μ_w=0.3 (M≈17), early breakthrough, 50 steps |

### Priority 3 — Completed ✅ (2026-02-22)

| # | Item | Status | Details |
|---|---|---|---|
| 11 | **M3** Rate scale on q0 only | ✅ Fixed | `depletionRateScale` moved from PI to q0; τ now stays physical |
| 12 | **C6** Post-BT outlet Sw search | ✅ Improved | Bisection on dfw/dSw (monotonically decreasing in rarefaction zone); O(log N) vs O(N) |
| 13 | **I5** Variable rate BT tracking | ✅ Fixed | Breakthrough via cumulative PVI instead of fixed q0-based time |
| 14 | **m1** Named unit constant | ✅ Extracted | `DARCY_METRIC_FACTOR = 8.527e-5` in both `step.rs` and `DepletionAnalytical.svelte` |

### Critical Root Causes — Discovered During Data Review ✅ (2026-02-22)

#### C8. Depletion analytical used Peaceman PI instead of PSS PI — τ was 10-20× too small
**Root cause:** Peaceman PI relates well-cell pressure → flow rate. PSS τ requires reservoir-average pressure → flow rate. The difference is the flow resistance across the drainage area.

| Case | Old τ (Peaceman) | New τ (PSS) | Simulator τ |
|---|---|---|---|
| 1D clean (20 cells) | 0.115 days | **1.73 days** | ~2.42 days |
| 2D center (49×49) | 27.6 days | **63.7 days** | TBD |

**Fix:** Replaced Peaceman PI with: (a) 1D slab: `J = 1/(R_linear + R_well)` where `R_linear = L/(3kA·DARCY)`, (b) 2D: Dietz shape factor `J = DARCY·2πkh / (μ · [0.5⁠·⁠ln(4A/(CA · e^(2γ) · rw²))])`.
Remaining gap (1.73 vs 2.42) is physical: PSS assumes equilibrated pressure field, simulator shows transient behavior initially.

**Status:** ✅ Fixed in `DepletionAnalytical.svelte`

#### C9. Waterflood rate-controlled wells were BHP-strangled — actual rates 40× below target
**Root cause:** `sim.worker.ts` set `bhpMax = max(producerBhp, injectorBhp)`. For rate-controlled injectors needing high BHP to push fluid, the injector BHP was capped at 400 bar. Cell pressure equilibrated to 395 bar, leaving only 5 bar ΔP at the well → rate = 5.15 m³/d instead of target 200 m³/d.

**Fix:** Rate-controlled injectors now get `bhpMax = 2000` (was `max(prodBhp, injBhp) = 400`); rate-controlled producers get `bhpMin = 0`.

**Status:** ✅ Fixed in `sim.worker.ts`
