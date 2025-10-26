# P2 Well Validation - Quick Reference

**Status:** ✅ COMPLETE | **Compilation:** No errors

---

## What Changed?

### `add_well()` Signature Changed

**Before:**
```rust
pub fn add_well(&mut self, i: usize, j: usize, k: usize, bhp: f64, pi: f64, injector: bool)
```

**After:**
```rust
pub fn add_well(&mut self, i: usize, j: usize, k: usize, bhp: f64, pi: f64, injector: bool) -> Result<(), String>
```

Now returns `Result` to report validation errors.

---

## Validation Checks

All of these are checked when you call `add_well()`:

| Check | Fails If | Error Message |
|-------|----------|---------------|
| Grid bounds | `i >= nx` | "Well index i=X out of bounds (nx=Y)" |
| Grid bounds | `j >= ny` | "Well index j=X out of bounds (ny=Y)" |
| Grid bounds | `k >= nz` | "Well index k=X out of bounds (nz=Y)" |
| BHP finite | `bhp.is_nan()` | "BHP must be finite, got: NaN" |
| BHP finite | `bhp.is_infinite()` | "BHP must be finite, got: Inf" |
| BHP range | `bhp < -100 \|\| bhp > 2000` | "BHP out of reasonable range [-100, 2000] bar, got: X" |
| PI non-negative | `pi < 0.0` | "Productivity index must be non-negative, got: X" |
| PI finite | `pi.is_nan()` | "Productivity index must be finite, got: NaN" |
| PI finite | `pi.is_infinite()` | "Productivity index must be finite, got: Inf" |

---

## Usage Examples

### Valid Producer Well
```rust
// Grid: 10×10×5, creating producer at cell (5,5,2)
let result = sim.add_well(5, 5, 2, 300.0, 1.5, false);
assert!(result.is_ok());  // ✅ Passes all checks
```

### Valid Injector Well
```rust
// Injector at cell (2,2,1) with high BHP
let result = sim.add_well(2, 2, 1, 400.0, 2.0, true);
assert!(result.is_ok());  // ✅ Passes all checks
```

### Invalid: Out of Bounds
```rust
// Grid is 10×10×5, trying to place well at (15, 5, 2)
let result = sim.add_well(15, 5, 2, 300.0, 1.5, false);
assert!(result.is_err());
assert_eq!(result.unwrap_err(), "Well index i=15 out of bounds (nx=10)");
```

### Invalid: NaN Pressure
```rust
let result = sim.add_well(5, 5, 2, f64::NAN, 1.5, false);
assert!(result.is_err());
assert!(result.unwrap_err().contains("BHP must be finite"));
```

### Invalid: Negative PI
```rust
// Productivity index cannot be negative
let result = sim.add_well(5, 5, 2, 300.0, -1.5, false);
assert!(result.is_err());
assert!(result.unwrap_err().contains("must be non-negative"));
```

### Invalid: Unrealistic BHP
```rust
// BHP out of reasonable range
let result = sim.add_well(5, 5, 2, 50000.0, 1.5, false);
assert!(result.is_err());
assert!(result.unwrap_err().contains("out of reasonable range"));
```

---

## Frontend Integration (App.svelte)

Update JavaScript well-adding code to handle Result:

### Before (if this was used):
```javascript
simulator.add_well(i, j, k, bhp, pi, injector);
```

### After (now required):
```javascript
try {
    simulator.add_well(i, j, k, bhp, pi, injector);
    console.log("✅ Well added successfully");
} catch (error) {
    console.error("❌ Well validation failed:", error);
    // Handle error - show user message, don't add well, etc.
}
```

Or with optional chaining (if using Result wrapper):
```javascript
const result = simulator.add_well(i, j, k, bhp, pi, injector);
if (result !== undefined) {
    console.error("Validation failed:", result);
    // Handle error
} else {
    console.log("Well added successfully");
}
```

---

## Oil-Field Units Reminder

When calling `add_well()`, use **oil-field units**:

| Parameter | Unit | Range | Example |
|-----------|------|-------|---------|
| `i` | - | 0 to nx-1 | 5 |
| `j` | - | 0 to ny-1 | 5 |
| `k` | - | 0 to nz-1 | 2 |
| `bhp` | bar | -100 to 2000 | 300.0 |
| `pi` | m³/day/bar | ≥ 0 | 1.5 |
| `injector` | bool | true/false | false |

**NOT SI units.** Use bar, not Pa. Use m³/day/bar, not m³/s/Pa.

---

## Physics Validation Rules

### Why `pi >= 0`?

Productivity index formula: $PI = \frac{C \cdot k}{\mu \cdot B}$

All factors are positive → PI must be positive. Negative PI is unphysical.

### Why `-100 <= bhp <= 2000` bar?

| BHP Range | Scenario | Common? |
|-----------|----------|---------|
| < -100 bar | Harder vacuum than atmosphere | ❌ No |
| -100 to 0 | Cavitation/vacuum | ⚠️ Rare special cases |
| 0 to 1000 | Normal producers | ✅ Yes |
| 1000 to 2000 | Injectors/high pressure | ✅ Yes |
| > 2000 bar | Extreme (deep wells) | ⚠️ Suspicious |

---

## Defensive Runtime Checks

Even if a well somehow passes validation, there are runtime checks:

1. **Pressure Equation Loop:** Skip well if `pi` or `bhp` is not finite
2. **Saturation Update Loop:** Skip well if computed rate is not finite

This prevents NaN/Inf from propagating if something goes wrong.

---

## Error Handling Patterns

### Pattern 1: Panic on Error
```rust
sim.add_well(i, j, k, bhp, pi, injector).expect("Failed to add well");
```

### Pattern 2: Match on Result
```rust
match sim.add_well(i, j, k, bhp, pi, injector) {
    Ok(()) => println!("Well added"),
    Err(e) => eprintln!("Error: {}", e),
}
```

### Pattern 3: Propagate with ?
```rust
sim.add_well(i, j, k, bhp, pi, injector)?;  // Propagate error up
```

---

## Summary of P2 Changes

| Aspect | Change |
|--------|--------|
| Input validation | ✅ Added `Well::validate()` method |
| Return type | ✅ `add_well()` now returns `Result<(), String>` |
| Checks at entry | ✅ 9 validation checks before well is added |
| Runtime safety | ✅ Defensive checks in pressure and saturation loops |
| Error messages | ✅ Clear, specific error reporting |
| Physics awareness | ✅ Checks respect oil-field conventions |
| Compilation | ✅ No errors |

---

## Troubleshooting

### "Well index out of bounds"
→ Your grid is smaller than you think. Check grid dimensions (nx, ny, nz).

### "BHP must be finite"
→ Don't pass NaN or Infinity for BHP. Check your pressure calculation.

### "Productivity index must be non-negative"
→ PI cannot be negative. Verify your well model calculation.

### "BHP out of reasonable range"
→ BHP is suspiciously high or low. Double-check your pressure units (use bar).

---

## Files Modified

- `src/lib/ressim/src/lib.rs` - Added validation (~50 lines)
  - `Well::validate()` method
  - Updated `add_well()` 
  - Defensive runtime checks

---

## Next Steps

1. ✅ P1: Capillary pressure implementation - COMPLETE
2. ✅ P2: Well parameter validation - COMPLETE
3. ⏳ P2b: Update App.svelte to handle Result from add_well()
4. ⏳ P3: Code cleanup and refactoring
5. ⏳ Regression testing with benchmark problems

