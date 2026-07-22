//! G4b0's inert Flow gas-RESV injector context.
//!
//! This module captures only the report-step reference state and validates the narrow supported
//! scope. It does not route an assembler or reinterpret a FIM rate unknown; those coupled changes
//! belong to G4b1/G4b2.

use crate::fim::ad::Scalar;
use crate::fim::state::FimState;
use crate::fim::wells::{FimWellTopology, build_well_topology};
use crate::well::WellScheduleControl;
use crate::{InjectedFluid, ReservoirSimulator};

/// Immutable conversion state corresponding to Flow RateConverter defineState for the one-region
/// gas-RESV probe. It is retained across Newton retries and replaced only after acceptance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FlowResvReference {
    pub(crate) hydrocarbon_pore_volume_m3: f64,
    pub(crate) pressure_bar: f64,
    pub(crate) bg_rm3_per_sm3: f64,
}

/// Immutable report-step context carried by FimNewtonOptions.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FlowResvReportStepContext {
    pub(crate) reference: FlowResvReference,
    pub(crate) physical_well_idx: usize,
    pub(crate) perforation_idx: usize,
    pub(crate) reservoir_target_rm3_day: f64,
}

/// The one place that selects the default-off RESV well formulation.  Keeping this separate
/// from `physical_well_control()` prevents a recognized RESV schedule from silently using the
/// historical BHP/q branch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum FimWellRoute {
    Historical,
    FlowResvGasInjector(FlowResvReportStepContext),
}

pub(crate) fn fim_well_route(
    context: Option<FlowResvReportStepContext>,
    topology: &FimWellTopology,
    well_idx: usize,
) -> FimWellRoute {
    let Some(context) = context else {
        return FimWellRoute::Historical;
    };
    if well_idx != context.physical_well_idx {
        return FimWellRoute::Historical;
    }
    let well = &topology.wells[well_idx];
    assert_eq!(
        well.perforation_indices.len(),
        1,
        "validated RESV route has one perforation"
    );
    assert_eq!(well.perforation_indices[0], context.perforation_idx);
    assert_eq!(
        topology.perforations[context.perforation_idx].physical_well_index,
        well_idx
    );
    FimWellRoute::FlowResvGasInjector(context)
}

pub(crate) fn flow_resv_context_for_perforation(
    context: Option<FlowResvReportStepContext>,
    topology: &FimWellTopology,
    perf_idx: usize,
) -> Option<FlowResvReportStepContext> {
    let context = context?;
    if perf_idx != context.perforation_idx {
        return None;
    }
    match fim_well_route(context.into(), topology, context.physical_well_idx) {
        FimWellRoute::FlowResvGasInjector(route) => Some(route),
        FimWellRoute::Historical => unreachable!("selected RESV perforation lost its route"),
    }
}

/// The scoped one-perforation Flow gas-RESV residual bundle. `q_reservoir` is the current
/// reservoir-condition connection rate (negative for injection), while `surface_rate` is the
/// new positive Flow-style well unknown. `bg_reference` and `reservoir_target` are report-step
/// constants, deliberately not AD variables.
///
/// This is G4b1's contract only. Production assembly remains on the existing q-coordinate well
/// path until G4b2 can route every coupled row together.
#[derive(Clone, Copy, Debug)]
pub(crate) struct FlowResvInjectorResidual<S> {
    pub(crate) connection_rate_sc_day: S,
    pub(crate) perforation: S,
    pub(crate) control: S,
    pub(crate) gas_source_sc_day: S,
}

/// Evaluate the exact local rows from the G4 design, once for both `f64` values and local AD
/// derivatives. The current `bg` intentionally appears in the connection and source terms; the
/// frozen report-step `bg_reference` intentionally appears only in the control row.
pub(crate) fn flow_resv_injector_residual<S: Scalar>(
    q_reservoir_m3_day: S,
    bg_current_rm3_per_sm3: S,
    surface_rate_sm3_day: S,
    bg_reference_rm3_per_sm3: f64,
    reservoir_target_rm3_day: f64,
) -> FlowResvInjectorResidual<S> {
    let connection_rate_sc_day = -q_reservoir_m3_day / bg_current_rm3_per_sm3;
    FlowResvInjectorResidual {
        perforation: connection_rate_sc_day - surface_rate_sm3_day,
        control: surface_rate_sm3_day * S::from_f64(bg_reference_rm3_per_sm3)
            - S::from_f64(reservoir_target_rm3_day),
        gas_source_sc_day: -connection_rate_sc_day,
        connection_rate_sc_day,
    }
}

