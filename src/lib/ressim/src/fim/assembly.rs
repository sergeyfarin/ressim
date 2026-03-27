use nalgebra::DVector;
use sprs::{CsMat, TriMatI};

use crate::fim::scaling::{
    build_equation_scaling, build_variable_scaling, EquationScaling, VariableScaling,
};
use crate::fim::state::FimState;
use crate::fim::wells::{
    collect_perforations, component_rates_sc_day, control_group_residual, control_groups,
    resolve_well_control, transport_rate_from_control,
};
use crate::ReservoirSimulator;

const DARCY_METRIC_FACTOR: f64 = 8.526_988_8e-3;

pub(crate) struct FimAssembly {
    pub(crate) residual: DVector<f64>,
    pub(crate) jacobian: CsMat<f64>,
    pub(crate) equation_scaling: EquationScaling,
    pub(crate) variable_scaling: VariableScaling,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FimAssemblyOptions {
    pub(crate) dt_days: f64,
    pub(crate) include_wells: bool,
}

pub(crate) fn unknown_offset(cell_idx: usize, local_var: usize) -> usize {
    cell_idx * 3 + local_var
}

pub(crate) fn equation_offset(cell_idx: usize, local_eq: usize) -> usize {
    cell_idx * 3 + local_eq
}

pub(crate) fn assemble_fim_system(
    sim: &ReservoirSimulator,
    previous_state: &FimState,
    state: &FimState,
    options: &FimAssemblyOptions,
) -> FimAssembly {
    let n_unknowns = state.n_unknowns();
    let equation_scaling = build_equation_scaling(sim, state, options.dt_days);
    let variable_scaling = build_variable_scaling(sim, state);
    let mut tri = TriMatI::<f64, usize>::new((n_unknowns, n_unknowns));
    let residual = assemble_residual(sim, previous_state, state, options);

    for unknown_idx in 0..n_unknowns {
        let perturbation = finite_difference_step(state, unknown_idx);
        let mut update = DVector::zeros(n_unknowns);
        update[unknown_idx] = perturbation;
        let perturbed_state = state.apply_newton_update(sim, &update, 1.0);
        let perturbed_residual = assemble_residual(sim, previous_state, &perturbed_state, options);

        for equation_idx in 0..n_unknowns {
            let jac = (perturbed_residual[equation_idx] - residual[equation_idx]) / perturbation;
            if jac.abs() > 1e-14 {
                tri.add_triplet(equation_idx, unknown_idx, jac);
            }
        }
    }

    FimAssembly {
        residual,
        jacobian: tri.to_csr(),
        equation_scaling,
        variable_scaling,
    }
}

fn finite_difference_step(state: &FimState, unknown_idx: usize) -> f64 {
    if unknown_idx < state.n_cell_unknowns() {
        let cell_idx = unknown_idx / 3;
        let local_var = unknown_idx % 3;
        let cell = state.cell(cell_idx);
        return match local_var {
            0 => 1e-5 * cell.pressure_bar.abs().max(1.0),
            1 => 1e-7,
            2 => 1e-7 * cell.hydrocarbon_var.abs().max(1.0),
            _ => unreachable!(),
        };
    }

    let bhp_bar = if Some(unknown_idx) == state.injector_group_unknown_offset() {
        state.injector_group_bhp().unwrap_or(1.0)
    } else {
        state.producer_group_bhp().unwrap_or(1.0)
    };
    1e-5 * bhp_bar.abs().max(1.0)
}

fn pore_volume_at_state(
    sim: &ReservoirSimulator,
    previous_state: &FimState,
    state: &FimState,
    cell_idx: usize,
) -> f64 {
    let pore_volume_ref_m3 = sim.pore_volume_m3(cell_idx);
    let pressure_delta_bar =
        state.cell(cell_idx).pressure_bar - previous_state.cell(cell_idx).pressure_bar;
    pore_volume_ref_m3 * f64::exp(sim.rock_compressibility * pressure_delta_bar)
}

fn cell_component_inventory_sc(
    sim: &ReservoirSimulator,
    previous_state: &FimState,
    state: &FimState,
    cell_idx: usize,
) -> [f64; 3] {
    let pore_volume_m3 = pore_volume_at_state(sim, previous_state, state, cell_idx).max(1e-9);
    let cell = state.cell(cell_idx);
    let derived = state.derive_cell(sim, cell_idx);

    let water_sc = pore_volume_m3 * cell.sw / sim.b_w.max(1e-9);
    let oil_sc = pore_volume_m3 * derived.so / derived.bo.max(1e-9);
    let gas_sc = pore_volume_m3 * derived.sg / derived.bg.max(1e-9) + oil_sc * derived.rs;

    [water_sc, oil_sc, gas_sc]
}

fn cell_accumulation_residual(
    sim: &ReservoirSimulator,
    previous_state: &FimState,
    state: &FimState,
    cell_idx: usize,
) -> [f64; 3] {
    let current = cell_component_inventory_sc(sim, previous_state, state, cell_idx);
    let previous = cell_component_inventory_sc(sim, previous_state, previous_state, cell_idx);
    [
        current[0] - previous[0],
        current[1] - previous[1],
        current[2] - previous[2],
    ]
}

fn assemble_residual(
    sim: &ReservoirSimulator,
    previous_state: &FimState,
    state: &FimState,
    options: &FimAssemblyOptions,
) -> DVector<f64> {
    let n_unknowns = state.n_unknowns();
    let mut residual = DVector::zeros(n_unknowns);

    for cell_idx in 0..state.cells.len() {
        let accumulation = cell_accumulation_residual(sim, previous_state, state, cell_idx);
        for local_eq in 0..3 {
            residual[equation_offset(cell_idx, local_eq)] += accumulation[local_eq];
        }
    }

    for k in 0..sim.nz {
        for j in 0..sim.ny {
            for i in 0..sim.nx {
                let id = sim.idx(i, j, k);

                if i + 1 < sim.nx {
                    add_interface_flux(
                        sim,
                        state,
                        options.dt_days,
                        id,
                        sim.idx(i + 1, j, k),
                        'x',
                        k,
                        k,
                        &mut residual,
                    );
                }
                if j + 1 < sim.ny {
                    add_interface_flux(
                        sim,
                        state,
                        options.dt_days,
                        id,
                        sim.idx(i, j + 1, k),
                        'y',
                        k,
                        k,
                        &mut residual,
                    );
                }
                if k + 1 < sim.nz {
                    add_interface_flux(
                        sim,
                        state,
                        options.dt_days,
                        id,
                        sim.idx(i, j, k + 1),
                        'z',
                        k,
                        k + 1,
                        &mut residual,
                    );
                }
            }
        }
    }

    if options.include_wells {
        add_well_source_terms(sim, state, options.dt_days, &mut residual);
        add_well_control_equations(sim, state, &mut residual);
    }

    residual
}

fn add_well_source_terms(
    sim: &ReservoirSimulator,
    state: &FimState,
    dt_days: f64,
    residual: &mut DVector<f64>,
) {
    for perforation in collect_perforations(sim) {
        let well = &sim.wells[perforation.well_index];
        let Some(control) = resolve_well_control(sim, state, well) else {
            continue;
        };
        let id = perforation.cell_index;
        let Some(q_m3_day) = transport_rate_from_control(sim, state, well, control) else {
            continue;
        };

        let components_sc_day = component_rates_sc_day(sim, state, well, control, q_m3_day);
        for (local_eq, component_rate) in components_sc_day.into_iter().enumerate() {
            residual[equation_offset(id, local_eq)] += component_rate * dt_days;
        }
    }
}

fn add_well_control_equations(
    sim: &ReservoirSimulator,
    state: &FimState,
    residual: &mut DVector<f64>,
) {
    for group in control_groups(sim, state) {
        if let Some((equation_idx, group_residual)) = control_group_residual(sim, state, group) {
            residual[equation_idx] += group_residual;
        }
    }
}

fn add_interface_flux(
    sim: &ReservoirSimulator,
    state: &FimState,
    dt_days: f64,
    id_i: usize,
    id_j: usize,
    dim: char,
    k_i: usize,
    k_j: usize,
    residual: &mut DVector<f64>,
) {
    let cell_i = state.cell(id_i);
    let cell_j = state.cell(id_j);
    let derived_i = state.derive_cell(sim, id_i);
    let derived_j = state.derive_cell(sim, id_j);

    let p_i = cell_i.pressure_bar;
    let p_j = cell_j.pressure_bar;
    let depth_i = sim.depth_at_k(k_i);
    let depth_j = sim.depth_at_k(k_j);
    let geom_t = DARCY_METRIC_FACTOR * sim.geometric_transmissibility(id_i, id_j, dim);

    if geom_t <= 0.0 {
        return;
    }

    let pcw_i = sim.get_capillary_pressure(cell_i.sw);
    let pcw_j = sim.get_capillary_pressure(cell_j.sw);
    let pcog_i = sim.get_gas_oil_capillary_pressure(derived_i.sg);
    let pcog_j = sim.get_gas_oil_capillary_pressure(derived_j.sg);

    let grav_w = sim.gravity_head_bar(
        depth_i,
        depth_j,
        sim.interface_density_barrier(derived_i.rho_w, derived_j.rho_w),
    );
    let grav_o = sim.gravity_head_bar(
        depth_i,
        depth_j,
        sim.interface_density_barrier(derived_i.rho_o, derived_j.rho_o),
    );
    let grav_g = sim.gravity_head_bar(
        depth_i,
        depth_j,
        sim.interface_density_barrier(derived_i.rho_g, derived_j.rho_g),
    );

    let dphi_w = (p_i - p_j) - (pcw_i - pcw_j) - grav_w;
    let dphi_o = (p_i - p_j) - grav_o;
    let dphi_g = (p_i - p_j) + (pcog_i - pcog_j) - grav_g;

    let mobilities_i = sim.phase_mobilities_for_state(cell_i.sw, derived_i.sg, p_i, derived_i.rs);
    let mobilities_j = sim.phase_mobilities_for_state(cell_j.sw, derived_j.sg, p_j, derived_j.rs);

    let water_upstream = if dphi_w >= 0.0 {
        (id_i, cell_i, derived_i, mobilities_i)
    } else {
        (id_j, cell_j, derived_j, mobilities_j)
    };
    let oil_upstream = if dphi_o >= 0.0 {
        (id_i, cell_i, derived_i, mobilities_i)
    } else {
        (id_j, cell_j, derived_j, mobilities_j)
    };
    let gas_upstream = if dphi_g >= 0.0 {
        (id_i, cell_i, derived_i, mobilities_i)
    } else {
        (id_j, cell_j, derived_j, mobilities_j)
    };

    let q_w_sc_day = geom_t * water_upstream.3.water * dphi_w / sim.b_w.max(1e-9);

    let q_o_res_day = geom_t * oil_upstream.3.oil * dphi_o;
    let q_o_sc_day = q_o_res_day / oil_upstream.2.bo.max(1e-9);

    let q_g_free_sc_day = geom_t * gas_upstream.3.gas * dphi_g / gas_upstream.2.bg.max(1e-9);
    let q_g_dissolved_sc_day = q_o_sc_day * oil_upstream.2.rs;
    let q_g_sc_day = q_g_free_sc_day + q_g_dissolved_sc_day;

    let flux_sc = [
        q_w_sc_day * dt_days,
        q_o_sc_day * dt_days,
        q_g_sc_day * dt_days,
    ];
    for (local_eq, component_flux) in flux_sc.into_iter().enumerate() {
        residual[equation_offset(id_i, local_eq)] += component_flux;
        residual[equation_offset(id_j, local_eq)] -= component_flux;
    }
}

#[cfg(test)]
mod tests {
    use crate::fim::state::{FimCellState, FimState, HydrocarbonState};
    use crate::ReservoirSimulator;

