# Three-Phase Flow Implementation Plan

## Context
Add oil-water-gas three-phase simulation capability as a new `'3p'` CaseMode.
Existing `'wf'` (waterflood) and `'dep'` (depletion) two-phase modes are fully preserved
including all analytical comparison curves. The 3-phase mode uses its own scenarios and will
be validated against user-supplied analytical solutions externally.

Constraint: the analytical solution layer (`fractionalFlow.ts`, `depletionAnalytical.ts`) is
**not touched**.

---

## Architecture Decisions

### 1. Three-Phase Relative Permeability — Stone II
- k_rw(S_w) — Corey, function of S_w only (same form as 2-phase)
- k_rg(S_g) — Corey, function of S_g only
- k_ro(S_w, S_g) — Stone II model using normalised saturations
  - Stone II: k_ro = k_ro_max · (k_ro_w/k_ro_max + k_rw) · (k_ro_g/k_ro_max + k_rg) − k_rw − k_rg
  - Where k_ro_w = k_ro(S_w, 0), k_ro_g = k_ro(0, S_g) (2-phase endpoints)
  - Clamped to [0, k_ro_max]

### 2. Capillary Pressure
- Keep existing oil-water curve: P_cow(S_w) — unchanged
- Add oil-gas curve: P_cog(S_g) — same Brooks-Corey form, own entry pressure + lambda
  - P_oil − P_gas = P_cog(S_g)
  - Phase pressures: P_water = P_oil − P_cow, P_gas = P_oil − P_cog

### 3. Injected Phase
- New parameter `injectedFluid: "water" | "gas"` controls what the injector injects
- Default for 3-phase mode: gas injection (gas flooding / solution gas)
- Injector fractional flow: f_inj_phase = 1.0, others = 0.0

### 4. Pressure Equation (IMPES stays single equation per cell)
Three phase potentials but still one pressure unknown per cell (oil pressure as reference):
- dphi_o = (P_oil_i − P_oil_j) − grav_o
- dphi_w = (P_oil_i − P_oil_j) − (P_cow_i − P_cow_j) − grav_w
- dphi_g = (P_oil_i − P_oil_j) + (P_cog_i − P_cog_j) − grav_g  ← note sign: P_gas = P_oil − P_cog, so P_oil = P_gas + P_cog
Total mobility: λ_t = λ_w + λ_o + λ_g

### 5. Saturation Update
Two explicit transport equations solved after pressure:
- Δv_water (from water fluxes)
- Δv_gas (from gas fluxes)
- S_w_new = S_w + Δv_water / V_p
- S_g_new = S_g + Δv_gas / V_p
- S_o_new = 1 − S_w_new − S_g_new (enforced by material balance)
Clamp all saturations to [0,1] and re-normalise if sum ≠ 1.

### 6. CFL Extended
Add gas saturation change check alongside existing water and pressure checks.

### 7. New CaseMode
`'3p'` added to `CaseMode = 'dep' | 'wf' | 'sim' | '3p'`.
The UI routes it to a new ScenarioPicker group and shows a gas-specific section.

---

## File-by-File Implementation

---

### LAYER 1: Rust Simulator

#### `src/lib/ressim/src/relperm.rs`
**Current:** 49 lines, Corey 2-phase (k_rw, k_ro).

Add `RockFluidPropsThreePhase` struct and Stone II implementation:

