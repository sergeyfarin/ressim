use crate::ReservoirSimulator;

impl ReservoirSimulator {
    pub fn pore_volume_m3(&self, id: usize) -> f64 {
        self.dx * self.dy * self.dz * self.porosity[id]
    }

    pub(crate) fn idx(&self, i: usize, j: usize, k: usize) -> usize {
        (k * self.nx * self.ny) + (j * self.nx) + i
    }

    pub(crate) fn depth_at_k(&self, k: usize) -> f64 {
        self.depth_reference_m + (k as f64 + 0.5) * self.dz
    }

    /// Geometric transmissibility factor [mD·m²/m] - geometry only, no mobility
    /// This is the constant part of transmissibility that depends only on rock properties
    /// and grid geometry. Used with upstream mobility for proper flow direction.
    /// Formula: T_geom = k_h * A / L where k_h is harmonic mean of permeabilities
    pub(crate) fn geometric_transmissibility(&self, id1: usize, id2: usize, dim: char) -> f64 {
        let (perm1, perm2, dist, area) = match dim {
            'x' => (
                self.perm_x[id1],
                self.perm_x[id2],
                self.dx,
                self.dy * self.dz,
            ),
            'y' => (
                self.perm_y[id1],
                self.perm_y[id2],
                self.dy,
                self.dx * self.dz,
            ),
            'z' => (
                self.perm_z[id1],
                self.perm_z[id2],
                self.dz,
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