    use super::*;

    #[test]
    fn offsets_follow_cell_major_three_unknown_layout() {
        assert_eq!(unknown_offset(0, 0), 0);
        assert_eq!(unknown_offset(0, 2), 2);
        assert_eq!(unknown_offset(1, 0), 3);
        assert_eq!(equation_offset(2, 1), 7);
    }

    #[test]
    fn assembly_scaffold_matches_state_size() {
        let sim = ReservoirSimulator::new(1, 1, 2, 0.2);
        let state = FimState {
            cells: vec![
                FimCellState {
                    pressure_bar: 300.0,
                    sw: 0.3,
                    hydrocarbon_var: 0.0,
                    regime: HydrocarbonState::Saturated,
                },
                FimCellState {
                    pressure_bar: 301.0,
                    sw: 0.31,
                    hydrocarbon_var: 0.0,
                    regime: HydrocarbonState::Saturated,
                },
            ],
            injector_group_bhp: None,
            producer_group_bhp: None,
        };

        let assembly = assemble_fim_system(
            &sim,
            &state,
            &state,
            &FimAssemblyOptions {
                dt_days: 1.0,
                include_wells: false,
            },
        );

        assert_eq!(assembly.residual.len(), 6);
        assert_eq!(assembly.jacobian.rows(), 6);
        assert_eq!(assembly.jacobian.cols(), 6);
        assert!(assembly.jacobian.nnz() >= 18);
        assert_eq!(assembly.equation_scaling.water.len(), 2);
        assert_eq!(assembly.variable_scaling.pressure.len(), 2);
        assert!(assembly.equation_scaling.well_control.is_empty());
    }

