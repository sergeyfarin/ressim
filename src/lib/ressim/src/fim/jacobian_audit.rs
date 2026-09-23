//! Finite-difference audit of the assembled FIM Jacobian (FIM-KINK-001 sweep). Test-only.
//!
//! Findings of the first sweep are in `docs/FIM_CONVERGENCE_WORKLOG.md` "FIM-KINK-001".
//!
//! Two defects in a row (FIM-DIRECT-001, FIM-BUBBLE-001) had the same signature: a primary sat
//! *exactly* on a clamp, so its derivative depended on which side roundoff put it, and Newton
//! two-cycled. Both were found by comparing the AD Jacobian with one-sided finite differences at
//! the stuck state. This module makes that comparison a sweep, run from inside the live Newton
//! loop of any test:
//!
//! ```bash
//! FIM_JAC_AUDIT=20 cargo test --release --lib <test> -- --nocapture --test-threads=1
//! ```
//!
//! `FIM_JAC_AUDIT` is the number of Newton assemblies to audit per process, `FIM_JAC_AUDIT_EVERY`
//! audits only every Nth one (default 1), and `FIM_JAC_AUDIT_MAX_ROWS` skips larger systems
//! (default 1500). Each audited entry falls in one of three classes:
//!
//! * `kink` — the forward and backward differences disagree, so a non-smooth point lies within
//!   one finite-difference step of the state. `side=fwd|bwd` says which side AD took. A primary
//!   parked on a clamp shows up here.
//! * `kink-neither` — a kink where AD matches neither side.
//! * `WRONG` — the residual is smooth there and AD still disagrees with the central difference: a
//!   derivative bug, whatever the state.
//!
//! `WRONG` is only as good as the one-step comparison behind it. Two kinks of opposite sense on
//! different faces of the same cell cancel in the split test and look smooth; that was the whole
//! of the "10% wrong water-row pressure derivative" in the 12x12x3 water case (two vertical faces
//! with Δp ≈ 3e-4 bar, inside the FD step). Samples inside one cell's own block therefore carry
//! a breakdown: accumulation by AD and by FD, then every face's AD/fwd/bwd, which separates a
//! real derivative error from superposed upwind switches.
//!
//! A kink is not automatically a defect. Upwind switching at zero potential difference is one too,
//! and every simulator has it. It becomes a defect when a primary *lives* on it: the unknown's value
//! (`value=`) sits on the clamp at every audited iteration, as Sg = ±1e-18 did.

use nalgebra::DVector;

use crate::ReservoirSimulator;
use crate::fim::assembly::{FimAssembly, FimAssemblyOptions};
use crate::fim::assembly_ad::assemble_fim_system_ad;
use crate::fim::flow_resv::FlowResvReportStepContext;
use crate::fim::state::{FimState, HydrocarbonState};
use crate::fim::wells::FimWellTopology;

const KINK_SPLIT: f64 = 0.05;
const SIDE_MATCH: f64 = 0.02;
const SMOOTH_MISMATCH: f64 = 0.02;
/// Entries smaller than this fraction of their row's largest entry are below FD resolution.
const ROW_FLOOR: f64 = 1e-8;

fn env_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn step(state: &FimState, col: usize) -> f64 {
    let n_cell = state.n_cell_unknowns();
    if col < n_cell {
        let cell = state.cell(col / 3);
        return match col % 3 {
            0 => 1e-5 * cell.pressure_bar.abs().max(1.0),
            1 => 1e-7,
            _ => 1e-7 * cell.hydrocarbon_var.abs().max(1.0),
        };
    }
    if col < n_cell + state.n_well_unknowns() {
        return 1e-4 * state.well_bhp[col - n_cell].abs().max(1.0);
    }
    let perf = col - n_cell - state.n_well_unknowns();
    1e-4 * state.perforation_primary_value(perf).abs().max(1.0)
}

fn perturb(state: &mut FimState, col: usize, delta: f64) {
    let n_cell = state.n_cell_unknowns();
    if col < n_cell {
        let cell = &mut state.cells[col / 3];
        match col % 3 {
            0 => cell.pressure_bar += delta,
            1 => cell.sw += delta,
            _ => cell.hydrocarbon_var += delta,
        }
    } else if col < n_cell + state.n_well_unknowns() {
        state.well_bhp[col - n_cell] += delta;
    } else {
        let perf = col - n_cell - state.n_well_unknowns();
        *state.perforation_primary_value_mut(perf) += delta;
    }
}

