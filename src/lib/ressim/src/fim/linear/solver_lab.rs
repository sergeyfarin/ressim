//! Offline solver lab (Phase 9 step 9.2): runs full solves with each linear backend on
//! captured real failing systems (`capture.rs`), on identical inputs, out of the Newton
//! loop. This is the component-isolation harness for the linear solver / CPR — a new
//! preconditioner hypothesis is evaluated here in seconds against the whole captured
//! corpus before any live-solver change is considered.
//!
//! Usage (native only, manual):
//! 1. Capture a corpus:
//!    `FIM_CAPTURE_DIR=<dir> cargo test --release --lib -- --ignored repro_water_pressure_12x12x3 --nocapture`
//! 2. Run the lab on it:
//!    `FIM_CAPTURE_DIR=<dir> cargo test --release --lib -- --ignored solver_lab_compare_backends --nocapture`
//!
//! Validity gates (stop conditions from the Phase 9 plan) are asserted, not just printed:
//! - sparse-LU must converge on every captured system (else the problem is upstream in
//!   assembly at that state, not in the preconditioner);
//! - the current FgmresCpr path must reproduce its live failure on the majority of the
//!   corpus (else the capture is missing state and comparisons are not trustworthy).

use nalgebra::DVector;
use sprs::CsMat;

use super::capture::{FimCapturedSystem, capture_dir_from_env, load_captures};
use super::gmres_block_jacobi::{
    self, CprFineSmootherKind, CprPressureRestrictionKind, solve_with_restriction_kind,
    solve_with_smoother_and_restriction,
};
use super::well_schur;
use super::{
    FimLinearBlockLayout, FimLinearSolveOptions, FimLinearSolveReport, FimLinearSolverKind,
    solve_linearized_system,
};
use crate::fim::scaling::EquationScaling;

fn residual_vector(
    jacobian: &CsMat<f64>,
    solution: &DVector<f64>,
    rhs: &DVector<f64>,
) -> DVector<f64> {
    let mut residual = rhs.clone();
    for (row, vec) in jacobian.outer_iterator().enumerate() {
        let mut sum = 0.0;
        for (&col, &value) in vec.indices().iter().zip(vec.data().iter()) {
            sum += value * solution[col];
        }
        residual[row] -= sum;
    }
    residual
}

fn residual_norm(jacobian: &CsMat<f64>, solution: &DVector<f64>, rhs: &DVector<f64>) -> f64 {
    residual_vector(jacobian, solution, rhs).norm()
}

/// Largest per-family overshoot beyond that family's own relative-reduction target, i.e. how
/// many multiples of `absolute_tolerance + relative_tolerance * initial_peak` the worst family
/// is currently at. `> 1.0` means at least one family (often the well/perforation rows) is left
/// under-resolved even though the whole-system norm may already look converged. Returns `None`
/// if the system has no captured `EquationScaling`.
fn worst_family_overshoot(
    scaling: &EquationScaling,
    rhs: &DVector<f64>,
    residual: &DVector<f64>,
    absolute_tolerance: f64,
    relative_tolerance: f64,
) -> (&'static str, f64) {
    let initial = scaling.family_peaks(rhs);
    let current = scaling.family_peaks(residual);
    let ratio = |current: f64, initial: f64| {
        current
            / (absolute_tolerance + relative_tolerance * initial.max(f64::EPSILON))
                .max(f64::EPSILON)
    };
    let candidates = [
        ("water", ratio(current.water, initial.water)),
        (
            "oil_component",
            ratio(current.oil_component, initial.oil_component),
        ),
        (
            "gas_component",
            ratio(current.gas_component, initial.gas_component),
        ),
        (
            "well_constraint",
            ratio(current.well_constraint, initial.well_constraint),
        ),
        (
            "perforation_flow",
            ratio(current.perforation_flow, initial.perforation_flow),
        ),
    ];
    candidates
        .into_iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .expect("candidates is non-empty")
}

struct BackendOutcome {
    label: &'static str,
    backend_used: &'static str,
    converged: bool,
    iterations: usize,
    true_residual_norm: f64,
    failure_reason: Option<&'static str>,
}

fn run_backend(
    label: &'static str,
    kind: FimLinearSolverKind,
    system: &FimCapturedSystem,
) -> BackendOutcome {
    let options = FimLinearSolveOptions {
        kind,
        ..FimLinearSolveOptions::default()
    };
    let report: FimLinearSolveReport = solve_linearized_system(
        &system.jacobian,
        &system.rhs,
        &options,
        system.layout,
        system.equation_scaling.as_ref(),
    );
    BackendOutcome {
        label,
        backend_used: report.backend_used.label(),
        converged: report.converged,
        iterations: report.iterations,
        true_residual_norm: residual_norm(&system.jacobian, &report.solution, &system.rhs),
        failure_reason: report
            .failure_diagnostics
            .as_ref()
            .map(|diagnostics| diagnostics.reason.label()),
    }
}

