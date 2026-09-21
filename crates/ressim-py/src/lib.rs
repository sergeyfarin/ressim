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
//! **Scope is set by what the engine exposes natively, not by what would be nice.** S4 left 12 of
//! the engine's 69 API functions behind the `wasm` feature because they speak `JsValue` — among
//! them the rate history and the packed grid state. So this module can configure a case, run it,
//! and read pressures and saturations, and it deliberately stops there. Anything rate-based needs
//! a serde-native counterpart in the engine first; binding around the gap by reaching into
//! private state would put physics in the binding layer, which is the one thing this crate must
//! not do.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
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

    // ---- running -------------------------------------------------------------------------

    fn step(&mut self, target_dt_days: f64) {
        self.inner.step(target_dt_days);
    }

    // ---- reading -------------------------------------------------------------------------
    //
    // Pressures and saturations only. See the module docs: rates and packed grid state are
    // behind the engine's `wasm` feature and have no native form yet.

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
}

#[pymodule]
fn ressim(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PySimulator>()?;
    m.add("__doc__", "Native bindings to the ResSim reservoir simulator.")?;
    Ok(())
}