fn row_kind(state: &FimState, row: usize) -> (String, String) {
    let n_cell = state.n_cell_unknowns();
    if row < n_cell {
        let eq = ["water", "oil", "gas"][row % 3];
        return (eq.to_string(), format!("cell{}", row / 3));
    }
    if row < n_cell + state.n_well_unknowns() {
        return ("well".to_string(), format!("well{}", row - n_cell));
    }
    (
        "perf".to_string(),
        format!("perf{}", row - n_cell - state.n_well_unknowns()),
    )
}

/// `(kind, site, value)` of a column's unknown.
fn col_kind(state: &FimState, col: usize) -> (String, String, f64) {
    let n_cell = state.n_cell_unknowns();
    if col < n_cell {
        let cell = state.cell(col / 3);
        let (kind, value) = match col % 3 {
            0 => ("p", cell.pressure_bar),
            1 => ("sw", cell.sw),
            _ => match cell.regime {
                HydrocarbonState::Saturated => ("Sg", cell.hydrocarbon_var),
                HydrocarbonState::Undersaturated => ("Rs", cell.hydrocarbon_var),
            },
        };
        return (kind.to_string(), format!("cell{}", col / 3), value);
    }
    if col < n_cell + state.n_well_unknowns() {
        let well = col - n_cell;
        return (
            "bhp".to_string(),
            format!("well{well}"),
            state.well_bhp[well],
        );
    }
    let perf = col - n_cell - state.n_well_unknowns();
    (
        "q".to_string(),
        format!("perf{perf}"),
        state.perforation_primary_value(perf),
    )
}

/// For an entry inside one cell's own block, the accumulation part of the derivative by AD and by
/// finite differences, plus the cell's state: localises a mismatch to accumulation versus flux/wells.
fn cell_detail(
    sim: &ReservoirSimulator,
    dt_days: f64,
    previous_state: &FimState,
    state: &FimState,
    row: usize,
    col: usize,
) -> Option<String> {
    let n_cell = state.n_cell_unknowns();
    if row >= n_cell || col >= n_cell || row / 3 != col / 3 {
        return None;
    }
    let idx = row / 3;
    let (eq, var) = (row % 3, col % 3);
    let c = state.cell(idx);
    let pc = previous_state.cell(idx);
    let base_rs = (!sim.gas_redissolution_enabled).then(|| sim.rs[idx]);
    let acc = |cell: &crate::fim::state::FimCellState| {
        crate::fim::properties::cell_accumulation_generic::<f64>(
            sim,
            idx,
            cell.pressure_bar,
            cell.sw,
            cell.hydrocarbon_var,
            cell.regime,
            base_rs,
            pc.pressure_bar,
            pc.sw,
            pc.hydrocarbon_var,
            pc.regime,
        )[eq]
    };
    let ad = crate::fim::properties::accumulation_jacobian_block(
        sim,
        idx,
        c.pressure_bar,
        c.sw,
        c.hydrocarbon_var,
        c.regime,
        base_rs,
        pc.pressure_bar,
        pc.sw,
        pc.hydrocarbon_var,
        pc.regime,
    )[eq][var];
    let h = step(state, col);
    let (mut plus, mut minus) = (*c, *c);
    match var {
        0 => {
            plus.pressure_bar += h;
            minus.pressure_bar -= h;
        }
        1 => {
            plus.sw += h;
            minus.sw -= h;
        }
        _ => {
            plus.hydrocarbon_var += h;
            minus.hydrocarbon_var -= h;
        }
    }
    let base = acc(c);
    let rs_sat = sim
        .pvt_table
        .as_ref()
        .map(|t| t.interpolate(c.pressure_bar).rs_m3m3)
        .unwrap_or(0.0);
    let faces =
        crate::fim::assembly_ad::audit_face_breakdown(sim, state, dt_days, idx, eq, idx, var, h)
            .join("; ");
    Some(format!(
        "faces=[{faces}] acc_ad={ad:.4e} acc_fwd={:.4e} acc_bwd={:.4e} | cell p={:.4} sw={:.6} hc={:.6e} {:?} rs_sat(p)={rs_sat:.5} base_rs={base_rs:?} | prev p={:.4} hc={:.6e} {:?}",
        (acc(&plus) - base) / h,
        (base - acc(&minus)) / h,
        c.pressure_bar,
        c.sw,
        c.hydrocarbon_var,
        c.regime,
        pc.pressure_bar,
        pc.hydrocarbon_var,
        pc.regime,
    ))
}

struct Entry {
    row: usize,
    col: usize,
    ad: f64,
    fwd: f64,
    bwd: f64,
}

