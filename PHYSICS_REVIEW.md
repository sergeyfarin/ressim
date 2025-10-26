# Reservoir Simulator Physics Review & Recommendations

## Executive Summary
The IMPES (Implicit Pressure, Explicit Saturation) simulator implementation is **structurally sound** but has several physics inconsistencies, unit conversion issues, and areas for optimization. Below is a detailed analysis with recommendations.

---

## 1. CRITICAL ISSUES

### 1.1 **Capillary Pressure Not Implemented** ⚠️ HIGH PRIORITY
**Current State:** Capillary pressure is completely absent.

**Physics Impact:** 
- In water-oil systems, capillary pressure acts as a pressure difference across the oil-water interface
- Neglecting it means pressure fields are decoupled by phase (unrealistic)
- Capillary pressure dominates in fine-grained rocks and determines oil/water distribution

**Recommended Fix:**
```rust
// Add to FluidProperties or new struct
pub struct CapillaryProperties {
    pub p_c_entry: f64,      // Entry pressure (Pa)
    pub lambda: f64,         // Pore-size distribution index (0.5-3.0)
}

// Add capillary pressure correlation (e.g., Brooks & Corey)
fn capillary_pressure(&self, s_w: f64) -> f64 {
    if s_w <= self.s_wc {
        return self.p_c_entry;
    }
    let s_eff = (s_w - self.s_wc) / (1.0 - self.s_wc - self.s_or);
    self.p_c_entry * s_eff.powf(-1.0 / self.lambda)
}
```

**Integration:** Add `p_c = capillary_pressure(s_w)` to saturation update:
```rust
// Current (WRONG):
let p_oil = p_water;

// Should be:
let p_oil = p_water + capillary_pressure(s_w);
```

---

### 1.2 **Unit System Inconsistency** ⚠️ CRITICAL
**Current Issues:**

1. **Transmissibility scaling factor `0.001127`** - This is a magic number without clear justification
   - Appears to assume some implicit unit conversion
   - Standard oil-field units: `T = 0.001127 * k * A * B / (mu * phi * c_t * L)` 
   - But here it's applied ad-hoc to `k_h * area / dist * mob`

2. **Time units mixing:**
   - Input: `delta_t_days` (days)
   - Accumulation: `(vp_bbl * c_t) / dt_days`
   - Transmissibility: implicitly assumes different time unit

3. **Pore volume conversion:**
   - m³ → bbl (multiply by 6.28981) ✓ Correct
   - BUT: compressibility `c_o = 1e-5, c_w = 3e-6` assumed as `1/psi`
   - Must convert: `c_t [1/psi] → c_t [1/Pa]` or adjust accumulation term

**Recommended Fix:**
```rust
// Define explicit unit system at module level
const METERS_TO_FEET: f64 = 3.28084;
const BARREL_PER_M3: f64 = 6.28981;
const PSI_TO_PA: f64 = 6894.76;
const MD_TO_M2: f64 = 9.8692e-16;  // Darcy to m²

// In step(), make conversions explicit:
pub fn step(&mut self, delta_t_days: f64) {
    let dt_seconds = delta_t_days * 86400.0;
    let dt_hours = delta_t_days * 24.0;
    
    // Choose ONE unit system and stick with it:
    // Option A: SI (Pa, m³, s)
    // Option B: Oil Field (psi, bbl, days)
}
```

---

### 1.3 **Relative Permeability Issues**
**Current Implementation:**
```rust
pub fn k_rw(&self, s_w: f64) -> f64 {
    let s_eff = ((s_w - self.s_wc) / (1.0 - self.s_wc - self.s_or)).clamp(0.0, 1.0);
    s_eff.powf(self.n_w)
}
```

**Issues:**
1. ✓ Corey-type correlation is correct
2. ✓ Normalization by moveable saturation is correct
3. ✓ Clamping prevents NaN is good
4. ✗ **Missing endpoint verification:**
   - When `s_w = 1 - s_or` (maximum oil): `k_rw` should equal `k_rw^max`, not 1.0
   - When `s_w = s_wc` (minimum water): `k_rw` should be 0

**Recommendation - Add endpoint verification:**
```rust
// Add to RockFluidProps
pub fn krw_max: f64,  // k_rw at s_w = 1 - s_or
pub fn kro_max: f64,  // k_ro at s_w = s_wc

pub fn k_rw(&self, s_w: f64) -> f64 {
    let s_eff = ((s_w - self.s_wc) / (1.0 - self.s_wc - self.s_or)).clamp(0.0, 1.0);
    self.krw_max * s_eff.powf(self.n_w)  // Multiply by endpoint value
}

pub fn k_ro(&self, s_w: f64) -> f64 {
    let s_eff = ((1.0 - s_w - self.s_or) / (1.0 - self.s_wc - self.s_or)).clamp(0.0, 1.0);
    self.kro_max * s_eff.powf(self.n_o)
}
```

---