```rust
// New struct alongside existing RockFluidProps
pub struct RockFluidPropsThreePhase {
    // Oil-water endpoints (same as 2-phase)
    pub s_wc: f64,      // connate water saturation
    pub s_or: f64,      // residual oil saturation (oil-water system)
    pub n_w: f64,
    pub n_o: f64,
    pub k_rw_max: f64,
    pub k_ro_max: f64,

    // Gas endpoints
    pub s_gc: f64,      // critical gas saturation (min S_g for gas to flow)
    pub s_gr: f64,      // residual gas saturation (trapped gas)
    pub n_g: f64,       // gas Corey exponent
    pub k_rg_max: f64,  // max gas rel perm at S_o = S_or
}

impl RockFluidPropsThreePhase {
    pub fn k_rw(&self, s_w: f64) -> f64 { /* Corey, same formula as 2-phase */ }

    pub fn k_rg(&self, s_g: f64) -> f64 {
        // Corey: S_g_eff = (S_g − S_gc) / (1 − S_wc − S_gc − S_gr)
        // k_rg = k_rg_max * S_g_eff^n_g
    }

    pub fn k_ro_water(&self, s_w: f64) -> f64 {
        // k_ro in oil-water 2-phase (no gas): same Corey
    }

    pub fn k_ro_gas(&self, s_g: f64) -> f64 {
        // k_ro in oil-gas 2-phase (no water beyond S_wc):
        // S_o_eff = (1 − S_wc − S_g − S_gr) / (1 − S_wc − S_gr)
        // k_ro_gas = k_ro_max * S_o_eff^n_o
    }

    pub fn k_ro_stone2(&self, s_w: f64, s_g: f64) -> f64 {
        // Stone II:
        // k_ro = k_ro_max * [ (k_ro_w/k_ro_max + k_rw) * (k_ro_g/k_ro_max + k_rg) − k_rw − k_rg ]
        // Clamped to [0, k_ro_max]
        let kro_w = self.k_ro_water(s_w);
        let kro_g = self.k_ro_gas(s_g);
        let krw   = self.k_rw(s_w);
        let krg   = self.k_rg(s_g);
        let val = k_ro_max * ((kro_w / k_ro_max + krw) * (kro_g / k_ro_max + krg) - krw - krg);
        val.clamp(0.0, k_ro_max)
    }
}
```

Keep existing `RockFluidProps` (2-phase) entirely unchanged.

---

#### `src/lib/ressim/src/capillary.rs`
**Current:** 52 lines, one struct `CapillaryPressure` for P_cow(S_w).

Add a second struct for gas-oil capillary:

```rust
// Existing CapillaryPressure: P_cow(S_w) — UNCHANGED

// New:
pub struct GasOilCapillaryPressure {
    pub p_entry: f64,   // entry pressure [bar]
    pub lambda: f64,    // Brooks-Corey exponent
}

impl GasOilCapillaryPressure {
    // P_cog(S_g): oil-gas capillary pressure as function of gas saturation
    // Higher S_g → lower P_cog (analogous to oil-water)
    // S_g_eff = (S_g − S_gc) / (1 − S_wc − S_gc − S_gr)
    pub fn capillary_pressure_og(&self, s_g: f64, rock: &RockFluidPropsThreePhase) -> f64 {
        // Same Brooks-Corey form as P_cow but parameterised on S_g
    }
}
```

---

#### `src/lib/ressim/src/lib.rs`
**Current:** 1612 lines. `ReservoirSimulator` has `sat_water`, `sat_oil`, `scal: RockFluidProps`, `pc: CapillaryPressure`.

**Changes:**

1. Add a `ThreePhaseMode` flag and optional fields to `ReservoirSimulator` (or use an enum):

```rust
pub struct ReservoirSimulator {
    // ... all existing fields unchanged ...

    // Three-phase additions (None when running 2-phase)
    sat_gas: Vec<f64>,                                  // gas saturation per cell
    scal_3p: Option<RockFluidPropsThreePhase>,          // 3-phase rel perm params
    pc_og: Option<GasOilCapillaryPressure>,             // oil-gas capillary curve
    three_phase_mode: bool,                             // true = use 3-phase equations
    injected_fluid: InjectedFluid,                      // "water" | "gas"
    mu_g: f64,                                          // gas viscosity [cP]
    c_g: f64,                                           // gas compressibility [1/bar]
    rho_g: f64,                                         // gas density [kg/m³]
}
```

2. New WASM export methods:

```rust
#[wasm_bindgen(js_name = setThreePhaseRelPermProps)]
pub fn set_three_phase_rel_perm_props(
    &mut self,
    s_wc: f64, s_or: f64,
    s_gc: f64, s_gr: f64,
    n_w: f64, n_o: f64, n_g: f64,
    k_rw_max: f64, k_ro_max: f64, k_rg_max: f64,
) -> Result<(), String>

#[wasm_bindgen(js_name = setGasOilCapillaryParams)]
pub fn set_gas_oil_capillary_params(&mut self, p_entry: f64, lambda: f64)

#[wasm_bindgen(js_name = setGasFluidProperties)]
pub fn set_gas_fluid_properties(&mut self, mu_g: f64, c_g: f64, rho_g: f64)

#[wasm_bindgen(js_name = setThreePhaseModeEnabled)]
pub fn set_three_phase_mode_enabled(&mut self, enabled: bool)

#[wasm_bindgen(js_name = setInjectedFluid)]
pub fn set_injected_fluid(&mut self, fluid: &str) -> Result<(), String>
// fluid: "water" | "gas"

#[wasm_bindgen(js_name = getSatGas)]
pub fn get_sat_gas(&self) -> Vec<f64>
```