/// Build the context if its native option is enabled. ResSim currently has one implicit PVT/FIP
/// region; a multi-region model needs an explicit region mapping before this can be generalized.
pub(crate) fn begin_flow_resv_report_step_context(
    sim: &ReservoirSimulator,
    state: &FimState,
    nested_well_solve: bool,
) -> Result<Option<FlowResvReportStepContext>, String> {
    if !sim.fim_flow_resv_injector {
        return Ok(None);
    }
    // G4b3 supplies a route-aware `(bhp,u)` inner solve. The flag is accepted here and carried
    // by Newton state update; historical wells still use their q-coordinate local systems.
    let _ = nested_well_solve;
    if !sim.three_phase_mode || sim.injected_fluid != InjectedFluid::Gas {
        return Err("requires a three-phase gas injector".to_string());
    }
    if sim.pvt_table.is_none() {
        return Err("requires a gas PVT table".to_string());
    }

    let topology = build_well_topology(sim);
    let mut selected = Vec::new();
    for (physical_well_idx, physical) in topology.wells.iter().enumerate() {
        let well = &sim.wells[physical.representative_well_index];
        if well.schedule.control_kind() == Some(WellScheduleControl::Resv) {
            selected.push((physical_well_idx, well));
        }
    }
    if selected.len() != 1 {
        return Err(format!(
            "requires exactly one explicit RESV well, found {}",
            selected.len()
        ));
    }

    let (physical_well_idx, well) = selected[0];
    let physical = &topology.wells[physical_well_idx];
    if !well.injector || !well.schedule.enabled {
        return Err("requires one enabled gas injector".to_string());
    }
    if physical.perforation_indices.len() != 1 {
        return Err(format!(
            "requires one open perforation, found {}",
            physical.perforation_indices.len()
        ));
    }
    if well.schedule.target_surface_rate_m3_day.is_some() {
        return Err("RESV probe cannot also specify a surface-rate target".to_string());
    }
    if well.schedule.bhp_limit.is_some() {
        return Err("BHP-limited RESV control is outside G4b0".to_string());
    }
    let reservoir_target_rm3_day = well
        .schedule
        .target_rate_m3_day
        .filter(|target| target.is_finite() && *target > 0.0)
        .ok_or_else(|| "requires a positive explicit RESV target".to_string())?;

    let reference = capture_reference(sim, state)?;
    Ok(Some(FlowResvReportStepContext {
        reference,
        physical_well_idx,
        perforation_idx: physical.perforation_indices[0],
        reservoir_target_rm3_day,
    }))
}

impl FlowResvReportStepContext {
    /// Refresh only after acceptance. A retry copies this immutable value instead.
    pub(crate) fn refreshed_after_accepted_step(
        self,
        sim: &ReservoirSimulator,
        accepted_state: &FimState,
    ) -> Result<Self, String> {
        Ok(Self {
            reference: capture_reference(sim, accepted_state)?,
            ..self
        })
    }
}

fn capture_reference(
    sim: &ReservoirSimulator,
    state: &FimState,
) -> Result<FlowResvReference, String> {
    if state.cells.len() != sim.nx * sim.ny * sim.nz {
        return Err("state/grid cell count mismatch".to_string());
    }

    let mut hydrocarbon_pore_volume_m3 = 0.0;
    let mut pressure_weighted_sum = 0.0;
    for (idx, cell) in state.cells.iter().enumerate() {
        let hydrocarbon_pore_volume = sim.pore_volume_m3(idx) * (1.0 - cell.sw).max(0.0);
        hydrocarbon_pore_volume_m3 += hydrocarbon_pore_volume;
        pressure_weighted_sum += hydrocarbon_pore_volume * cell.pressure_bar;
    }
    if !hydrocarbon_pore_volume_m3.is_finite() || hydrocarbon_pore_volume_m3 <= 0.0 {
        return Err("hydrocarbon pore volume must be finite and positive".to_string());
    }

    let pressure_bar = pressure_weighted_sum / hydrocarbon_pore_volume_m3;
    let bg_rm3_per_sm3 = sim
        .pvt_table
        .as_ref()
        .ok_or_else(|| "requires a gas PVT table".to_string())?
        .interpolate(pressure_bar)
        .bg_m3m3;
    if !pressure_bar.is_finite() || !bg_rm3_per_sm3.is_finite() || bg_rm3_per_sm3 <= 0.0 {
        return Err("reference pressure and gas FVF must be finite and positive".to_string());
    }

    Ok(FlowResvReference {
        hydrocarbon_pore_volume_m3,
        pressure_bar,
        bg_rm3_per_sm3,
    })
}