## 2. LOGIC ERRORS

### 2.1 **Well Productivity Index (PI) Units Mismatch**
**Current Code:**
```rust
pub fn add_well(&mut self, i: usize, j: usize, k: usize, bhp: f64, pi: f64, injector: bool)
```

**Issues:**
1. `bhp` units unknown (assumed Pa? psi?)
2. `pi` units unknown
3. No validation: `PI < 0` allowed (unphysical)
4. Sign convention ambiguous for injectors

**Recommended Fix:**
```rust
impl Well {
    pub fn validate(&self) -> Result<(), String> {
        if self.productivity_index < 0.0 {
            return Err("PI must be non-negative".to_string());
        }
        if self.bhp.is_nan() || self.bhp.is_infinite() {
            return Err("BHP must be finite".to_string());
        }
        Ok(())
    }
    
    // Compute actual rate more carefully
    pub fn compute_rate(&self, block_pressure: f64, fw: f64, injector: bool) -> f64 {
        // q = PI * (p_block - BHP) [bbl/day]
        // injector: true → negative q means injection (adds fluid)
        // producer: true → positive q means production (removes fluid)
        let q = self.productivity_index * (block_pressure - self.bhp);
        
        if injector && q < 0.0 {
            // Injection working as expected
            q * fw
        } else if !injector && q > 0.0 {
            // Production working as expected
            q * fw
        } else {
            // Well not flowing or flowing opposite intended direction
            0.0  // or handle as reversal?
        }
    }
}
```

---

### 2.2 **Well Rate Sign Convention Inconsistency**
**Current Code:**
```rust
let q = w.productivity_index * (p_new[id] - w.bhp);
let fw = self.frac_flow_water(&self.grid_cells[id]);
let water_q = q * fw;
net_water_bbl[id] -= water_q * dt_days;  // "- " for production
```

**Problem:** 
- For producers: `p_new[id] > BHP` → `q > 0` → removal is correct (-)
- For injectors: `p_new[id] < BHP` → `q < 0` → BUT code adds fluid anyway (-negative = +)
- **This might actually work by accident**, but it's confusing

**Clearer Approach:**
```rust
// Explicit producer/injector handling
for w in &self.wells {
    let id = self.idx(w.i, w.j, w.k);
    let dP = p_new[id] - w.bhp;
    
    let q_total = if w.injector {
        // Injector: negative BHP forces fluid in
        -w.productivity_index * dP.abs()  // Always pump in
    } else {
        // Producer: positive dP produces fluid
        w.productivity_index * dP  // Positive dP → production
    };
    
    let fw = self.frac_flow_water(&self.grid_cells[id]);
    let water_q = q_total * fw;
    
    net_water_bbl[id] -= water_q * dt_days;
}
```

---

### 2.3 **Saturation Clamping Without Material Balance**
**Current Code:**
```rust
let sw_new = (self.grid_cells[idx].sat_water + delta_sw).clamp(0.0, 1.0);
let so_new = (1.0 - sw_new).clamp(0.0, 1.0);
```

**Issue:**
- Clamping to `[0, 1]` **violates material balance**
- If `delta_sw > 1.0`, we're discarding oil mass
- Should maintain `S_w + S_o + S_g = 1.0` (in 2-phase: `S_w + S_o = 1.0`)

**Better Approach:**
```rust
let sw_new = (self.grid_cells[idx].sat_water + delta_sw).clamp(0.0, 1.0);
let so_new = 1.0 - sw_new;  // Always true: no second clamp

// Optional: Log when clamping occurs
if delta_sw > 0.0 && sw_new > 1.0 - f64::EPSILON {
    eprintln!("Warning: Cell {} hit Sw=1.0 (oil trapped)", idx);
}
```

---

## 3. PHYSICS SIMPLIFICATIONS (Acceptable but Document)

### 3.1 **Gravity Not Included**
**Current:** Gravity term `ρ*g*Δz` in pressure equation is omitted.

**When It Matters:**
- Thick reservoirs (Δz > 100 m)
- Active aquifer support (bottom-up pressure)
- Capillary-gravity transition zones

**Impact on Demo:** Small (test case is thin layer), but should add option:
```rust
pub fn step_with_gravity(&mut self, delta_t_days: f64, use_gravity: bool) {
    // ... normal setup ...
    let rho_w = 62.4;  // lb/ft³ (or 1000 kg/m³)
    let rho_o = 52.0;  // lb/ft³ typical
    let g = 32.174;    // ft/s²
    
    if use_gravity {
        // Add hydrostatic terms to transmissibility or flux calc
    }
}
```

### 3.2 **Compressibility Simplified**
**Current:** `c_t = c_o + c_w` (linear combination)

**Reality:** Should be:
```
c_t = φ(S_o * c_o + S_w * c_w) + c_f
```
where `c_f` is rock compressibility and the saturation weighting is correct.

