//! Three-phase IMPES mass closure: component masses in, saturations and fluid volume out.
//!
//! Three-phase IMPES transports all four black-oil masses (water, stock-tank oil, free gas,
//! dissolved gas) as surface volumes and recovers the cell state from them with [`flash_cell`].
//! Nothing is a residual, so every component is conserved by construction. The only thing left
//! for the pressure solve to satisfy is the volume constraint `V(p, N) = Vp(p)`. Before #37 oil
//! was the residual `So = 1 − Sw − Sg`, and any mismatch between the pressure equation's storage
//! term and this closure was booked silently as oil.
//!
//! [`flash_cell`]: ReservoirSimulator::flash_cell

use crate::ReservoirSimulator;
use crate::math;

/// One cell's component inventory at surface conditions [Sm³].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct CellMasses {
    pub(crate) water_sc: f64,
    pub(crate) oil_sc: f64,
    pub(crate) free_gas_sc: f64,
    pub(crate) dissolved_gas_sc: f64,
}

impl CellMasses {
    pub(crate) fn total_gas_sc(&self) -> f64 {
        self.free_gas_sc + self.dissolved_gas_sc
    }
}

/// The cell state a set of masses occupies at one pressure.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CellFlash {
    pub(crate) sw: f64,
    pub(crate) so: f64,
    pub(crate) sg: f64,
    pub(crate) rs: f64,
    /// Reservoir volume the three phases occupy [m³].
    pub(crate) fluid_volume_m3: f64,
    /// Pore volume at this pressure, rock compressibility included [m³].
    pub(crate) pore_volume_m3: f64,
}

impl CellFlash {
    /// Volume-balance residual `V − Vp` [m³]: positive when the fluid does not fit.
    pub(crate) fn volume_residual_m3(&self) -> f64 {
        self.fluid_volume_m3 - self.pore_volume_m3
    }
}

impl ReservoirSimulator {
    /// Pore volume at pressure `p`, with the same rock law as FIM:
    /// `Vp_ref · exp(c_r · (p − p_ref))`.
    pub(crate) fn pore_volume_at_pressure_m3(&self, id: usize, pressure_bar: f64) -> f64 {
        self.pore_volume_m3(id)
            * math::exp(
                (pressure_bar - self.rock_reference_pressure_bar) * self.rock_compressibility,
            )
    }

    fn oil_fvf_at(&self, pressure_bar: f64, rs: f64) -> f64 {
        match &self.pvt_table {
            Some(table) => table.interpolate_oil(pressure_bar, rs).0,
            None => self.base_oil_fvf_generic(pressure_bar),
        }
        .max(1e-9)
    }

    /// Component masses of cell `id` in its current state.
    pub(crate) fn cell_masses(&self, id: usize) -> CellMasses {
        let p = self.pressure[id];
        let vp = self.pore_volume_at_pressure_m3(id, p);
        let oil_sc = self.sat_oil[id].max(0.0) * vp / self.oil_fvf_at(p, self.rs[id]);
        CellMasses {
            water_sc: self.sat_water[id].max(0.0) * vp * self.water_inverse_fvf(p),
            oil_sc,
            free_gas_sc: self.sat_gas[id].max(0.0) * vp / self.get_b_g(p).max(1e-9),
            dissolved_gas_sc: if self.pvt_table.is_some() {
                oil_sc * self.rs[id].max(0.0)
            } else {
                0.0
            },
        }
    }

    /// Place `masses` in cell `id` at pressure `p`.
    ///
    /// Gas splits between oil and the free phase as in the previous IMPES flash. With
    /// redissolution, all gas is one pool, dissolved up to `Rs_sat(p)`. Without it
    /// (DRSDT 0), free gas stays free and the oil may hold at most `min(Rs_old, Rs_sat(p))`,
    /// where `Rs_old` is the cell's current Rs. Anything above that cap is liberated.
    ///
    /// Saturations are volumes over `Vp(p)`. They sum to one only when the pressure satisfies
    /// the volume balance, which is what the IMPES pressure iteration drives towards.
    pub(crate) fn flash_cell(
        &self,
        id: usize,
        pressure_bar: f64,
        masses: &CellMasses,
    ) -> CellFlash {
        let pore_volume_m3 = self.pore_volume_at_pressure_m3(id, pressure_bar).max(1e-12);
        let oil_sc = masses.oil_sc.max(0.0);
        let free_gas_sc = masses.free_gas_sc.max(0.0);
        let dissolved_gas_sc = masses.dissolved_gas_sc.max(0.0);

        let (rs, free_gas_sc) = match &self.pvt_table {
            None => (0.0, free_gas_sc + dissolved_gas_sc),
            Some(table) => {
                let rs_sat = table.interpolate(pressure_bar).rs_m3m3.max(0.0);
                if oil_sc <= 1e-12 {
                    (
                        self.rs[id].max(0.0).min(rs_sat),
                        free_gas_sc + dissolved_gas_sc,
                    )
                } else if self.gas_redissolution_enabled {
                    let total = free_gas_sc + dissolved_gas_sc;
                    if total <= oil_sc * rs_sat {
                        (total / oil_sc, 0.0)
                    } else {
                        (rs_sat, total - oil_sc * rs_sat)
                    }
                } else {
                    let cap = self.rs[id].max(0.0).min(rs_sat);
                    if dissolved_gas_sc <= oil_sc * cap {
                        (dissolved_gas_sc / oil_sc, free_gas_sc)
                    } else {
                        (cap, free_gas_sc + dissolved_gas_sc - oil_sc * cap)
                    }
                }
            }
        };

        let water_volume = masses.water_sc.max(0.0) * self.water_fvf(pressure_bar);
        let oil_volume = oil_sc * self.oil_fvf_at(pressure_bar, rs);
        let gas_volume = free_gas_sc * self.get_b_g(pressure_bar).max(1e-9);
        CellFlash {
            sw: water_volume / pore_volume_m3,
            so: oil_volume / pore_volume_m3,
            sg: gas_volume / pore_volume_m3,
            rs,
            fluid_volume_m3: water_volume + oil_volume + gas_volume,
            pore_volume_m3,
        }
    }

    /// `−∂(V − Vp)/∂p` at fixed masses [m³/bar]: the storage coefficient `Vp·c_t` the closure
    /// itself implies, used on the diagonal of the volume-balance Newton step.
    ///
    /// Central difference, so at the bubble-point kink it takes the mean of the two slopes. It
    /// is floored at a tiny positive value because a thermodynamically unstable PVT table
    /// (`dBo/dp > Bg·dRs/dp`, #39) can make it negative.
    pub(crate) fn closure_storage_m3_per_bar(
        &self,
        id: usize,
        pressure_bar: f64,
        masses: &CellMasses,
    ) -> f64 {
        const DP_BAR: f64 = 1e-3;
        let hi = self.flash_cell(id, pressure_bar + DP_BAR, masses);
        let lo = self.flash_cell(id, pressure_bar - DP_BAR, masses);
        let slope = -(hi.volume_residual_m3() - lo.volume_residual_m3()) / (2.0 * DP_BAR);
        slope.max(1e-9 * hi.pore_volume_m3)
    }
}
