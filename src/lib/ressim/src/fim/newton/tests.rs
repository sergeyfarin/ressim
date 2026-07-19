use nalgebra::DVector;

use crate::ReservoirSimulator;
use crate::fim::assembly::{FimAssemblyOptions, assemble_fim_system};
use crate::fim::scaling::EquationScaling;
use crate::fim::state::FimState;
use crate::pvt::{PvtRow, PvtTable};

use super::*;

#[test]
fn opm_relaxed_linear_acceptance_is_backend_neutral() {
    let mut report = FimLinearSolveReport {
        solution: DVector::from_element(1, 1.0),
        converged: false,
        iterations: 1,
        rhs_norm: 10.0,
        final_residual_norm: 0.05,
        // Sparse LU intentionally has no iterative-failure payload. The relaxed decision
        // must be identical to a CPR report with the same returned correction quality.
        failure_diagnostics: None,
        used_fallback: false,
        backend_used: FimLinearSolverKind::SparseLuDebug,
        cpr_diagnostics: None,
        total_time_ms: 0.0,
        preconditioner_build_time_ms: 0.0,
    };

    assert!(opm_accepts_relaxed_linear_report(&report));

    report.final_residual_norm = 0.1;
    assert!(!opm_accepts_relaxed_linear_report(&report));

    report.final_residual_norm = 0.05;
    report.solution[0] = f64::NAN;
    assert!(!opm_accepts_relaxed_linear_report(&report));
}

fn two_cell_state_for_chop(regimes: [HydrocarbonState; 2]) -> FimState {
    let sim = ReservoirSimulator::new(2, 1, 1, 0.2);
    let mut state = FimState::from_simulator(&sim);
    for (idx, regime) in regimes.iter().enumerate() {
        state.cells[idx].pressure_bar = 200.0;
        state.cells[idx].sw = 0.3;
        state.cells[idx].hydrocarbon_var = match regime {
            HydrocarbonState::Saturated => 0.1,       // Sg meaning
            HydrocarbonState::Undersaturated => 50.0, // Rs meaning
        };
        state.cells[idx].regime = *regime;
    }
    state
}

/// Boundary-only derivative evidence for Y2b.  This is deliberately a
/// one-cell gas injector so each reported reservoir row is the injector's
/// connected cell and the perforation row remains present.  The exact
/// 10x10x3 repro supplies the live trajectory; this fixture supplies the
/// controlled `bound-eps/bound/bound+eps` probes for every active clamp.
#[test]
fn y2b_boundary_injector_fixture_reports_ad_legacy_and_one_sided_fd() {
    let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
    sim.set_three_phase_rel_perm_props(0.15, 0.15, 0.05, 0.05, 0.2, 2.0, 2.0, 2.0, 1e-5, 1.0, 0.95)
        .unwrap();
    sim.set_three_phase_mode_enabled(true);
    sim.set_injected_fluid("gas").unwrap();
    sim.set_rate_controlled_wells(true);
    sim.set_target_well_rates(100.0, 0.0).unwrap();
    sim.add_well(0, 0, 0, 250.0, 0.1, 0.0, true).unwrap();
    let topology = build_well_topology(&sim);
    let perf_idx = 0;
    let cell_idx = 0;
    let options = FimAssemblyOptions {
        dt_days: 0.25,
        include_wells: true,
        assemble_residual_only: false,
        topology: Some(&topology),
        flow_resv_context: None,
    };
    let eps = 1e-7;
    let cases = [
        ("swc", 0.15, 0.10, 1usize),
        ("sg_zero", 0.30, 0.0, 2usize),
        ("sw_upper", 0.80, 0.0, 1usize),
        ("sg_upper", 0.15, 0.65, 2usize),
    ];

    for (boundary, sw, sg, boundary_column) in cases {
        for (sample, signed_eps) in [("minus", -eps), ("exact", 0.0), ("plus", eps)] {
            let mut state = FimState::from_simulator(&sim);
            state.cells[cell_idx].sw = sw;
            state.cells[cell_idx].hydrocarbon_var = sg;
            state.cells[cell_idx].regime = HydrocarbonState::Saturated;
            if boundary_column == 1 {
                state.cells[cell_idx].sw += signed_eps;
            } else {
                state.cells[cell_idx].hydrocarbon_var += signed_eps;
            }
            let previous_state = state.clone();
            let ad = crate::fim::assembly_ad::assemble_fim_system_ad(
                &sim,
                &previous_state,
                &state,
                &options,
            );
            let legacy = assemble_fim_system(&sim, &previous_state, &state, &options);
            let rows = [
                (
                    "rate_consistency",
                    state.perforation_equation_offset(perf_idx),
                ),
                ("water", equation_offset(cell_idx, 0)),
                ("oil", equation_offset(cell_idx, 1)),
                ("gas", equation_offset(cell_idx, 2)),
            ];
            let columns = [
                ("p", unknown_offset(cell_idx, 0)),
                ("sw", unknown_offset(cell_idx, 1)),
                ("hc", unknown_offset(cell_idx, 2)),
                ("bhp", state.well_bhp_unknown_offset(0)),
                ("q", state.perforation_rate_unknown_offset(perf_idx)),
            ];
            for (row_label, row) in rows {
                assert!(ad.residual[row].is_finite() && legacy.residual[row].is_finite());
                for (column_label, column) in columns {
                    let h = y2a_finite_difference_step(&state, column);
                    let mut plus = state.clone();
                    let mut minus = state.clone();
                    y2a_perturb_unknown(&mut plus, column, h);
                    y2a_perturb_unknown(&mut minus, column, -h);
                    let plus_residual =
                        assemble_fim_system(&sim, &previous_state, &plus, &options).residual[row];
                    let minus_residual =
                        assemble_fim_system(&sim, &previous_state, &minus, &options).residual[row];
                    let central = (plus_residual - minus_residual) / (2.0 * h);
                    let forward = (plus_residual - legacy.residual[row]) / h;
                    let backward = (legacy.residual[row] - minus_residual) / h;
                    let ad_derivative = ad.jacobian.get(row, column).copied().unwrap_or(0.0);
                    let legacy_derivative =
                        legacy.jacobian.get(row, column).copied().unwrap_or(0.0);
                    assert!(
                        ad_derivative.is_finite()
                            && legacy_derivative.is_finite()
                            && central.is_finite()
                            && forward.is_finite()
                            && backward.is_finite(),
                        "{boundary}/{sample} {row_label}/{column_label} must remain finite"
                    );
                    println!(
                        "Y2B fixture boundary={boundary} sample={sample} row={row_label} col={column_label} residual_ad={:.12e} residual_legacy={:.12e} ad={ad_derivative:.12e} legacy={legacy_derivative:.12e} central={central:.12e} forward={forward:.12e} backward={backward:.12e}",
                        ad.residual[row], legacy.residual[row],
                    );
                }
            }
        }
    }
}

#[test]
fn opm_per_cell_chop_scales_only_the_violating_cell_and_counts_implied_so() {
    let state = two_cell_state_for_chop([HydrocarbonState::Saturated; 2]);
    let mut update = DVector::zeros(state.n_unknowns());
    // Cell 0: dSw=0.15, dSg=0.15 → implied dSo=-0.3 is the max → satAlpha=0.2/0.3.
    update[1] = 0.15;
    update[2] = 0.15;
    // Cell 1: small, untouched.
    update[4] = 0.01;
    update[5] = -0.02;

    let chopped = opm_per_cell_chopped_update(&state, &update, 1.0);

    let alpha = OPM_DS_MAX / 0.3;
    assert!((chopped[1] - 0.15 * alpha).abs() < 1e-12);
    assert!((chopped[2] - 0.15 * alpha).abs() < 1e-12);
    // Neither dSw=0.15 nor dSg=0.15 alone exceeds 0.2 — only the implied So does;
    // a port missing the implied-So term would leave cell 0 unchopped.
    assert!(alpha < 1.0);
    // Cell 1 must be exactly untouched — no global coupling.
    assert_eq!(chopped[4], 0.01);
    assert_eq!(chopped[5], -0.02);
}

