use std::collections::HashMap;

use crate::fim::state::{FimState, HydrocarbonState};
use crate::well_control::ProducerControlState;
use crate::{InjectedFluid, ReservoirSimulator, Well};

const DARCY_METRIC_FACTOR: f64 = 8.526_988_8e-3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FimPerforation {
    pub(crate) well_entry_index: usize,
    pub(crate) physical_well_index: usize,
    pub(crate) cell_index: usize,
    pub(crate) i: usize,
    pub(crate) j: usize,
    pub(crate) k: usize,
    pub(crate) injector: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FimPhysicalWell {
    pub(crate) representative_well_index: usize,
    pub(crate) injector: bool,
    pub(crate) head_i: usize,
    pub(crate) head_j: usize,
    pub(crate) perforation_indices: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FimWellTopology {
    pub(crate) wells: Vec<FimPhysicalWell>,
    pub(crate) perforations: Vec<FimPerforation>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct FimPerforationLocalBlock<'a> {
    topology: &'a FimWellTopology,
    state: &'a FimState,
    perf_idx: usize,
}

impl<'a> FimPerforationLocalBlock<'a> {
    pub(crate) fn perforation(self) -> &'a FimPerforation {
        &self.topology.perforations[self.perf_idx]
    }

    pub(crate) fn cell_idx(self) -> usize {
        self.perforation().cell_index
    }

    pub(crate) fn physical_well_idx(self) -> usize {
        self.perforation().physical_well_index
    }

    pub(crate) fn rate_unknown_offset(self) -> usize {
        self.state.perforation_rate_unknown_offset(self.perf_idx)
    }

    pub(crate) fn equation_offset(self) -> usize {
        self.state.perforation_equation_offset(self.perf_idx)
    }

    pub(crate) fn control_influence_cells(self, sim: &ReservoirSimulator) -> Vec<usize> {
        perforation_control_cells(sim, self.perforation())
    }

    pub(crate) fn connection_rate_for_bhp(
        self,
        sim: &ReservoirSimulator,
        bhp_bar: f64,
    ) -> Option<f64> {
        connection_rate_for_bhp(sim, self.state, self.topology, self.perf_idx, bhp_bar)
    }

    pub(crate) fn component_rate_derivatives_sc_day(self, sim: &ReservoirSimulator) -> [f64; 3] {
        perforation_component_rate_derivatives_sc_day(sim, self.state, self.topology, self.perf_idx)
    }

    pub(crate) fn target_rate_derivative(self, sim: &ReservoirSimulator) -> f64 {
        perforation_target_rate_derivative(sim, self.state, self.topology, self.perf_idx)
    }

    pub(crate) fn connection_bhp_derivative(
        self,
        sim: &ReservoirSimulator,
        bhp_bar: f64,
    ) -> Option<f64> {
        perforation_connection_bhp_derivative(sim, self.state, self.topology, self.perf_idx, bhp_bar)
    }

    pub(crate) fn connection_cell_derivatives(
        self,
        sim: &ReservoirSimulator,
        bhp_bar: f64,
    ) -> Option<[f64; 3]> {
        perforation_connection_cell_derivatives(sim, self.state, self.topology, self.perf_idx, bhp_bar)
    }

    pub(crate) fn component_rate_cell_derivatives_sc_day_by_var(
        self,
        sim: &ReservoirSimulator,
        cell_idx: usize,
    ) -> [[f64; 3]; 3] {
        perforation_component_rate_cell_derivatives_sc_day_by_var(
            sim,
            self.state,
            self.topology,
            self.perf_idx,
            cell_idx,
        )
    }

    pub(crate) fn surface_rate_cell_derivatives_sc_day(
        self,
        sim: &ReservoirSimulator,
        cell_idx: usize,
    ) -> [f64; 3] {
        perforation_surface_rate_cell_derivatives_sc_day(
            sim,
            self.state,
            self.topology,
            self.perf_idx,
            cell_idx,
        )
    }

    pub(crate) fn surface_rate_sc_day(
        self,
        sim: &ReservoirSimulator,
        q_m3_day: f64,
    ) -> Option<f64> {
        perforation_surface_rate_sc_day(sim, self.state, self.topology, self.perf_idx, q_m3_day)
    }

    pub(crate) fn current_rate_unknown_m3_day(self) -> f64 {
        self.state.perforation_rates_m3_day[self.perf_idx]
    }

    pub(crate) fn rate_residual(self, sim: &ReservoirSimulator) -> Option<f64> {
        let well_block = well_local_block(self.topology, self.state, self.physical_well_idx());
        let control = well_block.control(sim);
        if !control.enabled {
            return Some(self.current_rate_unknown_m3_day());
        }

        let q_connection = self.connection_rate_for_bhp(sim, well_block.bhp_bar())?;
        Some(self.current_rate_unknown_m3_day() - q_connection)
    }

    pub(crate) fn residual_diagnostics(
        self,
        sim: &ReservoirSimulator,
    ) -> Option<PerforationResidualDiagnostics> {
        let perforation = self.perforation();
        let well_block = well_local_block(self.topology, self.state, self.physical_well_idx());
        let control = well_block.control(sim);
        let well = perforation_well(sim, perforation);
        let cell = self.state.cell(perforation.cell_index);
        let derived = self.state.derive_cell(sim, perforation.cell_index);
        let mobilities =
            sim.phase_mobilities_for_state(cell.sw, derived.sg, cell.pressure_bar, derived.rs);
        let well_index = geometric_well_index(sim, perforation)?;
        let connection_mobility = (mobilities.water + mobilities.oil + mobilities.gas).max(0.0);
        let bhp_bar = well_block.bhp_bar();
        let drawdown_bar = cell.pressure_bar - bhp_bar;
        let raw_connection_m3_day = well_index * connection_mobility * drawdown_bar;
        if !raw_connection_m3_day.is_finite() {
            return None;
        }

        let q_connection_m3_day = if !control.enabled {
            0.0
        } else if well.injector {
            raw_connection_m3_day.min(0.0)
        } else {
            raw_connection_m3_day.max(0.0)
        };
        let q_unknown_m3_day = self.current_rate_unknown_m3_day();
        let surface_rate_unknown_sc_day = if control.enabled && control.uses_surface_target {
            self.surface_rate_sc_day(sim, q_unknown_m3_day)
        } else {
            None
        };
        let (target_rate_sc_day, actual_well_rate_sc_day, bhp_slack, rate_slack) =
            if control.enabled && control.rate_controlled {
                let (bhp_slack, rate_slack) = well_block.control_slacks(sim)?;
                (
                    control.target_rate,
                    Some(well_block.total_rate_from_unknowns(sim)),
                    Some(bhp_slack),
                    Some(rate_slack),
                )
            } else {
                (None, None, None, None)
            };
        let (
            frozen_consistent_bhp_bar,
            frozen_consistent_perf_rate_m3_day,
            frozen_consistent_well_rate_sc_day,
            frozen_consistent_bhp_limited,
        ) = if !control.enabled {
            (Some(control.bhp_target), Some(0.0), Some(0.0), None)
        } else if control.rate_controlled {
            if let Some((consistent_bhp_bar, bhp_limited)) = well_block.solve_bhp_from_target(sim)
            {
                (
                    Some(consistent_bhp_bar),
                    self.connection_rate_for_bhp(sim, consistent_bhp_bar),
                    Some(well_block.total_rate_for_bhp(sim, consistent_bhp_bar)),
                    Some(bhp_limited),
                )
            } else {
                (None, None, None, None)
            }
        } else {
            let consistent_bhp_bar = control.bhp_target;
            (
                Some(consistent_bhp_bar),
                self.connection_rate_for_bhp(sim, consistent_bhp_bar),
                None,
                None,
            )
        };

        Some(PerforationResidualDiagnostics {
            perf_idx: self.perf_idx,
            physical_well_idx: perforation.physical_well_index,
            enabled: control.enabled,
            injector: well.injector,
            q_unknown_m3_day,
            q_connection_m3_day,
            raw_connection_m3_day,
            drawdown_bar,
            well_index,
            connection_mobility,
            bhp_bar,
            cell_pressure_bar: cell.pressure_bar,
            surface_rate_unknown_sc_day,
            target_rate_sc_day,
            actual_well_rate_sc_day,
            bhp_slack,
            rate_slack,
            frozen_consistent_bhp_bar,
            frozen_consistent_perf_rate_m3_day,
            frozen_consistent_well_rate_sc_day,
            frozen_consistent_bhp_limited,
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct FimWellLocalBlock<'a> {
    topology: &'a FimWellTopology,
    state: &'a FimState,
    well_idx: usize,
}

impl<'a> FimWellLocalBlock<'a> {
    #[cfg(test)]
    pub(crate) fn well_idx(self) -> usize {
        self.well_idx
    }

    pub(crate) fn well(self) -> &'a FimPhysicalWell {
        &self.topology.wells[self.well_idx]
    }

    pub(crate) fn bhp_unknown_offset(self) -> usize {
        self.state.well_bhp_unknown_offset(self.well_idx)
    }

    pub(crate) fn equation_offset(self) -> usize {
        self.state.well_equation_offset(self.well_idx)
    }

    pub(crate) fn bhp_bar(self) -> f64 {
        self.state.well_bhp[self.well_idx]
    }

    pub(crate) fn perforation_indices(self) -> &'a [usize] {
        &self.well().perforation_indices
    }

    pub(crate) fn perforations(self) -> impl Iterator<Item = FimPerforationLocalBlock<'a>> + 'a {
        self.perforation_indices()
            .iter()
            .copied()
            .map(move |perf_idx| FimPerforationLocalBlock {
                topology: self.topology,
                state: self.state,
                perf_idx,
            })
    }

    pub(crate) fn control(self, sim: &ReservoirSimulator) -> PhysicalWellControl {
        physical_well_control(sim, self.topology, self.well_idx)
    }

    pub(crate) fn control_slacks(self, sim: &ReservoirSimulator) -> Option<(f64, f64)> {
        let control = self.control(sim);
        let well = self.well();
        if !control.enabled || !control.rate_controlled {
            return None;
        }

        let target_rate = control.target_rate?;
        let actual_rate = self.total_rate_from_unknowns(sim);
        let bhp_slack = if well.injector {
            control.bhp_limit - self.bhp_bar()
        } else {
            self.bhp_bar() - control.bhp_limit
        };
        let rate_slack = target_rate - actual_rate;
        Some((bhp_slack, rate_slack))
    }

    pub(crate) fn constraint_residual(self, sim: &ReservoirSimulator) -> Option<f64> {
        let control = self.control(sim);
        let bhp_bar = self.bhp_bar();

        if !control.enabled || !control.rate_controlled {
            return Some(bhp_bar - control.bhp_target);
        }

        let (bhp_slack, rate_slack) = self.control_slacks(sim)?;
        let bhp_scale = control.bhp_limit.abs().max(1.0);
        let rate_scale = control.target_rate.unwrap_or(1.0).abs().max(1.0);
        Some(fischer_burmeister(
            bhp_slack / bhp_scale,
            rate_slack / rate_scale,
        ))
    }

    pub(crate) fn total_rate_for_bhp(
        self,
        sim: &ReservoirSimulator,
        bhp_bar: f64,
    ) -> f64 {
        let control = self.control(sim);
        let injector = self.well().injector;
        self.perforations()
            .filter_map(|perf| {
                let q_m3_day = perf.connection_rate_for_bhp(sim, bhp_bar)?;
                if injector {
                    if control.uses_surface_target {
                        perf.surface_rate_sc_day(sim, q_m3_day)
                    } else {
                        Some((-q_m3_day).max(0.0))
                    }
                } else if control.uses_surface_target {
                    perf.surface_rate_sc_day(sim, q_m3_day)
                } else {
                    Some(q_m3_day.max(0.0))
                }
            })
            .sum()
    }

    pub(crate) fn total_rate_from_unknowns(self, sim: &ReservoirSimulator) -> f64 {
        let control = self.control(sim);
        let injector = self.well().injector;
        self.perforations()
            .filter_map(|perf| {
                let q_m3_day = perf.current_rate_unknown_m3_day();
                if injector {
                    if control.uses_surface_target {
                        perf.surface_rate_sc_day(sim, q_m3_day)
                    } else {
                        Some((-q_m3_day).max(0.0))
                    }
                } else if control.uses_surface_target {
                    perf.surface_rate_sc_day(sim, q_m3_day)
                } else {
                    Some(q_m3_day.max(0.0))
                }
            })
            .sum()
    }

    pub(crate) fn solve_bhp_from_target(
        self,
        sim: &ReservoirSimulator,
    ) -> Option<(f64, bool)> {
        let control = self.control(sim);
        if !control.enabled || !control.rate_controlled {
            return None;
        }

        let perforations = self.perforations().collect::<Vec<_>>();
        if perforations.is_empty() {
            return None;
        }

        let injector = self.well().injector;
        let target_rate = control.target_rate?;
        let min_pressure = perforations
            .iter()
            .map(|perf| self.state.cell(perf.cell_idx()).pressure_bar)
            .fold(f64::INFINITY, f64::min);
        let max_pressure = perforations
            .iter()
            .map(|perf| self.state.cell(perf.cell_idx()).pressure_bar)
            .fold(f64::NEG_INFINITY, f64::max);

        if !min_pressure.is_finite() || !max_pressure.is_finite() {
            return None;
        }

        if injector {
            let bhp_limit = control.bhp_limit;
            let max_achievable_rate = self.total_rate_for_bhp(sim, bhp_limit);
            if target_rate >= max_achievable_rate - 1e-9 {
                return Some((bhp_limit, true));
            }

            let mut low = min_pressure.min(bhp_limit);
            let mut high = bhp_limit;
            for _ in 0..64 {
                let mid = 0.5 * (low + high);
                let rate_mid = self.total_rate_for_bhp(sim, mid);
                if rate_mid < target_rate {
                    low = mid;
                } else {
                    high = mid;
                }
            }
            Some((0.5 * (low + high), false))
        } else {
            let bhp_limit = control.bhp_limit;
            let max_achievable_rate = self.total_rate_for_bhp(sim, bhp_limit);
            if target_rate >= max_achievable_rate - 1e-9 {
                return Some((bhp_limit, true));
            }

            let mut low = bhp_limit;
            let mut high = max_pressure.max(bhp_limit);
            for _ in 0..64 {
                let mid = 0.5 * (low + high);
                let rate_mid = self.total_rate_for_bhp(sim, mid);
                if rate_mid > target_rate {
                    low = mid;
                } else {
                    high = mid;
                }
            }
            Some((0.5 * (low + high), false))
        }
    }
}

pub(crate) fn well_local_block<'a>(
    topology: &'a FimWellTopology,
    state: &'a FimState,
    well_idx: usize,
) -> FimWellLocalBlock<'a> {
    FimWellLocalBlock {
        topology,
        state,
        well_idx,
    }
}

