use crate::{InjectedFluid, ReservoirSimulator, Well};

/// Conversion factor from mD·m²/(m·cP) to m³/day/bar.
const DARCY_METRIC_FACTOR: f64 = 8.526_988_8e-3;

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
    pub(crate) producer_state: Option<ProducerControlState>,
}

#[derive(Clone, Copy)]
pub(crate) struct ProducerControlState {
    pub(crate) water_fraction: f64,
    pub(crate) oil_fraction: f64,
    pub(crate) gas_fraction: f64,
    pub(crate) oil_fvf: f64,
    pub(crate) gas_fvf: f64,
    pub(crate) rs_sm3_sm3: f64,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum WellControlGroupKey {
    ExplicitId(String),
    LegacyFingerprint {
        injector: bool,
        i: usize,
        j: usize,
        bhp_bits: u64,
        radius_bits: u64,
        skin_bits: u64,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct WellControlConfig {
    pub(crate) enabled: bool,
    pub(crate) rate_controlled: bool,
    pub(crate) target_rate_m3_day: Option<f64>,
    pub(crate) target_surface_rate_m3_day: Option<f64>,
    pub(crate) bhp_limit: f64,
    pub(crate) bhp_target: f64,
}

impl ReservoirSimulator {
    pub(crate) fn well_control_mode_matches(
        old_control: Option<ResolvedWellControl>,
        new_control: Option<ResolvedWellControl>,
    ) -> bool {
        match (old_control, new_control) {
            (None, None) => true,
            (Some(old_control), Some(new_control)) => {
                let decision_matches = matches!(
                    (old_control.decision, new_control.decision),
                    (WellControlDecision::Disabled, WellControlDecision::Disabled)
                        | (
                            WellControlDecision::Rate { .. },
                            WellControlDecision::Rate { .. }
                        )
                        | (
                            WellControlDecision::Bhp { .. },
                            WellControlDecision::Bhp { .. }
                        )
                );
                decision_matches && old_control.bhp_limited == new_control.bhp_limited
            }
            _ => false,
        }
    }

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

        Ok((DARCY_METRIC_FACTOR
            * 2.0
            * std::f64::consts::PI
            * k_avg
            * self.dz_at(id)
            * total_mobility)
            / denom)
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

    pub(crate) fn well_control_group_key(&self, well: &Well) -> WellControlGroupKey {
        if let Some(well_id) = &well.physical_well_id {
            WellControlGroupKey::ExplicitId(well_id.clone())
        } else {
            WellControlGroupKey::LegacyFingerprint {
                injector: well.injector,
                i: well.i,
                j: well.j,
                bhp_bits: well.bhp.to_bits(),
                radius_bits: well.well_radius.to_bits(),
                skin_bits: well.skin.to_bits(),
            }
        }
    }

    fn well_control_group_indices(&self, well: &Well) -> Vec<usize> {
        let group_key = self.well_control_group_key(well);
        self.wells
            .iter()
            .enumerate()
            .filter_map(|(idx, candidate)| {
                (self.well_control_group_key(candidate) == group_key).then_some(idx)
            })
            .collect()
    }

    pub(crate) fn well_control_config(&self, well: &Well) -> WellControlConfig {
        let explicit_schedule = well.schedule.has_explicit_control();
        let enabled = if explicit_schedule {
            well.schedule.enabled
        } else if well.injector {
            self.injector_enabled
        } else {
            true
        };

        let rate_controlled = if !enabled {
            false
        } else if explicit_schedule {
            matches!(
                well.schedule.control_mode.as_deref(),
                Some(mode) if mode.eq_ignore_ascii_case("rate")
            )
        } else if well.injector {
            self.injector_rate_controlled
        } else {
            self.producer_rate_controlled
        };

        let target_rate_m3_day = if rate_controlled {
            if explicit_schedule {
                well.schedule.target_rate_m3_day
            } else if well.injector {
                Some(self.target_injector_rate_m3_day)
            } else {
                Some(self.target_producer_rate_m3_day)
            }
        } else {
            None
        };

        let target_surface_rate_m3_day = if rate_controlled {
            if explicit_schedule {
                well.schedule.target_surface_rate_m3_day
            } else if well.injector {
                self.target_injector_surface_rate_m3_day
            } else {
                self.target_producer_surface_rate_m3_day
            }
        } else {
            None
        };

        let family_bhp_limit = if well.injector {
            self.well_bhp_max
        } else {
            self.well_bhp_min
        };

        WellControlConfig {
            enabled,
            rate_controlled,
            target_rate_m3_day,
            target_surface_rate_m3_day,
            bhp_limit: if explicit_schedule {
                well.schedule.bhp_limit.unwrap_or(family_bhp_limit)
            } else {
                family_bhp_limit
            },
            bhp_target: well.bhp,
        }
    }

    pub(crate) fn producer_control_phase_fractions_for_pressures(
        &self,
        well: &Well,
        pressures: &[f64],
    ) -> (f64, f64, f64) {
        // Use the well completion cell only. Averaging over a neighbourhood
        // dilutes the saturation signal, causing premature fractional-flow
        // response before the flood front reaches the well cell.
        let id = self.idx(well.i, well.j, well.k);
        let pressure_bar = pressures.get(id).copied().unwrap_or(self.pressure[id]);

        let (lambda_w, lambda_o, lambda_g) = if self.three_phase_mode {
            let (w, o, g) = self.phase_mobilities_3p_at_pressure(id, pressure_bar);
            eprintln!("DBG 3p id={id} sw={} sg={} lam=({w},{o},{g})", self.sat_water[id], self.sat_gas[id]);
            (w.max(0.0), o.max(0.0), g.max(0.0))
        } else {
            let (w, o) = self.phase_mobilities_at_pressure(id, pressure_bar);
            eprintln!("DBG 2p id={id} sw={} lam=({w},{o})", self.sat_water[id]);
            (w.max(0.0), o.max(0.0), 0.0)
        };

        let lambda_total = (lambda_w + lambda_o + lambda_g).max(f64::EPSILON);
        (
            (lambda_w / lambda_total).clamp(0.0, 1.0),
            (lambda_o / lambda_total).clamp(0.0, 1.0),
            (lambda_g / lambda_total).clamp(0.0, 1.0),
        )
    }

    fn producer_control_state_for_pressures(
        &self,
        well: &Well,
        pressures: &[f64],
    ) -> ProducerControlState {
        let id = self.idx(well.i, well.j, well.k);
        let pressure_bar = pressures.get(id).copied().unwrap_or(self.pressure[id]);
        let (water_fraction, oil_fraction, gas_fraction) =
            self.producer_control_phase_fractions_for_pressures(well, pressures);

        ProducerControlState {
            water_fraction,
            oil_fraction,
            gas_fraction,
            oil_fvf: self.get_b_o_cell(id, pressure_bar).max(1e-9),
            gas_fvf: self.get_b_g(pressure_bar).max(1e-9),
            rs_sm3_sm3: self.rs[id].max(0.0),
        }
    }

    pub(crate) fn producer_control_state_from_resolved_control(
        &self,
        well: &Well,
        control: ResolvedWellControl,
        pressures: &[f64],
    ) -> ProducerControlState {
        control
            .producer_state
            .unwrap_or_else(|| self.producer_control_state_for_pressures(well, pressures))
    }

    pub(crate) fn completion_rate_for_bhp(
        &self,
        well: &Well,
        pressure_bar: f64,
        bhp_bar: f64,
    ) -> Option<f64> {
        if !well.productivity_index.is_finite() || !pressure_bar.is_finite() || !bhp_bar.is_finite()
        {
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

    fn completion_surface_rate_sc_day(
        &self,
        well: &Well,
        pressures: &[f64],
        pressure_bar: f64,
        bhp_bar: f64,
    ) -> Option<f64> {
        let q_m3_day = self.completion_rate_for_bhp(well, pressure_bar, bhp_bar)?;
        if well.injector {
            let injected_sc_rate = match self.injected_fluid {
                InjectedFluid::Water => (-q_m3_day) / self.b_w.max(1e-9),
                InjectedFluid::Gas => (-q_m3_day) / self.get_b_g(pressure_bar).max(1e-9),
            };
            Some(injected_sc_rate.max(0.0))
        } else {
            let producer_state = self.producer_control_state_for_pressures(well, pressures);
            let oil_rate_sc = q_m3_day * producer_state.oil_fraction / producer_state.oil_fvf;
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

    fn total_rate_for_well_bhp(&self, well: &Well, pressures: &[f64], bhp_bar: f64) -> f64 {
        let config = self.well_control_config(well);
        eprintln!("DBG total_rate bhp={bhp_bar} surface_rate={:?}", config.target_surface_rate_m3_day);
        self.well_control_group_indices(well)
            .into_iter()
            .filter_map(|well_idx| {
                let group_well = &self.wells[well_idx];
                let id = self.idx(group_well.i, group_well.j, group_well.k);
                let pressure_bar = pressures[id];
                if group_well.injector {
                    match config.target_surface_rate_m3_day {
                        Some(_) => self.completion_surface_rate_sc_day(
                            group_well,
                            pressures,
                            pressure_bar,
                            bhp_bar,
                        ),
                        None => self
                            .completion_rate_for_bhp(group_well, pressure_bar, bhp_bar)
                            .map(|q| (-q).max(0.0)),
                    }
                } else {
                    match config.target_surface_rate_m3_day {
                        Some(_) => self.completion_surface_rate_sc_day(
                            group_well,
                            pressures,
                            pressure_bar,
                            bhp_bar,
                        ),
                        None => self.completion_rate_for_bhp(group_well, pressure_bar, bhp_bar),
                    }
                }
            })
            .sum()
    }

    pub(crate) fn solve_well_bhp_for_pressures(
        &self,
        well: &Well,
        pressures: &[f64],
    ) -> Option<(f64, bool)> {
        let config = self.well_control_config(well);
        if !config.rate_controlled {
            return None;
        }

        let wells = self.well_control_group_indices(well);
        if wells.is_empty() {
            return None;
        }

        let target_rate = config
            .target_surface_rate_m3_day
            .or(config.target_rate_m3_day)
            .unwrap_or(0.0)
            .max(0.0);

        let group_min_pressure = wells
            .iter()
            .map(|well_idx| {
                let group_well = &self.wells[*well_idx];
                pressures[self.idx(group_well.i, group_well.j, group_well.k)]
            })
            .fold(f64::INFINITY, f64::min);
        let group_max_pressure = wells
            .iter()
            .map(|well_idx| {
                let group_well = &self.wells[*well_idx];
                pressures[self.idx(group_well.i, group_well.j, group_well.k)]
            })
            .fold(f64::NEG_INFINITY, f64::max);

        if !group_min_pressure.is_finite() || !group_max_pressure.is_finite() {
            return None;
        }

        // Hysteresis margin: stay BHP-limited until achievable rate exceeds
        // target by this fraction, preventing control mode oscillation.
        let bhp_margin = target_rate * 0.05;

        if well.injector {
            let bhp_limit = config.bhp_limit;
            let max_achievable_rate = self.total_rate_for_well_bhp(well, pressures, bhp_limit);
            if target_rate >= max_achievable_rate - bhp_margin {
                return Some((bhp_limit, true));
            }

            let mut low = group_min_pressure.min(bhp_limit);
            let mut high = bhp_limit;
            for _ in 0..64 {
                let mid = 0.5 * (low + high);
                let rate_mid = self.total_rate_for_well_bhp(well, pressures, mid);
                if rate_mid < target_rate {
                    low = mid;
                } else {
                    high = mid;
                }
            }
            Some((0.5 * (low + high), false))
        } else {
            let bhp_limit = config.bhp_limit;
            let max_achievable_rate = self.total_rate_for_well_bhp(well, pressures, bhp_limit);
            if target_rate >= max_achievable_rate - bhp_margin {
                return Some((bhp_limit, true));
            }

            let mut low = bhp_limit;
            let mut high = group_max_pressure.max(bhp_limit);
            for _ in 0..64 {
                let mid = 0.5 * (low + high);
                let rate_mid = self.total_rate_for_well_bhp(well, pressures, mid);
                if rate_mid > target_rate {
                    low = mid;
                } else {
                    high = mid;
                }
            }
            Some((0.5 * (low + high), false))
        }
    }

    pub(crate) fn resolve_well_control_for_pressures(
        &self,
        well: &Well,
        pressures: &[f64],
    ) -> Option<ResolvedWellControl> {
        let config = self.well_control_config(well);
        if !config.enabled {
            return Some(ResolvedWellControl {
                decision: WellControlDecision::Disabled,
                bhp_limited: false,
                producer_state: None,
            });
        }

        let id = self.idx(well.i, well.j, well.k);
        let pressure_bar = pressures[id];
        let producer_state = if well.injector {
            None
        } else {
            Some(self.producer_control_state_for_pressures(well, pressures))
        };

        if config.rate_controlled {
            let (group_bhp, bhp_limited) = self.solve_well_bhp_for_pressures(well, pressures)?;
            let q_target = self.completion_rate_for_bhp(well, pressure_bar, group_bhp)?;
            return Some(ResolvedWellControl {
                decision: if bhp_limited {
                    WellControlDecision::Bhp { bhp_bar: group_bhp }
                } else {
                    WellControlDecision::Rate { q_m3_day: q_target }
                },
                bhp_limited,
                producer_state,
            });
        }

        if !well.productivity_index.is_finite()
            || !well.bhp.is_finite()
            || !pressure_bar.is_finite()
        {
            return None;
        }

        Some(ResolvedWellControl {
            decision: WellControlDecision::Bhp {
                bhp_bar: config.bhp_target,
            },
            bhp_limited: false,
            producer_state,
        })
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn target_rate_m3_day(&self, well: &Well, pressure_bar: f64) -> Option<f64> {
        let config = self.well_control_config(well);
        if !config.enabled {
            return Some(0.0);
        }

        if config.rate_controlled {
            let pressures = self.group_pressures_with_override(well, pressure_bar);
            let (bhp_bar, _) = self.solve_well_bhp_for_pressures(well, &pressures)?;
            return self.completion_rate_for_bhp(well, pressure_bar, bhp_bar);
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

    pub(crate) fn well_rate_m3_day_for_pressures(
        &self,
        well: &Well,
        pressures: &[f64],
    ) -> Option<f64> {
        let id = self.idx(well.i, well.j, well.k);
        let pressure_bar = pressures[id];
        match self
            .resolve_well_control_for_pressures(well, pressures)?
            .decision
        {
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

    fn group_pressures_with_override(&self, well: &Well, pressure_bar: f64) -> Vec<f64> {
        let mut pressures = self.pressure.clone();
        let id = self.idx(well.i, well.j, well.k);
        if id < pressures.len() {
            pressures[id] = pressure_bar;
        }
        pressures
    }
}
