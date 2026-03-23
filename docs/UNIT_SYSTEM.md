# Unit System Documentation

## Overview
The ReServoir SIMulator (ressim) uses **consistent oil-field units throughout** all calculations.

## Base Units

| Quantity | Unit | Symbol | Notes |
|----------|------|--------|-------|
| **Pressure** | bar | bar | 1 bar ≈ 0.987 atm = 100 kPa |
| **Distance** | meter | m | Grid dimensions, permeability |
| **Time** | day | d | Simulation time steps |
| **Volume** | cubic meter | m³ | Pore volumes, flow rates |
| **Permeability** | milliDarcy | mD | 1 D = 9.8692 × 10⁻¹³ m² |
| **Viscosity** | centiPoise | cP | 1 cP = 0.001 Pa·s |
| **Compressibility** | per bar | 1/bar | Fluid and rock compressibility |
| **Saturation** | dimensionless | — | [0, 1] range |

## Derived Units

### Transmissibility
- **Symbol:** T
- **Units:** m³/day/bar
- **Formula:** T = 8.5269888e-3 × k[mD] × A[m²] × λ[1/cP] / L[m]
- **Meaning:** Flow rate per unit pressure drop across a block interface

The current Rust implementation defines this as `DARCY_METRIC_FACTOR` in `src/lib/ressim/src/step.rs`. Older `0.001127` references in this repository described a different oilfield-unit convention and were stale.

### Well Productivity Index (PI)
- **Symbol:** PI
- **Units:** m³/day/bar
- **Formula:** Rate = PI × (p_block - BHP)
- **Positive PI:** Producer well (flow out of reservoir)
- **Negative rate (PI×ΔP < 0):** Injector well (flow into reservoir)

### Mobility
- **Symbol:** λ
- **Units:** 1/cP
- **Formula:** λ = k_r / μ
- **Total Mobility:** λ_t = k_rw/μ_w + k_ro/μ_o

For three-phase runs the implementation extends this to:

$$\lambda_t = \frac{k_{rw}}{\mu_w} + \frac{k_{ro}}{\mu_o} + \frac{k_{rg}}{\mu_g}$$

### Fractional Flow
- **Symbol:** f_w
- **Units:** dimensionless
- **Formula:** f_w = λ_w / λ_t
- **Range:** [0, 1]

## Fluid Properties (Default Values)

```rust
FluidProperties {
    mu_o: 1.0,      // Oil viscosity [cP]
    mu_w: 0.5,      // Water viscosity [cP]
    c_o: 1e-5,      // Oil compressibility [1/bar]
    c_w: 3e-6,      // Water compressibility [1/bar]
}
```

### Typical Ranges
- **Oil viscosity:** 0.1 - 10 cP
- **Water viscosity:** 0.3 - 1.0 cP
- **Oil compressibility:** 5e-6 to 1e-4 1/bar
- **Water compressibility:** 3e-6 to 5e-6 1/bar

Current PVT note:
- Constant-PVT input remains the default for many teaching scenarios.
- Black-oil PVT mode is also available: pressure-dependent `B_o`, `B_g`, `R_s`, `mu_o`, and `mu_g` can be generated from correlations or supplied through tabular input.

## Rock and Fluid Properties (Default Values)

```rust
RockFluidProps {
    s_wc: 0.1,      // Connate water saturation [dimensionless]
    s_or: 0.1,      // Residual oil saturation [dimensionless]
    n_w: 2.0,       // Corey exponent - water [dimensionless]
    n_o: 2.0,       // Corey exponent - oil [dimensionless]
}
```

### Corey Relative Permeability Curves

Water relative permeability:
$$k_{rw}(S_w) = \left[\frac{S_w - S_{wc}}{1 - S_{wc} - S_{or}}\right]^{n_w}$$

Oil relative permeability:
$$k_{ro}(S_w) = \left[\frac{1 - S_w - S_{or}}{1 - S_{wc} - S_{or}}\right]^{n_o}$$

Effective saturation range: $[S_{wc}, 1 - S_{or}]$

### Capillary Pressure

- **Oil-water sign convention:** $P_{cow} = P_{oil} - P_{water}$
- **Model:** Brooks-Corey capillary pressure
- **Current numerical protection:** the Rust implementation caps capillary pressure at $20 \times P_{entry}$ to avoid runaway capillary-sponge behavior in long-column gravity calculations.

Interpretation note:
- This cap is a numerical safeguard, not a physical plateau in the capillary curve. For capillary-dominated studies, document the chosen $P_{entry}$ and remember that the simulated curve is regularized.

## Grid Cell Properties (Default Values)

```rust
GridCell {
    porosity: 0.2,        // Porosity [dimensionless, 0-1]
    perm_x: 100.0,        // Permeability x-direction [mD]
    perm_y: 100.0,        // Permeability y-direction [mD]
    perm_z: 10.0,         // Permeability z-direction [mD] (vertical, typically lower)
    pressure: 300.0,      // Pressure [bar]
    sat_water: 0.3,       // Water saturation [dimensionless]
    sat_oil: 0.7,         // Oil saturation [dimensionless] (s_w + s_o = 1.0)
}
```

### Typical Ranges
- **Porosity:** 0.05 - 0.30
- **Horizontal permeability:** 1 - 1000 mD
- **Vertical permeability:** 0.01 - 100 mD (typically 1/10 of horizontal)
- **Pressure:** 10 - 500 bar
- **Water saturation:** 0.0 - 1.0

## Grid Dimensions (Default Values)

```rust
ReservoirSimulator::new(nx, ny, nz) {
    dx: 10.0,         // Cell size x-direction [m]
    dy: 10.0,         // Cell size y-direction [m]
    dz: vec![1.0; nz], // Cell size z-direction per layer [m]
}
```