pub(crate) fn perforation_local_block<'a>(
    topology: &'a FimWellTopology,
    state: &'a FimState,
    perf_idx: usize,
) -> FimPerforationLocalBlock<'a> {
    FimPerforationLocalBlock {
        topology,
        state,
        perf_idx,
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum WellGroupingKey {
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

pub(crate) fn build_well_topology(sim: &ReservoirSimulator) -> FimWellTopology {
    let mut wells = Vec::new();
    let mut perforations = Vec::with_capacity(sim.wells.len());
    let mut groups = HashMap::<WellGroupingKey, usize>::new();

    for (well_entry_index, well) in sim.wells.iter().enumerate() {
        let key = if let Some(well_id) = &well.physical_well_id {
            WellGroupingKey::ExplicitId(well_id.clone())
        } else {
            WellGroupingKey::LegacyFingerprint {
                injector: well.injector,
                i: well.i,
                j: well.j,
                bhp_bits: well.bhp.to_bits(),
                radius_bits: well.well_radius.to_bits(),
                skin_bits: well.skin.to_bits(),
            }
        };

        let physical_well_index = *groups.entry(key).or_insert_with(|| {
            let index = wells.len();
            wells.push(FimPhysicalWell {
                representative_well_index: well_entry_index,
                injector: well.injector,
                head_i: well.i,
                head_j: well.j,
                perforation_indices: Vec::new(),
            });
            index
        });

        let perforation_index = perforations.len();
        perforations.push(FimPerforation {
            well_entry_index,
            physical_well_index,
            cell_index: sim.idx(well.i, well.j, well.k),
            i: well.i,
            j: well.j,
            k: well.k,
            injector: well.injector,
        });
        wells[physical_well_index]
            .perforation_indices
            .push(perforation_index);
    }

    FimWellTopology {
        wells,
        perforations,
    }
}

fn perforation_well<'a>(sim: &'a ReservoirSimulator, perforation: &FimPerforation) -> &'a Well {
    &sim.wells[perforation.well_entry_index]
}

fn physical_well<'a>(
    sim: &'a ReservoirSimulator,
    topology: &FimWellTopology,
    well_idx: usize,
) -> &'a Well {
    &sim.wells[topology.wells[well_idx].representative_well_index]
}

