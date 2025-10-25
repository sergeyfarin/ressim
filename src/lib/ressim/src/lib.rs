
// File: `wasm/simulator/src/lib.rs`
use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};
use nalgebra::DVector;
use sprs::{CsMat, TriMatI};
use std::f64;

// Utility to log panics to the browser console
#[wasm_bindgen(start)]
pub fn set_panic_hook() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

// --- Data Structures ---
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GridCell {
    pub porosity: f64,
    pub perm_x: f64,
    pub perm_y: f64,
    pub perm_z: f64,
    pub pressure: f64,
    pub sat_water: f64,
    pub sat_oil: f64,
}

impl GridCell {
    fn default_cell() -> Self {
        GridCell {
            porosity: 0.2,
            perm_x: 100.0,
            perm_y: 100.0,
            perm_z: 10.0,
            pressure: 3000.0,
            sat_water: 0.3,
            sat_oil: 0.7,
        }
    }

    // returns pore volume in cubic meters (assuming dx,dy,dz in meters)
    pub fn pore_volume_m3(&self, dx: f64, dy: f64, dz: f64) -> f64 {
        dx * dy * dz * self.porosity
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Well {
    pub i: usize, pub j: usize, pub k: usize,
    pub bhp: f64,         // bottom-hole pressure (control)
    pub productivity_index: f64, // PI (units consistent with transmissibility)
    pub injector: bool,   // true if injector (positive flow into reservoir when bhp > res)
}

// --- Fluid / Rock ---
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FluidProperties {
    pub mu_o: f64,
    pub mu_w: f64,
    pub c_o: f64,
    pub c_w: f64,
}

impl FluidProperties {
    fn default_pvt() -> Self {
        Self {
            mu_o: 1.0,
            mu_w: 0.5,
            c_o: 1e-5,
            c_w: 3e-6,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RockFluidProps {
    pub s_wc: f64, // connate water
    pub s_or: f64, // residual oil
    pub n_w: f64,
    pub n_o: f64,
}

impl RockFluidProps {
    fn default_scal() -> Self {
        Self { s_wc: 0.2, s_or: 0.2, n_w: 2.0, n_o: 2.0 }
    }

    // Simple Corey-type krw and kro
    pub fn k_rw(&self, s_w: f64) -> f64 {
        let s_eff = ((s_w - self.s_wc) / (1.0 - self.s_wc - self.s_or)).clamp(0.0, 1.0);
        s_eff.powf(self.n_w)
    }
    pub fn k_ro(&self, s_w: f64) -> f64 {
        let s_eff = ((1.0 - s_w - self.s_or) / (1.0 - self.s_wc - self.s_or)).clamp(0.0, 1.0);
        s_eff.powf(self.n_o)
    }
}

// --- Simulator ---
#[wasm_bindgen]
pub struct ReservoirSimulator {
    nx: usize, ny: usize, nz: usize,
    dx: f64, dy: f64, dz: f64,
    grid_cells: Vec<GridCell>,
    wells: Vec<Well>,
    time_days: f64,
    pvt: FluidProperties,
    scal: RockFluidProps,
}

#[wasm_bindgen]
impl ReservoirSimulator {
    #[wasm_bindgen(constructor)]
    pub fn new(nx: usize, ny: usize, nz: usize) -> Self {
        let n = nx * ny * nz;
        let grid_cells = vec![GridCell::default_cell(); n];
        ReservoirSimulator {
            nx, ny, nz,
            dx: 100.0, dy: 100.0, dz: 20.0, // meters
            grid_cells,
            wells: Vec::new(),
            time_days: 0.0,
            pvt: FluidProperties::default_pvt(),
            scal: RockFluidProps::default_scal(),
        }
    }

    fn idx(&self, i: usize, j: usize, k: usize) -> usize {
        (k * self.nx * self.ny) + (j * self.nx) + i
    }

    // add well controlled by BHP and PI
    pub fn add_well(&mut self, i: usize, j: usize, k: usize, bhp: f64, pi: f64, injector: bool) {
        self.wells.push(Well { i, j, k, bhp, productivity_index: pi, injector });
    }

    // total mobility lambda_t = krw/mu_w + kro/mu_o
    fn total_mobility(&self, cell: &GridCell) -> f64 {
        let krw = self.scal.k_rw(cell.sat_water);
        let kro = self.scal.k_ro(cell.sat_water);
        krw / self.pvt.mu_w + kro / self.pvt.mu_o
    }

    // fractional flow of water fw = lambda_w / lambda_t
    fn frac_flow_water(&self, cell: &GridCell) -> f64 {
        let krw = self.scal.k_rw(cell.sat_water);
        let lam_w = krw / self.pvt.mu_w;
        let lam_t = lam_w + (self.scal.k_ro(cell.sat_water) / self.pvt.mu_o);
        if lam_t <= 0.0 { 0.0 } else { (lam_w / lam_t).clamp(0.0, 1.0) }
    }

    // transmissibility between two neighboring cells (simplified)
    fn transmissibility(&self, c1: &GridCell, c2: &GridCell, dim: char) -> f64 {
        let (perm1, perm2, dist, area) = match dim {
            'x' => (c1.perm_x, c2.perm_x, self.dx, self.dy * self.dz),
            'y' => (c1.perm_y, c2.perm_y, self.dy, self.dx * self.dz),
            'z' => (c1.perm_z, c2.perm_z, self.dz, self.dx * self.dy),
            _ => (0.0, 0.0, 1.0, 1.0),
        };
        // harmonic mean
        let k_h = if perm1 + perm2 == 0.0 { 0.0 } else { 2.0 * perm1 * perm2 / (perm1 + perm2) };
        if k_h == 0.0 { return 0.0; }
        let mob_up = (self.total_mobility(c1) + self.total_mobility(c2)) / 2.0;
        // Keep earlier conversion factor for rough scaling (units are heuristic)
        0.001127 * k_h * area / dist * mob_up
    }

    // main IMPES step: delta_t in days
    pub fn step(&mut self, delta_t_days: f64) {
        let n_cells = self.nx * self.ny * self.nz;
        if n_cells == 0 { return; }
        let dt_days = delta_t_days.max(1e-12);
        let dt_seconds = dt_days * 86400.0;
        // total compressibility (approx as sum of phase compressibilities)
        let c_t = self.pvt.c_o + self.pvt.c_w;

        // Build triplet lists for A matrix and RHS b
        let mut rows: Vec<usize> = Vec::with_capacity(n_cells * 7);
        let mut cols: Vec<usize> = Vec::with_capacity(n_cells * 7);
        let mut vals: Vec<f64> = Vec::with_capacity(n_cells * 7);

        let mut b_rhs = DVector::<f64>::zeros(n_cells);
        let mut diag_inv = DVector::<f64>::zeros(n_cells);

        // Assemble accumulation + transmissibility + well-implicit terms
        for k in 0..self.nz {
            for j in 0..self.ny {
                for i in 0..self.nx {
                    let id = self.idx(i, j, k);
                    let cell = &self.grid_cells[id];
                    let vp_m3 = cell.pore_volume_m3(self.dx, self.dy, self.dz);
                    // convert pore volume to barrels for saturation update (1 m3 = 6.28981 bbl)
                    let vp_bbl = vp_m3 * 6.28981;
                    // accumulation term: Vp * c_t / dt
                    let accum = (vp_bbl * c_t) / dt_days.max(1e-12); // use days to be consistent with transmissibility scale
                    let mut diag = accum;
                    // move old pressure term to RHS
                    b_rhs[id] += accum * cell.pressure;

                    // neighbors
                    let mut neighbors: Vec<(usize, char)> = Vec::new();
                    if i > 0 { neighbors.push((self.idx(i-1,j,k), 'x')); }
                    if i < self.nx-1 { neighbors.push((self.idx(i+1,j,k), 'x')); }
                    if j > 0 { neighbors.push((self.idx(i,j-1,k), 'y')); }
                    if j < self.ny-1 { neighbors.push((self.idx(i,j+1,k), 'y')); }
                    if k > 0 { neighbors.push((self.idx(i,j,k-1), 'z')); }
                    if k < self.nz-1 { neighbors.push((self.idx(i,j,k+1), 'z')); }

                    for (n_id, dim) in neighbors.iter() {
                        let t = self.transmissibility(cell, &self.grid_cells[*n_id], *dim);
                        diag += t;
                        rows.push(id); cols.push(*n_id); vals.push(-t);
                    }

                    // well implicit term: add PI to diagonal and PI*BHP to RHS
                    for w in &self.wells {
                        if w.i == i && w.j == j && w.k == k {
                            diag += w.productivity_index;
                            b_rhs[id] += w.productivity_index * w.bhp;
                        }
                    }

                    // push diagonal
                    rows.push(id); cols.push(id); vals.push(diag);
                    // safe inverse for Jacobi preconditioner
                    diag_inv[id] = if diag.abs() > f64::EPSILON { 1.0 / diag } else { 1.0 };
                }
            }
        }

        // build TriMat and convert to CSR
        let mut tri = TriMatI::<f64, usize>::new((n_cells, n_cells));
        for idx in 0..vals.len() {
            tri.add_triplet(rows[idx], cols[idx], vals[idx]);
        }
        let a_mat: CsMat<f64> = tri.to_csr();

        // Solve A x = b with PCG, initial guess = current pressures
        let mut x0 = DVector::<f64>::zeros(n_cells);
        for i in 0..n_cells { x0[i] = self.grid_cells[i].pressure; }
        let p_new = solve_pcg_with_guess(&a_mat, &b_rhs, &diag_inv, &x0, 1e-7, 1000);

        // Compute fluxes and explicit saturation update (upwind fractional flow)
        // We'll compute net water volume change per cell in barrels over dt_days.
        let mut net_water_bbl = vec![0.0f64; n_cells];

        // interface fluxes: for each neighbor pair compute once and distribute
        for k in 0..self.nz {
            for j in 0..self.ny {
                for i in 0..self.nx {
                    let id = self.idx(i,j,k);
                    let p_i = p_new[id];
                    // neighbors in + direction to avoid duplicate pairs
                    let mut check = Vec::new();
                    if i < self.nx - 1 { check.push((self.idx(i+1,j,k), 'x')); }
                    if j < self.ny - 1 { check.push((self.idx(i,j+1,k), 'y')); }
                    if k < self.nz - 1 { check.push((self.idx(i,j,k+1), 'z')); }

                    for (nid, dim) in check {
                        let p_j = p_new[nid];
                        let t = self.transmissibility(&self.grid_cells[id], &self.grid_cells[nid], dim);
                        // flux (bbl/day): positive means from id -> nid
                        let flux = t * (p_i - p_j);
                        // upwind fractional flow for water
                        let f_up = if flux >= 0.0 {
                            self.frac_flow_water(&self.grid_cells[id])
                        } else {
                            self.frac_flow_water(&self.grid_cells[nid])
                        };
                        let water_flux = flux * f_up; // bbl/day water across interface
                        // distribute: for id, outgoing is (+) flux so net change is -water_flux*dt; for nid opposite
                        net_water_bbl[id] -= water_flux * dt_days;
                        net_water_bbl[nid] += water_flux * dt_days;
                    }
                }
            }
        }

        // Add well explicit contributions (actual well rate using solved pressure)
        for w in &self.wells {
            let id = self.idx(w.i, w.j, w.k);
            // actual well rate q = PI * (p_block - bhp) ; positive -> production (outflow from reservoir)
            let q = w.productivity_index * (p_new[id] - w.bhp);
            // split by fractional flow at block
            let fw = self.frac_flow_water(&self.grid_cells[id]);
            let water_q = q * fw; // bbl/day
            // For cell, production (q>0) reduces reservoir volume: subtract q*dt; injection (q<0) adds volume
            net_water_bbl[id] -= water_q * dt_days;
        }

        // update saturations
        for idx in 0..n_cells {
            let vp_m3 = self.grid_cells[idx].pore_volume_m3(self.dx, self.dy, self.dz);
            let vp_bbl = vp_m3 * 6.28981;
            if vp_bbl <= 0.0 { continue; }

            let delta_sw = net_water_bbl[idx] / vp_bbl;
            let mut sw_new = (self.grid_cells[idx].sat_water + delta_sw).clamp(0.0, 1.0);
            // ensure oil = 1 - sw (no gas)
            let so_new = (1.0 - sw_new).clamp(0.0, 1.0);

            self.grid_cells[idx].sat_water = sw_new;
            self.grid_cells[idx].sat_oil = so_new;
            // update pressure to solved pressure
            self.grid_cells[idx].pressure = p_new[idx];
        }

        self.time_days += dt_days;
    }

    pub fn get_time(&self) -> f64 { self.time_days }

    #[wasm_bindgen(js_name = getGridState)]
    pub fn get_grid_state(&self) -> JsValue { serde_wasm_bindgen::to_value(&self.grid_cells).unwrap() }

    #[wasm_bindgen(js_name = getWellState)]
    pub fn get_well_state(&self) -> JsValue { serde_wasm_bindgen::to_value(&self.wells).unwrap() }

    #[wasm_bindgen(js_name = getDimensions)]
    pub fn get_dimensions(&self) -> JsValue { serde_wasm_bindgen::to_value(&[self.nx, self.ny, self.nz]).unwrap() }
}

// --- Helper: sparse matrix-vector multiply ---
fn cs_mat_mul_vec(a: &CsMat<f64>, x: &DVector<f64>) -> DVector<f64> {
    let n = a.rows();
    let mut y = DVector::<f64>::zeros(n);
    for (row, vec) in a.outer_iterator().enumerate() {
        let mut sum = 0.0;
        for (&col, &val) in vec.indices().iter().zip(vec.data().iter()) {
            sum += val * x[col];
        }
        y[row] = sum;
    }
    y
}

// PCG solver with initial guess
fn solve_pcg_with_guess(
    a: &CsMat<f64>,
    b: &DVector<f64>,
    m_inv_diag: &DVector<f64>,
    x0: &DVector<f64>,
    tolerance: f64,
    max_iter: usize,
) -> DVector<f64> {
    let n = b.len();
    let mut x = x0.clone();
    let mut r = b - &cs_mat_mul_vec(a, &x);
    let mut z = DVector::<f64>::zeros(n);
    for i in 0..n { z[i] = r[i] * m_inv_diag[i]; }
    let mut p = z.clone();
    let mut r_dot_z = r.dot(&z);
    let r0_norm = r.norm();
    if r0_norm == 0.0 { return x; }

    for _ in 0..max_iter {
        if r.norm() / r0_norm < tolerance { break; }
        let q = cs_mat_mul_vec(a, &p);
        let p_dot_q = p.dot(&q);
        if p_dot_q.abs() < f64::EPSILON { break; }
        let alpha = r_dot_z / p_dot_q;
        x += alpha * p.clone();
        let r_new = r - alpha * q;
        let mut z_new = DVector::<f64>::zeros(n);
        for i in 0..n { z_new[i] = r_new[i] * m_inv_diag[i]; }
        let r_new_dot_z_new = r_new.dot(&z_new);
        let beta = if r_dot_z.abs() < f64::EPSILON { 0.0 } else { r_new_dot_z_new / r_dot_z };
        p = z_new.clone() + beta * p;
        r = r_new;
        r_dot_z = r_new_dot_z_new;
    }
    x
}