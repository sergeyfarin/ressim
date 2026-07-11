//! Bundle W (`docs/FIM_BUNDLE_W_PLAN.md`): the per-well local nonlinear system.
//!
//! Plan §2's design constraint: the inner well solve must drive the SAME discrete residual
//! rows the global assembler (`assembly_ad.rs`) uses for `well_constraint` +
//! `rate_consistency`, not an independently-derived rate formula — that is exactly how
//! `relax_well_state_toward_local_consistency` produced the standoff `FIM-DIAG-002` measured
//! (worklog "Late-window trace diagnostic on the 18k pathology"). This module is built by
//! calling the identical shared AD primitives (`wells_ad.rs`) the global assembler calls for
//! those rows, restricted to one physical well's unknowns (`bhp` + its perforations' `q`) with
//! the reservoir cell state held frozen (input data, not unknowns). `W1AgreementTest` below
//! verifies this by construction: the local residual/Jacobian entries must exactly match the
//! corresponding rows/columns of a full `assemble_fim_system_ad` call.

use nalgebra::{DMatrix, DVector};

use crate::ReservoirSimulator;
use crate::fim::assembly_ad::{well_cell_input, well_control_generic};
use crate::fim::state::FimState;
use crate::fim::wells::{
    FimWellTopology, effective_injected_fluid, geometric_well_index, perforation_local_block,
    physical_well_control,
};
use crate::fim::wells_ad::{
    WellCellInput, WellPerforationInputGeneric, connection_rate_generic,
    producer_fractions_generic, rate_consistency_cell_bhp_jacobian,
    well_constraint_bhp_column_and_fb_gradient, well_constraint_own_perforation_rate_jacobian,
    well_constraint_residual_fb_generic,
};

/// One physical well's local residual/Jacobian, evaluated at the frozen reservoir cell state
/// carried by `state`. Local row/unknown ordering: index `0` is the well's `well_constraint`
/// equation (unknown `bhp`); indices `1..=n` are each perforation's `rate_consistency`
/// equation (unknown `q`), in the same order as `perforation_indices`. This ordering is local
/// to this struct — callers map back to global offsets via `state.well_equation_offset`/
/// `state.perforation_equation_offset` using `well_idx`/`perforation_indices`, not by assuming
/// any relationship to the global unknown layout.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FimWellLocalSystem {
    pub(crate) residual: DVector<f64>,
    pub(crate) jacobian: DMatrix<f64>,
    pub(crate) perforation_indices: Vec<usize>,
}

impl FimWellLocalSystem {
    pub(crate) fn dim(&self) -> usize {
        1 + self.perforation_indices.len()
    }
}