fn effective_injected_fluid(sim: &ReservoirSimulator) -> InjectedFluid {
    if sim.three_phase_mode {
        sim.injected_fluid
    } else {
        InjectedFluid::Water
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PhysicalWellControl {
    pub(crate) enabled: bool,
    pub(crate) rate_controlled: bool,
    pub(crate) uses_surface_target: bool,
    pub(crate) target_rate: Option<f64>,
    pub(crate) bhp_limit: f64,
    pub(crate) bhp_target: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PerforationResidualDiagnostics {
    pub(crate) perf_idx: usize,
    pub(crate) physical_well_idx: usize,
    pub(crate) enabled: bool,
    pub(crate) injector: bool,
    pub(crate) q_unknown_m3_day: f64,
    pub(crate) q_connection_m3_day: f64,
    pub(crate) raw_connection_m3_day: f64,
    pub(crate) drawdown_bar: f64,
    pub(crate) well_index: f64,
    pub(crate) connection_mobility: f64,
    pub(crate) bhp_bar: f64,
    pub(crate) cell_pressure_bar: f64,
    pub(crate) surface_rate_unknown_sc_day: Option<f64>,
    pub(crate) target_rate_sc_day: Option<f64>,
    pub(crate) actual_well_rate_sc_day: Option<f64>,
    pub(crate) bhp_slack: Option<f64>,
    pub(crate) rate_slack: Option<f64>,
    pub(crate) frozen_consistent_bhp_bar: Option<f64>,
    pub(crate) frozen_consistent_perf_rate_m3_day: Option<f64>,
    pub(crate) frozen_consistent_well_rate_sc_day: Option<f64>,
    pub(crate) frozen_consistent_bhp_limited: Option<bool>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct LocalPhaseSensitivity {
    mobilities: [f64; 3],
    mobility_derivatives: [[f64; 3]; 3],
    bo: f64,
    bg: f64,
    rs: f64,
    bo_derivatives: [f64; 3],
    bg_derivatives: [f64; 3],
    rs_derivatives: [f64; 3],
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ProducerRateSensitivity {
    water_fraction: f64,
    oil_fraction: f64,
    gas_fraction: f64,
    oil_fvf: f64,
    gas_fvf: f64,
    rs_sm3_sm3: f64,
    water_fraction_derivatives: [f64; 3],
    oil_fraction_derivatives: [f64; 3],
    gas_fraction_derivatives: [f64; 3],
    oil_fvf_derivatives: [f64; 3],
    gas_fvf_derivatives: [f64; 3],
    rs_derivatives: [f64; 3],
}

fn family_rate_controlled(sim: &ReservoirSimulator, injector: bool) -> bool {
    if injector {
        sim.injector_rate_controlled && sim.injector_enabled
    } else {
        sim.producer_rate_controlled
    }
}

fn family_uses_surface_target(sim: &ReservoirSimulator, injector: bool) -> bool {
    if injector {
        sim.target_injector_surface_rate_m3_day.is_some()
    } else {
        sim.target_producer_surface_rate_m3_day.is_some()
    }
}

fn family_target_rate(sim: &ReservoirSimulator, injector: bool) -> Option<f64> {
    if injector {
        if !family_rate_controlled(sim, true) {
            return None;
        }
        Some(
            sim.target_injector_surface_rate_m3_day
                .unwrap_or(sim.target_injector_rate_m3_day)
                .max(0.0),
        )
    } else {
        if !family_rate_controlled(sim, false) {
            return None;
        }
        Some(
            sim.target_producer_surface_rate_m3_day
                .unwrap_or(sim.target_producer_rate_m3_day)
                .max(0.0),
        )
    }
}

fn family_bhp_limit(sim: &ReservoirSimulator, injector: bool) -> f64 {
    if injector {
        sim.well_bhp_max
    } else {
        sim.well_bhp_min
    }
}

pub(crate) fn physical_well_control(
    sim: &ReservoirSimulator,
    topology: &FimWellTopology,
    well_idx: usize,
) -> PhysicalWellControl {
    let well = physical_well(sim, topology, well_idx);
    let explicit_schedule = well.schedule.has_explicit_control();
    let injector = topology.wells[well_idx].injector;

    let enabled = if explicit_schedule {
        well.schedule.enabled
    } else if injector {
        sim.injector_enabled
    } else {
        true
    };

    let rate_controlled = if !enabled {
        false
    } else if explicit_schedule {
        matches!(well.schedule.control_mode.as_deref(), Some("rate"))
    } else {
        family_rate_controlled(sim, injector)
    };

    let uses_surface_target = if explicit_schedule {
        well.schedule.target_surface_rate_m3_day.is_some()
    } else {
        family_uses_surface_target(sim, injector)
    };

    let target_rate = if !rate_controlled {
        None
    } else if explicit_schedule {
        Some(
            well.schedule
                .target_surface_rate_m3_day
                .or(well.schedule.target_rate_m3_day)
                .unwrap_or(0.0)
                .max(0.0),
        )
    } else {
        family_target_rate(sim, injector)
    };

    let bhp_limit = if explicit_schedule {
        well.schedule
            .bhp_limit
            .unwrap_or_else(|| family_bhp_limit(sim, injector))
    } else {
        family_bhp_limit(sim, injector)
    };

    PhysicalWellControl {
        enabled,
        rate_controlled,
        uses_surface_target,
        target_rate,
        bhp_limit,
        bhp_target: well.bhp,
    }
}

fn geometric_well_index(sim: &ReservoirSimulator, perforation: &FimPerforation) -> Option<f64> {
    let well = perforation_well(sim, perforation);
    let id = perforation.cell_index;
    let kx = sim.perm_x[id];
    let ky = sim.perm_y[id];
    if !kx.is_finite() || !ky.is_finite() || kx <= 0.0 || ky <= 0.0 {
        return None;
    }

    let r_eq = 0.28
        * f64::sqrt(f64::sqrt(kx / ky) * sim.dx.powi(2) + f64::sqrt(ky / kx) * sim.dy.powi(2))
        / ((kx / ky).powf(0.25) + (ky / kx).powf(0.25));
    if !r_eq.is_finite() || r_eq <= well.well_radius {
        return None;
    }

    let k_avg = f64::sqrt(kx * ky);
    let denom = f64::ln(r_eq / well.well_radius) + well.skin;
    if !k_avg.is_finite() || k_avg <= 0.0 || !denom.is_finite() || denom.abs() <= f64::EPSILON {
        return None;
    }

    Some(DARCY_METRIC_FACTOR * 2.0 * std::f64::consts::PI * k_avg * sim.dz_at(id) / denom)
}

pub(crate) fn producer_control_state(
    sim: &ReservoirSimulator,
    state: &FimState,
    perforation: &FimPerforation,
) -> ProducerControlState {
    let i_min = perforation.i.saturating_sub(1);
    let i_max = (perforation.i + 1).min(sim.nx.saturating_sub(1));
    let j_min = perforation.j.saturating_sub(1);
    let j_max = (perforation.j + 1).min(sim.ny.saturating_sub(1));
    let k = perforation.k;

    let mut lambda_w_sum = 0.0;
    let mut lambda_o_sum = 0.0;
    let mut lambda_g_sum = 0.0;

    for j in j_min..=j_max {
        for i in i_min..=i_max {
            let id = sim.idx(i, j, k);
            let cell = state.cell(id);
            let derived = state.derive_cell(sim, id);
            let mobilities =
                sim.phase_mobilities_for_state(cell.sw, derived.sg, cell.pressure_bar, derived.rs);
            lambda_w_sum += mobilities.water.max(0.0);
            lambda_o_sum += mobilities.oil.max(0.0);
            lambda_g_sum += mobilities.gas.max(0.0);
        }
    }

    let lambda_total = (lambda_w_sum + lambda_o_sum + lambda_g_sum).max(f64::EPSILON);
    let id = perforation.cell_index;
    let derived = state.derive_cell(sim, id);

    ProducerControlState {
        water_fraction: (lambda_w_sum / lambda_total).clamp(0.0, 1.0),
        oil_fraction: (lambda_o_sum / lambda_total).clamp(0.0, 1.0),
        gas_fraction: (lambda_g_sum / lambda_total).clamp(0.0, 1.0),
        oil_fvf: derived.bo.max(1e-9),
        gas_fvf: derived.bg.max(1e-9),
        rs_sm3_sm3: derived.rs.max(0.0),
    }
}

fn perforation_control_cells(sim: &ReservoirSimulator, perforation: &FimPerforation) -> Vec<usize> {
    if perforation.injector {
        return vec![perforation.cell_index];
    }

    let i_min = perforation.i.saturating_sub(1);
    let i_max = (perforation.i + 1).min(sim.nx.saturating_sub(1));
    let j_min = perforation.j.saturating_sub(1);
    let j_max = (perforation.j + 1).min(sim.ny.saturating_sub(1));
    let mut cells = Vec::with_capacity((i_max - i_min + 1) * (j_max - j_min + 1));
    for j in j_min..=j_max {
        for i in i_min..=i_max {
            cells.push(sim.idx(i, j, perforation.k));
        }
    }
    cells
}

fn local_phase_sensitivity(
    sim: &ReservoirSimulator,
    state: &FimState,
    cell_idx: usize,
) -> LocalPhaseSensitivity {
    let cell = state.cell(cell_idx);
    let derived = state.derive_cell(sim, cell_idx);
    let saturated = cell.regime == HydrocarbonState::Saturated;

    let krw = if sim.three_phase_mode {
        sim.scal_3p
            .as_ref()
            .map(|rock| rock.k_rw(cell.sw))
            .unwrap_or_else(|| sim.scal.k_rw(cell.sw))
    } else {
        sim.scal.k_rw(cell.sw)
    };
    let dkrw_dsw = if sim.three_phase_mode {
        sim.scal_3p
            .as_ref()
            .map(|rock| rock.d_k_rw_d_sw(cell.sw))
            .unwrap_or_else(|| sim.scal.d_k_rw_d_sw(cell.sw))
    } else {
        sim.scal.d_k_rw_d_sw(cell.sw)
    };

    let (kro, dkro_dsw, dkro_dsg, krg, dkrg_dsg) = if sim.three_phase_mode {
        sim.scal_3p
            .as_ref()
            .map(|rock| {
                (
                    rock.k_ro_stone2(cell.sw, derived.sg),
                    rock.d_k_ro_stone2_d_sw(cell.sw, derived.sg),
                    rock.d_k_ro_stone2_d_sg(cell.sw, derived.sg),
                    rock.k_rg(derived.sg),
                    rock.d_k_rg_d_sg(derived.sg),
                )
            })
            .unwrap_or_else(|| {
                (
                    sim.scal.k_ro(cell.sw),
                    sim.scal.d_k_ro_d_sw(cell.sw),
                    0.0,
                    0.0,
                    0.0,
                )
            })
    } else {
        (
            sim.scal.k_ro(cell.sw),
            sim.scal.d_k_ro_d_sw(cell.sw),
            0.0,
            0.0,
            0.0,
        )
    };

    let mu_w = sim.get_mu_w(cell.pressure_bar).max(1e-9);
    let mu_o = sim.get_mu_o_for_rs(cell.pressure_bar, derived.rs).max(1e-9);
    let mu_g = sim.get_mu_g(cell.pressure_bar).max(1e-9);

    let bo_derivatives = if saturated {
        [
            sim.get_d_bo_d_p_for_state(cell.pressure_bar, derived.rs, true),
            0.0,
            0.0,
        ]
    } else {
        [
            sim.get_d_bo_d_p_for_state(cell.pressure_bar, derived.rs, false),
            0.0,
            sim.get_d_bo_d_rs_for_state(cell.pressure_bar, derived.rs),
        ]
    };
    let bg_derivatives = [sim.get_d_bg_d_p_for_state(cell.pressure_bar), 0.0, 0.0];
    let rs_derivatives = if saturated {
        [sim.get_d_rs_sat_d_p_for_state(cell.pressure_bar), 0.0, 0.0]
    } else {
        [0.0, 0.0, 1.0]
    };

    let dmu_o_dp = sim.get_d_mu_o_d_p_for_state(cell.pressure_bar, derived.rs, saturated);
    let dmu_o_drs = if saturated {
        0.0
    } else {
        sim.get_d_mu_o_d_rs_for_state(cell.pressure_bar, derived.rs)
    };
    let dmu_g_dp = sim.get_d_mu_g_d_p_for_state(cell.pressure_bar);
    let dsg_dh = if saturated { 1.0 } else { 0.0 };

    let lambda_w = krw / mu_w;
    let lambda_o = kro / mu_o;
    let lambda_g = krg / mu_g;

    let dlam_w = [0.0, dkrw_dsw / mu_w, 0.0];
    let dlam_o = [
        -kro * dmu_o_dp / (mu_o * mu_o),
        dkro_dsw / mu_o,
        dkro_dsg * dsg_dh / mu_o - kro * dmu_o_drs / (mu_o * mu_o),
    ];
    let dlam_g = [
        -krg * dmu_g_dp / (mu_g * mu_g),
        0.0,
        dkrg_dsg * dsg_dh / mu_g,
    ];

    LocalPhaseSensitivity {
        mobilities: [lambda_w, lambda_o, lambda_g],
        mobility_derivatives: [dlam_w, dlam_o, dlam_g],
        bo: derived.bo.max(1e-9),
        bg: derived.bg.max(1e-9),
        rs: derived.rs.max(0.0),
        bo_derivatives,
        bg_derivatives,
        rs_derivatives,
    }
}

fn injector_connection_mobility(
    _sim: &ReservoirSimulator,
    local: &LocalPhaseSensitivity,
) -> (f64, [f64; 3]) {
    // Use total mobility for injectors. This ensures non-zero injectivity even
    // when the injected phase has zero saturation in the cell (e.g. gas injection
    // into an oil-saturated cell where krg=0). The injected component is tracked
    // separately via perforation_component_rates_sc_day.
    (
        local.mobilities[0] + local.mobilities[1] + local.mobilities[2],
        [
            local.mobility_derivatives[0][0]
                + local.mobility_derivatives[1][0]
                + local.mobility_derivatives[2][0],
            local.mobility_derivatives[0][1]
                + local.mobility_derivatives[1][1]
                + local.mobility_derivatives[2][1],
            local.mobility_derivatives[0][2]
                + local.mobility_derivatives[1][2]
                + local.mobility_derivatives[2][2],
        ],
    )
}

fn producer_rate_sensitivity(
    sim: &ReservoirSimulator,
    state: &FimState,
    perforation: &FimPerforation,
    influenced_cell_idx: usize,
) -> ProducerRateSensitivity {
    let control_cells = perforation_control_cells(sim, perforation);
    let mut lambda_w_sum = 0.0;
    let mut lambda_o_sum = 0.0;
    let mut lambda_g_sum = 0.0;
    let mut d_lambda_w = [0.0; 3];
    let mut d_lambda_o = [0.0; 3];
    let mut d_lambda_g = [0.0; 3];

    for cell_idx in control_cells {
        let local = local_phase_sensitivity(sim, state, cell_idx);
        lambda_w_sum += local.mobilities[0].max(0.0);
        lambda_o_sum += local.mobilities[1].max(0.0);
        lambda_g_sum += local.mobilities[2].max(0.0);
        if cell_idx == influenced_cell_idx {
            d_lambda_w = local.mobility_derivatives[0];
            d_lambda_o = local.mobility_derivatives[1];
            d_lambda_g = local.mobility_derivatives[2];
        }
    }

    let lambda_total = (lambda_w_sum + lambda_o_sum + lambda_g_sum).max(f64::EPSILON);
    let water_fraction = (lambda_w_sum / lambda_total).clamp(0.0, 1.0);
    let oil_fraction = (lambda_o_sum / lambda_total).clamp(0.0, 1.0);
    let gas_fraction = (lambda_g_sum / lambda_total).clamp(0.0, 1.0);

    let mut water_fraction_derivatives = [0.0; 3];
    let mut oil_fraction_derivatives = [0.0; 3];
    let mut gas_fraction_derivatives = [0.0; 3];
    for local_var in 0..3 {
        let d_total = d_lambda_w[local_var] + d_lambda_o[local_var] + d_lambda_g[local_var];
        water_fraction_derivatives[local_var] = (d_lambda_w[local_var] * lambda_total
            - lambda_w_sum * d_total)
            / (lambda_total * lambda_total);
        oil_fraction_derivatives[local_var] = (d_lambda_o[local_var] * lambda_total
            - lambda_o_sum * d_total)
            / (lambda_total * lambda_total);
        gas_fraction_derivatives[local_var] = (d_lambda_g[local_var] * lambda_total
            - lambda_g_sum * d_total)
            / (lambda_total * lambda_total);
    }

    let local_completion = local_phase_sensitivity(sim, state, perforation.cell_index);
    let (oil_fvf_derivatives, gas_fvf_derivatives, rs_derivatives) =
        if influenced_cell_idx == perforation.cell_index {
            (
                local_completion.bo_derivatives,
                local_completion.bg_derivatives,
                local_completion.rs_derivatives,
            )
        } else {
            ([0.0; 3], [0.0; 3], [0.0; 3])
        };

    ProducerRateSensitivity {
        water_fraction,
        oil_fraction,
        gas_fraction,
        oil_fvf: local_completion.bo,
        gas_fvf: local_completion.bg,
        rs_sm3_sm3: local_completion.rs,
        water_fraction_derivatives,
        oil_fraction_derivatives,
        gas_fraction_derivatives,
        oil_fvf_derivatives,
        gas_fvf_derivatives,
        rs_derivatives,
    }
}

pub(crate) fn connection_rate_for_bhp(
    sim: &ReservoirSimulator,
    state: &FimState,
    topology: &FimWellTopology,
    perf_idx: usize,
    bhp_bar: f64,
) -> Option<f64> {
    let perforation = &topology.perforations[perf_idx];
    let well = perforation_well(sim, perforation);
    let id = perforation.cell_index;
    let cell = state.cell(id);
    let derived = state.derive_cell(sim, id);
    let mobilities =
        sim.phase_mobilities_for_state(cell.sw, derived.sg, cell.pressure_bar, derived.rs);
    let wi_geom = geometric_well_index(sim, perforation)?;

    let connection_mobility = (mobilities.water + mobilities.oil + mobilities.gas).max(0.0);

    let raw_rate = wi_geom * connection_mobility * (cell.pressure_bar - bhp_bar);
    if !raw_rate.is_finite() {
        return None;
    }

    Some(if well.injector {
        raw_rate.min(0.0)
    } else {
        raw_rate.max(0.0)
    })
}

fn perforation_surface_rate_sc_day(
    sim: &ReservoirSimulator,
    state: &FimState,
    topology: &FimWellTopology,
    perf_idx: usize,
    q_m3_day: f64,
) -> Option<f64> {
    let perforation = &topology.perforations[perf_idx];
    let well = perforation_well(sim, perforation);
    let id = perforation.cell_index;

    if well.injector {
        return Some(match effective_injected_fluid(sim) {
            InjectedFluid::Water => (-q_m3_day).max(0.0) / sim.b_w.max(1e-9),
            InjectedFluid::Gas => (-q_m3_day).max(0.0) / state.derive_cell(sim, id).bg.max(1e-9),
        });
    }

    let producer = producer_control_state(sim, state, perforation);
    Some(q_m3_day.max(0.0) * producer.oil_fraction / producer.oil_fvf.max(1e-9))
}

pub(crate) fn perforation_component_rate_derivatives_sc_day(
    sim: &ReservoirSimulator,
    state: &FimState,
    topology: &FimWellTopology,
    perf_idx: usize,
) -> [f64; 3] {
    let perforation = &topology.perforations[perf_idx];
    let well = perforation_well(sim, perforation);
    let id = perforation.cell_index;

    if well.injector {
        return match effective_injected_fluid(sim) {
            InjectedFluid::Water => [1.0 / sim.b_w.max(1e-9), 0.0, 0.0],
            InjectedFluid::Gas => [0.0, 0.0, 1.0 / state.derive_cell(sim, id).bg.max(1e-9)],
        };
    }

    let producer = producer_control_state(sim, state, perforation);
    [
        producer.water_fraction / sim.b_w.max(1e-9),
        producer.oil_fraction / producer.oil_fvf.max(1e-9),
        producer.gas_fraction / producer.gas_fvf.max(1e-9)
            + producer.oil_fraction / producer.oil_fvf.max(1e-9) * producer.rs_sm3_sm3,
    ]
}

pub(crate) fn perforation_target_rate_derivative(
    sim: &ReservoirSimulator,
    state: &FimState,
    topology: &FimWellTopology,
    perf_idx: usize,
) -> f64 {
    let perforation = &topology.perforations[perf_idx];
    let control = physical_well_control(sim, topology, perforation.physical_well_index);
    let well = perforation_well(sim, perforation);
    let id = perforation.cell_index;

    if !control.enabled {
        return 0.0;
    }

    if well.injector {
        if control.uses_surface_target {
            return match effective_injected_fluid(sim) {
                InjectedFluid::Water => -1.0 / sim.b_w.max(1e-9),
                InjectedFluid::Gas => -1.0 / state.derive_cell(sim, id).bg.max(1e-9),
            };
        }
        return -1.0;
    }

    if control.uses_surface_target {
        let producer = producer_control_state(sim, state, perforation);
        return producer.oil_fraction / producer.oil_fvf.max(1e-9);
    }

    1.0
}

#[cfg(test)]
pub(crate) fn perforation_source_pressure_derivatives_sc_day(
    sim: &ReservoirSimulator,
    state: &FimState,
    topology: &FimWellTopology,
    perf_idx: usize,
) -> [f64; 3] {
    let perforation = &topology.perforations[perf_idx];
    let well = perforation_well(sim, perforation);
    if !well.injector {
        return [0.0, 0.0, 0.0];
    }

    match effective_injected_fluid(sim) {
        InjectedFluid::Water => [0.0, 0.0, 0.0],
        InjectedFluid::Gas => {
            let id = perforation.cell_index;
            let bg = state.derive_cell(sim, id).bg.max(1e-9);
            let dbg_dp = sim.get_d_bg_d_p_for_state(state.cell(id).pressure_bar);
            let q_m3_day = state.perforation_rates_m3_day[perf_idx];
            [0.0, 0.0, -q_m3_day * dbg_dp / (bg * bg)]
        }
    }
}

#[cfg(test)]
pub(crate) fn perforation_surface_rate_pressure_derivative(
    sim: &ReservoirSimulator,
    state: &FimState,
    topology: &FimWellTopology,
    perf_idx: usize,
) -> f64 {
    let perforation = &topology.perforations[perf_idx];
    let control = physical_well_control(sim, topology, perforation.physical_well_index);
    let well = perforation_well(sim, perforation);
    if !control.enabled || !control.uses_surface_target || !well.injector {
        return 0.0;
    }

    match effective_injected_fluid(sim) {
        InjectedFluid::Water => 0.0,
        InjectedFluid::Gas => {
            let id = perforation.cell_index;
            let bg = state.derive_cell(sim, id).bg.max(1e-9);
            let dbg_dp = sim.get_d_bg_d_p_for_state(state.cell(id).pressure_bar);
            let q_m3_day = state.perforation_rates_m3_day[perf_idx];
            q_m3_day * dbg_dp / (bg * bg)
        }
    }
}

pub(crate) fn perforation_connection_bhp_derivative(
    sim: &ReservoirSimulator,
    state: &FimState,
    topology: &FimWellTopology,
    perf_idx: usize,
    bhp_bar: f64,
) -> Option<f64> {
    let perforation = &topology.perforations[perf_idx];
    let well = perforation_well(sim, perforation);
    let id = perforation.cell_index;
    let cell = state.cell(id);
    let derived = state.derive_cell(sim, id);
    let mobilities =
        sim.phase_mobilities_for_state(cell.sw, derived.sg, cell.pressure_bar, derived.rs);
    let wi_geom = geometric_well_index(sim, perforation)?;

    let connection_mobility = (mobilities.water + mobilities.oil + mobilities.gas).max(0.0);

    let active_derivative = -wi_geom * connection_mobility;
    let raw_rate = wi_geom * connection_mobility * (cell.pressure_bar - bhp_bar);
    if !raw_rate.is_finite() {
        return None;
    }

    Some(if well.injector {
        if raw_rate < 0.0 {
            active_derivative
        } else {
            0.0
        }
    } else if raw_rate > 0.0 {
        active_derivative
    } else {
        0.0
    })
}

#[cfg(test)]
pub(crate) fn perforation_connection_pressure_derivative(
    sim: &ReservoirSimulator,
    state: &FimState,
    topology: &FimWellTopology,
    perf_idx: usize,
    bhp_bar: f64,
) -> Option<f64> {
    let perforation = &topology.perforations[perf_idx];
    let well = perforation_well(sim, perforation);
    if !well.injector {
        return None;
    }

    let id = perforation.cell_index;
    let cell = state.cell(id);
    let derived = state.derive_cell(sim, id);
    let mobilities =
        sim.phase_mobilities_for_state(cell.sw, derived.sg, cell.pressure_bar, derived.rs);
    let wi_geom = geometric_well_index(sim, perforation)?;
    let connection_mobility = (mobilities.water + mobilities.oil + mobilities.gas).max(0.0);
    let raw_rate = wi_geom * connection_mobility * (cell.pressure_bar - bhp_bar);
    if !raw_rate.is_finite() {
        return None;
    }

    Some(if raw_rate < 0.0 {
        wi_geom * connection_mobility
    } else {
        0.0
    })
}

pub(crate) fn perforation_connection_cell_derivatives(
    sim: &ReservoirSimulator,
    state: &FimState,
    topology: &FimWellTopology,
    perf_idx: usize,
    bhp_bar: f64,
) -> Option<[f64; 3]> {
    let perforation = &topology.perforations[perf_idx];
    let well = perforation_well(sim, perforation);
    let id = perforation.cell_index;
    let cell = state.cell(id);
    let wi_geom = geometric_well_index(sim, perforation)?;
    let drawdown = cell.pressure_bar - bhp_bar;
    let local = local_phase_sensitivity(sim, state, id);

    let (connection_mobility, dmob_dp, dmob_dsw, dmob_dh) = if well.injector {
        let (mobility, derivatives) = injector_connection_mobility(sim, &local);
        (mobility, derivatives[0], derivatives[1], derivatives[2])
    } else {
        (
            local.mobilities[0] + local.mobilities[1] + local.mobilities[2],
            local.mobility_derivatives[0][0]
                + local.mobility_derivatives[1][0]
                + local.mobility_derivatives[2][0],
            local.mobility_derivatives[0][1]
                + local.mobility_derivatives[1][1]
                + local.mobility_derivatives[2][1],
            local.mobility_derivatives[0][2]
                + local.mobility_derivatives[1][2]
                + local.mobility_derivatives[2][2],
        )
    };

    let raw_rate = wi_geom * connection_mobility * drawdown;
    if !raw_rate.is_finite() {
        return None;
    }

    let active = if well.injector {
        raw_rate < 0.0
    } else {
        raw_rate > 0.0
    };
    if !active {
        return Some([0.0, 0.0, 0.0]);
    }

    Some([
        wi_geom * (dmob_dp * drawdown + connection_mobility),
        wi_geom * dmob_dsw * drawdown,
        wi_geom * dmob_dh * drawdown,
    ])
}

pub(crate) fn perforation_component_rate_cell_derivatives_sc_day_by_var(
    sim: &ReservoirSimulator,
    state: &FimState,
    topology: &FimWellTopology,
    perf_idx: usize,
    cell_idx: usize,
) -> [[f64; 3]; 3] {
    let perforation = &topology.perforations[perf_idx];
    let well = perforation_well(sim, perforation);
    if well.injector {
        if cell_idx != perforation.cell_index {
            return [[0.0; 3]; 3];
        }
        return match effective_injected_fluid(sim) {
            InjectedFluid::Water => [[0.0; 3]; 3],
            InjectedFluid::Gas => {
                let q_m3_day = state.perforation_rates_m3_day[perf_idx];
                let local = local_phase_sensitivity(sim, state, cell_idx);
                [
                    [
                        0.0,
                        0.0,
                        -q_m3_day * local.bg_derivatives[0] / (local.bg * local.bg),
                    ],
                    [0.0, 0.0, 0.0],
                    [0.0, 0.0, 0.0],
                ]
            }
        };
    }

    let q_m3_day = state.perforation_rates_m3_day[perf_idx];
    let producer = producer_rate_sensitivity(sim, state, perforation, cell_idx);
    let bw = sim.b_w.max(1e-9);
    let bo = producer.oil_fvf.max(1e-9);
    let bg = producer.gas_fvf.max(1e-9);
    let mut derivatives = [[0.0; 3]; 3];
    for local_var in 0..3 {
        let d_oil_over_bo = producer.oil_fraction_derivatives[local_var] / bo
            - producer.oil_fraction * producer.oil_fvf_derivatives[local_var] / (bo * bo);
        derivatives[local_var][0] = q_m3_day * producer.water_fraction_derivatives[local_var] / bw;
        derivatives[local_var][1] = q_m3_day * d_oil_over_bo;
        derivatives[local_var][2] = q_m3_day
            * (producer.gas_fraction_derivatives[local_var] / bg
                - producer.gas_fraction * producer.gas_fvf_derivatives[local_var] / (bg * bg)
                + d_oil_over_bo * producer.rs_sm3_sm3
                + producer.oil_fraction / bo * producer.rs_derivatives[local_var]);
    }
    derivatives
}

pub(crate) fn perforation_surface_rate_cell_derivatives_sc_day(
    sim: &ReservoirSimulator,
    state: &FimState,
    topology: &FimWellTopology,
    perf_idx: usize,
    cell_idx: usize,
) -> [f64; 3] {
    let perforation = &topology.perforations[perf_idx];
    let control = physical_well_control(sim, topology, perforation.physical_well_index);
    let well = perforation_well(sim, perforation);
    if !control.enabled || !control.uses_surface_target {
        return [0.0; 3];
    }
    if well.injector {
        if cell_idx != perforation.cell_index {
            return [0.0; 3];
        }
        return match effective_injected_fluid(sim) {
            InjectedFluid::Water => [0.0; 3],
            InjectedFluid::Gas => {
                let q_m3_day = state.perforation_rates_m3_day[perf_idx];
                let local = local_phase_sensitivity(sim, state, cell_idx);
                [
                    q_m3_day * local.bg_derivatives[0] / (local.bg * local.bg),
                    0.0,
                    0.0,
                ]
            }
        };
    }

    let producer = producer_rate_sensitivity(sim, state, perforation, cell_idx);
    let bo = producer.oil_fvf.max(1e-9);
    let q_m3_day = state.perforation_rates_m3_day[perf_idx];
    let mut derivatives = [0.0; 3];
    for local_var in 0..3 {
        derivatives[local_var] = q_m3_day
            * (producer.oil_fraction_derivatives[local_var] / bo
                - producer.oil_fraction * producer.oil_fvf_derivatives[local_var] / (bo * bo));
    }
    derivatives
}

#[cfg(test)]
fn total_rate_for_well_bhp(
    sim: &ReservoirSimulator,
    state: &FimState,
    topology: &FimWellTopology,
    well_idx: usize,
    bhp_bar: f64,
) -> f64 {
    well_local_block(topology, state, well_idx).total_rate_for_bhp(sim, bhp_bar)
}

#[cfg(test)]
fn total_rate_from_unknowns(
    sim: &ReservoirSimulator,
    state: &FimState,
    topology: &FimWellTopology,
    well_idx: usize,
) -> f64 {
    well_local_block(topology, state, well_idx).total_rate_from_unknowns(sim)
}

#[cfg(test)]
pub(crate) fn solve_well_bhp_from_target(
    sim: &ReservoirSimulator,
    state: &FimState,
    topology: &FimWellTopology,
    well_idx: usize,
) -> Option<(f64, bool)> {
    well_local_block(topology, state, well_idx).solve_bhp_from_target(sim)
}

/// Regularized Fischer-Burmeister NCP function.
/// Adding ε² inside the sqrt makes the function C¹ at the origin,
/// removing the Jacobian discontinuity at the well control switching point
/// (where both BHP slack and rate slack approach zero simultaneously).
const FB_EPSILON: f64 = 1e-6;

fn fischer_burmeister(a: f64, b: f64) -> f64 {
    (a * a + b * b + 2.0 * FB_EPSILON * FB_EPSILON).sqrt() - a - b
}

pub(crate) fn fischer_burmeister_gradient(a: f64, b: f64) -> (f64, f64) {
    let norm = (a * a + b * b + 2.0 * FB_EPSILON * FB_EPSILON).sqrt();
    (a / norm - 1.0, b / norm - 1.0)
}

#[cfg(test)]
pub(crate) fn well_control_slacks(
    sim: &ReservoirSimulator,
    state: &FimState,
    topology: &FimWellTopology,
    well_idx: usize,
) -> Option<(f64, f64)> {
    well_local_block(topology, state, well_idx).control_slacks(sim)
}

#[cfg(test)]
pub(crate) fn well_constraint_residual(
    sim: &ReservoirSimulator,
    state: &FimState,
    topology: &FimWellTopology,
    well_idx: usize,
) -> Option<f64> {
    well_local_block(topology, state, well_idx).constraint_residual(sim)
}

#[cfg(test)]
pub(crate) fn perforation_rate_residual(
    sim: &ReservoirSimulator,
    state: &FimState,
    topology: &FimWellTopology,
    perf_idx: usize,
) -> Option<f64> {
    perforation_local_block(topology, state, perf_idx).rate_residual(sim)
}

#[cfg(test)]
pub(crate) fn perforation_residual_diagnostics(
    sim: &ReservoirSimulator,
    state: &FimState,
    topology: &FimWellTopology,
    perf_idx: usize,
) -> Option<PerforationResidualDiagnostics> {
    perforation_local_block(topology, state, perf_idx).residual_diagnostics(sim)
}

pub(crate) fn perforation_component_rates_sc_day(
    sim: &ReservoirSimulator,
    state: &FimState,
    topology: &FimWellTopology,
    perf_idx: usize,
) -> [f64; 3] {
    let perforation = &topology.perforations[perf_idx];
    let well = perforation_well(sim, perforation);
    let q_m3_day = state.perforation_rates_m3_day[perf_idx];
    let id = perforation.cell_index;
    if well.injector {
        return match effective_injected_fluid(sim) {
            InjectedFluid::Water => [q_m3_day / sim.b_w.max(1e-9), 0.0, 0.0],
            InjectedFluid::Gas => [0.0, 0.0, q_m3_day / state.derive_cell(sim, id).bg.max(1e-9)],
        };
    }

    let producer = producer_control_state(sim, state, perforation);
    let water_sc_day = q_m3_day * producer.water_fraction / sim.b_w.max(1e-9);
    let oil_sc_day = q_m3_day * producer.oil_fraction / producer.oil_fvf.max(1e-9);
    let free_gas_sc_day = q_m3_day * producer.gas_fraction / producer.gas_fvf.max(1e-9);
    let dissolved_gas_sc_day = oil_sc_day * producer.rs_sm3_sm3;
    [
        water_sc_day,
        oil_sc_day,
        free_gas_sc_day + dissolved_gas_sc_day,
    ]
}

#[cfg(test)]
mod tests {
    use nalgebra::DVector;

    use super::*;

    #[test]
    fn topology_maps_coordinates_to_correct_cells() {
        let mut sim = ReservoirSimulator::new(3, 2, 2, 0.2);
        sim.add_well(0, 0, 0, 400.0, 0.1, 0.0, true).unwrap();
        sim.add_well(2, 1, 1, 50.0, 0.1, 0.0, false).unwrap();

        let topology = build_well_topology(&sim);

        assert_eq!(topology.perforations.len(), 2);
        assert_eq!(topology.perforations[0].cell_index, sim.idx(0, 0, 0));
        assert_eq!(topology.perforations[1].cell_index, sim.idx(2, 1, 1));
        assert!(topology.perforations[0].injector);
        assert!(!topology.perforations[1].injector);
        assert_eq!(topology.wells.len(), 2);
    }

    #[test]
    fn topology_groups_vertical_completions_into_one_physical_well() {
        let mut sim = ReservoirSimulator::new(1, 1, 3, 0.2);
        sim.add_well(0, 0, 0, 100.0, 0.1, 0.0, false).unwrap();
        sim.add_well(0, 0, 2, 100.0, 0.1, 0.0, false).unwrap();

        let topology = build_well_topology(&sim);

        assert_eq!(topology.perforations.len(), 2);
        assert_eq!(topology.wells.len(), 1);
        assert_ne!(
            topology.perforations[0].cell_index,
            topology.perforations[1].cell_index
        );
        assert_eq!(topology.perforations[0].k, 0);
        assert_eq!(topology.perforations[1].k, 2);
        assert_eq!(topology.perforations[0].physical_well_index, 0);
        assert_eq!(topology.perforations[1].physical_well_index, 0);
    }

    #[test]
    fn topology_prefers_explicit_physical_well_ids_over_legacy_fingerprint() {
        let mut sim = ReservoirSimulator::new(1, 1, 2, 0.2);
        sim.add_well_with_id(0, 0, 0, 120.0, 0.1, 0.0, false, "prod-a".to_string())
            .unwrap();
        sim.add_well_with_id(0, 0, 1, 150.0, 0.1, 0.0, false, "prod-a".to_string())
            .unwrap();

        let topology = build_well_topology(&sim);

        assert_eq!(topology.wells.len(), 1);
        assert_eq!(topology.perforations.len(), 2);
        assert_eq!(topology.perforations[0].physical_well_index, 0);
        assert_eq!(topology.perforations[1].physical_well_index, 0);
    }

    #[test]
    fn local_block_exposes_bhp_and_perforation_offsets() {
        let mut sim = ReservoirSimulator::new(1, 1, 2, 0.2);
        sim.add_well_with_id(0, 0, 0, 120.0, 0.1, 0.0, false, "prod-a".to_string())
            .unwrap();
        sim.add_well_with_id(0, 0, 1, 120.0, 0.1, 0.0, false, "prod-a".to_string())
            .unwrap();

        let topology = build_well_topology(&sim);
        let state = FimState::from_simulator(&sim);
        let block = well_local_block(&topology, &state, 0);
        let perforations = block.perforations().collect::<Vec<_>>();

        assert_eq!(block.well_idx(), 0);
        assert_eq!(block.bhp_unknown_offset(), state.well_bhp_unknown_offset(0));
        assert_eq!(block.equation_offset(), state.well_equation_offset(0));
        assert_eq!(perforations.len(), 2);
        assert_eq!(perforations[0].rate_unknown_offset(), state.perforation_rate_unknown_offset(0));
        assert_eq!(perforations[0].equation_offset(), state.perforation_equation_offset(0));
        assert_eq!(perforations[1].rate_unknown_offset(), state.perforation_rate_unknown_offset(1));
        assert_eq!(perforations[1].equation_offset(), state.perforation_equation_offset(1));
    }

    #[test]
    fn local_block_perforation_control_cells_match_existing_control_stencil() {
        let mut sim = ReservoirSimulator::new(3, 3, 1, 0.2);
        sim.add_well(1, 1, 0, 100.0, 0.1, 0.0, false).unwrap();

        let topology = build_well_topology(&sim);
        let state = FimState::from_simulator(&sim);
        let block = well_local_block(&topology, &state, 0);
        let perf = block.perforations().next().expect("expected perforation");
        let mut cells = perf.control_influence_cells(&sim);
        cells.sort_unstable();

        let mut expected = vec![
            sim.idx(0, 0, 0),
            sim.idx(1, 0, 0),
            sim.idx(2, 0, 0),
            sim.idx(0, 1, 0),
            sim.idx(1, 1, 0),
            sim.idx(2, 1, 0),
            sim.idx(0, 2, 0),
            sim.idx(1, 2, 0),
            sim.idx(2, 2, 0),
        ];
        expected.sort_unstable();

        assert_eq!(cells, expected);
    }

    #[test]
    fn local_block_derivative_helpers_match_free_functions() {
        let mut sim = ReservoirSimulator::new(2, 1, 1, 0.2);
        sim.set_rate_controlled_wells(true);
        sim.set_injected_fluid("water").unwrap();
        sim.add_well(0, 0, 0, 400.0, 0.1, 0.0, true).unwrap();
        sim.add_well(1, 0, 0, 50.0, 0.1, 0.0, false).unwrap();

        let topology = build_well_topology(&sim);
        let state = FimState::from_simulator(&sim);
        let perf = perforation_local_block(&topology, &state, 0);
        let bhp = well_local_block(&topology, &state, perf.physical_well_idx()).bhp_bar();

        assert_eq!(
            perf.component_rate_derivatives_sc_day(&sim),
            perforation_component_rate_derivatives_sc_day(&sim, &state, &topology, 0)
        );
        assert_eq!(
            perf.target_rate_derivative(&sim),
            perforation_target_rate_derivative(&sim, &state, &topology, 0)
        );
        assert_eq!(
            perf.connection_bhp_derivative(&sim, bhp),
            perforation_connection_bhp_derivative(&sim, &state, &topology, 0, bhp)
        );
        assert_eq!(
            perf.connection_cell_derivatives(&sim, bhp),
            perforation_connection_cell_derivatives(&sim, &state, &topology, 0, bhp)
        );
    }

    #[test]
    fn local_well_block_control_helpers_match_free_functions() {
        let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        sim.set_rate_controlled_wells(true);
        sim.set_target_well_rates(0.0, 10.0).unwrap();
        sim.set_well_bhp_limits(50.0, 400.0).unwrap();
        sim.add_well(0, 0, 0, 100.0, 0.1, 0.0, false).unwrap();

        let topology = build_well_topology(&sim);
        let state = FimState::from_simulator(&sim);
        let block = well_local_block(&topology, &state, 0);

        assert_eq!(block.control(&sim), physical_well_control(&sim, &topology, 0));
        assert_eq!(block.control_slacks(&sim), well_control_slacks(&sim, &state, &topology, 0));
        assert_eq!(
            block.constraint_residual(&sim),
            well_constraint_residual(&sim, &state, &topology, 0)
        );
    }

    #[test]
    fn local_block_residual_helpers_match_free_functions() {
        let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        sim.set_rate_controlled_wells(true);
        sim.set_target_well_rates(0.0, 10.0).unwrap();
        sim.set_well_bhp_limits(50.0, 400.0).unwrap();
        sim.add_well(0, 0, 0, 100.0, 0.1, 0.0, false).unwrap();

        let topology = build_well_topology(&sim);
        let state = FimState::from_simulator(&sim);
        let perf = perforation_local_block(&topology, &state, 0);

        assert_eq!(perf.rate_residual(&sim), perforation_rate_residual(&sim, &state, &topology, 0));
        assert_eq!(
            perf.residual_diagnostics(&sim),
            perforation_residual_diagnostics(&sim, &state, &topology, 0)
        );
    }

    #[test]
    fn local_well_block_rate_helpers_match_free_functions() {
        let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        sim.set_rate_controlled_wells(true);
        sim.set_target_well_rates(0.0, 10.0).unwrap();
        sim.set_well_bhp_limits(50.0, 400.0).unwrap();
        sim.add_well(0, 0, 0, 100.0, 0.1, 0.0, false).unwrap();

        let topology = build_well_topology(&sim);
        let state = FimState::from_simulator(&sim);
        let block = well_local_block(&topology, &state, 0);

        assert_eq!(block.total_rate_from_unknowns(&sim), total_rate_from_unknowns(&sim, &state, &topology, 0));
        assert_eq!(block.total_rate_for_bhp(&sim, state.well_bhp[0]), total_rate_for_well_bhp(&sim, &state, &topology, 0, state.well_bhp[0]));
        assert_eq!(block.solve_bhp_from_target(&sim), solve_well_bhp_from_target(&sim, &state, &topology, 0));
    }

    #[test]
    fn water_injector_bhp_rate_uses_iterate_water_mobility() {
        let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        sim.set_injected_fluid("water").unwrap();
        sim.add_well(0, 0, 0, 1000.0, 0.1, 0.0, true).unwrap();
        sim.injector_rate_controlled = false;

        let low_sw = FimState::from_simulator(&sim);
        let mut high_sw = low_sw.clone();
        high_sw.cells[0].sw = 0.8;
        let topology = build_well_topology(&sim);

        let q_low = connection_rate_for_bhp(&sim, &low_sw, &topology, 0, 1000.0).unwrap();
        let q_high = connection_rate_for_bhp(&sim, &high_sw, &topology, 0, 1000.0).unwrap();

        assert!(q_high < q_low);
    }

    #[test]
    fn producer_rate_control_state_uses_iterate_phase_fractions() {
        let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        sim.add_well(0, 0, 0, 0.0, 0.1, 0.0, false).unwrap();
        let base = FimState::from_simulator(&sim);
        let mut wet = base.clone();
        wet.cells[0].sw = 0.8;
        let topology = build_well_topology(&sim);
        let perforation = topology.perforations[0];

        let base_state = producer_control_state(&sim, &base, &perforation);
        let wet_state = producer_control_state(&sim, &wet, &perforation);

        assert!(wet_state.water_fraction > base_state.water_fraction);
        assert!(wet_state.oil_fraction < base_state.oil_fraction);
    }

    #[test]
    fn rate_controlled_wells_keep_independent_bhp_equations() {
        let mut sim = ReservoirSimulator::new(2, 1, 2, 0.2);
        sim.add_well_with_id(0, 0, 0, 50.0, 0.1, 0.0, false, "prod-a".to_string())
            .unwrap();
        sim.add_well_with_id(1, 0, 0, 50.0, 0.1, 0.0, false, "prod-b".to_string())
            .unwrap();
        sim.set_well_schedule(
            "prod-a".to_string(),
            "rate".to_string(),
            10.0,
            f64::NAN,
            40.0,
            true,
        )
        .unwrap();
        sim.set_well_schedule(
            "prod-b".to_string(),
            "pressure".to_string(),
            f64::NAN,
            f64::NAN,
            35.0,
            true,
        )
        .unwrap();

        let topology = build_well_topology(&sim);
        let mut state = FimState::from_simulator(&sim);
        state.well_bhp[1] += 5.0;

        let residual = well_constraint_residual(&sim, &state, &topology, 1).unwrap();

        assert!((residual - (state.well_bhp[1] - 50.0)).abs() < 1e-9);
    }

    #[test]
    fn mixed_schedule_controls_do_not_share_rate_target() {
        let mut sim = ReservoirSimulator::new(2, 1, 1, 0.2);
        sim.add_well_with_id(0, 0, 0, 100.0, 0.1, 0.0, false, "prod-a".to_string())
            .unwrap();
        sim.add_well_with_id(1, 0, 0, 100.0, 0.1, 0.0, false, "prod-b".to_string())
            .unwrap();
        sim.set_well_schedule(
            "prod-a".to_string(),
            "rate".to_string(),
            25.0,
            f64::NAN,
            60.0,
            true,
        )
        .unwrap();
        sim.set_well_schedule(
            "prod-b".to_string(),
            "rate".to_string(),
            5.0,
            f64::NAN,
            60.0,
            true,
        )
        .unwrap();

        let topology = build_well_topology(&sim);
        let state = FimState::from_simulator(&sim);
        let slacks_a = well_control_slacks(&sim, &state, &topology, 0).unwrap();
        let slacks_b = well_control_slacks(&sim, &state, &topology, 1).unwrap();

        assert!(slacks_a.1.abs() < 1e-6);
        assert!(slacks_b.1.abs() < 1e-6);
        assert!((state.well_bhp[0] - state.well_bhp[1]).abs() > 1e-6);
    }

    #[test]
    fn feasible_rate_control_state_satisfies_complementarity_with_positive_bhp_slack() {
        let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        sim.set_rate_controlled_wells(true);
        sim.set_target_well_rates(0.0, 10.0).unwrap();
        sim.set_well_bhp_limits(50.0, 400.0).unwrap();
        sim.add_well(0, 0, 0, 100.0, 0.1, 0.0, false).unwrap();

        let topology = build_well_topology(&sim);
        let state = FimState::from_simulator(&sim);

        let residual = well_constraint_residual(&sim, &state, &topology, 0).unwrap();
        let (bhp_slack, rate_slack) = well_control_slacks(&sim, &state, &topology, 0).unwrap();

        assert!(residual.abs() < 1e-6);
        assert!(bhp_slack > 1e-6);
        assert!(rate_slack.abs() < 1e-6);
    }

    #[test]
    fn infeasible_rate_target_satisfies_complementarity_at_bhp_limit() {
        let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        sim.set_rate_controlled_wells(true);
        sim.set_target_well_rates(0.0, 1.0e6).unwrap();
        sim.set_well_bhp_limits(80.0, 400.0).unwrap();
        sim.add_well(0, 0, 0, 100.0, 0.1, 0.0, false).unwrap();

        let topology = build_well_topology(&sim);
        let state = FimState::from_simulator(&sim);

        let residual = well_constraint_residual(&sim, &state, &topology, 0).unwrap();
        let (bhp_slack, rate_slack) = well_control_slacks(&sim, &state, &topology, 0).unwrap();

        assert!(residual.abs() < 1e-6);
        assert!(bhp_slack.abs() < 1e-6);
        assert!(rate_slack > 1e-6);
    }

    #[test]
    fn water_injector_connection_pressure_derivative_matches_local_fd() {
        let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        sim.set_injected_fluid("water").unwrap();
        sim.add_well(0, 0, 0, 250.0, 0.1, 0.0, true).unwrap();
        sim.injector_rate_controlled = false;

        let state = FimState::from_simulator(&sim);
        let topology = build_well_topology(&sim);
        let bhp = state.well_bhp[0];
        let exact =
            perforation_connection_pressure_derivative(&sim, &state, &topology, 0, bhp).unwrap();

        let mut update = DVector::zeros(state.n_unknowns());
        let step = 1e-5 * state.cells[0].pressure_bar.abs().max(1.0);
        update[0] = step;
        let perturbed = state.apply_newton_update(&sim, &update, 1.0);
        let base = connection_rate_for_bhp(&sim, &state, &topology, 0, bhp).unwrap();
        let shifted = connection_rate_for_bhp(&sim, &perturbed, &topology, 0, bhp).unwrap();
        let fd = (shifted - base) / step;

        assert!((exact - fd).abs() < 1e-6);
    }

    #[test]
    fn gas_injector_surface_pressure_derivatives_match_local_fd() {
        let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        sim.set_injected_fluid("gas").unwrap();
        sim.add_well(0, 0, 0, 400.0, 0.1, 0.0, true).unwrap();
        sim.injector_enabled = true;
        sim.injector_rate_controlled = true;
        sim.target_injector_surface_rate_m3_day = Some(200.0);

        let mut state = FimState::from_simulator(&sim);
        state.perforation_rates_m3_day[0] = -120.0;
        let topology = build_well_topology(&sim);

        let source_exact =
            perforation_source_pressure_derivatives_sc_day(&sim, &state, &topology, 0)[2];
        let target_exact = perforation_surface_rate_pressure_derivative(&sim, &state, &topology, 0);

        let mut update = DVector::zeros(state.n_unknowns());
        let step = 1e-5 * state.cells[0].pressure_bar.abs().max(1.0);
        update[0] = step;
        let perturbed = state.apply_newton_update(&sim, &update, 1.0);

        let base_source = perforation_component_rates_sc_day(&sim, &state, &topology, 0)[2];
        let shifted_source = perforation_component_rates_sc_day(&sim, &perturbed, &topology, 0)[2];
        let source_fd = (shifted_source - base_source) / step;

        let base_target = total_rate_from_unknowns(&sim, &state, &topology, 0);
        let shifted_target = total_rate_from_unknowns(&sim, &perturbed, &topology, 0);
        let target_fd = (shifted_target - base_target) / step;

        let source_scale = source_exact.abs().max(source_fd.abs()).max(1e-9);
        let target_scale = target_exact.abs().max(target_fd.abs()).max(1e-9);

        assert!((source_exact - source_fd).abs() / source_scale < 1e-3);
        assert!((target_exact - target_fd).abs() / target_scale < 1e-3);
    }

    #[test]
    fn producer_perforation_connection_cell_derivatives_match_local_fd() {
        let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        sim.set_three_phase_rel_perm_props(
            0.12, 0.12, 0.04, 0.04, 0.18, 2.0, 2.5, 1.5, 1e-5, 1.0, 0.984,
        )
        .unwrap();
        sim.set_three_phase_mode_enabled(true);
        sim.add_well(0, 0, 0, 100.0, 0.1, 0.0, false).unwrap();

        let mut state = FimState::from_simulator(&sim);
        state.cells[0].pressure_bar = 220.0;
        state.cells[0].sw = 0.25;
        state.cells[0].regime = HydrocarbonState::Saturated;
        state.cells[0].hydrocarbon_var = 0.15;

        let topology = build_well_topology(&sim);
        let bhp = state.well_bhp[0];
        let exact =
            perforation_connection_cell_derivatives(&sim, &state, &topology, 0, bhp).unwrap();
        let base = connection_rate_for_bhp(&sim, &state, &topology, 0, bhp).unwrap();

        for local_var in 0..3 {
            let mut update = DVector::zeros(state.n_unknowns());
            let step = match local_var {
                0 => 1e-5 * state.cells[0].pressure_bar.abs().max(1.0),
                1 => 1e-7,
                2 => 1e-7 * state.cells[0].hydrocarbon_var.abs().max(1.0),
                _ => unreachable!(),
            };
            update[local_var] = step;
            let perturbed = state.apply_newton_update(&sim, &update, 1.0);
            let shifted = connection_rate_for_bhp(&sim, &perturbed, &topology, 0, bhp).unwrap();
            let fd = (shifted - base) / step;
            let scale = exact[local_var].abs().max(fd.abs()).max(1e-9);
            assert!((exact[local_var] - fd).abs() / scale < 5e-4);
        }
    }
}