#[test]
fn opm_per_cell_chop_clamps_pressure_relative_and_independent_of_saturation() {
    let state = two_cell_state_for_chop([HydrocarbonState::Saturated; 2]);
    let mut update = DVector::zeros(state.n_unknowns());
    update[0] = -100.0; // |dp| > 0.3 * 200 = 60 → clamp to -60
    update[1] = 0.5; // dSw drives satAlpha = 0.2/0.5 = 0.4
    update[3] = 30.0; // cell 1: within cap, untouched

    let chopped = opm_per_cell_chopped_update(&state, &update, 1.0);

    assert!(
        (chopped[0] - (-60.0)).abs() < 1e-12,
        "dp clamped to signum*0.3*p"
    );
    assert!((chopped[1] - 0.5 * 0.4).abs() < 1e-12);
    assert_eq!(chopped[3], 30.0);
}

#[test]
fn opm_per_cell_chop_guards_rs_nonnegative_without_sat_alpha() {
    let state = two_cell_state_for_chop([
        HydrocarbonState::Undersaturated,
        HydrocarbonState::Undersaturated,
    ]);
    let mut update = DVector::zeros(state.n_unknowns());
    // Rs delta would take hydrocarbon_var (50.0) to -30 → guard to exactly -50.
    update[2] = -80.0;
    // dSw alone at 0.25 → implied dSo=-0.25 → satAlpha=0.8; Rs delta must NOT be
    // scaled by satAlpha (it is not a saturation).
    update[1] = 0.25;
    update[5] = -10.0; // cell 1: Rs delta within bounds, untouched

    let chopped = opm_per_cell_chopped_update(&state, &update, 1.0);

    assert!(
        (chopped[2] - (-50.0)).abs() < 1e-12,
        "Rs guard: current + delta >= 0"
    );
    assert!((chopped[1] - 0.25 * 0.8).abs() < 1e-12);
    assert_eq!(chopped[5], -10.0);
}

#[test]
fn opm_per_cell_chop_applies_relaxation_before_chopping() {
    let state = two_cell_state_for_chop([HydrocarbonState::Saturated; 2]);
    let mut update = DVector::zeros(state.n_unknowns());
    update[1] = 0.3; // raw dSw exceeds dsMax, but relax=0.5 brings it to 0.15 → no chop

    let chopped = opm_per_cell_chopped_update(&state, &update, 0.5);

    assert!(
        (chopped[1] - 0.15).abs() < 1e-12,
        "relaxation applies first, then chop"
    );
}

#[test]
fn opm_per_cell_chop_clamps_well_bhp_relative_when_increasing() {
    // An INCREASING dBHP isolates the relative clamp from the positivity floor (a
    // decreasing dBHP at dbhp_max_rel=1.0 always drives next_bhp to exactly 0, which is
    // always below the floor too — covered separately below).
    let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
    sim.set_injected_fluid("water").unwrap();
    sim.add_well(0, 0, 0, 500.0, 0.1, 0.0, true).unwrap();
    let state = FimState::from_simulator(&sim);
    let mut update = DVector::zeros(state.n_unknowns());
    // Raw dBHP=+600 exceeds dbhp-max-rel=1.0 * bhp(500) = 500 → clamp to +500.
    update[state.well_bhp_unknown_offset(0)] = 600.0;

    let chopped = opm_per_cell_chopped_update(&state, &update, 1.0);

    assert!(
        (chopped[state.well_bhp_unknown_offset(0)] - 500.0).abs() < 1e-12,
        "dBHP clamped to +dbhp_max_rel*bhp_current"
    );
}

#[test]
fn opm_per_cell_chop_well_bhp_within_cap_is_untouched() {
    let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
    sim.set_injected_fluid("water").unwrap();
    sim.add_well(0, 0, 0, 500.0, 0.1, 0.0, true).unwrap();
    let state = FimState::from_simulator(&sim);
    let mut update = DVector::zeros(state.n_unknowns());
    update[state.well_bhp_unknown_offset(0)] = 200.0; // within 1.0*500 cap

    let chopped = opm_per_cell_chopped_update(&state, &update, 1.0);

    assert_eq!(chopped[state.well_bhp_unknown_offset(0)], 200.0);
}

#[test]
fn opm_per_cell_chop_well_bhp_floors_above_lower_limit() {
    let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
    sim.set_injected_fluid("water").unwrap();
    sim.add_well(0, 0, 0, 1.5, 0.1, 0.0, true).unwrap();
    let state = FimState::from_simulator(&sim);
    let mut update = DVector::zeros(state.n_unknowns());
    // bhp=1.5; dbhp_max_rel*1.5=1.5 caps |dbhp| at 1.5, which alone would take bhp to 0.0 —
    // below OPM_BHP_LOWER_LIMIT_BAR (1.0) — so the floor must bind instead of the raw clamp.
    update[state.well_bhp_unknown_offset(0)] = -3.0;

    let chopped = opm_per_cell_chopped_update(&state, &update, 1.0);

    let next_bhp = 1.5 + chopped[state.well_bhp_unknown_offset(0)];
    assert!(
        (next_bhp - OPM_BHP_LOWER_LIMIT_BAR).abs() < 1e-12,
        "next bhp floored at OPM_BHP_LOWER_LIMIT_BAR, got {next_bhp}"
    );
}

#[test]
fn cnv_mb_from_parts_matches_hand_computed_values() {
    // Two cells, pv = [100, 300] m³; FVFs chosen distinct per component so a mix-up
    // between components or between B_avg and per-cell B shows up in the numbers.
    // B_avg = [(1.0+1.0)/2, (1.2+1.4)/2, (0.01+0.03)/2] = [1.0, 1.3, 0.02].
    let residual = DVector::from_vec(vec![
        2.0, -0.6, 0.0, // cell 0: water, oil, gas
        -1.0, 0.9, 0.0, // cell 1
    ]);
    let pv = [100.0, 300.0];
    let fvf = [[1.0, 1.2, 0.01], [1.0, 1.4, 0.03]];

    let d = cnv_mb_from_parts(&residual, &pv, &fvf, false);

    // maxCoeff per component: water max(2/100, 1/300)=0.02; oil max(0.6/100, 0.9/300)=0.006.
    // CNV = B_avg * maxCoeff.
    assert!((d.cnv[0] - 1.0 * 0.02).abs() < 1e-12);
    assert!((d.cnv[1] - 1.3 * 0.006).abs() < 1e-12);
    assert_eq!(d.cnv[2], 0.0);
    // MB = |B_avg * signed sum| / pvSum; water sum = 1.0, oil sum = 0.3, pvSum = 400.
    assert!((d.mb[0] - 1.0 * 1.0 / 400.0).abs() < 1e-12);
    assert!((d.mb[1] - 1.3 * 0.3 / 400.0).abs() < 1e-12);
    // Cell 0's own max CNV = max(2*1.0, 0.6*1.3)/100 = 0.02 > 1e-2 → violating.
    // Cell 1's = max(1*1.0, 0.9*1.3)/300 = 0.0039 < 1e-2 → not violating.
    assert!((d.violating_pv_fraction - 100.0 / 400.0).abs() < 1e-12);
    assert!(!d.pv_rule_relaxes); // 25% of PV violating >> 3%
    assert!(!d.would_accept_strict);
    assert!(!d.would_accept);
}

#[test]
fn cnv_pv_rule_relaxes_when_violating_pv_below_three_percent() {
    // Cell 0 is a tiny plateau cell (1% of PV) with a CNV way above strict tolerance
    // but below the relaxed 1.0; cell 1 is huge and clean. MB stays tiny because the
    // residual is small in absolute terms. OPM accepts this state; the strict check
    // does not — exactly the water@1215 heavy-case pattern.
    let residual = DVector::from_vec(vec![
        0.5, 0.0, 0.0, // cell 0: water CNV = 0.5/10 = 0.05 (> 1e-2, < 1.0)
        0.0, 0.0, 0.0, // cell 1: clean
    ]);
    let pv = [10.0, 990.0];
    let fvf = [[1.0, 1.0, 1.0], [1.0, 1.0, 1.0]];

    let d = cnv_mb_from_parts(&residual, &pv, &fvf, false);

    assert!((d.cnv[0] - 0.05).abs() < 1e-12);
    assert!((d.violating_pv_fraction - 0.01).abs() < 1e-12);
    assert!(d.pv_rule_relaxes);
    assert!(!d.would_accept_strict, "0.05 > strict 1e-2");
    // MB = 0.5/1000 = 5e-4 > 1e-7 → even the pv-relaxed accept must fail on MB.
    assert!(!d.would_accept);

    // Shrink the residual so MB passes while local CNV still violates strict:
    // water residual 2e-4 on pv=10 → CNV 2e-5... too small. Use pv weighting instead:
    // keep CNV at 0.05 but cancel MB with an opposite residual in the big cell.
    let residual = DVector::from_vec(vec![
        0.5, 0.0, 0.0, //
        -0.5, 0.0, 0.0, // cancels the sum → MB = 0 exactly
    ]);
    let d = cnv_mb_from_parts(&residual, &pv, &fvf, false);
    assert_eq!(d.mb[0], 0.0);
    assert!(!d.would_accept_strict, "local CNV 0.05 still above strict");
    assert!(
        d.would_accept,
        "1% violating PV < 3% rule → relaxed CNV 1.0 applies, MB clean → OPM accepts"
    );
}

