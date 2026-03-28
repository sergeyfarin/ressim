use nalgebra::DVector;

use crate::fim::assembly::{assemble_fim_system, FimAssemblyOptions};
use crate::fim::linear::{
    solve_linearized_system, FimLinearBlockLayout, FimLinearSolveOptions, FimLinearSolveReport,
    FimLinearSolverKind,
};
use crate::fim::state::FimState;
use crate::fim::wells::build_well_topology;
use crate::ReservoirSimulator;

/// Diagnostic print macro — compiles to nothing on WASM, prints to stderr on native.
macro_rules! fim_trace {
    ($verbose:expr, $($arg:tt)*) => {
        #[cfg(not(target_arch = "wasm32"))]
        if $verbose {
            eprintln!($($arg)*);
        }
    };
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FimNewtonOptions {
    pub(crate) max_newton_iterations: usize,
    pub(crate) residual_tolerance: f64,
    pub(crate) update_tolerance: f64,
    pub(crate) min_damping: f64,
    pub(crate) max_pressure_change_bar: f64,
    pub(crate) max_saturation_change: f64,
    pub(crate) max_rs_change_fraction: f64,
    pub(crate) linear: FimLinearSolveOptions,
    pub(crate) verbose: bool,
}

impl Default for FimNewtonOptions {
    fn default() -> Self {
        Self {
            max_newton_iterations: 20,
            residual_tolerance: 1e-5,
            update_tolerance: 1e-3,
            min_damping: 1.0 / 64.0,
            // Generous limits — only prevent gross overshoot, not convergence-limiting.
            max_pressure_change_bar: 500.0,
            max_saturation_change: 0.5,
            max_rs_change_fraction: 1.0,
            linear: FimLinearSolveOptions::default(),
            verbose: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FimStepReport {
    pub(crate) accepted_state: FimState,
    pub(crate) converged: bool,
    pub(crate) newton_iterations: usize,
    pub(crate) final_residual_inf_norm: f64,
    pub(crate) final_update_inf_norm: f64,
    pub(crate) last_linear_report: Option<FimLinearSolveReport>,
    pub(crate) cutback_factor: f64,
}

/// Appleyard chop: compute the largest damping factor such that no cell variable
/// exceeds its per-iteration limit. Returns a value in (0, 1].
fn appleyard_damping(
    state: &FimState,
    update: &DVector<f64>,
    options: &FimNewtonOptions,
) -> f64 {
    let mut max_damping = 1.0_f64;
    let n_cells = state.cells.len();

    for idx in 0..n_cells {
        let offset = idx * 3;
        let cell = state.cell(idx);

        // Pressure
        let dp = update[offset].abs();
        if dp > 1e-12 {
            max_damping = max_damping.min(options.max_pressure_change_bar / dp);
        }

        // Water saturation
        let dsw = update[offset + 1].abs();
        if dsw > 1e-12 {
            max_damping = max_damping.min(options.max_saturation_change / dsw);
        }

        // Hydrocarbon variable (Sg or Rs)
        let dh = update[offset + 2].abs();
        if dh > 1e-12 {
            match cell.regime {
                crate::fim::state::HydrocarbonState::Saturated => {
                    // Sg: limit absolute change
                    max_damping = max_damping.min(options.max_saturation_change / dh);
                }
                crate::fim::state::HydrocarbonState::Undersaturated => {
                    // Rs: limit relative change
                    let rs_scale = cell.hydrocarbon_var.abs().max(1.0);
                    max_damping = max_damping.min(options.max_rs_change_fraction * rs_scale / dh);
                }
            }
        }
    }

    // Well BHP: same pressure limit
    let well_offset = state.n_cell_unknowns();
    for well_idx in 0..state.n_well_unknowns() {
        let dbhp = update[well_offset + well_idx].abs();
        if dbhp > 1e-12 {
            max_damping = max_damping.min(options.max_pressure_change_bar / dbhp);
        }
    }

    max_damping.clamp(options.min_damping, 1.0)
}

fn scaled_residual_inf_norm(
    residual: &DVector<f64>,
    scaling: &crate::fim::scaling::EquationScaling,
) -> f64 {
    let mut max_norm = 0.0_f64;
    let n_cells = scaling.water.len();

    for i in 0..n_cells {
        max_norm = max_norm.max(residual[i * 3].abs() / scaling.water[i]);
        max_norm = max_norm.max(residual[i * 3 + 1].abs() / scaling.oil_component[i]);
        max_norm = max_norm.max(residual[i * 3 + 2].abs() / scaling.gas_component[i]);
    }

    let mut offset = n_cells * 3;
    for i in 0..scaling.well_constraint.len() {
        max_norm = max_norm.max(residual[offset + i].abs() / scaling.well_constraint[i]);
    }
    offset += scaling.well_constraint.len();
    for i in 0..scaling.perforation_flow.len() {
        max_norm = max_norm.max(residual[offset + i].abs() / scaling.perforation_flow[i]);
    }

    max_norm
}

fn scaled_update_inf_norm(
    update: &DVector<f64>,
    scaling: &crate::fim::scaling::VariableScaling,
) -> f64 {
    let mut max_norm = 0.0_f64;
    let n_cells = scaling.pressure.len();

    for i in 0..n_cells {
        max_norm = max_norm.max(update[i * 3].abs() / scaling.pressure[i]);
        max_norm = max_norm.max(update[i * 3 + 1].abs() / scaling.sw[i]);
        max_norm = max_norm.max(update[i * 3 + 2].abs() / scaling.hydrocarbon_var[i]);
    }

    let mut offset = n_cells * 3;
    for i in 0..scaling.well_bhp.len() {
        max_norm = max_norm.max(update[offset + i].abs() / scaling.well_bhp[i]);
    }
    offset += scaling.well_bhp.len();
    for i in 0..scaling.perforation_rate.len() {
        max_norm = max_norm.max(update[offset + i].abs() / scaling.perforation_rate[i]);
    }

    max_norm
}

pub(crate) fn run_fim_timestep(
    sim: &ReservoirSimulator,
    previous_state: &FimState,
    initial_iterate: &FimState,
    dt_days: f64,
    options: &FimNewtonOptions,
) -> FimStepReport {
    let mut state = initial_iterate.clone();
    let mut last_linear_report = None;
    let mut final_residual_inf_norm: Option<f64>;
    let mut final_update_inf_norm = f64::INFINITY;
    let mut accepted_damping = 1.0;
    let mut prev_residual_norm = f64::INFINITY;
    let mut stagnation_count: u32 = 0;
    let block_layout = Some(FimLinearBlockLayout {
        cell_block_count: state.cells.len(),
        cell_block_size: 3,
        scalar_tail_start: state.n_cell_unknowns(),
    });
    let topology = build_well_topology(sim);

    fim_trace!(options.verbose, "  Newton: dt={:.6} days, n_cells={}, n_wells={}", dt_days, state.cells.len(), state.n_well_unknowns());

    for iteration in 0..options.max_newton_iterations {
        let assembly = assemble_fim_system(
            sim,
            previous_state,
            &state,
            &FimAssemblyOptions {
                dt_days,
                include_wells: true,
                topology: Some(&topology),
            },
        );
        final_residual_inf_norm = Some(scaled_residual_inf_norm(&assembly.residual, &assembly.equation_scaling));

        // Early termination: if residual is not decreasing, bail out to trigger timestep cut.
        let current_norm = final_residual_inf_norm.unwrap_or(f64::INFINITY);
        if iteration >= 2 && current_norm >= prev_residual_norm * 0.95 {
            stagnation_count += 1;
            if stagnation_count >= 3 {
                fim_trace!(options.verbose, "    iter {:>2}: STAGNATION (count={}) res={:.3e} — bailing out", iteration, stagnation_count, current_norm);
                return FimStepReport {
                    accepted_state: state,
                    converged: false,
                    newton_iterations: iteration + 1,
                    final_residual_inf_norm: current_norm,
                    final_update_inf_norm,
                    last_linear_report,
                    cutback_factor: 0.25,
                };
            }
        } else {
            stagnation_count = 0;
        }
        prev_residual_norm = current_norm;

        let rhs = -&assembly.residual;
        let mut linear_report = solve_linearized_system(
            &assembly.jacobian,
            &rhs,
            &options.linear,
            block_layout,
        );

        let mut used_fallback = false;
        if !linear_report.converged || !linear_report.solution.iter().all(|value| value.is_finite()) {
            fim_trace!(options.verbose, "    iter {:>2}: linear solver FAILED (converged={}, finite={}), trying fallback",
                iteration, linear_report.converged, linear_report.solution.iter().all(|v| v.is_finite()));
            let mut fallback_options = options.linear;
            fallback_options.kind = {
                #[cfg(target_arch = "wasm32")]
                {
                    FimLinearSolverKind::DenseLuDebug
                }
                #[cfg(not(target_arch = "wasm32"))]
                {
                    FimLinearSolverKind::SparseLuDebug
                }
            };
            linear_report = solve_linearized_system(&assembly.jacobian, &rhs, &fallback_options, block_layout);
            used_fallback = true;
        }

        final_update_inf_norm = scaled_update_inf_norm(&linear_report.solution, &assembly.variable_scaling);
        last_linear_report = Some(linear_report.clone());

        // Accept convergence if both residual and update are below tolerance,
        // OR if residual alone is very tight (handles well-switching oscillations
        // where the update stays "large" but the system is already well-balanced).
        let converged = (final_residual_inf_norm.unwrap_or(f64::INFINITY) <= options.residual_tolerance
            && final_update_inf_norm <= options.update_tolerance)
            || final_residual_inf_norm.unwrap_or(f64::INFINITY) <= options.residual_tolerance * 0.01;
        if converged {
            fim_trace!(options.verbose, "    iter {:>2}: CONVERGED res={:.3e} upd={:.3e} linear_iters={}{}",
                iteration, current_norm, final_update_inf_norm, linear_report.iterations,
                if used_fallback { " [fallback]" } else { "" });
            // Reclassify regimes now that Newton has converged with frozen regime map.
            state.classify_regimes(sim);
            return FimStepReport {
                accepted_state: state,
                converged: true,
                newton_iterations: iteration + 1,
                final_residual_inf_norm: final_residual_inf_norm.unwrap_or(f64::INFINITY),
                final_update_inf_norm,
                last_linear_report: Some(linear_report),
                cutback_factor: accepted_damping,
            };
        }

        let mut damping = appleyard_damping(&state, &linear_report.solution, options);
        let initial_damping = damping;
        let mut accepted_state = None;
        let mut damping_cuts = 0u32;
        while damping >= options.min_damping {
            let candidate = state.apply_newton_update_frozen(sim, &linear_report.solution, damping, &topology);
            if candidate.is_finite() && candidate.respects_basic_bounds(sim) {
                accepted_state = Some(candidate);
                break;
            }
            damping *= 0.5;
            damping_cuts += 1;
        }

        fim_trace!(options.verbose, "    iter {:>2}: res={:.3e} upd={:.3e} damp={:.4} (init={:.4}, cuts={}) linear_iters={}{}{}",
            iteration, current_norm, final_update_inf_norm, damping, initial_damping, damping_cuts,
            linear_report.iterations,
            if used_fallback { " [fallback]" } else { "" },
            if stagnation_count > 0 { format!(" stag={}", stagnation_count) } else { String::new() });

        let Some(candidate) = accepted_state else {
            fim_trace!(options.verbose, "    iter {:>2}: DAMPING FAILED — all candidates non-finite or out of bounds", iteration);
            return FimStepReport {
                accepted_state: state,
                converged: false,
                newton_iterations: iteration + 1,
                final_residual_inf_norm: final_residual_inf_norm.unwrap_or(f64::INFINITY),
                final_update_inf_norm,
                last_linear_report: Some(linear_report),
                cutback_factor: 0.5,
            };
        };

        accepted_damping = damping;
        state = candidate;
    }

    let final_assembly = assemble_fim_system(
        sim,
        previous_state,
        &state,
        &FimAssemblyOptions {
            dt_days,
            include_wells: true,
            topology: Some(&topology),
        },
    );
    final_residual_inf_norm = Some(scaled_residual_inf_norm(&final_assembly.residual, &final_assembly.equation_scaling));
    if final_residual_inf_norm.unwrap_or(f64::INFINITY) <= options.residual_tolerance {
        fim_trace!(options.verbose, "    post-loop: CONVERGED on final residual check res={:.3e}", final_residual_inf_norm.unwrap_or(f64::INFINITY));
        state.classify_regimes(sim);
        return FimStepReport {
            accepted_state: state,
            converged: true,
            newton_iterations: options.max_newton_iterations,
            final_residual_inf_norm: final_residual_inf_norm.unwrap_or(f64::INFINITY),
            final_update_inf_norm,
            last_linear_report,
            cutback_factor: accepted_damping,
        };
    }

    fim_trace!(options.verbose, "    post-loop: NOT CONVERGED after {} iterations, res={:.3e} upd={:.3e}",
        options.max_newton_iterations, final_residual_inf_norm.unwrap_or(f64::INFINITY), final_update_inf_norm);

    FimStepReport {
        accepted_state: state,
        converged: false,
        newton_iterations: options.max_newton_iterations,
        final_residual_inf_norm: final_residual_inf_norm.unwrap_or(f64::INFINITY),
        final_update_inf_norm,
        last_linear_report,
        cutback_factor: accepted_damping * 0.5,
    }
}

#[cfg(test)]
mod tests {
    use crate::fim::state::FimState;
    use crate::pvt::{PvtRow, PvtTable};
    use crate::ReservoirSimulator;

    use super::*;

    #[test]
    fn zero_residual_scaffold_converges_in_one_newton_step() {
        let sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        let state = FimState::from_simulator(&sim);

        let report = run_fim_timestep(&sim, &state, &state, 1.0, &FimNewtonOptions::default());

        assert!(report.converged);
        assert_eq!(report.newton_iterations, 1);
        assert_eq!(report.cutback_factor, 1.0);
        assert!(report.final_residual_inf_norm <= 1e-12);
        assert!(report.final_update_inf_norm <= 1e-12);
    }

    #[test]
    fn local_closed_system_newton_recovers_previous_state_from_perturbed_iterate() {
        let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        sim.set_three_phase_mode_enabled(true);
        sim.pvt_table = Some(PvtTable::new(
            vec![
                PvtRow {
                    p_bar: 100.0,
                    rs_m3m3: 10.0,
                    bo_m3m3: 1.1,
                    mu_o_cp: 1.2,
                    bg_m3m3: 0.01,
                    mu_g_cp: 0.02,
                },
                PvtRow {
                    p_bar: 200.0,
                    rs_m3m3: 20.0,
                    bo_m3m3: 1.0,
                    mu_o_cp: 1.1,
                    bg_m3m3: 0.005,
                    mu_g_cp: 0.02,
                },
            ],
            sim.pvt.c_o,
        ));
        let previous_state = FimState::from_simulator(&sim);
        let mut iterate = previous_state.clone();
        iterate.cells[0].pressure_bar += 5.0;
        iterate.cells[0].sw += 0.02;

        let report = run_fim_timestep(
            &sim,
            &previous_state,
            &iterate,
            1.0,
            &FimNewtonOptions::default(),
        );

        assert!(report.converged);
        assert!(
            (report.accepted_state.cells[0].pressure_bar - previous_state.cells[0].pressure_bar)
                .abs()
                < 0.5
        );
        assert!((report.accepted_state.cells[0].sw - previous_state.cells[0].sw).abs() < 1e-3);
    }

    #[test]
    fn rate_controlled_well_bhp_unknown_is_solved_implicitly() {
        let mut sim = ReservoirSimulator::new(2, 1, 1, 0.2);
        sim.set_rate_controlled_wells(true);
        sim.set_injected_fluid("water").unwrap();
        sim.add_well(0, 0, 0, 400.0, 0.1, 0.0, true).unwrap();
        sim.add_well(1, 0, 0, 50.0, 0.1, 0.0, false).unwrap();
        let previous_state = FimState::from_simulator(&sim);

        let report = run_fim_timestep(
            &sim,
            &previous_state,
            &previous_state,
            1.0,
            &FimNewtonOptions::default(),
        );

        assert!(report.converged);
        assert_eq!(report.accepted_state.well_bhp.len(), 2);
        assert_eq!(report.accepted_state.perforation_rates_m3_day.len(), 2);
    }
}
