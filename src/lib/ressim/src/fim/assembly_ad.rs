//! Full grid-wide FIM residual/Jacobian assembly via automatic
//! differentiation, wired behind the same [`FimAssembly`] /
//! [`FimAssemblyOptions`] interface as `assembly::assemble_fim_system`.
//!
//! Every term reuses a Phase 0-3 primitive that has already been individually
//! gated (bit-parity against the real production formula, plus AD-vs-
//! numerical-Jacobian on a canonical case): accumulation
//! (`properties::cell_accumulation_generic`), face flux
//! (`flux::face_flux_residual_f64` / `face_flux_jacobian_blocks`), and wells
//! (`wells_ad`). This module's own job is purely the grid/topology loop and
//! the sparse-matrix scatter, matching `assembly.rs`'s row/column
//! conventions (`equation_offset` / `unknown_offset`) exactly so it is a
//! drop-in alternative assembler.

#![allow(dead_code)]

use nalgebra::DVector;
use sprs::TriMatI;

use crate::ReservoirSimulator;
use crate::fim::assembly::{
    DARCY_METRIC_FACTOR, FimAssembly, FimAssemblyOptions, FimAssemblyTiming, equation_offset,
    unknown_offset,
};
use crate::fim::flux::{FaceCellInput, face_flux_jacobian_blocks, face_flux_residual_f64};
use crate::fim::properties::{accumulation_jacobian_block, cell_accumulation_generic};
use crate::fim::scaling::{build_equation_scaling, build_variable_scaling};
use crate::fim::state::FimState;
use crate::fim::wells::{
    FimWellTopology, build_well_topology, effective_injected_fluid, geometric_well_index,
    perforation_local_block, physical_well_control,
};
use crate::fim::wells_ad::{
    WellCellInput, WellControlValuesGeneric, WellPerforationInputGeneric,
    component_rate_coefficients_generic, connection_rate_generic, mass_balance_neighbor_jacobian,
    mass_balance_own_jacobian, producer_fractions_generic, rate_consistency_cell_bhp_jacobian,
    well_constraint_bhp_column_and_fb_gradient, well_constraint_neighbor_rate_jacobian,
    well_constraint_own_perforation_rate_jacobian, well_constraint_residual_fb_generic,
};

fn cell_drsdt0_base_rs(sim: &ReservoirSimulator, cell_idx: usize) -> Option<f64> {
    if sim.gas_redissolution_enabled {
        None
    } else {
        Some(sim.rs[cell_idx])
    }
}

fn face_cell_input(sim: &ReservoirSimulator, state: &FimState, cell_idx: usize, depth_k: usize) -> FaceCellInput<f64> {
    let cell = state.cell(cell_idx);
    FaceCellInput {
        p: cell.pressure_bar,
        sw: cell.sw,
        hydrocarbon_var: cell.hydrocarbon_var,
        regime: cell.regime,
        depth: sim.depth_at_k(depth_k),
        drsdt0_base_rs: cell_drsdt0_base_rs(sim, cell_idx),
    }
}

fn well_cell_input(sim: &ReservoirSimulator, state: &FimState, cell_idx: usize) -> WellCellInput<f64> {
    let cell = state.cell(cell_idx);
    WellCellInput {
        p: cell.pressure_bar,
        sw: cell.sw,
        hydrocarbon_var: cell.hydrocarbon_var,
        regime: cell.regime,
        drsdt0_base_rs: cell_drsdt0_base_rs(sim, cell_idx),
    }
}

fn add_if_nonzero(tri: &mut TriMatI<f64, usize>, row: usize, col: usize, value: f64) {
    if value.abs() > 1e-14 {
        tri.add_triplet(row, col, value);
    }
}

