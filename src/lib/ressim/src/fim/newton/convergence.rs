use super::*;

pub(super) fn scaled_residual_inf_norm(
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

pub(super) fn scaled_update_inf_norm(
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum UpdateVariableFamily {
    Pressure,
    WaterSaturation,
    HydrocarbonVariable,
    WellBhp,
    PerforationRate,
}

impl UpdateVariableFamily {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Pressure => "pressure",
            Self::WaterSaturation => "sw",
            Self::HydrocarbonVariable => "hc",
            Self::WellBhp => "bhp",
            Self::PerforationRate => "perf-rate",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct UpdateFamilyPeak {
    pub(super) family: UpdateVariableFamily,
    pub(super) scaled_value: f64,
    pub(super) row: usize,
    pub(super) item_index: usize,
}

pub(super) fn update_variable_peak(
    current: &mut Option<UpdateFamilyPeak>,
    family: UpdateVariableFamily,
    scaled_value: f64,
    row: usize,
    item_index: usize,
) {
    let scaled_value = if scaled_value.is_finite() {
        scaled_value
    } else {
        f64::INFINITY
    };
    let candidate = UpdateFamilyPeak {
        family,
        scaled_value,
        row,
        item_index,
    };
    if current.is_none_or(|existing| candidate.scaled_value > existing.scaled_value) {
        *current = Some(candidate);
    }
}

pub(super) fn scaled_update_peak(
    update: &DVector<f64>,
    scaling: &crate::fim::scaling::VariableScaling,
) -> UpdateFamilyPeak {
    let n_cells = scaling.pressure.len();
    let mut peak = None;

    for i in 0..n_cells {
        update_variable_peak(
            &mut peak,
            UpdateVariableFamily::Pressure,
            update[i * 3].abs() / scaling.pressure[i],
            i * 3,
            i,
        );
        update_variable_peak(
            &mut peak,
            UpdateVariableFamily::WaterSaturation,
            update[i * 3 + 1].abs() / scaling.sw[i],
            i * 3 + 1,
            i,
        );
        update_variable_peak(
            &mut peak,
            UpdateVariableFamily::HydrocarbonVariable,
            update[i * 3 + 2].abs() / scaling.hydrocarbon_var[i],
            i * 3 + 2,
            i,
        );
    }

    let mut offset = n_cells * 3;
    for i in 0..scaling.well_bhp.len() {
        update_variable_peak(
            &mut peak,
            UpdateVariableFamily::WellBhp,
            update[offset + i].abs() / scaling.well_bhp[i],
            offset + i,
            i,
        );
    }
    offset += scaling.well_bhp.len();
    for i in 0..scaling.perforation_rate.len() {
        update_variable_peak(
            &mut peak,
            UpdateVariableFamily::PerforationRate,
            update[offset + i].abs() / scaling.perforation_rate[i],
            offset + i,
            i,
        );
    }

    peak.expect("update diagnostics require at least one unknown")
}

pub(super) fn scaled_applied_update_peak(
    state: &FimState,
    candidate: &FimState,
    scaling: &crate::fim::scaling::VariableScaling,
) -> UpdateFamilyPeak {
    let mut peak = None;

    for (idx, (current, next)) in state.cells.iter().zip(candidate.cells.iter()).enumerate() {
        update_variable_peak(
            &mut peak,
            UpdateVariableFamily::Pressure,
            (next.pressure_bar - current.pressure_bar).abs() / scaling.pressure[idx],
            idx * 3,
            idx,
        );
        update_variable_peak(
            &mut peak,
            UpdateVariableFamily::WaterSaturation,
            (next.sw - current.sw).abs() / scaling.sw[idx],
            idx * 3 + 1,
            idx,
        );
        update_variable_peak(
            &mut peak,
            UpdateVariableFamily::HydrocarbonVariable,
            (next.hydrocarbon_var - current.hydrocarbon_var).abs() / scaling.hydrocarbon_var[idx],
            idx * 3 + 2,
            idx,
        );
    }

    let mut offset = state.cells.len() * 3;
    for (idx, (current, next)) in state
        .well_bhp
        .iter()
        .zip(candidate.well_bhp.iter())
        .enumerate()
    {
        update_variable_peak(
            &mut peak,
            UpdateVariableFamily::WellBhp,
            (next - current).abs() / scaling.well_bhp[idx],
            offset + idx,
            idx,
        );
    }
    offset += state.well_bhp.len();
    for (idx, (current, next)) in state
        .perforation_primaries()
        .iter()
        .zip(candidate.perforation_primaries().iter())
        .enumerate()
    {
        update_variable_peak(
            &mut peak,
            UpdateVariableFamily::PerforationRate,
            (next.value - current.value).abs() / scaling.perforation_rate[idx],
            offset + idx,
            idx,
        );
    }

    peak.expect("applied update diagnostics require at least one unknown")
}

pub(super) fn update_peak_trace(peak: UpdateFamilyPeak) -> String {
    format!(
        " upd_peak=[{}={:.3e} row={} item={}]",
        peak.family.label(),
        peak.scaled_value,
        peak.row,
        peak.item_index
    )
}

pub(crate) fn iterate_has_material_change(previous_state: &FimState, state: &FimState) -> bool {
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
            .perforation_primaries()
            .iter()
            .zip(state.perforation_primaries().iter())
            .any(|(previous, current)| {
                (current.value - previous.value).abs() > PERF_RATE_EPS
                    || current.kind != previous.kind
            })
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct GlobalMaterialBalanceDiagnostics {
    pub(super) water: f64,
    pub(super) oil_component: f64,
    pub(super) gas_component: f64,
    pub(super) global_family: ResidualRowFamily,
    pub(super) global_value: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct AcceptedStateConvergenceDiagnostics {
    pub(super) state: FimState,
    pub(super) residual_inf_norm: f64,
    pub(super) residual_diagnostics: ResidualFamilyDiagnostics,
    pub(super) residual_detail: Option<String>,
    pub(super) material_balance_inf_norm: f64,
    pub(super) material_balance_diagnostics: GlobalMaterialBalanceDiagnostics,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ResidualRowFamily {
    Water,
    OilComponent,
    GasComponent,
    WellConstraint,
    PerforationFlow,
}

impl ResidualRowFamily {
    pub(super) fn label(self) -> &'static str {
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
pub(super) struct ResidualFamilyPeak {
    pub(super) family: ResidualRowFamily,
    pub(super) scaled_value: f64,
    pub(super) row: usize,
    pub(super) item_index: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ResidualFamilyDiagnostics {
    pub(super) water: ResidualFamilyPeak,
    pub(super) oil_component: ResidualFamilyPeak,
    pub(super) gas_component: ResidualFamilyPeak,
    pub(super) well_constraint: Option<ResidualFamilyPeak>,
    pub(super) perforation_flow: Option<ResidualFamilyPeak>,
    pub(super) global: ResidualFamilyPeak,
}

pub(super) fn update_family_peak(
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

/// OPM shipped convergence tolerances (Flow 2025.10 defaults, verified from the installed
/// binary's `--help-all` and `opm-simulators` tag `release/2025.10/final` — see
/// `docs/FIM_BUNDLE_N_DESIGN.md` §9.1). Used by the inert Bundle N checkpoint-1 diagnostic
/// below; they do NOT participate in any accept/retry decision yet.
const OPM_TOLERANCE_CNV: f64 = 1e-2;
const OPM_TOLERANCE_CNV_RELAXED: f64 = 1.0;
const OPM_TOLERANCE_MB: f64 = 1e-7;
const OPM_TOLERANCE_MB_RELAXED: f64 = 1e-6;
const OPM_RELAXED_MAX_PV_FRACTION: f64 = 0.03;
/// OPM `newton-min-iterations` default (2): `iteration() >= minIter` gates
/// `NonlinearSystemBlackOilReservoir::initialLinearization`'s accept decision
/// (`NonlinearSystemBlackOilReservoir_impl.hpp:175`), where `iteration()` is
/// `NewtonIterationContext`'s 0-based counter, incremented once per full
/// assemble-check-then-update cycle via `advanceIteration()` — called only
/// *after* `nonlinearIteration()` completes (`:229`), i.e. after the check.
/// ResSim's own `for iteration in 0..max_newton_iterations` loop has the
/// identical shape (assemble, check `converged_on_entry`, apply update only
/// if not converged), so `iteration` here is the direct analog of OPM's
/// `iteration()`: both equal the number of Newton updates already applied
/// prior to the current check. OPM's default `minIter=2` therefore requires
/// `iteration >= 2` (two prior updates, three total residual evaluations)
/// before acceptance is even possible. `FIM-DIAG-003` D2 (2026-07-11/12,
/// `docs/FIM_CONVERGENCE_WORKLOG.md` "checkpoint D2"/"checkpoint D5")
/// source-traced this exactly and fixed a confirmed off-by-one that
/// previously read `1` here (letting acceptance fire one iteration too
/// early on fast-converging cases) — verified isolated to `OpmAligned`
/// (bounded-case control matrix bit-identical on the Legacy/flag-off path,
/// which never reads this constant).
pub(super) const OPM_NEWTON_MIN_ITERATION_INDEX: usize = 2;
/// OPM `relaxed-linear-solver-reduction` default: a linear solve that didn't fully converge
/// but reduced the residual by at least this factor relative to `rhs_norm` (x0=0 so
/// r0=rhs, design doc §9.5) is accepted with a warning rather than triggering a fallback.
pub(super) const OPM_RELAXED_LINEAR_SOLVER_REDUCTION: f64 = 0.01;

/// OPM's relaxed linear-solver criterion applies to the returned correction's actual residual
/// reduction, not to a backend-specific failure classification. In particular, direct LU has no
/// iterative failure payload but can still return a finite, useful non-strict correction.
pub(super) fn opm_accepts_relaxed_linear_report(report: &FimLinearSolveReport) -> bool {
    report.solution.iter().all(|value| value.is_finite())
        && report.reduction().is_finite()
        && report.reduction() < OPM_RELAXED_LINEAR_SOLVER_REDUCTION
}

/// Bundle N checkpoint 1 (read-only): OPM-style CNV/MB convergence measures computed from the
/// RAW (unscaled) residual, mirroring `BlackoilModel::getReservoirConvergence` /
/// `getMaxCoeff` / `characteriseCnvPvSplit`. ResSim's residual is already dt-integrated
/// (surface m³), unlike OPM's rate residual, so OPM's `* dt` factor is intentionally absent —
/// `CNV = B_avg * max_i(|R_i|/pv_i)` here is dimensionally identical to OPM's
/// `B_avg * dt * max_i(|R_rate_i|/pv_i)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct CnvMbDiagnostics {
    /// Per component (water, oil-component, gas-component): field-average FVF times the
    /// worst per-cell pore-volume-normalized residual. Dimensionless local error.
    pub(super) cnv: [f64; 3],
    /// Per component: |B_avg * signed residual sum| / total pore volume. Dimensionless
    /// global mass-balance error (cancellation across cells intended).
    pub(super) mb: [f64; 3],
    /// Fraction of total pore volume held by cells whose own worst-component CNV exceeds
    /// the strict tolerance.
    pub(super) violating_pv_fraction: f64,
    /// OPM's `relaxed-max-pv-fraction` rule: violating PV under 3% lets the whole CNV check
    /// run at the relaxed tolerance this iteration.
    pub(super) pv_rule_relaxes: bool,
    /// All components pass strict CNV and strict MB.
    pub(super) would_accept_strict: bool,
    /// Effective accept condition at the CURRENT iteration and its `relax_final_iteration`
    /// flag: strict CNV/MB, or CNV via the 3%-PV relaxed tier, or (only when
    /// `relax_final_iteration` was passed in) OPM's unconditional final-iteration relaxed
    /// MB/CNV tolerances (design doc §9.1's `relax_final_iteration_mb`/`_cnv`).
    pub(super) would_accept: bool,
    /// Per component, the cell with the largest `|r_i,c|` among cells whose sign matches the
    /// summed residual `r_sum[c]` (i.e. the cell actually driving the MB imbalance, not one
    /// that partly cancels against it). `FIM-DIAG-003` D0 instrumentation.
    pub(super) mb_peak_cell: [usize; 3],
    /// Per component, the cell with the largest scaled CNV coefficient (`|r_i,c| * B_avg[c] /
    /// pv_i`, the same quantity `cnv[c]` is the max of). `FIM-DIAG-003` D0 instrumentation.
    pub(super) cnv_peak_cell: [usize; 3],
    /// The single failing criterion with the largest `value / effective_tolerance` ratio, or
    /// `None` when `would_accept`. `FIM-DIAG-003` D0 instrumentation — names which criterion
    /// blocks acceptance so the trace line doesn't require reading six numbers by hand.
    pub(super) binding: Option<BindingCriterion>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum BindingCriterionKind {
    Cnv,
    Mb,
}

impl BindingCriterionKind {
    fn label(self) -> &'static str {
        match self {
            Self::Cnv => "cnv",
            Self::Mb => "mb",
        }
    }
}

const RESIDUAL_COMPONENT_LABELS: [&str; 3] = ["water", "oil", "gas"];

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct BindingCriterion {
    pub(super) kind: BindingCriterionKind,
    pub(super) component: usize,
    pub(super) peak_cell: usize,
    pub(super) value: f64,
    pub(super) tolerance: f64,
}

impl BindingCriterion {
    pub(super) fn trace_string(&self) -> String {
        format!(
            "{}[{}]={:.3e}/{:.3e} cell={}",
            self.kind.label(),
            RESIDUAL_COMPONENT_LABELS[self.component],
            self.value,
            self.tolerance,
            self.peak_cell,
        )
    }
}

/// Pure core of the CNV/MB computation, split out for direct unit testing.
/// `fvf_per_cell[i] = [B_w, B_o, B_g]` (reservoir m³ per surface m³) for cell `i`;
/// `pore_volumes_m3` are REFERENCE pore volumes (porosity * bulk volume, no compressibility
/// factor), matching OPM's `referencePorosity * dofTotalVolume`. `relax_final_iteration`
/// mirrors OPM's `iteration == maxIter && min_strict_{mb,cnv}_iter == -1` triggers (the
/// shipped defaults — `min_strict_mb_iter`/`min_strict_cnv_iter` are not modeled since they
/// default to "off"): true unconditionally applies `tolerance-mb-relaxed`/`tolerance-cnv-relaxed`
/// to the WHOLE check, independent of the 3%-PV rule below.
pub(super) fn cnv_mb_from_parts(
    residual: &DVector<f64>,
    pore_volumes_m3: &[f64],
    fvf_per_cell: &[[f64; 3]],
    relax_final_iteration: bool,
) -> CnvMbDiagnostics {
    let n_cells = pore_volumes_m3.len();

    let mut b_avg = [0.0_f64; 3];
    for fvf in fvf_per_cell {
        for c in 0..3 {
            b_avg[c] += fvf[c];
        }
    }
    for avg in &mut b_avg {
        *avg /= n_cells.max(1) as f64;
    }

    let mut max_coeff = [0.0_f64; 3];
    let mut cnv_peak_cell = [0usize; 3];
    let mut r_sum = [0.0_f64; 3];
    let mut pv_sum = 0.0_f64;
    let mut violating_pv = 0.0_f64;
    for i in 0..n_cells {
        let pv = pore_volumes_m3[i].max(1e-9);
        pv_sum += pv;
        let mut cell_max_cnv = 0.0_f64;
        for c in 0..3 {
            let r = residual[i * 3 + c];
            r_sum[c] += r;
            let coeff = r.abs() / pv;
            if coeff > max_coeff[c] {
                max_coeff[c] = coeff;
                cnv_peak_cell[c] = i;
            }
            cell_max_cnv = cell_max_cnv.max(r.abs() * b_avg[c] / pv);
        }
        if cell_max_cnv > OPM_TOLERANCE_CNV {
            violating_pv += pv;
        }
    }

    let mut cnv = [0.0_f64; 3];
    let mut mb = [0.0_f64; 3];
    for c in 0..3 {
        cnv[c] = b_avg[c] * max_coeff[c];
        mb[c] = (b_avg[c] * r_sum[c]).abs() / pv_sum.max(1e-9);
    }

    // Peak-contributing cell per component for MB: the largest |r_i,c| among cells whose sign
    // agrees with the summed imbalance r_sum[c] — the cell(s) actually driving the mass-balance
    // error rather than one that cancels against it. `FIM-DIAG-003` D0 instrumentation.
    let mut mb_peak_cell = [0usize; 3];
    let mut mb_peak_abs = [0.0_f64; 3];
    for c in 0..3 {
        let sign = r_sum[c].signum();
        for i in 0..n_cells {
            let r = residual[i * 3 + c];
            if r.signum() == sign && r.abs() > mb_peak_abs[c] {
                mb_peak_abs[c] = r.abs();
                mb_peak_cell[c] = i;
            }
        }
    }

    let violating_pv_fraction = violating_pv / pv_sum.max(1e-9);
    let pv_rule_relaxes = violating_pv < OPM_RELAXED_MAX_PV_FRACTION * pv_sum;
    let mb_ok = mb.iter().all(|&v| v <= OPM_TOLERANCE_MB);
    let cnv_strict_ok = cnv.iter().all(|&v| v <= OPM_TOLERANCE_CNV);
    let mb_tol_effective = if relax_final_iteration {
        OPM_TOLERANCE_MB_RELAXED
    } else {
        OPM_TOLERANCE_MB
    };
    let cnv_tol_effective = if relax_final_iteration {
        OPM_TOLERANCE_CNV_RELAXED
    } else {
        OPM_TOLERANCE_CNV
    };
    let cnv_component_ok: [bool; 3] = std::array::from_fn(|c| {
        cnv[c] <= cnv_tol_effective || (pv_rule_relaxes && cnv[c] <= OPM_TOLERANCE_CNV_RELAXED)
    });
    let mb_component_ok: [bool; 3] = std::array::from_fn(|c| mb[c] <= mb_tol_effective);
    let cnv_effective_ok = cnv_component_ok.iter().all(|&ok| ok);
    let mb_effective_ok = mb_component_ok.iter().all(|&ok| ok);

    // Binding criterion: among the failing components, the one with the largest
    // value/tolerance overshoot ratio. `FIM-DIAG-003` D0 — names which criterion blocks
    // acceptance without requiring a human to compare six numbers against two tolerances.
    let mut binding: Option<BindingCriterion> = None;
    let mut worst_ratio = 1.0_f64;
    for c in 0..3 {
        if !cnv_component_ok[c] {
            let ratio = cnv[c] / cnv_tol_effective.max(1e-300);
            if ratio > worst_ratio {
                worst_ratio = ratio;
                binding = Some(BindingCriterion {
                    kind: BindingCriterionKind::Cnv,
                    component: c,
                    peak_cell: cnv_peak_cell[c],
                    value: cnv[c],
                    tolerance: cnv_tol_effective,
                });
            }
        }
        if !mb_component_ok[c] {
            let ratio = mb[c] / mb_tol_effective.max(1e-300);
            if ratio > worst_ratio {
                worst_ratio = ratio;
                binding = Some(BindingCriterion {
                    kind: BindingCriterionKind::Mb,
                    component: c,
                    peak_cell: mb_peak_cell[c],
                    value: mb[c],
                    tolerance: mb_tol_effective,
                });
            }
        }
    }

    CnvMbDiagnostics {
        cnv,
        mb,
        violating_pv_fraction,
        pv_rule_relaxes,
        would_accept_strict: cnv_strict_ok && mb_ok,
        would_accept: cnv_effective_ok && mb_effective_ok,
        mb_peak_cell,
        cnv_peak_cell,
        binding,
    }
}

/// OPM shipped per-cell update-chopping limits (Flow 2025.10 defaults `--ds-max` /
/// `--dp-max-rel`, verified from the installed binary and `blackoilnewtonmethod.hpp` at the
/// pinned tag — `docs/FIM_BUNDLE_N_DESIGN.md` §9.2). Used by the `OpmAligned` nonlinear flavor.
pub(super) const OPM_DS_MAX: f64 = 0.2;
const OPM_DP_MAX_REL: f64 = 0.3;
/// OPM shipped well-BHP update-chopping limit (`--dbhp-max-rel`, default `1.0`, verified from
/// the installed binary and `StandardWellPrimaryVariables.cpp::updateNewton` at the pinned
/// tag — Bundle N §5 follow-up, worklog "Bundle N §5 end-metric evaluation (2026-07-09)").
/// OPM clamps the ABSOLUTE BHP delta to at most this fraction of the CURRENT BHP magnitude
/// (`dx = sign(dx) * min(|dx|, |bhp_current| * dBHPLimit)`), then floors the result just above
/// zero (`bhp_lower_limit = 1 bar - 1 Pa`, i.e. effectively `>= 1.0` bar here). OPM does NOT
/// clamp well RATE (`WQTotal`) magnitude at all — only a post-hoc sign-consistency check
/// (injector can't produce, producer can't inject) — so ResSim's perforation-rate deltas stay
/// unchopped, matching OPM's own choice rather than inventing a new limit it doesn't have.
const OPM_DBHP_MAX_REL: f64 = 1.0;
pub(super) const OPM_BHP_LOWER_LIMIT_BAR: f64 = 1.0;

/// Bundle N checkpoint 2 (N2, `OpmAligned` flavor only): OPM's per-cell update chopping,
/// ported from `updatePrimaryVariables_` (`blackoilnewtonmethod.hpp`, design doc §9.2).
/// Replaces the Legacy global damping scalar: each cell's own saturation deltas are scaled by
/// that cell's `satAlpha = dsMax / maxSatDelta` (including the IMPLIED oil delta
/// `dSo = -(dSw + dSg)`), and its pressure delta is clamped to `±dpMaxRel * p_current` —
/// no cell restricts any other cell's movement. Matching OPM's composition order, the global
/// oscillation-relaxation scalar (`dampen` mode, Phase 7) multiplies the RAW update first,
/// then the chop applies per cell.
///
/// Sign convention: ResSim applies `next = current + update` (OPM uses `current - delta`);
/// the chop is symmetric in sign so only the Rs non-negativity guard direction differs.
///
/// Bundle N §5 follow-up (worklog "Bundle N §5 end-metric evaluation (2026-07-09)"): the
/// well-BHP tail entry is now chopped too, matching OPM's `dbhp-max-rel` exactly — added after
/// the heavy case's §5 failure traced to a producer pinned at its BHP limit whose raw,
/// previously-unchopped BHP update oscillated each iteration, perturbing the coupled
/// reservoir residual via the shared linear solve and stalling its own MB convergence for
/// ~20 iterations per substep. Perforation-rate entries stay unchopped, matching OPM's own
/// choice not to limit well rate (`WQTotal`) magnitude. ResSim's Schur-recovered well state is
/// still post-processed by `relax_well_state_toward_local_consistency` after application.
pub(super) fn opm_per_cell_chopped_update(
    state: &FimState,
    update: &DVector<f64>,
    relaxation: f64,
) -> DVector<f64> {
    let mut chopped = update * relaxation;
    for (well_idx, &bhp_bar) in state.well_bhp.iter().enumerate() {
        let offset = state.well_bhp_unknown_offset(well_idx);
        let dbhp = chopped[offset];
        let dbhp_cap = OPM_DBHP_MAX_REL * bhp_bar.abs();
        let dbhp_limited = if dbhp.abs() > dbhp_cap {
            dbhp.signum() * dbhp_cap
        } else {
            dbhp
        };
        chopped[offset] = if bhp_bar + dbhp_limited < OPM_BHP_LOWER_LIMIT_BAR {
            OPM_BHP_LOWER_LIMIT_BAR - bhp_bar
        } else {
            dbhp_limited
        };
    }
    for (idx, cell) in state.cells.iter().enumerate() {
        let offset = idx * 3;
        let dp = chopped[offset];
        let dsw = chopped[offset + 1];
        let dhc = chopped[offset + 2];

        // Saturation deltas, including the implied oil delta (design doc §9.2: OPM counts
        // dSo = -(dSw + dSg) toward the per-cell max even though So is not a primary var).
        let (dsg, dso) = match cell.regime {
            HydrocarbonState::Saturated => (dhc, -(dsw + dhc)),
            HydrocarbonState::Undersaturated => (0.0, -dsw),
        };
        let max_sat_delta = dsw.abs().max(dso.abs()).max(dsg.abs());
        let sat_alpha = if max_sat_delta > OPM_DS_MAX {
            OPM_DS_MAX / max_sat_delta
        } else {
            1.0
        };
        chopped[offset + 1] = dsw * sat_alpha;
        match cell.regime {
            HydrocarbonState::Saturated => {
                chopped[offset + 2] = dhc * sat_alpha;
            }
            HydrocarbonState::Undersaturated => {
                // hydrocarbon_var means Rs: not a saturation, so no satAlpha — only OPM's
                // guard that the R factor cannot go negative after the update.
                if cell.hydrocarbon_var + chopped[offset + 2] < 0.0 {
                    chopped[offset + 2] = -cell.hydrocarbon_var;
                }
            }
        }

        // Pressure: relative clamp, independent of satAlpha.
        let dp_cap = OPM_DP_MAX_REL * cell.pressure_bar.abs();
        if dp.abs() > dp_cap {
            chopped[offset] = dp.signum() * dp_cap;
        }
    }
    chopped
}

/// Sim-facing wrapper: extracts reference pore volumes and per-cell FVFs, then delegates to
/// `cnv_mb_from_parts`. Only the cell rows of `residual` are read; well/perforation rows are
/// excluded exactly as in OPM (wells have their own `tolerance-wells` criterion).
pub(super) fn cnv_mb_diagnostics(
    sim: &ReservoirSimulator,
    state: &FimState,
    residual: &DVector<f64>,
    relax_final_iteration: bool,
) -> CnvMbDiagnostics {
    let n_cells = state.cells.len();
    let mut pore_volumes = Vec::with_capacity(n_cells);
    let mut fvf = Vec::with_capacity(n_cells);
    let b_w = sim.b_w.max(1e-9);
    for idx in 0..n_cells {
        pore_volumes.push(sim.pore_volume_m3(idx));
        let pressure_bar = state.cells[idx].pressure_bar;
        fvf.push([
            b_w,
            sim.get_b_o_cell(idx, pressure_bar).max(1e-9),
            sim.get_b_g(pressure_bar).max(1e-9),
        ]);
    }
    cnv_mb_from_parts(residual, &pore_volumes, &fvf, relax_final_iteration)
}

pub(super) fn residual_family_diagnostics(
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
    for peak in [
        Some(oil_component),
        Some(gas_component),
        well_constraint,
        perforation_flow,
    ]
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

pub(super) fn residual_family_trace(diagnostics: &ResidualFamilyDiagnostics) -> String {
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
        parts.push(format!(
            "well={:.3e}@well{}",
            peak.scaled_value, peak.item_index
        ));
    }
    if let Some(peak) = diagnostics.perforation_flow {
        parts.push(format!(
            "perf={:.3e}@perf{}",
            peak.scaled_value, peak.item_index
        ));
    }
    parts.push(format!(
        "top={} row={} item={}",
        diagnostics.global.family.label(),
        diagnostics.global.row,
        diagnostics.global.item_index
    ));
    parts.join(" ")
}

pub(super) fn normalized_material_balance(component_sum: f64, component_scaling: &[f64]) -> f64 {
    let denominator = component_scaling
        .iter()
        .copied()
        .sum::<f64>()
        .abs()
        .max(1.0);
    component_sum.abs() / denominator
}

pub(super) fn global_material_balance_diagnostics(
    residual: &DVector<f64>,
    scaling: &crate::fim::scaling::EquationScaling,
) -> GlobalMaterialBalanceDiagnostics {
    let n_cells = scaling.water.len();
    let mut water_sum = 0.0_f64;
    let mut oil_component_sum = 0.0_f64;
    let mut gas_component_sum = 0.0_f64;

    for i in 0..n_cells {
        water_sum += residual[i * 3];
        oil_component_sum += residual[i * 3 + 1];
        gas_component_sum += residual[i * 3 + 2];
    }

    let water = normalized_material_balance(water_sum, &scaling.water);
    let oil_component = normalized_material_balance(oil_component_sum, &scaling.oil_component);
    let gas_component = normalized_material_balance(gas_component_sum, &scaling.gas_component);

    let mut global_family = ResidualRowFamily::Water;
    let mut global_value = water;
    for (family, value) in [
        (ResidualRowFamily::OilComponent, oil_component),
        (ResidualRowFamily::GasComponent, gas_component),
    ] {
        if value > global_value {
            global_family = family;
            global_value = value;
        }
    }

    GlobalMaterialBalanceDiagnostics {
        water,
        oil_component,
        gas_component,
        global_family,
        global_value,
    }
}

pub(super) fn global_material_balance_trace(
    diagnostics: &GlobalMaterialBalanceDiagnostics,
) -> String {
    format!(
        "water={:.3e} oil={:.3e} gas={:.3e} top={}",
        diagnostics.water,
        diagnostics.oil_component,
        diagnostics.gas_component,
        diagnostics.global_family.label(),
    )
}

pub(super) fn cell_index_to_ijk(
    sim: &ReservoirSimulator,
    cell_idx: usize,
) -> (usize, usize, usize) {
    let cells_per_layer = sim.nx * sim.ny;
    let k = cell_idx / cells_per_layer;
    let in_layer = cell_idx % cells_per_layer;
    let j = in_layer / sim.nx;
    let i = in_layer % sim.nx;
    (i, j, k)
}

pub(super) fn format_phase_flux_diagnostic(
    sim: &ReservoirSimulator,
    label: &str,
    diagnostic: &PhaseFluxDiagnostic,
) -> String {
    let (i, j, k) = cell_index_to_ijk(sim, diagnostic.upwind_cell_idx);
    format!(
        "{}(dphi={:.3e},up=({}, {}, {}),mob={:.3e},flux={:.3e})",
        label, diagnostic.dphi, i, j, k, diagnostic.mobility, diagnostic.flux,
    )
}

pub(super) fn format_face_phase_diagnostics(
    sim: &ReservoirSimulator,
    label: &str,
    diagnostics: Option<&FacePhaseDiagnostics>,
) -> String {
    match diagnostics {
        Some(face) => format!(
            "{}=[{} {} {}]",
            label,
            format_phase_flux_diagnostic(sim, "w", &face.water),
            format_phase_flux_diagnostic(sim, "o", &face.oil),
            format_phase_flux_diagnostic(sim, "g", &face.gas),
        ),
        None => format!("{}=[boundary]", label),
    }
}

pub(super) fn format_cell_face_phase_diagnostics(
    sim: &ReservoirSimulator,
    diagnostics: &CellFacePhaseDiagnostics,
) -> String {
    [
        format_face_phase_diagnostics(sim, "x-", diagnostics.x_minus.as_ref()),
        format_face_phase_diagnostics(sim, "x+", diagnostics.x_plus.as_ref()),
        format_face_phase_diagnostics(sim, "y-", diagnostics.y_minus.as_ref()),
        format_face_phase_diagnostics(sim, "y+", diagnostics.y_plus.as_ref()),
    ]
    .join(" ")
}

pub(super) fn cell_residual_detail_trace(
    sim: &ReservoirSimulator,
    previous_state: &FimState,
    state: &FimState,
    topology: &crate::fim::wells::FimWellTopology,
    dt_days: f64,
    peak: &ResidualFamilyPeak,
) -> Option<String> {
    let cell_idx = peak.item_index;
    if cell_idx >= state.cells.len() {
        return None;
    }
    let (i, j, k) = cell_index_to_ijk(sim, cell_idx);
    let cell = state.cell(cell_idx);
    let derived = state.derive_cell(sim, cell_idx);
    let equation = match peak.family {
        ResidualRowFamily::Water => "water",
        ResidualRowFamily::OilComponent => "oil",
        ResidualRowFamily::GasComponent => "gas",
        _ => return None,
    };
    let component = match peak.family {
        ResidualRowFamily::Water => 0,
        ResidualRowFamily::OilComponent => 1,
        ResidualRowFamily::GasComponent => 2,
        _ => return None,
    };
    let breakdown = cell_equation_residual_breakdown(
        sim,
        previous_state,
        state,
        topology,
        dt_days,
        cell_idx,
        component,
    )?;
    let face_diagnostics = cell_face_phase_flux_diagnostics(sim, state, dt_days, cell_idx)?;

    Some(format!(
        "eq={} cell{}=({}, {}, {}) p={:.3} sw={:.4} so={:.4} sg={:.4} rs={:.4} regime={:?} accum={:.3e} x-={:.3e} x+={:.3e} y-={:.3e} y+={:.3e} z-={:.3e} z+={:.3e} well={:.3e} total={:.3e} faces={}",
        equation,
        cell_idx,
        i,
        j,
        k,
        cell.pressure_bar,
        cell.sw,
        derived.so,
        derived.sg,
        derived.rs,
        cell.regime,
        breakdown.accumulation,
        breakdown.x_minus,
        breakdown.x_plus,
        breakdown.y_minus,
        breakdown.y_plus,
        breakdown.z_minus,
        breakdown.z_plus,
        breakdown.well_source,
        breakdown.total,
        format_cell_face_phase_diagnostics(sim, &face_diagnostics),
    ))
}

pub(super) fn well_constraint_detail_trace(
    sim: &ReservoirSimulator,
    state: &FimState,
    topology: &crate::fim::wells::FimWellTopology,
    peak: &ResidualFamilyPeak,
) -> Option<String> {
    let well_idx = peak.item_index;
    let well_topology = topology.wells.get(well_idx)?;
    let representative = &sim.wells[well_topology.representative_well_index];
    let control = physical_well_control(sim, topology, well_idx);
    let decision = if !control.enabled {
        "disabled".to_string()
    } else if control.rate_controlled {
        if control.uses_surface_target {
            "rate(surface)".to_string()
        } else {
            "rate(reservoir)".to_string()
        }
    } else {
        "bhp".to_string()
    };

    Some(format!(
        "well{} id={} inj={} head=({}, {}) bhp={:.3} mode={} target={} bhp_limit={:.3} nperf={}",
        well_idx,
        representative
            .physical_well_id
            .as_deref()
            .unwrap_or("<legacy>"),
        well_topology.injector,
        well_topology.head_i,
        well_topology.head_j,
        state
            .well_bhp
            .get(well_idx)
            .copied()
            .unwrap_or(representative.bhp),
        decision,
        control
            .target_rate
            .map(|value| format!("{:.3e}", value))
            .unwrap_or_else(|| "none".to_string()),
        control.bhp_limit,
        well_topology.perforation_indices.len(),
    ))
}

pub(super) fn residual_family_detail_trace(
    sim: &ReservoirSimulator,
    previous_state: &FimState,
    state: &FimState,
    topology: &crate::fim::wells::FimWellTopology,
    dt_days: f64,
    diagnostics: &ResidualFamilyDiagnostics,
) -> Option<String> {
    match diagnostics.global.family {
        ResidualRowFamily::Water
        | ResidualRowFamily::OilComponent
        | ResidualRowFamily::GasComponent => cell_residual_detail_trace(
            sim,
            previous_state,
            state,
            topology,
            dt_days,
            &diagnostics.global,
        ),
        ResidualRowFamily::WellConstraint => {
            well_constraint_detail_trace(sim, state, topology, &diagnostics.global)
        }
        ResidualRowFamily::PerforationFlow => {
            let detail = perforation_local_block(topology, state, diagnostics.global.item_index)
                .residual_diagnostics(sim)?;
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
    }
}

pub(super) fn evaluate_accepted_state_convergence(
    sim: &ReservoirSimulator,
    previous_state: &FimState,
    candidate_state: &FimState,
    topology: &crate::fim::wells::FimWellTopology,
    dt_days: f64,
) -> AcceptedStateConvergenceDiagnostics {
    let mut state = candidate_state.clone();
    state.classify_regimes(sim);

    let assembly = assemble_fim_system(
        sim,
        previous_state,
        &state,
        &FimAssemblyOptions {
            dt_days,
            include_wells: true,
            assemble_residual_only: true,
            topology: Some(topology),
            flow_resv_context: None,
        },
    );
    let residual_inf_norm =
        scaled_residual_inf_norm(&assembly.residual, &assembly.equation_scaling);
    let residual_diagnostics =
        residual_family_diagnostics(&assembly.residual, &assembly.equation_scaling);
    let residual_detail = residual_family_detail_trace(
        sim,
        previous_state,
        &state,
        topology,
        dt_days,
        &residual_diagnostics,
    );
    let material_balance_diagnostics =
        global_material_balance_diagnostics(&assembly.residual, &assembly.equation_scaling);

    AcceptedStateConvergenceDiagnostics {
        state,
        residual_inf_norm,
        residual_diagnostics,
        residual_detail,
        material_balance_inf_norm: material_balance_diagnostics.global_value,
        material_balance_diagnostics,
    }
}

pub(super) fn convergence_limits(options: &FimNewtonOptions, use_guard_band: bool) -> (f64, f64) {
    let factor = if use_guard_band {
        ENTRY_RESIDUAL_GUARD_FACTOR
    } else {
        1.0
    };
    (
        options.residual_tolerance * factor,
        options.material_balance_tolerance,
    )
}

pub(super) fn accepted_state_meets_convergence(
    diagnostics: &AcceptedStateConvergenceDiagnostics,
    residual_limit: f64,
    material_balance_limit: f64,
) -> bool {
    diagnostics.residual_inf_norm <= residual_limit
        && diagnostics.material_balance_inf_norm <= material_balance_limit
}

pub(super) fn zero_move_appleyard_acceptance_allows(
    materially_changed: bool,
    residual_inf_norm: f64,
    material_balance_inf_norm: f64,
    options: &FimNewtonOptions,
) -> bool {
    if materially_changed {
        return false;
    }

    residual_inf_norm
        <= options.residual_tolerance * NOOP_ENTRY_EXACT_FACTOR * ENTRY_RESIDUAL_GUARD_FACTOR
        && material_balance_inf_norm
            <= options.material_balance_tolerance
                * NOOP_ENTRY_EXACT_FACTOR
                * ENTRY_RESIDUAL_GUARD_FACTOR
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct StagnationAcceptanceGateStatus {
    pub(super) materially_changed: bool,
    pub(super) update_ok: bool,
    pub(super) residual_ok: bool,
    pub(super) material_balance_ok: bool,
}

impl StagnationAcceptanceGateStatus {
    pub(super) fn allows(self) -> bool {
        self.materially_changed && self.update_ok && self.residual_ok && self.material_balance_ok
    }
}

pub(super) fn stagnation_acceptance_gate_status(
    materially_changed: bool,
    residual_inf_norm: f64,
    material_balance_inf_norm: f64,
    update_inf_norm: f64,
    options: &FimNewtonOptions,
) -> StagnationAcceptanceGateStatus {
    StagnationAcceptanceGateStatus {
        materially_changed,
        update_ok: update_inf_norm <= options.update_tolerance,
        residual_ok: residual_inf_norm
            <= options.residual_tolerance * NONLINEAR_HISTORY_RESIDUAL_BAND_FACTOR,
        material_balance_ok: material_balance_inf_norm <= options.material_balance_tolerance,
    }
}

pub(super) fn stagnation_acceptance_gate_trace(
    status: StagnationAcceptanceGateStatus,
    residual_inf_norm: f64,
    material_balance_inf_norm: f64,
    update_inf_norm: f64,
    options: &FimNewtonOptions,
) -> String {
    format!(
        " gates=[changed={} upd={:.3e}/{:.3e} {} res={:.3e}/{:.3e} {} mb={:.3e}/{:.3e} {}]",
        status.materially_changed,
        update_inf_norm,
        options.update_tolerance,
        if status.update_ok { "ok" } else { "reject" },
        residual_inf_norm,
        options.residual_tolerance * NONLINEAR_HISTORY_RESIDUAL_BAND_FACTOR,
        if status.residual_ok { "ok" } else { "reject" },
        material_balance_inf_norm,
        options.material_balance_tolerance,
        if status.material_balance_ok {
            "ok"
        } else {
            "reject"
        },
    )
}

pub(super) fn stagnation_acceptance_allows(
    materially_changed: bool,
    residual_inf_norm: f64,
    material_balance_inf_norm: f64,
    update_inf_norm: f64,
    options: &FimNewtonOptions,
) -> bool {
    stagnation_acceptance_gate_status(
        materially_changed,
        residual_inf_norm,
        material_balance_inf_norm,
        update_inf_norm,
        options,
    )
    .allows()
}
