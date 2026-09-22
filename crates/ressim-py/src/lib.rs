//! Python bindings for the reservoir simulator.
//!
//! **A second consumer, not a second implementation.** Every method here forwards to
//! `simulator::ReservoirSimulator` and does nothing else: no physics, no defaults, no unit
//! conversion. If a number needs adjusting, it is adjusted in the engine where the Rust suite
//! can see it.
//!
//! **Why a separate crate rather than a shim inside `simulator`.** The engine would otherwise
//! carry two proc-macro attribute systems — `wasm_bindgen` and `pyo3` — on the same types, and
//! every feature combination would become a build matrix to maintain. Depending on the engine as
//! a plain `rlib` with `default-features = false` keeps the physics crate unaware that this
//! exists, and turns S4's feature gate into something the compiler checks on every build:
//! `wasm-bindgen` is not in this binary's dependency graph, and cannot be without an edit here.
//!
//! **Scope is set by what the engine exposes natively, not by what would be nice.** Phase 1 of
//! `docs/ENGINE_PAYLOAD_BOUNDARY_DESIGN_2026-09-21.md` moved the reporting accessors into
//! `simulator::api`, so rates, wells, dimensions and FIM step statistics are now reachable here
//! and this module can say what came out of a run rather than only that one happened.
//!
//! Still `JsValue`-only, pending Phase 2 and 3: `set_pvt_table`, `set_three_phase_scal_tables`,
//! `set_sweep_config`, `load_state` (configuration and restore) and the packed `get_grid_state`.
//! Binding around a gap by reaching into private engine state would put physics in the binding
//! layer, which is the one thing this crate must not do — so the gaps stay visible instead.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pythonize::{depythonize, pythonize};
use simulator::ReservoirSimulator;

/// Engine errors are plain `String`s; surface them as `ValueError` rather than panicking.
fn to_py(result: Result<(), String>) -> PyResult<()> {
    result.map_err(PyValueError::new_err)
}

#[pyclass(name = "Simulator", unsendable)]
pub struct PySimulator {
    inner: ReservoirSimulator,
}

#[pymethods]
impl PySimulator {
    #[new]
    fn new(nx: usize, ny: usize, nz: usize, porosity: f64) -> Self {
        Self { inner: ReservoirSimulator::new(nx, ny, nz, porosity) }
    }

    // ---- configuration -------------------------------------------------------------------

    fn set_fim_enabled(&mut self, enabled: bool) {
        self.inner.set_fim_enabled(enabled);
    }

    fn set_cell_dimensions(&mut self, dx: f64, dy: f64, dz: f64) -> PyResult<()> {
        to_py(self.inner.set_cell_dimensions(dx, dy, dz))
    }

    #[pyo3(signature = (s_wc, s_or, n_w, n_o, k_rw_max=1.0, k_ro_max=1.0))]
    fn set_rel_perm_props(
        &mut self,
        s_wc: f64,
        s_or: f64,
        n_w: f64,
        n_o: f64,
        k_rw_max: f64,
        k_ro_max: f64,
    ) -> PyResult<()> {
        to_py(self.inner.set_rel_perm_props(s_wc, s_or, n_w, n_o, k_rw_max, k_ro_max))
    }

    fn set_fluid_properties(&mut self, mu_o: f64, mu_w: f64) -> PyResult<()> {
        to_py(self.inner.set_fluid_properties(mu_o, mu_w))
    }

    fn set_capillary_params(&mut self, p_entry: f64, lambda: f64) -> PyResult<()> {
        to_py(self.inner.set_capillary_params(p_entry, lambda))
    }

    fn set_initial_pressure(&mut self, pressure: f64) {
        self.inner.set_initial_pressure(pressure);
    }

    fn set_initial_saturation(&mut self, sat_water: f64) {
        self.inner.set_initial_saturation(sat_water);
    }

    fn set_permeability_random_seeded(
        &mut self,
        min_perm: f64,
        max_perm: f64,
        seed: u64,
    ) -> PyResult<()> {
        to_py(self.inner.set_permeability_random_seeded(min_perm, max_perm, seed))
    }

    fn set_stability_params(
        &mut self,
        max_sat_change_per_step: f64,
        max_pressure_change_per_step: f64,
        max_well_rate_change_fraction: f64,
    ) {
        self.inner.set_stability_params(
            max_sat_change_per_step,
            max_pressure_change_per_step,
            max_well_rate_change_fraction,
        );
    }

    #[pyo3(signature = (i, j, k, bhp, well_radius=0.1, skin=0.0, injector=false))]
    #[allow(clippy::too_many_arguments)]
    fn add_well(
        &mut self,
        i: usize,
        j: usize,
        k: usize,
        bhp: f64,
        well_radius: f64,
        skin: f64,
        injector: bool,
    ) -> PyResult<()> {
        to_py(self.inner.add_well(i, j, k, bhp, well_radius, skin, injector).map(|_| ()))
    }

    /// Add a well belonging to a named physical well.
    ///
    /// Not a convenience over `add_well`: the id chooses how `build_well_topology` groups
    /// completions into physical wells — `ExplicitId` when present, a fingerprint of
    /// injector/i/j/bhp/radius/skin when absent. Two clients that configure the same case through
    /// different ones are not running the same case, which is why the parity matrix uses this.
    #[pyo3(signature = (i, j, k, bhp, well_radius, skin, injector, physical_well_id))]
    #[allow(clippy::too_many_arguments)]
    fn add_well_with_id(
        &mut self,
        i: usize,
        j: usize,
        k: usize,
        bhp: f64,
        well_radius: f64,
        skin: f64,
        injector: bool,
        physical_well_id: String,
    ) -> PyResult<()> {
        to_py(
            self.inner
                .add_well_with_id(i, j, k, bhp, well_radius, skin, injector, physical_well_id),
        )
    }

