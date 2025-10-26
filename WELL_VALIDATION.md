# P2: Well Parameter Validation - Prevent NaN/Inf Inputs

**Status:** ✅ COMPLETE | **Lines of Code:** ~50 | **Compilation:** No errors

---

## Executive Summary

Well parameters are now validated at two levels:
1. **Compile-time:** `add_well()` returns `Result<(), String>` - validation fails before well is added
2. **Runtime:** Defensive checks in pressure equation and saturation loops ensure NaN/Inf never propagates

This prevents silent failures and NaN/Inf values that can corrupt the simulation.

---

## Problem Statement

### Issue 1: No Input Validation on `add_well()`
**Before:**
```rust
pub fn add_well(&mut self, i: usize, j: usize, k: usize, bhp: f64, pi: f64, injector: bool) {
    self.wells.push(Well { i, j, k, bhp, productivity_index: pi, injector });
}
// Accepts ANY values - including NaN, Inf, negative PI, out-of-bounds indices
```

**After:**
```rust
pub fn add_well(...) -> Result<(), String> {
    let well = Well { i, j, k, bhp, productivity_index: pi, injector };
    well.validate(self.nx, self.ny, self.nz)?;  // Validate before adding
    self.wells.push(well);
    Ok(())
}
```

### Issue 2: No Bounds Checking
**Before:** Well indices (i, j, k) could be anywhere - crashes if >= grid size

**After:** Validation ensures `i < nx`, `j < ny`, `k < nz`

### Issue 3: No Sanity Checks on BHP/PI
**Before:**
- `bhp = NaN` would corrupt pressure equation
- `pi = -999.0` (negative) violates physics
- `pi = Inf` would create infinite mobility

**After:**
- `bhp` must be finite and in reasonable range [-100, 2000] bar
- `pi` must be non-negative and finite

### Issue 4: Silent NaN/Inf Propagation
**Before:** If somehow a bad value got through, it would silently propagate through calculations

**After:** Runtime defensive checks catch any stragglers and skip malformed wells

---

## Implementation Details

### Level 1: Well::validate() Method

**Location:** `lib.rs` lines 87-120

```rust
impl Well {
    /// Validate well parameters to prevent NaN/Inf and unphysical values
    /// Returns Ok(()) if parameters are valid, Err(message) otherwise
    pub fn validate(&self, nx: usize, ny: usize, nz: usize) -> Result<(), String> {
        // 1. Grid bounds checking
        if self.i >= nx {
            return Err(format!("Well index i={} out of bounds (nx={})", self.i, nx));
        }
        if self.j >= ny {
            return Err(format!("Well index j={} out of bounds (ny={})", self.j, ny));
        }
        if self.k >= nz {
            return Err(format!("Well index k={} out of bounds (nz={})", self.k, nz));
        }
        
        // 2. Finiteness check for BHP
        if !self.bhp.is_finite() {
            return Err(format!("BHP must be finite, got: {}", self.bhp));
        }
        
        // 3. Non-negativity check for PI
        if self.productivity_index < 0.0 {
            return Err(format!(
                "Productivity index must be non-negative, got: {}",
                self.productivity_index
            ));
        }
        
        // 4. Finiteness check for PI
        if !self.productivity_index.is_finite() {
            return Err(format!(
                "Productivity index must be finite, got: {}",
                self.productivity_index
            ));
        }
        
        // 5. Range check for BHP (sanity)
        // Typical range: [-100 vacuum, +2000 bar]
        if self.bhp < -100.0 || self.bhp > 2000.0 {
            return Err(format!(
                "BHP out of reasonable range [-100, 2000] bar, got: {}",
                self.bhp
            ));
        }
        
        Ok(())
    }
}
```

**Validation Checks:**

| Check | Why | Enforces |
|-------|-----|----------|
| `i < nx` | Prevent index out of bounds | Physical grid bounds |
| `j < ny` | Prevent array access crashes | Physical grid bounds |
| `k < nz` | Prevent array access crashes | Physical grid bounds |
| `bhp.is_finite()` | Prevent NaN/Inf in pressure | Finite simulation |
| `pi < 0.0` → error | Productivity index must be >= 0 | Physical well flow |
| `pi.is_finite()` | Prevent NaN/Inf in flow | Finite calculations |
| `bhp in [-100, 2000]` | Prevent unrealistic pressures | Engineering sense check |

---

### Level 2: Updated add_well() Method

**Location:** `lib.rs` lines 273-293

**Before:**
```rust
pub fn add_well(&mut self, i: usize, j: usize, k: usize, bhp: f64, pi: f64, injector: bool) {
    self.wells.push(Well { i, j, k, bhp, productivity_index: pi, injector });
}
```

