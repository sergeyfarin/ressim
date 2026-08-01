use crate::{InjectedFluid, ReservoirSimulator};

/// Fraction of total pore volume the cumulative volumetric material-balance
/// error may reach before the run is called unreliable.
///
/// Well-behaved shipped scenarios remain many orders of magnitude below this
/// threshold. Explicit transport run past its stability limit, and unsupported
/// well configurations historically, can instead produce plausible-looking
/// results while creating or destroying material.
const MATERIAL_BALANCE_WARNING_FRACTION: f64 = 1e-3;

impl ReservoirSimulator {
    pub(crate) fn step_internal(&mut self, target_dt_days: f64) {
        // Lag the wellbore column on the state entering the step, so every
        // completion's datum offset is a constant for the whole step (and every
        // FIM Newton iteration inside it).
        self.refresh_well_head_offsets();

        if self.fim_enabled {
            crate::fim::timestep::step_internal(self, target_dt_days);
        } else {
            crate::impes::timestep::step_internal(self, target_dt_days);
        }

        self.warn_on_material_balance_drift();
    }

    /// Raise a solver-independent warning when the shared reporting ledger says
    /// reservoir volumes have stopped being conserved.
    ///
    /// Deliberately additive: a warning raised by either timestep controller is
    /// more specific about the failure, so it remains the user-facing message.
    fn warn_on_material_balance_drift(&mut self) {
        if !self.last_solver_warning.is_empty() {
            return;
        }
        // This warning and threshold are calibrated for water transport. Gas-
        // injection/depletion runs use the gas-component ledger instead; their
        // connate-water `PV * Sw` can move under rock compaction without being a
        // valid water-conservation oracle.
        if (self.three_phase_mode && self.injected_fluid == InjectedFluid::Gas)
            || self.cumulative_injection_m3.abs() + self.cumulative_production_m3.abs() <= 1e-12
        {
            return;
        }
        let pore_volume_m3: f64 = (0..self.nx * self.ny * self.nz)
            .map(|idx| self.pore_volume_m3(idx))
            .sum();
        if !(pore_volume_m3 > 0.0) {
            return;
        }
        let drift_fraction = self.cumulative_mb_error_m3.abs() / pore_volume_m3;
        if drift_fraction > MATERIAL_BALANCE_WARNING_FRACTION {
            self.last_solver_warning = format!(
                "Material balance has drifted {:.2}% of pore volume ({:.4e} m3) - volumes are no longer \
                 being conserved and reported production cannot be trusted. Most often this is the \
                 explicit saturation update run past its stability limit (reduce \
                 max_sat_change_per_step); it is also seen with degenerate well layouts such as two \
                 wells on different controls in one block.",
                drift_fraction * 100.0,
                self.cumulative_mb_error_m3
            );
        }
    }

    pub(crate) fn solve_rs_for_dissolved_gas(
        &self,
        pressure_bar: f64,
        water_saturation: f64,
        gas_saturation: f64,
        pore_volume_m3: f64,
        dissolved_gas_sc: f64,
        rs_upper: f64,
    ) -> f64 {
        let table = match &self.pvt_table {
            Some(table) => table,
            None => return 0.0,
        };

        let target_dissolved_gas_sc = dissolved_gas_sc.max(0.0);
        if target_dissolved_gas_sc <= 0.0 || rs_upper <= 0.0 {
            return 0.0;
        }

        let oil_saturation = (1.0 - water_saturation - gas_saturation).max(0.0);
        if oil_saturation <= 1e-12 || pore_volume_m3 <= 0.0 {
            return 0.0;
        }

        let mut low = 0.0;
        let mut high = rs_upper.max(0.0);
        for _ in 0..64 {
            let mid = 0.5 * (low + high);
            let (bo_mid, _) = table.interpolate_oil(pressure_bar, mid);
            let dissolved_mid = (oil_saturation * pore_volume_m3 / bo_mid.max(1e-9)) * mid;
            if dissolved_mid < target_dissolved_gas_sc {
                low = mid;
            } else {
                high = mid;
            }
        }

        0.5 * (low + high)
    }

