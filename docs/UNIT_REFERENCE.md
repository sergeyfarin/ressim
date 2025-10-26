# Oil-Field Units Quick Reference

## Units at a Glance

```
PRESSURE        → bar
DISTANCE        → m (meter)
TIME            → d (day)
PERMEABILITY    → mD (milliDarcy)
VISCOSITY       → cP (centiPoise)
COMPRESSIBILITY → 1/bar
SATURATION      → dimensionless
VOLUME          → m³ (cubic meter)
```

## Key Equations with Units

### Transmissibility
```
T [m³/day/bar] = 0.001127 × k[mD] × A[m²] / (L[m] × μ[cP])
```

### Well Rate
```
Rate [m³/day] = PI [m³/day/bar] × (p_block [bar] - BHP [bar])
```

### Mobility
```
λ [1/cP] = k_r [dimensionless] / μ [cP]
```

### Fractional Flow
```
f_w [dimensionless] = (k_rw/μ_w) / (k_rw/μ_w + k_ro/μ_o)
```

### Corey Curves
```
k_rw = ((S_w - S_wc) / (1 - S_wc - S_or))^n_w
k_ro = ((1 - S_w - S_or) / (1 - S_wc - S_or))^n_o
```

## Default Values

| Property | Value | Unit |
|----------|-------|------|
| Oil viscosity | 1.0 | cP |
| Water viscosity | 0.5 | cP |
| Oil compressibility | 1e-5 | 1/bar |
| Water compressibility | 3e-6 | 1/bar |
| Porosity | 0.2 | — |
| Horiz. permeability | 100 | mD |
| Vert. permeability | 10 | mD |
| Initial pressure | 300 | bar |
| Initial water sat. | 0.3 | — |
| Cell size (x,y,z) | 100,100,20 | m |
| Connate water sat. | 0.2 | — |
| Residual oil sat. | 0.2 | — |
| Corey exponents | 2.0, 2.0 | — |

## Common Values

### Permeability
- Tight sand: 1-10 mD
- Poor sand: 10-100 mD
- Good sand: 100-500 mD
- Excellent sand: >500 mD

### Viscosity
- Water: 0.3-1.0 cP
- Light oil: 0.5-2.0 cP
- Medium oil: 2-10 cP
- Heavy oil: 10-1000 cP

### Pressure (bar)
- Shallow reservoirs: 50-200 bar
- Typical: 200-400 bar
- Deep: 400-1000+ bar

## API Usage

### Create simulator
```rust
let mut sim = ReservoirSimulator::new(20, 10, 10);  // nx, ny, nz
```

### Add well
```rust
sim.add_well(
    10,      // i index
    5,       // j index
    3,       // k index
    100.0,   // BHP [bar]
    0.5,     // PI [m³/day/bar]
    false    // injector flag (false=producer)
);
```

### Run simulation
```rust
for step in 0..100 {
    sim.step(0.1);  // 0.1 day time step
}
```

### Get results
```rust
let state = sim.get_grid_state();  // Returns GridCell array
let time = sim.get_time();          // Returns time in days
```

## Typical Time Steps

| Scenario | Δt [days] | Notes |
|----------|-----------|-------|
| Stability check | 0.01 | Very small, tight coupling |
| Early time | 0.1 | Transient response |
| Middle time | 1.0 | Typical simulation |
| Late time | 10.0 | Quasi-steady behavior |
| Coarse | 100.0 | Very loose coupling |

## Unit Conversions (if needed)

### To SI units (multiply by):
```
bar → Pa:     × 100,000
mD → m²:      × 9.8692e-13
cP → Pa·s:    × 0.001
1/bar → 1/Pa: × 100,000
day → s:      × 86,400
```

### From SI units (divide by):
```
Pa → bar:     ÷ 100,000
m² → mD:      ÷ 9.8692e-13
Pa·s → cP:    ÷ 0.001
1/Pa → 1/bar: ÷ 100,000
s → day:      ÷ 86,400
```

## Validation Checklist

When setting up a simulation:
- [ ] Pressure values in reasonable range (50-1000 bar)
- [ ] Permeabilities positive (1-1000+ mD typical)
- [ ] Saturations sum to 1.0 per cell
- [ ] Porosity in [0.05, 0.3]
- [ ] Viscosities positive (0.1-100 cP range typical)
- [ ] Compressibilities positive and small
- [ ] Time steps reasonable for flow dynamics
- [ ] Well PI positive for producers
- [ ] Well BHP less than reservoir pressure for producers

## Troubleshooting

| Issue | Check |
|-------|-------|
| Pressure solver diverges | Time step too large? Permeability too anisotropic? |
| Unrealistic saturation | Gravity effects missing? Capillary pressure needed? |
| Zero well rate | BHP equals reservoir pressure? PI too small? |
| Simulation very slow | Time step too small? Grid too fine? |
| Memory issues | Grid dimensions nx×ny×nz too large? |

## Physics Notes

1. **Two-phase only:** Oil + Water, no gas phase (S_w + S_o = 1.0)
2. **IMPES method:** Implicit pressure, explicit saturation
3. **Upwind scheme:** Flux evaluation for saturation stability
4. **Corey model:** Standard relative permeability correlation
5. **No gravity:** Vertical equilibrium assumed or negligible
6. **No capillary pressure:** Only pressure difference drives flow

## Documentation Files

- `UNIT_SYSTEM.md` - Comprehensive unit documentation
- `PHYSICS_REVIEW.md` - Physics model analysis
- `UNIT_SYSTEM_CHANGES.md` - Change log from refactoring