**Current implementation is actually close enough** for most cases, but update to:
```rust
fn total_compressibility(&self, cell: &GridCell, c_f: f64) -> f64 {
    let c_fluid = cell.sat_oil * self.pvt.c_o + cell.sat_water * self.pvt.c_w;
    cell.porosity * c_fluid + c_f  // Rock compressibility ~1e-6 1/Pa
}
```

---

### 3.3 **Hysteresis Not Included**
**Current:** Rel perm curves are single-valued (no scanning curves).

**Acceptable** for initial model, but note:
- Water advancing vs. retreating has different `k_rw`
- Oil trapping reduces `k_ro` post-imbibition
- This affects production predictions but not pressure solution

---

## 4. CODE QUALITY & OPTIMIZATION

### 4.1 **Neighbor Loop Can Be Simplified**
**Current:**
```rust
let mut neighbors: Vec<(usize, char)> = Vec::new();
if i > 0 { neighbors.push((self.idx(i-1,j,k), 'x')); }
if i < self.nx-1 { neighbors.push((self.idx(i+1,j,k), 'x')); }
// ... 4 more if statements
```

**Better:**
```rust
let deltas = [
    ((-1, 0, 0), 'x'),
    ((1, 0, 0), 'x'),
    ((0, -1, 0), 'y'),
    ((0, 1, 0), 'y'),
    ((0, 0, -1), 'z'),
    ((0, 0, 1), 'z'),
];

for (di, dj, dk), dim in deltas {
    let ni = (i as i32 + di) as usize;
    let nj = (j as i32 + dj) as usize;
    let nk = (k as i32 + dk) as usize;
    
    if ni < self.nx && nj < self.ny && nk < self.nz {
        neighbors.push((self.idx(ni, nj, nk), dim));
    }
}
```

### 4.2 **Remove Redundant Conversions**
**Current:**
```rust
let mut x0 = DVector::<f64>::zeros(n_cells);
for i in 0..n_cells { x0[i] = self.grid_cells[i].pressure; }
```

**Better:**
```rust
let x0: DVector<f64> = DVector::from_fn(n_cells, |i, _| self.grid_cells[i].pressure);
```

---

## 5. RECOMMENDED FIXES (Priority Order)

### Priority 1: Physics Correctness
1. **Add capillary pressure** - High impact on saturation distribution
2. **Fix unit system** - Prevent accumulation errors
3. **Verify relative permeability endpoints** - Ensure proper production/injection

### Priority 2: Robustness
1. **Well validation** - Catch invalid inputs
2. **Material balance check** - Detect mass conservation errors
3. **Saturation bounds** - Log when bounds hit

### Priority 3: Clarity
1. **Document unit system** at top of lib.rs
2. **Add comments** to magic numbers (0.001127, etc.)
3. **Sign convention** for well rates

### Priority 4: Optimization
1. Simplify neighbor loops
2. Reduce vector allocations in PCG solver
3. Add time-step control (adaptive dt for stability)

---

## 6. SUGGESTED MINIMAL IMPLEMENTATION (Next Step)

```rust
// src/lib.rs - minimal capillary pressure addition

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CapillaryProps {
    pub p_entry: f64,  // Entry pressure (Pa)
    pub lambda: f64,   // Pore-size distribution index
}

impl CapillaryProps {
    fn brooks_corey(&self, s_w: f64, s_wc: f64, s_or: f64) -> f64 {
        if s_w <= s_wc { return self.p_entry; }
        let s_e = (s_w - s_wc) / (1.0 - s_wc - s_or);
        self.p_entry * s_e.powf(-1.0 / self.lambda)
    }
}

// In ReservoirSimulator, add:
pub capillary: CapillaryProps,

// In step(), when updating saturation:
let p_c = self.capillary.brooks_corey(sw_new, self.scal.s_wc, self.scal.s_or);
self.grid_cells[idx].pressure_oil = p_c + self.grid_cells[idx].pressure_water;
```

---

## 7. SUMMARY TABLE

| Component | Status | Priority | Action |
|-----------|--------|----------|--------|
| Capillary Pressure | ❌ Missing | P1 | Implement Brooks-Corey |
| Units System | ⚠️ Inconsistent | P1 | Define & unify |
| Relative Permeability | ✓ Correct (mostly) | P3 | Add endpoints scaling |
| Well PI Sign | ✓ Works (confusing) | P2 | Clarify logic |
| Gravity | ⚠️ Omitted | P3 | Optional feature |
| Saturation Clamping | ⚠️ Discards mass | P2 | Log warnings |
| Code Style | ✓ Clean | P4 | Minor cleanup |

---

## Conclusion

**The simulator is a solid foundation** but needs physics enhancements (especially capillary pressure) for accurate water-oil interaction modeling. The current unit system works but is fragile. Start with **Priority 1** fixes, then move to robustness improvements.

For a demonstration tool, current implementation is **acceptable**. For engineering predictions, **add capillary pressure minimum**.