3. Constructor: initialise `sat_gas` as `vec![0.0; n_cells]`, `three_phase_mode: false`.

4. Extend `getWellState` / `getRateHistory` to include gas production when in 3-phase mode.

5. `TimePointRates` (in `well.rs`): add `total_production_gas: f64` field.

---

#### `src/lib/ressim/src/step.rs`
**Current:** 733 lines. This is the largest change.

**Section A: Helper methods (~lines 109–149)**

Add gas mobility helpers:

```rust
fn gas_mobility(&self, id: usize) -> f64 {
    self.scal_3p.as_ref().map_or(0.0, |s| s.k_rg(self.sat_gas[id]) / self.mu_g)
}

fn get_gas_oil_capillary_pressure(&self, s_g: f64) -> f64 {
    self.pc_og.as_ref().map_or(0.0, |pc| pc.capillary_pressure_og(s_g, self.scal_3p.as_ref().unwrap()))
}

fn total_mobility_3p(&self, id: usize) -> f64 {
    // λ_w + λ_o + λ_g using Stone II k_ro
    let s = self.scal_3p.as_ref().unwrap();
    let sw = self.sat_water[id];
    let sg = self.sat_gas[id];
    (s.k_rw(sw) / self.pvt.mu_w)
      + (s.k_ro_stone2(sw, sg) / self.pvt.mu_o)
      + (s.k_rg(sg) / self.mu_g)
}
```

**Section B: Pressure Assembly (`calculate_fluxes`, ~lines 336–467)**

Add gas-phase branch inside the neighbor loop (runs only when `three_phase_mode = true`):

```rust
// After computing dphi_o and dphi_w, add:
let pc_og_i = self.get_gas_oil_capillary_pressure(self.sat_gas[id]);
let pc_og_j = self.get_gas_oil_capillary_pressure(self.sat_gas[nid]);
let grav_g = self.gravity_head_bar(depth_i, depth_j, self.rho_g);

// Gas potential: P_gas = P_oil − P_cog, so dphi_g = dphi_oil + d(P_cog)
let dphi_g = (p_i - p_j) + (pc_og_i - pc_og_j) - grav_g;

// Gas upwind mobility
let lam_g_up = if dphi_g >= 0.0 {
    self.gas_mobility(id)
} else {
    self.gas_mobility(nid)
};

let t_g = geom_t * lam_g_up;
t_total += t_g;  // Add to total transmissibility for pressure equation

// RHS gas contribution (explicit gravity + capillary)
b[id] -= t_g * (pc_og_i - pc_og_j - grav_g);
b[nid] += t_g * (pc_og_i - pc_og_j - grav_g);
```

Existing 2-phase accumulation, oil, and water terms are structurally unchanged; gas is additive.

The `c_t` accumulation term expands to:
```rust
let s_o = if three_phase_mode { 1.0 - sw - sg } else { 1.0 - sw };
let c_t = (c_o * s_o + c_w * sw + c_g * sg) + c_r;
// c_g is gas compressibility
```

**Section C: Saturation Transport (~lines 484–563)**

After computing `delta_water` (unchanged), add a second pass for gas:

