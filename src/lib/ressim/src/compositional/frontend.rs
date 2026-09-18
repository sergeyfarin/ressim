//! The WASM edge for the compositional model.
//!
//! **Deliberately thin.** Every decision — what a payload may contain, what is rejected, what a
//! snapshot carries, what a checkpoint is — lives in [`super::api`], which is plain Rust over
//! `serde` and is tested natively by `comp_api_*`. This file converts between `JsValue` and those
//! types and does nothing else, so the plan's "verify native/WASM results on the same tiny
//! fixtures" is true by construction rather than by a second set of tests.
//!
//! Nothing here reaches into the black-oil [`crate::ReservoirSimulator`]. The two are separate
//! wasm-bindgen types on purpose: the black-oil path is validated against analytical solutions and
//! OPM Flow, and C13's exit criterion is that it remains unchanged.

use wasm_bindgen::prelude::*;

use super::api::{CaseConfig, Checkpoint, CompositionalCase, ConfigError};

/// A compositional run, as the browser sees it.
#[wasm_bindgen]
pub struct CompositionalSimulator {
    case: CompositionalCase,
}

/// Errors cross as the serialized [`ConfigError`], so the UI can branch on `kind` and show the
/// field rather than a formatted string it would have to parse.
fn config_error(error: ConfigError) -> JsValue {
    serde_wasm_bindgen::to_value(&error).unwrap_or_else(|_| JsValue::from_str(&error.to_string()))
}

/// Payloads cross as plain JS objects, not as strings.
///
/// The worker already holds a structured-cloneable object; serializing it to JSON only to parse it
/// back is work for nothing, and a string would have to be re-validated as JSON on both sides.
fn parse<T: serde::de::DeserializeOwned>(value: JsValue, what: &str) -> Result<T, JsValue> {
    serde_wasm_bindgen::from_value(value).map_err(|e| {
        config_error(ConfigError::Rejected {
            field: what.to_string(),
            reason: format!("could not be read: {e}"),
        })
    })
}

#[wasm_bindgen]
impl CompositionalSimulator {
    /// Build a case from a `ressim-compositional-case/1` payload.
    ///
    /// Everything unsupported is refused here, before anything is allocated, and the rejection
    /// names the field.
    #[wasm_bindgen(constructor)]
    pub fn new(config: JsValue) -> Result<CompositionalSimulator, JsValue> {
        let config: CaseConfig = parse(config, "config")?;
        let case = CompositionalCase::new(config).map_err(config_error)?;
        Ok(Self { case })
    }

    /// Check a payload without building anything, and return the advisories.
    ///
    /// This is what a scenario-admission check calls: it is how a caller learns that a case is
    /// running on verification-only relative permeability without having to allocate it first.
    #[wasm_bindgen(js_name = validateConfig)]
    pub fn validate_config(config: JsValue) -> Result<JsValue, JsValue> {
        let config: CaseConfig = parse(config, "config")?;
        let advisories = config.validate().map_err(config_error)?;
        serde_wasm_bindgen::to_value(&advisories).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Attempt one step. A failure is **returned**, not thrown: a flash that cannot resolve is
    /// something the UI has to show rather than an exception to swallow.
    #[wasm_bindgen(js_name = step)]
    pub fn step(&mut self, dt_days: f64) -> JsValue {
        let outcome = self.case.step(dt_days);
        serde_wasm_bindgen::to_value(&outcome)
            .unwrap_or_else(|e| JsValue::from_str(&format!("step outcome: {e}")))
    }

    #[wasm_bindgen(js_name = getTime)]
    pub fn get_time(&self) -> f64 {
        self.case.time_days()
    }

    /// The accepted state and its totals. No derivative arrays — see [`super::api::Snapshot`].
    #[wasm_bindgen(js_name = getSnapshot)]
    pub fn get_snapshot(&self) -> JsValue {
        serde_wasm_bindgen::to_value(&self.case.snapshot())
            .unwrap_or_else(|e| JsValue::from_str(&format!("snapshot: {e}")))
    }

    /// A versioned checkpoint, as a plain object the worker can post.
    #[wasm_bindgen(js_name = checkpoint)]
    pub fn checkpoint(&self) -> JsValue {
        serde_wasm_bindgen::to_value(&self.case.checkpoint())
            .unwrap_or_else(|e| JsValue::from_str(&format!("checkpoint: {e}")))
    }

    /// Restore a checkpoint. A payload from another schema is refused rather than reinterpreted.
    #[wasm_bindgen(js_name = restore)]
    pub fn restore(checkpoint: JsValue) -> Result<CompositionalSimulator, JsValue> {
        let checkpoint: Checkpoint = parse(checkpoint, "checkpoint")?;
        let case = CompositionalCase::restore(&checkpoint).map_err(config_error)?;
        Ok(Self { case })
    }
}
