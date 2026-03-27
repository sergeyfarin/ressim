use nalgebra::DVector;

use crate::fim::assembly::{assemble_fim_system, FimAssemblyOptions};
use crate::fim::linear::{solve_linearized_system, FimLinearSolveOptions, FimLinearSolveReport};
use crate::fim::state::FimState;
use crate::ReservoirSimulator;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FimNewtonOptions {
    pub(crate) max_newton_iterations: usize,
    pub(crate) residual_tolerance: f64,
    pub(crate) update_tolerance: f64,
    pub(crate) min_damping: f64,
    pub(crate) linear: FimLinearSolveOptions,
}

impl Default for FimNewtonOptions {
    fn default() -> Self {
        Self {
            max_newton_iterations: 12,
            residual_tolerance: 1e-6,
            update_tolerance: 1e-5,
            min_damping: 1.0 / 64.0,
            linear: FimLinearSolveOptions::default(),
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

fn scaled_residual_inf_norm(residual: &DVector<f64>) -> f64 {
    residual.iter().map(|value| value.abs()).fold(0.0, f64::max)
}

fn scaled_update_inf_norm(update: &DVector<f64>) -> f64 {
    update.iter().map(|value| value.abs()).fold(0.0, f64::max)
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

    for iteration in 0..options.max_newton_iterations {
        let assembly = assemble_fim_system(
            sim,
            previous_state,
            &state,
            &FimAssemblyOptions {
                dt_days,
                include_wells: true,
            },
        );
        final_residual_inf_norm = Some(scaled_residual_inf_norm(&assembly.residual));

        let rhs = -&assembly.residual;
        let linear_report = solve_linearized_system(&assembly.jacobian, &rhs, &options.linear);
        final_update_inf_norm = scaled_update_inf_norm(&linear_report.solution);
        last_linear_report = Some(linear_report.clone());

        if final_residual_inf_norm.unwrap_or(f64::INFINITY) <= options.residual_tolerance
            && final_update_inf_norm <= options.update_tolerance
        {
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

        let mut damping = 1.0;
        let mut accepted_state = None;
        while damping >= options.min_damping {
            let candidate = state.apply_newton_update(sim, &linear_report.solution, damping);
            if candidate.is_finite() && candidate.respects_basic_bounds(sim) {
                accepted_state = Some(candidate);
                break;
            }
            damping *= 0.5;
        }

        let Some(candidate) = accepted_state else {
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
        },
    );
    final_residual_inf_norm = Some(scaled_residual_inf_norm(&final_assembly.residual));
    if final_residual_inf_norm.unwrap_or(f64::INFINITY) <= options.residual_tolerance {
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
                < 1e-5
        );
        assert!((report.accepted_state.cells[0].sw - previous_state.cells[0].sw).abs() < 1e-6);
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