    pub(crate) fn split_gas_inventory_after_transport(
        &self,
        pressure_bar: f64,
        pore_volume_m3: f64,
        water_saturation: f64,
        transported_free_gas_sc: f64,
        dissolved_gas_sc: f64,
        drsdt0_base_rs: Option<f64>,
    ) -> (f64, f64, f64) {
        let table = match &self.pvt_table {
            Some(table) => table,
            None => {
                let bg = self.get_b_g(pressure_bar).max(1e-9);
                let sg = ((transported_free_gas_sc.max(0.0) * bg) / pore_volume_m3.max(1e-9))
                    .clamp(0.0, (1.0 - water_saturation).max(0.0));
                let so = (1.0 - water_saturation - sg).max(0.0);
                return (sg, so, 0.0);
            }
        };

        let total_hydrocarbon_saturation = (1.0 - water_saturation).max(0.0);
        let bg = self.get_b_g(pressure_bar).max(1e-9);
        let free_gas_sc_transport = transported_free_gas_sc.max(0.0);
        let sg_transport = ((free_gas_sc_transport * bg) / pore_volume_m3.max(1e-9))
            .clamp(0.0, total_hydrocarbon_saturation);
        let so_transport = (total_hydrocarbon_saturation - sg_transport).max(0.0);
        let dissolved_gas_sc = dissolved_gas_sc.max(0.0);

        let rs_max = table.interpolate(pressure_bar).rs_m3m3.max(0.0);
        let rs_dissolution_cap = if self.gas_redissolution_enabled {
            rs_max
        } else {
            drsdt0_base_rs
                .map(|base_rs| base_rs.max(0.0).min(rs_max))
                .unwrap_or(rs_max)
        };
        let (bo_dissolution_cap, _) = table.interpolate_oil(pressure_bar, rs_dissolution_cap);
        let bo_dissolution_cap = bo_dissolution_cap.max(1e-9);

        if !self.gas_redissolution_enabled {
            let max_dissolved_sc_transport =
                (so_transport * pore_volume_m3 / bo_dissolution_cap) * rs_dissolution_cap;
            if dissolved_gas_sc <= max_dissolved_sc_transport + 1e-9 {
                let rs = self.solve_rs_for_dissolved_gas(
                    pressure_bar,
                    water_saturation,
                    sg_transport,
                    pore_volume_m3,
                    dissolved_gas_sc,
                    rs_dissolution_cap,
                );
                return (sg_transport, so_transport, rs);
            }
        }

        let total_gas_sc = free_gas_sc_transport + dissolved_gas_sc;
        let (rs_saturated, bo_saturated) = if self.gas_redissolution_enabled {
            let (bo_sat, _) = table.interpolate_oil(pressure_bar, rs_max);
            (rs_max, bo_sat.max(1e-9))
        } else {
            (rs_dissolution_cap, bo_dissolution_cap)
        };
        let max_all_dissolved_sc =
            (total_hydrocarbon_saturation * pore_volume_m3 / bo_saturated) * rs_saturated;
        if self.gas_redissolution_enabled && total_gas_sc <= max_all_dissolved_sc + 1e-9 {
            let rs = self.solve_rs_for_dissolved_gas(
                pressure_bar,
                water_saturation,
                0.0,
                pore_volume_m3,
                total_gas_sc,
                rs_saturated,
            );
            return (0.0, total_hydrocarbon_saturation, rs);
        }

        let denom = (1.0 / bg) - (rs_saturated / bo_saturated);
        let sg_saturated = if denom.abs() > 1e-12 {
            ((total_gas_sc / pore_volume_m3)
                - (total_hydrocarbon_saturation * rs_saturated / bo_saturated))
                / denom
        } else {
            sg_transport
        };
        let sg_lower_bound = if self.gas_redissolution_enabled {
            0.0
        } else {
            sg_transport
        };
        let sg = sg_saturated.clamp(sg_lower_bound, total_hydrocarbon_saturation);
        let so = (total_hydrocarbon_saturation - sg).max(0.0);
        (sg, so, rs_saturated)
    }
}
