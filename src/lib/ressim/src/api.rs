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

use crate::reporting::{FimStepStats, TimePointRates};
use crate::well::Well;
use crate::ReservoirSimulator;

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
}