#[cfg(test)]
mod tests {
    use nalgebra::DVector;

    use super::*;
    use crate::fim::ad::Ad;
    use crate::fim::assembly::{FimAssemblyOptions, assemble_fim_system};
    use crate::fim::assembly_ad::{
        assemble_fim_system_ad, cell_equation_residual_breakdown_ad, flow_resv_terms_f64,
    };
    use crate::fim::linear::{
        FimLinearBlockLayout, FimLinearSolveOptions, FimLinearSolverKind, solve_linearized_system,
    };
    use crate::fim::state::WellStateUpdateMode;
    use crate::fim::wells::build_well_topology;
    use crate::pvt::{PvtRow, PvtTable};
    use crate::well::WellSchedule;

    fn gas_resv_sim() -> ReservoirSimulator {
        let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        sim.set_three_phase_mode_enabled(true);
        sim.pvt_table = Some(PvtTable::new(
            vec![
                PvtRow {
                    p_bar: 100.0,
                    rs_m3m3: 10.0,
                    bo_m3m3: 1.2,
                    mu_o_cp: 1.0,
                    bg_m3m3: 0.01,
                    mu_g_cp: 0.02,
                },
                PvtRow {
                    p_bar: 200.0,
                    rs_m3m3: 20.0,
                    bo_m3m3: 1.1,
                    mu_o_cp: 1.0,
                    bg_m3m3: 0.0065,
                    mu_g_cp: 0.02,
                },
                PvtRow {
                    p_bar: 300.0,
                    rs_m3m3: 30.0,
                    bo_m3m3: 1.0,
                    mu_o_cp: 1.0,
                    bg_m3m3: 0.005,
                    mu_g_cp: 0.02,
                },
            ],
            sim.pvt.c_o,
        ));
        sim.set_three_phase_rel_perm_props(
            0.1, 0.1, 0.05, 0.05, 0.15, 2.0, 2.0, 2.0, 1.0, 1.0, 1.0,
        )
        .unwrap();
        sim.set_initial_pressure(200.0);
        sim.set_initial_saturation(0.15);
        sim.injected_fluid = InjectedFluid::Gas;
        sim.add_well_with_id(0, 0, 0, 250.0, 0.1, 0.0, true, "inj".to_string())
            .unwrap();
        sim.wells[0].schedule = WellSchedule {
            control_mode: Some("resv".to_string()),
            target_rate_m3_day: Some(500.0),
            target_surface_rate_m3_day: None,
            bhp_limit: None,
            enabled: true,
        };
        sim.set_fim_flow_resv_injector(true);
        sim
    }

    #[test]
    fn captures_hydrocarbon_pv_weighted_report_step_reference() {
        let sim = gas_resv_sim();
        sim.wells[0].validate(sim.nx, sim.ny, sim.nz).unwrap();
        let state = FimState::from_simulator(&sim);
        let context = begin_flow_resv_report_step_context(&sim, &state, false)
            .unwrap()
            .expect("enabled option must create context");

        assert_eq!(context.physical_well_idx, 0);
        assert_eq!(context.perforation_idx, 0);
        assert_eq!(context.reservoir_target_rm3_day, 500.0);
        assert!((context.reference.pressure_bar - 200.0).abs() < 1e-12);
        assert!((context.reference.bg_rm3_per_sm3 - 0.0065).abs() < 1e-12);
        assert!(context.reference.hydrocarbon_pore_volume_m3 > 0.0);
    }

    #[test]
    fn disabled_option_leaves_resv_context_absent() {
        let mut sim = gas_resv_sim();
        sim.set_fim_flow_resv_injector(false);
        let state = FimState::from_simulator(&sim);

        assert_eq!(
            begin_flow_resv_report_step_context(&sim, &state, false).unwrap(),
            None
        );
    }