/// Assembles one physical well's local system. Mirrors `assembly_ad.rs`'s
/// `add_well_residual_terms`/`add_well_jacobian_terms` well-row loops exactly (same function
/// calls, same argument construction) but scoped to a single `well_idx` and omitting every
/// cell-unknown column (the reservoir state is frozen input here, not solved-for).
pub(crate) fn assemble_well_local_system(
    sim: &ReservoirSimulator,
    state: &FimState,
    topology: &FimWellTopology,
    well_idx: usize,
) -> FimWellLocalSystem {
    let injected_fluid = effective_injected_fluid(sim);
    let injector = topology.wells[well_idx].injector;
    let control_real = physical_well_control(sim, topology, well_idx);
    let control = well_control_generic(&control_real);
    let bhp = state.well_bhp[well_idx];
    let perforation_indices = topology.wells[well_idx].perforation_indices.clone();
    let n_perf = perforation_indices.len();
    let dim = 1 + n_perf;

    // Per-perforation generic inputs, built identically to the global assembler's
    // `well_perf_inputs[well_idx]` construction.
    let mut well_perf_inputs: Vec<WellPerforationInputGeneric<f64>> = Vec::with_capacity(n_perf);
    let mut cells: Vec<WellCellInput<f64>> = Vec::with_capacity(n_perf);
    let mut neighborhoods: Vec<Vec<WellCellInput<f64>>> = Vec::with_capacity(n_perf);
    let mut connected_indices: Vec<usize> = Vec::with_capacity(n_perf);
    let mut wi_geoms: Vec<Option<f64>> = Vec::with_capacity(n_perf);

    for &perf_idx in &perforation_indices {
        let perforation = &topology.perforations[perf_idx];
        let cell = well_cell_input(sim, state, perforation.cell_index);
        let q = state.perforation_rates_m3_day[perf_idx];

        let neighborhood_cells =
            perforation_local_block(topology, state, perf_idx).control_influence_cells(sim);
        let neighborhood: Vec<WellCellInput<f64>> = neighborhood_cells
            .iter()
            .map(|&c| well_cell_input(sim, state, c))
            .collect();
        let connected_index = neighborhood_cells
            .iter()
            .position(|&c| c == perforation.cell_index)
            .unwrap_or(0);
        let fractions = (!injector).then(|| producer_fractions_generic::<f64>(sim, &neighborhood));

        well_perf_inputs.push(WellPerforationInputGeneric { cell, fractions, q });
        cells.push(cell);
        neighborhoods.push(neighborhood);
        connected_indices.push(connected_index);
        wi_geoms.push(geometric_well_index(sim, perforation));
    }

    let mut residual = DVector::zeros(dim);
    let mut jacobian = DMatrix::zeros(dim, dim);

    // Row 0: well_constraint, unknown 0 = bhp. Mirrors the global assembler's residual loop
    // (`add_well_residual_terms`'s well_idx loop) and Jacobian loop
    // (`add_well_jacobian_terms`'s well_idx loop) exactly.
    if let Some(value) = well_constraint_residual_fb_generic(
        sim,
        injector,
        injected_fluid,
        &control,
        bhp,
        &well_perf_inputs,
    ) {
        residual[0] = value;
    }
    if !control_real.enabled || !control_real.rate_controlled {
        jacobian[(0, 0)] = 1.0;
    } else if let Some((bhp_col_value, dphi_db, rate_scale)) =
        well_constraint_bhp_column_and_fb_gradient(
            sim,
            injector,
            injected_fluid,
            &control,
            bhp,
            &well_perf_inputs,
        )
    {
        jacobian[(0, 0)] = bhp_col_value;
        let factor = -dphi_db / rate_scale;
        for (local_perf, &perf_idx) in perforation_indices.iter().enumerate() {
            let q = state.perforation_rates_m3_day[perf_idx];
            let producer_neighborhood = (!injector).then_some((
                neighborhoods[local_perf].as_slice(),
                connected_indices[local_perf],
            ));
            let own = well_constraint_own_perforation_rate_jacobian(
                sim,
                injector,
                injected_fluid,
                control_real.uses_surface_target,
                &cells[local_perf],
                producer_neighborhood,
                q,
            );
            // own[3] = ∂(rate-slack term)/∂q for this perforation's own connected cell; the
            // [0..3] entries are ∂/∂(p,sw,hc) of that cell, and any cross-neighborhood terms
            // (`well_constraint_neighbor_rate_jacobian`) are ∂/∂ OTHER reservoir cells — all
            // frozen inputs here, correctly omitted from this local (bhp,q)-only Jacobian.
            jacobian[(0, 1 + local_perf)] += factor * own[3];
        }
    }
    // Else: `well_constraint_bhp_column_and_fb_gradient` returned `None` — matches the global
    // assembler's `let Some(...) = ... else { continue; }`, leaving this row's Jacobian at
    // zero (no contribution), same as the global assembly would produce for that row.

    // Rows 1..=n_perf: rate_consistency, unknown 1+i = this perforation's q. Mirrors the
    // residual loop's `if let Some(wi_geom) = geometric_well_index(...) { residual[...] += q -
    // connection; }` guard exactly — when WI is absent the row/column stay at zero, matching
    // the global assembler leaving that row's contribution unset.
    for (local_perf, &perf_idx) in perforation_indices.iter().enumerate() {
        let row = 1 + local_perf;
        let Some(wi_geom) = wi_geoms[local_perf] else {
            continue;
        };
        let q = state.perforation_rates_m3_day[perf_idx];
        let connection = connection_rate_generic(sim, wi_geom, injector, &cells[local_perf], bhp);
        residual[row] = q - connection;
        jacobian[(row, row)] = 1.0;
        let (_cell_derivs, dbhp) =
            rate_consistency_cell_bhp_jacobian(sim, wi_geom, injector, &cells[local_perf], bhp);
        jacobian[(row, 0)] = dbhp;
    }

    FimWellLocalSystem {
        residual,
        jacobian,
        perforation_indices,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fim::assembly::FimAssemblyOptions;
    use crate::fim::assembly_ad::assemble_fim_system_ad;
    use crate::fim::wells::build_well_topology;

    /// W1's core agreement test: the local system's rows/columns must be bit-identical to the
    /// corresponding slice of a full global assembly — not "close", not "structurally
    /// similar", exactly equal, since both are built from the same underlying function calls.
    /// This is what makes "wells converged in the local solve" mean, by construction, "well
    /// residual rows are zero in the next global assembly" (plan §2).
    /// The global assembler's `add_if_nonzero` (`assembly_ad.rs`) drops Jacobian entries with
    /// `|value| <= 1e-14` rather than storing them as explicit sparse zeros; the local (dense)
    /// Jacobian doesn't apply that same threshold. Use the same tolerance here so a value that
    /// legitimately rounds to ~1e-16 on one side and is absent (implicit 0.0) on the other
    /// doesn't read as a formula divergence — this is a sparse-storage convention difference,
    /// not a disagreement about the underlying math.
    fn assert_jacobian_entry_matches(local_value: f64, global_value: f64, context: &str) {
        assert!(
            (local_value - global_value).abs() < 1e-12,
            "{context}: local={local_value:e} global={global_value:e}"
        );
    }

    fn assert_local_system_matches_global(
        sim: &ReservoirSimulator,
        state: &FimState,
        topology: &FimWellTopology,
        well_idx: usize,
        label: &str,
    ) {
        let local = assemble_well_local_system(sim, state, topology, well_idx);

        let previous_state = state.clone();
        let options = FimAssemblyOptions {
            dt_days: 0.1,
            include_wells: true,
            assemble_residual_only: false,
            topology: Some(topology),
        };
        let global = assemble_fim_system_ad(sim, &previous_state, state, &options);

        let well_row = state.well_equation_offset(well_idx);
        let bhp_col = state.well_bhp_unknown_offset(well_idx);
        assert_eq!(
            local.residual[0], global.residual[well_row],
            "{label}: well_constraint residual diverges from global assembly"
        );
        assert_jacobian_entry_matches(
            local.jacobian[(0, 0)],
            global
                .jacobian
                .get(well_row, bhp_col)
                .copied()
                .unwrap_or(0.0),
            &format!("{label}: well_constraint d/dbhp"),
        );

        for (local_perf, &perf_idx) in local.perforation_indices.iter().enumerate() {
            let row = 1 + local_perf;
            let perf_row = state.perforation_equation_offset(perf_idx);
            let q_col = state.perforation_rate_unknown_offset(perf_idx);

            assert_eq!(
                local.residual[row], global.residual[perf_row],
                "{label}: perf {perf_idx} rate_consistency residual diverges from global assembly"
            );
            assert_jacobian_entry_matches(
                local.jacobian[(row, row)],
                global.jacobian.get(perf_row, q_col).copied().unwrap_or(0.0),
                &format!("{label}: perf {perf_idx} d/dq"),
            );
            assert_jacobian_entry_matches(
                local.jacobian[(row, 0)],
                global
                    .jacobian
                    .get(perf_row, bhp_col)
                    .copied()
                    .unwrap_or(0.0),
                &format!("{label}: perf {perf_idx} d/dbhp"),
            );
            assert_jacobian_entry_matches(
                local.jacobian[(0, row)],
                global.jacobian.get(well_row, q_col).copied().unwrap_or(0.0),
                &format!("{label}: well_constraint d/dq[{perf_idx}]"),
            );

            // A perforation's rate_consistency row never couples to another perforation's q
            // in the global assembly (each row only touches its own q_col, per
            // `add_well_jacobian_terms`'s `tri.add_triplet(perf_row, q_col, 1.0)`) — confirm
            // the local system doesn't invent a cross-coupling either.
            for (other_local_perf, &other_perf_idx) in local.perforation_indices.iter().enumerate()
            {
                if other_local_perf == local_perf {
                    continue;
                }
                assert_eq!(
                    local.jacobian[(row, 1 + other_local_perf)],
                    0.0,
                    "{label}: perf {perf_idx} rate_consistency row spuriously couples to perf {other_perf_idx}'s q"
                );
            }
        }
    }

    #[test]
    fn local_system_matches_global_assembly_bhp_controlled() {
        let mut sim = ReservoirSimulator::new(2, 1, 1, 0.2);
        sim.set_fim_enabled(true);
        sim.add_well(0, 0, 0, 500.0, 0.1, 0.0, true).unwrap();
        sim.add_well(1, 0, 0, 100.0, 0.1, 0.0, false).unwrap();

        let state = FimState::from_simulator(&sim);
        let topology = build_well_topology(&sim);
        for well_idx in 0..topology.wells.len() {
            assert_local_system_matches_global(&sim, &state, &topology, well_idx, "bhp_controlled");
        }
    }

    #[test]
    fn local_system_matches_global_assembly_rate_controlled() {
        let mut sim = ReservoirSimulator::new(2, 1, 1, 0.2);
        sim.set_fim_enabled(true);
        sim.set_injected_fluid("water").unwrap();
        sim.add_well(0, 0, 0, 400.0, 0.1, 0.0, true).unwrap();
        sim.add_well(1, 0, 0, 100.0, 0.1, 0.0, false).unwrap();
        sim.set_rate_controlled_wells(true);
        sim.set_target_well_surface_rates(30.0, 20.0).unwrap();
        sim.well_bhp_min = 50.0;
        sim.well_bhp_max = 500.0;

        let state = FimState::from_simulator(&sim);
        let topology = build_well_topology(&sim);
        for well_idx in 0..topology.wells.len() {
            assert_local_system_matches_global(
                &sim,
                &state,
                &topology,
                well_idx,
                "rate_controlled",
            );
        }
    }

    /// A non-converged state (`q` deliberately off from the connection-rate-consistent value)
    /// to make sure the agreement test isn't accidentally passing because both sides are
    /// evaluated near a trivial zero residual.
    #[test]
    fn local_system_matches_global_assembly_away_from_convergence() {
        let mut sim = ReservoirSimulator::new(2, 1, 1, 0.2);
        sim.set_fim_enabled(true);
        sim.add_well(0, 0, 0, 500.0, 0.1, 0.0, true).unwrap();
        sim.add_well(1, 0, 0, 100.0, 0.1, 0.0, false).unwrap();

        let mut state = FimState::from_simulator(&sim);
        for q in state.perforation_rates_m3_day.iter_mut() {
            *q += 500.0;
        }
        let topology = build_well_topology(&sim);
        for well_idx in 0..topology.wells.len() {
            assert_local_system_matches_global(
                &sim,
                &state,
                &topology,
                well_idx,
                "away_from_convergence",
            );
        }
    }

    #[test]
    fn assemble_well_local_system_dim_matches_perforation_count() {
        let mut sim = ReservoirSimulator::new(2, 1, 1, 0.2);
        sim.set_fim_enabled(true);
        sim.add_well(0, 0, 0, 500.0, 0.1, 0.0, true).unwrap();
        sim.add_well(1, 0, 0, 100.0, 0.1, 0.0, false).unwrap();

        let state = FimState::from_simulator(&sim);
        let topology = build_well_topology(&sim);
        for well_idx in 0..topology.wells.len() {
            let local = assemble_well_local_system(&sim, &state, &topology, well_idx);
            assert_eq!(
                local.dim(),
                2,
                "single-perforation well should have dim = 1 + 1"
            );
            assert_eq!(local.residual.len(), local.dim());
            assert_eq!(local.jacobian.nrows(), local.dim());
            assert_eq!(local.jacobian.ncols(), local.dim());
        }
    }
}
