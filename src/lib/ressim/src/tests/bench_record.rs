//! Benchmark records for `docs/BENCHMARKS.md` (#54).
//!
//! A benchmark test calls [`metric`] or [`series`] beside its assertions. With
//! `RESSIM_BENCH_OUT=<dir>` set, each call appends one JSON line to `<dir>/records.jsonl`;
//! without it the calls do nothing, so ordinary test runs are unaffected. The acceptance band is
//! passed from the test's own constant, so the page generated from these records cannot state a
//! band the test does not enforce. `tools/benchmarks/benchmarks.py` collects the lines, stamps
//! them with the commit and renders the page.

use std::io::Write;

use serde_json::{Value, json};

/// One scalar measurement.
pub(crate) struct Metric<'a> {
    /// `BENCHMARKS.md` section key, e.g. `"buckley"`.
    pub section: &'a str,
    /// Case within the section, e.g. `"A"` or `"nx=40"`.
    pub case: &'a str,
    /// What was measured, e.g. `"breakthrough_rel_err"`.
    pub metric: &'a str,
    /// The measured value. Signed where the sign carries meaning.
    pub value: f64,
    /// Acceptance band on `|value|`, when the test enforces one.
    pub band: Option<f64>,
    /// Unit of `value` and `band`: `"frac"` for a relative error, otherwise a physical unit.
    pub unit: &'a str,
    /// What the value is compared against, e.g. `"Welge"` or `"OPM Flow"`.
    pub reference: &'a str,
    /// True when the reference solves the same discrete model, so a gap is a finding rather
    /// than discretization error.
    pub same_model: bool,
    /// Where the value was taken, e.g. `"t=1095 d"`; empty when it is not a point.
    pub at: &'a str,
}

/// Records one scalar measurement when `RESSIM_BENCH_OUT` is set.
pub(crate) fn metric(m: Metric) {
    write(json!({
        "kind": "metric",
        "section": m.section,
        "case": m.case,
        "metric": m.metric,
        "value": m.value,
        "band": m.band,
        "unit": m.unit,
        "reference": m.reference,
        "same_model": m.same_model,
        "at": m.at,
    }));
}

/// Records a time series (`quantity` against `t`, days) for the aggregator to compare against
/// another source's series, e.g. an OPM Flow run of the same case.
pub(crate) fn series(
    section: &str,
    case: &str,
    source: &str,
    quantity: &str,
    t: &[f64],
    v: &[f64],
) {
    write(json!({
        "kind": "series",
        "section": section,
        "case": case,
        "source": source,
        "quantity": quantity,
        "t": t,
        "v": v,
    }));
}

fn write(record: Value) {
    let Some(dir) = std::env::var_os("RESSIM_BENCH_OUT") else {
        return;
    };
    let path = std::path::Path::new(&dir).join("records.jsonl");
    // Tests run on parallel threads; one `write_all` of a whole line to an `O_APPEND` file keeps
    // the lines whole.
    let line = format!("{record}\n");
    let result = std::fs::create_dir_all(&dir).and_then(|()| {
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?
            .write_all(line.as_bytes())
    });
    if let Err(error) = result {
        panic!(
            "RESSIM_BENCH_OUT is set but {} is not writable: {error}",
            path.display()
        );
    }
}

/// The largest `|value|` seen over a run and the time it was seen at, for recording a
/// criterion's worst checkpoint.
#[derive(Clone, Copy)]
pub(crate) struct Worst {
    pub value: f64,
    pub t_days: f64,
}

impl Worst {
    pub(crate) fn new() -> Self {
        Self {
            value: 0.0,
            t_days: 0.0,
        }
    }

    pub(crate) fn update(&mut self, value: f64, t_days: f64) {
        if value.abs() > self.value.abs() {
            *self = Self { value, t_days };
        }
    }

    /// `"t=1095 d"`, the form [`Metric::at`] takes.
    pub(crate) fn at(&self) -> String {
        format!("t={} d", self.t_days)
    }
}