    #[test]
    fn retry_keeps_reference_while_accepted_step_refreshes_it() {
        let sim = gas_resv_sim();
        let state = FimState::from_simulator(&sim);
        let context = begin_flow_resv_report_step_context(&sim, &state, false)
            .unwrap()
            .unwrap();
        let retry_context = context;

        let mut accepted = state.clone();
        accepted.cells[0].pressure_bar = 300.0;
        let refreshed = context
            .refreshed_after_accepted_step(&sim, &accepted)
            .unwrap();

        assert_eq!(retry_context, context);
        assert!((context.reference.pressure_bar - 200.0).abs() < 1e-12);
        assert!((refreshed.reference.pressure_bar - 300.0).abs() < 1e-12);
        assert!((refreshed.reference.bg_rm3_per_sm3 - 0.005).abs() < 1e-12);
    }

    #[test]
    fn rejects_incompatible_scope_before_newton_assembly() {
        let mut sim = gas_resv_sim();
        let state = FimState::from_simulator(&sim);
        assert!(
            begin_flow_resv_report_step_context(&sim, &state, true)
                .unwrap()
                .is_some(),
            "G4b3 must accept nested solve for the selected u-coordinate route"
        );

        sim.wells[0].schedule.bhp_limit = Some(350.0);
        let bhp_limited = begin_flow_resv_report_step_context(&sim, &state, false).unwrap_err();
        assert!(bhp_limited.contains("BHP-limited"));
    }