#[test]
fn cnv_mb_relax_final_iteration_applies_relaxed_tiers_unconditionally() {
    // A state that fails BOTH strict tolerances and the PV-rule tier (>3% of PV
    // violating strict CNV) — the pv-relaxed path alone must not accept it. Both cells
    // have CNV 25/500 = 0.05 (> strict 1e-2); residuals cancel so MB = 0.
    let residual = DVector::from_vec(vec![25.0, 0.0, 0.0, -25.0, 0.0, 0.0]);
    let pv = [500.0, 500.0];
    let fvf = [[1.0, 1.0, 1.0], [1.0, 1.0, 1.0]];

    let not_final = cnv_mb_from_parts(&residual, &pv, &fvf, false);
    assert!(!not_final.pv_rule_relaxes, "100% violating PV >> 3% rule");
    assert!(
        !not_final.would_accept,
        "CNV 0.05 > strict 1e-2, PV rule doesn't apply"
    );

    let final_iter = cnv_mb_from_parts(&residual, &pv, &fvf, true);
    assert!(
        final_iter.would_accept,
        "relax_final_iteration unconditionally applies CNV-relaxed(1.0)/MB-relaxed(1e-6), \
             independent of the PV rule"
    );
}

#[test]
fn zero_residual_scaffold_converges_in_one_newton_step() {
    let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
    let state = FimState::from_simulator(&sim);

    let report = run_fim_timestep(&mut sim, &state, &state, 1.0, &FimNewtonOptions::default());

    assert!(report.converged);
    assert_eq!(report.newton_iterations, 1);
    assert_eq!(report.retry_factor, 1.0);
    assert!(report.final_residual_inf_norm <= 1e-12);
    assert!(report.final_material_balance_inf_norm <= 1e-12);
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
        &mut sim,
        &previous_state,
        &iterate,
        1.0,
        &FimNewtonOptions::default(),
    );

    assert!(report.converged);
    assert!(
        (report.accepted_state.cells[0].pressure_bar - previous_state.cells[0].pressure_bar).abs()
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
        &mut sim,
        &previous_state,
        &previous_state,
        1.0,
        &FimNewtonOptions::default(),
    );

    assert!(report.converged);
    assert_eq!(report.accepted_state.well_bhp.len(), 2);
    assert_eq!(report.accepted_state.perforation_primaries.len(), 2);
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
            assemble_residual_only: false,
            topology: None,
            flow_resv_context: None,
        },
    );
    let residual_norm = scaled_residual_inf_norm(&assembly.residual, &assembly.equation_scaling);
    assert!(residual_norm.is_finite() && residual_norm > 0.0);

    let options = FimNewtonOptions {
        residual_tolerance: residual_norm * 0.75,
        ..FimNewtonOptions::default()
    };

    let report = run_fim_timestep(&mut sim, &previous_state, &previous_state, 0.01, &options);

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
fn iterate_has_material_change_detects_well_and_perforation_updates() {
    let previous_state = FimState {
        cells: vec![crate::fim::state::FimCellState {
            pressure_bar: 250.0,
            sw: 0.25,
            hydrocarbon_var: 0.0,
            regime: crate::fim::state::HydrocarbonState::Saturated,
        }],
        well_bhp: vec![300.0],
        perforation_primaries: vec![
            crate::fim::state::FimPerforationPrimary::reservoir_connection_q(-150.0),
        ],
    };

    let mut bhp_changed = previous_state.clone();
    bhp_changed.well_bhp[0] += 1.0;
    assert!(iterate_has_material_change(&previous_state, &bhp_changed));

    let mut perf_changed = previous_state.clone();
    perf_changed.perforation_primaries[0].value += 1.0;
    assert!(iterate_has_material_change(&previous_state, &perf_changed));
}

#[test]
fn stagnation_acceptance_requires_material_change() {
    let options = FimNewtonOptions::default();
    assert!(!stagnation_acceptance_allows(
        false,
        options.residual_tolerance * 2.0,
        options.material_balance_tolerance * 0.5,
        options.update_tolerance * 0.5,
        &options,
    ));
}

#[test]
fn stagnation_acceptance_allows_near_converged_state() {
    let options = FimNewtonOptions::default();
    assert!(stagnation_acceptance_allows(
        true,
        options.residual_tolerance * 6.0,
        options.material_balance_tolerance * 0.5,
        options.update_tolerance * 0.1,
        &options,
    ));
}

#[test]
fn zero_move_appleyard_acceptance_allows_guarded_unchanged_state() {
    let options = FimNewtonOptions::default();
    assert!(zero_move_appleyard_acceptance_allows(
        false,
        options.residual_tolerance * NOOP_ENTRY_EXACT_FACTOR * ENTRY_RESIDUAL_GUARD_FACTOR * 0.9,
        options.material_balance_tolerance
            * NOOP_ENTRY_EXACT_FACTOR
            * ENTRY_RESIDUAL_GUARD_FACTOR
            * 0.9,
        &options,
    ));
}

#[test]
fn zero_move_appleyard_acceptance_rejects_changed_or_out_of_band_state() {
    let options = FimNewtonOptions::default();
    assert!(!zero_move_appleyard_acceptance_allows(
        true,
        options.residual_tolerance * NOOP_ENTRY_EXACT_FACTOR * ENTRY_RESIDUAL_GUARD_FACTOR,
        options.material_balance_tolerance * NOOP_ENTRY_EXACT_FACTOR * ENTRY_RESIDUAL_GUARD_FACTOR,
        &options,
    ));
    assert!(!zero_move_appleyard_acceptance_allows(
        false,
        options.residual_tolerance * NOOP_ENTRY_EXACT_FACTOR * ENTRY_RESIDUAL_GUARD_FACTOR * 1.1,
        options.material_balance_tolerance * NOOP_ENTRY_EXACT_FACTOR * ENTRY_RESIDUAL_GUARD_FACTOR,
        &options,
    ));
    assert!(!zero_move_appleyard_acceptance_allows(
        false,
        options.residual_tolerance * NOOP_ENTRY_EXACT_FACTOR * ENTRY_RESIDUAL_GUARD_FACTOR,
        options.material_balance_tolerance
            * NOOP_ENTRY_EXACT_FACTOR
            * ENTRY_RESIDUAL_GUARD_FACTOR
            * 1.1,
        &options,
    ));
}

#[test]
fn stagnation_acceptance_rejects_material_balance_failure() {
    let options = FimNewtonOptions::default();
    assert!(!stagnation_acceptance_allows(
        true,
        options.residual_tolerance * 6.0,
        options.material_balance_tolerance * 2.0,
        options.update_tolerance * 0.1,
        &options,
    ));
}

#[test]
fn stagnation_acceptance_gate_status_reports_update_failure() {
    let options = FimNewtonOptions::default();
    let status = stagnation_acceptance_gate_status(
        true,
        options.residual_tolerance * 6.0,
        options.material_balance_tolerance * 0.5,
        options.update_tolerance * 1.5,
        &options,
    );

    assert!(status.materially_changed);
    assert!(!status.update_ok);
    assert!(status.residual_ok);
    assert!(status.material_balance_ok);
    assert!(!status.allows());
}