Layer thickness `dz` is stored per layer (`Vec<f64>` of length `nz`). The uniform setter `setCellDimensions(dx, dy, dz)` fills all layers to the same value. The per-layer setter `setCellDimensionsPerLayer(dx, dy, dz_per_layer)` accepts a vector of per-layer thicknesses for non-uniform grids (e.g., SPE1 with layers of 6.1, 9.1, 15.2 m).

Example: 20×10×10 grid = 2000 cells
- Bulk volume: (20×10 m) × (10×10 m) × (10×1 m) = 200 × 100 × 10 m³ = 200,000 m³
- Pore volume at 20% porosity: 40,000 m³

## API Input/Output

### Step Function
```rust
simulator.step(delta_t_days: f64)
```
- **Input:** Time step in days [d]
- **Internally:** Converts to appropriate units for calculations
- **Simulation time:** Accumulated in days

### Well Definition
```rust
simulator.add_well(i, j, k, bhp_bar, well_radius_m, skin, injector)
```
- **Position:** Grid indices (i, j, k)
- **BHP:** Bottom-hole pressure [bar]
- **Well radius:** Wellbore radius [m]
- **Skin:** Skin factor [dimensionless]
- **Injector:** Boolean flag (true=injector, false=producer)

### Grid State Output
```rust
let state = simulator.get_grid_state()
```
Returns array of GridCell objects with current state:
- **pressure** [bar]
- **sat_water** [dimensionless]
- **sat_oil** [dimensionless]

## Transmissibility Calculation Details

The transmissibility between two cells is computed as:

$$T = 8.5269888 \times 10^{-3} \times \frac{k_h \times A}{L} \times \bar{\lambda}$$

Where:
- $k_h$ = harmonic mean of permeabilities [mD]
- $A$ = interface area [m²]
- $L$ = distance between cell centers [m]
- $\bar{\lambda}$ = average total mobility [1/cP]
- **8.5269888 × 10⁻³** = conversion factor used by the current implementation

This factor ensures:
- Input: k[mD], A[m²], L[m], λ[1/cP]
- Output: T[m³/day/bar]

## Pressure Equation (IMPES Method)

**Implicit pressure solve:**
$$\frac{\phi V_p c_t}{dt} (p^{n+1} - p^n) + \sum_{\text{neighbors}} T_i (p^{n+1} - p_i^{n+1}) + \sum_{\text{wells}} PI_j (p^{n+1} - BHP_j) = 0$$

Where:
- $\phi$ = porosity [dimensionless]
- $V_p$ = pore volume [m³]
- $c_t$ = total compressibility [1/bar]
- $dt$ = time step [day]
- $p$ = pressure [bar]

**Accumulation term:**
$$\text{Accumulation} = \frac{\phi V_p c_t}{dt} \text{ [m³/bar/day]}$$

## Saturation Equation (Explicit Method)

**Upwind fractional flow:**
$$\frac{\partial S_w}{\partial t} + \nabla \cdot (f_w \mathbf{v}) = 0$$

Where:
- $S_w$ = water saturation [dimensionless]
- $f_w$ = fractional flow [dimensionless]
- $\mathbf{v}$ = Darcy velocity [m/day]

**Discrete form (per cell):**
$$S_w^{n+1} = S_w^n + \frac{\Delta V_w}{V_p}$$

Where $\Delta V_w$ = water volume change [m³] over time step [day]

## Material Balance

**Two-phase system (no gas):**
$$S_w + S_o = 1.0$$

Where:
- $S_w$ = water saturation [dimensionless]
- $S_o$ = oil saturation [dimensionless]

**Conservation:**
$$\sum_{\text{cells}} (S_w^{n+1} - S_w^n) \times V_p = \text{Total fluid inflow/outflow}$$

## Unit Conversion Reference

If converting between systems:

### From oil-field units to SI:
- Pressure: multiply by 100,000 (bar → Pa)
- Permeability: multiply by 9.8692 × 10⁻¹³ (mD → m²)
- Viscosity: multiply by 0.001 (cP → Pa·s)
- Compressibility: multiply by 100,000 (1/bar → 1/Pa)
- Time: multiply by 86,400 (day → s)

### From SI to oil-field units:
- Pressure: divide by 100,000 (Pa → bar)
- Permeability: divide by 9.8692 × 10⁻¹³ (m² → mD)
- Viscosity: divide by 0.001 (Pa·s → cP)
- Compressibility: divide by 100,000 (1/Pa → 1/bar)
- Time: divide by 86,400 (s → day)

## Notes

1. **Transmissibility factor (`8.5269888e-3`):** This is the implementation constant currently used in the Rust solver for transmissibility in the repo's metric/bar/day unit system.

2. **Time in API:** The API accepts time steps in days (`step(delta_t_days)`), which is convenient for users. Internally, all calculations maintain consistency with the oilfield unit system.

3. **Saturation clamping:** Saturations are clamped to [0, 1] after each update to maintain physical validity.

4. **Material balance:** While the IMPES method is not strictly conservative (due to splitting), material balance errors are typically small for small time steps and reasonable mobility ratios.

5. **Three-phase diagnostics:** water and gas cumulative material-balance errors are reported explicitly. Oil remains the residual phase in current diagnostics, so this is not yet a full per-phase closure check.
6. **Black-oil solver safeguard:** below bubble point, the simulator can fall back to the base positive `c_o` in the pressure accumulation path to avoid destabilizing the IMPES pressure solve when saturated `B_o(P)` slope would otherwise imply a negative compressibility contribution.

6. **Default grid:** the constructor starts from 10 m × 10 m × 1 m cells and 100 mD / 100 mD / 10 mD permeability until scenario parameters overwrite them.
