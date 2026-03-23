use crate::ReservoirSimulator;

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

    /// Geometric transmissibility factor [mD·m²/m] - geometry only, no mobility
    /// This is the constant part of transmissibility that depends only on rock properties
    /// and grid geometry. Used with upstream mobility for proper flow direction.
    /// Formula: T_geom = k_h * A / L where k_h is harmonic mean of permeabilities
    pub(crate) fn geometric_transmissibility(&self, id1: usize, id2: usize, dim: char) -> f64 {
        let k1 = id1 / (self.nx * self.ny);
        let k2 = id2 / (self.nx * self.ny);
        let dz1 = self.dz[k1];
        let dz2 = self.dz[k2];

        let (perm1, perm2, dist, area) = match dim {
            'x' => (
                self.perm_x[id1],
                self.perm_x[id2],
                self.dx,
                self.dy * dz1,
            ),
            'y' => (
                self.perm_y[id1],
                self.perm_y[id2],
                self.dy,
                self.dx * dz1,
            ),
            'z' => (
                self.perm_z[id1],
                self.perm_z[id2],
                // Half-cell distance: dz1/2 + dz2/2
                (dz1 + dz2) * 0.5,
                self.dx * self.dy,
            ),
            _ => (0.0, 0.0, 1.0, 1.0),
        };
        // Harmonic mean of permeabilities [mD]
        let k_h = if perm1 + perm2 == 0.0 {
            0.0
        } else {
            2.0 * perm1 * perm2 / (perm1 + perm2)
        };
        if k_h == 0.0 {
            return 0.0;
        }

        // Geometric transmissibility factor [mD·m²/m]
        k_h * area / dist
    }
}