#[test]
fn stagnation_acceptance_gate_trace_marks_rejected_limits() {
    let options = FimNewtonOptions::default();
    let status = stagnation_acceptance_gate_status(
        true,
        options.residual_tolerance * 12.0,
        options.material_balance_tolerance * 2.0,
        options.update_tolerance * 1.5,
        &options,
    );

    let trace = stagnation_acceptance_gate_trace(
        status,
        options.residual_tolerance * 12.0,
        options.material_balance_tolerance * 2.0,
        options.update_tolerance * 1.5,
        &options,
    );

    assert!(trace.contains("upd="));
    assert!(trace.contains("res="));
    assert!(trace.contains("mb="));
    assert!(trace.contains("reject"));
}

#[test]
fn guard_band_keeps_material_balance_limit_strict() {
    let options = FimNewtonOptions::default();
    let (residual_limit, material_balance_limit) = convergence_limits(&options, true);

    assert_eq!(
        residual_limit,
        options.residual_tolerance * ENTRY_RESIDUAL_GUARD_FACTOR
    );
    assert_eq!(material_balance_limit, options.material_balance_tolerance);
}

#[test]
fn accepted_state_convergence_rejects_guard_band_material_balance_violation() {
    let diagnostics = AcceptedStateConvergenceDiagnostics {
        state: FimState {
            cells: Vec::new(),
            well_bhp: Vec::new(),
            perforation_primaries: Vec::new(),
        },
        residual_inf_norm: 1.5e-5,
        residual_diagnostics: ResidualFamilyDiagnostics {
            water: ResidualFamilyPeak {
                family: ResidualRowFamily::Water,
                scaled_value: 1.5e-5,
                row: 0,
                item_index: 0,
            },
            oil_component: ResidualFamilyPeak {
                family: ResidualRowFamily::OilComponent,
                scaled_value: 1.0e-5,
                row: 1,
                item_index: 0,
            },
            gas_component: ResidualFamilyPeak {
                family: ResidualRowFamily::GasComponent,
                scaled_value: 0.5e-5,
                row: 2,
                item_index: 0,
            },
            well_constraint: None,
            perforation_flow: None,
            global: ResidualFamilyPeak {
                family: ResidualRowFamily::Water,
                scaled_value: 1.5e-5,
                row: 0,
                item_index: 0,
            },
        },
        residual_detail: None,
        material_balance_inf_norm: 1.5e-5,
        material_balance_diagnostics: GlobalMaterialBalanceDiagnostics {
            water: 1.5e-5,
            oil_component: 1.0e-5,
            gas_component: 0.5e-5,
            global_family: ResidualRowFamily::Water,
            global_value: 1.5e-5,
        },
    };
    let options = FimNewtonOptions::default();
    let (residual_limit, material_balance_limit) = convergence_limits(&options, true);

    assert!(diagnostics.residual_inf_norm <= residual_limit);
    assert!(diagnostics.material_balance_inf_norm > material_balance_limit);
    assert!(!accepted_state_meets_convergence(
        &diagnostics,
        residual_limit,
        material_balance_limit,
    ));
}

#[test]
fn scaled_update_peak_reports_dominant_family() {
    let update = DVector::from_vec(vec![2.0, 0.1, 0.05, 30.0, 0.2]);
    let scaling = crate::fim::scaling::VariableScaling {
        pressure: vec![100.0],
        sw: vec![1.0],
        hydrocarbon_var: vec![1.0],
        well_bhp: vec![1000.0],
        perforation_rate: vec![1.0],
    };

    let peak = scaled_update_peak(&update, &scaling);

    assert_eq!(peak.family, UpdateVariableFamily::PerforationRate);
    assert!((peak.scaled_value - 0.2).abs() < 1e-12);
    assert_eq!(peak.row, 4);
    assert_eq!(peak.item_index, 0);
}

#[test]
fn scaled_applied_update_peak_reports_effective_family() {
    let state = FimState {
        cells: vec![crate::fim::state::FimCellState {
            pressure_bar: 200.0,
            sw: 0.1,
            hydrocarbon_var: 80.0,
            regime: crate::fim::state::HydrocarbonState::Undersaturated,
        }],
        well_bhp: vec![150.0],
        perforation_primaries: vec![
            crate::fim::state::FimPerforationPrimary::reservoir_connection_q(10.0),
        ],
    };

    let candidate = FimState {
        cells: vec![crate::fim::state::FimCellState {
            pressure_bar: 200.5,
            sw: 0.11,
            hydrocarbon_var: 80.0,
            regime: crate::fim::state::HydrocarbonState::Undersaturated,
        }],
        well_bhp: vec![150.0],
        perforation_primaries: vec![
            crate::fim::state::FimPerforationPrimary::reservoir_connection_q(10.2),
        ],
    };

    let scaling = crate::fim::scaling::VariableScaling {
        pressure: vec![100.0],
        sw: vec![1.0],
        hydrocarbon_var: vec![100.0],
        well_bhp: vec![1000.0],
        perforation_rate: vec![100.0],
    };

    let peak = scaled_applied_update_peak(&state, &candidate, &scaling);

    assert_eq!(peak.family, UpdateVariableFamily::WaterSaturation);
    assert!((peak.scaled_value - 0.01).abs() < 1e-12);
    assert_eq!(peak.row, 1);
    assert_eq!(peak.item_index, 0);
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
    assert_eq!(
        diagnostics.well_constraint.expect("well peak").item_index,
        1
    );
    assert!((diagnostics.well_constraint.expect("well peak").scaled_value - 8.0).abs() < 1e-12);
    assert_eq!(
        diagnostics.perforation_flow.expect("perf peak").item_index,
        0
    );
    assert!(
        (diagnostics
            .perforation_flow
            .expect("perf peak")
            .scaled_value
            - 0.5)
            .abs()
            < 1e-12
    );
    assert_eq!(diagnostics.global.family, ResidualRowFamily::WellConstraint);
    assert_eq!(diagnostics.global.row, 7);
    assert_eq!(diagnostics.global.item_index, 1);
    assert!((diagnostics.global.scaled_value - 8.0).abs() < 1e-12);
}

#[test]
fn global_material_balance_diagnostics_normalizes_component_sums() {
    let residual = DVector::from_vec(vec![1.0, -4.0, 9.0, 3.0, 6.0, -3.0, 50.0, -20.0, 7.0]);
    let scaling = EquationScaling {
        water: vec![10.0, 10.0],
        oil_component: vec![10.0, 10.0],
        gas_component: vec![10.0, 10.0],
        well_constraint: vec![5.0, 5.0],
        perforation_flow: vec![2.0],
    };

    let diagnostics = global_material_balance_diagnostics(&residual, &scaling);

    assert!((diagnostics.water - 0.2).abs() < 1e-12);
    assert!((diagnostics.oil_component - 0.1).abs() < 1e-12);
    assert!((diagnostics.gas_component - 0.3).abs() < 1e-12);
    assert_eq!(diagnostics.global_family, ResidualRowFamily::GasComponent);
    assert!((diagnostics.global_value - 0.3).abs() < 1e-12);
}

#[test]
fn failure_classification_marks_clean_cpr_failure_as_nonlinear_bad() {
    let diagnostics = ResidualFamilyDiagnostics {
        water: ResidualFamilyPeak {
            family: ResidualRowFamily::Water,
            scaled_value: 1.0,
            row: 0,
            item_index: 0,
        },
        oil_component: ResidualFamilyPeak {
            family: ResidualRowFamily::OilComponent,
            scaled_value: 0.5,
            row: 1,
            item_index: 0,
        },
        gas_component: ResidualFamilyPeak {
            family: ResidualRowFamily::GasComponent,
            scaled_value: 0.25,
            row: 2,
            item_index: 0,
        },
        well_constraint: None,
        perforation_flow: None,
        global: ResidualFamilyPeak {
            family: ResidualRowFamily::Water,
            scaled_value: 1.0,
            row: 0,
            item_index: 0,
        },
    };
    let report = FimLinearSolveReport {
        solution: DVector::zeros(1),
        converged: true,
        iterations: 12,
        rhs_norm: 1.0,
        final_residual_norm: 1e-12,
        failure_diagnostics: None,
        used_fallback: false,
        backend_used: FimLinearSolverKind::FgmresCpr,
        cpr_diagnostics: Some(crate::fim::linear::FimCprDiagnostics {
            coarse_rows: 10,
            coarse_solver: crate::fim::linear::FimPressureCoarseSolverKind::ExactDense,
            smoother_label: "ilu0",
            coarse_applications: 4,
            average_reduction_ratio: 1e-12,
            last_reduction_ratio: 1e-12,
            build_timing: None,
        }),
        total_time_ms: 0.0,
        preconditioner_build_time_ms: 0.0,
    };

    let classified = classify_retry_failure(Some(&report), &diagnostics);

    assert_eq!(classified.class, FimRetryFailureClass::NonlinearBad);
    assert_eq!(classified.dominant_family_label, "water");
    assert_eq!(classified.cpr_last_reduction_ratio, Some(1e-12));
}