**After:**
```rust
/// Add a well to the simulator
/// Parameters in oil-field units:
/// - i, j, k: grid cell indices (must be within grid bounds)
/// - bhp: bottom-hole pressure [bar] (must be finite, typical: -100 to 2000 bar)
/// - pi: productivity index [m³/day/bar] (must be non-negative and finite)
/// - injector: true for injector (injects fluid), false for producer (extracts fluid)
/// 
/// Returns Ok(()) on success, or Err(message) if parameters are invalid.
pub fn add_well(&mut self, i: usize, j: usize, k: usize, bhp: f64, pi: f64, injector: bool) -> Result<(), String> {
    let well = Well { i, j, k, bhp, productivity_index: pi, injector };
    
    // Validate well parameters
    well.validate(self.nx, self.ny, self.nz)?;
    
    self.wells.push(well);
    Ok(())
}
```

**Key Features:**
- Returns `Result<(), String>` for error reporting
- Validates before pushing to wells vector (fail-fast)
- Updated docstring clarifies units (bar, m³/day/bar)
- Lists all validation checks in comments

---

### Level 3: Defensive Runtime Checks

**Location 1: Pressure Equation Loop** (lines 397-407)

```rust
// well implicit coupling: add PI to diagonal and PI*BHP to RHS
for w in &self.wells {
    if w.i == i && w.j == j && w.k == k {
        // Defensive checks: well should be validated on add_well, but check at runtime too
        if w.productivity_index.is_finite() && w.bhp.is_finite() {
            diag += w.productivity_index;
            b_rhs[id] += w.productivity_index * w.bhp;
        }
        // Skip malformed well parameters
    }
}
```

**Location 2: Saturation Update Loop** (lines 482-501)

```rust
// Add well explicit contributions using solved pressure
for w in &self.wells {
    let id = self.idx(w.i, w.j, w.k);
    
    // Defensive check: ensure well parameters are finite
    if w.productivity_index.is_finite() && w.bhp.is_finite() && p_new[id].is_finite() {
        let q_m3_day = w.productivity_index * (p_new[id] - w.bhp);
        
        if q_m3_day.is_finite() {
            let fw = self.frac_flow_water(&self.grid_cells[id]);
            let water_q_m3_day = q_m3_day * fw;
            delta_water_m3[id] -= water_q_m3_day * dt_days;
        }
    }
    // Skip malformed well parameters
}
```

**Purpose:** Defense-in-depth - even if validation somehow fails, runtime checks prevent NaN/Inf propagation

---

## Unit System Integration

All well parameters now use **oil-field units** consistently:

| Parameter | Unit | Comment |
|-----------|------|---------|
| `i, j, k` | dimensionless | Grid indices [0, nx-1] × [0, ny-1] × [0, nz-1] |
| `bhp` | bar | Bottom-hole pressure, typical range: -100 to 2000 bar |
| `pi` | m³/day/bar | Productivity index, must be ≥ 0 |
| `injector` | bool | true = injector, false = producer |

**Conversion Notes:**
- Well rate: `q [m³/day] = pi [m³/day/bar] × (p_block [bar] - bhp [bar])`
- For producers: `p_block > bhp` → positive rate (production)
- For injectors: Set `injector=true` and use appropriate BHP

---

## Validation Guarantees

After calling `add_well()`:

| Guarantee | Enforcement |
|-----------|-------------|
| All well indices are in bounds | `validate()` checks i < nx, j < ny, k < nz |
| BHP is finite (not NaN/Inf) | `validate()` calls `bhp.is_finite()` |
| PI is non-negative | `validate()` checks `pi >= 0.0` |
| PI is finite (not NaN/Inf) | `validate()` calls `pi.is_finite()` |
| BHP is physically reasonable | `validate()` checks `bhp in [-100, 2000] bar` |
| Pressure equation is safe | Runtime checks in pressure loop |
| Saturation update is safe | Runtime checks in saturation loop |

---

## Error Handling Examples

### Example 1: Out-of-bounds index
```rust
// Grid is 10×10×5
sim.add_well(15, 5, 2, 300.0, 1.0, false)?;
// Error: "Well index i=15 out of bounds (nx=10)"
```

### Example 2: NaN in BHP
```rust
sim.add_well(5, 5, 2, f64::NAN, 1.0, false)?;
// Error: "BHP must be finite, got: NaN"
```

### Example 3: Negative PI
```rust
sim.add_well(5, 5, 2, 300.0, -1.5, false)?;
// Error: "Productivity index must be non-negative, got: -1.5"
```

### Example 4: BHP out of reasonable range
```rust
sim.add_well(5, 5, 2, 50000.0, 1.0, false)?;
// Error: "BHP out of reasonable range [-100, 2000] bar, got: 50000"
```

### Example 5: Valid well (success)
```rust
sim.add_well(5, 5, 2, 300.0, 1.5, false)?;
// Ok(()) - well added successfully
```

---

## Physics Implications

### Why Validate PI ≥ 0?

Productivity index $PI = \frac{C \times k}{\mu \times B}$ is always ≥ 0 in physics:
- $C$: geometric constant (constant)
- $k$: permeability (always > 0)
- $\mu$: viscosity (always > 0)
- $B$: formation volume factor (always > 0)

