use crate::{InjectedFluid, ReservoirSimulator, Well, DARCY_METRIC_FACTOR};

#[derive(Clone, Copy)]
pub(crate) enum WellControlDecision {
    Disabled,
    Rate { q_m3_day: f64 },
    Bhp { bhp_bar: f64 },
}

#[derive(Clone, Copy)]
pub(crate) struct ResolvedWellControl {
    pub(crate) decision: WellControlDecision,
    pub(crate) bhp_limited: bool,
}

impl ReservoirSimulator {
    // ── Productivity index ────────────────────────────────────────────────────

    pub(crate) fn calculate_well_productivity_index(
        &self,
        id: usize,
        well_radius: f64,
        skin: f64,
    ) -> Result<f64, String> {
        // Calculate equivalent radius (Peaceman's model)
        let kx = self.perm_x[id];
        let ky = self.perm_y[id];
        if !kx.is_finite() || !ky.is_finite() || kx <= 0.0 || ky <= 0.0 {
            return Err(format!(
                "Cell permeability must be positive and finite for well PI calculation, got kx={}, ky={}",
                kx, ky
            ));
        }

        let r_eq =
            0.28 * f64::sqrt(
                f64::sqrt(kx / ky) * self.dx.powi(2) + f64::sqrt(ky / kx) * self.dy.powi(2),
            ) / ((kx / ky).powf(0.25) + (ky / kx).powf(0.25));
        if !r_eq.is_finite() || r_eq <= 0.0 {
            return Err(format!(
                "Equivalent radius must be positive and finite, got: {}",
                r_eq
            ));
        }

        if r_eq <= well_radius {
            return Err(format!(
                "Equivalent radius must be greater than well radius for valid PI. r_eq={}, rw={}",
                r_eq, well_radius
            ));
        }

        // Calculate productivity index (PI)
        let k_avg = f64::sqrt(kx * ky); // Geometric mean of horizontal permeabilities
        let total_mobility = self.total_mobility(id);
        if !k_avg.is_finite() || k_avg <= 0.0 {
            return Err(format!(
                "Average permeability must be positive and finite, got: {}",
                k_avg
            ));
        }
        if !total_mobility.is_finite() || total_mobility < 0.0 {
            return Err(format!(
                "Total mobility must be finite and non-negative, got: {}",
                total_mobility
            ));
        }

        let denom = f64::ln(r_eq / well_radius) + skin;
        if !denom.is_finite() || denom.abs() <= f64::EPSILON {
            return Err(format!(
                "Invalid PI denominator ln(r_eq/r_w)+skin = {}. Check well radius and skin.",
                denom
            ));
        }

        // Peaceman's well index formula for metric/bar units (m³, bar, day)
        // PI = C * 2π * k_avg * h * total_mobility / (ln(r_eq/r_w) + skin)
        Ok(
            (DARCY_METRIC_FACTOR * 2.0 * std::f64::consts::PI * k_avg * self.dz_at(id) * total_mobility)
                / denom,
        )
    }

    pub(crate) fn update_dynamic_well_productivity_indices(&mut self) {
        let mut updated_pi: Vec<Option<f64>> = Vec::with_capacity(self.wells.len());

        for well in self.wells.iter() {
            let id = self.idx(well.i, well.j, well.k);

            let maybe_pi = self
                .calculate_well_productivity_index(id, well.well_radius, well.skin)
                .ok()
                .filter(|pi| pi.is_finite() && *pi >= 0.0);
            updated_pi.push(maybe_pi);
        }

        for (well, maybe_pi) in self.wells.iter_mut().zip(updated_pi.into_iter()) {
            if let Some(pi) = maybe_pi {
                well.productivity_index = pi;
            }
        }
    }