#[test]
fn failure_classification_marks_direct_backend_as_nonlinear_bad_when_clean() {
    let diagnostics = ResidualFamilyDiagnostics {
        water: ResidualFamilyPeak {
            family: ResidualRowFamily::Water,
            scaled_value: 1.0,
            row: 0,
            item_index: 0,
        },
        oil_component: ResidualFamilyPeak {
            family: ResidualRowFamily::OilComponent,
            scaled_value: 0.5,
            row: 1,
            item_index: 0,
        },
        gas_component: ResidualFamilyPeak {
            family: ResidualRowFamily::GasComponent,
            scaled_value: 0.25,
            row: 2,
            item_index: 0,
        },
        well_constraint: None,
        perforation_flow: None,
        global: ResidualFamilyPeak {
            family: ResidualRowFamily::Water,
            scaled_value: 1.0,
            row: 0,
            item_index: 0,
        },
    };
    let report = FimLinearSolveReport {
        solution: DVector::zeros(1),
        converged: true,
        iterations: 3,
        rhs_norm: 1.0,
        final_residual_norm: 1e-12,
        failure_diagnostics: None,
        used_fallback: false,
        backend_used: FimLinearSolverKind::DenseLuDebug,
        cpr_diagnostics: None,
        total_time_ms: 0.0,
        preconditioner_build_time_ms: 0.0,
    };

    let classified = classify_retry_failure(Some(&report), &diagnostics);

    assert_eq!(classified.class, FimRetryFailureClass::NonlinearBad);
    assert!(!classified.used_linear_fallback);
}

#[test]
fn failure_classification_marks_weak_cpr_as_mixed() {
    let diagnostics = ResidualFamilyDiagnostics {
        water: ResidualFamilyPeak {
            family: ResidualRowFamily::Water,
            scaled_value: 1.0,
            row: 0,
            item_index: 0,
        },
        oil_component: ResidualFamilyPeak {
            family: ResidualRowFamily::OilComponent,
            scaled_value: 0.5,
            row: 1,
            item_index: 0,
        },
        gas_component: ResidualFamilyPeak {
            family: ResidualRowFamily::GasComponent,
            scaled_value: 0.25,
            row: 2,
            item_index: 0,
        },
        well_constraint: None,
        perforation_flow: None,
        global: ResidualFamilyPeak {
            family: ResidualRowFamily::Water,
            scaled_value: 1.0,
            row: 0,
            item_index: 0,
        },
    };
    let report = FimLinearSolveReport {
        solution: DVector::zeros(1),
        converged: true,
        iterations: 12,
        rhs_norm: 1.0,
        final_residual_norm: 1e-12,
        failure_diagnostics: None,
        used_fallback: false,
        backend_used: FimLinearSolverKind::FgmresCpr,
        cpr_diagnostics: Some(crate::fim::linear::FimCprDiagnostics {
            coarse_rows: 10,
            coarse_solver: crate::fim::linear::FimPressureCoarseSolverKind::ExactDense,
            smoother_label: "block-jacobi",
            coarse_applications: 4,
            average_reduction_ratio: 0.6,
            last_reduction_ratio: 0.8,
            build_timing: None,
        }),
        total_time_ms: 0.0,
        preconditioner_build_time_ms: 0.0,
    };

    let classified = classify_retry_failure(Some(&report), &diagnostics);

    assert_eq!(classified.class, FimRetryFailureClass::Mixed);
    assert_eq!(classified.cpr_average_reduction_ratio, Some(0.6));
    assert_eq!(classified.cpr_last_reduction_ratio, Some(0.8));
}

#[test]
fn failure_classification_marks_converged_fallback_path_as_nonlinear_bad() {
    let diagnostics = ResidualFamilyDiagnostics {
        water: ResidualFamilyPeak {
            family: ResidualRowFamily::Water,
            scaled_value: 0.1,
            row: 0,
            item_index: 0,
        },
        oil_component: ResidualFamilyPeak {
            family: ResidualRowFamily::OilComponent,
            scaled_value: 1.0,
            row: 1,
            item_index: 0,
        },
        gas_component: ResidualFamilyPeak {
            family: ResidualRowFamily::GasComponent,
            scaled_value: 0.0,
            row: 2,
            item_index: 0,
        },
        well_constraint: None,
        perforation_flow: None,
        global: ResidualFamilyPeak {
            family: ResidualRowFamily::OilComponent,
            scaled_value: 1.0,
            row: 1,
            item_index: 0,
        },
    };
    let report = FimLinearSolveReport {
        solution: DVector::zeros(1),
        converged: true,
        iterations: 1,
        rhs_norm: 1.0,
        final_residual_norm: 1e-8,
        failure_diagnostics: None,
        used_fallback: true,
        backend_used: FimLinearSolverKind::SparseLuDebug,
        cpr_diagnostics: None,
        total_time_ms: 0.0,
        preconditioner_build_time_ms: 0.0,
    };

    let classified = classify_retry_failure(Some(&report), &diagnostics);

    assert_eq!(classified.class, FimRetryFailureClass::NonlinearBad);
    assert!(classified.used_linear_fallback);
}

#[test]
fn restart_stagnation_fallback_streak_only_accumulates_matching_failures() {
    let streak =
        next_restart_stagnation_fallback_streak(0, Some(FimLinearFailureReason::RestartStagnation));
    let streak = next_restart_stagnation_fallback_streak(
        streak,
        Some(FimLinearFailureReason::RestartStagnation),
    );
    let reset = next_restart_stagnation_fallback_streak(
        streak,
        Some(FimLinearFailureReason::DeadStateDetected),
    );

    assert_eq!(streak, 2);
    assert_eq!(reset, 0);
}

#[test]
fn restart_stagnation_direct_bypass_requires_two_consecutive_fallbacks() {
    assert!(!should_enable_restart_stagnation_direct_bypass(1));
    assert!(should_enable_restart_stagnation_direct_bypass(2));
}

#[test]
fn zero_move_fallback_direct_bypass_uses_existing_effective_move_floor() {
    assert!(should_enable_zero_move_fallback_direct_bypass(
        true, 0.0049, 0.000049,
    ));
    assert!(!should_enable_zero_move_fallback_direct_bypass(
        false, 0.0049, 0.000049,
    ));
    assert!(!should_enable_zero_move_fallback_direct_bypass(
        true, 0.0051, 0.000049,
    ));
    assert!(!should_enable_zero_move_fallback_direct_bypass(
        true, 0.0049, 0.000051,
    ));
}

#[test]
fn repeated_zero_move_direct_bypass_groups_nearby_non_gas_cells_in_same_layer() {
    let mut sim = ReservoirSimulator::new(20, 20, 3, 0.2);
    sim.add_well(0, 0, 0, 400.0, 0.1, 0.0, true)
        .expect("injector");
    sim.add_well(19, 19, 0, 50.0, 0.1, 0.0, false)
        .expect("producer");
    let peak = ResidualFamilyPeak {
        family: ResidualRowFamily::OilComponent,
        scaled_value: 0.99,
        row: 250,
        item_index: sim.idx(3, 4, 0),
    };
    let diagnostics = ResidualFamilyDiagnostics {
        water: peak,
        oil_component: peak,
        gas_component: peak,
        well_constraint: None,
        perforation_flow: None,
        global: peak,
    };

    assert!(should_enable_repeated_zero_move_direct_bypass(
        &sim,
        Some(FimHotspotSite::Cell(sim.idx(3, 3, 0))),
        &diagnostics,
        FimHotspotSite::Cell(sim.idx(3, 4, 0)),
    ));
}