/// Manual lab entry point; requires FIM_CAPTURE_DIR pointing at a captured corpus.
#[test]
#[ignore]
fn solver_lab_compare_backends() {
    let dir = capture_dir_from_env()
        .expect("set FIM_CAPTURE_DIR to a directory produced by a capture driver run");
    let systems = load_captures(&dir).expect("load captured systems");
    assert!(
        !systems.is_empty(),
        "no fim_capture_*.txt files in {}",
        dir.display()
    );

    println!(
        "solver lab: {} captured systems from {}",
        systems.len(),
        dir.display()
    );

    let mut lu_failures = 0usize;
    let mut cpr_reproduced_failure = 0usize;
    let mut cpr_converged = 0usize;
    let mut gmres_ilu0_converged = 0usize;

    for (index, system) in systems.iter().enumerate() {
        let rhs_norm = system.rhs.norm();
        println!(
            "--- capture {index:05}: rows={} nnz={} rhs_norm={:.3e} newton_iter={} live_reason={} dominant={}@{}",
            system.jacobian.rows(),
            system.jacobian.nnz(),
            rhs_norm,
            system.metadata.newton_iteration,
            system.metadata.failure_reason,
            system.metadata.dominant_family,
            system.metadata.dominant_item_index,
        );

        let outcomes = [
            run_backend("sparse-lu", FimLinearSolverKind::SparseLuDebug, system),
            run_backend("gmres-ilu0", FimLinearSolverKind::GmresIlu0, system),
            run_backend("fgmres-cpr", FimLinearSolverKind::FgmresCpr, system),
        ];

        for outcome in &outcomes {
            println!(
                "    {:<11} used={:<10} converged={:<5} iters={:<4} true_res={:.3e} rel={:.3e}{}",
                outcome.label,
                outcome.backend_used,
                outcome.converged,
                outcome.iterations,
                outcome.true_residual_norm,
                outcome.true_residual_norm / rhs_norm.max(f64::EPSILON),
                outcome
                    .failure_reason
                    .map(|reason| format!(" reason={reason}"))
                    .unwrap_or_default(),
            );
        }

        // Stop-condition 1 accounting: the direct reference must solve every system.
        let lu = &outcomes[0];
        let lu_ok = lu.converged && lu.true_residual_norm / rhs_norm.max(f64::EPSILON) < 1e-6;
        if !lu_ok {
            lu_failures += 1;
            println!("    !! sparse-LU reference did NOT cleanly solve this system");
        }

        if outcomes[1].converged {
            gmres_ilu0_converged += 1;
        }
        if outcomes[2].converged {
            cpr_converged += 1;
        } else {
            cpr_reproduced_failure += 1;
        }
    }

    println!("=== aggregate over {} systems ===", systems.len());
    println!("sparse-lu reference failures: {lu_failures}");
    println!("gmres-ilu0 converged: {gmres_ilu0_converged}");
    println!("fgmres-cpr converged: {cpr_converged}");
    println!("fgmres-cpr reproduced live failure offline: {cpr_reproduced_failure}");

    // Stop condition 1: direct solve struggling means assembly-level trouble at those
    // states — the lab's comparisons would be meaningless.
    assert_eq!(
        lu_failures, 0,
        "sparse-LU reference failed on {lu_failures} captured systems — problem is upstream \
         of the preconditioner (plan stop condition 1)"
    );

    // Stop condition 2: if the current CPR path converges offline on most systems that
    // failed live, the capture is missing state and cannot be trusted for comparisons.
    assert!(
        cpr_reproduced_failure * 2 >= systems.len(),
        "current FgmresCpr converged offline on {}/{} systems that failed live — capture \
         fidelity is suspect (plan stop condition 2)",
        cpr_converged,
        systems.len()
    );
}

struct VariantOutcome {
    kind: CprPressureRestrictionKind,
    converged: bool,
    true_residual_norm: f64,
}

fn run_restriction_variant(
    kind: CprPressureRestrictionKind,
    system: &FimCapturedSystem,
) -> VariantOutcome {
    let options = FimLinearSolveOptions {
        kind: FimLinearSolverKind::FgmresCpr,
        ..FimLinearSolveOptions::default()
    };
    let report =
        solve_with_restriction_kind(&system.jacobian, &system.rhs, &options, system.layout, kind);
    VariantOutcome {
        kind,
        converged: report.converged,
        true_residual_norm: residual_norm(&system.jacobian, &report.solution, &system.rhs),
    }
}

