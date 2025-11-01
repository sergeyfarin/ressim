# Saturation Not Updating - Root Cause Analysis

## Observed Problem
- Pressure distribution looks correct
- Water/Oil saturation NOT changing (stays at initial 0.3/0.7)
- Wells are active and producing/injecting

## Root Causes Identified

### 1. **Transmissibility too high** ❌
The current transmissibility calculation in the pressure equation:
```rust
let mob_avg = (self.total_mobility(c1) + self.total_mobility(c2)) / 2.0;
0.001127 * k_h * area / dist * mob_avg
```

**Problem:** This includes TOTAL mobility (water + oil), making fluxes huge, which makes saturation changes tiny

**Issue:** When total flow is large, the fractional flow approach makes saturation changes negligible
- Example: If total flux is 1000 m³/day and fw=0.3, water flux is only 300 m³/day
- Saturation changes per step: delta_sw = 300 / pore_volume ≈ negligible

### 2. **SCAL parameters too restrictive** ❌
Default parameters:
```rust
s_wc: 0.2,    // Connate water saturation
s_or: 0.2,    // Residual oil saturation
n_w: 2.0,     // Corey exponent for water
n_o: 2.0,     // Corey exponent for oil
```

Initial saturation: s_w = 0.3

**Problem:** With s_wc=0.2 and initial s_w=0.3, effective saturation is very small:
```
s_eff = (0.3 - 0.2) / (1.0 - 0.2 - 0.2) = 0.1 / 0.6 ≈ 0.167
k_rw = (0.167)^2 ≈ 0.028
```

**Result:** Water relative permeability is extremely low (2.8%), strangling water flow!

### 3. **Pressure equation doesn't use phase-specific mobility** ❌
The pressure equation uses total mobility:
```
lambda_t = k_rw/mu_w + k_ro/mu_o
```

But for a two-phase system, this averages out saturation effects, making it hard for saturation waves to develop.

### 4. **Well productivity index too low** ⚠️
```javascript
simulator.add_well(..., Number(50), ...);  // PI = 50 m³/day/bar
```

With PI=50 and typical pressure differences, well rates are only ~5000 m³/day total
- Injector at 400 bar adds 5000 m³/day * (1.0) = 5000 m³/day water
- But pore volume is only: 10*10*10 * 10*10*1 * 0.2 = 200,000 m³
- So delta_sw per 100 steps = 5000 * 1000 / 200,000 = 25% ✓ (should work!)

## Solutions

### Option 1: Reduce SCAL Parameters (Easier Flow) ✅
Lower s_wc and s_or to allow water to flow at initial conditions
```rust
s_wc: 0.1,    // Lower connate water
s_or: 0.1,    // Lower residual oil
```

### Option 2: Increase Well Productivity Index ✅
Increase PI to drive faster injection/production
```javascript
simulator.add_well(..., Number(200), ...);  // Higher PI
```

### Option 3: Reduce Time Step ⚠️
Use smaller dt to see changes accumulate (debugging only)

### Option 4: Adjust Initial Saturation ✅
Start with s_w = 0.25 instead of 0.3 to be further from s_wc=0.2

## Recommended Fix
1. **Lower SCAL parameters**: s_wc=0.1, s_or=0.1
2. **Increase PI to 200**: Stronger well effects
3. **Verify fractional flow calculation** works correctly

This ensures:
- Water can flow at initial conditions (higher k_rw)
- Saturation waves develop properly
- Capillary pressure effects are visible