```rust
if self.three_phase_mode {
    let mut delta_gas = vec![0.0f64; n_cells];

    // Interface gas fluxes (same structure as water loop)
    for (id, nid, geom_t) in &interfaces {
        let pc_og_i = ...;
        let pc_og_j = ...;
        let dphi_g = (p[id] - p[nid]) + (pc_og_i - pc_og_j) - grav_g_for_pair;
        let lam_g_up = upwind_gas_mobility(id, nid, dphi_g);
        let t_g = geom_t * lam_g_up;
        let gas_flux = t_g * dphi_g * dt_days;
        delta_gas[id] -= gas_flux;
        delta_gas[nid] += gas_flux;
    }

    // Well gas source terms
    for well in &self.wells {
        if well.injector && self.injected_fluid == InjectedFluid::Gas {
            delta_gas[well_id] -= q_m3_day * dt_days;  // Gas injection
        } else if !well.injector {
            let f_g = gas_mobility(id) / total_mobility_3p(id);
            delta_gas[well_id] -= q_m3_day * f_g * dt_days;
        }
    }

    // ... update sat_gas below
}
```

**Section D: CFL Check (~lines 565–615)**

Add gas saturation criterion:

```rust
if three_phase_mode {
    let max_sg_change = compute_max_sat_change(&delta_gas, &pore_volumes);
    let gas_factor = stability_factor(max_sg_change, max_sat_change_per_step);
    stable_dt_factor = stable_dt_factor.min(gas_factor);
}
```

**Section E: Saturation Update (~lines 626–732)**

Extend with gas update and three-way normalisation:

```rust
for id in 0..n_cells {
    let vp = pore_volume_m3(id);
    let ds_w = delta_water[id] / vp;

    if three_phase_mode {
        let ds_g = delta_gas[id] / vp;
        let s_w_new = (s_w_old + ds_w).clamp(s_wc, 1.0 - s_or - s_gc);
        let s_g_new = (s_g_old + ds_g).clamp(0.0, 1.0 - s_wc - s_gr);
        let s_o_new = (1.0 - s_w_new - s_g_new).clamp(0.0, 1.0);

        // Re-normalise if round-off causes sum ≠ 1
        let sum = s_w_new + s_o_new + s_g_new;
        sat_water[id] = s_w_new / sum;
        sat_oil[id]   = s_o_new / sum;
        sat_gas[id]   = s_g_new / sum;
    } else {
        // Original 2-phase update — UNCHANGED
        sat_water[id] = (s_w_old + ds_w).clamp(s_wc, 1.0 - s_or);
        sat_oil[id]   = 1.0 - sat_water[id];
    }
}
```

**Section F: `TimePointRates` recording**

When `three_phase_mode`, populate `total_production_gas` from well gas fractional flow.

---

### LAYER 2: TypeScript — Types & Pipeline

#### `src/lib/simulator-types.ts`

Add to `SimulatorCreatePayload` (~line 45):
```typescript
// Three-phase SCAL
s_gc?: number;          // gas critical saturation
s_gr?: number;          // gas residual saturation
n_g?: number;           // gas Corey exponent
k_rg_max?: number;      // max gas relative permeability
// Gas-oil capillary pressure
pcogEnabled?: boolean;
pcogPEntry?: number;
pcogLambda?: number;
// Gas fluid properties
mu_g?: number;
c_g?: number;
rho_g?: number;
// Mode flags
threePhaseModeEnabled?: boolean;
injectedFluid?: 'water' | 'gas';
```

Add to `GridState` (~line 103):
```typescript
sat_gas: Float64Array;   // gas saturation per cell (zeros when 2-phase)
```

Add to `TimePointRates` (wherever it is typed):
```typescript
total_production_gas?: number;
```

---

#### `src/lib/buildCreatePayload.ts`

Add to `buildCreatePayloadFromState` return (~line 113):
```typescript
// Three-phase SCAL
s_gc:    toClamped(state.s_gc,    0, 1,    0.05),
s_gr:    toClamped(state.s_gr,    0, 1,    0.05),
n_g:     toMin(state.n_g,         0.01,    1.5),
k_rg_max: toClamped(state.k_rg_max, 0.01, 1, 1.0),
// Gas-oil capillary
pcogEnabled: Boolean(state.pcogEnabled ?? false),
pcogPEntry:  toMin(state.pcogPEntry, 0, 0),
pcogLambda:  toMin(state.pcogLambda, 0, 2),
// Gas fluid properties
mu_g:  toMin(state.mu_g,  0.001, 0.02),
c_g:   toMin(state.c_g,   0,     1e-4),
rho_g: toMin(state.rho_g, 0.1,   10.0),
// Mode
threePhaseModeEnabled: Boolean(state.threePhaseModeEnabled ?? false),
injectedFluid: state.injectedFluid ?? 'gas',
```

