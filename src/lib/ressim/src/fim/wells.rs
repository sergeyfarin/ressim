use std::collections::HashMap;

use crate::fim::state::FimState;
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct WellGroupingKey {
    injector: bool,
    i: usize,
    j: usize,
    bhp_bits: u64,
    radius_bits: u64,
    skin_bits: u64,
}

pub(crate) fn build_well_topology(sim: &ReservoirSimulator) -> FimWellTopology {
    let mut wells = Vec::new();
    let mut perforations = Vec::with_capacity(sim.wells.len());
    let mut groups = HashMap::<WellGroupingKey, usize>::new();

    for (well_entry_index, well) in sim.wells.iter().enumerate() {
        let key = WellGroupingKey {
            injector: well.injector,
            i: well.i,
            j: well.j,
            bhp_bits: well.bhp.to_bits(),
            radius_bits: well.well_radius.to_bits(),
            skin_bits: well.skin.to_bits(),
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

    FimWellTopology { wells, perforations }
}

fn perforation_well<'a>(sim: &'a ReservoirSimulator, perforation: &FimPerforation) -> &'a Well {
    &sim.wells[perforation.well_entry_index]
}

fn physical_well<'a>(sim: &'a ReservoirSimulator, topology: &FimWellTopology, well_idx: usize) -> &'a Well {
    &sim.wells[topology.wells[well_idx].representative_well_index]
}

pub(crate) fn physical_well_bhp_target(
    sim: &ReservoirSimulator,
    topology: &FimWellTopology,
    well_idx: usize,
) -> f64 {
    physical_well(sim, topology, well_idx).bhp
}

fn family_anchor_well(topology: &FimWellTopology, injector: bool) -> Option<usize> {
    topology.wells.iter().position(|well| well.injector == injector)
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

    let connection_mobility = if well.injector {
        match sim.injected_fluid {
            InjectedFluid::Water => mobilities.water,
            InjectedFluid::Gas => mobilities.gas,
        }
    } else {
        mobilities.water + mobilities.oil + mobilities.gas
    }
    .max(0.0);

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
        return Some(match sim.injected_fluid {
            InjectedFluid::Water => (-q_m3_day).max(0.0) / sim.b_w.max(1e-9),
            InjectedFluid::Gas => (-q_m3_day).max(0.0) / state.derive_cell(sim, id).bg.max(1e-9),
        });
    }

    let producer = producer_control_state(sim, state, perforation);
    Some(q_m3_day.max(0.0) * producer.oil_fraction / producer.oil_fvf.max(1e-9))
}

fn total_rate_for_family_bhp(
    sim: &ReservoirSimulator,
    state: &FimState,
    topology: &FimWellTopology,
    injector: bool,
    bhp_bar: f64,
) -> f64 {
    let surface_target = family_uses_surface_target(sim, injector);
    topology
        .perforations
        .iter()
        .enumerate()
        .filter(|(_, perforation)| perforation.injector == injector)
        .filter_map(|(perf_idx, _)| {
            let q_m3_day = connection_rate_for_bhp(sim, state, topology, perf_idx, bhp_bar)?;
            if injector {
                if surface_target {
                    perforation_surface_rate_sc_day(sim, state, topology, perf_idx, q_m3_day)
                } else {
                    Some((-q_m3_day).max(0.0))
                }
            } else if surface_target {
                perforation_surface_rate_sc_day(sim, state, topology, perf_idx, q_m3_day)
            } else {
                Some(q_m3_day.max(0.0))
            }
        })
        .sum()
}

fn total_rate_from_unknowns(
    sim: &ReservoirSimulator,
    state: &FimState,
    topology: &FimWellTopology,
    injector: bool,
) -> f64 {
    let surface_target = family_uses_surface_target(sim, injector);
    topology
        .perforations
        .iter()
        .enumerate()
        .filter(|(_, perforation)| perforation.injector == injector)
        .filter_map(|(perf_idx, _)| {
            let q_m3_day = state.perforation_rates_m3_day[perf_idx];
            if injector {
                if surface_target {
                    perforation_surface_rate_sc_day(sim, state, topology, perf_idx, q_m3_day)
                } else {
                    Some((-q_m3_day).max(0.0))
                }
            } else if surface_target {
                perforation_surface_rate_sc_day(sim, state, topology, perf_idx, q_m3_day)
            } else {
                Some(q_m3_day.max(0.0))
            }
        })
        .sum()
}

pub(crate) fn well_constraint_residual(
    sim: &ReservoirSimulator,
    state: &FimState,
    topology: &FimWellTopology,
    well_idx: usize,
) -> Option<f64> {
    let well = &topology.wells[well_idx];
    let bhp_bar = state.well_bhp[well_idx];

    if well.injector && !sim.injector_enabled {
        let anchor_well_idx = family_anchor_well(topology, true)?;
        return Some(if well_idx == anchor_well_idx {
            bhp_bar - physical_well_bhp_target(sim, topology, well_idx)
        } else {
            bhp_bar - state.well_bhp[anchor_well_idx]
        });
    }

    if !family_rate_controlled(sim, well.injector) {
        return Some(bhp_bar - physical_well_bhp_target(sim, topology, well_idx));
    }

    let anchor_well_idx = family_anchor_well(topology, well.injector)?;
    if well_idx != anchor_well_idx {
        return Some(bhp_bar - state.well_bhp[anchor_well_idx]);
    }

    let target_rate = family_target_rate(sim, well.injector)?;
    let bhp_limit = family_bhp_limit(sim, well.injector);
    let max_achievable_rate = total_rate_for_family_bhp(sim, state, topology, well.injector, bhp_limit);
    if target_rate >= max_achievable_rate - 1e-9 {
        Some(bhp_bar - bhp_limit)
    } else {
        Some(total_rate_from_unknowns(sim, state, topology, well.injector) - target_rate)
    }
}

pub(crate) fn perforation_rate_residual(
    sim: &ReservoirSimulator,
    state: &FimState,
    topology: &FimWellTopology,
    perf_idx: usize,
) -> Option<f64> {
    let perforation = &topology.perforations[perf_idx];
    if perforation.injector && !sim.injector_enabled {
        return Some(state.perforation_rates_m3_day[perf_idx]);
    }

    let bhp_bar = state.well_bhp[perforation.physical_well_index];
    let q_connection = connection_rate_for_bhp(sim, state, topology, perf_idx, bhp_bar)?;
    Some(state.perforation_rates_m3_day[perf_idx] - q_connection)
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
        return match sim.injected_fluid {
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
        assert_ne!(topology.perforations[0].cell_index, topology.perforations[1].cell_index);
        assert_eq!(topology.perforations[0].k, 0);
        assert_eq!(topology.perforations[1].k, 2);
        assert_eq!(topology.perforations[0].physical_well_index, 0);
        assert_eq!(topology.perforations[1].physical_well_index, 0);
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
    fn non_anchor_rate_controlled_wells_share_family_bhp_equation() {
        let mut sim = ReservoirSimulator::new(2, 1, 2, 0.2);
        sim.set_rate_controlled_wells(true);
        sim.add_well(0, 0, 0, 50.0, 0.1, 0.0, false).unwrap();
        sim.add_well(1, 0, 0, 50.0, 0.1, 0.0, false).unwrap();

        let topology = build_well_topology(&sim);
        let mut state = FimState::from_simulator(&sim);
        state.well_bhp[1] += 5.0;

        let residual = well_constraint_residual(&sim, &state, &topology, 1).unwrap();

        assert!((residual - 5.0).abs() < 1e-9);
    }
}