#[test]
fn repeated_zero_move_direct_bypass_does_not_group_vertical_non_gas_shift() {
    let mut sim = ReservoirSimulator::new(20, 20, 3, 0.2);
    sim.add_well(0, 0, 0, 400.0, 0.1, 0.0, true)
        .expect("injector");
    sim.add_well(19, 19, 0, 50.0, 0.1, 0.0, false)
        .expect("producer");
    let peak = ResidualFamilyPeak {
        family: ResidualRowFamily::OilComponent,
        scaled_value: 0.99,
        row: 1390,
        item_index: sim.idx(3, 3, 1),
    };
    let diagnostics = ResidualFamilyDiagnostics {
        water: peak,
        oil_component: peak,
        gas_component: peak,
        well_constraint: None,
        perforation_flow: None,
        global: peak,
    };

    assert!(!should_enable_repeated_zero_move_direct_bypass(
        &sim,
        Some(FimHotspotSite::Cell(sim.idx(3, 3, 0))),
        &diagnostics,
        FimHotspotSite::Cell(sim.idx(3, 3, 1)),
    ));
}

#[test]
fn near_converged_iterative_accept_requires_small_outer_and_bounded_candidate_worsening() {
    let report = FimLinearSolveReport {
        solution: DVector::zeros(1),
        converged: false,
        iterations: 80,
        rhs_norm: 1.0,
        final_residual_norm: 1.0e-6,
        failure_diagnostics: Some(crate::fim::linear::FimLinearFailureDiagnostics {
            reason: FimLinearFailureReason::RestartStagnation,
            tolerance: 1.0e-7,
            rhs_norm: 1.0,
            outer_residual_norm: 1.2e-6,
            preconditioned_residual_norm: Some(1.1e-6),
            estimated_residual_norm: Some(1.0e-8),
            candidate_residual_norm: Some(4.0e-6),
            restart_diagnostics: vec![crate::fim::linear::FimLinearRestartDiagnostics {
                restart_index: 3,
                start_iteration: 60,
                end_iteration: 80,
                inner_steps: 20,
                outer_residual_norm: 1.2e-6,
                preconditioned_residual_norm: 1.1e-6,
                best_estimated_residual_norm: Some(1.0e-8),
                best_candidate_residual_norm: Some(4.0e-6),
                solution_improved: true,
            }],
        }),
        used_fallback: false,
        backend_used: FimLinearSolverKind::FgmresCpr,
        cpr_diagnostics: None,
        total_time_ms: 0.0,
        preconditioner_build_time_ms: 0.0,
    };

    assert!(should_accept_near_converged_iterative_step(&report));
}

#[test]
fn near_converged_iterative_accept_rejects_large_outer_tail() {
    let report = FimLinearSolveReport {
        solution: DVector::zeros(1),
        converged: false,
        iterations: 80,
        rhs_norm: 1.0,
        final_residual_norm: 1.0e-6,
        failure_diagnostics: Some(crate::fim::linear::FimLinearFailureDiagnostics {
            reason: FimLinearFailureReason::MaxIterations,
            tolerance: 1.0e-7,
            rhs_norm: 1.0,
            outer_residual_norm: 3.0e-6,
            preconditioned_residual_norm: Some(3.0e-6),
            estimated_residual_norm: Some(1.0e-8),
            candidate_residual_norm: Some(4.0e-6),
            restart_diagnostics: vec![crate::fim::linear::FimLinearRestartDiagnostics {
                restart_index: 3,
                start_iteration: 60,
                end_iteration: 80,
                inner_steps: 20,
                outer_residual_norm: 3.0e-6,
                preconditioned_residual_norm: 3.0e-6,
                best_estimated_residual_norm: Some(1.0e-8),
                best_candidate_residual_norm: Some(4.0e-6),
                solution_improved: true,
            }],
        }),
        used_fallback: false,
        backend_used: FimLinearSolverKind::FgmresCpr,
        cpr_diagnostics: None,
        total_time_ms: 0.0,
        preconditioner_build_time_ms: 0.0,
    };

    assert!(!should_accept_near_converged_iterative_step(&report));
}

#[test]
fn failure_classification_keeps_nonconverged_fallback_path_linear_bad() {
    let diagnostics = ResidualFamilyDiagnostics {
        water: ResidualFamilyPeak {
            family: ResidualRowFamily::Water,
            scaled_value: 0.5,
            row: 0,
            item_index: 0,
        },
        oil_component: ResidualFamilyPeak {
            family: ResidualRowFamily::OilComponent,
            scaled_value: 1.0,
            row: 1,
            item_index: 0,
        },
        gas_component: ResidualFamilyPeak {
            family: ResidualRowFamily::GasComponent,
            scaled_value: 0.0,
            row: 2,
            item_index: 0,
        },
        well_constraint: None,
        perforation_flow: None,
        global: ResidualFamilyPeak {
            family: ResidualRowFamily::OilComponent,
            scaled_value: 1.0,
            row: 1,
            item_index: 0,
        },
    };
    let report = FimLinearSolveReport {
        solution: DVector::zeros(1),
        converged: false,
        iterations: 1,
        rhs_norm: 1.0,
        final_residual_norm: 1e-2,
        failure_diagnostics: None,
        used_fallback: true,
        backend_used: FimLinearSolverKind::DenseLuDebug,
        cpr_diagnostics: None,
        total_time_ms: 0.0,
        preconditioner_build_time_ms: 0.0,
    };

    let classified = classify_retry_failure(Some(&report), &diagnostics);

    assert_eq!(classified.class, FimRetryFailureClass::LinearBad);
    assert!(classified.used_linear_fallback);
}

#[test]
fn repeated_nonlinear_hotspot_streak_groups_phase_rows_by_cell_site() {
    let mut sim = ReservoirSimulator::new(20, 20, 3, 0.2);
    sim.add_well(0, 0, 0, 400.0, 0.1, 0.0, true)
        .expect("injector");
    sim.add_well(19, 19, 0, 50.0, 0.1, 0.0, false)
        .expect("producer");
    let current_peak = ResidualFamilyPeak {
        family: ResidualRowFamily::OilComponent,
        scaled_value: 0.98,
        row: 430,
        item_index: 143,
    };
    let current = ResidualFamilyDiagnostics {
        water: current_peak,
        oil_component: current_peak,
        gas_component: current_peak,
        well_constraint: None,
        perforation_flow: None,
        global: current_peak,
    };

    let streak = repeated_nonlinear_hotspot_streak(
        &sim,
        Some(FimHotspotSite::Cell(143)),
        1.0,
        &current,
        FimHotspotSite::Cell(143),
        0.99,
        0,
    );

    assert_eq!(streak, 1);
}

#[test]
fn repeated_nonlinear_hotspot_streak_resets_after_strong_progress() {
    let mut sim = ReservoirSimulator::new(20, 20, 3, 0.2);
    sim.add_well(0, 0, 0, 400.0, 0.1, 0.0, true)
        .expect("injector");
    sim.add_well(19, 19, 0, 50.0, 0.1, 0.0, false)
        .expect("producer");
    let peak = ResidualFamilyPeak {
        family: ResidualRowFamily::Water,
        scaled_value: 1.0,
        row: 429,
        item_index: 143,
    };
    let diagnostics = ResidualFamilyDiagnostics {
        water: peak,
        oil_component: peak,
        gas_component: peak,
        well_constraint: None,
        perforation_flow: None,
        global: peak,
    };

    let streak = repeated_nonlinear_hotspot_streak(
        &sim,
        Some(FimHotspotSite::Cell(143)),
        1.0,
        &diagnostics,
        FimHotspotSite::Cell(143),
        0.5,
        2,
    );

    assert_eq!(streak, 0);
}

#[test]
fn repeated_nonlinear_hotspot_streak_relaxes_threshold_for_gas_hotspot_site() {
    let mut sim = ReservoirSimulator::new(10, 10, 3, 0.2);
    sim.add_well(0, 0, 0, 400.0, 0.1, 0.0, true)
        .expect("injector");
    sim.add_well(9, 9, 2, 50.0, 0.1, 0.0, false)
        .expect("producer");
    let current_peak = ResidualFamilyPeak {
        family: ResidualRowFamily::GasComponent,
        scaled_value: 0.91,
        row: 92,
        item_index: 30,
    };
    let current = ResidualFamilyDiagnostics {
        water: current_peak,
        oil_component: current_peak,
        gas_component: current_peak,
        well_constraint: None,
        perforation_flow: None,
        global: current_peak,
    };

    let streak = repeated_nonlinear_hotspot_streak(
        &sim,
        Some(FimHotspotSite::Cell(30)),
        1.0e-4,
        &current,
        FimHotspotSite::Cell(30),
        9.1e-5,
        0,
    );

    assert_eq!(streak, 1);
}