    #[test]
    fn initializes_surface_primary_and_matches_ad_legacy_selected_route() {
        let sim = gas_resv_sim();
        let mut state = FimState::from_simulator(&sim);
        let context = begin_flow_resv_report_step_context(&sim, &state, false)
            .unwrap()
            .unwrap();
        let topology = build_well_topology(&sim);
        state
            .initialize_flow_resv_gas_primary(&sim, &topology, context)
            .unwrap();

        let primary = state.perforation_primary(context.perforation_idx);
        assert_eq!(
            primary.kind,
            crate::fim::state::FimPerforationPrimaryKind::FlowResvGasSurfaceU
        );
        assert!(
            state
                .reservoir_connection_q(context.perforation_idx)
                .is_none()
        );
        let u = state
            .flow_resv_surface_u(context.perforation_idx)
            .expect("initialized RESV slot is typed u");
        assert_eq!(primary.value, u);
        assert!(u > 0.0);
        assert!(
            (u - context.reservoir_target_rm3_day / context.reference.bg_rm3_per_sm3).abs() < 1e-8
        );
        let q = crate::fim::wells::connection_rate_for_bhp(
            &sim,
            &state,
            &topology,
            context.perforation_idx,
            state.well_bhp[context.physical_well_idx],
        )
        .unwrap();
        assert!((-q / context.reference.bg_rm3_per_sm3 - u).abs() < 1e-6);

        // Exercise the routed Jacobian away from PVT/relperm endpoints and away from the
        // report-step reference pressure. Initialization was checked above at the report-start
        // state; this is an arbitrary Newton evaluation state.
        state.cells[0].pressure_bar = 205.0;
        state.cells[0].sw = 0.20;
        state.cells[0].hydrocarbon_var = 0.08;
        state.cells[0].regime = crate::fim::state::HydrocarbonState::Saturated;

        let options = FimAssemblyOptions {
            dt_days: 0.25,
            include_wells: true,
            assemble_residual_only: false,
            topology: Some(&topology),
            flow_resv_context: Some(context),
        };
        let ad = assemble_fim_system_ad(&sim, &state, &state, &options);
        let legacy = assemble_fim_system(&sim, &state, &state, &options);
        assert_eq!(ad.residual.len(), legacy.residual.len());
        for (actual, expected) in ad.residual.iter().zip(legacy.residual.iter()) {
            assert!((actual - expected).abs() < 1e-10);
        }
        for row in 0..ad.residual.len() {
            for col in 0..ad.residual.len() {
                let lhs = ad.jacobian.get(row, col).copied().unwrap_or(0.0);
                let rhs = legacy.jacobian.get(row, col).copied().unwrap_or(0.0);
                // Reciprocal PVDG interpolation changes the floating-point evaluation order;
                // 2e-10 is below 1e-15 relative error for the largest selected-well entry.
                assert!(
                    (lhs - rhs).abs() < 2e-10,
                    "AD/legacy mismatch at ({row},{col}): {lhs} vs {rhs}"
                );
            }
        }
        let gas_row = crate::fim::assembly::equation_offset(0, 2);
        let perf_row = state.perforation_equation_offset(context.perforation_idx);
        let u_col = state.perforation_rate_unknown_offset(context.perforation_idx);
        let control_row = state.well_equation_offset(context.physical_well_idx);
        assert!(ad.jacobian.get(gas_row, u_col).is_none());
        assert!(
            (ad.jacobian.get(control_row, u_col).copied().unwrap_or(0.0)
                - context.reference.bg_rm3_per_sm3)
                .abs()
                < 1e-12
        );
        assert!((ad.equation_scaling.well_constraint[0] - 500.0).abs() < 1e-12);
        assert!((ad.equation_scaling.perforation_flow[0] - u).abs() < 1e-8);
        assert!((ad.variable_scaling.perforation_rate[0] - u).abs() < 1e-8);

        let selected_terms =
            flow_resv_terms_f64(&sim, &state, &topology, context.perforation_idx, context)
                .expect("selected RESV diagnostic terms");
        for component in 0..3 {
            let partition = cell_equation_residual_breakdown_ad(
                &sim,
                &state,
                &state,
                &topology,
                options.dt_days,
                Some(context),
                0,
                component,
            )
            .expect("selected reservoir partition");
            assert!(
                (partition.total - ad.residual[component]).abs() < 1e-10,
                "component {component} partition={} assembly={}",
                partition.total,
                ad.residual[component]
            );
            let expected_source = if component == 2 {
                selected_terms.gas_source_sc_day * options.dt_days
            } else {
                0.0
            };
            assert!((partition.well_source - expected_source).abs() < 1e-10);
        }

        let p_col = crate::fim::assembly::unknown_offset(0, 0);
        let sw_col = crate::fim::assembly::unknown_offset(0, 1);
        let hc_col = crate::fim::assembly::unknown_offset(0, 2);
        let bhp_col = state.well_bhp_unknown_offset(context.physical_well_idx);
        for (column, step) in [
            (p_col, 1e-4),
            (sw_col, 1e-6),
            (hc_col, 1e-6),
            (bhp_col, 1e-4),
            (u_col, 1e-2),
        ] {
            let mut lower = state.clone();
            let mut upper = state.clone();
            match column {
                c if c == p_col => {
                    lower.cells[0].pressure_bar -= step;
                    upper.cells[0].pressure_bar += step;
                }
                c if c == sw_col => {
                    lower.cells[0].sw -= step;
                    upper.cells[0].sw += step;
                }
                c if c == hc_col => {
                    lower.cells[0].hydrocarbon_var -= step;
                    upper.cells[0].hydrocarbon_var += step;
                }
                c if c == bhp_col => {
                    lower.well_bhp[0] -= step;
                    upper.well_bhp[0] += step;
                }
                c if c == u_col => {
                    *lower.perforation_primary_value_mut(0) -= step;
                    *upper.perforation_primary_value_mut(0) += step;
                }
                _ => unreachable!(),
            }
            let lower_residual = assemble_fim_system_ad(&sim, &state, &lower, &options).residual;
            let upper_residual = assemble_fim_system_ad(&sim, &state, &upper, &options).residual;
            for row in [gas_row, perf_row, control_row] {
                let fd = (upper_residual[row] - lower_residual[row]) / (2.0 * step);
                let analytic = ad.jacobian.get(row, column).copied().unwrap_or(0.0);
                let tolerance = 2e-5 * analytic.abs().max(fd.abs()).max(1.0);
                assert!(
                    (analytic - fd).abs() < tolerance,
                    "FD mismatch row={row} col={column}: {analytic} vs {fd}"
                );
            }
        }

        let rhs = -ad.residual.clone();
        let layout = FimLinearBlockLayout {
            cell_block_count: 1,
            cell_block_size: 3,
            well_bhp_count: 1,
            perforation_tail_start: 4,
        };
        let direct_options = FimLinearSolveOptions {
            kind: FimLinearSolverKind::SparseLuDebug,
            relative_tolerance: 1e-12,
            absolute_tolerance: 1e-14,
            ..FimLinearSolveOptions::default()
        };
        let direct = solve_linearized_system(
            &ad.jacobian,
            &rhs,
            &direct_options,
            Some(layout),
            Some(&ad.equation_scaling),
        );
        assert!(
            direct.converged,
            "direct RESV fixture must factorize: {direct:?}"
        );
        let schur_options = FimLinearSolveOptions {
            kind: FimLinearSolverKind::GmresIlu0,
            eliminate_wells: true,
            max_iterations: 100,
            relative_tolerance: 1e-12,
            absolute_tolerance: 1e-14,
            ..FimLinearSolveOptions::default()
        };
        let schur = solve_linearized_system(
            &ad.jacobian,
            &rhs,
            &schur_options,
            Some(layout),
            Some(&ad.equation_scaling),
        );
        assert!(
            schur.converged,
            "RESV well-Schur solve must converge: {schur:?}"
        );
        for row in 0..rhs.len() {
            assert!(
                (schur.solution[row] - direct.solution[row]).abs() < 1e-9,
                "RESV Schur/direct correction mismatch row={row}: {} vs {}",
                schur.solution[row],
                direct.solution[row]
            );
        }
    }