    #[test]
    fn accumulation_residual_is_nonzero_for_perturbed_cell_state() {
        let sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        let previous_state = FimState::from_simulator(&sim);
        let mut state = previous_state.clone();
        state.cells[0].pressure_bar += 10.0;
        state.cells[0].sw += 0.05;

        let assembly = assemble_fim_system(
            &sim,
            &previous_state,
            &state,
            &FimAssemblyOptions {
                dt_days: 1.0,
                include_wells: false,
            },
        );

        assert!(assembly.residual.norm() > 1e-9);
    }

    #[test]
    fn uniform_state_has_zero_flux_residual() {
        let sim = ReservoirSimulator::new(2, 1, 1, 0.2);
        let state = FimState::from_simulator(&sim);

        let assembly = assemble_fim_system(
            &sim,
            &state,
            &state,
            &FimAssemblyOptions {
                dt_days: 1.0,
                include_wells: false,
            },
        );

        assert!(assembly.residual.norm() <= 1e-12);
    }

    #[test]
    fn intercell_flux_is_component_conservative() {
        let sim = ReservoirSimulator::new(2, 1, 1, 0.2);
        let previous_state = FimState::from_simulator(&sim);
        let mut state = previous_state.clone();
        state.cells[0].pressure_bar = 250.0;
        state.cells[1].pressure_bar = 150.0;

        let assembly = assemble_fim_system(
            &sim,
            &previous_state,
            &state,
            &FimAssemblyOptions {
                dt_days: 1.0,
                include_wells: false,
            },
        );

        let water_sum =
            assembly.residual[equation_offset(0, 0)] + assembly.residual[equation_offset(1, 0)];
        let oil_sum =
            assembly.residual[equation_offset(0, 1)] + assembly.residual[equation_offset(1, 1)];
        let gas_sum =
            assembly.residual[equation_offset(0, 2)] + assembly.residual[equation_offset(1, 2)];

        assert!(water_sum.abs() < 1e-9);
        assert!(oil_sum.abs() < 1e-9);
        assert!(gas_sum.abs() < 1e-9);
    }

