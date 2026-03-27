// File: `wasm/simulator/src/lib.rs`
//
// UNIT SYSTEM: OIL-FIELD UNITS (CONSISTENT THROUGHOUT)
// =====================================================
// Pressure: bar
// Distance: meter (m)
// Time: day (d)
// Volume: cubic meter (m³)
// Permeability: milliDarcy (mD) [1 D = 9.8692e-13 m²]
// Viscosity: centiPoise (cP) [1 cP = 0.001 Pa·s]
// Compressibility: 1/bar
// Saturation: dimensionless [0, 1]
//
// CONVERSION FACTORS USED:
// - Transmissibility / PI use a metric Darcy factor that converts mD·m²/(m·cP) to m³/day/bar
// - All calculations maintain consistency in these base units with no hidden conversions

use serde::{Deserialize, Serialize};
use std::f64;
use wasm_bindgen::prelude::*;

mod capillary;
mod frontend;
mod grid;
mod mobility;
mod pressure_eqn;
mod pvt;
mod relperm;
mod reporting;
mod solver;
mod step;
mod transport;
mod well;
mod well_control;

pub use capillary::{CapillaryPressure, GasOilCapillaryPressure};
pub use relperm::{
    RockFluidProps, RockFluidPropsThreePhase, SgofRow, SwofRow, ThreePhaseScalTables,
};
pub use reporting::{TimePointRates, WellRates};
pub use well::Well;

/// Which fluid the injector injects in three-phase mode.
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug)]
pub enum InjectedFluid {
    Water,
    Gas,
}

#[wasm_bindgen(start)]
pub fn set_panic_hook() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct FluidProperties {
    pub mu_o: f64,
    pub mu_w: f64,
    pub c_o: f64,
    pub c_w: f64,
    pub rho_o: f64,
    pub rho_w: f64,
}

impl FluidProperties {
    fn default_pvt() -> Self {
        Self {
            mu_o: 1.0,
            mu_w: 0.5,
            c_o: 1e-5,
            c_w: 3e-6,
            rho_o: 800.0,
            rho_w: 1000.0,
        }
    }
}

#[wasm_bindgen]
pub struct ReservoirSimulator {
    nx: usize,
    ny: usize,
    nz: usize,
    dx: f64,
    dy: f64,
    dz: Vec<f64>,
    porosity: Vec<f64>,
    perm_x: Vec<f64>,
    perm_y: Vec<f64>,
    perm_z: Vec<f64>,
    pressure: Vec<f64>,
    sat_water: Vec<f64>,
    sat_oil: Vec<f64>,
    wells: Vec<Well>,
    time_days: f64,
    pvt: FluidProperties,
    scal: RockFluidProps,
    pc: CapillaryPressure,
    gravity_enabled: bool,
    max_sat_change_per_step: f64,
    max_pressure_change_per_step: f64,
    max_well_rate_change_fraction: f64,
    rate_controlled_wells: bool,
    injector_rate_controlled: bool,
    producer_rate_controlled: bool,
    injector_enabled: bool,
    target_injector_rate_m3_day: f64,
    target_injector_surface_rate_m3_day: Option<f64>,
    well_bhp_min: f64,
    well_bhp_max: f64,
    last_solver_warning: String,
    cumulative_injection_m3: f64,
    cumulative_production_m3: f64,
    pub cumulative_mb_error_m3: f64,
    pub cumulative_mb_gas_error_m3: f64,
    target_producer_rate_m3_day: f64,
    target_producer_surface_rate_m3_day: Option<f64>,
    rock_compressibility: f64,
    depth_reference_m: f64,
    b_o: f64,
    b_w: f64,
    rate_history: Vec<TimePointRates>,
    pub(crate) sat_gas: Vec<f64>,
    pub(crate) scal_3p: Option<RockFluidPropsThreePhase>,
    pub(crate) pc_og: Option<GasOilCapillaryPressure>,
    pub(crate) three_phase_mode: bool,
    pub(crate) injected_fluid: InjectedFluid,
    pub(crate) mu_g: f64,
    pub(crate) c_g: f64,
    pub(crate) rho_g: f64,
    pub(crate) pvt_table: Option<pvt::PvtTable>,
    pub(crate) rs: Vec<f64>,
    pub(crate) gas_redissolution_enabled: bool,
}

impl ReservoirSimulator {
    pub(crate) fn get_mu_o(&self, p: f64) -> f64 {
        if let Some(table) = &self.pvt_table {
            table.interpolate(p).mu_o_cp
        } else {
            self.pvt.mu_o
        }
    }

    pub(crate) fn get_mu_o_cell(&self, id: usize, p: f64) -> f64 {
        if let Some(table) = &self.pvt_table {
            if self.three_phase_mode {
                let (_, mu) = table.interpolate_oil(p, self.rs[id]);
                return mu;
            }
            table.interpolate(p).mu_o_cp
        } else {
            self.pvt.mu_o
        }
    }

    pub(crate) fn get_mu_w(&self, _p: f64) -> f64 {
        self.pvt.mu_w
    }

    pub(crate) fn get_mu_g(&self, p: f64) -> f64 {
        if let Some(table) = &self.pvt_table {
            table.interpolate(p).mu_g_cp
        } else {
            self.mu_g
        }
    }

    pub(crate) fn get_c_o(&self, p: f64) -> f64 {
        if let Some(table) = &self.pvt_table {
            let dp = 1.0;
            let p_minus = if p > dp { p - dp } else { 0.0 };
            let b1 = table.interpolate(p_minus).bo_m3m3;
            let b2 = table.interpolate(p + dp).bo_m3m3;
            let bo = table.interpolate(p).bo_m3m3;
            if bo > 1e-12 {
                let derived_c_o = (-1.0 / bo) * (b2 - b1) / (2.0 * dp);
                if derived_c_o.is_finite() && derived_c_o > 0.0 {
                    derived_c_o.max(self.pvt.c_o)
                } else {
                    self.pvt.c_o
                }
            } else {
                self.pvt.c_o
            }
        } else {
            self.pvt.c_o
        }
    }

    pub(crate) fn get_c_g(&self, p: f64) -> f64 {
        if let Some(table) = &self.pvt_table {
            let dp = 1.0;
            let p_minus = if p > dp { p - dp } else { 0.0 };
            let b1 = table.interpolate(p_minus).bg_m3m3;
            let b2 = table.interpolate(p + dp).bg_m3m3;
            let bg = table.interpolate(p).bg_m3m3;
            if bg > 1e-12 {
                (-1.0 / bg) * (b2 - b1) / (2.0 * dp)
            } else {
                self.c_g
            }
        } else {
            self.c_g
        }
    }

    pub(crate) fn get_c_o_effective(&self, p: f64, rs_cell: f64) -> f64 {
        if let Some(table) = &self.pvt_table {
            let rs_sat = table.interpolate(p).rs_m3m3;
            let c_sat = self.saturated_c_o_eff(table, p);

            if rs_cell < rs_sat - 1e-6 {
                let c_unsat = self.pvt.c_o;
                let p_b = table.bubble_point_pressure(rs_cell);
                let distance = p - p_b;
                let margin = self.max_pressure_change_per_step;

                if distance > 0.0 && distance < margin && c_sat > c_unsat {
                    let t = 1.0 - distance / margin;
                    let blend = t * t;
                    return c_unsat + blend * (c_sat - c_unsat);
                }
                return c_unsat;
            }

            c_sat
        } else {
            self.pvt.c_o
        }
    }

    fn saturated_c_o_eff(&self, table: &crate::pvt::PvtTable, p: f64) -> f64 {
        let dp = 1.0;
        let p_lo = (p - dp).max(0.0);
        let row_lo = table.interpolate(p_lo);
        let row_hi = table.interpolate(p + dp);
        let row_mid = table.interpolate(p);

        let bo = row_mid.bo_m3m3;
        let bg = row_mid.bg_m3m3;
        if bo > 1e-12 {
            let dbo_dp = (row_hi.bo_m3m3 - row_lo.bo_m3m3) / (2.0 * dp);
            let c_o = -dbo_dp / bo;

            let drs_dp = (row_hi.rs_m3m3 - row_lo.rs_m3m3) / (2.0 * dp);
            let c_dg = if bg > 0.0 { (bg / bo) * drs_dp } else { 0.0 };

            let c_eff = c_o + c_dg;
            if c_eff.is_finite() && c_eff > 0.0 {
                return c_eff;
            }
        }
        self.pvt.c_o
    }

    pub(crate) fn get_b_o_cell(&self, id: usize, p: f64) -> f64 {
        if let Some(table) = &self.pvt_table {
            if self.three_phase_mode {
                let (bo, _) = table.interpolate_oil(p, self.rs[id]);
                return bo;
            }
            table.interpolate(p).bo_m3m3
        } else {
            self.b_o
        }
    }

    pub(crate) fn get_rho_o_cell(&self, id: usize, p: f64) -> f64 {
        if let Some(table) = &self.pvt_table {
            let rs = self.rs[id];
            let (bo, _) = table.interpolate_oil(p, rs);
            (self.pvt.rho_o + rs * self.rho_g) / bo
        } else {
            self.pvt.rho_o
        }
    }

    pub(crate) fn get_rho_o(&self, p: f64) -> f64 {
        if let Some(table) = &self.pvt_table {
            let row = table.interpolate(p);
            (self.pvt.rho_o + row.rs_m3m3 * self.rho_g) / row.bo_m3m3
        } else {
            self.pvt.rho_o
        }
    }

    pub(crate) fn get_rho_g(&self, p: f64) -> f64 {
        if let Some(table) = &self.pvt_table {
            self.rho_g / table.interpolate(p).bg_m3m3
        } else {
            self.rho_g
        }
    }

    pub(crate) fn get_rho_w(&self, _p: f64) -> f64 {
        self.pvt.rho_w
    }

    pub(crate) fn get_b_g(&self, p: f64) -> f64 {
        if let Some(table) = &self.pvt_table {
            table.interpolate(p).bg_m3m3
        } else {
            1.0
        }
    }

    pub(crate) fn dz_at(&self, id: usize) -> f64 {
        let k = id / (self.nx * self.ny);
        self.dz[k]
    }

