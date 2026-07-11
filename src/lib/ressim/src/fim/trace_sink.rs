//! Native-only, env-gated file trace sink for the heavy-case 18k-substep pathology
//! (`docs/FIM_BUNDLE_N_DESIGN.md` §10 "Late-window trace diagnostic").
//!
//! `fim_trace!`'s existing in-memory `String` sink (`capture_fim_trace`,
//! `ReservoirSimulator::append_fim_trace_line`) is unusable across an 18,002-substep run —
//! it is what made the `--diagnostic step` wasm runner inconclusive and forced the ~176-minute
//! native traced repro used to confirm the pathology. This module lets a native run instead
//! stream trace lines straight to a file, and lets the substep loop narrow tracing to just the
//! dt-collapse window instead of the whole run.
//!
//! Off unless `FIM_TRACE_FILE` is set. The whole module is compiled for wasm too (it only uses
//! `std`, same as `fim/linear/capture.rs`), but every call site is `#[cfg(not(target_arch =
//! "wasm32"))]` — production/wasm behavior is untouched, and reading unset env vars cannot
//! affect solver math (it only ever gates whether/where a line is written).

use std::env;
use std::fs::{self, OpenOptions};
use std::io::{BufWriter, Write as _};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

pub(crate) const TRACE_FILE_ENV: &str = "FIM_TRACE_FILE";
pub(crate) const TRACE_DT_BELOW_ENV: &str = "FIM_TRACE_DT_BELOW";
pub(crate) const TRACE_SUBSTEP_START_ENV: &str = "FIM_TRACE_SUBSTEP_START";
pub(crate) const MAX_SUBSTEPS_ENV: &str = "FIM_MAX_SUBSTEPS";

type SinkWriter = std::fs::File;

fn sink_slot() -> &'static Mutex<Option<BufWriter<SinkWriter>>> {
    static SINK: OnceLock<Mutex<Option<BufWriter<SinkWriter>>>> = OnceLock::new();
    SINK.get_or_init(|| Mutex::new(open_sink_from_env()))
}

fn open_sink_from_env() -> Option<BufWriter<SinkWriter>> {
    let path = PathBuf::from(env::var_os(TRACE_FILE_ENV)?);
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            let _ = fs::create_dir_all(parent);
        }
    }
    match OpenOptions::new().create(true).append(true).open(&path) {
        Ok(file) => Some(BufWriter::new(file)),
        Err(error) => {
            eprintln!("FIM trace sink: failed to open {path:?}: {error}");
            None
        }
    }
}

/// True if `FIM_TRACE_FILE` is set and the sink opened successfully. Cheap after the first
/// call (mutex lock + `Option::is_some`); the file itself is opened lazily exactly once.
pub(crate) fn sink_enabled() -> bool {
    sink_slot()
        .lock()
        .map(|guard| guard.is_some())
        .unwrap_or(false)
}

/// Appends one line (newline-terminated) to the trace file, flushing immediately so a killed
/// or crashed long run does not lose its tail. Errors are swallowed: this is a diagnostic aid
/// and must never abort a solve.
pub(crate) fn write_line(line: &str) {
    if let Ok(mut guard) = sink_slot().lock() {
        if let Some(writer) = guard.as_mut() {
            let _ = writeln!(writer, "{line}");
            let _ = writer.flush();
        }
    }
}

fn parse_dt_below(value: Option<&str>) -> Option<f64> {
    value
        .and_then(|raw| raw.parse::<f64>().ok())
        .filter(|v| v.is_finite() && *v > 0.0)
}

fn parse_substep_start(value: Option<&str>) -> Option<u32> {
    value.and_then(|raw| raw.parse::<u32>().ok())
}

fn parse_max_substeps(value: Option<&str>) -> Option<u32> {
    value
        .and_then(|raw| raw.parse::<u32>().ok())
        .filter(|v| *v > 0)
}

/// `FIM_TRACE_DT_BELOW=<days>`: the window activates once a substep's trial dt drops below
/// this threshold. Self-locating — no prior knowledge of the collapse substep is needed.
pub(crate) fn dt_below_threshold_from_env() -> Option<f64> {
    parse_dt_below(env::var(TRACE_DT_BELOW_ENV).ok().as_deref())
}

/// `FIM_TRACE_SUBSTEP_START=<n>`: companion substep-indexed window trigger, for when the
/// collapse substep is already known from a prior ledger run.
pub(crate) fn substep_start_from_env() -> Option<u32> {
    parse_substep_start(env::var(TRACE_SUBSTEP_START_ENV).ok().as_deref())
}

/// `FIM_MAX_SUBSTEPS=<n>`: overrides the hardcoded substep cap so a windowed rerun can abort
/// shortly after capturing the window instead of running to completion.
pub(crate) fn max_substeps_override_from_env() -> Option<u32> {
    parse_max_substeps(env::var(MAX_SUBSTEPS_ENV).ok().as_deref())
}

/// Returns true if the per-iteration trace window should be (or already is) active for this
/// substep/trial-dt. Checks `sink_enabled()` first so the threshold env vars are never even
/// read when `FIM_TRACE_FILE` is unset — the common (production) case stays a single cheap
/// mutex-guarded boolean check.
pub(crate) fn should_activate_window(trial_dt_days: f64, substep: u32) -> bool {
    if !sink_enabled() {
        return false;
    }
    if let Some(threshold) = dt_below_threshold_from_env() {
        if trial_dt_days < threshold {
            return true;
        }
    }
    if let Some(start) = substep_start_from_env() {
        if substep >= start {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_dt_below_accepts_positive_finite_values_only() {
        assert_eq!(parse_dt_below(Some("1e-3")), Some(1e-3));
        assert_eq!(parse_dt_below(Some("0.5")), Some(0.5));
        assert_eq!(parse_dt_below(Some("0")), None);
        assert_eq!(parse_dt_below(Some("-1")), None);
        assert_eq!(parse_dt_below(Some("nan")), None);
        assert_eq!(parse_dt_below(Some("not-a-number")), None);
        assert_eq!(parse_dt_below(None), None);
    }

    #[test]
    fn parse_substep_start_accepts_any_valid_u32() {
        assert_eq!(parse_substep_start(Some("0")), Some(0));
        assert_eq!(parse_substep_start(Some("17500")), Some(17500));
        assert_eq!(parse_substep_start(Some("-1")), None);
        assert_eq!(parse_substep_start(Some("abc")), None);
        assert_eq!(parse_substep_start(None), None);
    }

    #[test]
    fn parse_max_substeps_rejects_zero_and_garbage() {
        assert_eq!(parse_max_substeps(Some("500")), Some(500));
        assert_eq!(parse_max_substeps(Some("0")), None);
        assert_eq!(parse_max_substeps(Some("abc")), None);
        assert_eq!(parse_max_substeps(None), None);
    }

    #[test]
    fn should_activate_window_is_false_when_sink_disabled_regardless_of_thresholds() {
        // `sink_enabled()` reflects the real (unset in test runs) `FIM_TRACE_FILE` env var,
        // so this exercises the short-circuit that keeps the production/no-op path a single
        // cheap check without touching the threshold env vars at all.
        if sink_enabled() {
            // A prior test process/run left FIM_TRACE_FILE set — nothing to assert here,
            // this test only guards the disabled case.
            return;
        }
        assert!(!should_activate_window(0.0, 0));
        assert!(!should_activate_window(1e-9, 999_999));
    }
}