#[test]
fn repeated_nonlinear_hotspot_streak_keeps_stricter_threshold_for_non_gas_sites() {
    let mut sim = ReservoirSimulator::new(20, 20, 3, 0.2);
    sim.add_well(0, 0, 0, 400.0, 0.1, 0.0, true)
        .expect("injector");
    sim.add_well(19, 19, 0, 50.0, 0.1, 0.0, false)
        .expect("producer");
    let current_peak = ResidualFamilyPeak {
        family: ResidualRowFamily::Water,
        scaled_value: 0.91,
        row: 429,
        item_index: 143,
    };
    let current = ResidualFamilyDiagnostics {
        water: current_peak,
        oil_component: current_peak,
        gas_component: current_peak,
        well_constraint: None,
        perforation_flow: None,
        global: current_peak,
    };

    let streak = repeated_nonlinear_hotspot_streak(
        &sim,
        Some(FimHotspotSite::Cell(143)),
        1.0e-4,
        &current,
        FimHotspotSite::Cell(143),
        9.1e-5,
        0,
    );

    assert_eq!(streak, 0);
}

#[test]
fn repeated_nonlinear_hotspot_streak_groups_nearby_non_gas_cells_in_same_layer() {
    let mut sim = ReservoirSimulator::new(20, 20, 3, 0.2);
    sim.add_well(0, 0, 0, 400.0, 0.1, 0.0, true)
        .expect("injector");
    sim.add_well(19, 19, 0, 50.0, 0.1, 0.0, false)
        .expect("producer");
    let peak = ResidualFamilyPeak {
        family: ResidualRowFamily::OilComponent,
        scaled_value: 0.99,
        row: 250,
        item_index: sim.idx(3, 4, 0),
    };
    let diagnostics = ResidualFamilyDiagnostics {
        water: peak,
        oil_component: peak,
        gas_component: peak,
        well_constraint: None,
        perforation_flow: None,
        global: peak,
    };

    let streak = repeated_nonlinear_hotspot_streak(
        &sim,
        Some(FimHotspotSite::Cell(sim.idx(3, 3, 0))),
        1.0,
        &diagnostics,
        FimHotspotSite::Cell(sim.idx(3, 4, 0)),
        0.99,
        0,
    );

    assert_eq!(streak, 1);
}

#[test]
fn repeated_nonlinear_hotspot_streak_does_not_group_vertical_non_gas_shift() {
    let mut sim = ReservoirSimulator::new(20, 20, 3, 0.2);
    sim.add_well(0, 0, 0, 400.0, 0.1, 0.0, true)
        .expect("injector");
    sim.add_well(19, 19, 0, 50.0, 0.1, 0.0, false)
        .expect("producer");
    let peak = ResidualFamilyPeak {
        family: ResidualRowFamily::OilComponent,
        scaled_value: 0.99,
        row: 1390,
        item_index: sim.idx(3, 3, 1),
    };
    let diagnostics = ResidualFamilyDiagnostics {
        water: peak,
        oil_component: peak,
        gas_component: peak,
        well_constraint: None,
        perforation_flow: None,
        global: peak,
    };

    let streak = repeated_nonlinear_hotspot_streak(
        &sim,
        Some(FimHotspotSite::Cell(sim.idx(3, 3, 0))),
        1.0,
        &diagnostics,
        FimHotspotSite::Cell(sim.idx(3, 3, 1)),
        0.99,
        0,
    );

    assert_eq!(streak, 0);
}

#[test]
fn gas_injector_symmetry_site_groups_axis_swapped_cells() {
    let mut sim = ReservoirSimulator::new(10, 10, 3, 0.2);
    sim.add_well(0, 0, 0, 400.0, 0.1, 0.0, true)
        .expect("injector");
    sim.add_well(9, 9, 2, 50.0, 0.1, 0.0, false)
        .expect("producer");
    let topology = build_well_topology(&sim);

    let east_site = gas_injector_symmetry_site(&sim, &topology, sim.idx(3, 0, 0));
    let north_site = gas_injector_symmetry_site(&sim, &topology, sim.idx(0, 3, 0));

    assert_eq!(east_site, north_site);
    assert_eq!(
        east_site,
        Some(FimHotspotSite::GasInjectorSymmetry {
            injector_well_index: 0,
            major_offset: 3,
            minor_offset: 0,
            vertical_offset: 0,
        })
    );
}

#[test]
fn detect_oscillation_flags_single_phase_two_step_relative_change() {
    // Water swings back close to its value from 2 iterations ago (small d1) while
    // having moved a lot 1 iteration ago (large d2) -> classic oscillation signature.
    let current = PerFamilyNorms {
        water: 1.0,
        oil_component: 1.0,
        gas_component: 1.0,
        well_constraint: 1.0,
        perforation_flow: 1.0,
    };
    let prev1 = PerFamilyNorms {
        water: 2.0,
        oil_component: 1.0,
        gas_component: 1.0,
        well_constraint: 1.0,
        perforation_flow: 1.0,
    };
    let prev2 = PerFamilyNorms {
        water: 1.01,
        oil_component: 1.0,
        gas_component: 1.0,
        well_constraint: 1.0,
        perforation_flow: 1.0,
    };
    assert_eq!(detect_oscillation(current, prev1, prev2), 1);
}

#[test]
fn detect_oscillation_flags_perforation_flow_two_step_relative_change() {
    // Matches the measured heavy-case pattern: perforation_flow alternates while the cell
    // families stay flat (`FIM-NEWTON-006`).
    let current = PerFamilyNorms {
        water: 1.0,
        oil_component: 1.0,
        gas_component: 1.0,
        perforation_flow: 2.137e-5,
        well_constraint: 1.0,
    };
    let prev1 = PerFamilyNorms {
        water: 1.0,
        oil_component: 1.0,
        gas_component: 1.0,
        perforation_flow: 3.419e-5,
        well_constraint: 1.0,
    };
    let prev2 = PerFamilyNorms {
        water: 1.0,
        oil_component: 1.0,
        gas_component: 1.0,
        perforation_flow: 2.137e-5,
        well_constraint: 1.0,
    };
    assert_eq!(detect_oscillation(current, prev1, prev2), 1);
}

#[test]
fn detect_oscillation_ignores_missing_well_and_perforation_families() {
    // No wells/perforations in this system: both default to infinity (from_diagnostics'
    // `None` mapping) and must never register as oscillating.
    let missing = PerFamilyNorms {
        water: 1.0,
        oil_component: 1.0,
        gas_component: 1.0,
        well_constraint: f64::INFINITY,
        perforation_flow: f64::INFINITY,
    };
    assert_eq!(detect_oscillation(missing, missing, missing), 0);
}

#[test]
fn detect_oscillation_requires_below_two_step_above_one_step_threshold() {
    // Monotonic decrease (no oscillation): d1 and d2 both large, but d1 is NOT < tol.
    let current = PerFamilyNorms {
        water: 1.0,
        oil_component: 1.0,
        gas_component: 1.0,
        well_constraint: 1.0,
        perforation_flow: 1.0,
    };
    let prev1 = PerFamilyNorms {
        water: 2.0,
        oil_component: 1.0,
        gas_component: 1.0,
        well_constraint: 1.0,
        perforation_flow: 1.0,
    };
    let prev2 = PerFamilyNorms {
        water: 4.0,
        oil_component: 1.0,
        gas_component: 1.0,
        well_constraint: 1.0,
        perforation_flow: 1.0,
    };
    assert_eq!(detect_oscillation(current, prev1, prev2), 0);

    // Steady state (both d1 and d2 tiny): not oscillating either.
    let steady = PerFamilyNorms {
        water: 1.0,
        oil_component: 1.0,
        gas_component: 1.0,
        well_constraint: 1.0,
        perforation_flow: 1.0,
    };
    assert_eq!(detect_oscillation(steady, steady, steady), 0);
}