    pub(crate) fn average_reservoir_pressure_pv_weighted(&self) -> f64 {
        let mut weighted_pressure_sum = 0.0;
        let mut pore_volume_sum = 0.0;

        for id in 0..self.nx * self.ny * self.nz {
            let pore_volume = self.pore_volume_m3(id);
            if pore_volume <= 0.0 || !pore_volume.is_finite() {
                continue;
            }
            weighted_pressure_sum += self.pressure[id] * pore_volume;
            pore_volume_sum += pore_volume;
        }

        if pore_volume_sum > 0.0 {
            weighted_pressure_sum / pore_volume_sum
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::well_control::{ProducerControlState, ResolvedWellControl, WellControlDecision};

    struct BuckleyCase {
        name: &'static str,
        nx: usize,
        permeability_md: f64,
        dt_days: f64,
        max_steps: usize,
        injector_bhp: f64,
        producer_bhp: f64,
        s_wc: f64,
        s_or: f64,
        n_w: f64,
        n_o: f64,
        mu_w: f64,
        mu_o: f64,
        breakthrough_watercut: f64,
        rel_tol_breakthrough_pv: f64,
    }

    struct BuckleyMetrics {
        breakthrough_pv: f64,
        reference_breakthrough_pv: f64,
    }

    #[test]
    fn black_oil_compressibility_falls_back_when_bo_slope_goes_negative() {
        let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        sim.pvt.c_o = 1e-5;
        sim.pvt_table = Some(pvt::PvtTable::new(
            vec![
                pvt::PvtRow {
                    p_bar: 100.0,
                    rs_m3m3: 5.0,
                    bo_m3m3: 1.05,
                    mu_o_cp: 1.5,
                    bg_m3m3: 0.01,
                    mu_g_cp: 0.02,
                },
                pvt::PvtRow {
                    p_bar: 150.0,
                    rs_m3m3: 15.0,
                    bo_m3m3: 1.12,
                    mu_o_cp: 1.2,
                    bg_m3m3: 0.006,
                    mu_g_cp: 0.025,
                },
                pvt::PvtRow {
                    p_bar: 200.0,
                    rs_m3m3: 15.0,
                    bo_m3m3: 1.11944,
                    mu_o_cp: 1.3,
                    bg_m3m3: 0.0045,
                    mu_g_cp: 0.03,
                },
            ],
            sim.pvt.c_o,
        ));

        let c_o_below_bubble_point = sim.get_c_o(149.0);
        let c_o_above_bubble_point = sim.get_c_o(175.0);

        assert!(c_o_below_bubble_point.is_finite());
        assert!(c_o_above_bubble_point.is_finite());
        assert_eq!(c_o_below_bubble_point, sim.pvt.c_o);
        assert!(c_o_above_bubble_point >= sim.pvt.c_o);
    }

    #[test]
    fn effective_oil_compressibility_includes_dissolved_gas_below_bubble_point() {
        // Below bubble point, Bo increases with pressure and Rs increases with pressure.
        // The dissolved gas term (Bg/Bo)·dRs/dp should dominate, giving c_o_eff >> c_o_base.
        let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        sim.pvt.c_o = 1e-5;
        sim.rho_g = 0.9; // surface gas density for density test
        sim.pvt_table = Some(pvt::PvtTable::new(
            vec![
                pvt::PvtRow {
                    p_bar: 100.0,
                    rs_m3m3: 5.0,
                    bo_m3m3: 1.05,
                    mu_o_cp: 1.5,
                    bg_m3m3: 0.01,
                    mu_g_cp: 0.02,
                },
                pvt::PvtRow {
                    p_bar: 150.0,
                    rs_m3m3: 15.0,
                    bo_m3m3: 1.12,
                    mu_o_cp: 1.2,
                    bg_m3m3: 0.006,
                    mu_g_cp: 0.025,
                },
                pvt::PvtRow {
                    p_bar: 200.0,
                    rs_m3m3: 15.0,
                    bo_m3m3: 1.119,
                    mu_o_cp: 1.3,
                    bg_m3m3: 0.0045,
                    mu_g_cp: 0.03,
                },
            ],
            sim.pvt.c_o,
        ));

        // Below bubble point (125 bar): Bo is rising, Rs is rising → dissolved gas term active
        // Pass saturated Rs so the cell is recognized as saturated (gas liberation active)
        let rs_sat_125 = sim.pvt_table.as_ref().unwrap().interpolate(125.0).rs_m3m3;
        let c_eff_below = sim.get_c_o_effective(125.0, rs_sat_125);
        let c_o_below = sim.get_c_o(125.0);
        assert!(c_eff_below.is_finite());
        assert!(c_eff_below > 0.0);
        // Effective compressibility should be much larger than fallback c_o due to dissolved gas
        assert!(
            c_eff_below > c_o_below,
            "c_o_effective ({c_eff_below}) must exceed c_o ({c_o_below}) below bubble point"
        );

        // Above bubble point (175 bar): Rs is constant → dissolved gas term ≈ 0
        let rs_sat_175 = sim.pvt_table.as_ref().unwrap().interpolate(175.0).rs_m3m3;
        let c_eff_above = sim.get_c_o_effective(175.0, rs_sat_175);
        let c_o_above = sim.get_c_o(175.0);
        assert!(c_eff_above.is_finite());
        assert!(c_eff_above > 0.0);
        // Above bubble point, effective ≈ standard (both come from Bo decline with p)
        assert!(
            (c_eff_above - c_o_above).abs() / c_o_above < 0.5,
            "c_o_effective ({c_eff_above}) should be close to c_o ({c_o_above}) above bubble point"
        );

        // Density should include dissolved gas mass: ρ = (ρ_o_sc + Rs·ρ_g_sc) / Bo
        let rho = sim.get_rho_o(125.0);
        let row = sim.pvt_table.as_ref().unwrap().interpolate(125.0);
        let expected = (sim.pvt.rho_o + row.rs_m3m3 * sim.rho_g) / row.bo_m3m3;
        assert!(
            (rho - expected).abs() < 1e-6,
            "ρ_o ({rho}) should include dissolved gas ({expected})"
        );
        // With dissolved gas, density must exceed simple ρ_o_sc / Bo
        let rho_simple = sim.pvt.rho_o / row.bo_m3m3;
        assert!(
            rho > rho_simple,
            "ρ_o with Rs ({rho}) must exceed dead-oil density ({rho_simple})"
        );
    }

    #[test]
    fn bubble_point_blending_smooths_compressibility_near_bubble_point() {
        // Use SPE1-like PVT where the dissolved gas term dominates (large dRs/dp,
        // small dBo/dp relative to (Bg/Bo)*dRs/dp), giving c_o_eff >> c_o.
        let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        sim.pvt.c_o = 2e-4; // undersaturated c_o (SPE1-like)
        sim.max_pressure_change_per_step = 50.0;
        sim.pvt_table = Some(pvt::PvtTable::new(
            vec![
                pvt::PvtRow {
                    p_bar: 100.0,
                    rs_m3m3: 50.0,
                    bo_m3m3: 1.30,
                    mu_o_cp: 0.7,
                    bg_m3m3: 0.010,
                    mu_g_cp: 0.020,
                },
                pvt::PvtRow {
                    p_bar: 200.0,
                    rs_m3m3: 150.0,
                    bo_m3m3: 1.50,
                    mu_o_cp: 0.5,
                    bg_m3m3: 0.005,
                    mu_g_cp: 0.025,
                },
                pvt::PvtRow {
                    p_bar: 300.0,
                    rs_m3m3: 250.0,
                    bo_m3m3: 1.70,
                    mu_o_cp: 0.4,
                    bg_m3m3: 0.004,
                    mu_g_cp: 0.030,
                },
            ],
            sim.pvt.c_o,
        ));

        // Cell with Rs=150: bubble point = 200 bar
        let rs_cell = 150.0;
        let c_unsat = sim.pvt.c_o;

        // Far above bubble point (250 bar, distance=50 = margin) → pure undersaturated c_o
        let c_far = sim.get_c_o_effective(250.0, rs_cell);
        assert!(
            (c_far - c_unsat).abs() < 1e-9,
            "Far from bubble point: should equal c_o={c_unsat}, got {c_far}"
        );

        // Just inside the blending zone (225 bar, distance=25 = 0.5×margin)
        let c_near = sim.get_c_o_effective(225.0, rs_cell);
        assert!(
            c_near > c_unsat,
            "Near bubble point: should exceed c_o={c_unsat}, got {c_near}"
        );

        // Very close to bubble point (202 bar, distance=2)
        let c_close = sim.get_c_o_effective(202.0, rs_cell);
        assert!(
            c_close > c_near,
            "Closer to BP: c_o_eff({c_close}) should exceed value at 225 bar ({c_near})"
        );

        // At bubble point (200 bar, rs_cell == rs_sat) → fully saturated
        let c_at_bp = sim.get_c_o_effective(200.0, rs_cell);
        assert!(
            c_at_bp > c_unsat,
            "At bubble point: should use saturated c_o_eff={c_at_bp} > c_o={c_unsat}"
        );

        // Blending should be monotonically increasing toward bubble point
        assert!(
            c_close > c_near && c_near > c_far,
            "Compressibility should increase monotonically toward bubble point"
        );
    }

    #[test]
    fn producer_surface_rate_target_converts_using_oil_fraction_and_bo() {
        let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        sim.set_initial_pressure(200.0);
        sim.set_initial_saturation(0.2);
        sim.set_well_control_modes("pressure".to_string(), "rate".to_string());
        sim.set_target_well_surface_rates(0.0, 100.0).unwrap();
        sim.add_well(0, 0, 0, 100.0, 0.1, 0.0, false).unwrap();

        let well = sim.wells.first().unwrap();
        let q_target = sim.target_rate_m3_day(well, 200.0).unwrap();
        let krw = sim.scal.k_rw(sim.sat_water[0]);
        let kro = sim.scal.k_ro(sim.sat_water[0]);
        let lambda_w = krw / sim.get_mu_w(200.0);
        let lambda_o = kro / sim.get_mu_o(200.0);
        let oil_fraction = lambda_o / (lambda_w + lambda_o);
        let expected = 100.0 * sim.get_b_o_cell(0, 200.0) / oil_fraction;

        assert!((q_target - expected).abs() < 1e-9);
    }

    #[test]
    fn producer_surface_rate_target_uses_same_layer_neighborhood_sampling() {
        let mut sim = ReservoirSimulator::new(2, 2, 1, 0.2);
        sim.set_initial_pressure(200.0);
        sim.set_initial_saturation(0.12);
        sim.set_three_phase_rel_perm_props(
            0.12, 0.12, 0.04, 0.04, 0.18, 2.0, 2.5, 1.5, 1e-5, 1.0, 0.984,
        )
        .unwrap();
        sim.set_three_phase_mode_enabled(true);
        sim.set_well_control_modes("pressure".to_string(), "rate".to_string());
        sim.set_target_well_surface_rates(0.0, 100.0).unwrap();
        sim.add_well(1, 1, 0, 100.0, 0.1, 0.0, false).unwrap();

        let producer_id = sim.idx(1, 1, 0);
        let gas_hit_id = producer_id;
        let left_id = sim.idx(0, 1, 0);
        let down_id = sim.idx(1, 0, 0);
        let diag_id = sim.idx(0, 0, 0);

        sim.sat_water = vec![0.12; 4];
        sim.sat_gas[gas_hit_id] = 0.25;
        sim.sat_oil[gas_hit_id] = 1.0 - sim.sat_water[gas_hit_id] - sim.sat_gas[gas_hit_id];
        for id in [left_id, down_id, diag_id] {
            sim.sat_gas[id] = 0.0;
            sim.sat_oil[id] = 1.0 - sim.sat_water[id];
        }

        let well = sim.wells.first().unwrap();
        let q_target = sim.target_rate_m3_day(well, 200.0).unwrap();

        let neighbor_ids = [gas_hit_id, left_id, down_id, diag_id];
        let mut lambda_o_sum = 0.0;
        let mut lambda_total_sum = 0.0;
        for id in neighbor_ids {
            let sw = sim.sat_water[id];
            let sg = sim.sat_gas[id];
            let scal = sim.scal_3p.as_ref().unwrap();
            let lam_w = scal.k_rw(sw) / sim.get_mu_w(200.0);
            let lam_o = scal.k_ro_stone2(sw, sg) / sim.get_mu_o_cell(id, 200.0);
            let lam_g = scal.k_rg(sg) / sim.get_mu_g(200.0);
            lambda_o_sum += lam_o;
            lambda_total_sum += lam_w + lam_o + lam_g;
        }

        let sampled_oil_fraction = lambda_o_sum / lambda_total_sum;
        let expected = 100.0 * sim.get_b_o_cell(producer_id, 200.0) / sampled_oil_fraction;

        let local_scal = sim.scal_3p.as_ref().unwrap();
        let local_lam_w = local_scal.k_rw(sim.sat_water[producer_id]) / sim.get_mu_w(200.0);
        let local_lam_o = local_scal
            .k_ro_stone2(sim.sat_water[producer_id], sim.sat_gas[producer_id])
            / sim.get_mu_o_cell(producer_id, 200.0);
        let local_lam_g = local_scal.k_rg(sim.sat_gas[producer_id]) / sim.get_mu_g(200.0);
        let local_oil_fraction = local_lam_o / (local_lam_w + local_lam_o + local_lam_g);
        let local_only = 100.0 * sim.get_b_o_cell(producer_id, 200.0) / local_oil_fraction;

        assert!((q_target - expected).abs() < 1e-9);
        assert!(q_target < local_only,
            "same-layer neighborhood sampling should request less total reservoir withdrawal than local-only sampling when neighboring cells remain oil-rich");
    }

    #[test]
    fn producer_reporting_uses_same_sampled_near_well_mixture() {
        use nalgebra::DVector;

        let mut sim = ReservoirSimulator::new(2, 2, 1, 0.2);
        sim.set_initial_pressure(200.0);
        sim.set_initial_saturation(0.12);
        sim.set_three_phase_rel_perm_props(
            0.12, 0.12, 0.04, 0.04, 0.18, 2.0, 2.5, 1.5, 1e-5, 1.0, 0.984,
        )
        .unwrap();
        sim.set_three_phase_mode_enabled(true);
        sim.set_well_control_modes("pressure".to_string(), "rate".to_string());
        sim.add_well(1, 1, 0, 100.0, 0.1, 0.0, false).unwrap();

        let producer_id = sim.idx(1, 1, 0);
        let left_id = sim.idx(0, 1, 0);
        let down_id = sim.idx(1, 0, 0);
        let diag_id = sim.idx(0, 0, 0);

        sim.sat_water = vec![0.12; 4];
        sim.sat_gas[producer_id] = 0.25;
        sim.sat_oil[producer_id] = 1.0 - sim.sat_water[producer_id] - sim.sat_gas[producer_id];
        for id in [left_id, down_id, diag_id] {
            sim.sat_gas[id] = 0.0;
            sim.sat_oil[id] = 1.0 - sim.sat_water[id];
        }

        let (sample_fw, sample_fo, sample_fg) =
            sim.producer_control_phase_fractions_for_pressures(&sim.wells[0], &sim.pressure);
        let cached_state = ProducerControlState {
            water_fraction: sample_fw,
            oil_fraction: sample_fo,
            gas_fraction: sample_fg,
            oil_fvf: sim
                .get_b_o_cell(producer_id, sim.pressure[producer_id])
                .max(1e-9),
            gas_fvf: sim.get_b_g(sim.pressure[producer_id]).max(1e-9),
            rs_sm3_sm3: sim.rs[producer_id],
        };

        let controls = vec![Some(ResolvedWellControl {
            decision: WellControlDecision::Rate { q_m3_day: 1000.0 },
            bhp_limited: false,
            producer_state: Some(cached_state),
        })];

        sim.update_saturations_and_pressure(
            &DVector::from_vec(vec![150.0, 150.0, 150.0, 150.0]),
            &vec![0.0, 0.0, 0.0, 0.0],
            &vec![0.0, 0.0, 0.0, 0.0],
            &vec![0.0, 0.0, 0.0, 0.0],
            &controls,
            1.0,
        );

        let latest = sim
            .rate_history
            .last()
            .expect("rate history should have an entry");
        let expected_oil_sc = 1000.0 * cached_state.oil_fraction / cached_state.oil_fvf;
        let expected_total_gas_sc = 1000.0 * cached_state.gas_fraction / cached_state.gas_fvf
            + expected_oil_sc * cached_state.rs_sm3_sm3;

        assert!((latest.total_production_oil - expected_oil_sc).abs() < 1e-9);
        assert!((latest.total_production_gas - expected_total_gas_sc).abs() < 1e-9);
        assert!((latest.total_production_liquid_reservoir - 1000.0).abs() < 1e-9);
        assert!(sample_fo > 0.0 && sample_fo < 1.0);
        assert!(sample_fg > 0.0);
        assert!(sample_fw >= 0.0);
    }

    #[test]
    fn rate_history_records_bhp_limited_fraction_for_rate_controlled_wells() {
        let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        sim.set_initial_pressure(200.0);
        sim.set_initial_saturation(0.2);
        sim.set_well_control_modes("pressure".to_string(), "rate".to_string());
        sim.set_target_well_rates(0.0, 5000.0).unwrap();
        sim.set_well_bhp_limits(150.0, 300.0).unwrap();
        sim.add_well(0, 0, 0, 150.0, 0.1, 0.0, false).unwrap();

        sim.step(1.0);

        let point = sim.rate_history.last().unwrap();
        assert_eq!(point.producer_bhp_limited_fraction, 1.0);
        assert_eq!(point.injector_bhp_limited_fraction, 0.0);
    }

    fn buckley_case_a(
        name: &'static str,
        nx: usize,
        dt_days: f64,
        max_steps: usize,
    ) -> BuckleyCase {
        BuckleyCase {
            name,
            nx,
            permeability_md: 2000.0,
            dt_days,
            max_steps,
            injector_bhp: 500.0,
            producer_bhp: 100.0,
            s_wc: 0.1,
            s_or: 0.1,
            n_w: 2.0,
            n_o: 2.0,
            mu_w: 0.5,
            mu_o: 1.0,
            breakthrough_watercut: 0.01,
            rel_tol_breakthrough_pv: 0.25,
        }
    }

    fn buckley_case_b(
        name: &'static str,
        nx: usize,
        dt_days: f64,
        max_steps: usize,
    ) -> BuckleyCase {
        BuckleyCase {
            name,
            nx,
            permeability_md: 2000.0,
            dt_days,
            max_steps,
            injector_bhp: 500.0,
            producer_bhp: 100.0,
            s_wc: 0.15,
            s_or: 0.15,
            n_w: 2.2,
            n_o: 2.0,
            mu_w: 0.6,
            mu_o: 1.4,
            breakthrough_watercut: 0.01,
            rel_tol_breakthrough_pv: 0.30,
        }
    }

    fn err_contains(result: Result<(), String>, expected: &str) {
        match result {
            Ok(()) => panic!("Expected error containing '{}', got Ok(())", expected),
            Err(message) => assert!(
                message.contains(expected),
                "Expected error containing '{}', got '{}'",
                expected,
                message
            ),
        }
    }

    fn total_water_volume(sim: &ReservoirSimulator) -> f64 {
        (0..sim.nx * sim.ny * sim.nz)
            .map(|i| sim.sat_water[i] * sim.pore_volume_m3(i))
            .sum()
    }

    fn corey_fractional_flow(
        s_w: f64,
        s_wc: f64,
        s_or: f64,
        n_w: f64,
        n_o: f64,
        mu_w: f64,
        mu_o: f64,
    ) -> f64 {
        let denom_sat = 1.0 - s_wc - s_or;
        if denom_sat <= 0.0 {
            return 0.0;
        }

        let s_eff_w = ((s_w - s_wc) / denom_sat).clamp(0.0, 1.0);
        let s_eff_o = ((1.0 - s_w - s_or) / denom_sat).clamp(0.0, 1.0);
        let krw = s_eff_w.powf(n_w);
        let kro = s_eff_o.powf(n_o);
        let lam_w = krw / mu_w;
        let lam_o = kro / mu_o;
        let lam_t = lam_w + lam_o;

        if lam_t <= f64::EPSILON {
            0.0
        } else {
            (lam_w / lam_t).clamp(0.0, 1.0)
        }
    }

    fn buckley_reference_breakthrough_pv(case: &BuckleyCase) -> f64 {
        let sw_init = case.s_wc;
        let mut sw_shock = sw_init;
        let mut best_slope = 0.0;
        let ds = 5e-4;
        let mut s = sw_init + ds;
        let s_max = 1.0 - case.s_or;

        while s <= s_max {
            let fw = corey_fractional_flow(
                s, case.s_wc, case.s_or, case.n_w, case.n_o, case.mu_w, case.mu_o,
            );
            let slope = fw / (s - sw_init);
            if slope > best_slope && slope.is_finite() {
                best_slope = slope;
                sw_shock = s;
            }
            s += ds;
        }

        let fw_eps = 1e-4;
        let fw_plus = corey_fractional_flow(
            (sw_shock + fw_eps).clamp(sw_init, s_max),
            case.s_wc,
            case.s_or,
            case.n_w,
            case.n_o,
            case.mu_w,
            case.mu_o,
        );
        let fw_minus = corey_fractional_flow(
            (sw_shock - fw_eps).clamp(sw_init, s_max),
            case.s_wc,
            case.s_or,
            case.n_w,
            case.n_o,
            case.mu_w,
            case.mu_o,
        );
        let dfw_dsw = (fw_plus - fw_minus) / (2.0 * fw_eps);

        if dfw_dsw <= f64::EPSILON {
            f64::INFINITY
        } else {
            1.0 / dfw_dsw
        }
    }

    fn run_buckley_case(case: &BuckleyCase) -> BuckleyMetrics {
        let mut sim = ReservoirSimulator::new(case.nx, 1, 1, 0.2);
        sim.set_rel_perm_props(case.s_wc, case.s_or, case.n_w, case.n_o, 1.0, 1.0)
            .unwrap();
        sim.set_initial_saturation(case.s_wc);
        sim.set_permeability_random_seeded(case.permeability_md, case.permeability_md, 42)
            .unwrap();
        sim.set_stability_params(0.05, 75.0, 0.75);
        sim.pc.p_entry = 0.0;
        sim.pvt.mu_w = case.mu_w;
        sim.pvt.mu_o = case.mu_o;

        sim.add_well(0, 0, 0, case.injector_bhp, 0.1, 0.0, true)
            .unwrap();
        sim.add_well(case.nx - 1, 0, 0, case.producer_bhp, 0.1, 0.0, false)
            .unwrap();

        let total_pv = (0..sim.nx * sim.ny * sim.nz)
            .map(|i| sim.pore_volume_m3(i))
            .sum::<f64>();

        let mut cumulative_injection = 0.0;
        let mut previous_time = 0.0;
        let mut breakthrough_pv = None;

        for _ in 0..case.max_steps {
            sim.step(case.dt_days);
            let point = sim
                .rate_history
                .last()
                .expect("rate history should have entries");
            let dt = point.time - previous_time;
            previous_time = point.time;

            cumulative_injection += point.total_injection.max(0.0) * dt;

            if point.total_production_liquid > 1e-9 {
                let water_rate =
                    (point.total_production_liquid - point.total_production_oil).max(0.0);
                let watercut = (water_rate / point.total_production_liquid).clamp(0.0, 1.0);
                if watercut >= case.breakthrough_watercut {
                    breakthrough_pv = Some(cumulative_injection / total_pv);
                    break;
                }
            }
        }

        let breakthrough_pv = breakthrough_pv.unwrap_or_else(|| {
            panic!(
                "{} did not reach breakthrough (watercut >= {}) in {} steps",
                case.name, case.breakthrough_watercut, case.max_steps
            )
        });

        BuckleyMetrics {
            breakthrough_pv,
            reference_breakthrough_pv: buckley_reference_breakthrough_pv(case),
        }
    }

    #[test]
    fn saturation_stays_within_physical_bounds() {
        let mut sim = ReservoirSimulator::new(5, 1, 1, 0.2);
        sim.add_well(0, 0, 0, 500.0, 0.1, 0.0, true).unwrap();
        sim.add_well(4, 0, 0, 100.0, 0.1, 0.0, false).unwrap();

        for _ in 0..20 {
            sim.step(0.5);
        }

        let sw_min = sim.scal.s_wc;
        let sw_max = 1.0 - sim.scal.s_or;

        for i in 0..sim.nx * sim.ny * sim.nz {
            assert!(sim.sat_water[i] >= sw_min - 1e-9);
            assert!(sim.sat_water[i] <= sw_max + 1e-9);
            assert!(sim.sat_oil[i] >= -1e-9);
            assert!(sim.sat_oil[i] <= 1.0 + 1e-9);
            assert!((sim.sat_water[i] + sim.sat_oil[i] - 1.0).abs() < 1e-8);
        }
    }

    #[test]
    fn water_mass_balance_sanity_without_wells() {
        let mut sim = ReservoirSimulator::new(4, 4, 1, 0.2);
        let water_before = total_water_volume(&sim);

        sim.step(1.0);

        let water_after = total_water_volume(&sim);
        assert!((water_after - water_before).abs() < 1e-6);
    }

    #[test]
    fn adaptive_timestep_produces_multiple_substeps_for_strong_flow() {
        let mut sim = ReservoirSimulator::new(3, 1, 1, 0.2);
        sim.set_permeability_random(100_000.0, 100_000.0).unwrap();
        sim.set_stability_params(0.01, 75.0, 0.75);
        sim.add_well(0, 0, 0, 700.0, 0.1, 0.0, true).unwrap();
        sim.add_well(2, 0, 0, 50.0, 0.1, 0.0, false).unwrap();

        sim.step(30.0);

        assert!(sim.rate_history.len() > 1);
        assert!(sim.time_days > 0.0);
        assert!((sim.time_days - 30.0).abs() < 1e-9);
    }

    #[test]
    fn multiple_wells_in_same_block_keep_rates_finite() {
        let mut sim = ReservoirSimulator::new(4, 1, 1, 0.2);
        sim.add_well(0, 0, 0, 600.0, 0.1, 0.0, true).unwrap();
        sim.add_well(0, 0, 0, 550.0, 0.1, 0.0, true).unwrap();
        sim.add_well(3, 0, 0, 120.0, 0.1, 0.0, false).unwrap();

        for _ in 0..12 {
            sim.step(0.5);
        }

        assert!(!sim.rate_history.is_empty());
        let latest = sim.rate_history.last().unwrap();
        assert!(latest.total_injection.is_finite());
        assert!(latest.total_production_liquid.is_finite());
        assert!(latest.total_production_oil.is_finite());

        for i in 0..sim.nx * sim.ny * sim.nz {
            assert!(sim.pressure[i].is_finite());
            assert!(sim.sat_water[i].is_finite());
            assert!(sim.sat_oil[i].is_finite());
        }
    }

    #[test]
    fn out_of_bounds_well_is_rejected_without_state_change() {
        let mut sim = ReservoirSimulator::new(2, 2, 1, 0.2);
        let wells_before = sim.wells.len();

        let result = sim.add_well(2, 0, 0, 250.0, 0.1, 0.0, false);
        err_contains(result, "out of bounds");

        assert_eq!(sim.wells.len(), wells_before);
    }

    #[test]
    fn stability_extremes_produce_finite_state() {
        let mut sim_loose = ReservoirSimulator::new(3, 1, 1, 0.2);
        sim_loose.set_stability_params(1.0, 75.0, 0.75);
        sim_loose
            .set_permeability_random(20_000.0, 20_000.0)
            .unwrap();
        sim_loose.add_well(0, 0, 0, 650.0, 0.1, 0.0, true).unwrap();
        sim_loose.add_well(2, 0, 0, 50.0, 0.1, 0.0, false).unwrap();
        sim_loose.step(5.0);

        let mut sim_tight = ReservoirSimulator::new(3, 1, 1, 0.2);
        sim_tight.set_stability_params(0.01, 75.0, 0.75);
        sim_tight
            .set_permeability_random(20_000.0, 20_000.0)
            .unwrap();
        sim_tight.add_well(0, 0, 0, 650.0, 0.1, 0.0, true).unwrap();
        sim_tight.add_well(2, 0, 0, 50.0, 0.1, 0.0, false).unwrap();
        sim_tight.step(5.0);

        for sim in [&sim_loose, &sim_tight] {
            for i in 0..sim.nx * sim.ny * sim.nz {
                assert!(sim.pressure[i].is_finite());
                assert!(sim.sat_water[i].is_finite());
                assert!(sim.sat_oil[i].is_finite());
            }
            assert!(sim.time_days > 0.0);
            assert!(sim.time_days <= 5.0);
            assert!(!sim.rate_history.is_empty());
        }

        assert!(sim_tight.rate_history.len() >= sim_loose.rate_history.len());
    }

    #[test]
    fn api_contract_rejects_invalid_relperm_parameters() {
        let mut sim = ReservoirSimulator::new(2, 2, 1, 0.2);
        err_contains(
            sim.set_rel_perm_props(0.6, 0.5, 2.0, 2.0, 1.0, 1.0),
            "must be < 1.0",
        );
        err_contains(
            sim.set_rel_perm_props(0.1, 0.1, 0.0, 2.0, 1.0, 1.0),
            "must be positive",
        );
        err_contains(
            sim.set_rel_perm_props(f64::NAN, 0.1, 2.0, 2.0, 1.0, 1.0),
            "finite numbers",
        );
    }

    #[test]
    fn api_contract_allows_zero_water_relperm_endpoint() {
        let mut sim = ReservoirSimulator::new(2, 2, 1, 0.2);
        sim.set_rel_perm_props(0.1, 0.1, 2.0, 2.0, 0.0, 1.0)
            .expect("k_rw_max = 0 should be accepted for immobile-water cases");
    }

    #[test]
    fn api_contract_rejects_invalid_density_inputs() {
        let mut sim = ReservoirSimulator::new(2, 2, 1, 0.2);
        err_contains(sim.set_fluid_densities(-800.0, 1000.0), "must be positive");
        err_contains(sim.set_fluid_densities(800.0, f64::NAN), "finite numbers");
    }

    #[test]
    fn api_contract_rejects_invalid_capillary_inputs() {
        let mut sim = ReservoirSimulator::new(2, 2, 1, 0.2);
        err_contains(sim.set_capillary_params(-1.0, 2.0), "non-negative");
        err_contains(sim.set_capillary_params(5.0, 0.0), "positive");
        err_contains(sim.set_capillary_params(f64::NAN, 2.0), "finite numbers");
    }

    #[test]
    fn gravity_toggle_builds_hydrostatic_vertical_gradient() {
        let mut sim_no_g = ReservoirSimulator::new(1, 1, 2, 0.2);
        sim_no_g
            .set_permeability_random_seeded(50_000.0, 50_000.0, 42)
            .unwrap();
        sim_no_g.set_initial_pressure(300.0);
        sim_no_g.set_initial_saturation(0.9);
        sim_no_g.pc.p_entry = 0.0;
        sim_no_g.set_gravity_enabled(false);
        sim_no_g.step(2.0);

        let p_top_no_g = sim_no_g.pressure[sim_no_g.idx(0, 0, 0)];
        let p_bot_no_g = sim_no_g.pressure[sim_no_g.idx(0, 0, 1)];

        let mut sim_g = ReservoirSimulator::new(1, 1, 2, 0.2);
        sim_g
            .set_permeability_random_seeded(50_000.0, 50_000.0, 42)
            .unwrap();
        sim_g.set_initial_pressure(300.0);
        sim_g.set_initial_saturation(0.9);
        sim_g.pc.p_entry = 0.0;
        sim_g.set_gravity_enabled(true);
        sim_g.step(2.0);

        let p_top_g = sim_g.pressure[sim_g.idx(0, 0, 0)];
        let p_bot_g = sim_g.pressure[sim_g.idx(0, 0, 1)];

        assert!((p_bot_no_g - p_top_no_g).abs() < 1e-5);
        assert!(p_bot_g > p_top_g);
    }

    #[test]
    fn hydrostatic_initial_gradient_stays_quieter_with_gravity_enabled() {
        let initial_sw = 0.9;

        let mut sim_g = ReservoirSimulator::new(1, 1, 2, 0.2);
        sim_g
            .set_permeability_random_seeded(80_000.0, 80_000.0, 7)
            .unwrap();
        sim_g.set_initial_saturation(initial_sw);
        sim_g.pc.p_entry = 0.0;
        sim_g.set_fluid_densities(800.0, 1000.0).unwrap();
        sim_g.set_gravity_enabled(true);

        let hydro_dp_bar = sim_g.pvt.rho_w * 9.80665 * sim_g.dz[0] * 1e-5;
        let top_id_g = sim_g.idx(0, 0, 0);
        let bot_id_g = sim_g.idx(0, 0, 1);
        sim_g.pressure[top_id_g] = 300.0;
        sim_g.pressure[bot_id_g] = 300.0 + hydro_dp_bar;
        sim_g.step(5.0);
        let sw_change_top_g = (sim_g.sat_water[top_id_g] - initial_sw).abs();

        let mut sim_no_g = ReservoirSimulator::new(1, 1, 2, 0.2);
        sim_no_g
            .set_permeability_random_seeded(80_000.0, 80_000.0, 7)
            .unwrap();
        sim_no_g.set_initial_saturation(initial_sw);
        sim_no_g.pc.p_entry = 0.0;
        sim_no_g.set_fluid_densities(800.0, 1000.0).unwrap();
        sim_no_g.set_gravity_enabled(false);

        let top_id_no_g = sim_no_g.idx(0, 0, 0);
        let bot_id_no_g = sim_no_g.idx(0, 0, 1);
        sim_no_g.pressure[top_id_no_g] = 300.0;
        sim_no_g.pressure[bot_id_no_g] = 300.0 + hydro_dp_bar;
        sim_no_g.step(5.0);
        let sw_change_top_no_g = (sim_no_g.sat_water[top_id_no_g] - initial_sw).abs();

        assert!(sw_change_top_g <= sw_change_top_no_g);
    }

    #[test]
    fn api_contract_rejects_invalid_permeability_inputs() {
        let mut sim = ReservoirSimulator::new(2, 2, 2, 0.2);
        err_contains(
            sim.set_permeability_random(200.0, 50.0),
            "cannot exceed max",
        );
        err_contains(
            sim.set_permeability_random_seeded(-1.0, 100.0, 123),
            "must be positive",
        );
        err_contains(
            sim.set_permeability_per_layer(vec![100.0], vec![100.0, 120.0], vec![10.0, 12.0]),
            "length equal to nz",
        );
        err_contains(
            sim.set_permeability_per_layer(vec![100.0, 120.0], vec![100.0, 120.0], vec![0.0, 12.0]),
            "must be positive",
        );
    }

    #[test]
    fn pressure_resolve_on_substep_produces_physical_results() {
        // Setup: high permeability + large dt forces stable_dt_factor < 1.0
        // triggering the re-solve path in step_internal
        let mut sim = ReservoirSimulator::new(5, 1, 1, 0.2);
        sim.set_permeability_random_seeded(100_000.0, 100_000.0, 42)
            .unwrap();
        sim.set_stability_params(0.02, 50.0, 0.5);
        sim.pc.p_entry = 0.0;
        sim.add_well(0, 0, 0, 600.0, 0.1, 0.0, true).unwrap();
        sim.add_well(4, 0, 0, 100.0, 0.1, 0.0, false).unwrap();

        // Large dt to force sub-stepping
        sim.step(20.0);

        // Must have sub-stepped (multiple rate history entries)
        assert!(
            sim.rate_history.len() > 1,
            "Expected sub-stepping, got {} entries",
            sim.rate_history.len()
        );

        // All state must be finite and physical
        for i in 0..sim.nx * sim.ny * sim.nz {
            assert!(
                sim.pressure[i].is_finite(),
                "Pressure not finite at cell {}",
                i
            );
            assert!(sim.sat_water[i].is_finite(), "Sw not finite at cell {}", i);
            assert!(
                sim.sat_water[i] >= sim.scal.s_wc - 1e-9,
                "Sw below s_wc at cell {}",
                i
            );
            assert!(
                sim.sat_water[i] <= 1.0 - sim.scal.s_or + 1e-9,
                "Sw above 1-s_or at cell {}",
                i
            );
            assert!((sim.sat_water[i] + sim.sat_oil[i] - 1.0).abs() < 1e-8);
        }

        // Pressure should remain within physical range (bounded by well BHPs)
        for i in 0..sim.nx * sim.ny * sim.nz {
            assert!(
                sim.pressure[i] > 50.0 && sim.pressure[i] < 700.0,
                "Pressure {} at cell {} outside physical range",
                sim.pressure[i],
                i
            );
        }

        // Material balance: each rate entry should have finite MB error
        for entry in &sim.rate_history {
            assert!(
                entry.material_balance_error_m3.is_finite(),
                "MB error not finite"
            );
        }
    }

    #[test]
    fn benchmark_like_substepping_completes_requested_dt() {
        let mut sim = ReservoirSimulator::new(24, 1, 1, 0.2);
        sim.set_rel_perm_props(0.1, 0.1, 2.0, 2.0, 1.0, 1.0)
            .unwrap();
        sim.set_initial_saturation(0.1);
        sim.set_permeability_random_seeded(2000.0, 2000.0, 42)
            .unwrap();
        sim.set_stability_params(0.05, 75.0, 0.75);
        sim.pc.p_entry = 0.0;
        sim.pvt.mu_w = 0.5;
        sim.pvt.mu_o = 1.0;
        sim.add_well(0, 0, 0, 500.0, 0.1, 0.0, true).unwrap();
        sim.add_well(23, 0, 0, 100.0, 0.1, 0.0, false).unwrap();

        sim.step(0.5);

        assert!(
            (sim.time_days - 0.5).abs() < 1e-9,
            "Expected the simulator to complete the requested 0.5 day step, advanced {} days",
            sim.time_days
        );
        assert!(
            !sim.rate_history.is_empty()
                && (sim.rate_history.last().unwrap().time - 0.5).abs() < 1e-9,
            "Expected the last recorded rate-history time to match the completed step"
        );
    }

    #[test]
    fn benchmark_buckley_leverett_case_a_favorable_mobility() {
        let case = buckley_case_a("BL-Case-A", 24, 0.5, 4000);

        let metrics = run_buckley_case(&case);
        let rel_err = ((metrics.breakthrough_pv - metrics.reference_breakthrough_pv)
            / metrics.reference_breakthrough_pv)
            .abs();

        println!(
            "{}: breakthrough_pv_sim={:.4}, breakthrough_pv_ref={:.4}, rel_err={:.3}",
            case.name, metrics.breakthrough_pv, metrics.reference_breakthrough_pv, rel_err
        );

        assert!(
            rel_err <= case.rel_tol_breakthrough_pv,
            "{} breakthrough PV mismatch too high: sim={:.4}, ref={:.4}, rel_err={:.3}, tol={:.3}",
            case.name,
            metrics.breakthrough_pv,
            metrics.reference_breakthrough_pv,
            rel_err,
            case.rel_tol_breakthrough_pv,
        );
    }

    #[test]
    fn benchmark_buckley_leverett_case_b_more_adverse_mobility() {
        let case = buckley_case_b("BL-Case-B", 24, 0.25, 4000);

        let metrics = run_buckley_case(&case);
        let rel_err = ((metrics.breakthrough_pv - metrics.reference_breakthrough_pv)
            / metrics.reference_breakthrough_pv)
            .abs();

        println!(
            "{}: breakthrough_pv_sim={:.4}, breakthrough_pv_ref={:.4}, rel_err={:.3}",
            case.name, metrics.breakthrough_pv, metrics.reference_breakthrough_pv, rel_err
        );

        assert!(
            rel_err <= case.rel_tol_breakthrough_pv,
            "{} breakthrough PV mismatch too high: sim={:.4}, ref={:.4}, rel_err={:.3}, tol={:.3}",
            case.name,
            metrics.breakthrough_pv,
            metrics.reference_breakthrough_pv,
            rel_err,
            case.rel_tol_breakthrough_pv,
        );
    }

    #[test]
    #[ignore = "slow discretization-sensitivity regression; run explicitly when tuning Buckley-Leverett numerics"]
    fn benchmark_buckley_leverett_refined_discretization_improves_alignment() {
        let coarse_a = buckley_case_a("BL-Case-A-Coarse", 24, 0.5, 4000);
        let refined_a = buckley_case_a("BL-Case-A-Refined", 96, 0.125, 20000);

        let coarse_b = buckley_case_b("BL-Case-B-Coarse", 24, 0.25, 4000);
        let refined_b = buckley_case_b("BL-Case-B-Refined", 96, 0.125, 20000);

        let metrics_coarse_a = run_buckley_case(&coarse_a);
        let metrics_refined_a = run_buckley_case(&refined_a);
        let rel_err_coarse_a = ((metrics_coarse_a.breakthrough_pv
            - metrics_coarse_a.reference_breakthrough_pv)
            / metrics_coarse_a.reference_breakthrough_pv)
            .abs();
        let rel_err_refined_a = ((metrics_refined_a.breakthrough_pv
            - metrics_refined_a.reference_breakthrough_pv)
            / metrics_refined_a.reference_breakthrough_pv)
            .abs();

        let metrics_coarse_b = run_buckley_case(&coarse_b);
        let metrics_refined_b = run_buckley_case(&refined_b);
        let rel_err_coarse_b = ((metrics_coarse_b.breakthrough_pv
            - metrics_coarse_b.reference_breakthrough_pv)
            / metrics_coarse_b.reference_breakthrough_pv)
            .abs();
        let rel_err_refined_b = ((metrics_refined_b.breakthrough_pv
            - metrics_refined_b.reference_breakthrough_pv)
            / metrics_refined_b.reference_breakthrough_pv)
            .abs();

        println!(
            "Case-A coarse/refined rel_err: {:.3} -> {:.3}",
            rel_err_coarse_a, rel_err_refined_a
        );
        println!(
            "BL-Case-A-Refined: breakthrough_pv_sim={:.4}, breakthrough_pv_ref={:.4}, rel_err={:.3}",
            metrics_refined_a.breakthrough_pv,
            metrics_refined_a.reference_breakthrough_pv,
            rel_err_refined_a
        );
        println!(
            "Case-B coarse/refined rel_err: {:.3} -> {:.3}",
            rel_err_coarse_b, rel_err_refined_b
        );
        println!(
            "BL-Case-B-Refined: breakthrough_pv_sim={:.4}, breakthrough_pv_ref={:.4}, rel_err={:.3}",
            metrics_refined_b.breakthrough_pv,
            metrics_refined_b.reference_breakthrough_pv,
            rel_err_refined_b
        );

        assert!(
            rel_err_refined_a <= rel_err_coarse_a,
            "Refined discretization should not worsen Case-A alignment: coarse={:.3}, refined={:.3}",
            rel_err_coarse_a,
            rel_err_refined_a
        );
        assert!(
            rel_err_refined_b <= rel_err_coarse_b,
            "Refined discretization should not worsen Case-B alignment: coarse={:.3}, refined={:.3}",
            rel_err_coarse_b,
            rel_err_refined_b
        );
    }

    #[test]
    fn benchmark_buckley_leverett_smaller_dt_improves_coarse_alignment() {
        let case_a_dt_050 = buckley_case_a("BL-Case-A-Coarse-dt0.50", 24, 0.5, 4000);
        let case_a_dt_025 = buckley_case_a("BL-Case-A-Coarse-dt0.25", 24, 0.25, 8000);
        let metrics_a_dt_050 = run_buckley_case(&case_a_dt_050);
        let metrics_a_dt_025 = run_buckley_case(&case_a_dt_025);
        let rel_err_a_dt_050 = ((metrics_a_dt_050.breakthrough_pv
            - metrics_a_dt_050.reference_breakthrough_pv)
            / metrics_a_dt_050.reference_breakthrough_pv)
            .abs();
        let rel_err_a_dt_025 = ((metrics_a_dt_025.breakthrough_pv
            - metrics_a_dt_025.reference_breakthrough_pv)
            / metrics_a_dt_025.reference_breakthrough_pv)
            .abs();

        let case_b_dt_050 = buckley_case_b("BL-Case-B-Coarse-dt0.50", 24, 0.5, 4000);
        let case_b_dt_025 = buckley_case_b("BL-Case-B-Coarse-dt0.25", 24, 0.25, 4000);
        let metrics_b_dt_050 = run_buckley_case(&case_b_dt_050);
        let metrics_b_dt_025 = run_buckley_case(&case_b_dt_025);
        let rel_err_b_dt_050 = ((metrics_b_dt_050.breakthrough_pv
            - metrics_b_dt_050.reference_breakthrough_pv)
            / metrics_b_dt_050.reference_breakthrough_pv)
            .abs();
        let rel_err_b_dt_025 = ((metrics_b_dt_025.breakthrough_pv
            - metrics_b_dt_025.reference_breakthrough_pv)
            / metrics_b_dt_025.reference_breakthrough_pv)
            .abs();

        println!(
            "Case-A coarse dt sweep rel_err: dt=0.50 -> {:.3}, dt=0.25 -> {:.3}",
            rel_err_a_dt_050, rel_err_a_dt_025
        );
        println!(
            "Case-B coarse dt sweep rel_err: dt=0.50 -> {:.3}, dt=0.25 -> {:.3}",
            rel_err_b_dt_050, rel_err_b_dt_025
        );

        assert!(
            rel_err_a_dt_025 + 1e-9 < rel_err_a_dt_050,
            "Smaller dt should improve Case-A coarse alignment: dt=0.50 -> {:.3}, dt=0.25 -> {:.3}",
            rel_err_a_dt_050,
            rel_err_a_dt_025
        );
        assert!(
            rel_err_b_dt_025 + 1e-9 < rel_err_b_dt_050,
            "Smaller dt should improve Case-B coarse alignment: dt=0.50 -> {:.3}, dt=0.25 -> {:.3}",
            rel_err_b_dt_050,
            rel_err_b_dt_025
        );
    }

    #[test]
    fn set_initial_saturation_per_layer_applies_uniformly_by_k() {
        let mut sim = ReservoirSimulator::new(2, 2, 3, 0.2);
        sim.set_initial_saturation_per_layer(vec![0.1, 0.4, 0.8])
            .unwrap();

        for k in 0..sim.nz {
            for j in 0..sim.ny {
                for i in 0..sim.nx {
                    let id = sim.idx(i, j, k);
                    let sw = sim.sat_water[id];
                    assert!((sw - [0.1, 0.4, 0.8][k]).abs() < 1e-12);
                    assert!((sim.sat_oil[id] - (1.0 - sw)).abs() < 1e-12);
                }
            }
        }
    }

    #[test]
    fn dynamic_pi_increases_with_higher_water_saturation() {
        let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        sim.set_rel_perm_props(0.1, 0.1, 2.0, 2.0, 1.0, 1.0)
            .unwrap();
        sim.set_fluid_properties(3.0, 0.5).unwrap();
        sim.add_well(0, 0, 0, 100.0, 0.1, 0.0, false).unwrap();

        let id = sim.idx(0, 0, 0);
        sim.sat_water[id] = sim.scal.s_wc;
        sim.sat_oil[id] = 1.0 - sim.scal.s_wc;
        sim.update_dynamic_well_productivity_indices();
        let pi_low_sw = sim.wells[0].productivity_index;

        let sw_high = 0.95 - sim.scal.s_or;
        sim.sat_water[id] = sw_high;
        sim.sat_oil[id] = 1.0 - sw_high;
        sim.update_dynamic_well_productivity_indices();
        let pi_high_sw = sim.wells[0].productivity_index;

        assert!(pi_high_sw > pi_low_sw);
    }

    #[test]
    fn well_productivity_index_matches_metric_unit_conversion() {
        let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        sim.set_rel_perm_props(0.1, 0.1, 2.0, 2.0, 1.0, 1.0)
            .unwrap();
        sim.set_fluid_properties(2.0, 0.5).unwrap();
        sim.set_initial_saturation(0.1);

        let id = sim.idx(0, 0, 0);
        let well_radius = 0.1;
        let skin = 0.0;

        let pi = sim
            .calculate_well_productivity_index(id, well_radius, skin)
            .expect("PI should calculate for a valid isotropic cell");

        let kx = sim.perm_x[id];
        let ky = sim.perm_y[id];
        let k_avg = (kx * ky).sqrt();
        let ratio = kx / ky;
        let r_eq = 0.28
            * ((ratio.sqrt() * sim.dx.powi(2) + (1.0 / ratio).sqrt() * sim.dy.powi(2)).sqrt())
            / (ratio.powf(0.25) + (1.0 / ratio).powf(0.25));
        let denom = (r_eq / well_radius).ln() + skin;
        let total_mobility = 1.0 / sim.pvt.mu_o;

        // 1 mD = 9.8692e-16 m², 1/cP = 1000/(Pa·s), 1 bar = 1e5 Pa, 1 day = 86400 s
        let expected_darcy_metric_factor = 9.8692e-16 * 1e3 * 1e5 * 86400.0;
        let expected_pi = expected_darcy_metric_factor
            * 2.0
            * std::f64::consts::PI
            * k_avg
            * sim.dz[0]
            * total_mobility
            / denom;

        assert!(
            (pi - expected_pi).abs() / expected_pi < 1e-9,
            "PI mismatch: got {}, expected {}",
            pi,
            expected_pi
        );
    }

    #[test]
    fn rate_control_switches_to_bhp_when_limits_are_hit() {
        let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        sim.set_well_control_modes("pressure".to_string(), "rate".to_string());
        sim.set_target_well_rates(0.0, 500.0).unwrap();
        sim.set_well_bhp_limits(80.0, 120.0).unwrap();
        sim.add_well(0, 0, 0, 100.0, 0.1, 0.0, false).unwrap();

        let well = &sim.wells[0];
        let pressure = 100.0;

        let control = sim
            .resolve_well_control(well, pressure)
            .expect("control decision should be available");

        assert!(control.bhp_limited);

        match control.decision {
            WellControlDecision::Bhp { bhp_bar } => {
                assert!((bhp_bar - 80.0).abs() < 1e-9);
            }
            _ => panic!("Expected BHP fallback control when target rate violates BHP limits"),
        }

        let q = sim.well_rate_m3_day(well, pressure).unwrap();
        let expected_q = well.productivity_index * (pressure - 80.0);
        assert!((q - expected_q).abs() < 1e-9);
        assert!(q < 500.0);
    }

    #[test]
    fn multi_completion_producer_rate_control_uses_shared_bhp() {
        let mut sim = ReservoirSimulator::new(1, 1, 2, 0.2);
        sim.set_well_control_modes("pressure".to_string(), "rate".to_string());
        sim.set_target_well_rates(0.0, 100.0).unwrap();
        sim.set_well_bhp_limits(0.0, 300.0).unwrap();
        sim.add_well(0, 0, 0, 100.0, 0.1, 0.0, false).unwrap();
        sim.add_well(0, 0, 1, 100.0, 0.1, 0.0, false).unwrap();

        let id0 = sim.idx(0, 0, 0);
        let id1 = sim.idx(0, 0, 1);
        sim.pressure[id0] = 200.0;
        sim.pressure[id1] = 180.0;

        let well0 = &sim.wells[0];
        let well1 = &sim.wells[1];
        let q0 = sim.well_rate_m3_day(well0, sim.pressure[id0]).unwrap();
        let q1 = sim.well_rate_m3_day(well1, sim.pressure[id1]).unwrap();

        let bhp0 = sim.pressure[id0] - q0 / well0.productivity_index;
        let bhp1 = sim.pressure[id1] - q1 / well1.productivity_index;

        assert!(
            ((q0 + q1) - 100.0).abs() < 1e-6,
            "Expected producer completions to satisfy the group target, got q0={}, q1={}",
            q0,
            q1
        );
        assert!(
            (bhp0 - bhp1).abs() < 1e-9,
            "Expected all producer completions to share one flowing BHP, got {} and {}",
            bhp0,
            bhp1
        );
    }

    // ── Three-phase tests ────────────────────────────────────────────────────

    #[test]
    fn three_phase_relperm_k_ro_stone2_endpoints() {
        use crate::relperm::RockFluidPropsThreePhase;

        let rock = RockFluidPropsThreePhase {
            s_wc: 0.10,
            s_or: 0.10,
            n_w: 2.0,
            n_o: 2.0,
            k_rw_max: 0.8,
            k_ro_max: 0.9,
            s_gc: 0.05,
            s_gr: 0.05,
            s_org: 0.10,
            n_g: 1.5,
            k_rg_max: 0.7,
            tables: None,
        };

        // At connate water with no free gas → k_ro should equal k_ro_max
        let kro_at_swc = rock.k_ro_stone2(rock.s_wc, 0.0);
        assert!(
            (kro_at_swc - rock.k_ro_max).abs() < 1e-10,
            "k_ro_stone2(Swc, 0) should equal k_ro_max ({}) but got {}",
            rock.k_ro_max,
            kro_at_swc
        );

        // When gas saturation reaches 1 − Swc − Sorg, oil is at residual → k_ro = 0
        let sg_at_sorg = 1.0 - rock.s_wc - rock.s_org;
        let kro_max_gas = rock.k_ro_stone2(rock.s_wc, sg_at_sorg);
        assert!(
            kro_max_gas < 1e-9,
            "k_ro_stone2(Swc, sg_at_sorg) should be ~0 but got {}",
            kro_max_gas
        );

        // At fully flooded water (1-Sor), no gas → k_ro = 0
        let kro_at_max_water = rock.k_ro_stone2(1.0 - rock.s_or, 0.0);
        assert!(
            kro_at_max_water < 1e-9,
            "k_ro_stone2(1-Sor, 0) should be ~0 but got {}",
            kro_at_max_water
        );

        // k_ro must stay in [0, k_ro_max] across the entire saturation triangle
        for i in 0..=20 {
            let sw = rock.s_wc + i as f64 * (1.0 - rock.s_wc - rock.s_or) / 20.0;
            for j in 0..=20 {
                let sg = j as f64 * (1.0 - rock.s_wc - rock.s_gr) / 20.0;
                if sw + sg <= 1.0 {
                    let kro = rock.k_ro_stone2(sw, sg);
                    assert!(
                        kro >= -1e-10,
                        "k_ro_stone2 negative at sw={:.3}, sg={:.3}: {}",
                        sw,
                        sg,
                        kro
                    );
                    assert!(
                        kro <= rock.k_ro_max + 1e-10,
                        "k_ro_stone2 exceeds k_ro_max at sw={:.3}, sg={:.3}: {}",
                        sw,
                        sg,
                        kro
                    );
                }
            }
        }
    }

    #[test]
    fn three_phase_relperm_k_rg_endpoints_and_monotonicity() {
        use crate::relperm::RockFluidPropsThreePhase;

        let rock = RockFluidPropsThreePhase {
            s_wc: 0.10,
            s_or: 0.10,
            n_w: 2.0,
            n_o: 2.0,
            k_rw_max: 0.8,
            k_ro_max: 0.9,
            s_gc: 0.05,
            s_gr: 0.05,
            s_org: 0.10,
            n_g: 2.0,
            k_rg_max: 0.7,
            tables: None,
        };

        // Below and at critical gas saturation → k_rg = 0
        assert_eq!(rock.k_rg(0.0), 0.0);
        assert_eq!(rock.k_rg(rock.s_gc * 0.5), 0.0);
        assert!(
            rock.k_rg(rock.s_gc) < 1e-10,
            "k_rg(Sgc) = {}",
            rock.k_rg(rock.s_gc)
        );

        // At maximum mobile gas saturation (Sg = 1 - Swc - Sgr) → k_rg = k_rg_max
        let sg_at_kmax = 1.0 - rock.s_wc - rock.s_gr;
        let krg_at_max = rock.k_rg(sg_at_kmax);
        assert!(
            (krg_at_max - rock.k_rg_max).abs() < 1e-10,
            "k_rg at max gas sat should be k_rg_max ({}) but got {}",
            rock.k_rg_max,
            krg_at_max
        );

        // k_rg is monotonically non-decreasing from Sgc to sg_at_kmax
        let mut prev_krg = 0.0;
        let n = 50;
        for i in 0..=n {
            let sg = rock.s_gc + i as f64 * (sg_at_kmax - rock.s_gc) / n as f64;
            let krg = rock.k_rg(sg);
            assert!(
                krg >= prev_krg - 1e-12,
                "k_rg not monotone at sg={:.4}: {} < prev {}",
                sg,
                krg,
                prev_krg
            );
            prev_krg = krg;
        }
    }

    #[test]
    fn three_phase_relperm_stone2_reduces_to_two_phase_at_zero_gas() {
        use crate::relperm::RockFluidPropsThreePhase;

        let rock = RockFluidPropsThreePhase {
            s_wc: 0.10,
            s_or: 0.10,
            n_w: 2.0,
            n_o: 2.0,
            k_rw_max: 0.8,
            k_ro_max: 0.9,
            s_gc: 0.05,
            s_gr: 0.05,
            s_org: 0.10,
            n_g: 1.5,
            k_rg_max: 0.7,
            tables: None,
        };

        // When Sg = 0, Stone II must collapse exactly to the oil-water k_ro curve:
        //   Stone II at Sg=0: kro_g→k_ro_max, krg→0
        //   => k_ro = k_ro_max * [(kro_w/k_ro_max + krw)(1 + 0) − krw] = kro_w
        let sw_vals = [0.10, 0.20, 0.30, 0.50, 0.70, 0.85, 0.90];
        for &sw in &sw_vals {
            let kro_stone2 = rock.k_ro_stone2(sw, 0.0);
            let kro_ow = rock.k_ro_water(sw);
            assert!(
                (kro_stone2 - kro_ow).abs() < 1e-10,
                "Stone II at Sg=0 does not match k_ro_water at sw={}: stone2={}, k_ro_w={}",
                sw,
                kro_stone2,
                kro_ow
            );
        }
    }

    #[test]
    fn three_phase_relperm_tables_interpolate_exact_spe1_points() {
        use crate::relperm::{RockFluidPropsThreePhase, SgofRow, SwofRow, ThreePhaseScalTables};

        let rock = RockFluidPropsThreePhase {
            s_wc: 0.12,
            s_or: 0.12,
            n_w: 2.0,
            n_o: 2.5,
            k_rw_max: 1e-5,
            k_ro_max: 1.0,
            s_gc: 0.04,
            s_gr: 0.04,
            s_org: 0.18,
            n_g: 1.5,
            k_rg_max: 0.984,
            tables: Some(ThreePhaseScalTables {
                swof: vec![
                    SwofRow {
                        sw: 0.12,
                        krw: 0.0,
                        krow: 1.0,
                        pcow: Some(0.0),
                    },
                    SwofRow {
                        sw: 0.24,
                        krw: 1.86e-7,
                        krow: 0.997,
                        pcow: Some(0.0),
                    },
                    SwofRow {
                        sw: 1.0,
                        krw: 1e-5,
                        krow: 0.0,
                        pcow: Some(0.0),
                    },
                ],
                sgof: vec![
                    SgofRow {
                        sg: 0.0,
                        krg: 0.0,
                        krog: 1.0,
                        pcog: Some(0.0),
                    },
                    SgofRow {
                        sg: 0.5,
                        krg: 0.72,
                        krog: 0.001,
                        pcog: Some(0.0),
                    },
                    SgofRow {
                        sg: 0.88,
                        krg: 0.984,
                        krog: 0.0,
                        pcog: Some(0.0),
                    },
                ],
            }),
        };

        assert!((rock.k_rw(0.12) - 0.0).abs() < 1e-12);
        assert!((rock.k_ro_water(0.24) - 0.997).abs() < 1e-12);
        assert!((rock.k_rg(0.5) - 0.72).abs() < 1e-12);
        assert!((rock.k_ro_gas(0.5) - 0.001).abs() < 1e-12);
    }

    #[test]
    fn three_phase_scal_tables_validate_valid_spe1_fragment() {
        use crate::relperm::{SgofRow, SwofRow, ThreePhaseScalTables};

        let tables = ThreePhaseScalTables {
            swof: vec![
                SwofRow {
                    sw: 0.12,
                    krw: 0.0,
                    krow: 1.0,
                    pcow: Some(0.0),
                },
                SwofRow {
                    sw: 1.0,
                    krw: 1e-5,
                    krow: 0.0,
                    pcow: Some(0.0),
                },
            ],
            sgof: vec![
                SgofRow {
                    sg: 0.0,
                    krg: 0.0,
                    krog: 1.0,
                    pcog: Some(0.0),
                },
                SgofRow {
                    sg: 0.88,
                    krg: 0.984,
                    krog: 0.0,
                    pcog: Some(0.0),
                },
            ],
        };

        assert!(tables.validate().is_ok());
    }

    /// Build a minimal 3-phase simulator with gas injection for physics tests.
    fn make_3phase_gas_injection_sim(nx: usize) -> ReservoirSimulator {
        let mut sim = ReservoirSimulator::new(nx, 1, 1, 0.2);
        sim.set_three_phase_rel_perm_props(
            0.10, 0.10, 0.05, 0.05, 0.10, 2.0, 2.0, 1.5, 0.8, 0.9, 0.7,
        )
        .unwrap();
        sim.set_gas_fluid_properties(0.02, 1e-4, 10.0).unwrap();
        sim.set_three_phase_mode_enabled(true);
        sim.set_injected_fluid("gas").unwrap();
        sim.set_initial_saturation(0.10);
        sim.pc.p_entry = 0.0;
        sim.add_well(0, 0, 0, 400.0, 0.1, 0.0, true).unwrap();
        sim.add_well(nx - 1, 0, 0, 100.0, 0.1, 0.0, false).unwrap();
        sim
    }

    #[test]
    fn three_phase_saturation_sum_equals_one() {
        let mut sim = make_3phase_gas_injection_sim(8);

        for _ in 0..30 {
            sim.step(1.0);
        }

        let n = sim.nx * sim.ny * sim.nz;
        for i in 0..n {
            let sw = sim.sat_water[i];
            let so = sim.sat_oil[i];
            let sg = sim.sat_gas[i];
            let sum = sw + so + sg;
            assert!(
                (sum - 1.0).abs() < 1e-8,
                "sw+so+sg != 1 at cell {}: sw={:.6}, so={:.6}, sg={:.6}, sum={:.9}",
                i,
                sw,
                so,
                sg,
                sum
            );
            assert!(sw >= -1e-9, "Sw negative at cell {}: {}", i, sw);
            assert!(so >= -1e-9, "So negative at cell {}: {}", i, so);
            assert!(sg >= -1e-9, "Sg negative at cell {}: {}", i, sg);
        }
    }

    #[test]
    fn three_phase_gas_injection_increases_avg_gas_saturation() {
        let mut sim = make_3phase_gas_injection_sim(5);

        let n = sim.nx * sim.ny * sim.nz;
        let avg_sg_initial: f64 = sim.sat_gas.iter().sum::<f64>() / n as f64;
        assert_eq!(avg_sg_initial, 0.0, "Initial gas saturation should be zero");

        for _ in 0..50 {
            sim.step(2.0);
        }

        let avg_sg_final: f64 = sim.sat_gas.iter().sum::<f64>() / n as f64;
        assert!(
            avg_sg_final > 0.01,
            "Gas saturation should increase during gas injection, avg_sg={:.6}",
            avg_sg_final
        );
    }

    #[test]
    fn three_phase_rate_history_records_gas_production() {
        let mut sim = make_3phase_gas_injection_sim(5);

        for _ in 0..20 {
            sim.step(2.0);
        }

        let last = sim
            .rate_history
            .last()
            .expect("rate history should have entries");
        assert!(
            last.total_production_gas.is_finite(),
            "total_production_gas should be finite, got {}",
            last.total_production_gas
        );

        let total_gas_produced: f64 = sim
            .rate_history
            .iter()
            .map(|r| r.total_production_gas.max(0.0))
            .sum();
        assert!(
            total_gas_produced > 0.0,
            "Expected positive cumulative gas production after gas injection"
        );
    }

    #[test]
    fn three_phase_gas_injection_keeps_gas_balance_bounded() {
        let mut sim = make_3phase_gas_injection_sim(8);

        for _ in 0..40 {
            sim.step(2.0);
        }

        let latest = sim
            .rate_history
            .last()
            .expect("rate history should have entries");
        assert!(latest.material_balance_error_gas_m3.is_finite());
        assert!(
            latest.material_balance_error_gas_m3 < 5.0e3,
            "gas material balance drift too large: {} Sm3",
            latest.material_balance_error_gas_m3
        );
    }

    #[test]
    fn three_phase_gas_injection_keeps_pressures_bounded_under_large_steps() {
        let mut sim = ReservoirSimulator::new(6, 1, 3, 0.2);
        sim.set_three_phase_rel_perm_props(
            0.10, 0.10, 0.05, 0.05, 0.10, 2.0, 2.0, 1.5, 0.8, 0.9, 0.7,
        )
        .unwrap();
        sim.set_gas_fluid_properties(0.02, 1e-4, 10.0).unwrap();
        sim.set_three_phase_mode_enabled(true);
        sim.set_injected_fluid("gas").unwrap();
        sim.set_initial_pressure(330.0);
        sim.set_initial_saturation(0.12);
        sim.set_stability_params(0.05, 75.0, 0.75);
        sim.pc.p_entry = 0.0;
        sim.add_well(0, 0, 0, 450.0, 0.1, 0.0, true).unwrap();
        sim.add_well(5, 0, 2, 150.0, 0.1, 0.0, false).unwrap();

        for _ in 0..12 {
            sim.step(5.0);
        }

        for (idx, pressure) in sim.pressure.iter().enumerate() {
            assert!(
                pressure.is_finite(),
                "pressure must remain finite at cell {}",
                idx
            );
            assert!(
                *pressure > 1.0 && *pressure < 5_000.0,
                "pressure {} at cell {} escaped the physical envelope",
                pressure,
                idx
            );
        }

        for (idx, sg) in sim.sat_gas.iter().enumerate() {
            assert!(
                sg.is_finite(),
                "gas saturation must remain finite at cell {}",
                idx
            );
            assert!(
                *sg >= -1e-9 && *sg <= 1.0 + 1e-9,
                "gas saturation {} at cell {} escaped bounds",
                sg,
                idx
            );
        }

        for point in &sim.rate_history {
            assert!(point.avg_reservoir_pressure.is_finite());
            assert!(point.avg_reservoir_pressure > 1.0);
            assert!(point.avg_reservoir_pressure < 5_000.0);
        }
    }

    #[test]
    fn gas_injection_surface_totals_use_bg_conversion() {
        use crate::pvt::{PvtRow, PvtTable};

        let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        sim.set_three_phase_rel_perm_props(
            0.10, 0.10, 0.05, 0.05, 0.10, 2.0, 2.0, 1.5, 0.8, 0.9, 0.7,
        )
        .unwrap();
        sim.set_three_phase_mode_enabled(true);
        sim.set_injected_fluid("gas").unwrap();
        sim.set_gas_fluid_properties(0.02, 1e-4, 10.0).unwrap();
        sim.set_initial_pressure(100.0);
        sim.set_initial_saturation(0.10);
        sim.pvt_table = Some(PvtTable::new(
            vec![PvtRow {
                p_bar: 100.0,
                rs_m3m3: 0.0,
                bo_m3m3: 1.2,
                mu_o_cp: 1.0,
                bg_m3m3: 0.25,
                mu_g_cp: 0.02,
            }],
            sim.pvt.c_o,
        ));
        sim.set_well_control_modes("rate".to_string(), "bhp".to_string());
        sim.set_target_well_surface_rates(120.0, 0.0).unwrap();
        sim.set_well_bhp_limits(0.0, 1.0e9).unwrap();
        sim.add_well(0, 0, 0, 100.0, 0.1, 0.0, true).unwrap();

        sim.step(1.0);

        let latest = sim
            .rate_history
            .last()
            .expect("rate history should have an entry");
        // After the fix, the reported SC injection is computed from the actual
        // reservoir rate and Bg at the new pressure — not the target surface rate.
        // With injection, pressure rises above 100 bar, so Bg(p_new) < Bg(100) = 0.25,
        // and the reported SC rate slightly exceeds the 120 Sm³/d target.
        assert!(
            latest.total_injection > 100.0 && latest.total_injection < 200.0,
            "Expected gas injector surface total near target surface rate, got {}",
            latest.total_injection
        );
    }

    fn make_spe1_like_sim(
        layer_perms_z: Vec<f64>,
        max_sat_change_per_step: f64,
        max_pressure_change_per_step: f64,
        max_well_rate_change_fraction: f64,
    ) -> ReservoirSimulator {
        use crate::pvt::{PvtRow, PvtTable};
        use crate::relperm::{SgofRow, SwofRow, ThreePhaseScalTables};

        let mut sim = ReservoirSimulator::new(10, 10, 3, 0.3);
        sim.set_cell_dimensions_per_layer(304.8, 304.8, vec![6.096, 9.144, 15.24])
            .unwrap();
        sim.set_fluid_properties(0.51, 0.318).unwrap();
        sim.set_fluid_compressibilities(2.06e-4, 4.67e-5).unwrap();
        sim.pvt_table = Some(PvtTable::new(
            vec![
                PvtRow {
                    p_bar: 1.01,
                    rs_m3m3: 0.18,
                    bo_m3m3: 1.062,
                    mu_o_cp: 1.040,
                    bg_m3m3: 0.9361,
                    mu_g_cp: 0.0080,
                },
                PvtRow {
                    p_bar: 18.25,
                    rs_m3m3: 16.12,
                    bo_m3m3: 1.150,
                    mu_o_cp: 0.975,
                    bg_m3m3: 0.0679,
                    mu_g_cp: 0.0096,
                },
                PvtRow {
                    p_bar: 35.49,
                    rs_m3m3: 32.06,
                    bo_m3m3: 1.207,
                    mu_o_cp: 0.910,
                    bg_m3m3: 0.0352,
                    mu_g_cp: 0.0112,
                },
                PvtRow {
                    p_bar: 69.96,
                    rs_m3m3: 66.08,
                    bo_m3m3: 1.295,
                    mu_o_cp: 0.830,
                    bg_m3m3: 0.0179,
                    mu_g_cp: 0.0140,
                },
                PvtRow {
                    p_bar: 138.91,
                    rs_m3m3: 113.29,
                    bo_m3m3: 1.435,
                    mu_o_cp: 0.695,
                    bg_m3m3: 0.00906,
                    mu_g_cp: 0.0189,
                },
                PvtRow {
                    p_bar: 173.38,
                    rs_m3m3: 138.03,
                    bo_m3m3: 1.500,
                    mu_o_cp: 0.641,
                    bg_m3m3: 0.00727,
                    mu_g_cp: 0.0208,
                },
                PvtRow {
                    p_bar: 207.85,
                    rs_m3m3: 165.64,
                    bo_m3m3: 1.565,
                    mu_o_cp: 0.594,
                    bg_m3m3: 0.00607,
                    mu_g_cp: 0.0228,
                },
                PvtRow {
                    p_bar: 276.79,
                    rs_m3m3: 226.20,
                    bo_m3m3: 1.695,
                    mu_o_cp: 0.510,
                    bg_m3m3: 0.00455,
                    mu_g_cp: 0.0268,
                },
                PvtRow {
                    p_bar: 345.73,
                    rs_m3m3: 288.17,
                    bo_m3m3: 1.827,
                    mu_o_cp: 0.449,
                    bg_m3m3: 0.00364,
                    mu_g_cp: 0.0309,
                },
            ],
            sim.pvt.c_o,
        ));
        sim.set_initial_rs(226.197);
        sim.set_rock_properties(4.35e-5, 2560.0, 1.695, 1.038)
            .unwrap();
        sim.set_fluid_densities(860.0, 1033.0).unwrap();
        sim.set_initial_pressure(331.0);
        sim.set_initial_saturation(0.12);
        sim.set_capillary_params(0.0, 2.0).unwrap();
        sim.set_gravity_enabled(true);
        sim.set_three_phase_rel_perm_props(
            0.12, 0.12, 0.04, 0.04, 0.18, 2.0, 2.5, 1.5, 1e-5, 1.0, 0.984,
        )
        .unwrap();
        sim.scal_3p.as_mut().unwrap().tables = Some(ThreePhaseScalTables {
            swof: vec![
                SwofRow {
                    sw: 0.12,
                    krw: 0.0,
                    krow: 1.0,
                    pcow: Some(0.0),
                },
                SwofRow {
                    sw: 0.18,
                    krw: 4.64876033057851e-8,
                    krow: 1.0,
                    pcow: Some(0.0),
                },
                SwofRow {
                    sw: 0.24,
                    krw: 1.86e-7,
                    krow: 0.997,
                    pcow: Some(0.0),
                },
                SwofRow {
                    sw: 0.3,
                    krw: 4.18388429752066e-7,
                    krow: 0.98,
                    pcow: Some(0.0),
                },
                SwofRow {
                    sw: 0.36,
                    krw: 7.43801652892562e-7,
                    krow: 0.7,
                    pcow: Some(0.0),
                },
                SwofRow {
                    sw: 0.42,
                    krw: 1.16219008264463e-6,
                    krow: 0.35,
                    pcow: Some(0.0),
                },
                SwofRow {
                    sw: 0.48,
                    krw: 1.67355371900826e-6,
                    krow: 0.2,
                    pcow: Some(0.0),
                },
                SwofRow {
                    sw: 0.54,
                    krw: 2.27789256198347e-6,
                    krow: 0.09,
                    pcow: Some(0.0),
                },
                SwofRow {
                    sw: 0.6,
                    krw: 2.97520661157025e-6,
                    krow: 0.021,
                    pcow: Some(0.0),
                },
                SwofRow {
                    sw: 0.66,
                    krw: 3.7654958677686e-6,
                    krow: 0.01,
                    pcow: Some(0.0),
                },
                SwofRow {
                    sw: 0.72,
                    krw: 4.64876033057851e-6,
                    krow: 0.001,
                    pcow: Some(0.0),
                },
                SwofRow {
                    sw: 0.78,
                    krw: 5.625e-6,
                    krow: 0.0001,
                    pcow: Some(0.0),
                },
                SwofRow {
                    sw: 0.84,
                    krw: 6.69421487603306e-6,
                    krow: 0.0,
                    pcow: Some(0.0),
                },
                SwofRow {
                    sw: 0.91,
                    krw: 8.05914256198347e-6,
                    krow: 0.0,
                    pcow: Some(0.0),
                },
                SwofRow {
                    sw: 1.0,
                    krw: 1e-5,
                    krow: 0.0,
                    pcow: Some(0.0),
                },
            ],
            sgof: vec![
                SgofRow {
                    sg: 0.0,
                    krg: 0.0,
                    krog: 1.0,
                    pcog: Some(0.0),
                },
                SgofRow {
                    sg: 0.001,
                    krg: 0.0,
                    krog: 1.0,
                    pcog: Some(0.0),
                },
                SgofRow {
                    sg: 0.02,
                    krg: 0.0,
                    krog: 0.997,
                    pcog: Some(0.0),
                },
                SgofRow {
                    sg: 0.05,
                    krg: 0.005,
                    krog: 0.98,
                    pcog: Some(0.0),
                },
                SgofRow {
                    sg: 0.12,
                    krg: 0.025,
                    krog: 0.7,
                    pcog: Some(0.0),
                },
                SgofRow {
                    sg: 0.2,
                    krg: 0.075,
                    krog: 0.35,
                    pcog: Some(0.0),
                },
                SgofRow {
                    sg: 0.25,
                    krg: 0.125,
                    krog: 0.2,
                    pcog: Some(0.0),
                },
                SgofRow {
                    sg: 0.3,
                    krg: 0.19,
                    krog: 0.09,
                    pcog: Some(0.0),
                },
                SgofRow {
                    sg: 0.4,
                    krg: 0.41,
                    krog: 0.021,
                    pcog: Some(0.0),
                },
                SgofRow {
                    sg: 0.45,
                    krg: 0.6,
                    krog: 0.01,
                    pcog: Some(0.0),
                },
                SgofRow {
                    sg: 0.5,
                    krg: 0.72,
                    krog: 0.001,
                    pcog: Some(0.0),
                },
                SgofRow {
                    sg: 0.6,
                    krg: 0.87,
                    krog: 0.0001,
                    pcog: Some(0.0),
                },
                SgofRow {
                    sg: 0.7,
                    krg: 0.94,
                    krog: 0.0,
                    pcog: Some(0.0),
                },
                SgofRow {
                    sg: 0.85,
                    krg: 0.98,
                    krog: 0.0,
                    pcog: Some(0.0),
                },
                SgofRow {
                    sg: 0.88,
                    krg: 0.984,
                    krog: 0.0,
                    pcog: Some(0.0),
                },
            ],
        });
        sim.set_gas_fluid_properties(0.027, 1e-4, 0.854).unwrap();
        sim.set_three_phase_mode_enabled(true);
        sim.set_injected_fluid("gas").unwrap();
        sim.set_gas_redissolution_enabled(false);
        sim.set_stability_params(
            max_sat_change_per_step,
            max_pressure_change_per_step,
            max_well_rate_change_fraction,
        );
        sim.set_well_control_modes("rate".to_string(), "rate".to_string());
        sim.set_target_well_rates(12_000.0, 5_400.0).unwrap();
        sim.set_target_well_surface_rates(2_831_680.0, 3_179.74)
            .unwrap();
        sim.set_well_bhp_limits(69.0, 621.0).unwrap();
        sim.set_permeability_per_layer(
            vec![500.0, 50.0, 200.0],
            vec![500.0, 50.0, 200.0],
            layer_perms_z,
        )
        .unwrap();
        sim.add_well(9, 9, 2, 69.0, 0.0762, 0.0, false).unwrap();
        sim.add_well(0, 0, 0, 621.0, 0.0762, 0.0, true).unwrap();
        sim
    }

    fn make_spe1_like_base_sim() -> ReservoirSimulator {
        make_spe1_like_sim(vec![500.0, 50.0, 200.0], 0.05, 50.0, 0.5)
    }

    fn make_spe1_like_low_kv_sim() -> ReservoirSimulator {
        make_spe1_like_sim(vec![50.0, 5.0, 20.0], 0.05, 50.0, 0.5)
    }

    fn producer_breakthrough_snapshot(sim: &ReservoirSimulator) -> String {
        let producer = &sim.wells[0];
        let producer_id = sim.idx(producer.i, producer.j, producer.k);
        let control = sim
            .resolve_well_control_for_pressures(producer, &sim.pressure)
            .expect("producer control should resolve");
        let q_res = sim
            .well_rate_m3_day_for_pressures(producer, &sim.pressure)
            .expect("producer transport rate should resolve");
        let scal = sim
            .scal_3p
            .as_ref()
            .expect("three-phase relperm should exist");
        let sw = sim.sat_water[producer_id];
        let sg = sim.sat_gas[producer_id];
        let lam_w = scal.k_rw(sw) / sim.get_mu_w(sim.pressure[producer_id]);
        let lam_o =
            scal.k_ro_stone2(sw, sg) / sim.get_mu_o_cell(producer_id, sim.pressure[producer_id]);
        let lam_g = scal.k_rg(sg) / sim.get_mu_g(sim.pressure[producer_id]);
        let lam_t = (lam_w + lam_o + lam_g).max(f64::EPSILON);
        let fg_local = lam_g / lam_t;
        let fo_local = lam_o / lam_t;
        let (_fw_sampled, fo_sampled, fg_sampled) =
            sim.producer_control_phase_fractions_for_pressures(producer, &sim.pressure);
        let bo = sim
            .get_b_o_cell(producer_id, sim.pressure[producer_id])
            .max(1e-9);
        let bg = sim.get_b_g(sim.pressure[producer_id]).max(1e-9);
        let oil_sc = q_res * fo_sampled / bo;
        let free_gas_sc = q_res * fg_sampled / bg;
        let dissolved_gas_sc = oil_sc * sim.rs[producer_id];
        let control_label = match control.decision {
            WellControlDecision::Disabled => "disabled".to_string(),
            WellControlDecision::Rate { q_m3_day } => format!("rate({q_m3_day:.2})"),
            WellControlDecision::Bhp { bhp_bar } => format!("bhp({bhp_bar:.2})"),
        };

        format!(
            "t={:7.2} d | p={:7.2} bar | sw={:.3} so={:.3} sg={:.3} rs={:.1} | lam_o={:.4} lam_g={:.4} fo_local={:.3} fg_local={:.3} fo_eff={:.3} fg_eff={:.3} | q_res={:8.2} | oil_sc={:7.2} gas_sc={:8.2} gor={:7.1} | ctrl={} | bhp_limited={} | avg_p={:7.2}",
            sim.time_days,
            sim.pressure[producer_id],
            sim.sat_water[producer_id],
            sim.sat_oil[producer_id],
            sim.sat_gas[producer_id],
            sim.rs[producer_id],
            lam_o,
            lam_g,
            fo_local,
            fg_local,
            fo_sampled,
            fg_sampled,
            q_res,
            oil_sc,
            free_gas_sc + dissolved_gas_sc,
            if oil_sc > 1e-9 {
                (free_gas_sc + dissolved_gas_sc) / oil_sc
            } else {
                0.0
            },
            control_label,
            control.bhp_limited,
            sim.average_reservoir_pressure_pv_weighted(),
        )
    }

    #[test]
    #[ignore = "debug helper for low-kv injector-cell balance investigation"]
    fn debug_low_kv_injector_balance_probe() {
        let mut sim = make_spe1_like_low_kv_sim();
        for _ in 0..80 {
            sim.step(5.0);
        }

        let injector_id = sim.idx(0, 0, 0);
        let east_id = sim.idx(1, 0, 0);
        let south_id = sim.idx(0, 1, 0);
        let down_id = sim.idx(0, 0, 1);
        let vp = sim.pore_volume_m3(injector_id);

        let (p_new, delta_w, delta_g_sc, delta_dg_sc, controls, stable_dt, converged, iterations) =
            sim.debug_calculate_fluxes(5.0);

        let injector_control = controls[1].expect("injector control should exist");
        let control_label = match injector_control.decision {
            WellControlDecision::Disabled => "disabled".to_string(),
            WellControlDecision::Rate { q_m3_day } => format!("rate({q_m3_day:.6})"),
            WellControlDecision::Bhp { bhp_bar } => format!("bhp({bhp_bar:.6})"),
        };
        let q_inj_res = match injector_control.decision {
            WellControlDecision::Rate { q_m3_day } => q_m3_day,
            WellControlDecision::Bhp { bhp_bar } => {
                let well = &sim.wells[1];
                (well.productivity_index * (p_new[injector_id] - bhp_bar)).min(0.0)
            }
            WellControlDecision::Disabled => 0.0,
        };
        let bg_inj = sim.get_b_g(p_new[injector_id]).max(1e-9);
        let q_inj_sc = -q_inj_res / bg_inj;

        println!(
            "injector state before update: p_old={:.6}, p_new={:.6}, sw={:.6}, so={:.6}, sg={:.6}, rs={:.6}, pv={:.6}",
            sim.pressure[injector_id],
            p_new[injector_id],
            sim.sat_water[injector_id],
            sim.sat_oil[injector_id],
            sim.sat_gas[injector_id],
            sim.rs[injector_id],
            vp,
        );
        println!(
            "neighbors before update: east so={:.6}, sg={:.6}; south so={:.6}, sg={:.6}; down so={:.6}, sg={:.6}",
            sim.sat_oil[east_id], sim.sat_gas[east_id],
            sim.sat_oil[south_id], sim.sat_gas[south_id],
            sim.sat_oil[down_id], sim.sat_gas[down_id],
        );
        println!(
            "flux/source deltas at injector: delta_w_m3={:.6}, delta_g_sc={:.6}, delta_dg_sc={:.6}, delta_sg_equiv={:.6}",
            delta_w[injector_id],
            delta_g_sc[injector_id],
            delta_dg_sc[injector_id],
            delta_g_sc[injector_id] * bg_inj / vp,
        );
        println!(
            "injector control/rate: q_res={:.6}, q_sc={:.6}, bg={:.9}, control={}, stable_dt={:.6}, converged={}, iterations={}",
            q_inj_res,
            q_inj_sc,
            bg_inj,
            control_label,
            stable_dt,
            converged,
            iterations,
        );
    }

    #[test]
    #[ignore = "debug helper for SPE1 producer breakthrough diagnostics"]
    fn debug_spe1_producer_breakthrough_probe() {
        let sample_times = [
            700.0, 750.0, 800.0, 850.0, 900.0, 950.0, 1000.0, 1050.0, 1095.0, 1100.0, 1150.0,
            1200.0, 1250.0,
        ];

        for (label, dt_days) in [("base_dt5", 5.0), ("base_dt0.25", 0.25)] {
            let mut sim = make_spe1_like_base_sim();
            let mut next_sample_idx = 0usize;
            let mut first_high_gor_reported = false;

            println!("=== {label} ===");
            while sim.time_days < 1250.0 {
                sim.step(dt_days);
                let latest = sim.rate_history.last().expect("rate history should exist");

                if !first_high_gor_reported && latest.producing_gor > 400.0 {
                    println!(
                        "first-high-gor: total_gor={:.1}, total_gas_sc={:.2}, oil_sc={:.2}",
                        latest.producing_gor,
                        latest.total_production_gas,
                        latest.total_production_oil,
                    );
                    println!("{}", producer_breakthrough_snapshot(&sim));
                    first_high_gor_reported = true;
                }

                while next_sample_idx < sample_times.len()
                    && sim.time_days + 1e-9 >= sample_times[next_sample_idx]
                {
                    println!("{}", producer_breakthrough_snapshot(&sim));
                    next_sample_idx += 1;
                }
            }
        }
    }

    #[test]
    #[ignore = "debug helper for SPE1 late-time producer decline diagnostics"]
    fn debug_spe1_producer_late_time_probe() {
        let sample_times = [
            1300.0, 1400.0, 1500.0, 1600.0, 1700.0, 1800.0, 1900.0, 1950.0, 1975.0, 2000.0, 2025.0,
            2050.0, 2100.0, 2250.0, 2500.0, 2750.0, 3000.0,
        ];

        for (label, dt_days) in [("base_dt5", 5.0), ("base_dt0.25", 0.25)] {
            let mut sim = make_spe1_like_base_sim();
            let mut next_sample_idx = 0usize;

            println!("=== {label} ===");
            while sim.time_days < 3000.0 {
                sim.step(dt_days);
                let latest = sim.rate_history.last().expect("rate history should exist");

                while next_sample_idx < sample_times.len()
                    && sim.time_days + 1e-9 >= sample_times[next_sample_idx]
                {
                    println!(
                        "{} | oil_hist={:.2} liq_hist={:.2} gor_hist={:.1} prod_bhp_frac={:.3} warning={}",
                        producer_breakthrough_snapshot(&sim),
                        latest.total_production_oil,
                        latest.total_production_liquid,
                        latest.producing_gor,
                        latest.producer_bhp_limited_fraction,
                        sim.get_last_solver_warning(),
                    );
                    next_sample_idx += 1;
                }
            }
        }
    }

    #[test]
    fn below_bubble_point_flash_conserves_total_gas_inventory() {
        use crate::pvt::{PvtRow, PvtTable};
        use nalgebra::DVector;

        let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        sim.set_three_phase_rel_perm_props(
            0.10, 0.10, 0.05, 0.05, 0.10, 2.0, 2.0, 1.5, 0.8, 0.9, 0.7,
        )
        .unwrap();
        sim.set_three_phase_mode_enabled(true);
        sim.set_gas_redissolution_enabled(false);
        sim.set_initial_pressure(175.0);
        sim.set_initial_saturation(0.10);
        sim.set_initial_gas_saturation(0.0);
        sim.pvt.c_o = 1e-5;
        sim.pvt_table = Some(PvtTable::new(
            vec![
                PvtRow {
                    p_bar: 100.0,
                    rs_m3m3: 5.0,
                    bo_m3m3: 1.05,
                    mu_o_cp: 1.5,
                    bg_m3m3: 0.01,
                    mu_g_cp: 0.02,
                },
                PvtRow {
                    p_bar: 150.0,
                    rs_m3m3: 15.0,
                    bo_m3m3: 1.12,
                    mu_o_cp: 1.2,
                    bg_m3m3: 0.006,
                    mu_g_cp: 0.025,
                },
                PvtRow {
                    p_bar: 200.0,
                    rs_m3m3: 15.0,
                    bo_m3m3: 1.119,
                    mu_o_cp: 1.3,
                    bg_m3m3: 0.0045,
                    mu_g_cp: 0.03,
                },
            ],
            sim.pvt.c_o,
        ));
        sim.set_initial_rs(15.0);

        let vp_m3 = sim.pore_volume_m3(0);
        let p_old = sim.pressure[0];
        let bg_old = sim.get_b_g(p_old).max(1e-9);
        let bo_old = sim.get_b_o_cell(0, p_old).max(1e-9);
        let gas_before_sc =
            sim.sat_gas[0] * vp_m3 / bg_old + (sim.sat_oil[0] * vp_m3 / bo_old) * sim.rs[0];

        sim.update_saturations_and_pressure(
            &DVector::from_vec(vec![125.0]),
            &vec![0.0],
            &vec![0.0],
            &vec![0.0],
            &[],
            1.0,
        );

        let p_new = sim.pressure[0];
        let bg_new = sim.get_b_g(p_new).max(1e-9);
        let bo_new = sim.get_b_o_cell(0, p_new).max(1e-9);
        let gas_after_sc =
            sim.sat_gas[0] * vp_m3 / bg_new + (sim.sat_oil[0] * vp_m3 / bo_new) * sim.rs[0];

        assert!(
            sim.sat_gas[0] > 0.0,
            "pressure drop below bubble point should liberate free gas"
        );
        assert!(
            (gas_after_sc - gas_before_sc).abs() < 1e-8,
            "local flash should conserve total gas inventory, before={}, after={}",
            gas_before_sc,
            gas_after_sc,
        );
    }

    #[test]
    fn reporting_reuses_transport_control_rates() {
        use crate::pvt::{PvtRow, PvtTable};
        use nalgebra::DVector;

        let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        sim.set_three_phase_rel_perm_props(
            0.10, 0.10, 0.05, 0.05, 0.10, 2.0, 2.0, 1.5, 0.8, 0.9, 0.7,
        )
        .unwrap();
        sim.set_three_phase_mode_enabled(true);
        sim.set_injected_fluid("gas").unwrap();
        sim.set_initial_pressure(100.0);
        sim.set_initial_saturation(0.10);
        sim.pvt_table = Some(PvtTable::new(
            vec![PvtRow {
                p_bar: 100.0,
                rs_m3m3: 0.0,
                bo_m3m3: 1.2,
                mu_o_cp: 1.0,
                bg_m3m3: 0.25,
                mu_g_cp: 0.02,
            }],
            sim.pvt.c_o,
        ));
        sim.set_well_control_modes("rate".to_string(), "bhp".to_string());
        sim.add_well(0, 0, 0, 100.0, 0.1, 0.0, true).unwrap();

        let controls = vec![Some(ResolvedWellControl {
            decision: WellControlDecision::Rate { q_m3_day: -30.0 },
            bhp_limited: false,
            producer_state: None,
        })];

        sim.update_saturations_and_pressure(
            &DVector::from_vec(vec![300.0]),
            &vec![0.0],
            &vec![0.0],
            &vec![0.0],
            &controls,
            1.0,
        );

        let latest = sim
            .rate_history
            .last()
            .expect("rate history should have an entry");
        assert!(
            (latest.total_injection - 360.0).abs() < 1e-6,
            "reporting should reuse the transport rate-control decision, got {}",
            latest.total_injection,
        );
        assert_eq!(latest.injector_bhp_limited_fraction, 0.0);
    }

    #[test]
    fn producing_gor_is_zero_when_oil_rate_is_negligible() {
        use crate::pvt::{PvtRow, PvtTable};
        use nalgebra::DVector;

        let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        sim.set_three_phase_rel_perm_props(
            0.10, 0.10, 0.05, 0.05, 0.10, 2.0, 2.0, 1.5, 0.8, 0.9, 0.7,
        )
        .unwrap();
        sim.set_three_phase_mode_enabled(true);
        sim.set_initial_pressure(100.0);
        sim.set_initial_saturation(0.10);
        sim.pvt_table = Some(PvtTable::new(
            vec![PvtRow {
                p_bar: 100.0,
                rs_m3m3: 0.0,
                bo_m3m3: 1.2,
                mu_o_cp: 1.0,
                bg_m3m3: 0.25,
                mu_g_cp: 0.02,
            }],
            sim.pvt.c_o,
        ));
        sim.add_well(0, 0, 0, 100.0, 0.1, 0.0, false).unwrap();

        let id = sim.idx(0, 0, 0);
        sim.pressure[id] = 300.0;
        sim.sat_water[id] = 0.12;
        sim.sat_gas[id] = 0.879_999;
        sim.sat_oil[id] = 0.000_001;

        let controls = vec![Some(ResolvedWellControl {
            decision: WellControlDecision::Bhp { bhp_bar: 100.0 },
            bhp_limited: false,
            producer_state: Some(ProducerControlState {
                water_fraction: 0.12,
                oil_fraction: 0.000_001,
                gas_fraction: 0.879_999,
                oil_fvf: sim.get_b_o_cell(id, sim.pressure[id]).max(1e-9),
                gas_fvf: sim.get_b_g(sim.pressure[id]).max(1e-9),
                rs_sm3_sm3: sim.rs[id],
            }),
        })];

        sim.update_saturations_and_pressure(
            &DVector::from_vec(vec![300.0]),
            &vec![0.0],
            &vec![0.0],
            &vec![0.0],
            &controls,
            1.0,
        );

        let latest = sim
            .rate_history
            .last()
            .expect("rate history should have an entry");
        assert_eq!(latest.producing_gor, 0.0);
    }

    #[test]
    fn three_phase_mode_disabled_sat_gas_stays_zero() {
        // In the default 2-phase mode, sat_gas must remain all zeros and sw+so=1.
        let mut sim = ReservoirSimulator::new(5, 1, 1, 0.2);
        sim.add_well(0, 0, 0, 400.0, 0.1, 0.0, true).unwrap();
        sim.add_well(4, 0, 0, 100.0, 0.1, 0.0, false).unwrap();

        for _ in 0..20 {
            sim.step(1.0);
        }

        for (i, &sg) in sim.sat_gas.iter().enumerate() {
            assert_eq!(
                sg, 0.0,
                "sat_gas[{}] should be zero in 2-phase mode, got {}",
                i, sg
            );
        }
        for i in 0..sim.nx * sim.ny * sim.nz {
            assert!(
                (sim.sat_water[i] + sim.sat_oil[i] - 1.0).abs() < 1e-8,
                "2-phase sw+so != 1 at cell {}",
                i
            );
        }
    }

    #[test]
    fn api_contract_rejects_invalid_3phase_relperm_params() {
        let mut sim = ReservoirSimulator::new(2, 2, 1, 0.2);

        // Endpoint sum >= 1.0 must be rejected
        err_contains(
            sim.set_three_phase_rel_perm_props(
                0.4, 0.3, 0.2, 0.2, 0.1, 2.0, 2.0, 1.5, 1.0, 1.0, 0.7,
            ),
            "must be < 1.0",
        );

        // Zero Corey exponent for water
        err_contains(
            sim.set_three_phase_rel_perm_props(
                0.1, 0.1, 0.05, 0.05, 0.10, 0.0, 2.0, 1.5, 1.0, 1.0, 0.7,
            ),
            "must be positive",
        );

        // Zero Corey exponent for gas
        err_contains(
            sim.set_three_phase_rel_perm_props(
                0.1, 0.1, 0.05, 0.05, 0.10, 2.0, 2.0, 0.0, 1.0, 1.0, 0.7,
            ),
            "must be positive",
        );
    }

    #[test]
    fn api_contract_rejects_invalid_gas_fluid_properties() {
        let mut sim = ReservoirSimulator::new(2, 2, 1, 0.2);

        err_contains(
            sim.set_gas_fluid_properties(0.0, 1e-4, 10.0),
            "must be positive",
        );
        err_contains(
            sim.set_gas_fluid_properties(-0.01, 1e-4, 10.0),
            "must be positive",
        );
        err_contains(
            sim.set_gas_fluid_properties(0.02, -1e-4, 10.0),
            "non-negative",
        );
        err_contains(
            sim.set_gas_fluid_properties(0.02, 1e-4, 0.0),
            "must be positive",
        );
    }

    #[test]
    fn api_contract_rejects_invalid_injected_fluid_string() {
        let mut sim = ReservoirSimulator::new(2, 2, 1, 0.2);

        err_contains(sim.set_injected_fluid("steam"), "Unknown injected fluid");
        err_contains(sim.set_injected_fluid(""), "Unknown injected fluid");

        // Valid strings must succeed
        assert!(sim.set_injected_fluid("water").is_ok());
        assert!(sim.set_injected_fluid("gas").is_ok());
    }

    #[test]
    fn api_contract_rejects_invalid_gas_oil_capillary_params() {
        let mut sim = ReservoirSimulator::new(2, 2, 1, 0.2);

        err_contains(sim.set_gas_oil_capillary_params(-1.0, 2.0), "non-negative");
        err_contains(sim.set_gas_oil_capillary_params(5.0, 0.0), "positive");

        // Valid parameters must succeed
        assert!(sim.set_gas_oil_capillary_params(0.0, 2.0).is_ok());
        assert!(sim.set_gas_oil_capillary_params(5.0, 1.5).is_ok());
    }

    #[test]
    fn per_layer_dz_affects_pore_volume_and_depth() {
        let mut sim = ReservoirSimulator::new(2, 2, 3, 0.25);
        sim.set_cell_dimensions_per_layer(100.0, 100.0, vec![6.0, 9.0, 15.0])
            .unwrap();

        // Pore volume = dx * dy * dz_k * porosity
        let id_k0 = sim.idx(0, 0, 0);
        let id_k1 = sim.idx(0, 0, 1);
        let id_k2 = sim.idx(0, 0, 2);

        let pv0 = sim.pore_volume_m3(id_k0);
        let pv1 = sim.pore_volume_m3(id_k1);
        let pv2 = sim.pore_volume_m3(id_k2);

        assert!((pv0 - 100.0 * 100.0 * 6.0 * 0.25).abs() < 1e-10);
        assert!((pv1 - 100.0 * 100.0 * 9.0 * 0.25).abs() < 1e-10);
        assert!((pv2 - 100.0 * 100.0 * 15.0 * 0.25).abs() < 1e-10);

        // Depth at k: sum of layers above + half of current layer
        let d0 = sim.depth_at_k(0);
        let d1 = sim.depth_at_k(1);
        let d2 = sim.depth_at_k(2);

        assert!(
            (d0 - 3.0).abs() < 1e-10,
            "k=0: depth should be 6/2 = 3, got {}",
            d0
        );
        assert!(
            (d1 - 10.5).abs() < 1e-10,
            "k=1: depth should be 6 + 9/2 = 10.5, got {}",
            d1
        );
        assert!(
            (d2 - 22.5).abs() < 1e-10,
            "k=2: depth should be 6 + 9 + 15/2 = 22.5, got {}",
            d2
        );
    }

    #[test]
    fn per_layer_dz_validation_rejects_invalid_inputs() {
        let mut sim = ReservoirSimulator::new(2, 2, 3, 0.2);

        // Wrong length
        err_contains(
            sim.set_cell_dimensions_per_layer(10.0, 10.0, vec![1.0, 2.0]),
            "length equal to nz",
        );

        // Non-positive dz
        err_contains(
            sim.set_cell_dimensions_per_layer(10.0, 10.0, vec![1.0, 0.0, 3.0]),
            "positive and finite",
        );

        // Non-positive dx
        err_contains(
            sim.set_cell_dimensions_per_layer(-1.0, 10.0, vec![1.0, 2.0, 3.0]),
            "positive",
        );
    }

    #[test]
    fn set_initial_gas_saturation_per_layer_applies_by_k() {
        let mut sim = ReservoirSimulator::new(2, 2, 3, 0.2);
        sim.set_initial_saturation(0.2); // Sw = 0.2 everywhere
        sim.set_initial_gas_saturation_per_layer(vec![0.7, 0.0, 0.0])
            .unwrap();

        // Layer 0: Sg = 0.7, So = 1 - 0.2 - 0.7 = 0.1
        for j in 0..2 {
            for i in 0..2 {
                let id = sim.idx(i, j, 0);
                assert!((sim.sat_gas[id] - 0.7).abs() < 1e-10);
                assert!((sim.sat_oil[id] - 0.1).abs() < 1e-10);
            }
        }

        // Layers 1-2: Sg = 0, So = 0.8
        for k in 1..3 {
            for j in 0..2 {
                for i in 0..2 {
                    let id = sim.idx(i, j, k);
                    assert!((sim.sat_gas[id] - 0.0).abs() < 1e-10);
                    assert!((sim.sat_oil[id] - 0.8).abs() < 1e-10);
                }
            }
        }
    }

    #[test]
    fn set_initial_gas_saturation_per_layer_clamps_to_available() {
        let mut sim = ReservoirSimulator::new(1, 1, 2, 0.2);
        sim.set_initial_saturation(0.5); // Sw = 0.5

        // Request Sg = 0.8 but only 0.5 is available (1 - Sw)
        sim.set_initial_gas_saturation_per_layer(vec![0.8, 0.0])
            .unwrap();

        let id0 = sim.idx(0, 0, 0);
        assert!(
            (sim.sat_gas[id0] - 0.5).abs() < 1e-10,
            "Sg should clamp to 0.5"
        );
        assert!((sim.sat_oil[id0] - 0.0).abs() < 1e-10, "So should be 0");
    }

    #[test]
    fn set_initial_gas_saturation_per_layer_validation() {
        let mut sim = ReservoirSimulator::new(2, 2, 3, 0.2);

        // Wrong length
        err_contains(
            sim.set_initial_gas_saturation_per_layer(vec![0.5, 0.0]),
            "length equal to nz",
        );

        // Out of range
        err_contains(
            sim.set_initial_gas_saturation_per_layer(vec![0.5, -0.1, 0.0]),
            "within [0, 1]",
        );
    }

    #[test]
    fn non_uniform_dz_transmissibility_z_direction() {
        let mut sim = ReservoirSimulator::new(1, 1, 2, 0.2);
        sim.set_cell_dimensions_per_layer(10.0, 10.0, vec![6.0, 15.0])
            .unwrap();
        sim.set_permeability_random_seeded(100.0, 100.0, 42)
            .unwrap();

        let id0 = sim.idx(0, 0, 0);
        let id1 = sim.idx(0, 0, 1);

        let t_z = sim.geometric_transmissibility(id0, id1, 'z');

        // For z-direction: area = dx * dy = 100, dist = (dz0 + dz1) / 2 = 10.5
        // T = k_h * area / dist
        let kz0 = sim.perm_z[id0];
        let kz1 = sim.perm_z[id1];
        let k_h = 2.0 * kz0 * kz1 / (kz0 + kz1);
        let expected = k_h * 100.0 / 10.5;

        assert!(
            (t_z - expected).abs() / expected < 1e-9,
            "Z-transmissibility with non-uniform dz: expected {}, got {}",
            expected,
            t_z
        );
    }

    #[test]
    fn average_reservoir_pressure_is_pv_weighted() {
        let mut sim = ReservoirSimulator::new(1, 1, 2, 0.25);
        sim.set_cell_dimensions_per_layer(10.0, 10.0, vec![1.0, 9.0])
            .unwrap();

        let id0 = sim.idx(0, 0, 0);
        let id1 = sim.idx(0, 0, 1);
        sim.pressure[id0] = 100.0;
        sim.pressure[id1] = 200.0;

        let pv0 = sim.pore_volume_m3(id0);
        let pv1 = sim.pore_volume_m3(id1);
        let expected = (100.0 * pv0 + 200.0 * pv1) / (pv0 + pv1);

        assert!(
            (sim.average_reservoir_pressure_pv_weighted() - expected).abs() < 1e-12,
            "Expected PV-weighted pressure {}, got {}",
            expected,
            sim.average_reservoir_pressure_pv_weighted()
        );
        assert!(
            (sim.average_reservoir_pressure_pv_weighted() - 150.0).abs() > 1e-6,
            "PV-weighted average should differ from arithmetic mean when pore volumes differ"
        );
    }
}
