# Transmissibility Factor: `8.5269888e-3` in the Current Implementation

## Overview
The current Rust solver uses **`DARCY_METRIC_FACTOR = 8.5269888e-3`** when computing transmissibility between grid cells.

This document is intentionally implementation-focused. Its purpose is to describe the value the repository currently uses and to prevent confusion with the older `0.001127` documentation that was left behind from a different unit-system convention.

## Transmissibility Formula

$$T = 8.5269888 \times 10^{-3} \times \frac{k_h \times A}{L} \times \bar{\lambda}$$

Where:
- **T** = Transmissibility [m³/day/bar]
- **k_h** = Harmonic mean permeability [mD]
- **A** = Interface area [m²]
- **L** = Distance between cell centers [m]
- **λ̄** = Average total mobility [1/cP]
- **8.5269888e-3** = Unit conversion factor used by the current solver [dimensionless]

## Source of Truth

The authoritative definition lives in `src/lib/ressim/src/step.rs`:

```rust
/// Conversion factor from mD·m²/(m·cP) to m³/day/bar.
const DARCY_METRIC_FACTOR: f64 = 8.526_988_8e-3;
```

If the solver constant changes, this document and `docs/UNIT_SYSTEM.md` should be updated in the same change.

## What Changed

- Older repository docs used **`0.001127`**.
- That value does **not** match the current Rust implementation.
- The old text mixed in a different oilfield-unit convention and became misleading after the simulator standardized its current metric/bar/day documentation.

## Practical Interpretation

The solver treats transmissibility as the multiplier that converts:

$$\frac{k_h \times A}{L} \times \bar{\lambda} \times \Delta p$$

into a flow rate in the repo's working units:

$$q \; [m^3/day]$$

with:

- permeability in `mD`
- geometry in `m` and `m²`
- mobility in `1/cP`
- pressure in `bar`
- time in `day`

## Example

Given:

- `k_h = 100 mD`
- `A = 10,000 m²`
- `L = 100 m`
- `λ̄ = 2.0 1/cP`

Then:

$$T = 8.5269888 \times 10^{-3} \times \frac{100 \times 10000}{100} \times 2.0$$

$$T = 170.539776 \; [m^3/day/bar]$$

That means a `1 bar` pressure drop would produce about `170.54 m³/day` across that connection in the current implementation.

## Maintenance Note

The important cleanup in this repository is not just the numeric change from `0.001127` to `8.5269888e-3`; it is the removal of mixed unit-system explanations. Future edits should keep this document aligned with the exact constant and wording in `step.rs`.

## Usage in Code

> **Illustrative outline** — actual signature and implementation details are in `src/lib/ressim/src/step.rs`.

```rust
fn transmissibility(&self, c1: &GridCell, c2: &GridCell, dim: char) -> f64 {
    // Get permeabilities [mD], area [m²], distance [m]
    let (perm1, perm2, dist, area) = /* ... */;
    
    // Harmonic mean permeability [mD]
    let k_h = 2.0 * perm1 * perm2 / (perm1 + perm2);
    
    // Average total mobility [1/cP]
    let mob_avg = (self.total_mobility(c1) + self.total_mobility(c2)) / 2.0;
    
    // Transmissibility [m³/day/bar]
    8.5269888e-3 * k_h * area / dist * mob_avg
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

## Sensitivity Analysis

The transmissibility is proportional to:
- **k↑** → T↑ → More flow (physical)
- **A↑** → T↑ → Larger interface (physical)
- **L↑** → T↓ → Farther apart (physical)
- **μ↑** → T↓ → More viscous (physical)

All sensitivities match physical intuition when properly unitized.

## Related Documentation

- See `UNIT_SYSTEM.md` for comprehensive unit documentation
- See `src/lib/ressim/src/step.rs` for implementation
- See `UNIT_REFERENCE.md` for quick reference