fn well_control_generic(control: &crate::fim::wells::PhysicalWellControl) -> WellControlValuesGeneric {
    WellControlValuesGeneric {
        enabled: control.enabled,
        rate_controlled: control.rate_controlled,
        uses_surface_target: control.uses_surface_target,
        target_rate: control.target_rate,
        bhp_limit: control.bhp_limit,
        bhp_target: control.bhp_target,
    }
}

/// Mirrors `add_well_source_terms` + `add_well_constraint_equations` +
/// `add_perforation_equations`: perforation mass-balance source (+dt_days),
/// perforation rate-consistency row, and well-level constraint row.
fn add_well_residual_terms(
    sim: &ReservoirSimulator,
    state: &FimState,
    topology: &FimWellTopology,
    dt_days: f64,
    residual: &mut DVector<f64>,
) {
    let injected_fluid = effective_injected_fluid(sim);
    let mut well_perf_inputs: Vec<Vec<WellPerforationInputGeneric<f64>>> =
        (0..topology.wells.len()).map(|_| Vec::new()).collect();

    for (perf_idx, perforation) in topology.perforations.iter().enumerate() {
        let well_idx = perforation.physical_well_index;
        let injector = topology.wells[well_idx].injector;
        let cell = well_cell_input(sim, state, perforation.cell_index);
        let bhp = state.well_bhp[well_idx];
        let q = state.perforation_rates_m3_day[perf_idx];

        let neighborhood_cells = perforation_local_block(topology, state, perf_idx).control_influence_cells(sim);
        let neighborhood: Vec<WellCellInput<f64>> = neighborhood_cells
            .iter()
            .map(|&c| well_cell_input(sim, state, c))
            .collect();
        let fractions = (!injector).then(|| producer_fractions_generic::<f64>(sim, &neighborhood));

        well_perf_inputs[well_idx].push(WellPerforationInputGeneric { cell, fractions, q });

        let coefficients =
            component_rate_coefficients_generic(sim, injector, injected_fluid, &cell, fractions.as_ref());
        for local_eq in 0..3 {
            residual[equation_offset(perforation.cell_index, local_eq)] += coefficients[local_eq] * q * dt_days;
        }

        if let Some(wi_geom) = geometric_well_index(sim, perforation) {
            let connection = connection_rate_generic::<f64>(sim, wi_geom, injector, &cell, bhp);
            residual[state.perforation_equation_offset(perf_idx)] += q - connection;
        }
    }

    for well_idx in 0..topology.wells.len() {
        let injector = topology.wells[well_idx].injector;
        let control = well_control_generic(&physical_well_control(sim, topology, well_idx));
        let bhp = state.well_bhp[well_idx];
        if let Some(value) = well_constraint_residual_fb_generic(
            sim,
            injector,
            injected_fluid,
            &control,
            bhp,
            &well_perf_inputs[well_idx],
        ) {
            residual[state.well_equation_offset(well_idx)] += value;
        }
    }
}

