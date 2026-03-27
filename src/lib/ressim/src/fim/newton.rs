use nalgebra::DVector;

use crate::fim::assembly::{assemble_fim_system, FimAssemblyOptions};
use crate::fim::linear::{
    solve_linearized_system, FimLinearSolveOptions, FimLinearSolveReport,
};
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
    initial_state: &FimState,
    dt_days: f64,
    options: &FimNewtonOptions,
) -> FimStepReport {
    let mut state = initial_state.clone();
    let mut last_linear_report = None;
    let mut final_residual_inf_norm = f64::INFINITY;
    let mut final_update_inf_norm = f64::INFINITY;

    for iteration in 0..options.max_newton_iterations {
        let assembly = assemble_fim_system(
            sim,
            &state,
            &FimAssemblyOptions {
                dt_days,
                include_wells: true,
            },
        );
        final_residual_inf_norm = scaled_residual_inf_norm(&assembly.residual);

        let rhs = -&assembly.residual;
        let linear_report = solve_linearized_system(&assembly.jacobian, &rhs, &options.linear);
        final_update_inf_norm = scaled_update_inf_norm(&linear_report.solution);
        last_linear_report = Some(linear_report.clone());

        // Temporary scaffold: once real assembly and state-update logic land, the
        // Newton step will apply the damped update to `state`. For the current zero
        // residual scaffold, convergence is already determined by the assembled system.
        if final_residual_inf_norm <= options.residual_tolerance
            && final_update_inf_norm <= options.update_tolerance
        {
            return FimStepReport {
                accepted_state: state,
                converged: true,
                newton_iterations: iteration + 1,
                final_residual_inf_norm,
                final_update_inf_norm,
                last_linear_report: Some(linear_report),
                cutback_factor: 1.0,
            };
        }
    }

    FimStepReport {
        accepted_state: state,
        converged: false,
        newton_iterations: options.max_newton_iterations,
        final_residual_inf_norm,
        final_update_inf_norm,
        last_linear_report,
        cutback_factor: 0.5,
    }
}

#[cfg(test)]
mod tests {
    use crate::fim::state::FimState;
    use crate::ReservoirSimulator;

    use super::*;

    #[test]
    fn zero_residual_scaffold_converges_in_one_newton_step() {
        let sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        let state = FimState::from_simulator(&sim);

        let report = run_fim_timestep(&sim, &state, 1.0, &FimNewtonOptions::default());

        assert!(report.converged);
        assert_eq!(report.newton_iterations, 1);
        assert_eq!(report.cutback_factor, 1.0);
        assert!(report.final_residual_inf_norm <= 1e-12);
        assert!(report.final_update_inf_norm <= 1e-12);
    }
}