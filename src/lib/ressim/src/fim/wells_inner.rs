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

/// OPM-verified inner-solve budget/tolerance/chop defaults (`docs/FIM_BUNDLE_W_PLAN.md` W0
/// appendix H, `BlackoilModelParameters.hpp`): `MaxInnerIterWells` = 50, `ToleranceWells` =
/// 1e-4, `DbhpMaxRel` = 1.0.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FimWellInnerSolveOptions {
    pub(crate) max_iterations: usize,
    pub(crate) tolerance: f64,
    pub(crate) dbhp_max_rel: f64,
}

impl Default for FimWellInnerSolveOptions {
    fn default() -> Self {
        Self {
            max_iterations: 50,
            tolerance: 1e-4,
            dbhp_max_rel: 1.0,
        }
    }
}

/// Result of converging one physical well's local system.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FimWellInnerSolveReport {
    pub(crate) well_idx: usize,
    pub(crate) converged: bool,
    pub(crate) iterations: usize,
    pub(crate) final_scaled_residual_peak: f64,
}

/// `StandardWellPrimaryVariables::updateNewton`'s BHP lower limit (W0 appendix D): "some cases
/// might have defaulted bhp constraint of 1 bar, we use a slightly smaller value... so that bhp
/// constraint can be an active control when needed" — `1 bar - 1 Pa`.
const BHP_LOWER_LIMIT_BAR: f64 = 1.0 - 1.0e-5;

/// OPM's `dbhp-max-rel` chop (W0 appendix D, `StandardWellPrimaryVariables.cpp:262`): the BHP
/// update magnitude is capped at `dbhp_max_rel * |current bhp|`, then floored at
/// `BHP_LOWER_LIMIT_BAR`. `raw_delta_bhp` is the *signed* proposed increment
/// (`bhp_new = bhp + raw_delta_bhp`) — algebraically identical to OPM's own
/// subtract-a-signed-magnitude form, just consistent with this module's `delta` sign
/// convention (`jacobian * delta = -residual`).
fn chop_bhp_update(bhp: f64, raw_delta_bhp: f64, dbhp_max_rel: f64) -> f64 {
    let cap = bhp.abs() * dbhp_max_rel;
    let delta_limited = raw_delta_bhp.clamp(-cap, cap);
    (bhp + delta_limited).max(BHP_LOWER_LIMIT_BAR)
}

/// OPM's flow-direction sign check (W0 appendix C, `StandardWellEval.cpp`'s
/// `WrongFlowDirection` failure inside `getWellConvergence`), scoped to pressure-controlled
/// (non-rate-controlled) wells only — matches OPM's `isPressureControlled` gating exactly.
/// Applied per-perforation since ResSim has no single aggregate `WQTotal` unknown the way OPM's
/// `StandardWell` does (see plan §0 appendix E). ResSim's own sign convention (confirmed via
/// `relax_well_state_toward_local_consistency`'s prior clamp and the `FIM-DIAG-002` trace
/// data): injector perforation rates are `<= 0`, producer rates are `>= 0`.
fn perforation_flow_direction_ok(injector: bool, q: f64) -> bool {
    if injector { q <= 0.0 } else { q >= 0.0 }
}

fn well_constraint_scale_for(
    sim: &ReservoirSimulator,
    state: &FimState,
    topology: &FimWellTopology,
    well_idx: usize,
    control_real: &crate::fim::wells::PhysicalWellControl,
) -> f64 {
    let control_slacks = if control_real.rate_controlled {
        crate::fim::wells::well_local_block(topology, state, well_idx).control_slacks(sim)
    } else {
        None
    };
    crate::fim::scaling::well_constraint_scale(state.well_bhp[well_idx], control_slacks)
}

/// One well's scaled-residual-peak + flow-direction convergence status at its *current* local
/// system — the single formula shared by `solve_well_locally`'s per-iteration loop check and
/// the standalone `well_is_converged`/`all_wells_converged` outer-criteria check (plan §5 item
/// 3: "inner converged" and "outer sees zero" must be the same statement, not two hand-matched
/// copies of the same test).
struct FimWellConvergenceStatus {
    converged: bool,
    scaled_residual_peak: f64,
}

