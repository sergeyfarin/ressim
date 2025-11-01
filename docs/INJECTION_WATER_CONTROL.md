# Injection Water Control Implementation

## Summary
Modified the reservoir simulator to use the `injector` flag to control what fluid is injected.

**Key Change:** Injectors now inject **100% water**, while producers produce fluid at the local reservoir composition.

## Code Changes

### File: `src/lib/ressim/src/lib.rs`

#### Well Composition Logic (Explicit Saturation Update)

**Location:** Line ~490-510 (well explicit contributions section)

```rust
// Determine water composition of well fluid
let fw = if w.injector {
    // Injectors inject 100% water
    1.0
} else {
    // Producers produce at reservoir fluid composition (fractional flow)
    self.frac_flow_water(&self.grid_cells[id])
};

let water_q_m3_day = q_m3_day * fw;

// Volume change [m³]. Production (q>0) removes fluid from block.
// For injector: q<0 (inflow), so -q_water*dt adds water to the block
delta_water_m3[id] -= water_q_m3_day * dt_days;
```

**Behavior:**
- **Injector (`w.injector = true`):** `fw = 1.0` → injects 100% water
  - When well pressure is lower than block pressure, water flows into the block
  - Since `q_m3_day` will be negative (inflow), `-q_water*dt` adds water
  - This increases `sat_water` and decreases `sat_oil` in the block

- **Producer (`w.injector = false`):** `fw = frac_flow_water()` → produces at local composition
  - Fractional flow depends on current water saturation at the block
  - When block pressure > well BHP, fluids flow out of the block
  - Production composition matches the local fluid saturation

#### Pressure Equation Comment Update

**Location:** Line ~396-407 (well implicit coupling section)

Updated comments to clarify injector behavior:
```rust
// For producer (injector=false): positive PI, well produces when p_cell > BHP
// For injector (injector=true): well injects 100% water when p_cell < BHP
```

## Physical Interpretation

### Current Well Configuration (in App.svelte)
```javascript
// Producers at right side (i=19, j=0, all k)
simulator.add_well(19, 0, k, 100, 100, false);

// Injectors at left side (i=0, j=0, all k)  
simulator.add_well(0, 0, k, 400, 100, true);
```

### Pressure-Driven Flow

**Injector wells:**
- BHP = 400 bar (high pressure)
- Initial block pressure ≈ 300 bar
- Pressure difference: 400 - 300 = 100 bar → injects fluid
- Fluid composition: **100% water**
- Effect: Increases water saturation in left side blocks

**Producer wells:**
- BHP = 100 bar (low pressure)
- Initial block pressure ≈ 300 bar
- Pressure difference: 100 - 300 = -200 bar → produces fluid
- Fluid composition: Matches local saturation (initially ~30% water, 70% oil)
- Effect: Depletes pressure and fluids from right side blocks

## Simulation Results

With this change, you should observe:
1. **Left side (injectors):** Water saturation increases over time
2. **Right side (producers):** Pressure decreases as fluids are extracted
3. **Water spread:** Water from injection front migrates rightward toward producers
4. **Oil displacement:** Oil moves ahead of the water front

## Validation

The implementation:
- ✅ Uses explicit logic within conditional (efficient single check)
- ✅ Maintains material balance (saturation constraint: s_w + s_o = 1)
- ✅ Correctly handles sign conventions (positive q = production, negative q = injection)
- ✅ Preserves existing pressure equation coupling
- ✅ Works with all three property visualizations (pressure, water sat, oil sat)

## Future Enhancements

If needed, you could extend this to support:
- Custom injection fluid composition (e.g., 50% water + 50% oil)
- Different BHP values for oil vs. water phases (multiphase well modeling)
- Fluid composition dependent on well type (gas injection, etc.)
