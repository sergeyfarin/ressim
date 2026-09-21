//! The black-oil engine's payload boundary: what a consumer may read, in Rust terms.
//!
//! **Why this module exists.** Every accessor here used to be a `JsValue`-returning function in
//! [`crate::frontend`], which made the browser the only target that could read a rate history.
//! The data was never browser-shaped — `Well`, `TimePointRates` and `FimStepStats` already derive
//! `Serialize`, and those functions were one-line calls to `serde_wasm_bindgen::to_value`. The
//! `JsValue` was the *encoding*, not the content.
//!
//! **The rule this module enforces.** The engine never names a target type: no `JsValue` here,
//! and equally no `PyObject`. A consumer converts on its own side —
//! [`crate::frontend`] for the browser, `crates/ressim-py` for Python — and each conversion is
//! one line with no decision in it. That is what keeps the answer independent of which targets
//! exist, including ones nobody has proposed.
//!
//! **Borrowed, not cloned.** These return slices and references. A caller that needs owned data
//! says so; a caller that is about to serialize does not pay for a copy first. The browser's
//! `getRateHistorySince` is called once per step, so this is not hypothetical.
//!
//! Design: `docs/ENGINE_PAYLOAD_BOUNDARY_DESIGN_2026-09-21.md`, Phase 1. `compositional/api.rs`
//! is the same pattern for the compositional model, and is where this one's shape comes from.

use serde::{Deserialize, Serialize};

use crate::pvt;
use crate::reporting::{FimStepStats, TimePointRates};
use crate::well::Well;
use crate::{ReservoirSimulator, SweepConfig, ThreePhaseScalTables};

/// A grid state, as a consumer supplies or receives one.
///
/// This is the **schema**, and it used to live inside `frontend.rs` as a private
/// `GridStatePayload` — which meant the browser owned the definition of what a saved grid is.
/// `sat_gas` and `rs` are optional because two-phase states predate them; absent means zero.
///
/// No version field yet, deliberately. Adding one is now a small change — the type is named and
/// in one place — but it is a *behaviour* decision about what to do with a payload that lacks it,
/// so it belongs in its own change rather than inside a refactor. See
/// `docs/ENGINE_PAYLOAD_BOUNDARY_DESIGN_2026-09-21.md` §6.2.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GridState {
    pub pressure: Vec<f64>,
    pub sat_water: Vec<f64>,
    pub sat_oil: Vec<f64>,
    pub sat_gas: Option<Vec<f64>>,
    pub rs: Option<Vec<f64>>,
}

impl ReservoirSimulator {
    /// Grid dimensions as `[nx, ny, nz]`.
    pub fn dimensions(&self) -> [usize; 3] {
        [self.nx, self.ny, self.nz]
    }

    /// Every well, in the order they were added.
    pub fn wells(&self) -> &[Well] {
        &self.wells
    }

    /// The whole rate history, one entry per completed report step.
    pub fn rate_history(&self) -> &[TimePointRates] {
        &self.rate_history
    }

    /// The rate history from `start_index` onward.
    ///
    /// An out-of-range index yields an empty slice rather than an error: the caller is a consumer
    /// asking "what happened since I last looked", and having looked past the end is a normal
    /// answer to that question, not a fault.
    pub fn rate_history_since(&self, start_index: usize) -> &[TimePointRates] {
        &self.rate_history[start_index.min(self.rate_history.len())..]
    }

    /// The most recent rate-history point, or `None` before the first completed step.
    ///
    /// Exists separately from [`Self::rate_history_since`] because the browser worker's per-step
    /// termination check needs exactly one point, and serializing the undelivered tail to get it
    /// is the kind of waste that only shows up on long runs.
    pub fn latest_rate_point(&self) -> Option<&TimePointRates> {
        self.rate_history.last()
    }

    /// Solver statistics for the most recent FIM step, or `None` when none has run.
    ///
    /// Consumed by `scripts/fim-wasm-diagnostic.mjs`, which produced the convergence baselines in
    /// `docs/FIM_STATUS.md`.
    pub fn last_fim_step_stats(&self) -> Option<&FimStepStats> {
        self.last_fim_step_stats.as_ref()
    }

    /// FIM solver statistics for every step so far.
    pub fn fim_step_stats_history(&self) -> &[FimStepStats] {
        &self.fim_step_stats_history
    }

    // ---- grid state ----------------------------------------------------------------------

    /// Borrowed cell arrays. No copy, no schema, no encoding — the cheapest thing the engine can
    /// hand out, and what every target's zero-copy path is built from.
    pub fn pressure_slice(&self) -> &[f64] {
        &self.pressure
    }

    /// See [`Self::pressure_slice`].
    pub fn sat_water_slice(&self) -> &[f64] {
        &self.sat_water
    }

    /// See [`Self::pressure_slice`].
    pub fn sat_oil_slice(&self) -> &[f64] {
        &self.sat_oil
    }