fn well_convergence_status(
    sim: &ReservoirSimulator,
    state: &FimState,
    topology: &FimWellTopology,
    well_idx: usize,
    control_real: &crate::fim::wells::PhysicalWellControl,
    local: &FimWellLocalSystem,
    options: &FimWellInnerSolveOptions,
) -> FimWellConvergenceStatus {
    let injector = topology.wells[well_idx].injector;
    let pressure_controlled = !control_real.rate_controlled;

    let well_scale = well_constraint_scale_for(sim, state, topology, well_idx, control_real);
    let mut scaled_peak = local.residual[0].abs() / well_scale;
    for (local_perf, &perf_idx) in local.perforation_indices.iter().enumerate() {
        let perf_scale =
            crate::fim::scaling::perforation_flow_scale(state.perforation_rates_m3_day[perf_idx]);
        scaled_peak = scaled_peak.max(local.residual[1 + local_perf].abs() / perf_scale);
    }

    let direction_ok = !pressure_controlled
        || local.perforation_indices.iter().all(|&perf_idx| {
            perforation_flow_direction_ok(injector, state.perforation_rates_m3_day[perf_idx])
        });

    FimWellConvergenceStatus {
        converged: scaled_peak <= options.tolerance && direction_ok,
        scaled_residual_peak: scaled_peak,
    }
}

/// Converges one physical well's local system (`assemble_well_local_system`) via a bounded,
/// chopped Newton loop, mutating `state.well_bhp[well_idx]` / `state.perforation_rates_m3_day`
/// in place — same call shape as `relax_well_state_toward_local_consistency`, so this is a
/// drop-in replacement at that call site (`docs/FIM_BUNDLE_W_PLAN.md` §5 item 1). Convergence
/// uses the identical `EquationScaling` family-scale formulas (`fim/scaling.rs`) the global
/// assembly's convergence test uses, so "inner converged" and "outer sees zero" are the same
/// statement (plan §5 item 2). On a singular local Jacobian or exhausted budget, the last
/// iterate is kept and `converged: false` is reported — the caller's outer retry ladder handles
/// it; this function never widens acceptance to paper over an inner failure (`FIM-NEWTON-005`
/// lesson).
pub(crate) fn solve_well_locally(
    sim: &ReservoirSimulator,
    state: &mut FimState,
    topology: &FimWellTopology,
    well_idx: usize,
    options: &FimWellInnerSolveOptions,
) -> FimWellInnerSolveReport {
    let control_real = physical_well_control(sim, topology, well_idx);

    let mut iterations = 0usize;
    let mut converged: bool;
    let mut final_scaled_residual_peak: f64;

    loop {
        let local = assemble_well_local_system(sim, state, topology, well_idx);
        let status = well_convergence_status(
            sim,
            state,
            topology,
            well_idx,
            &control_real,
            &local,
            options,
        );
        converged = status.converged;
        final_scaled_residual_peak = status.scaled_residual_peak;

        if converged || iterations >= options.max_iterations {
            break;
        }

        let neg_residual = -local.residual.clone();
        let Some(delta) = local.jacobian.lu().solve(&neg_residual) else {
            // Singular local Jacobian: cannot proceed. Keep the last iterate, report
            // not-converged (see doc comment above — do not paper over this).
            break;
        };

        state.well_bhp[well_idx] =
            chop_bhp_update(state.well_bhp[well_idx], delta[0], options.dbhp_max_rel);
        for (local_perf, &perf_idx) in local.perforation_indices.iter().enumerate() {
            // No magnitude clamp on q, matching OPM's WQTotal update (W0 appendix D).
            state.perforation_rates_m3_day[perf_idx] += delta[1 + local_perf];
        }

        iterations += 1;
    }

    FimWellInnerSolveReport {
        well_idx,
        converged,
        iterations,
        final_scaled_residual_peak,
    }
}

/// Converges every physical well's local system, in well-index order. Same aggregate call
/// shape as the code it will replace at the `apply_raw_update` site (plan §5 item 1) — one call
/// covers all wells for a candidate state.
pub(crate) fn solve_wells_locally(
    sim: &ReservoirSimulator,
    state: &mut FimState,
    topology: &FimWellTopology,
    options: &FimWellInnerSolveOptions,
) -> Vec<FimWellInnerSolveReport> {
    (0..topology.wells.len())
        .map(|well_idx| solve_well_locally(sim, state, topology, well_idx, options))
        .collect()
}

