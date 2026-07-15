//! G4b0's inert Flow gas-RESV injector context.
//!
//! This module captures only the report-step reference state and validates the narrow supported
//! scope. It does not route an assembler or reinterpret a FIM rate unknown; those coupled changes
//! belong to G4b1/G4b2.

use crate::fim::state::FimState;
use crate::fim::wells::build_well_topology;
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
    if nested_well_solve {
        return Err(
            "FIM_NESTED_WELL_SOLVE is q-coordinate only; G4b3 must re-derive it for surface u"
                .to_string(),
        );
    }
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
    use super::*;
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
        let nested = begin_flow_resv_report_step_context(&sim, &state, true).unwrap_err();
        assert!(nested.contains("q-coordinate"));

        sim.wells[0].schedule.bhp_limit = Some(350.0);
        let bhp_limited = begin_flow_resv_report_step_context(&sim, &state, false).unwrap_err();
        assert!(bhp_limited.contains("BHP-limited"));
    }
}