    // ---- running -------------------------------------------------------------------------

    fn step(&mut self, target_dt_days: f64) {
        self.inner.step(target_dt_days);
    }

    /// Step with the FIM iteration trace captured, and return it.
    ///
    /// The browser has had this as `stepWithDiagnostics` all along. Binding it here is what makes
    /// a cross-target comparison possible at all: the two clients can now be asked not just
    /// whether they agree but *at which iteration they stop agreeing*.
    fn step_with_diagnostics(&mut self, target_dt_days: f64) -> String {
        self.inner.step_with_diagnostics(target_dt_days)
    }

    fn fim_trace(&self) -> String {
        self.inner.get_fim_trace()
    }

    // ---- reading -------------------------------------------------------------------------

    fn pressures(&self) -> Vec<f64> {
        self.inner.get_pressures()
    }

    fn sat_water(&self) -> Vec<f64> {
        self.inner.get_sat_water()
    }

    fn sat_oil(&self) -> Vec<f64> {
        self.inner.get_sat_oil()
    }

    fn cell_count(&self) -> usize {
        self.inner.get_pressures().len()
    }

    fn dimensions(&self) -> (usize, usize, usize) {
        let [nx, ny, nz] = self.inner.dimensions();
        (nx, ny, nz)
    }

    // ---- reporting -----------------------------------------------------------------------
    //
    // These reach the engine through `simulator::api`, the same accessors the browser shim
    // serializes. Until Phase 1 of the payload-boundary design they were `JsValue`-only, which
    // is why this module could previously run a case but not say what came out of it.

    fn rate_history<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        pythonize(py, self.inner.rate_history()).map_err(|e| PyValueError::new_err(e.to_string()))
    }

    fn rate_history_since<'py>(&self, py: Python<'py>, start_index: usize) -> PyResult<Bound<'py, PyAny>> {
        pythonize(py, self.inner.rate_history_since(start_index))
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    fn latest_rate_point<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        match self.inner.latest_rate_point() {
            Some(point) => Ok(Some(
                pythonize(py, point).map_err(|e| PyValueError::new_err(e.to_string()))?,
            )),
            None => Ok(None),
        }
    }

    fn wells<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        pythonize(py, self.inner.wells()).map_err(|e| PyValueError::new_err(e.to_string()))
    }

    // ---- configuration payloads ------------------------------------------------------------
    //
    // Phase 2 of the payload-boundary design moved these rules into `simulator::api`, so the
    // validation a caller hits here is the same validation the browser hits -- one implementation,
    // not two that must be kept in agreement.

    fn set_pvt_table(&mut self, rows: &Bound<'_, PyAny>) -> PyResult<()> {
        let rows = depythonize(rows).map_err(|e| PyValueError::new_err(e.to_string()))?;
        to_py(self.inner.apply_pvt_table(rows))
    }

    fn set_three_phase_scal_tables(&mut self, tables: &Bound<'_, PyAny>) -> PyResult<()> {
        let tables = depythonize(tables).map_err(|e| PyValueError::new_err(e.to_string()))?;
        to_py(self.inner.apply_three_phase_scal_tables(tables))
    }

    #[pyo3(signature = (config=None))]
    fn set_sweep_config(&mut self, config: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        let config = match config {
            Some(c) if !c.is_none() => {
                Some(depythonize(c).map_err(|e| PyValueError::new_err(e.to_string()))?)
            }
            _ => None,
        };
        to_py(self.inner.apply_sweep_config(config))
    }

    /// Restore a captured run state: grid, wells and rate history at a given time.
    fn load_state(
        &mut self,
        time_days: f64,
        grid: &Bound<'_, PyAny>,
        wells: &Bound<'_, PyAny>,
        rate_history: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let grid = depythonize(grid).map_err(|e| PyValueError::new_err(e.to_string()))?;
        let wells = depythonize(wells).map_err(|e| PyValueError::new_err(e.to_string()))?;
        let history = depythonize(rate_history).map_err(|e| PyValueError::new_err(e.to_string()))?;
        to_py(self.inner.apply_state(time_days, grid, wells, history))
    }

    /// The grid state, in the same shape `load_state` accepts.
    ///
    /// Phase 3 moved the assembly into `simulator::api::grid_state`. This shim used to build the
    /// struct field by field, which meant it -- not the engine -- decided what a grid state
    /// contains, and a sixth field would have had to be remembered here.
    fn grid_state<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        pythonize(py, &self.inner.grid_state()).map_err(|e| PyValueError::new_err(e.to_string()))
    }

    fn time_days(&self) -> f64 {
        self.inner.get_time()
    }

    fn last_fim_step_stats<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        match self.inner.last_fim_step_stats() {
            Some(stats) => Ok(Some(
                pythonize(py, stats).map_err(|e| PyValueError::new_err(e.to_string()))?,
            )),
            None => Ok(None),
        }
    }
}

#[pymodule]
fn ressim(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PySimulator>()?;
    m.add("__doc__", "Native bindings to the ResSim reservoir simulator.")?;
    Ok(())
}
