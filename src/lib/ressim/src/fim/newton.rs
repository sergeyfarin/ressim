use nalgebra::DVector;

use crate::fim::assembly::{assemble_fim_system, FimAssemblyOptions};
use crate::fim::linear::{
    solve_linearized_system, FimLinearBlockLayout, FimLinearSolveOptions, FimLinearSolveReport,
    FimLinearSolverKind,
};
use crate::fim::state::FimState;
use crate::fim::wells::{build_well_topology, perforation_residual_diagnostics};
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

const ENTRY_RESIDUAL_GUARD_FACTOR: f64 = 2.0;

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

fn iterate_has_material_change(previous_state: &FimState, state: &FimState) -> bool {
    const PRESSURE_EPS: f64 = 1e-12;
    const SATURATION_EPS: f64 = 1e-12;
    const RS_EPS: f64 = 1e-12;
    const WELL_BHP_EPS: f64 = 1e-12;
    const PERF_RATE_EPS: f64 = 1e-12;

    previous_state
        .cells
        .iter()
        .zip(state.cells.iter())
        .any(|(previous, current)| {
            (current.pressure_bar - previous.pressure_bar).abs() > PRESSURE_EPS
                || (current.sw - previous.sw).abs() > SATURATION_EPS
                || (current.hydrocarbon_var - previous.hydrocarbon_var).abs() > RS_EPS
                || current.regime != previous.regime
        })
        || previous_state
            .well_bhp
            .iter()
            .zip(state.well_bhp.iter())
            .any(|(previous, current)| (current - previous).abs() > WELL_BHP_EPS)
        || previous_state
            .perforation_rates_m3_day
            .iter()
            .zip(state.perforation_rates_m3_day.iter())
            .any(|(previous, current)| (current - previous).abs() > PERF_RATE_EPS)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ResidualRowFamily {
    Water,
    OilComponent,
    GasComponent,
    WellConstraint,
    PerforationFlow,
}

impl ResidualRowFamily {
    fn label(self) -> &'static str {
        match self {
            Self::Water => "water",
            Self::OilComponent => "oil",
            Self::GasComponent => "gas",
            Self::WellConstraint => "well",
            Self::PerforationFlow => "perf",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ResidualFamilyPeak {
    family: ResidualRowFamily,
    scaled_value: f64,
    row: usize,
    item_index: usize,
}

#[derive(Clone, Debug, PartialEq)]
struct ResidualFamilyDiagnostics {
    water: ResidualFamilyPeak,
    oil_component: ResidualFamilyPeak,
    gas_component: ResidualFamilyPeak,
    well_constraint: Option<ResidualFamilyPeak>,
    perforation_flow: Option<ResidualFamilyPeak>,
    global: ResidualFamilyPeak,
}

fn update_family_peak(
    current: &mut Option<ResidualFamilyPeak>,
    family: ResidualRowFamily,
    scaled_value: f64,
    row: usize,
    item_index: usize,
) {
    let scaled_value = if scaled_value.is_finite() {
        scaled_value
    } else {
        f64::INFINITY
    };
    let candidate = ResidualFamilyPeak {
        family,
        scaled_value,
        row,
        item_index,
    };
    if current.is_none_or(|existing| candidate.scaled_value > existing.scaled_value) {
        *current = Some(candidate);
    }
}

fn residual_family_diagnostics(
    residual: &DVector<f64>,
    scaling: &crate::fim::scaling::EquationScaling,
) -> ResidualFamilyDiagnostics {
    let n_cells = scaling.water.len();
    let mut water = None;
    let mut oil_component = None;
    let mut gas_component = None;
    let mut well_constraint = None;
    let mut perforation_flow = None;

    for i in 0..n_cells {
        update_family_peak(
            &mut water,
            ResidualRowFamily::Water,
            residual[i * 3].abs() / scaling.water[i],
            i * 3,
            i,
        );
        update_family_peak(
            &mut oil_component,
            ResidualRowFamily::OilComponent,
            residual[i * 3 + 1].abs() / scaling.oil_component[i],
            i * 3 + 1,
            i,
        );
        update_family_peak(
            &mut gas_component,
            ResidualRowFamily::GasComponent,
            residual[i * 3 + 2].abs() / scaling.gas_component[i],
            i * 3 + 2,
            i,
        );
    }

    let mut offset = n_cells * 3;
    for i in 0..scaling.well_constraint.len() {
        update_family_peak(
            &mut well_constraint,
            ResidualRowFamily::WellConstraint,
            residual[offset + i].abs() / scaling.well_constraint[i],
            offset + i,
            i,
        );
    }
    offset += scaling.well_constraint.len();
    for i in 0..scaling.perforation_flow.len() {
        update_family_peak(
            &mut perforation_flow,
            ResidualRowFamily::PerforationFlow,
            residual[offset + i].abs() / scaling.perforation_flow[i],
            offset + i,
            i,
        );
    }

    let water = water.expect("residual diagnostics require at least one cell");
    let oil_component = oil_component.expect("residual diagnostics require at least one cell");
    let gas_component = gas_component.expect("residual diagnostics require at least one cell");
    let mut global = water;
    for peak in [Some(oil_component), Some(gas_component), well_constraint, perforation_flow]
        .into_iter()
        .flatten()
    {
        if peak.scaled_value > global.scaled_value {
            global = peak;
        }
    }

    ResidualFamilyDiagnostics {
        water,
        oil_component,
        gas_component,
        well_constraint,
        perforation_flow,
        global,
    }
}

fn residual_family_trace(diagnostics: &ResidualFamilyDiagnostics) -> String {
    let mut parts = vec![
        format!(
            "water={:.3e}@cell{}",
            diagnostics.water.scaled_value, diagnostics.water.item_index
        ),
        format!(
            "oil={:.3e}@cell{}",
            diagnostics.oil_component.scaled_value, diagnostics.oil_component.item_index
        ),
        format!(
            "gas={:.3e}@cell{}",
            diagnostics.gas_component.scaled_value, diagnostics.gas_component.item_index
        ),
    ];
    if let Some(peak) = diagnostics.well_constraint {
        parts.push(format!("well={:.3e}@well{}", peak.scaled_value, peak.item_index));
    }
    if let Some(peak) = diagnostics.perforation_flow {
        parts.push(format!("perf={:.3e}@perf{}", peak.scaled_value, peak.item_index));
    }
    parts.push(format!(
        "top={} row={} item={}",
        diagnostics.global.family.label(),
        diagnostics.global.row,
        diagnostics.global.item_index
    ));
    parts.join(" ")
}

fn residual_family_detail_trace(
    sim: &ReservoirSimulator,
    state: &FimState,
    topology: &crate::fim::wells::FimWellTopology,
    diagnostics: &ResidualFamilyDiagnostics,
) -> Option<String> {
    match diagnostics.global.family {
        ResidualRowFamily::PerforationFlow => {
            let detail = perforation_residual_diagnostics(sim, state, topology, diagnostics.global.item_index)?;
            let mut parts = vec![
                format!(
                    "perf{} well{} inj={} q={:.3e} conn={:.3e} raw={:.3e}",
                    detail.perf_idx,
                    detail.physical_well_idx,
                    detail.injector,
                    detail.q_unknown_m3_day,
                    detail.q_connection_m3_day,
                    detail.raw_connection_m3_day,
                ),
                format!(
                    "wi={:.3e} mob={:.3e} draw={:.3e} p={:.3} bhp={:.3}",
                    detail.well_index,
                    detail.connection_mobility,
                    detail.drawdown_bar,
                    detail.cell_pressure_bar,
                    detail.bhp_bar,
                ),
            ];
            if let Some(surface_rate) = detail.surface_rate_unknown_sc_day {
                parts.push(format!("surf_q={:.3e}", surface_rate));
            }
            if let Some(target_rate) = detail.target_rate_sc_day {
                parts.push(format!("target={:.3e}", target_rate));
            }
            if let Some(actual_rate) = detail.actual_well_rate_sc_day {
                parts.push(format!("well_rate={:.3e}", actual_rate));
            }
            if let Some(bhp_slack) = detail.bhp_slack {
                parts.push(format!("bhp_slack={:.3e}", bhp_slack));
            }
            if let Some(rate_slack) = detail.rate_slack {
                parts.push(format!("rate_slack={:.3e}", rate_slack));
            }
            if let Some(frozen_bhp) = detail.frozen_consistent_bhp_bar {
                parts.push(format!("frozen_bhp={:.3}", frozen_bhp));
            }
            if let Some(frozen_q) = detail.frozen_consistent_perf_rate_m3_day {
                parts.push(format!(
                    "frozen_q={:.3e} dq={:.3e}",
                    frozen_q,
                    detail.q_unknown_m3_day - frozen_q,
                ));
            }
            if let Some(frozen_rate) = detail.frozen_consistent_well_rate_sc_day {
                parts.push(format!("frozen_well_rate={:.3e}", frozen_rate));
            }
            if let Some(frozen_limited) = detail.frozen_consistent_bhp_limited {
                parts.push(format!("frozen_bhp_limited={}", frozen_limited));
            }
            Some(parts.join(" "))
        }
        _ => None,
    }
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
        let residual_diagnostics = residual_family_diagnostics(&assembly.residual, &assembly.equation_scaling);
        let residual_detail = residual_family_detail_trace(sim, &state, &topology, &residual_diagnostics);

        let current_norm = final_residual_inf_norm.unwrap_or(f64::INFINITY);
        let converged_on_entry = current_norm <= options.residual_tolerance
            || (iteration == 0
            && iterate_has_material_change(previous_state, &state)
                && current_norm <= options.residual_tolerance * ENTRY_RESIDUAL_GUARD_FACTOR);
        if converged_on_entry {
            final_update_inf_norm = 0.0;
            fim_trace!(options.verbose, "    iter {:>2}: CONVERGED on residual check res={:.3e}{} fam=[{}]{}",
                iteration, current_norm,
                if current_norm > options.residual_tolerance {
                    format!(" (entry guard {:.1}x)", ENTRY_RESIDUAL_GUARD_FACTOR)
                } else {
                    String::new()
                },
                residual_family_trace(&residual_diagnostics),
                residual_detail
                    .as_ref()
                    .map(|detail| format!(" detail=[{}]", detail))
                    .unwrap_or_default());
            state.classify_regimes(sim);
            return FimStepReport {
                accepted_state: state,
                converged: true,
                newton_iterations: iteration + 1,
                final_residual_inf_norm: current_norm,
                final_update_inf_norm,
                last_linear_report,
                cutback_factor: accepted_damping,
            };
        }

        // Early termination: if residual is not decreasing, bail out to trigger timestep cut.
        if iteration >= 2 && current_norm >= prev_residual_norm * 0.95 {
            stagnation_count += 1;
            if stagnation_count >= 3 {
                fim_trace!(options.verbose, "    iter {:>2}: STAGNATION (count={}) res={:.3e} fam=[{}]{} — bailing out",
                    iteration, stagnation_count, current_norm, residual_family_trace(&residual_diagnostics),
                    residual_detail
                        .as_ref()
                        .map(|detail| format!(" detail=[{}]", detail))
                        .unwrap_or_default());
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

        let converged = final_update_inf_norm <= options.update_tolerance
            && current_norm <= options.residual_tolerance * ENTRY_RESIDUAL_GUARD_FACTOR
            && iterate_has_material_change(previous_state, &state);
        if converged {
            fim_trace!(options.verbose, "    iter {:>2}: CONVERGED res={:.3e} upd={:.3e} linear_iters={}{} fam=[{}]{}",
                iteration, current_norm, final_update_inf_norm, linear_report.iterations,
                if used_fallback { " [fallback]" } else { "" }, residual_family_trace(&residual_diagnostics),
                residual_detail
                    .as_ref()
                    .map(|detail| format!(" detail=[{}]", detail))
                    .unwrap_or_default());
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
        let mut best_candidate_norm = f64::INFINITY;
        let mut damping_cuts = 0u32;
        while damping >= options.min_damping {
            let candidate = state.apply_newton_update_frozen(sim, &linear_report.solution, damping, &topology);
            if candidate.is_finite() && candidate.respects_basic_bounds(sim) {
                let candidate_assembly = assemble_fim_system(
                    sim,
                    previous_state,
                    &candidate,
                    &FimAssemblyOptions {
                        dt_days,
                        include_wells: true,
                        topology: Some(&topology),
                    },
                );
                let candidate_norm = scaled_residual_inf_norm(
                    &candidate_assembly.residual,
                    &candidate_assembly.equation_scaling,
                );
                if candidate_norm.is_finite() && candidate_norm < best_candidate_norm {
                    best_candidate_norm = candidate_norm;
                }
                if candidate_norm.is_finite() && candidate_norm < current_norm {
                    accepted_state = Some(candidate);
                    break;
                }
            }
            damping *= 0.5;
            damping_cuts += 1;
        }

        fim_trace!(options.verbose, "    iter {:>2}: res={:.3e} upd={:.3e} damp={:.4} (init={:.4}, cuts={}) cand_res={:.3e} linear_iters={}{}{} fam=[{}]{}",
            iteration, current_norm, final_update_inf_norm, damping, initial_damping, damping_cuts,
            best_candidate_norm,
            linear_report.iterations,
            if used_fallback { " [fallback]" } else { "" },
            if stagnation_count > 0 { format!(" stag={}", stagnation_count) } else { String::new() },
            residual_family_trace(&residual_diagnostics),
            residual_detail
                .as_ref()
                .map(|detail| format!(" detail=[{}]", detail))
                .unwrap_or_default());

        let Some(candidate) = accepted_state else {
            if current_norm <= options.residual_tolerance * ENTRY_RESIDUAL_GUARD_FACTOR
                && iterate_has_material_change(previous_state, &state)
            {
                final_update_inf_norm = 0.0;
                fim_trace!(options.verbose, "    iter {:>2}: CONVERGED after rejecting non-improving candidates res={:.3e}{} fam=[{}]{}",
                    iteration, current_norm,
                    if current_norm > options.residual_tolerance {
                        format!(" (guard {:.1}x)", ENTRY_RESIDUAL_GUARD_FACTOR)
                    } else {
                        String::new()
                    },
                    residual_family_trace(&residual_diagnostics),
                    residual_detail
                        .as_ref()
                        .map(|detail| format!(" detail=[{}]", detail))
                        .unwrap_or_default());
                state.classify_regimes(sim);
                return FimStepReport {
                    accepted_state: state,
                    converged: true,
                    newton_iterations: iteration + 1,
                    final_residual_inf_norm: current_norm,
                    final_update_inf_norm,
                    last_linear_report: Some(linear_report),
                    cutback_factor: accepted_damping,
                };
            }
            fim_trace!(options.verbose, "    iter {:>2}: DAMPING FAILED — no residual-reducing candidate (best={:.3e}, current={:.3e})", iteration, best_candidate_norm, current_norm);
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
    let final_residual_diagnostics = residual_family_diagnostics(&final_assembly.residual, &final_assembly.equation_scaling);
    let final_residual_detail = residual_family_detail_trace(sim, &state, &topology, &final_residual_diagnostics);
    if final_residual_inf_norm.unwrap_or(f64::INFINITY) <= options.residual_tolerance {
        fim_trace!(options.verbose, "    post-loop: CONVERGED on final residual check res={:.3e} fam=[{}]{}",
            final_residual_inf_norm.unwrap_or(f64::INFINITY), residual_family_trace(&final_residual_diagnostics),
            final_residual_detail
                .as_ref()
                .map(|detail| format!(" detail=[{}]", detail))
                .unwrap_or_default());
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

    fim_trace!(options.verbose, "    post-loop: NOT CONVERGED after {} iterations, res={:.3e} upd={:.3e} fam=[{}]{}",
        options.max_newton_iterations, final_residual_inf_norm.unwrap_or(f64::INFINITY), final_update_inf_norm,
        residual_family_trace(&final_residual_diagnostics),
        final_residual_detail
            .as_ref()
            .map(|detail| format!(" detail=[{}]", detail))
            .unwrap_or_default());

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
    use nalgebra::DVector;

    use crate::fim::assembly::{assemble_fim_system, FimAssemblyOptions};
    use crate::fim::scaling::EquationScaling;
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

    #[test]
    fn residual_tolerance_short_circuits_before_large_update_check() {
        let mut sim = ReservoirSimulator::new(2, 1, 1, 0.2);
        sim.set_rate_controlled_wells(true);
        sim.set_injected_fluid("water").unwrap();
        sim.add_well(0, 0, 0, 400.0, 0.1, 0.0, true).unwrap();
        sim.add_well(1, 0, 0, 50.0, 0.1, 0.0, false).unwrap();
        let previous_state = FimState::from_simulator(&sim);
        let mut iterate = previous_state.clone();
        iterate.well_bhp[0] += 1e-3;

        let assembly = assemble_fim_system(
            &sim,
            &previous_state,
            &iterate,
            &FimAssemblyOptions {
                dt_days: 1.0,
                include_wells: true,
                topology: None,
            },
        );
        let residual_norm = scaled_residual_inf_norm(&assembly.residual, &assembly.equation_scaling);
        assert!(residual_norm.is_finite() && residual_norm > 0.0);

        let options = FimNewtonOptions {
            residual_tolerance: residual_norm * 0.75,
            update_tolerance: -1.0,
            ..FimNewtonOptions::default()
        };

        let report = run_fim_timestep(
            &sim,
            &previous_state,
            &iterate,
            1.0,
            &options,
        );

        assert!(report.converged);
        assert_eq!(report.newton_iterations, 1);
        assert!((report.accepted_state.well_bhp[0] - iterate.well_bhp[0]).abs() < 1e-15);
        assert_eq!(report.final_update_inf_norm, 0.0);
    }

    #[test]
    fn entry_guard_does_not_accept_unchanged_previous_state() {
        let mut sim = ReservoirSimulator::new(2, 1, 1, 0.2);
        sim.set_rate_controlled_wells(true);
        sim.set_injected_fluid("water").unwrap();
        sim.add_well(0, 0, 0, 400.0, 0.1, 0.0, true).unwrap();
        sim.add_well(1, 0, 0, 50.0, 0.1, 0.0, false).unwrap();
        let previous_state = FimState::from_simulator(&sim);

        let assembly = assemble_fim_system(
            &sim,
            &previous_state,
            &previous_state,
            &FimAssemblyOptions {
                dt_days: 0.01,
                include_wells: true,
                topology: None,
            },
        );
        let residual_norm = scaled_residual_inf_norm(&assembly.residual, &assembly.equation_scaling);
        assert!(residual_norm.is_finite() && residual_norm > 0.0);

        let options = FimNewtonOptions {
            residual_tolerance: residual_norm * 0.75,
            ..FimNewtonOptions::default()
        };

        let report = run_fim_timestep(&sim, &previous_state, &previous_state, 0.01, &options);

        assert!(
            !report.converged || iterate_has_material_change(&previous_state, &report.accepted_state),
            "unchanged previous state must not be accepted as converged inside the residual guard band"
        );
        if report.converged {
            assert!(
                report.final_update_inf_norm > 0.0,
                "guarded residual acceptance should not report a zero-update shortcut for an unchanged previous state"
            );
        }
    }

    #[test]
    fn residual_family_diagnostics_reports_global_peak_family() {
        let residual = DVector::from_vec(vec![5.0, 12.0, 8.0, 4.0, 9.0, 6.0, 3.0, 40.0, 1.0]);
        let scaling = EquationScaling {
            water: vec![10.0, 10.0],
            oil_component: vec![10.0, 10.0],
            gas_component: vec![10.0, 10.0],
            well_constraint: vec![10.0, 5.0],
            perforation_flow: vec![2.0],
        };

        let diagnostics = residual_family_diagnostics(&residual, &scaling);

        assert_eq!(diagnostics.water.item_index, 0);
        assert!((diagnostics.water.scaled_value - 0.5).abs() < 1e-12);
        assert_eq!(diagnostics.oil_component.item_index, 0);
        assert!((diagnostics.oil_component.scaled_value - 1.2).abs() < 1e-12);
        assert_eq!(diagnostics.gas_component.item_index, 0);
        assert!((diagnostics.gas_component.scaled_value - 0.8).abs() < 1e-12);
        assert_eq!(diagnostics.well_constraint.expect("well peak").item_index, 1);
        assert!((diagnostics.well_constraint.expect("well peak").scaled_value - 8.0).abs() < 1e-12);
        assert_eq!(diagnostics.perforation_flow.expect("perf peak").item_index, 0);
        assert!((diagnostics.perforation_flow.expect("perf peak").scaled_value - 0.5).abs() < 1e-12);
        assert_eq!(diagnostics.global.family, ResidualRowFamily::WellConstraint);
        assert_eq!(diagnostics.global.row, 7);
        assert_eq!(diagnostics.global.item_index, 1);
        assert!((diagnostics.global.scaled_value - 8.0).abs() < 1e-12);
    }
}