    #[test]
    fn flow_resv_update_applies_bhp_and_u_without_historical_q_postprocessing() {
        let sim = gas_resv_sim();
        let mut state = FimState::from_simulator(&sim);
        let context = begin_flow_resv_report_step_context(&sim, &state, false)
            .unwrap()
            .unwrap();
        let topology = build_well_topology(&sim);
        state
            .initialize_flow_resv_gas_primary(&sim, &topology, context)
            .unwrap();
        state.well_bhp[context.physical_well_idx] = 220.0;
        *state.perforation_primary_value_mut(context.perforation_idx) = 10.0;

        let mut update = DVector::zeros(state.n_unknowns());
        update[state.well_bhp_unknown_offset(context.physical_well_idx)] = 5.0;
        update[state.perforation_rate_unknown_offset(context.perforation_idx)] = -20.0;
        let candidate = state.apply_newton_update_frozen(
            &sim,
            &update,
            1.0,
            &topology,
            WellStateUpdateMode::FlowResv {
                context,
                nested_well_solve: false,
            },
        );

        assert!((candidate.well_bhp[context.physical_well_idx] - 225.0).abs() < 1e-12);
        assert_eq!(
            candidate.flow_resv_surface_u(context.perforation_idx),
            Some(0.0)
        );
        assert!(
            candidate
                .reservoir_connection_q(context.perforation_idx)
                .is_none()
        );
    }

    #[test]
    fn g4b3_local_rows_and_columns_match_selected_global_assembly() {
        let sim = gas_resv_sim();
        let mut state = FimState::from_simulator(&sim);
        let context = begin_flow_resv_report_step_context(&sim, &state, true)
            .unwrap()
            .unwrap();
        let topology = build_well_topology(&sim);
        state
            .initialize_flow_resv_gas_primary(&sim, &topology, context)
            .unwrap();
        state.cells[0].pressure_bar = 205.0;
        state.cells[0].sw = 0.20;
        state.cells[0].hydrocarbon_var = 0.08;
        state.well_bhp[context.physical_well_idx] = 220.0;
        *state.perforation_primary_value_mut(context.perforation_idx) = 10.0;

        let local = crate::fim::wells_inner::assemble_flow_resv_well_local_system(
            &sim, &state, &topology, context,
        );
        let options = FimAssemblyOptions {
            dt_days: 0.25,
            include_wells: true,
            assemble_residual_only: false,
            topology: Some(&topology),
            flow_resv_context: Some(context),
        };
        let global = assemble_fim_system_ad(&sim, &state, &state, &options);
        let global_rows = [
            state.well_equation_offset(context.physical_well_idx),
            state.perforation_equation_offset(context.perforation_idx),
        ];
        let global_cols = [
            state.well_bhp_unknown_offset(context.physical_well_idx),
            state.perforation_rate_unknown_offset(context.perforation_idx),
        ];
        for local_row in 0..2 {
            assert_eq!(
                local.residual[local_row],
                global.residual[global_rows[local_row]]
            );
            for local_col in 0..2 {
                let global_value = global
                    .jacobian
                    .get(global_rows[local_row], global_cols[local_col])
                    .copied()
                    .unwrap_or(0.0);
                assert!(
                    (local.jacobian[(local_row, local_col)] - global_value).abs() < 1e-12,
                    "local/global mismatch at ({local_row},{local_col}): {} vs {global_value}",
                    local.jacobian[(local_row, local_col)]
                );
            }
        }
    }