/// Compares every salvaged/new CPR pressure-restriction variant (`CprPressureRestrictionKind::ALL`
/// — the four restrictions salvaged from the reverted in-situ probe plus the new OPM-style
/// `QuasiImpes` weighting) as full solves against a captured corpus. This is the actual
/// systematic component test the Phase 9 lab exists for: a new restriction hypothesis is one
/// enum arm plus this rerun, not a live-solver change and wasm replay.
///
/// Manual lab entry point; requires FIM_CAPTURE_DIR pointing at a captured corpus.
#[test]
#[ignore]
fn solver_lab_compare_restriction_variants() {
    let dir = capture_dir_from_env()
        .expect("set FIM_CAPTURE_DIR to a directory produced by a capture driver run");
    let systems = load_captures(&dir).expect("load captured systems");
    assert!(
        !systems.is_empty(),
        "no fim_capture_*.txt files in {}",
        dir.display()
    );

    println!(
        "restriction-variant lab: {} captured systems from {}",
        systems.len(),
        dir.display()
    );

    let variants = CprPressureRestrictionKind::ALL;
    let mut converged_counts = vec![0usize; variants.len()];
    let mut win_counts = vec![0usize; variants.len()];
    let mut relative_residuals: Vec<Vec<f64>> = vec![Vec::new(); variants.len()];

    for (index, system) in systems.iter().enumerate() {
        let rhs_norm = system.rhs.norm().max(f64::EPSILON);
        let outcomes: Vec<VariantOutcome> = variants
            .iter()
            .map(|&kind| run_restriction_variant(kind, system))
            .collect();

        println!(
            "--- capture {index:05}: rows={} newton_iter={} live_reason={} dominant={}@{}",
            system.jacobian.rows(),
            system.metadata.newton_iteration,
            system.metadata.failure_reason,
            system.metadata.dominant_family,
            system.metadata.dominant_item_index,
        );
        let mut best_variant_idx = 0usize;
        let mut best_rel = f64::INFINITY;
        for (variant_idx, outcome) in outcomes.iter().enumerate() {
            let rel = outcome.true_residual_norm / rhs_norm;
            println!(
                "    {:<20} converged={:<5} true_res={:.3e} rel={:.3e}",
                outcome.kind.label(),
                outcome.converged,
                outcome.true_residual_norm,
                rel,
            );
            if outcome.converged {
                converged_counts[variant_idx] += 1;
            }
            relative_residuals[variant_idx].push(rel);
            if rel < best_rel {
                best_rel = rel;
                best_variant_idx = variant_idx;
            }
        }
        win_counts[best_variant_idx] += 1;
    }

    println!("=== aggregate over {} systems ===", systems.len());
    for (variant_idx, &kind) in variants.iter().enumerate() {
        let mut residuals = relative_residuals[variant_idx].clone();
        residuals.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let median = residuals[residuals.len() / 2];
        println!(
            "{:<20} converged={:>3}/{} wins={:>3}/{} median_rel_res={:.3e}",
            kind.label(),
            converged_counts[variant_idx],
            systems.len(),
            win_counts[variant_idx],
            systems.len(),
            median,
        );
    }
}

#[derive(Clone)]
struct BundleRow {
    label: String,
    relative_tolerance: f64,
    max_iterations: usize,
    smoother: CprFineSmootherKind,
    // Step 10.1 follow-up (`FIM-LINEAR-008` reopened): when true and the captured system has
    // an `EquationScaling`, require every equation family (not just the whole-system norm) to
    // clear its own relative-reduction target — see `EquationScaling::family_peaks`/
    // `within_relative_reduction` and `gmres_block_jacobi::solve_with_cpr_fine_smoother`'s
    // `family_ok` closure.
    family_aware: bool,
}

fn run_bundle_row(row: &BundleRow, system: &FimCapturedSystem) -> (bool, usize, f64) {
    let options = FimLinearSolveOptions {
        kind: FimLinearSolverKind::FgmresCpr,
        relative_tolerance: row.relative_tolerance,
        // Step 10.0: at OPM-equivalent looseness the old 1e-10 absolute floor is vestigial
        // across the observed rhs_norm range (5.2e-2 to 2.5e3) — drop it to a tiny
        // degenerate-case guard rather than let it silently dominate at small rhs_norm.
        absolute_tolerance: 1e-12,
        max_iterations: row.max_iterations,
        ..FimLinearSolveOptions::default()
    };
    let equation_scaling = row
        .family_aware
        .then(|| system.equation_scaling.as_ref())
        .flatten();
    let report = solve_with_smoother_and_restriction(
        &system.jacobian,
        &system.rhs,
        &options,
        system.layout,
        row.smoother,
        CprPressureRestrictionKind::QuasiImpes,
        equation_scaling,
    );
    let true_res = residual_norm(&system.jacobian, &report.solution, &system.rhs);
    (report.converged, report.iterations, true_res)
}