    // ── Well counting ─────────────────────────────────────────────────────────

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn injector_well_count(&self) -> usize {
        self.wells.iter().filter(|w| w.injector).count()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn producer_well_count(&self) -> usize {
        self.wells.iter().filter(|w| !w.injector).count()
    }

    // ── Rate computation ──────────────────────────────────────────────────────

    pub(crate) fn completion_rate_for_bhp(&self, well: &Well, pressure_bar: f64, bhp_bar: f64) -> Option<f64> {
        if !well.productivity_index.is_finite() || !pressure_bar.is_finite() || !bhp_bar.is_finite() {
            return None;
        }
        let raw_rate = well.productivity_index * (pressure_bar - bhp_bar);
        if !raw_rate.is_finite() {
            return None;
        }
        if well.injector {
            Some(raw_rate.min(0.0))
        } else {
            Some(raw_rate.max(0.0))
        }
    }

    pub(crate) fn completion_surface_rate_sc_day(&self, well: &Well, pressure_bar: f64, bhp_bar: f64) -> Option<f64> {
        let q_m3_day = self.completion_rate_for_bhp(well, pressure_bar, bhp_bar)?;
        let id = self.idx(well.i, well.j, well.k);
        if well.injector {
            let injected_sc_rate = match self.injected_fluid {
                InjectedFluid::Water => (-q_m3_day) / self.b_w.max(1e-9),
                InjectedFluid::Gas => (-q_m3_day) / self.get_b_g(pressure_bar).max(1e-9),
            };
            Some(injected_sc_rate.max(0.0))
        } else {
            let oil_fraction = self.producer_oil_fraction_at_pressure(id, pressure_bar);
            let oil_rate_sc = q_m3_day * oil_fraction / self.get_b_o_cell(id, pressure_bar).max(1e-9);
            Some(oil_rate_sc.max(0.0))
        }
    }

    pub(crate) fn well_transport_rate_from_control(
        &self,
        well: &Well,
        control: ResolvedWellControl,
        pressure_bar: f64,
    ) -> Option<f64> {
        match control.decision {
            WellControlDecision::Disabled => Some(0.0),
            WellControlDecision::Rate { q_m3_day } => Some(q_m3_day),
            WellControlDecision::Bhp { bhp_bar } => {
                self.completion_rate_for_bhp(well, pressure_bar, bhp_bar)
            }
        }
    }

    // ── Group BHP solve ───────────────────────────────────────────────────────

    pub(crate) fn solve_group_bhp_for_pressures(&self, injector: bool, pressures: &[f64]) -> Option<(f64, bool)> {
        let use_rate_control = if injector {
            self.injector_rate_controlled
        } else {
            self.producer_rate_controlled
        };
        if !use_rate_control {
            return None;
        }

        let wells: Vec<&Well> = self.wells.iter().filter(|well| well.injector == injector).collect();
        if wells.is_empty() {
            return None;
        }

        let total_surface_target = if injector {
            self.target_injector_surface_rate_m3_day
        } else {
            self.target_producer_surface_rate_m3_day
        };
        let total_reservoir_target = if injector {
            self.target_injector_rate_m3_day
        } else {
            self.target_producer_rate_m3_day
        };

        let total_rate_for_bhp = |bhp_bar: f64| -> f64 {
            wells.iter()
                .filter_map(|well| {
                    let id = self.idx(well.i, well.j, well.k);
                    let pressure_bar = pressures[id];
                    if injector {
                        match total_surface_target {
                            Some(_) => self.completion_surface_rate_sc_day(well, pressure_bar, bhp_bar),
                            None => self.completion_rate_for_bhp(well, pressure_bar, bhp_bar).map(|q| (-q).max(0.0)),
                        }
                    } else {
                        match total_surface_target {
                            Some(_) => self.completion_surface_rate_sc_day(well, pressure_bar, bhp_bar),
                            None => self.completion_rate_for_bhp(well, pressure_bar, bhp_bar),
                        }
                    }
                })
                .sum()
        };

        let target_rate = if let Some(surface_target) = total_surface_target {
            surface_target.max(0.0)
        } else {
            total_reservoir_target.max(0.0)
        };

        let group_min_pressure = wells
            .iter()
            .map(|well| pressures[self.idx(well.i, well.j, well.k)])
            .fold(f64::INFINITY, f64::min);
        let group_max_pressure = wells
            .iter()
            .map(|well| pressures[self.idx(well.i, well.j, well.k)])
            .fold(f64::NEG_INFINITY, f64::max);

        if !group_min_pressure.is_finite() || !group_max_pressure.is_finite() {
            return None;
        }

        if injector {
            let bhp_limit = self.well_bhp_max;
            let max_achievable_rate = total_rate_for_bhp(bhp_limit);
            if target_rate >= max_achievable_rate - 1e-9 {
                return Some((bhp_limit, true));
            }

            let mut low = group_min_pressure.min(bhp_limit);
            let mut high = bhp_limit;
            for _ in 0..64 {
                let mid = 0.5 * (low + high);
                let rate_mid = total_rate_for_bhp(mid);
                if rate_mid < target_rate {
                    low = mid;
                } else {
                    high = mid;
                }
            }
            Some((0.5 * (low + high), false))
        } else {
            let bhp_limit = self.well_bhp_min;
            let max_achievable_rate = total_rate_for_bhp(bhp_limit);
            if target_rate >= max_achievable_rate - 1e-9 {
                return Some((bhp_limit, true));
            }

            let mut low = bhp_limit;
            let mut high = group_max_pressure.max(bhp_limit);
            for _ in 0..64 {
                let mid = 0.5 * (low + high);
                let rate_mid = total_rate_for_bhp(mid);
                if rate_mid > target_rate {
                    low = mid;
                } else {
                    high = mid;
                }
            }
            Some((0.5 * (low + high), false))
        }
    }

    // ── Control resolution ────────────────────────────────────────────────────

    pub(crate) fn resolve_well_control_for_pressures(
        &self,
        well: &Well,
        pressures: &[f64],
    ) -> Option<ResolvedWellControl> {
        if well.injector && !self.injector_enabled {
            return Some(ResolvedWellControl {
                decision: WellControlDecision::Disabled,
                bhp_limited: false,
            });
        }

        let use_rate_control = if well.injector {
            self.injector_rate_controlled
        } else {
            self.producer_rate_controlled
        };

        let id = self.idx(well.i, well.j, well.k);
        let pressure_bar = pressures[id];

        if use_rate_control {
            let (group_bhp, bhp_limited) = self.solve_group_bhp_for_pressures(well.injector, pressures)?;
            let q_target = self.completion_rate_for_bhp(well, pressure_bar, group_bhp)?;
            return Some(ResolvedWellControl {
                decision: if bhp_limited {
                    WellControlDecision::Bhp { bhp_bar: group_bhp }
                } else {
                    WellControlDecision::Rate { q_m3_day: q_target }
                },
                bhp_limited,
            });
        }

        if !well.productivity_index.is_finite()
            || !well.bhp.is_finite()
            || !pressure_bar.is_finite()
        {
            return None;
        }

        Some(ResolvedWellControl {
            decision: WellControlDecision::Bhp { bhp_bar: well.bhp },
            bhp_limited: false,
        })
    }

    // ── Query wrappers ────────────────────────────────────────────────────────

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn target_rate_m3_day(&self, well: &Well, pressure_bar: f64) -> Option<f64> {
        if well.injector && !self.injector_enabled {
            return Some(0.0);
        }

        let use_rate_control = if well.injector {
            self.injector_rate_controlled
        } else {
            self.producer_rate_controlled
        };

        if use_rate_control {
            if well.injector {
                let n_inj = self.injector_well_count();
                if n_inj == 0 {
                    return Some(0.0);
                }
                if let Some(surface_rate_sc_day) = self.target_injector_surface_rate_m3_day {
                    let surface_rate_per_well = surface_rate_sc_day / n_inj as f64;
                    let reservoir_rate_per_well = match self.injected_fluid {
                        InjectedFluid::Water => surface_rate_per_well * self.b_w.max(1e-9),
                        InjectedFluid::Gas => surface_rate_per_well * self.get_b_g(pressure_bar).max(1e-9),
                    };
                    return Some(-reservoir_rate_per_well);
                }
                return Some(-(self.target_injector_rate_m3_day / n_inj as f64));
            }

            let n_prod = self.producer_well_count();
            if n_prod == 0 {
                return Some(0.0);
            }
            if let Some(surface_rate_sc_day) = self.target_producer_surface_rate_m3_day {
                let surface_rate_per_well = surface_rate_sc_day / n_prod as f64;
                let id = self.idx(well.i, well.j, well.k);
                let (fw, fg) = if self.three_phase_mode {
                    (self.frac_flow_water_3p(id), self.frac_flow_gas(id))
                } else {
                    (self.frac_flow_water(id), 0.0)
                };
                let oil_fraction = (1.0 - fw - fg).max(1e-6);
                let reservoir_rate_per_well = surface_rate_per_well
                    * self.get_b_o_cell(id, pressure_bar).max(1e-9)
                    / oil_fraction;
                return Some(reservoir_rate_per_well);
            }
            return Some(self.target_producer_rate_m3_day / n_prod as f64);
        }

        None
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn resolve_well_control(
        &self,
        well: &Well,
        pressure_bar: f64,
    ) -> Option<ResolvedWellControl> {
        let pressures = self.group_pressures_with_override(well, pressure_bar);
        self.resolve_well_control_for_pressures(well, &pressures)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn well_rate_m3_day(&self, well: &Well, pressure_bar: f64) -> Option<f64> {
        let pressures = self.group_pressures_with_override(well, pressure_bar);
        self.well_rate_m3_day_for_pressures(well, &pressures)
    }

    pub(crate) fn well_rate_m3_day_for_pressures(&self, well: &Well, pressures: &[f64]) -> Option<f64> {
        let id = self.idx(well.i, well.j, well.k);
        let pressure_bar = pressures[id];
        match self.resolve_well_control_for_pressures(well, pressures)?.decision {
            WellControlDecision::Disabled => Some(0.0),
            WellControlDecision::Rate { q_m3_day } => Some(q_m3_day),
            WellControlDecision::Bhp { bhp_bar } => {
                let q_m3_day = self.completion_rate_for_bhp(well, pressure_bar, bhp_bar)?;
                if q_m3_day.is_finite() {
                    Some(q_m3_day)
                } else {
                    None
                }
            }
        }
    }

    pub(crate) fn group_pressures_with_override(&self, well: &Well, pressure_bar: f64) -> Vec<f64> {
        let mut pressures = self.pressure.clone();
        let id = self.idx(well.i, well.j, well.k);
        if id < pressures.len() {
            pressures[id] = pressure_bar;
        }
        pressures
    }
}