    #[test]
    fn g4b3_nested_update_restores_control_and_connection_rows() {
        let sim = gas_resv_sim();
        let mut state = FimState::from_simulator(&sim);
        let context = begin_flow_resv_report_step_context(&sim, &state, true)
            .unwrap()
            .unwrap();
        let topology = build_well_topology(&sim);
        state
            .initialize_flow_resv_gas_primary(&sim, &topology, context)
            .unwrap();
        state.cells[0].pressure_bar = 205.0;
        state.cells[0].sw = 0.20;
        state.cells[0].hydrocarbon_var = 0.08;
        state.well_bhp[context.physical_well_idx] = 220.0;
        *state.perforation_primary_value_mut(context.perforation_idx) = 10.0;

        let candidate = state.apply_newton_update_frozen(
            &sim,
            &DVector::zeros(state.n_unknowns()),
            1.0,
            &topology,
            WellStateUpdateMode::FlowResv {
                context,
                nested_well_solve: true,
            },
        );
        let expected_u = context.reservoir_target_rm3_day / context.reference.bg_rm3_per_sm3;
        assert!(
            (candidate
                .flow_resv_surface_u(context.perforation_idx)
                .unwrap()
                - expected_u)
                .abs()
                < 1e-8
        );
        let local = crate::fim::wells_inner::assemble_flow_resv_well_local_system(
            &sim, &candidate, &topology, context,
        );
        assert!(
            local.residual.iter().all(|value| value.abs() < 1e-7),
            "selected local rows not restored: {:?}",
            local.residual
        );
        assert!(crate::fim::wells_inner::flow_resv_well_is_converged(
            &sim,
            &candidate,
            &topology,
            context,
            &crate::fim::wells_inner::FimWellInnerSolveOptions::default(),
        ));
    }

    #[test]
    fn g4b3_mixed_route_preserves_historical_well_inner_solve() {
        let mut sim = gas_resv_sim();
        sim.add_well_with_id(0, 0, 0, 150.0, 0.1, 0.0, false, "prod".to_string())
            .unwrap();
        let mut state = FimState::from_simulator(&sim);
        let context = begin_flow_resv_report_step_context(&sim, &state, true)
            .unwrap()
            .unwrap();
        let topology = build_well_topology(&sim);
        state
            .initialize_flow_resv_gas_primary(&sim, &topology, context)
            .unwrap();
        let historical_well = (0..topology.wells.len())
            .find(|&well_idx| well_idx != context.physical_well_idx)
            .unwrap();
        let historical_perf = topology.wells[historical_well].perforation_indices[0];
        state.well_bhp[historical_well] += 25.0;
        *state
            .reservoir_connection_q_mut(historical_perf)
            .expect("non-selected producer remains a reservoir-q route") += 500.0;

        let candidate = state.apply_newton_update_frozen(
            &sim,
            &DVector::zeros(state.n_unknowns()),
            1.0,
            &topology,
            WellStateUpdateMode::FlowResv {
                context,
                nested_well_solve: true,
            },
        );
        let options = crate::fim::wells_inner::FimWellInnerSolveOptions::default();
        assert!(crate::fim::wells_inner::flow_resv_well_is_converged(
            &sim, &candidate, &topology, context, &options,
        ));
        assert!(crate::fim::wells_inner::well_is_converged(
            &sim,
            &candidate,
            &topology,
            historical_well,
            &options,
        ));
        assert!(crate::fim::wells_inner::all_wells_converged_with_flow_resv(
            &sim, &candidate, &topology, context, &options,
        ));
    }

    #[derive(Clone, Copy)]
    struct LocalConnectionCase {
        pressure_bar: f64,
        q_reservoir_m3_day: f64,
        dq_dp_m3_day_bar: f64,
        bg_rm3_per_sm3: f64,
        dbg_dp_rm3_per_sm3_bar: f64,
    }

    fn current_connection(case: LocalConnectionCase, pressure_bar: f64) -> (f64, f64) {
        let delta_p = pressure_bar - case.pressure_bar;
        (
            case.q_reservoir_m3_day + case.dq_dp_m3_day_bar * delta_p,
            case.bg_rm3_per_sm3 + case.dbg_dp_rm3_per_sm3_bar * delta_p,
        )
    }

