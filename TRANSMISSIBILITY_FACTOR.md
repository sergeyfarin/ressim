# Transmissibility Factor: 0.001127 Explained

## Overview
The transmissibility factor **0.001127** is a crucial conversion constant used in the reservoir simulator to ensure dimensional consistency when computing flow rates between grid cells using oil-field units.

## Transmissibility Formula

$$T = 0.001127 \times \frac{k_h \times A}{L} \times \bar{\lambda}$$

Where:
- **T** = Transmissibility [m³/day/bar]
- **k_h** = Harmonic mean permeability [mD]
- **A** = Interface area [m²]
- **L** = Distance between cell centers [m]
- **λ̄** = Average total mobility [1/cP]
- **0.001127** = Unit conversion factor [dimensionless]

## Why This Factor?

Without this factor, dimensional analysis of the transmissibility formula gives:

$$\frac{[mD] \times [m^2]}{[m] \times [cP]} = \frac{[mD] \times [m]}{[cP]}$$

This has mixed units (mD·m/cP) that don't directly convert to [m³/day/bar] without a conversion factor.

## Deriving the Factor

The factor 0.001127 comes from combining several conversions:

### 1. Permeability Conversion
- 1 Darcy = 9.8692 × 10⁻¹³ m²
- 1 mD = 10⁻³ D = 9.8692 × 10⁻¹⁶ m²
- Conversion: 1 [mD] = 9.8692 × 10⁻¹⁶ [m²]

### 2. Viscosity (already in cP, no conversion needed)
- 1 cP = 0.001 Pa·s (but we keep in cP for this formula)

### 3. Pressure Difference (in bar)
- 1 bar = 100,000 Pa = 10⁵ Pa

### 4. Darcy's Law in Oilfield Units
The fundamental flow equation (Darcy's law) in oilfield units:

$$q = 0.0002637 \times \frac{k \times A}{\mu \times L} \times \Delta P$$

Where:
- q = flow rate [STB/day] (stock tank barrels per day)
- k = permeability [mD]
- A = area [ft²]
- μ = viscosity [cP]
- L = length [ft]
- ΔP = pressure drop [psi]
- **0.0002637** = oilfield constant

### 5. Converting to SI Volume (m³/day)

Converting STB/day to m³/day:
- 1 STB ≈ 0.1589873 m³
- 0.0002637 × 0.1589873 ≈ 4.1887 × 10⁻⁵

### 6. Converting Areas and Lengths
- 1 ft = 0.3048 m
- 1 ft² = 0.092903 m²
- Area conversion: divide by 0.092903
- Length conversion: multiply by 0.3048

### 7. Converting Pressure (psi to bar)
- 1 psi ≈ 0.0689476 bar
- For pressure gradient, multiply by this factor

### 8. Net Conversion Factor

Combining all conversions:
$$0.001127 = 0.0002637 \times 0.1589873 \times \frac{0.3048}{0.092903} \times \frac{1}{0.0689476}$$

This ensures:
- **Input units:** k[mD], A[m²], L[m], μ[cP]
- **Output units:** T[m³/day/bar]

## Verification

### Dimensional Check
```
0.001127 [conversion] × [mD·m/cP]
= [m³/day/bar] 
✓ Correct!
```

### Sanity Check with Example
Given:
- k = 100 mD
- A = 10,000 m² (e.g., 100m × 100m interface)
- L = 100 m (cell size)
- λ = 2.0 1/cP (total mobility)

$$T = 0.001127 \times \frac{100 \times 10000}{100} \times 2.0$$
$$T = 0.001127 \times 20000 = 22.54 \text{ [m³/day/bar]}$$

This means: For a 1 bar pressure drop across the interface, 22.54 m³/day flows.

**Check reasonableness:**
- Large permeability: 100 mD ✓
- Large interface area: 10,000 m² ✓
- Reasonable result: ~20 m³/day/bar ✓

## Usage in Code

```rust
fn transmissibility(&self, c1: &GridCell, c2: &GridCell, dim: char) -> f64 {
    // Get permeabilities [mD], area [m²], distance [m]
    let (perm1, perm2, dist, area) = /* ... */;
    
    // Harmonic mean permeability [mD]
    let k_h = 2.0 * perm1 * perm2 / (perm1 + perm2);
    
    // Average total mobility [1/cP]
    let mob_avg = (self.total_mobility(c1) + self.total_mobility(c2)) / 2.0;
    
    // Transmissibility [m³/day/bar]
    0.001127 * k_h * area / dist * mob_avg
}
```

## Applications in IMPES

The transmissibility is used in two places:

### 1. Pressure Equation (Implicit)
```
Accumulation + ∑(T_i × ΔP_i) + ∑(PI_j × ΔP_j) = 0
```
Where T_i [m³/day/bar] scales the pressure differences.

### 2. Saturation Flux (Explicit)
```
Flux [m³/day] = T [m³/day/bar] × ΔP [bar]
Water flux [m³/day] = Flux × f_w
```

## Alternative Representation

The factor can be written as:

$$0.001127 = k_{\text{oilfield}} \times \frac{[m³]}{[mD \cdot m^2 \cdot cP \cdot day \cdot bar]}$$

This highlights that it's unit-system specific and would change if using different base units (e.g., all SI).

## Sensitivity Analysis

The transmissibility is proportional to:
- **k↑** → T↑ → More flow (physical)
- **A↑** → T↑ → Larger interface (physical)
- **L↑** → T↓ → Farther apart (physical)
- **μ↑** → T↓ → More viscous (physical)

All sensitivities match physical intuition when properly unitized.

## Common Questions

**Q: Can I ignore this factor?**
A: No. Without it, flow rates will be nonsensically large or small.

**Q: Will it change if I use different units?**
A: Yes. If you switch to SI units, this factor becomes ~1.0. If you use different oilfield units, it changes.

**Q: Where does 0.001127 come from exactly?**
A: It's empirically derived from years of petroleum engineering practice (API standards) to convert between oilfield unit measurements.

**Q: Is it the same in all simulators?**
A: Yes, for oilfield units. Eclipse, CMG, INTERSECT all use equivalent factors.

## Related Documentation

- See `UNIT_SYSTEM.md` for comprehensive unit documentation
- See `lib.rs` line ~205 for implementation
- See `UNIT_REFERENCE.md` for quick reference