/// Phase 10 Step 10.3: tests the OPM-recipe bundle (loosened tolerance + reduced iteration
/// budget + block-ILU0) together against a captured corpus, using `QuasiImpes` throughout
/// (already the live restriction as of Step 9.3) — directly testing whether the Step 9.3
/// wall-clock regression was an artifact of the old, much tighter tolerance philosophy, or
/// an intrinsic property of the restriction choice. Iteration count (not offline wall-clock,
/// which doesn't map 1:1 to live wasm timing) is the comparable metric across rows.
///
/// Manual lab entry point; requires FIM_CAPTURE_DIR pointing at a captured corpus. Run once
/// per corpus (heavy, then bounded) — this test does not itself iterate multiple corpora.
#[test]
#[ignore]
fn solver_lab_compare_bundle_tolerance_iterations() {
    let dir = capture_dir_from_env()
        .expect("set FIM_CAPTURE_DIR to a directory produced by a capture driver run");
    let systems = load_captures(&dir).expect("load captured systems");
    assert!(
        !systems.is_empty(),
        "no fim_capture_*.txt files in {}",
        dir.display()
    );

    println!(
        "bundle lab: {} captured systems from {}",
        systems.len(),
        dir.display()
    );

    let rows: Vec<BundleRow> = vec![
        BundleRow {
            label: "baseline: tol=1e-7 iter=150 ilu0".to_string(),
            relative_tolerance: 1e-7,
            max_iterations: 150,
            smoother: CprFineSmootherKind::FullIlu0,
            family_aware: false,
        },
        BundleRow {
            label: "tol=5e-3 iter=150 ilu0".to_string(),
            relative_tolerance: 5e-3,
            max_iterations: 150,
            smoother: CprFineSmootherKind::FullIlu0,
            family_aware: false,
        },
        BundleRow {
            label: "tol=5e-3 iter=50 ilu0".to_string(),
            relative_tolerance: 5e-3,
            max_iterations: 50,
            smoother: CprFineSmootherKind::FullIlu0,
            family_aware: false,
        },
        BundleRow {
            label: "tol=5e-3 iter=30 ilu0".to_string(),
            relative_tolerance: 5e-3,
            max_iterations: 30,
            smoother: CprFineSmootherKind::FullIlu0,
            family_aware: false,
        },
        BundleRow {
            label: "tol=5e-3 iter=20 ilu0".to_string(),
            relative_tolerance: 5e-3,
            max_iterations: 20,
            smoother: CprFineSmootherKind::FullIlu0,
            family_aware: false,
        },
        BundleRow {
            label: "tol=5e-3 iter=50 block-ilu0".to_string(),
            relative_tolerance: 5e-3,
            max_iterations: 50,
            smoother: CprFineSmootherKind::BlockIlu0,
            family_aware: false,
        },
        BundleRow {
            label: "tol=5e-3 iter=30 block-ilu0".to_string(),
            relative_tolerance: 5e-3,
            max_iterations: 30,
            smoother: CprFineSmootherKind::BlockIlu0,
            family_aware: false,
        },
        BundleRow {
            label: "tol=5e-3 iter=20 block-ilu0".to_string(),
            relative_tolerance: 5e-3,
            max_iterations: 20,
            smoother: CprFineSmootherKind::BlockIlu0,
            family_aware: false,
        },
    ];

    println!("=== per-row aggregate over {} systems ===", systems.len());
    for row in &rows {
        let mut converged_count = 0usize;
        let mut iterations: Vec<usize> = Vec::with_capacity(systems.len());
        let mut relative_residuals: Vec<f64> = Vec::with_capacity(systems.len());

        for system in &systems {
            let rhs_norm = system.rhs.norm().max(f64::EPSILON);
            let (converged, iters, true_res) = run_bundle_row(row, system);
            if converged {
                converged_count += 1;
            }
            iterations.push(iters);
            relative_residuals.push(true_res / rhs_norm);
        }

        iterations.sort_unstable();
        relative_residuals.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let median_iters = iterations[iterations.len() / 2];
        let mean_iters: f64 = iterations.iter().sum::<usize>() as f64 / iterations.len() as f64;
        let median_rel_res = relative_residuals[relative_residuals.len() / 2];

        println!(
            "{:<36} converged={:>3}/{} median_iters={:>4} mean_iters={:>7.1} median_rel_res={:.3e}",
            row.label,
            converged_count,
            systems.len(),
            median_iters,
            mean_iters,
            median_rel_res,
        );
    }
}