    fn residual_at_pressure(
        case: LocalConnectionCase,
        pressure_bar: f64,
        surface_rate_sm3_day: f64,
    ) -> FlowResvInjectorResidual<f64> {
        let (q_reservoir, bg_current) = current_connection(case, pressure_bar);
        flow_resv_injector_residual(q_reservoir, bg_current, surface_rate_sm3_day, 0.0065, 500.0)
    }

    #[test]
    fn residual_contract_keeps_current_fvf_in_connection_and_source_at_two_pressures() {
        let surface_rate = 500.0 / 0.0065;
        let cases = [
            LocalConnectionCase {
                pressure_bar: 242.679,
                q_reservoir_m3_day: -surface_rate * 0.005_219_627_384,
                dq_dp_m3_day_bar: -2.75,
                bg_rm3_per_sm3: 0.005_219_627_384,
                dbg_dp_rm3_per_sm3_bar: -8.0e-6,
            },
            LocalConnectionCase {
                pressure_bar: 275.0,
                q_reservoir_m3_day: -surface_rate * 0.004_85,
                dq_dp_m3_day_bar: -1.5,
                bg_rm3_per_sm3: 0.004_85,
                dbg_dp_rm3_per_sm3_bar: -5.0e-6,
            },
        ];

        for case in cases {
            let value = residual_at_pressure(case, case.pressure_bar, surface_rate);
            assert!((value.connection_rate_sc_day - surface_rate).abs() < 1e-8);
            assert!(value.perforation.abs() < 1e-8);
            assert!(value.control.abs() < 1e-12);
            assert!((value.gas_source_sc_day + surface_rate).abs() < 1e-8);

            let q_ad = Ad::<2>::seeded(case.q_reservoir_m3_day, [case.dq_dp_m3_day_bar, 0.0]);
            let bg_ad = Ad::<2>::seeded(case.bg_rm3_per_sm3, [case.dbg_dp_rm3_per_sm3_bar, 0.0]);
            let u_ad = Ad::<2>::variable(surface_rate, 1);
            let ad = flow_resv_injector_residual(q_ad, bg_ad, u_ad, 0.0065, 500.0);

            let expected_connection_dp = -(case.dq_dp_m3_day_bar * case.bg_rm3_per_sm3
                - case.q_reservoir_m3_day * case.dbg_dp_rm3_per_sm3_bar)
                / case.bg_rm3_per_sm3.powi(2);
            assert!((ad.connection_rate_sc_day.d(0) - expected_connection_dp).abs() < 1e-8);
            assert!((ad.perforation.d(0) - expected_connection_dp).abs() < 1e-8);
            assert!((ad.gas_source_sc_day.d(0) + expected_connection_dp).abs() < 1e-8);
            assert_eq!(ad.gas_source_sc_day.d(1), 0.0);
            assert_eq!(ad.perforation.d(1), -1.0);
            assert!((ad.control.d(1) - 0.0065).abs() < 1e-15);
            assert_eq!(ad.control.d(0), 0.0);
        }
    }

    #[test]
    fn residual_contract_ad_pressure_derivatives_match_central_finite_difference() {
        let surface_rate = 500.0 / 0.0065;
        let case = LocalConnectionCase {
            pressure_bar: 242.679,
            q_reservoir_m3_day: -surface_rate * 0.005_219_627_384,
            dq_dp_m3_day_bar: -2.75,
            bg_rm3_per_sm3: 0.005_219_627_384,
            dbg_dp_rm3_per_sm3_bar: -8.0e-6,
        };
        let h = 1.0e-4;
        let lower = residual_at_pressure(case, case.pressure_bar - h, surface_rate);
        let upper = residual_at_pressure(case, case.pressure_bar + h, surface_rate);
        let q_ad = Ad::<1>::seeded(case.q_reservoir_m3_day, [case.dq_dp_m3_day_bar]);
        let bg_ad = Ad::<1>::seeded(case.bg_rm3_per_sm3, [case.dbg_dp_rm3_per_sm3_bar]);
        let ad = flow_resv_injector_residual(
            q_ad,
            bg_ad,
            Ad::<1>::constant(surface_rate),
            0.0065,
            500.0,
        );

        let fd_perforation = (upper.perforation - lower.perforation) / (2.0 * h);
        let fd_source = (upper.gas_source_sc_day - lower.gas_source_sc_day) / (2.0 * h);
        assert!((ad.perforation.d(0) - fd_perforation).abs() < 1e-5);
        assert!((ad.gas_source_sc_day.d(0) - fd_source).abs() < 1e-5);
    }
}
