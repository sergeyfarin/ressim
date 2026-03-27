use crate::ReservoirSimulator;

fn harmonic_mean(a: f64, b: f64) -> f64 {
    if a + b == 0.0 {
        0.0
    } else {
        2.0 * a * b / (a + b)
    }
}

impl ReservoirSimulator {
    pub fn pore_volume_m3(&self, id: usize) -> f64 {
        self.dx * self.dy * self.dz_at(id) * self.porosity[id]
    }

    pub(crate) fn idx(&self, i: usize, j: usize, k: usize) -> usize {
        (k * self.nx * self.ny) + (j * self.nx) + i
    }

    pub(crate) fn depth_at_k(&self, k: usize) -> f64 {
        let mut depth = self.depth_reference_m;
        for layer in 0..k {
            depth += self.dz[layer];
        }
        depth += self.dz[k] * 0.5;
        depth
    }

    /// Geometric transmissibility factor [mD·m²/m] - geometry only, no mobility.
    ///
    /// Uses the standard two-point flux approximation (TPFA): the harmonic mean
    /// of half-cell transmissibilities.
    ///
    /// For x/y directions (same layer, same dz):
    ///   T = 2·k1·k2 / (k1 + k2) · A / L
    ///
    /// For z direction (different layer thicknesses and permeabilities):
    ///   T = 2·k1·k2·A / (k2·dz1 + k1·dz2)
    ///
    /// The z formula is the harmonic mean of half-cell transmissibilities:
    ///   T_half_i = k_i · A / (dz_i / 2), then T = 1/(1/T1 + 1/T2).
    pub(crate) fn geometric_transmissibility(&self, id1: usize, id2: usize, dim: char) -> f64 {
        let k1 = id1 / (self.nx * self.ny);
        let k2 = id2 / (self.nx * self.ny);
        let dz1 = self.dz[k1];
        let dz2 = self.dz[k2];

        match dim {
            'x' => {
                let perm1 = self.perm_x[id1];
                let perm2 = self.perm_x[id2];
                let area = self.dy * dz1;
                let k_h = harmonic_mean(perm1, perm2);
                k_h * area / self.dx
            }
            'y' => {
                let perm1 = self.perm_y[id1];
                let perm2 = self.perm_y[id2];
                let area = self.dx * dz1;
                let k_h = harmonic_mean(perm1, perm2);
                k_h * area / self.dy
            }
            'z' => {
                let perm1 = self.perm_z[id1];
                let perm2 = self.perm_z[id2];
                let area = self.dx * self.dy;
                let denom = perm2 * dz1 + perm1 * dz2;
                if denom <= 0.0 {
                    return 0.0;
                }
                2.0 * perm1 * perm2 * area / denom
            }
            _ => 0.0,
        }
    }
}
