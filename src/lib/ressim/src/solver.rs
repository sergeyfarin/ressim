use nalgebra::DVector;
use sprs::CsMat;
use std::f64;

/// Result from PCG solver including convergence info
pub(crate) struct PcgResult {
    pub(crate) solution: DVector<f64>,
    pub(crate) converged: bool,
    pub(crate) iterations: usize,
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

// PCG solver with initial guess — returns solution, convergence flag, and iteration count
pub(crate) fn solve_pcg_with_guess(
    a: &CsMat<f64>,
    b: &DVector<f64>,
    m_inv_diag: &DVector<f64>,
    x0: &DVector<f64>,
    tolerance: f64,
    max_iter: usize,
) -> PcgResult {
    let n = b.len();
    let mut x = x0.clone();
    let mut r = b - &cs_mat_mul_vec(a, &x);
    let mut z = DVector::<f64>::zeros(n);
    for i in 0..n {
        z[i] = r[i] * m_inv_diag[i];
    }
    let mut p = z.clone();
    let mut r_dot_z = r.dot(&z);
    let r0_norm = r.norm();
    if r0_norm == 0.0 {
        return PcgResult {
            solution: x,
            converged: true,
            iterations: 0,
        };
    }

    let mut converged = false;
    let mut iter_count = 0;
    for it in 0..max_iter {
        iter_count = it + 1;
        if r.norm() / r0_norm < tolerance {
            converged = true;
            break;
        }
        let q = cs_mat_mul_vec(a, &p);
        let p_dot_q = p.dot(&q);
        if p_dot_q.abs() < f64::EPSILON {
            converged = false;
            break;
        }
        let alpha = r_dot_z / p_dot_q;
        x += alpha * p.clone();
        let r_new = r - alpha * q;
        let mut z_new = DVector::<f64>::zeros(n);
        for i in 0..n {
            z_new[i] = r_new[i] * m_inv_diag[i];
        }
        let r_new_dot_z_new = r_new.dot(&z_new);
        let beta = if r_dot_z.abs() < f64::EPSILON {
            0.0
        } else {
            r_new_dot_z_new / r_dot_z
        };
        p = z_new.clone() + beta * p;
        r = r_new;
        r_dot_z = r_new_dot_z_new;
    }
    PcgResult {
        solution: x,
        converged,
        iterations: iter_count,
    }
}