A negative PI violates fundamental reservoir engineering.

### Why Check BHP Range [-100, 2000] bar?

| BHP | Meaning | Feasibility |
|-----|---------|-------------|
| < -100 bar | Hard vacuum (mercury column > 1000 m) | Impossible |
| -100 to 0 bar | Vacuum/cavitation | Very rare (special cases) |
| 0 to 1000 bar | Normal producer (most cases) | ✅ Common |
| 1000 to 2000 bar | Injection/high pressure | ✅ Common |
| > 2000 bar | Extreme pressure (20,000 meters depth?) | Suspicious |

Out-of-range values are engineering red flags.

### Why Validate at Grid Bounds?

If `i >= nx`, accessing `self.grid_cells[self.idx(i, j, k)]` causes:
1. Out-of-bounds access
2. Undefined behavior (memory corruption)
3. Crash or silent NaN propagation
4. Impossible to debug

Validation prevents this at the source.

---

## Testing Recommendations

### Test 1: Valid Well
```javascript
const result = sim.add_well(5, 5, 2, 300.0, 1.5, false);
assert(result === undefined);  // Success
assert(sim.getWellState().length === 1);
```

### Test 2: Out-of-bounds Index
```javascript
const result = sim.add_well(100, 5, 2, 300.0, 1.5, false);
assert(result !== undefined);  // Error
assert(result.includes("out of bounds"));
assert(sim.getWellState().length === 0);  // Not added
```

### Test 3: NaN BHP
```javascript
const result = sim.add_well(5, 5, 2, NaN, 1.5, false);
assert(result !== undefined);  // Error
assert(result.includes("must be finite"));
assert(sim.getWellState().length === 0);
```

### Test 4: Negative PI
```javascript
const result = sim.add_well(5, 5, 2, 300.0, -1.5, false);
assert(result !== undefined);  // Error
assert(result.includes("must be non-negative"));
assert(sim.getWellState().length === 0);
```

### Test 5: Infinity PI
```javascript
const result = sim.add_well(5, 5, 2, 300.0, Infinity, false);
assert(result !== undefined);  // Error
assert(result.includes("must be finite"));
assert(sim.getWellState().length === 0);
```

### Test 6: Multiple Wells
```javascript
sim.add_well(5, 5, 2, 300.0, 1.5, false);  // Producer
sim.add_well(2, 2, 1, 250.0, 2.0, true);   // Injector
assert(sim.getWellState().length === 2);
```

---

## Files Modified

1. **`src/lib/ressim/src/lib.rs`** (50 lines added)
   - Added `Well::validate()` method (lines 87-120)
   - Updated `add_well()` signature to return `Result<(), String>` (lines 273-293)
   - Added defensive checks in pressure equation loop (lines 397-407)
   - Added defensive checks in saturation update loop (lines 482-501)
   - Updated docstring with oil-field units and validation info

---

## Backward Compatibility

⚠️ **Breaking Change:** `add_well()` return type changed

**Before:**
```rust
sim.add_well(i, j, k, bhp, pi, injector);  // Returns nothing
```

**After:**
```rust
sim.add_well(i, j, k, bhp, pi, injector)?;  // Returns Result<(), String>
```

**Frontend Impact:**
- JavaScript will receive error if validation fails
- Must handle `Result` from wasm_bindgen
- Update `App.svelte` to handle validation errors

---

## Integration with Capillary Pressure (P1)

Well validation works independently of capillary pressure but complements it:

| Feature | Purpose |
|---------|---------|
| Capillary pressure (P1) | Physical coupling between phases |
| Well validation (P2) | Prevent bad inputs that corrupt physics |

Both contribute to robust, trustworthy simulation:
- ✅ P1: Correct physics equations
- ✅ P2: Safe inputs to those equations

---

## Key Achievements

| Achievement | Evidence |
|-------------|----------|
| Input validation | `Well::validate()` method checks all parameters |
| Fail-fast design | Errors caught at `add_well()` time, not later |
| Defensive programming | Runtime checks in pressure/saturation loops |
| Clear error messages | Each validation error explains what's wrong |
| Physics-aware | Checks respect oil-field conventions (PI ≥ 0, BHP range) |
| Documentation | Comprehensive docstrings and comments |
| Backward compatibility | Note: Breaking change to `add_well()` signature |
| Compilation | ✅ Zero errors after implementation |

---

## Summary

**P2 Implementation Status:** ✅ COMPLETE

- ✅ Well parameter validation at input time
- ✅ Defensive runtime checks to prevent NaN/Inf propagation
- ✅ Clear error messages for debugging
- ✅ Integration with oil-field unit system
- ✅ Comprehensive documentation
- ✅ Zero compilation errors
- ✅ Physics-aware sanity checks

**Lines of Code:** ~50
**Documentation:** This file (400+ lines)
**Compilation Status:** ✅ No errors
**Next Phase:** Frontend integration to handle Result<(), String> from add_well()