---

#### `src/lib/validateInputs.ts`

Replace 2-phase saturation endpoint check (line 101) with mode-aware check:
```typescript
// In validateInputs, pass a flag or read from input
if (input.threePhaseModeEnabled) {
    if (!isFiniteNumber(input.s_gc) || ...) errors.s_gc = '...';
    if (!isFiniteNumber(input.s_gr) || ...) errors.s_gr = '...';
    if (!isFiniteNumber(input.n_g)  || numeric(input.n_g) <= 0) errors.n_g = '...';
    if (input.s_wc + input.s_or + (input.s_gc ?? 0) + (input.s_gr ?? 0) >= 1)
        errors.saturationEndpoints = 'S_wc + S_or + S_gc + S_gr must be < 1.';
    if (!isFiniteNumber(input.mu_g) || numeric(input.mu_g) <= 0) errors.mu_g = '...';
    if (!isFiniteNumber(input.c_g)  || numeric(input.c_g) < 0)  errors.c_g = '...';
} else {
    // Existing 2-phase check — UNCHANGED (line 101)
    if (input.s_wc + input.s_or >= 1) errors.saturationEndpoints = '...';
}
```

Add `s_gc`, `s_gr`, `n_g`, `k_rg_max`, `mu_g`, `c_g`, `rho_g`, `threePhaseModeEnabled`
to the `SimulationInputs` type.

---

#### `src/lib/stores/simulationStore.svelte.ts`

Add new `$state` variables (after existing SCAL vars, ~line 169):
```typescript
let s_gc    = $state(0.05);
let s_gr    = $state(0.05);
let n_g     = $state(1.5);
let k_rg_max = $state(1.0);
let pcogEnabled = $state(false);
let pcogPEntry  = $state(3.0);
let pcogLambda  = $state(2.0);
let mu_g    = $state(0.02);
let c_g     = $state(1e-4);
let rho_g   = $state(10.0);
let threePhaseModeEnabled = $state(false);
let injectedFluid = $state<'water' | 'gas'>('gas');
```

Expose all in the store's exposed bindings getter (the `params` object that feeds `ModePanelParameterBindings`).

---

#### `src/lib/ui/modePanelTypes.ts`

Add to `ModePanelParameterBindings` (~line 70):
```typescript
s_gc: number;
s_gr: number;
n_g: number;
k_rg_max: number;
pcogEnabled: boolean;
pcogPEntry: number;
pcogLambda: number;
mu_g: number;
c_g: number;
rho_g: number;
threePhaseModeEnabled: boolean;
injectedFluid: 'water' | 'gas';
```

---

#### `src/lib/workers/sim.worker.ts`

After the existing `setRelPermProps` call (~line 128), add defensive calls:
```typescript
if (payload.threePhaseModeEnabled) {
    call(simulator, 'setThreePhaseModeEnabled', true);

    call(simulator, 'setThreePhaseRelPermProps',
        payload.s_wc, payload.s_or,
        payload.s_gc ?? 0.05, payload.s_gr ?? 0.05,
        payload.n_w, payload.n_o, payload.n_g ?? 1.5,
        payload.k_rw_max ?? 1.0, payload.k_ro_max ?? 1.0, payload.k_rg_max ?? 1.0
    );

    call(simulator, 'setGasFluidProperties',
        payload.mu_g ?? 0.02, payload.c_g ?? 1e-4, payload.rho_g ?? 10.0
    );

    if (payload.pcogEnabled) {
        call(simulator, 'setGasOilCapillaryParams',
            payload.pcogPEntry ?? 0, payload.pcogLambda ?? 2
        );
    }

    call(simulator, 'setInjectedFluid', payload.injectedFluid ?? 'gas');
}
```

Where `call` is the existing defensive pattern:
```typescript
const call = (sim: any, name: string, ...args: any[]) => {
    if (typeof sim[name] === 'function') sim[name](...args);
};
```

Also update the snapshot/state handler to include `sat_gas`:
```typescript
// In the state message handler, after getSatOil:
const sat_gas = simulator.getSatGas?.() ?? new Float64Array(n_cells);
```