    /// See [`Self::pressure_slice`].
    pub fn sat_gas_slice(&self) -> &[f64] {
        &self.sat_gas
    }

    /// See [`Self::pressure_slice`].
    pub fn rs_slice(&self) -> &[f64] {
        &self.rs
    }

    /// The grid state as a portable payload, in the shape [`Self::apply_state`] accepts.
    ///
    /// **This copies, and the browser deliberately does not use it.** `frontend.rs` keeps a
    /// zero-copy path that wraps the slices above in `Float64Array` views, because the worker
    /// posts grid state every step and five array copies per step is a real cost. Both read the
    /// same fields, so what is shared between targets is the *schema*, not the representation —
    /// and the native/wasm parity gate asserts the two carry identical values, so the
    /// optimization cannot silently drift from the contract it is optimizing.
    ///
    /// `sat_gas` and `rs` are always `Some` here: the optionality in [`GridState`] exists for
    /// two-phase payloads arriving from outside, not for states the engine produces.
    pub fn grid_state(&self) -> GridState {
        GridState {
            pressure: self.pressure.clone(),
            sat_water: self.sat_water.clone(),
            sat_oil: self.sat_oil.clone(),
            sat_gas: Some(self.sat_gas.clone()),
            rs: Some(self.rs.clone()),
        }
    }

    // ---- configuration payloads ---------------------------------------------------------
    //
    // Each of these was a `JsValue`-taking function in `frontend.rs`, and each carried its own
    // validation there — which put the rules about what a valid payload is on the browser's side
    // of the boundary. The rules are the engine's; only the decoding was ever the browser's.

    /// Install a PVT table and re-derive dissolved gas at the current cell pressures.
    pub fn apply_pvt_table(&mut self, rows: Vec<pvt::PvtRow>) -> Result<(), String> {
        let table = pvt::PvtTable::new(rows, self.pvt.c_o);
        for i in 0..self.nx * self.ny * self.nz {
            self.rs[i] = table.interpolate(self.pressure[i]).rs_m3m3;
        }
        self.pvt_table = Some(table);
        Ok(())
    }

    /// Install SWOF/SGOF tables onto an already-configured three-phase SCAL model.
    ///
    /// Ordering is a real constraint, not an implementation detail: the tables refine a model
    /// that must already exist, so supplying them first is a caller error worth naming.
    pub fn apply_three_phase_scal_tables(
        &mut self,
        tables: ThreePhaseScalTables,
    ) -> Result<(), String> {
        tables.validate()?;
        let scal = self
            .scal_3p
            .as_mut()
            .ok_or("Three-phase relperm props must be configured before SWOF/SGOF tables")?;
        scal.tables = Some(tables);
        Ok(())
    }

    /// Set or clear the sweep configuration. `None` clears it.
    pub fn apply_sweep_config(&mut self, config: Option<SweepConfig>) -> Result<(), String> {
        self.sweep_config = config;
        Ok(())
    }

    /// Restore a previously captured run state.
    ///
    /// Diagnostics are cleared rather than restored: a trace and the last solver warning describe
    /// the step that produced them, and carrying them across a restore would attribute them to a
    /// step that never ran. Cumulative volumes are re-derived from the final rate point for the
    /// same reason — they are a consequence of the history, not an independent fact about it.
    pub fn apply_state(
        &mut self,
        time_days: f64,
        grid: GridState,
        wells: Vec<Well>,
        rate_history: Vec<TimePointRates>,
    ) -> Result<(), String> {
        let expected = self.nx * self.ny * self.nz;
        let check = |name: &str, len: usize| -> Result<(), String> {
            if len == expected {
                Ok(())
            } else {
                Err(format!("Mismatch grid size. Expected {expected}, got {name} len: {len}"))
            }
        };
        check("pressure", grid.pressure.len())?;
        check("sat_water", grid.sat_water.len())?;
        check("sat_oil", grid.sat_oil.len())?;
        if let Some(sat_gas) = grid.sat_gas.as_ref() {
            check("sat_gas", sat_gas.len())?;
        }
        if let Some(rs) = grid.rs.as_ref() {
            check("rs", rs.len())?;
        }

        self.time_days = time_days;
        self.pressure = grid.pressure;
        self.sat_water = grid.sat_water;
        self.sat_oil = grid.sat_oil;
        self.sat_gas = grid.sat_gas.unwrap_or_else(|| vec![0.0; expected]);
        self.rs = grid.rs.unwrap_or_else(|| vec![0.0; expected]);
        self.wells = wells;
        self.refresh_well_head_offsets();
        self.rate_history = rate_history;
        self.last_solver_warning.clear();
        self.last_fim_trace.clear();
        self.capture_fim_trace = false;
        self.last_fim_step_stats = None;
        self.fim_step_stats_history.clear();

        if let Some(last) = self.rate_history.last() {
            self.cumulative_injection_m3 = last.total_injection_reservoir;
            self.cumulative_production_m3 = last.total_production_liquid_reservoir;
        }
        Ok(())
    }
}