#[test]
fn next_relaxation_factor_floors_at_newton_max_relax() {
    let mut relax = 1.0;
    for _ in 0..20 {
        relax = next_relaxation_factor(relax, 1);
    }
    assert!((relax - OSCILLATION_MAX_RELAX_FLOOR).abs() < 1e-12);
}

#[test]
fn next_relaxation_factor_holds_when_not_oscillating() {
    assert!((next_relaxation_factor(0.7, 0) - 0.7).abs() < 1e-12);
    // One decrement step from full relaxation.
    assert!((next_relaxation_factor(1.0, 1) - 0.9).abs() < 1e-12);
}

#[test]
fn appleyard_and_oscillation_relaxation_compose_via_min() {
    // No history-stabilization cap active; Appleyard is tighter than relaxation.
    assert!((compose_damping(0.3, None, 0.8) - 0.3).abs() < 1e-12);
    // Oscillation relaxation is tighter than Appleyard.
    assert!((compose_damping(0.9, None, 0.5) - 0.5).abs() < 1e-12);
    // History-stabilization cap is the tightest of all three.
    assert!((compose_damping(0.9, Some(0.25), 0.5) - 0.25).abs() < 1e-12);
    // All three at 1.0 (nothing binding) -> 1.0.
    assert!((compose_damping(1.0, None, 1.0) - 1.0).abs() < 1e-12);
}

#[test]
fn nonlinear_history_stabilization_caps_damping_for_repeated_weak_progress() {
    let peak = ResidualFamilyPeak {
        family: ResidualRowFamily::OilComponent,
        scaled_value: 1.0,
        row: 430,
        item_index: 143,
    };
    let diagnostics = ResidualFamilyDiagnostics {
        water: peak,
        oil_component: peak,
        gas_component: peak,
        well_constraint: None,
        perforation_flow: None,
        global: peak,
    };
    let report = FimLinearSolveReport {
        solution: DVector::zeros(1),
        converged: true,
        iterations: 6,
        rhs_norm: 1.0,
        final_residual_norm: 1e-12,
        failure_diagnostics: None,
        used_fallback: false,
        backend_used: FimLinearSolverKind::FgmresCpr,
        cpr_diagnostics: None,
        total_time_ms: 0.0,
        preconditioner_build_time_ms: 0.0,
    };

    let first = nonlinear_history_stabilization_decision(
        &report,
        &diagnostics,
        5e-5,
        &FimNewtonOptions::default(),
        1,
        FimHotspotSite::Cell(143),
    )
    .expect("expected first stabilization decision");
    let repeated = nonlinear_history_stabilization_decision(
        &report,
        &diagnostics,
        5e-5,
        &FimNewtonOptions::default(),
        2,
        FimHotspotSite::Cell(143),
    )
    .expect("expected repeated stabilization decision");

    assert_eq!(first.site, FimHotspotSite::Cell(143));
    assert!((first.damping_cap - 0.5).abs() < 1e-12);
    assert!((repeated.damping_cap - 0.25).abs() < 1e-12);

    assert!(
        nonlinear_history_stabilization_decision(
            &report,
            &diagnostics,
            1e-3,
            &FimNewtonOptions::default(),
            2,
            FimHotspotSite::Cell(143),
        )
        .is_none()
    );
}

#[test]
fn nonlinear_history_stabilization_allows_converged_fallback_path() {
    let peak = ResidualFamilyPeak {
        family: ResidualRowFamily::GasComponent,
        scaled_value: 1.0,
        row: 92,
        item_index: 30,
    };
    let diagnostics = ResidualFamilyDiagnostics {
        water: peak,
        oil_component: peak,
        gas_component: peak,
        well_constraint: None,
        perforation_flow: None,
        global: peak,
    };
    let report = FimLinearSolveReport {
        solution: DVector::zeros(1),
        converged: true,
        iterations: 1,
        rhs_norm: 1.0,
        final_residual_norm: 1e-12,
        failure_diagnostics: None,
        used_fallback: true,
        backend_used: FimLinearSolverKind::DenseLuDebug,
        cpr_diagnostics: None,
        total_time_ms: 0.0,
        preconditioner_build_time_ms: 0.0,
    };

    let decision = nonlinear_history_stabilization_decision(
        &report,
        &diagnostics,
        5e-5,
        &FimNewtonOptions::default(),
        1,
        FimHotspotSite::Cell(30),
    )
    .expect("expected stabilization after converged fallback");

    assert_eq!(decision.site, FimHotspotSite::Cell(30));
    assert!((decision.damping_cap - 0.5).abs() < 1e-12);
}

#[test]
fn appleyard_damping_limits_combined_oil_saturation_change() {
    let state = FimState {
        cells: vec![crate::fim::state::FimCellState {
            pressure_bar: 200.0,
            sw: 0.2,
            hydrocarbon_var: 0.2,
            regime: crate::fim::state::HydrocarbonState::Saturated,
        }],
        well_bhp: Vec::new(),
        perforation_primaries: Vec::new(),
    };
    let mut update = DVector::zeros(state.n_unknowns());
    update[1] = 0.15;
    update[2] = 0.15;

    let sim = ReservoirSimulator::new(1, 1, 1, 0.2);
    let damping = appleyard_damping_breakdown(&sim, &state, &update, &FimNewtonOptions::default())
        .final_damping;

    assert!((damping - (1.0 / 3.0)).abs() < 1e-12);
}

#[test]
fn move_is_below_effective_trace_threshold_detects_rounds_to_zero() {
    assert!(move_is_below_effective_trace_threshold(
        0.0049, 0.000049, 0.000049, 0.0
    ));
    assert!(!move_is_below_effective_trace_threshold(
        0.0051, 0.000049, 0.000049, 0.0
    ));
    assert!(!move_is_below_effective_trace_threshold(
        0.0049, 0.000051, 0.000049, 0.0
    ));
}

#[test]
fn local_cell_move_deltas_tracks_pressure_and_phase_changes() {
    let previous_state = FimState {
        cells: vec![crate::fim::state::FimCellState {
            pressure_bar: 200.0,
            sw: 0.2,
            hydrocarbon_var: 0.1,
            regime: crate::fim::state::HydrocarbonState::Saturated,
        }],
        well_bhp: Vec::new(),
        perforation_primaries: Vec::new(),
    };
    let candidate_state = FimState {
        cells: vec![crate::fim::state::FimCellState {
            pressure_bar: 200.004,
            sw: 0.20002,
            hydrocarbon_var: 0.10001,
            regime: crate::fim::state::HydrocarbonState::Saturated,
        }],
        well_bhp: Vec::new(),
        perforation_primaries: Vec::new(),
    };

    let (pressure_delta_bar, water_delta, oil_delta, gas_delta) =
        local_cell_move_deltas(&previous_state, &candidate_state, 0).expect("cell move");

    assert!((pressure_delta_bar - 0.004).abs() < 1e-12);
    assert!((water_delta - 0.00002).abs() < 1e-12);
    assert!((oil_delta - 0.00003).abs() < 1e-12);
    assert!((gas_delta - 0.00001).abs() < 1e-12);
}

#[test]
fn candidate_update_bounds_include_oil_saturation_change() {
    let previous_state = FimState {
        cells: vec![crate::fim::state::FimCellState {
            pressure_bar: 200.0,
            sw: 0.2,
            hydrocarbon_var: 0.2,
            regime: crate::fim::state::HydrocarbonState::Saturated,
        }],
        well_bhp: Vec::new(),
        perforation_primaries: Vec::new(),
    };
    let candidate_state = FimState {
        cells: vec![crate::fim::state::FimCellState {
            pressure_bar: 200.0,
            sw: 0.35,
            hydrocarbon_var: 0.35,
            regime: crate::fim::state::HydrocarbonState::Saturated,
        }],
        well_bhp: Vec::new(),
        perforation_primaries: Vec::new(),
    };

    let (max_pressure_change, max_saturation_change) =
        state_update_change_bounds(&previous_state, &candidate_state);

    assert_eq!(max_pressure_change, 0.0);
    assert!((max_saturation_change - 0.3).abs() < 1e-12);
    assert!(!candidate_respects_update_bounds(
        &previous_state,
        &candidate_state,
        &FimNewtonOptions::default(),
    ));
}