    #[test]
    fn water_injector_adds_negative_water_source_term() {
        let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        sim.set_injected_fluid("water").unwrap();
        sim.add_well(0, 0, 0, 1000.0, 0.1, 0.0, true).unwrap();
        sim.injector_rate_controlled = false;
        sim.injector_enabled = true;

        let state = FimState::from_simulator(&sim);
        let baseline = assemble_fim_system(
            &sim,
            &state,
            &state,
            &FimAssemblyOptions {
                dt_days: 1.0,
                include_wells: false,
            },
        );
        let assembly = assemble_fim_system(
            &sim,
            &state,
            &state,
            &FimAssemblyOptions {
                dt_days: 1.0,
                include_wells: true,
            },
        );

        assert!(baseline.residual.norm() <= 1e-12);
        assert!(
            assembly.residual[equation_offset(0, 0)] < baseline.residual[equation_offset(0, 0)]
        );
        assert!(
            (assembly.residual[equation_offset(0, 1)] - baseline.residual[equation_offset(0, 1)])
                .abs()
                < 1e-12
        );
        assert!(
            (assembly.residual[equation_offset(0, 2)] - baseline.residual[equation_offset(0, 2)])
                .abs()
                < 1e-12
        );
    }

    #[test]
    fn producer_source_uses_iterate_state_phase_split() {
        let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        sim.add_well(0, 0, 0, 0.0, 0.1, 0.0, false).unwrap();
        let low_sw_state = FimState::from_simulator(&sim);
        let mut high_sw_state = low_sw_state.clone();
        high_sw_state.cells[0].sw = 0.8;

        let low_sw_assembly = assemble_fim_system(
            &sim,
            &low_sw_state,
            &low_sw_state,
            &FimAssemblyOptions {
                dt_days: 1.0,
                include_wells: true,
            },
        );
        let high_sw_assembly = assemble_fim_system(
            &sim,
            &high_sw_state,
            &high_sw_state,
            &FimAssemblyOptions {
                dt_days: 1.0,
                include_wells: true,
            },
        );

        assert!(
            high_sw_assembly.residual[equation_offset(0, 0)]
                > low_sw_assembly.residual[equation_offset(0, 0)]
        );
        assert!(
            high_sw_assembly.residual[equation_offset(0, 1)]
                < low_sw_assembly.residual[equation_offset(0, 1)]
        );
    }

    #[test]
    fn rate_control_adds_extra_unknowns_and_equations() {
        let mut sim = ReservoirSimulator::new(2, 1, 1, 0.2);
        sim.set_rate_controlled_wells(true);
        sim.set_injected_fluid("water").unwrap();
        sim.add_well(0, 0, 0, 400.0, 0.1, 0.0, true).unwrap();
        sim.add_well(1, 0, 0, 50.0, 0.1, 0.0, false).unwrap();
        let state = FimState::from_simulator(&sim);

        let assembly = assemble_fim_system(
            &sim,
            &state,
            &state,
            &FimAssemblyOptions {
                dt_days: 1.0,
                include_wells: true,
            },
        );

        assert_eq!(state.n_unknowns(), 8);
        assert_eq!(assembly.residual.len(), 8);
        assert_eq!(assembly.equation_scaling.well_control.len(), 2);
        assert_eq!(assembly.variable_scaling.well_bhp.len(), 2);
    }
}