/// Mirrors `add_exact_well_source_jacobian` + `add_exact_well_source_cell_jacobian`
/// + `add_exact_well_constraint_jacobian` + `add_exact_well_constraint_cell_jacobian`
/// + `add_exact_perforation_jacobian` + `add_exact_perforation_cell_pressure_jacobian`.
fn add_well_jacobian_terms(
    sim: &ReservoirSimulator,
    state: &FimState,
    topology: &FimWellTopology,
    dt_days: f64,
    tri: &mut TriMatI<f64, usize>,
) {
    let injected_fluid = effective_injected_fluid(sim);
    let mut well_perf_inputs: Vec<Vec<WellPerforationInputGeneric<f64>>> =
        (0..topology.wells.len()).map(|_| Vec::new()).collect();

    for (perf_idx, perforation) in topology.perforations.iter().enumerate() {
        let well_idx = perforation.physical_well_index;
        let injector = topology.wells[well_idx].injector;
        let cell = well_cell_input(sim, state, perforation.cell_index);
        let bhp = state.well_bhp[well_idx];
        let q = state.perforation_rates_m3_day[perf_idx];

        let neighborhood_cells = perforation_local_block(topology, state, perf_idx).control_influence_cells(sim);
        let neighborhood: Vec<WellCellInput<f64>> = neighborhood_cells
            .iter()
            .map(|&c| well_cell_input(sim, state, c))
            .collect();
        let connected_index = neighborhood_cells
            .iter()
            .position(|&c| c == perforation.cell_index)
            .unwrap_or(0);
        let producer_neighborhood = (!injector).then_some((neighborhood.as_slice(), connected_index));
        let fractions = (!injector).then(|| producer_fractions_generic::<f64>(sim, &neighborhood));

        well_perf_inputs[well_idx].push(WellPerforationInputGeneric { cell, fractions, q });

        let perf_row = state.perforation_equation_offset(perf_idx);
        let q_col = state.perforation_rate_unknown_offset(perf_idx);
        let bhp_col = state.well_bhp_unknown_offset(well_idx);

        if let Some(wi_geom) = geometric_well_index(sim, perforation) {
            let ([dp, dsw, dhc], dbhp) = rate_consistency_cell_bhp_jacobian(sim, wi_geom, injector, &cell, bhp);
            tri.add_triplet(perf_row, q_col, 1.0);
            add_if_nonzero(tri, perf_row, unknown_offset(perforation.cell_index, 0), dp);
            add_if_nonzero(tri, perf_row, unknown_offset(perforation.cell_index, 1), dsw);
            add_if_nonzero(tri, perf_row, unknown_offset(perforation.cell_index, 2), dhc);
            add_if_nonzero(tri, perf_row, bhp_col, dbhp);
        }

        let own = mass_balance_own_jacobian(sim, injector, injected_fluid, &cell, producer_neighborhood, q);
        for (local_eq, row) in own.iter().enumerate() {
            let eq_row = equation_offset(perforation.cell_index, local_eq);
            for v in 0..3 {
                add_if_nonzero(tri, eq_row, unknown_offset(perforation.cell_index, v), row[v] * dt_days);
            }
            add_if_nonzero(tri, eq_row, q_col, row[3] * dt_days);
        }
        if !injector {
            for (n_idx, &neighbor_cell_idx) in neighborhood_cells.iter().enumerate() {
                if n_idx == connected_index {
                    continue;
                }
                let cross = mass_balance_neighbor_jacobian(sim, &cell, &neighborhood, n_idx, q);
                for (local_eq, row) in cross.iter().enumerate() {
                    let eq_row = equation_offset(perforation.cell_index, local_eq);
                    for v in 0..3 {
                        add_if_nonzero(tri, eq_row, unknown_offset(neighbor_cell_idx, v), row[v] * dt_days);
                    }
                }
            }
        }
    }

    for well_idx in 0..topology.wells.len() {
        let injector = topology.wells[well_idx].injector;
        let control_real = physical_well_control(sim, topology, well_idx);
        let control = well_control_generic(&control_real);
        let bhp = state.well_bhp[well_idx];
        let row = state.well_equation_offset(well_idx);
        let bhp_col = state.well_bhp_unknown_offset(well_idx);

        if !control_real.enabled || !control_real.rate_controlled {
            tri.add_triplet(row, bhp_col, 1.0);
            continue;
        }

        let Some((bhp_col_value, dphi_db, rate_scale)) = well_constraint_bhp_column_and_fb_gradient(
            sim,
            injector,
            injected_fluid,
            &control,
            bhp,
            &well_perf_inputs[well_idx],
        ) else {
            continue;
        };
        add_if_nonzero(tri, row, bhp_col, bhp_col_value);
        let factor = -dphi_db / rate_scale;

        for &perf_idx in &topology.wells[well_idx].perforation_indices {
            let perforation = &topology.perforations[perf_idx];
            let cell = well_cell_input(sim, state, perforation.cell_index);
            let q = state.perforation_rates_m3_day[perf_idx];
            let q_col = state.perforation_rate_unknown_offset(perf_idx);

            let neighborhood_cells = perforation_local_block(topology, state, perf_idx).control_influence_cells(sim);
            let neighborhood: Vec<WellCellInput<f64>> = neighborhood_cells
                .iter()
                .map(|&c| well_cell_input(sim, state, c))
                .collect();
            let connected_index = neighborhood_cells
                .iter()
                .position(|&c| c == perforation.cell_index)
                .unwrap_or(0);
            let producer_neighborhood = (!injector).then_some((neighborhood.as_slice(), connected_index));

            let own = well_constraint_own_perforation_rate_jacobian(
                sim,
                injector,
                injected_fluid,
                control_real.uses_surface_target,
                &cell,
                producer_neighborhood,
                q,
            );
            add_if_nonzero(tri, row, unknown_offset(perforation.cell_index, 0), factor * own[0]);
            add_if_nonzero(tri, row, unknown_offset(perforation.cell_index, 1), factor * own[1]);
            add_if_nonzero(tri, row, unknown_offset(perforation.cell_index, 2), factor * own[2]);
            add_if_nonzero(tri, row, q_col, factor * own[3]);

            if !injector && control_real.uses_surface_target {
                for (n_idx, &neighbor_cell_idx) in neighborhood_cells.iter().enumerate() {
                    if n_idx == connected_index {
                        continue;
                    }
                    let cross = well_constraint_neighbor_rate_jacobian(sim, &cell, &neighborhood, n_idx, q);
                    for v in 0..3 {
                        add_if_nonzero(tri, row, unknown_offset(neighbor_cell_idx, v), factor * cross[v]);
                    }
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn add_face_residual(
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
    let geom_t = DARCY_METRIC_FACTOR * sim.geometric_transmissibility(id_i, id_j, dim);
    if geom_t <= 0.0 {
        return;
    }
    let i = face_cell_input(sim, state, id_i, k_i);
    let j = face_cell_input(sim, state, id_j, k_j);
    let r = face_flux_residual_f64(sim, geom_t, dt_days, &i, &j);
    for component in 0..3 {
        residual[equation_offset(id_i, component)] += r[component];
        residual[equation_offset(id_j, component)] += r[3 + component];
    }
}

fn scatter_block(tri: &mut TriMatI<f64, usize>, row_cell: usize, col_cell: usize, block: [[f64; 3]; 3]) {
    for (eq, row) in block.iter().enumerate() {
        for (var, value) in row.iter().enumerate() {
            if value.abs() > 1e-14 {
                tri.add_triplet(equation_offset(row_cell, eq), unknown_offset(col_cell, var), *value);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn add_face_jacobian(
    sim: &ReservoirSimulator,
    state: &FimState,
    dt_days: f64,
    id_i: usize,
    id_j: usize,
    dim: char,
    k_i: usize,
    k_j: usize,
    tri: &mut TriMatI<f64, usize>,
) {
    let geom_t = DARCY_METRIC_FACTOR * sim.geometric_transmissibility(id_i, id_j, dim);
    if geom_t <= 0.0 {
        return;
    }
    let i = face_cell_input(sim, state, id_i, k_i);
    let j = face_cell_input(sim, state, id_j, k_j);
    let (bii, bij, bji, bjj) = face_flux_jacobian_blocks(sim, geom_t, dt_days, &i, &j);

    scatter_block(tri, id_i, id_i, bii);
    scatter_block(tri, id_i, id_j, bij);
    scatter_block(tri, id_j, id_i, bji);
    scatter_block(tri, id_j, id_j, bjj);
}

/// AD-based drop-in replacement for `assembly::assemble_fim_system`.
pub(crate) fn assemble_fim_system_ad(
    sim: &ReservoirSimulator,
    previous_state: &FimState,
    state: &FimState,
    options: &FimAssemblyOptions,
) -> FimAssembly {
    let owned_topology;
    let topology = if let Some(cached) = options.topology {
        cached
    } else {
        owned_topology = build_well_topology(sim);
        &owned_topology
    };

    let n_cells = state.cells.len();
    let n_unknowns = state.n_unknowns();
    let equation_scaling = build_equation_scaling(sim, state, topology, options.dt_days);
    let variable_scaling = build_variable_scaling(sim, state);

    let mut residual = DVector::zeros(n_unknowns);

    for cell_idx in 0..n_cells {
        let cell = state.cell(cell_idx);
        let prev_cell = previous_state.cell(cell_idx);
        let drsdt0 = cell_drsdt0_base_rs(sim, cell_idx);
        let acc = cell_accumulation_generic::<f64>(
            sim,
            cell_idx,
            cell.pressure_bar,
            cell.sw,
            cell.hydrocarbon_var,
            cell.regime,
            drsdt0,
            prev_cell.pressure_bar,
            prev_cell.sw,
            prev_cell.hydrocarbon_var,
            prev_cell.regime,
        );
        for local_eq in 0..3 {
            residual[equation_offset(cell_idx, local_eq)] += acc[local_eq];
        }
    }

    for k in 0..sim.nz {
        for j in 0..sim.ny {
            for i in 0..sim.nx {
                let id = sim.idx(i, j, k);
                if i + 1 < sim.nx {
                    add_face_residual(sim, state, options.dt_days, id, sim.idx(i + 1, j, k), 'x', k, k, &mut residual);
                }
                if j + 1 < sim.ny {
                    add_face_residual(sim, state, options.dt_days, id, sim.idx(i, j + 1, k), 'y', k, k, &mut residual);
                }
                if k + 1 < sim.nz {
                    add_face_residual(
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
        add_well_residual_terms(sim, state, topology, options.dt_days, &mut residual);
    }

    if options.assemble_residual_only {
        return FimAssembly {
            residual,
            jacobian: TriMatI::<f64, usize>::new((n_unknowns, n_unknowns)).to_csr(),
            equation_scaling,
            variable_scaling,
            timing: FimAssemblyTiming::default(),
        };
    }

    let mut tri = TriMatI::<f64, usize>::new((n_unknowns, n_unknowns));

    for cell_idx in 0..n_cells {
        let cell = state.cell(cell_idx);
        let prev_cell = previous_state.cell(cell_idx);
        let drsdt0 = cell_drsdt0_base_rs(sim, cell_idx);
        let block = accumulation_jacobian_block(
            sim,
            cell_idx,
            cell.pressure_bar,
            cell.sw,
            cell.hydrocarbon_var,
            cell.regime,
            drsdt0,
            prev_cell.pressure_bar,
            prev_cell.sw,
            prev_cell.hydrocarbon_var,
            prev_cell.regime,
        );
        scatter_block(&mut tri, cell_idx, cell_idx, block);
    }

    for k in 0..sim.nz {
        for j in 0..sim.ny {
            for i in 0..sim.nx {
                let id = sim.idx(i, j, k);
                if i + 1 < sim.nx {
                    add_face_jacobian(sim, state, options.dt_days, id, sim.idx(i + 1, j, k), 'x', k, k, &mut tri);
                }
                if j + 1 < sim.ny {
                    add_face_jacobian(sim, state, options.dt_days, id, sim.idx(i, j + 1, k), 'y', k, k, &mut tri);
                }
                if k + 1 < sim.nz {
                    add_face_jacobian(
                        sim,
                        state,
                        options.dt_days,
                        id,
                        sim.idx(i, j, k + 1),
                        'z',
                        k,
                        k + 1,
                        &mut tri,
                    );
                }
            }
        }
    }

    if options.include_wells {
        add_well_jacobian_terms(sim, state, topology, options.dt_days, &mut tri);
    }

    FimAssembly {
        residual,
        jacobian: tri.to_csr(),
        equation_scaling,
        variable_scaling,
        timing: FimAssemblyTiming::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fim::assembly::assemble_fim_system;
    use crate::fim::numjac::{assert_jacobian_matches, central_difference_jacobian};
    use crate::fim::state::HydrocarbonState;
    use crate::pvt::{PvtRow, PvtTable};

    /// 2x2x1 grid (4 cells) so both x- and y-direction faces are exercised,
    /// with gravity and capillary pressure on so the flux Jacobian's gravity/
    /// capillary terms are covered too.
    fn reservoir_only_fixture() -> (ReservoirSimulator, FimState, FimState) {
        let mut sim = ReservoirSimulator::new(2, 2, 1, 0.2);
        sim.set_three_phase_mode_enabled(true);
        sim.set_gravity_enabled(true);
        sim.pvt_table = Some(PvtTable::new(
            vec![
                PvtRow {
                    p_bar: 100.0,
                    rs_m3m3: 10.0,
                    bo_m3m3: 1.2,
                    mu_o_cp: 1.4,
                    bg_m3m3: 0.02,
                    mu_g_cp: 0.03,
                },
                PvtRow {
                    p_bar: 200.0,
                    rs_m3m3: 20.0,
                    bo_m3m3: 1.1,
                    mu_o_cp: 1.2,
                    bg_m3m3: 0.01,
                    mu_g_cp: 0.025,
                },
            ],
            sim.pvt.c_o,
        ));
        sim.set_three_phase_rel_perm_props(0.1, 0.1, 0.05, 0.05, 0.15, 2.0, 2.0, 2.0, 1.0, 1.0, 1.0)
            .unwrap();
        sim.set_gas_oil_capillary_params(2.0, 2.0).unwrap();

        let previous_state = FimState::from_simulator(&sim);
        let mut state = previous_state.clone();
        // Distinct, off-kink saturations/pressures per cell so every face and
        // both regimes are actually exercised.
        state.cells[0].pressure_bar = 210.0;
        state.cells[0].sw = 0.32;
        state.cells[0].regime = HydrocarbonState::Saturated;
        state.cells[0].hydrocarbon_var = 0.20;
        state.cells[1].pressure_bar = 195.0;
        state.cells[1].sw = 0.28;
        state.cells[1].regime = HydrocarbonState::Undersaturated;
        state.cells[1].hydrocarbon_var = 16.0;
        state.cells[2].pressure_bar = 205.0;
        state.cells[2].sw = 0.30;
        state.cells[2].regime = HydrocarbonState::Saturated;
        state.cells[2].hydrocarbon_var = 0.15;
        state.cells[3].pressure_bar = 190.0;
        state.cells[3].sw = 0.26;
        state.cells[3].regime = HydrocarbonState::Undersaturated;
        state.cells[3].hydrocarbon_var = 14.0;

        (sim, previous_state, state)
    }

    fn no_wells_options() -> FimAssemblyOptions<'static> {
        FimAssemblyOptions {
            dt_days: 0.75,
            include_wells: false,
            assemble_residual_only: false,
            topology: None,
        }
    }

    /// Bit-identical residual gate: the AD assembler's residual (accumulation
    /// + flux, no wells) must match the real `assemble_fim_system` exactly --
    /// only the Jacobian construction differs between the two assemblers.
    #[test]
    fn residual_matches_real_assembler_no_wells() {
        let (sim, previous_state, state) = reservoir_only_fixture();
        let options = no_wells_options();

        let real = assemble_fim_system(&sim, &previous_state, &state, &options);
        let generic = assemble_fim_system_ad(&sim, &previous_state, &state, &options);

        assert_eq!(real.residual.len(), generic.residual.len());
        for i in 0..real.residual.len() {
            assert!(
                (real.residual[i] - generic.residual[i]).abs() < 1e-10,
                "residual[{i}]: real={} generic={}",
                real.residual[i],
                generic.residual[i]
            );
        }
    }

    /// The AD Jacobian must match a numerical (central-difference) Jacobian
    /// of the REAL scalar residual (`assemble_fim_system`'s, not the AD
    /// assembler's own) on the full 4-cell system -- this is the sharpest
    /// possible check: it validates both that the AD Jacobian is internally
    /// consistent AND that it is the Jacobian of the actual production
    /// residual, not just of this module's own residual re-implementation.
    #[test]
    fn jacobian_matches_numerical_of_real_residual_no_wells() {
        let (sim, previous_state, state) = reservoir_only_fixture();
        let options = no_wells_options();

        let generic = assemble_fim_system_ad(&sim, &previous_state, &state, &options);
        let n = generic.residual.len();

        let mut analytic = vec![vec![0.0; n]; n];
        for (value, (row, col)) in generic.jacobian.iter() {
            analytic[row][col] += *value;
        }

        let x0: Vec<f64> = state
            .cells
            .iter()
            .flat_map(|c| [c.pressure_bar, c.sw, c.hydrocarbon_var])
            .collect();
        let residual = |x: &[f64]| {
            let mut perturbed = state.clone();
            for (idx, cell) in perturbed.cells.iter_mut().enumerate() {
                cell.pressure_bar = x[3 * idx];
                cell.sw = x[3 * idx + 1];
                cell.hydrocarbon_var = x[3 * idx + 2];
            }
            assemble_fim_system(&sim, &previous_state, &perturbed, &options)
                .residual
                .iter()
                .copied()
                .collect::<Vec<_>>()
        };
        let numerical = central_difference_jacobian(&x0, n, residual);

        assert_jacobian_matches(&analytic, &numerical, 1e-5, 1e-7);
    }

    /// 3x3x1 grid with a rate-controlled, surface-target producer at the
    /// CENTER cell (1,1,0) -- giving it the full 3x3 control neighborhood, so
    /// both the mass-balance and well-constraint neighbor cross-terms are
    /// genuinely exercised (not just the trivial single-cell case) -- plus a
    /// BHP-controlled injector at a corner for contrast.
    fn reservoir_with_wells_fixture() -> (ReservoirSimulator, FimState, FimState) {
        let mut sim = ReservoirSimulator::new(3, 3, 1, 0.2);
        sim.set_three_phase_mode_enabled(true);
        sim.set_gravity_enabled(true);
        sim.set_injected_fluid("water").unwrap();
        sim.pvt_table = Some(PvtTable::new(
            vec![
                PvtRow {
                    p_bar: 100.0,
                    rs_m3m3: 10.0,
                    bo_m3m3: 1.2,
                    mu_o_cp: 1.4,
                    bg_m3m3: 0.02,
                    mu_g_cp: 0.03,
                },
                PvtRow {
                    p_bar: 200.0,
                    rs_m3m3: 20.0,
                    bo_m3m3: 1.1,
                    mu_o_cp: 1.2,
                    bg_m3m3: 0.01,
                    mu_g_cp: 0.025,
                },
            ],
            sim.pvt.c_o,
        ));
        sim.set_three_phase_rel_perm_props(0.1, 0.1, 0.05, 0.05, 0.15, 2.0, 2.0, 2.0, 1.0, 1.0, 1.0)
            .unwrap();
        sim.set_gas_oil_capillary_params(2.0, 2.0).unwrap();

        sim.add_well(0, 0, 0, 250.0, 0.1, 0.0, true).unwrap(); // injector, corner
        sim.add_well(1, 1, 0, 60.0, 0.1, 0.0, false).unwrap(); // producer, center
        sim.set_rate_controlled_wells(true);
        sim.set_target_well_surface_rates(80.0, 30.0).unwrap();
        sim.well_bhp_min = 30.0;
        sim.well_bhp_max = 400.0;

        let previous_state = FimState::from_simulator(&sim);
        let mut state = previous_state.clone();
        for (idx, cell) in state.cells.iter_mut().enumerate() {
            let base_p = 195.0 + 3.0 * idx as f64;
            cell.pressure_bar = base_p;
            cell.sw = 0.25 + 0.01 * idx as f64;
            if idx % 2 == 0 {
                cell.regime = HydrocarbonState::Saturated;
                cell.hydrocarbon_var = 0.10 + 0.01 * idx as f64;
            } else {
                cell.regime = HydrocarbonState::Undersaturated;
                cell.hydrocarbon_var = 14.0 + 0.2 * idx as f64;
            }
        }
        state.well_bhp[0] = 230.0;
        state.well_bhp[1] = 90.0;
        state.perforation_rates_m3_day[0] = -25.0;
        state.perforation_rates_m3_day[1] = 20.0;

        (sim, previous_state, state)
    }

    fn with_wells_options() -> FimAssemblyOptions<'static> {
        FimAssemblyOptions {
            dt_days: 0.5,
            include_wells: true,
            assemble_residual_only: false,
            topology: None,
        }
    }

    #[test]
    fn residual_matches_real_assembler_with_wells() {
        let (sim, previous_state, state) = reservoir_with_wells_fixture();
        let options = with_wells_options();

        let real = assemble_fim_system(&sim, &previous_state, &state, &options);
        let generic = assemble_fim_system_ad(&sim, &previous_state, &state, &options);

        assert_eq!(real.residual.len(), generic.residual.len());
        for i in 0..real.residual.len() {
            assert!(
                (real.residual[i] - generic.residual[i]).abs() < 1e-9,
                "residual[{i}]: real={} generic={}",
                real.residual[i],
                generic.residual[i]
            );
        }
    }

    /// Same sharp check as the no-wells gate, but now over the full unknown
    /// vector (cells + well BHPs + perforation rates), which is exactly what
    /// exercises the neighbor cross-term scatter this final integration step
    /// added (`mass_balance_neighbor_jacobian` /
    /// `well_constraint_neighbor_rate_jacobian`).
    #[test]
    fn jacobian_matches_numerical_of_real_residual_with_wells() {
        let (sim, previous_state, state) = reservoir_with_wells_fixture();
        let options = with_wells_options();

        let generic = assemble_fim_system_ad(&sim, &previous_state, &state, &options);
        let n = generic.residual.len();

        let mut analytic = vec![vec![0.0; n]; n];
        for (value, (row, col)) in generic.jacobian.iter() {
            analytic[row][col] += *value;
        }

        let n_cells = state.cells.len();
        let n_wells = state.n_well_unknowns();
        let mut x0: Vec<f64> = state
            .cells
            .iter()
            .flat_map(|c| [c.pressure_bar, c.sw, c.hydrocarbon_var])
            .collect();
        x0.extend_from_slice(&state.well_bhp);
        x0.extend_from_slice(&state.perforation_rates_m3_day);

        let residual = |x: &[f64]| {
            let mut perturbed = state.clone();
            for (idx, cell) in perturbed.cells.iter_mut().enumerate() {
                cell.pressure_bar = x[3 * idx];
                cell.sw = x[3 * idx + 1];
                cell.hydrocarbon_var = x[3 * idx + 2];
            }
            for (idx, bhp) in perturbed.well_bhp.iter_mut().enumerate() {
                *bhp = x[3 * n_cells + idx];
            }
            for (idx, rate) in perturbed.perforation_rates_m3_day.iter_mut().enumerate() {
                *rate = x[3 * n_cells + n_wells + idx];
            }
            assemble_fim_system(&sim, &previous_state, &perturbed, &options)
                .residual
                .iter()
                .copied()
                .collect::<Vec<_>>()
        };
        let numerical = central_difference_jacobian(&x0, n, residual);

        assert_jacobian_matches(&analytic, &numerical, 1e-5, 1e-6);
    }
}