/// Step 10.1 follow-up (`FIM-LINEAR-008` reopened): the live heavy-case regression traced back
/// to `perf@1299` being repeatedly left a small relative-residual multiple over tolerance by the
/// whole-system norm, even though the norm itself was satisfied. This test measures, on the
/// currently-live bundle (`tol=5e-3 iter=20 block-ilu0`), (a) how large that per-family
/// overshoot actually is on this corpus's captured systems (using `EquationScaling` recorded at
/// capture time — requires corpora captured after the capture-format v2 change), and (b) whether
/// requiring every family to clear its own relative-reduction target (`family_aware: true`)
/// still converges at a comparable rate, or costs materially more iterations. A real offline win
/// here is the required gate before wiring `family_ok` on by default in the live path.
///
/// Manual lab entry point; requires FIM_CAPTURE_DIR pointing at a corpus captured with
/// `equation_scaling` present (recapture after the `fim-capture-v2` format change).
#[test]
#[ignore]
fn solver_lab_compare_family_aware_convergence() {
    let dir = capture_dir_from_env()
        .expect("set FIM_CAPTURE_DIR to a directory produced by a capture driver run");
    let systems = load_captures(&dir).expect("load captured systems");
    assert!(
        !systems.is_empty(),
        "no fim_capture_*.txt files in {}",
        dir.display()
    );

    let with_scaling = systems
        .iter()
        .filter(|system| system.equation_scaling.is_some())
        .count();
    println!(
        "family-aware lab: {} captured systems from {} ({} carry equation_scaling)",
        systems.len(),
        dir.display(),
        with_scaling
    );
    assert!(
        with_scaling > 0,
        "no captured system carries equation_scaling — recapture with the fim-capture-v2 format \
         before running this test"
    );

    let base = BundleRow {
        label: "live bundle: tol=5e-3 iter=20 block-ilu0".to_string(),
        relative_tolerance: 5e-3,
        max_iterations: 20,
        smoother: CprFineSmootherKind::BlockIlu0,
        family_aware: false,
    };
    let family_aware = BundleRow {
        label: "live bundle + family-aware convergence".to_string(),
        family_aware: true,
        ..base.clone()
    };

    let mut base_converged = 0usize;
    let mut family_converged = 0usize;
    let mut base_iterations = Vec::with_capacity(systems.len());
    let mut family_iterations = Vec::with_capacity(systems.len());
    let mut overshoots: Vec<(String, &'static str, f64)> = Vec::new();

    for (index, system) in systems.iter().enumerate() {
        let (base_ok, base_iters, base_true_res) = run_bundle_row(&base, system);
        if base_ok {
            base_converged += 1;
        }
        base_iterations.push(base_iters);

        if let Some(scaling) = &system.equation_scaling {
            let options = FimLinearSolveOptions {
                kind: FimLinearSolverKind::FgmresCpr,
                relative_tolerance: base.relative_tolerance,
                absolute_tolerance: 1e-12,
                max_iterations: base.max_iterations,
                ..FimLinearSolveOptions::default()
            };
            let base_report = solve_with_smoother_and_restriction(
                &system.jacobian,
                &system.rhs,
                &options,
                system.layout,
                base.smoother,
                CprPressureRestrictionKind::QuasiImpes,
                None,
            );
            let base_residual =
                residual_vector(&system.jacobian, &base_report.solution, &system.rhs);
            let (worst_label, worst_ratio) = worst_family_overshoot(
                scaling,
                &system.rhs,
                &base_residual,
                options.absolute_tolerance,
                options.relative_tolerance,
            );
            let _ = base_true_res;
            overshoots.push((format!("capture {index:05}"), worst_label, worst_ratio));
        }

        let (family_ok, family_iters, _) = run_bundle_row(&family_aware, system);
        if family_ok {
            family_converged += 1;
        }
        family_iterations.push(family_iters);
    }

    overshoots.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
    println!("=== worst per-family overshoot under the current (non-family-aware) bundle ===");
    println!(
        "(ratio > 1.0 means that family is left under-resolved relative to its own target \
         even though the whole-system norm may already be satisfied)"
    );
    for (label, family, ratio) in overshoots.iter().take(15) {
        println!("{label:<16} worst_family={family:<18} overshoot={ratio:.2}x");
    }
    let overshooting = overshoots
        .iter()
        .filter(|(_, _, ratio)| *ratio > 1.0)
        .count();
    println!(
        "{overshooting}/{} systems have at least one family left over its own target",
        overshoots.len()
    );

    base_iterations.sort_unstable();
    family_iterations.sort_unstable();
    let median = |values: &[usize]| values[values.len() / 2];
    let mean = |values: &[usize]| values.iter().sum::<usize>() as f64 / values.len() as f64;

    println!(
        "=== convergence comparison over {} systems ===",
        systems.len()
    );
    println!(
        "{:<36} converged={:>3}/{} median_iters={:>4} mean_iters={:>7.1}",
        base.label,
        base_converged,
        systems.len(),
        median(&base_iterations),
        mean(&base_iterations),
    );
    println!(
        "{:<36} converged={:>3}/{} median_iters={:>4} mean_iters={:>7.1}",
        family_aware.label,
        family_converged,
        systems.len(),
        median(&family_iterations),
        mean(&family_iterations),
    );
}

/// Phase 11 (`FIM-LINEAR-010`): offline correctness + iteration-cost check for well/perforation
/// Schur elimination (`fim/linear/well_schur.rs`) against real captured systems, before any live
/// wiring. Compares the live bundle's default dispatch (`eliminate_wells: false`, current
/// production behavior) against the same options with `eliminate_wells: true` on every captured
/// system: both should converge to closely matching solutions (both solve the same underlying
/// system, just via a different reduction), and the run reports iteration-count deltas.
///
/// Manual lab entry point; requires FIM_CAPTURE_DIR pointing at a corpus captured with
/// `equation_scaling` present (`fim-capture-v2` format).
#[test]
#[ignore]
fn solver_lab_compare_well_elimination() {
    let dir = capture_dir_from_env()
        .expect("set FIM_CAPTURE_DIR to a directory produced by a capture driver run");
    let systems = load_captures(&dir).expect("load captured systems");
    assert!(
        !systems.is_empty(),
        "no fim_capture_*.txt files in {}",
        dir.display()
    );

    println!(
        "well-elimination lab: {} captured systems from {}",
        systems.len(),
        dir.display()
    );

    let mut baseline_converged = 0usize;
    let mut eliminated_converged = 0usize;
    let mut baseline_iterations = Vec::with_capacity(systems.len());
    let mut eliminated_iterations = Vec::with_capacity(systems.len());
    let mut max_solution_delta = 0.0_f64;
    let mut worst_delta_capture = 0usize;

    for (index, system) in systems.iter().enumerate() {
        let Some(layout) = system.layout else {
            continue;
        };
        if layout.well_bhp_count == 0 && layout.perforation_tail_start >= system.jacobian.rows() {
            continue;
        }

        let baseline_options = FimLinearSolveOptions {
            eliminate_wells: false,
            ..FimLinearSolveOptions::default()
        };
        let eliminated_options = FimLinearSolveOptions {
            eliminate_wells: true,
            ..FimLinearSolveOptions::default()
        };

        let baseline_report = solve_linearized_system(
            &system.jacobian,
            &system.rhs,
            &baseline_options,
            system.layout,
            system.equation_scaling.as_ref(),
        );
        let eliminated_report = solve_linearized_system(
            &system.jacobian,
            &system.rhs,
            &eliminated_options,
            system.layout,
            system.equation_scaling.as_ref(),
        );

        if baseline_report.converged {
            baseline_converged += 1;
        }
        if eliminated_report.converged {
            eliminated_converged += 1;
        }
        baseline_iterations.push(baseline_report.iterations);
        eliminated_iterations.push(eliminated_report.iterations);

        let mut delta = 0.0_f64;
        for row in 0..system.jacobian.rows() {
            delta =
                delta.max((baseline_report.solution[row] - eliminated_report.solution[row]).abs());
        }
        if delta > max_solution_delta {
            max_solution_delta = delta;
            worst_delta_capture = index;
        }
    }

    baseline_iterations.sort_unstable();
    eliminated_iterations.sort_unstable();
    let median = |values: &[usize]| {
        if values.is_empty() {
            0
        } else {
            values[values.len() / 2]
        }
    };
    let mean = |values: &[usize]| {
        if values.is_empty() {
            0.0
        } else {
            values.iter().sum::<usize>() as f64 / values.len() as f64
        }
    };

    println!(
        "=== well-elimination comparison over {} systems ===",
        baseline_iterations.len()
    );
    println!(
        "baseline (no elimination):   converged={:>3}/{} median_iters={:>4} mean_iters={:>7.1}",
        baseline_converged,
        baseline_iterations.len(),
        median(&baseline_iterations),
        mean(&baseline_iterations),
    );
    println!(
        "eliminated (well Schur):     converged={:>3}/{} median_iters={:>4} mean_iters={:>7.1}",
        eliminated_converged,
        eliminated_iterations.len(),
        median(&eliminated_iterations),
        mean(&eliminated_iterations),
    );
    println!(
        "max solution delta across all systems: {max_solution_delta:.3e} (capture {worst_delta_capture:05})"
    );
}

/// Bundle P (`FIM-BUNDLE-P`) P0.1: per-phase CPR preconditioner build-cost breakdown over a
/// captured corpus, using the exact production entry point (`solve_linearized_system`, default
/// options — well elimination included, matching `eliminate_wells: true`) so the timings
/// reflect what the live path actually builds against. Decides whether P2 (LU factorization
/// instead of an explicit dense inverse for the coarse pressure operator) matters independently
/// of P1 (setup reuse) — see `docs/FIM_BUNDLE_P_PLAN.md`.
///
/// Manual lab entry point; requires `FIM_CAPTURE_DIR` pointing at a captured corpus.
#[test]
#[ignore]
fn solver_lab_cpr_build_cost_breakdown() {
    let dir = capture_dir_from_env()
        .expect("set FIM_CAPTURE_DIR to a directory produced by a capture driver run");
    let systems = load_captures(&dir).expect("load captured systems");
    assert!(
        !systems.is_empty(),
        "no fim_capture_*.txt files in {}",
        dir.display()
    );

    println!(
        "build-cost breakdown lab: {} captured systems from {}",
        systems.len(),
        dir.display()
    );

    let mut weights_ms = Vec::with_capacity(systems.len());
    let mut coarse_assembly_ms = Vec::with_capacity(systems.len());
    let mut coarse_factorization_ms = Vec::with_capacity(systems.len());
    let mut fine_smoother_ms = Vec::with_capacity(systems.len());
    let mut block_inverses_ms = Vec::with_capacity(systems.len());
    let mut build_fraction_of_total = Vec::with_capacity(systems.len());
    let mut with_timing = 0usize;

    for system in &systems {
        let options = FimLinearSolveOptions::default();
        let report = solve_linearized_system(
            &system.jacobian,
            &system.rhs,
            &options,
            system.layout,
            system.equation_scaling.as_ref(),
        );
        let Some(timing) = report
            .cpr_diagnostics
            .as_ref()
            .and_then(|diagnostics| diagnostics.build_timing)
        else {
            continue;
        };
        with_timing += 1;
        weights_ms.push(timing.weights_ms);
        coarse_assembly_ms.push(timing.coarse_assembly_ms);
        coarse_factorization_ms.push(timing.coarse_factorization_ms);
        fine_smoother_ms.push(timing.fine_smoother_ms);
        block_inverses_ms.push(timing.block_inverses_ms);
        if report.total_time_ms > 0.0 {
            build_fraction_of_total.push(report.preconditioner_build_time_ms / report.total_time_ms);
        }
    }

    assert!(
        with_timing > 0,
        "no captured system produced CPR build timing — check that FimLinearSolveOptions::default() \
         actually dispatches to the CPR/FGMRES backend"
    );

    fn median(values: &mut [f64]) -> f64 {
        values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        values[values.len() / 2]
    }

    println!("=== per-phase build-cost medians over {with_timing} systems (ms) ===");
    println!(
        "weights_ms              median={:.4}",
        median(&mut weights_ms)
    );
    println!(
        "coarse_assembly_ms      median={:.4}",
        median(&mut coarse_assembly_ms)
    );
    println!(
        "coarse_factorization_ms median={:.4}",
        median(&mut coarse_factorization_ms)
    );
    println!(
        "fine_smoother_ms        median={:.4}",
        median(&mut fine_smoother_ms)
    );
    println!(
        "block_inverses_ms       median={:.4}",
        median(&mut block_inverses_ms)
    );
    if !build_fraction_of_total.is_empty() {
        println!(
            "preconditioner_build_time_ms / total_time_ms median={:.3}",
            median(&mut build_fraction_of_total)
        );
    }
}

/// Reduces a captured `(jacobian, rhs, layout)` triple by Schur-eliminating the well/perforation
/// tail (mirrors `eliminate_wells: true`, the production default) so the offline reuse study
/// below builds/reuses a CPR preconditioner against the exact same reduced system the live path
/// actually solves against. Falls through unchanged when there is no tail to eliminate. Returns
/// `None` only when the captured system has no layout at all.
fn reduced_system_for_lab(
    system: &FimCapturedSystem,
) -> Option<(CsMat<f64>, DVector<f64>, FimLinearBlockLayout)> {
    let layout = system.layout?;
    match well_schur::eliminate_wells(
        &system.jacobian,
        &system.rhs,
        layout,
        system.equation_scaling.as_ref(),
    ) {
        Some(elimination) => Some((
            elimination.reduced_jacobian,
            elimination.reduced_rhs,
            elimination.reduced_layout,
        )),
        None => Some((system.jacobian.clone(), system.rhs.clone(), layout)),
    }
}

/// Bundle P (`FIM-BUNDLE-P`) P0.2: offline CPR-setup-reuse staleness study. For every pair of
/// consecutive-in-file-order captured systems `(i, i+k)` (`k` up to OPM's own reuse interval,
/// 30) whose *reduced* (well-Schur-eliminated) systems share the same row count and block
/// layout, builds the preconditioner once on system `i` and reuses it — stale — to solve system
/// `i+k`, comparing iteration count and convergence against a fresh-preconditioner baseline
/// solved directly on `i+k`. Pairs whose reduced systems don't share a key are skipped (the same
/// rebuild-on-key-mismatch rule the live cache will use — no reuse would happen there anyway).
///
/// Offline gate (`docs/FIM_BUNDLE_P_PLAN.md` P0.2): median inflation <= +2 iterations and no new
/// convergence failures (fresh converged, reused did not) at k <= 30. Asserted, not just printed
/// — a failure here means live wiring is not attempted and `FIM-BUNDLE-P` closes REFUTED.
///
/// Manual lab entry point; requires `FIM_CAPTURE_SEQUENCE_DIR` (not `FIM_CAPTURE_DIR` — that
/// corpus is failures-only and not a consecutive sequence) pointing at a corpus captured by
/// setting that env var on a capture-driver run.
#[test]
#[ignore]
fn solver_lab_cpr_reuse_inflation_study() {
    let dir = super::capture::capture_sequence_dir_from_env().expect(
        "set FIM_CAPTURE_SEQUENCE_DIR to a directory produced by a capture driver run with \
         FIM_CAPTURE_SEQUENCE_DIR set (not FIM_CAPTURE_DIR — that corpus is failures-only and \
         not a consecutive sequence)",
    );
    let systems = load_captures(&dir).expect("load captured systems");
    assert!(
        !systems.is_empty(),
        "no fim_capture_*.txt files in {}",
        dir.display()
    );

    println!(
        "reuse-inflation lab: {} captured systems from {}",
        systems.len(),
        dir.display()
    );

    let reduced: Vec<Option<(CsMat<f64>, DVector<f64>, FimLinearBlockLayout)>> =
        systems.iter().map(reduced_system_for_lab).collect();

    const MAX_K: usize = 30;
    let options = FimLinearSolveOptions::default();
    let fine_smoother_kind = CprFineSmootherKind::BlockIlu0;
    let restriction_kind = CprPressureRestrictionKind::QuasiImpes;

    // Each target system's fresh-preconditioner baseline doesn't depend on which origin `i`
    // reused a stale preconditioner into it — compute it once per system instead of once per
    // (i, k) pair (a given target is the `i+k` for up to `MAX_K` different origins).
    let fresh_reports: Vec<Option<(usize, bool)>> = reduced
        .iter()
        .map(|entry| {
            let (jacobian, rhs, layout) = entry.as_ref()?;
            let report =
                solve_linearized_system(jacobian, rhs, &options, Some(*layout), None);
            Some((report.iterations, report.converged))
        })
        .collect();

    let mut inflations: Vec<i64> = Vec::new();
    let mut new_failures = 0usize;
    let mut pairs_evaluated = 0usize;
    let mut pairs_skipped_key_mismatch = 0usize;
    // Bundle P P0.2 follow-up: bucket by staleness distance `k` so a gate failure can
    // distinguish "breaks at any k" (REFUTED) from "only breaks past some smaller k" (adjust
    // the target reuse interval instead of abandoning the bundle).
    let mut by_k_total = vec![0usize; MAX_K + 1];
    let mut by_k_failures = vec![0usize; MAX_K + 1];

    for i in 0..reduced.len() {
        let Some((build_jacobian, _build_rhs, build_layout)) = &reduced[i] else {
            continue;
        };
        let max_k = MAX_K.min(reduced.len().saturating_sub(i + 1));
        if max_k == 0 {
            continue;
        }
        // Build once per origin system `i`, reuse across every `k` — rebuilding per (i, k) pair
        // would redo the O(n^3) coarse factorization up to `MAX_K`x more than necessary.
        let handle = gmres_block_jacobi::build_preconditioner_for_lab(
            build_jacobian,
            Some(*build_layout),
            fine_smoother_kind,
            restriction_kind,
        );
        for k in 1..=max_k {
            let Some((solve_jacobian, solve_rhs, solve_layout)) = &reduced[i + k] else {
                continue;
            };
            if solve_jacobian.rows() != build_jacobian.rows() || solve_layout != build_layout {
                pairs_skipped_key_mismatch += 1;
                continue;
            }

            let (fresh_iterations, fresh_converged) =
                fresh_reports[i + k].expect("reduced[i + k] is Some, so fresh_reports[i + k] is too");
            let reused = gmres_block_jacobi::solve_with_prebuilt_preconditioner(
                solve_jacobian,
                solve_rhs,
                &options,
                &handle,
                None,
            );

            pairs_evaluated += 1;
            inflations.push(reused.iterations as i64 - fresh_iterations as i64);
            by_k_total[k] += 1;
            if fresh_converged && !reused.converged {
                new_failures += 1;
                by_k_failures[k] += 1;
            }
        }
    }

    println!("=== new-convergence-failures by staleness distance k ===");
    for k in 1..=MAX_K {
        if by_k_total[k] > 0 {
            println!(
                "k={k:>2} failures={:>4}/{:<4} ({:.1}%)",
                by_k_failures[k],
                by_k_total[k],
                100.0 * by_k_failures[k] as f64 / by_k_total[k] as f64
            );
        }
    }

    assert!(
        pairs_evaluated > 0,
        "no matching-key consecutive pairs found in {} systems ({} skipped for key mismatch) — \
         corpus too small or too fragmented to run the staleness study",
        reduced.len(),
        pairs_skipped_key_mismatch
    );

    inflations.sort_unstable();
    let median_inflation = inflations[inflations.len() / 2];
    let mean_inflation: f64 = inflations.iter().sum::<i64>() as f64 / inflations.len() as f64;

    println!(
        "=== reuse-inflation over {pairs_evaluated} matching-key pairs ({pairs_skipped_key_mismatch} skipped for key mismatch) ==="
    );
    println!(
        "median_inflation_iters={median_inflation} mean_inflation_iters={mean_inflation:.2} new_convergence_failures={new_failures}"
    );

    assert!(
        median_inflation <= 2,
        "median reuse inflation {median_inflation} exceeds the offline gate of +2 iterations \
         (FIM_BUNDLE_P_PLAN.md P0.2) — do not attempt live wiring, close FIM-BUNDLE-P REFUTED"
    );
    assert_eq!(
        new_failures, 0,
        "{new_failures} pairs converged fresh but failed to converge when reusing a stale \
         preconditioner — offline gate (P0.2) failed, do not attempt live wiring"
    );
}