/// Read-only check: is this one well converged at `state` as it stands, with no update applied?
/// A single `assemble_well_local_system` call plus the shared `well_convergence_status` test —
/// this is the outer-criteria counterpart of OPM's `getWellConvergence`
/// (`docs/FIM_BUNDLE_W_PLAN.md` W0 appendix G), a pure convergence *test*, not a solve.
pub(crate) fn well_is_converged(
    sim: &ReservoirSimulator,
    state: &FimState,
    topology: &FimWellTopology,
    well_idx: usize,
    options: &FimWellInnerSolveOptions,
) -> bool {
    let control_real = physical_well_control(sim, topology, well_idx);
    let local = assemble_well_local_system(sim, state, topology, well_idx);
    well_convergence_status(
        sim,
        state,
        topology,
        well_idx,
        &control_real,
        &local,
        options,
    )
    .converged
}

/// Read-only check across every physical well — the `OpmAligned` outer-criteria analog of
/// OPM's `getWellConvergence` (plan §5 item 3), used to gate acceptance when `nested_well_solve`
/// is enabled.
pub(crate) fn all_wells_converged(
    sim: &ReservoirSimulator,
    state: &FimState,
    topology: &FimWellTopology,
    options: &FimWellInnerSolveOptions,
) -> bool {
    (0..topology.wells.len())
        .all(|well_idx| well_is_converged(sim, state, topology, well_idx, options))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fim::assembly::FimAssemblyOptions;
    use crate::fim::assembly_ad::assemble_fim_system_ad;
    use crate::fim::wells::{build_well_topology, well_local_block};

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
            flow_resv_context: None,
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

    // --- W2: inner Newton loop tests ---

    #[test]
    fn chop_bhp_update_caps_at_dbhp_max_rel_and_floors_at_lower_limit() {
        // Within cap: applied as-is.
        assert!((chop_bhp_update(200.0, 10.0, 1.0) - 210.0).abs() < 1e-12);
        // Exceeds cap (100% of |bhp|=200 -> cap=200): clamped to +200, not the raw +500.
        assert!((chop_bhp_update(200.0, 500.0, 1.0) - 400.0).abs() < 1e-12);
        // Negative direction, same cap logic: 200 - cap(200) = 0.0, which is then floored at
        // BHP_LOWER_LIMIT_BAR (the floor applies here too, not just to extreme inputs).
        assert!((chop_bhp_update(200.0, -500.0, 1.0) - BHP_LOWER_LIMIT_BAR).abs() < 1e-12);
        // Tighter relative cap (10%).
        assert!((chop_bhp_update(200.0, 500.0, 0.1) - 220.0).abs() < 1e-12);
        // Floored at BHP_LOWER_LIMIT_BAR even when the chopped step would go lower.
        assert!((chop_bhp_update(0.5, -100.0, 1.0) - BHP_LOWER_LIMIT_BAR).abs() < 1e-12);
    }

    #[test]
    fn perforation_flow_direction_ok_matches_ressim_sign_convention() {
        assert!(perforation_flow_direction_ok(true, -100.0));
        assert!(perforation_flow_direction_ok(true, 0.0));
        assert!(!perforation_flow_direction_ok(true, 100.0));
        assert!(perforation_flow_direction_ok(false, 100.0));
        assert!(perforation_flow_direction_ok(false, 0.0));
        assert!(!perforation_flow_direction_ok(false, -100.0));
    }

    /// Plan §2's closed-form observation from W1, now exercised end-to-end: a BHP-controlled
    /// well's local system has `well_constraint` (no `q` dependence) and `rate_consistency`
    /// (linear identity in `q`, `connection_rate_generic` doesn't depend on `q`) — so once `bhp`
    /// is at target, `q` has an exact one-shot solution. Starting from a state perturbed away
    /// from consistency (not the already-near-consistent `FimState::from_simulator` output),
    /// this should converge in very few iterations.
    #[test]
    fn bhp_controlled_well_converges_from_perturbed_state() {
        let mut sim = ReservoirSimulator::new(2, 1, 1, 0.2);
        sim.set_fim_enabled(true);
        sim.add_well(0, 0, 0, 500.0, 0.1, 0.0, true).unwrap();
        sim.add_well(1, 0, 0, 100.0, 0.1, 0.0, false).unwrap();

        let mut state = FimState::from_simulator(&sim);
        for q in state.perforation_rates_m3_day.iter_mut() {
            *q += 800.0;
        }
        let topology = build_well_topology(&sim);
        let options = FimWellInnerSolveOptions::default();

        for well_idx in 0..topology.wells.len() {
            let bhp_target = physical_well_control(&sim, &topology, well_idx).bhp_target;
            let report = solve_well_locally(&sim, &mut state, &topology, well_idx, &options);
            assert!(
                report.converged,
                "well {well_idx} failed to converge: {report:?}"
            );
            // Empirically exactly 1 iteration (residual ~1e-16, machine epsilon) — the W1
            // closed-form observation confirmed, not just bounded: `connection_rate_generic`
            // doesn't depend on `q`, so once `bhp` is pinned at target the single Newton step
            // lands exactly on the closed-form solution.
            assert!(
                report.iterations <= 3,
                "well {well_idx} took {} iterations, expected the closed-form case to be fast",
                report.iterations
            );
            assert!(
                (state.well_bhp[well_idx] - bhp_target).abs() < 1e-9,
                "well {well_idx} bhp should stay pinned at target"
            );

            // The converged local system's own residual must actually be at/under tolerance —
            // don't just trust the report, recheck independently.
            let local = assemble_well_local_system(&sim, &state, &topology, well_idx);
            assert!(
                local.residual.iter().all(|r| r.abs() < 1e-6),
                "well {well_idx} local residual not actually small: {:?}",
                local.residual
            );
        }
    }

    /// Rate-controlled (Fischer-Burmeister) well: genuinely nonlinear, unlike the BHP-controlled
    /// case above. Converges and lands on a slack-feasible (bhp, q): either bhp is at its limit,
    /// or the aggregate rate matches its target (within the well's own convergence tolerance).
    #[test]
    fn rate_controlled_well_converges_to_slack_feasible_state() {
        let mut sim = ReservoirSimulator::new(2, 1, 1, 0.2);
        sim.set_fim_enabled(true);
        sim.set_injected_fluid("water").unwrap();
        sim.add_well(0, 0, 0, 400.0, 0.1, 0.0, true).unwrap();
        sim.add_well(1, 0, 0, 100.0, 0.1, 0.0, false).unwrap();
        sim.set_rate_controlled_wells(true);
        sim.set_target_well_surface_rates(30.0, 20.0).unwrap();
        sim.well_bhp_min = 50.0;
        sim.well_bhp_max = 500.0;

        let mut state = FimState::from_simulator(&sim);
        for q in state.perforation_rates_m3_day.iter_mut() {
            *q *= 0.3;
        }
        let topology = build_well_topology(&sim);
        let options = FimWellInnerSolveOptions::default();

        for well_idx in 0..topology.wells.len() {
            let report = solve_well_locally(&sim, &mut state, &topology, well_idx, &options);
            assert!(
                report.converged,
                "well {well_idx} failed to converge: {report:?}"
            );

            let block = well_local_block(&topology, &state, well_idx);
            let control = block.control(&sim);
            let (bhp_slack, rate_slack) = block
                .control_slacks(&sim)
                .expect("rate-controlled well should report slacks");
            let bhp_scale = control.bhp_limit.abs().max(1.0);
            let rate_scale = control.target_rate.unwrap_or(1.0).abs().max(1.0);
            assert!(
                bhp_slack / bhp_scale > -1e-3 || rate_slack / rate_scale > -1e-3,
                "well {well_idx} converged to an infeasible (bhp,q): bhp_slack={bhp_slack} rate_slack={rate_slack}"
            );
        }
    }

    /// A zero-iteration budget can never converge (unless already exactly at the residual on
    /// entry) — used here as a deterministic way to exercise the exhausted-budget path without
    /// panicking, matching the plan's "deliberately infeasible case reports non-convergence"
    /// requirement without needing a contrived physically-infeasible scenario.
    #[test]
    fn exhausted_budget_reports_not_converged_without_panicking() {
        let mut sim = ReservoirSimulator::new(2, 1, 1, 0.2);
        sim.set_fim_enabled(true);
        sim.add_well(0, 0, 0, 500.0, 0.1, 0.0, true).unwrap();
        sim.add_well(1, 0, 0, 100.0, 0.1, 0.0, false).unwrap();

        let mut state = FimState::from_simulator(&sim);
        for q in state.perforation_rates_m3_day.iter_mut() {
            *q += 800.0;
        }
        let topology = build_well_topology(&sim);
        let options = FimWellInnerSolveOptions {
            max_iterations: 0,
            ..FimWellInnerSolveOptions::default()
        };

        let report = solve_well_locally(&sim, &mut state, &topology, 1, &options);
        assert!(!report.converged);
        assert_eq!(report.iterations, 0);
        assert!(report.final_scaled_residual_peak.is_finite());
    }

    #[test]
    fn solve_wells_locally_covers_every_well() {
        let mut sim = ReservoirSimulator::new(2, 1, 1, 0.2);
        sim.set_fim_enabled(true);
        sim.add_well(0, 0, 0, 500.0, 0.1, 0.0, true).unwrap();
        sim.add_well(1, 0, 0, 100.0, 0.1, 0.0, false).unwrap();

        let mut state = FimState::from_simulator(&sim);
        for q in state.perforation_rates_m3_day.iter_mut() {
            *q += 800.0;
        }
        let topology = build_well_topology(&sim);
        let options = FimWellInnerSolveOptions::default();

        let reports = solve_wells_locally(&sim, &mut state, &topology, &options);
        assert_eq!(reports.len(), topology.wells.len());
        assert!(reports.iter().all(|r| r.converged));
        for (well_idx, report) in reports.iter().enumerate() {
            assert_eq!(report.well_idx, well_idx);
        }
    }

    /// `well_is_converged`/`all_wells_converged` must agree with `solve_well_locally`'s own
    /// convergence verdict (the shared `well_convergence_status` formula is the whole point of
    /// plan §5 item 3: "inner converged" and "outer sees zero" are the same statement).
    #[test]
    fn well_is_converged_matches_solve_result_before_and_after() {
        let mut sim = ReservoirSimulator::new(2, 1, 1, 0.2);
        sim.set_fim_enabled(true);
        sim.add_well(0, 0, 0, 500.0, 0.1, 0.0, true).unwrap();
        sim.add_well(1, 0, 0, 100.0, 0.1, 0.0, false).unwrap();

        let mut state = FimState::from_simulator(&sim);
        for q in state.perforation_rates_m3_day.iter_mut() {
            *q += 800.0;
        }
        let topology = build_well_topology(&sim);
        let options = FimWellInnerSolveOptions::default();

        assert!(
            !all_wells_converged(&sim, &state, &topology, &options),
            "perturbed state should not read as converged before solving"
        );
        for well_idx in 0..topology.wells.len() {
            assert!(!well_is_converged(
                &sim, &state, &topology, well_idx, &options
            ));
        }

        for well_idx in 0..topology.wells.len() {
            let report = solve_well_locally(&sim, &mut state, &topology, well_idx, &options);
            assert!(report.converged);
            assert_eq!(
                well_is_converged(&sim, &state, &topology, well_idx, &options),
                report.converged,
                "well {well_idx}: read-only check must agree with the solve's own verdict"
            );
        }
        assert!(all_wells_converged(&sim, &state, &topology, &options));
    }

    #[test]
    fn all_wells_converged_requires_every_well() {
        let mut sim = ReservoirSimulator::new(2, 1, 1, 0.2);
        sim.set_fim_enabled(true);
        sim.add_well(0, 0, 0, 500.0, 0.1, 0.0, true).unwrap();
        sim.add_well(1, 0, 0, 100.0, 0.1, 0.0, false).unwrap();

        let mut state = FimState::from_simulator(&sim);
        for q in state.perforation_rates_m3_day.iter_mut() {
            *q += 800.0;
        }
        let topology = build_well_topology(&sim);
        let options = FimWellInnerSolveOptions::default();

        // Converge only well 0, leave well 1 perturbed.
        let report = solve_well_locally(&sim, &mut state, &topology, 0, &options);
        assert!(report.converged);
        assert!(well_is_converged(&sim, &state, &topology, 0, &options));
        assert!(!well_is_converged(&sim, &state, &topology, 1, &options));
        assert!(
            !all_wells_converged(&sim, &state, &topology, &options),
            "one converged well should not make the aggregate check pass"
        );
    }
}
