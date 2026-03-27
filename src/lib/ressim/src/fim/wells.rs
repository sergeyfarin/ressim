use crate::fim::state::FimState;
use crate::well_control::{ProducerControlState, ResolvedWellControl, WellControlDecision};
use crate::{InjectedFluid, ReservoirSimulator, Well};

const DARCY_METRIC_FACTOR: f64 = 8.526_988_8e-3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FimControlGroup {
    Injector,
    Producer,
}

fn geometric_well_index(sim: &ReservoirSimulator, well: &Well) -> Option<f64> {
    let id = sim.idx(well.i, well.j, well.k);
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
    well: &Well,
) -> ProducerControlState {
    let i_min = well.i.saturating_sub(1);
    let i_max = (well.i + 1).min(sim.nx.saturating_sub(1));
    let j_min = well.j.saturating_sub(1);
    let j_max = (well.j + 1).min(sim.ny.saturating_sub(1));
    let k = well.k;

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
    let id = sim.idx(well.i, well.j, well.k);
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

fn completion_rate_for_bhp(
    sim: &ReservoirSimulator,
    state: &FimState,
    well: &Well,
    bhp_bar: f64,
) -> Option<f64> {
    let id = sim.idx(well.i, well.j, well.k);
    let cell = state.cell(id);
    let derived = state.derive_cell(sim, id);
    let mobilities =
        sim.phase_mobilities_for_state(cell.sw, derived.sg, cell.pressure_bar, derived.rs);
    let wi_geom = geometric_well_index(sim, well)?;

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

fn completion_surface_rate_sc_day(
    sim: &ReservoirSimulator,
    state: &FimState,
    well: &Well,
    bhp_bar: f64,
) -> Option<f64> {
    let id = sim.idx(well.i, well.j, well.k);
    let q_m3_day = completion_rate_for_bhp(sim, state, well, bhp_bar)?;

    if well.injector {
        return Some(match sim.injected_fluid {
            InjectedFluid::Water => (-q_m3_day).max(0.0) / sim.b_w.max(1e-9),
            InjectedFluid::Gas => (-q_m3_day).max(0.0) / state.derive_cell(sim, id).bg.max(1e-9),
        });
    }

    let producer = producer_control_state(sim, state, well);
    Some(q_m3_day.max(0.0) * producer.oil_fraction / producer.oil_fvf.max(1e-9))
}

pub(crate) fn solve_group_bhp(
    sim: &ReservoirSimulator,
    state: &FimState,
    injector: bool,
) -> Option<(f64, bool)> {
    let use_rate_control = if injector {
        sim.injector_rate_controlled
    } else {
        sim.producer_rate_controlled
    };
    if !use_rate_control {
        return None;
    }

    let wells: Vec<&Well> = sim
        .wells
        .iter()
        .filter(|well| well.injector == injector)
        .collect();
    if wells.is_empty() {
        return None;
    }

    let total_surface_target = if injector {
        sim.target_injector_surface_rate_m3_day
    } else {
        sim.target_producer_surface_rate_m3_day
    };
    let total_reservoir_target = if injector {
        sim.target_injector_rate_m3_day
    } else {
        sim.target_producer_rate_m3_day
    };

    let total_rate_for_bhp = |bhp_bar: f64| -> f64 {
        wells
            .iter()
            .filter_map(|well| {
                if injector {
                    match total_surface_target {
                        Some(_) => completion_surface_rate_sc_day(sim, state, well, bhp_bar),
                        None => completion_rate_for_bhp(sim, state, well, bhp_bar)
                            .map(|q| (-q).max(0.0)),
                    }
                } else {
                    match total_surface_target {
                        Some(_) => completion_surface_rate_sc_day(sim, state, well, bhp_bar),
                        None => completion_rate_for_bhp(sim, state, well, bhp_bar),
                    }
                }
            })
            .sum()
    };

    let target_rate = total_surface_target
        .unwrap_or(total_reservoir_target)
        .max(0.0);
    let group_min_pressure = wells
        .iter()
        .map(|well| state.cell(sim.idx(well.i, well.j, well.k)).pressure_bar)
        .fold(f64::INFINITY, f64::min);
    let group_max_pressure = wells
        .iter()
        .map(|well| state.cell(sim.idx(well.i, well.j, well.k)).pressure_bar)
        .fold(f64::NEG_INFINITY, f64::max);

    if !group_min_pressure.is_finite() || !group_max_pressure.is_finite() {
        return None;
    }

    if injector {
        let bhp_limit = sim.well_bhp_max;
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
        let bhp_limit = sim.well_bhp_min;
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

fn control_group_bhp(state: &FimState, group: FimControlGroup) -> Option<f64> {
    match group {
        FimControlGroup::Injector => state.injector_group_bhp(),
        FimControlGroup::Producer => state.producer_group_bhp(),
    }
}

pub(crate) fn control_group_equation_offset(
    state: &FimState,
    group: FimControlGroup,
) -> Option<usize> {
    match group {
        FimControlGroup::Injector => state.injector_group_unknown_offset(),
        FimControlGroup::Producer => state.producer_group_unknown_offset(),
    }
}

fn group_target_rate(sim: &ReservoirSimulator, group: FimControlGroup) -> Option<f64> {
    match group {
        FimControlGroup::Injector => {
            if !sim.injector_rate_controlled
                || !sim.injector_enabled
                || !sim.wells.iter().any(|well| well.injector)
            {
                return None;
            }
            Some(
                sim.target_injector_surface_rate_m3_day
                    .unwrap_or(sim.target_injector_rate_m3_day)
                    .max(0.0),
            )
        }
        FimControlGroup::Producer => {
            if !sim.producer_rate_controlled || !sim.wells.iter().any(|well| !well.injector) {
                return None;
            }
            Some(
                sim.target_producer_surface_rate_m3_day
                    .unwrap_or(sim.target_producer_rate_m3_day)
                    .max(0.0),
            )
        }
    }
}

fn group_bhp_limit(sim: &ReservoirSimulator, group: FimControlGroup) -> f64 {
    match group {
        FimControlGroup::Injector => sim.well_bhp_max,
        FimControlGroup::Producer => sim.well_bhp_min,
    }
}

fn group_uses_surface_target(sim: &ReservoirSimulator, group: FimControlGroup) -> bool {
    match group {
        FimControlGroup::Injector => sim.target_injector_surface_rate_m3_day.is_some(),
        FimControlGroup::Producer => sim.target_producer_surface_rate_m3_day.is_some(),
    }
}

fn total_rate_for_group_bhp(
    sim: &ReservoirSimulator,
    state: &FimState,
    group: FimControlGroup,
    bhp_bar: f64,
) -> f64 {
    let injector = matches!(group, FimControlGroup::Injector);
    let surface_target = group_uses_surface_target(sim, group);
    sim.wells
        .iter()
        .filter(|well| well.injector == injector)
        .filter_map(|well| {
            if injector {
                if surface_target {
                    completion_surface_rate_sc_day(sim, state, well, bhp_bar)
                } else {
                    completion_rate_for_bhp(sim, state, well, bhp_bar).map(|q| (-q).max(0.0))
                }
            } else if surface_target {
                completion_surface_rate_sc_day(sim, state, well, bhp_bar)
            } else {
                completion_rate_for_bhp(sim, state, well, bhp_bar)
            }
        })
        .sum()
}

pub(crate) fn control_groups(sim: &ReservoirSimulator, state: &FimState) -> Vec<FimControlGroup> {
    let mut groups = Vec::new();
    if state.injector_group_bhp().is_some()
        && sim.injector_enabled
        && sim.wells.iter().any(|well| well.injector)
    {
        groups.push(FimControlGroup::Injector);
    }
    if state.producer_group_bhp().is_some() && sim.wells.iter().any(|well| !well.injector) {
        groups.push(FimControlGroup::Producer);
    }
    groups
}

pub(crate) fn control_group_residual(
    sim: &ReservoirSimulator,
    state: &FimState,
    group: FimControlGroup,
) -> Option<(usize, f64)> {
    let equation_offset = control_group_equation_offset(state, group)?;
    let bhp_bar = control_group_bhp(state, group)?;
    let target_rate = group_target_rate(sim, group)?;
    let bhp_limit = group_bhp_limit(sim, group);
    let max_achievable_rate = total_rate_for_group_bhp(sim, state, group, bhp_limit);
    let bhp_limited = target_rate >= max_achievable_rate - 1e-9;

    let residual = if bhp_limited {
        bhp_bar - bhp_limit
    } else {
        total_rate_for_group_bhp(sim, state, group, bhp_bar) - target_rate
    };
    Some((equation_offset, residual))
}

pub(crate) fn resolve_well_control(
    sim: &ReservoirSimulator,
    state: &FimState,
    well: &Well,
) -> Option<ResolvedWellControl> {
    if well.injector && !sim.injector_enabled {
        return Some(ResolvedWellControl {
            decision: WellControlDecision::Disabled,
            bhp_limited: false,
            producer_state: None,
        });
    }

    let producer_state = if well.injector {
        None
    } else {
        Some(producer_control_state(sim, state, well))
    };

    let use_rate_control = if well.injector {
        sim.injector_rate_controlled
    } else {
        sim.producer_rate_controlled
    };

    if use_rate_control {
        let group = if well.injector {
            FimControlGroup::Injector
        } else {
            FimControlGroup::Producer
        };
        let group_bhp = control_group_bhp(state, group)?;
        let target_rate = group_target_rate(sim, group)?;
        let bhp_limit = group_bhp_limit(sim, group);
        let max_achievable_rate = total_rate_for_group_bhp(sim, state, group, bhp_limit);
        let bhp_limited = target_rate >= max_achievable_rate - 1e-9;
        let q_target = completion_rate_for_bhp(sim, state, well, group_bhp)?;
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

    if !well.bhp.is_finite() {
        return None;
    }

    Some(ResolvedWellControl {
        decision: WellControlDecision::Bhp { bhp_bar: well.bhp },
        bhp_limited: false,
        producer_state,
    })
}

pub(crate) fn transport_rate_from_control(
    sim: &ReservoirSimulator,
    state: &FimState,
    well: &Well,
    control: ResolvedWellControl,
) -> Option<f64> {
    match control.decision {
        WellControlDecision::Disabled => Some(0.0),
        WellControlDecision::Rate { q_m3_day } => Some(q_m3_day),
        WellControlDecision::Bhp { bhp_bar } => completion_rate_for_bhp(sim, state, well, bhp_bar),
    }
}

pub(crate) fn component_rates_sc_day(
    sim: &ReservoirSimulator,
    state: &FimState,
    well: &Well,
    control: ResolvedWellControl,
    q_m3_day: f64,
) -> [f64; 3] {
    let id = sim.idx(well.i, well.j, well.k);
    if well.injector {
        return match sim.injected_fluid {
            InjectedFluid::Water => [q_m3_day / sim.b_w.max(1e-9), 0.0, 0.0],
            InjectedFluid::Gas => [0.0, 0.0, q_m3_day / state.derive_cell(sim, id).bg.max(1e-9)],
        };
    }

    let producer = control
        .producer_state
        .unwrap_or_else(|| producer_control_state(sim, state, well));
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
    fn water_injector_bhp_rate_uses_iterate_water_mobility() {
        let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        sim.set_injected_fluid("water").unwrap();
        sim.add_well(0, 0, 0, 1000.0, 0.1, 0.0, true).unwrap();
        sim.injector_rate_controlled = false;

        let low_sw = FimState::from_simulator(&sim);
        let mut high_sw = low_sw.clone();
        high_sw.cells[0].sw = 0.8;

        let q_low = completion_rate_for_bhp(&sim, &low_sw, &sim.wells[0], 1000.0).unwrap();
        let q_high = completion_rate_for_bhp(&sim, &high_sw, &sim.wells[0], 1000.0).unwrap();

        assert!(q_high < q_low);
    }

    #[test]
    fn producer_rate_control_state_uses_iterate_phase_fractions() {
        let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        sim.add_well(0, 0, 0, 0.0, 0.1, 0.0, false).unwrap();
        let base = FimState::from_simulator(&sim);
        let mut wet = base.clone();
        wet.cells[0].sw = 0.8;

        let base_state = producer_control_state(&sim, &base, &sim.wells[0]);
        let wet_state = producer_control_state(&sim, &wet, &sim.wells[0]);

        assert!(wet_state.water_fraction > base_state.water_fraction);
        assert!(wet_state.oil_fraction < base_state.oil_fraction);
    }

    #[test]
    fn rate_control_group_residual_vanishes_at_initialized_group_bhp() {
        let mut sim = ReservoirSimulator::new(2, 1, 1, 0.2);
        sim.set_rate_controlled_wells(true);
        sim.set_injected_fluid("water").unwrap();
        sim.add_well(0, 0, 0, 400.0, 0.1, 0.0, true).unwrap();
        sim.add_well(1, 0, 0, 50.0, 0.1, 0.0, false).unwrap();
        let state = FimState::from_simulator(&sim);

        let (_, inj_residual) =
            control_group_residual(&sim, &state, FimControlGroup::Injector).unwrap();
        let (_, prod_residual) =
            control_group_residual(&sim, &state, FimControlGroup::Producer).unwrap();

        assert!(inj_residual.abs() < 1e-6);
        assert!(prod_residual.abs() < 1e-6);
    }
}