/// Audit one Newton assembly if the environment asks for it. Prints `JACAUDIT` lines to stderr.
#[allow(clippy::too_many_arguments)]
pub(crate) fn maybe_audit(
    sim: &ReservoirSimulator,
    previous_state: &FimState,
    state: &FimState,
    dt_days: f64,
    iteration: usize,
    topology: &FimWellTopology,
    flow_resv_context: Option<FlowResvReportStepContext>,
    assembly: &FimAssembly,
) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static SEEN: AtomicUsize = AtomicUsize::new(0);
    static DONE: AtomicUsize = AtomicUsize::new(0);

    let limit = env_usize("FIM_JAC_AUDIT", 0);
    if limit == 0 || DONE.load(Ordering::Relaxed) >= limit {
        return;
    }
    let every = env_usize("FIM_JAC_AUDIT_EVERY", 1).max(1);
    if SEEN.fetch_add(1, Ordering::Relaxed) % every != 0 {
        return;
    }
    let n = assembly.residual.len();
    if n > env_usize("FIM_JAC_AUDIT_MAX_ROWS", 1500) {
        return;
    }
    let audit_idx = DONE.fetch_add(1, Ordering::Relaxed);

    let options = FimAssemblyOptions {
        dt_days,
        include_wells: true,
        assemble_residual_only: true,
        topology: Some(topology),
        flow_resv_context,
    };
    let base: &DVector<f64> = &assembly.residual;
    let mut entries = Vec::new();
    let mut row_max = vec![0.0_f64; n];
    for col in 0..n {
        let h = step(state, col);
        let (mut plus, mut minus) = (state.clone(), state.clone());
        perturb(&mut plus, col, h);
        perturb(&mut minus, col, -h);
        let rp = assemble_fim_system_ad(sim, previous_state, &plus, &options).residual;
        let rm = assemble_fim_system_ad(sim, previous_state, &minus, &options).residual;
        for row in 0..n {
            let ad = assembly.jacobian.get(row, col).copied().unwrap_or(0.0);
            let fwd = (rp[row] - base[row]) / h;
            let bwd = (base[row] - rm[row]) / h;
            let sig = ad.abs().max(fwd.abs()).max(bwd.abs());
            if sig == 0.0 || !sig.is_finite() {
                continue;
            }
            row_max[row] = row_max[row].max(sig);
            entries.push(Entry {
                row,
                col,
                ad,
                fwd,
                bwd,
            });
        }
    }

    use std::collections::BTreeMap;
    let mut classes: BTreeMap<(String, String, String), usize> = BTreeMap::new();
    let mut samples: BTreeMap<String, Vec<(f64, String)>> = BTreeMap::new();
    for e in &entries {
        let sig = e.ad.abs().max(e.fwd.abs()).max(e.bwd.abs());
        if sig <= ROW_FLOOR * row_max[e.row] {
            continue;
        }
        let split = (e.fwd - e.bwd).abs() / sig;
        let class = if split > KINK_SPLIT {
            if (e.ad - e.fwd).abs() / sig < SIDE_MATCH {
                "kink side=fwd"
            } else if (e.ad - e.bwd).abs() / sig < SIDE_MATCH {
                "kink side=bwd"
            } else {
                "kink-neither"
            }
        } else if (e.ad - 0.5 * (e.fwd + e.bwd)).abs() / sig > SMOOTH_MISMATCH {
            "WRONG"
        } else {
            continue;
        };
        let (rk, rsite) = row_kind(state, e.row);
        let (ck, csite, value) = col_kind(state, e.col);
        *classes
            .entry((class.to_string(), rk.clone(), ck.clone()))
            .or_default() += 1;
        let weight = if class == "WRONG" {
            split.max((e.ad - 0.5 * (e.fwd + e.bwd)).abs() / sig)
        } else {
            split
        };
        let detail = if class != "" {
            cell_detail(sim, dt_days, previous_state, state, e.row, e.col)
                .map(|d| format!(" || {d}"))
                .unwrap_or_default()
        } else {
            String::new()
        };
        samples.entry(class.to_string()).or_default().push((
            weight,
            format!(
                "row={rk}@{rsite} col={ck}@{csite} value={value:.6e} ad={:.4e} fwd={:.4e} bwd={:.4e}{detail}",
                e.ad, e.fwd, e.bwd
            ),
        ));
    }

    let total: usize = classes.values().sum();
    eprintln!(
        "JACAUDIT audit={audit_idx} t={:.6} dt={dt_days:.4e} iter={iteration} rows={n} flagged={total}",
        sim.time_days
    );
    for ((class, rk, ck), count) in &classes {
        eprintln!("JACAUDIT   class={class} row={rk} col={ck} count={count}");
    }
    for (class, mut list) in samples {
        list.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        for (_, line) in list.into_iter().take(4) {
            eprintln!("JACAUDIT   sample class={class} {line}");
        }
    }
}