---

### LAYER 3: Catalog & Scenarios

#### `src/lib/catalog/caseCatalog.ts`

1. Extend `CaseMode`:
```typescript
export type CaseMode = 'dep' | 'wf' | 'sim' | '3p';
```

2. Add '3p' entry to the catalog with a gas-flood scenario as base case.

3. Add `getModeDimensions` support for '3p' mode (returns relevant dimension toggles — geometry, well, timestep; no fluid system toggle needed since it's always 3-phase).

---

#### `src/lib/catalog/scenarios.ts`

Add a new `scenarioClass: '3phase'` (or reuse `'depletion'` for solution-gas drive).

New base scenarios — two representative cases:

```typescript
// Scenario A: Solution-gas drive (depletion with gas coming out of solution)
// S_wc=0.2, no free gas initially, pressure drops below bubble point during run
{
    key: '3p_solution_gas_drive',
    label: 'Solution Gas Drive',
    scenarioClass: '3phase',
    description: 'Pressure depletion releasing dissolved gas.',
    params: {
        nx: 20, ny: 1, nz: 1,
        cellDx: 50, cellDy: 50, cellDz: 10,
        initialPressure: 250, initialSaturation: 0.2,
        // Initial gas saturation = 0 (gas comes from depletion)
        initialGasSaturation: 0.05,
        threePhaseModeEnabled: true,
        injectorEnabled: false,
        injectedFluid: 'gas',
        s_wc: 0.2, s_or: 0.15,
        s_gc: 0.05, s_gr: 0.05,
        n_w: 2, n_o: 2, n_g: 1.5,
        k_rw_max: 0.4, k_ro_max: 1.0, k_rg_max: 0.8,
        mu_w: 0.5, mu_o: 2.0, mu_g: 0.02,
        c_w: 3e-6, c_o: 1e-5, c_g: 1e-4,
        rho_w: 1000, rho_o: 800, rho_g: 10,
        capillaryEnabled: false,
        pcogEnabled: false,
        delta_t_days: 10,
        ...
    }
},

// Scenario B: Gas injection / WAG
{
    key: '3p_gas_injection',
    label: 'Gas Injection',
    scenarioClass: '3phase',
    description: 'Gas injector displacing oil with concurrent water.',
    params: {
        ...
        threePhaseModeEnabled: true,
        injectorEnabled: true,
        injectedFluid: 'gas',
        injectorBhp: 350, producerBhp: 100,
        ...
    }
}
```

Add `'3phase'` to the `ScenarioClass` union type.

---

#### `src/lib/ui/modePanelSections.ts`

Add `'gasfluid'` section key to `ModePanelSectionKey` union:
```typescript
| 'gasfluid'    // gas fluid properties and 3-phase SCAL
```

The `MODE_PANEL_SECTIONS` constant stays mode-independent, but `getModePanelSections()` (now parameter-free) could be extended later to return mode-specific sections if needed.

---

### LAYER 4: UI Components

#### New: `src/lib/ui/sections/GasFluidSection.svelte`

A new collapsible section shown only in 3-phase mode (when `section.key === 'gasfluid'` in `ScenarioSectionsPanel`).

Panels:
1. **Gas Saturation Endpoints** (table row parallel to water/oil in SCAL):
   - s_gc (critical gas saturation)
   - s_gr (residual gas saturation)
   - n_g (Corey exponent)
   - k_rg_max (max gas rel perm)

2. **Gas Fluid Properties** (grid layout, similar to ReservoirSection):
   - mu_g (gas viscosity)
   - c_g (gas compressibility)
   - rho_g (gas density)

3. **Injection Control**:
   - `injectedFluid` toggle: "Water" / "Gas"

4. **Gas-Oil Capillary Pressure** (optional, behind `pcogEnabled` checkbox):
   - pcogPEntry
   - pcogLambda

```svelte
<!-- src/lib/ui/sections/GasFluidSection.svelte -->
<script lang="ts">
  import Collapsible from "../controls/Collapsible.svelte";
  import Input from "../controls/Input.svelte";
  import ValidatedInput from "../controls/ValidatedInput.svelte";
  import PanelTable from "../controls/PanelTable.svelte";

  let {
    s_gc = $bindable(0.05), s_gr = $bindable(0.05),
    n_g = $bindable(1.5), k_rg_max = $bindable(1.0),
    mu_g = $bindable(0.02), c_g = $bindable(1e-4), rho_g = $bindable(10.0),
    pcogEnabled = $bindable(false), pcogPEntry = $bindable(3.0), pcogLambda = $bindable(2.0),
    injectedFluid = $bindable<'water' | 'gas'>('gas'),
    fieldErrors = {},
  } = $props();

  const groupSummary = $derived(
    `S_gc=${s_gc.toFixed(2)}, S_gr=${s_gr.toFixed(2)}, n_g=${n_g.toFixed(1)}, μ_g=${mu_g.toFixed(3)} cP`
  );
  const hasError = $derived(!!fieldErrors.s_gc || !!fieldErrors.s_gr || !!fieldErrors.n_g || !!fieldErrors.mu_g);
</script>

<Collapsible title="Gas Phase" {hasError}>
  <!-- Gas SCAL row (table alongside water/oil) -->
  <!-- Gas fluid properties -->
  <!-- Injection toggle -->
  <!-- Gas-oil Pc (conditional) -->
</Collapsible>
```

---

#### Modified: `src/lib/ui/sections/RelativeCapillarySection.svelte`

The existing section stays entirely 2-phase. No changes to existing fields.

Add only: a gas row to the relperm table **when in 3-phase mode** — controlled by a new boolean prop `threePhaseMode?: boolean`.

If `threePhaseMode === true`, the table renders a third row "Gas" with fields bound to `s_gc`, `s_gr`, `n_g`, `k_rg_max` (all passed as new optional bindable props).

This keeps the section backward-compatible: when `threePhaseMode` is not passed (defaults to `false`), it renders exactly as today.

---

#### Modified: `src/lib/ui/sections/ScenarioSectionsPanel.svelte`

Add `gasfluid` section case:
```svelte
{:else if section.key === "gasfluid"}
    <GasFluidSection
        bind:s_gc={params.s_gc}
        bind:s_gr={params.s_gr}
        bind:n_g={params.n_g}
        bind:k_rg_max={params.k_rg_max}
        bind:mu_g={params.mu_g}
        bind:c_g={params.c_g}
        bind:rho_g={params.rho_g}
        bind:pcogEnabled={params.pcogEnabled}
        bind:pcogPEntry={params.pcogPEntry}
        bind:pcogLambda={params.pcogLambda}
        bind:injectedFluid={params.injectedFluid}
        fieldErrors={validationErrors}
    />
```

Also pass `threePhaseMode={activeMode === '3p'}` prop to `RelativeCapillarySection` so it shows the gas row.

---

#### Modified: `src/lib/ui/modes/ScenarioPicker.svelte`

Add a third group for 3-phase scenarios:
```svelte
<!-- Three-phase group (only shown when activeMode === '3p') -->
{#if activeMode === '3p'}
<div class="flex flex-wrap gap-1.5 rounded border border-border/50 p-1.5">
    {#each SCENARIOS.filter((s) => s.scenarioClass === '3phase') as scenario}
        <Button ...>{scenario.label}</Button>
    {/each}
</div>
{/if}
```

---

#### Modified: App-level mode picker (ModePanel or equivalent)

Add '3p' as a selectable mode in whatever tab/button group switches CaseMode. This is likely in `src/lib/ui/modes/ModePanel.svelte` or similar.

---

### LAYER 5: Chart & Visualization

#### `src/lib/charts/` (minimal changes)

Rate charts already display oil/water production over time. For 3-phase:
1. Expose `total_production_gas` from `TimePointRates` and add a gas production series.
2. Add `avg_gas_saturation` to `TimePointRates` and plot it alongside `avg_water_saturation`.

The chart system is driven by `CHART_PRESETS` in `scenarios.ts` — add a `'3p_depletion'` and `'3p_injection'` preset that selects the appropriate curves.

No new chart types needed — existing multi-series line charts handle this.

---

### LAYER 6: Tests

#### New test files:

- `src/lib/ressim/relperm3p.test.ts` — unit tests for Stone II:
  - At S_g=0, Stone II reduces to 2-phase k_ro
  - k_ro(S_wc, S_gr) = 0 (both residuals)
  - k_ro(S_wc, 0) = k_ro_max

- `src/lib/validateInputs.test.ts` (extend existing) — 3-phase saturation constraint check.

- Update `modePanelComposition.test.ts` to include `'gasfluid'` section check.

---

## Implementation Order (Recommended Sequence)

```
Step 1: Rust — relperm.rs (add RockFluidPropsThreePhase + Stone II)
Step 2: Rust — capillary.rs (add GasOilCapillaryPressure)
Step 3: Rust — lib.rs (add sat_gas, new WASM methods, threephasemode flag)
Step 4: Rust — step.rs (pressure assembly + gas transport + CFL + sat update)
Step 5: Build WASM and verify it compiles: `wasm-pack build`
Step 6: simulator-types.ts (extend payload and GridState types)
Step 7: buildCreatePayload.ts (add normalization for new fields)
Step 8: validateInputs.ts (extend validation)
Step 9: simulationStore.svelte.ts (add $state vars + expose in bindings)
Step 10: modePanelTypes.ts (extend ModePanelParameterBindings)
Step 11: sim.worker.ts (add setThreePhaseRelPermProps etc. calls)
Step 12: caseCatalog.ts (add '3p' CaseMode)
Step 13: scenarios.ts (add 3-phase scenario definitions)
Step 14: modePanelSections.ts (add 'gasfluid' section key)
Step 15: GasFluidSection.svelte (new component)
Step 16: RelativeCapillarySection.svelte (add optional gas row)
Step 17: ScenarioSectionsPanel.svelte (add gasfluid case)
Step 18: ScenarioPicker.svelte (add 3-phase group)
Step 19: ModePanel.svelte (add '3p' mode button)
Step 20: charts — add gas series to rate chart
Step 21: Write tests
```

---

## Key Unchanged Files

These files are **not touched**:
- `src/lib/analytical/fractionalFlow.ts` — 2-phase waterflood, unchanged
- `src/lib/analytical/depletionAnalytical.ts` — 2-phase depletion, unchanged
- All existing scenarios (`wf`, `dep`)
- Existing `RockFluidProps` struct in Rust (2-phase)
- Existing `CapillaryPressure` struct in Rust (P_cow)
- Existing `setRelPermProps` WASM method

---

## New Parameter Reference

| Parameter | Rust field | TS field | Default | Units |
|-----------|-----------|---------|---------|-------|
| Critical gas sat. | s_gc | s_gc | 0.05 | fraction |
| Residual gas sat. | s_gr | s_gr | 0.05 | fraction |
| Gas Corey exponent | n_g | n_g | 1.5 | — |
| Max gas rel perm | k_rg_max | k_rg_max | 1.0 | fraction |
| Gas viscosity | mu_g | mu_g | 0.02 | cP |
| Gas compressibility | c_g | c_g | 1e-4 | 1/bar |
| Gas density | rho_g | rho_g | 10.0 | kg/m³ |
| Gas-oil Pc entry | pc_og.p_entry | pcogPEntry | 3.0 | bar |
| Gas-oil Pc lambda | pc_og.lambda | pcogLambda | 2.0 | — |
| Injected fluid | injected_fluid | injectedFluid | "gas" | — |
| Three-phase flag | three_phase_mode | threePhaseModeEnabled | false | — |

---

## Estimated Scope

| Layer | Files Changed / Added | Notes |
|-------|-----------------------|-------|
| Rust | relperm.rs, capillary.rs, lib.rs, step.rs | ~400 new lines |
| TS types | simulator-types.ts, modePanelTypes.ts | ~30 new lines |
| TS pipeline | buildCreatePayload.ts, validateInputs.ts, sim.worker.ts | ~60 new lines |
| Store | simulationStore.svelte.ts | ~30 new state vars + expose |
| Catalog | caseCatalog.ts, scenarios.ts, modePanelSections.ts | ~100 new lines |
| UI | GasFluidSection.svelte (new), + 4 modified | ~200 new lines |
| Charts | 1-2 chart files | ~20 new lines |
| Tests | 2 new + extend 2 | ~100 new lines |

Existing 2-phase code path: **zero changes to runtime behavior**, only additive.
