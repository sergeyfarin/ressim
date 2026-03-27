use nalgebra::DVector;
use sprs::CsMat;
use std::f64;

/// Result from the linear solver including convergence info.
pub(crate) struct LinearSolveResult {
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

fn apply_jacobi_preconditioner(m_inv_diag: &DVector<f64>, rhs: &DVector<f64>) -> DVector<f64> {
    let mut out = DVector::<f64>::zeros(rhs.len());
    for i in 0..rhs.len() {
        out[i] = rhs[i] * m_inv_diag[i];
    }
    out
}

// BiCGSTAB with Jacobi preconditioning. Unlike PCG, this remains valid for the
// mildly non-symmetric pressure matrices produced by upwinded multiphase flow.
pub(crate) fn solve_bicgstab_with_guess(
    a: &CsMat<f64>,
    b: &DVector<f64>,
    m_inv_diag: &DVector<f64>,
    x0: &DVector<f64>,
    tolerance: f64,
    max_iter: usize,
) -> LinearSolveResult {
    let mut x = x0.clone();
    let mut r = b - &cs_mat_mul_vec(a, &x);
    let r0_norm = r.norm();
    if r0_norm == 0.0 {
        return LinearSolveResult {
            solution: x,
            converged: true,
            iterations: 0,
        };
    }

    let r_hat = r.clone();
    let mut rho_prev = 1.0;
    let mut alpha = 1.0;
    let mut omega = 1.0;
    let mut v = DVector::<f64>::zeros(b.len());
    let mut p = DVector::<f64>::zeros(b.len());
    let mut converged = false;
    let mut iter_count = 0;
    for it in 0..max_iter {
        iter_count = it + 1;
        if r.norm() / r0_norm < tolerance {
            converged = true;
            break;
        }

        let rho = r_hat.dot(&r);
        if !rho.is_finite() || rho.abs() < f64::EPSILON {
            converged = false;
            break;
        }

        let beta = if it == 0 {
            0.0
        } else {
            (rho / rho_prev) * (alpha / omega)
        };
        p = &r + beta * (&p - omega * &v);

        let p_hat = apply_jacobi_preconditioner(m_inv_diag, &p);
        v = cs_mat_mul_vec(a, &p_hat);
        let r_hat_dot_v = r_hat.dot(&v);
        if !r_hat_dot_v.is_finite() || r_hat_dot_v.abs() < f64::EPSILON {
            converged = false;
            break;
        }
        alpha = rho / r_hat_dot_v;
        let s = &r - alpha * &v;
        if s.norm() / r0_norm < tolerance {
            x += alpha * p_hat;
            converged = true;
            break;
        }

        let s_hat = apply_jacobi_preconditioner(m_inv_diag, &s);
        let t = cs_mat_mul_vec(a, &s_hat);
        let t_dot_t = t.dot(&t);
        if !t_dot_t.is_finite() || t_dot_t.abs() < f64::EPSILON {
            converged = false;
            break;
        }
        omega = t.dot(&s) / t_dot_t;
        if !omega.is_finite() || omega.abs() < f64::EPSILON {
            converged = false;
            break;
        }

        x += alpha * p_hat + omega * &s_hat;
        r = s - omega * t;
        rho_prev = rho;
    }

    LinearSolveResult {
        solution: x,
        converged,
        iterations: iter_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sprs::TriMatI;

    #[test]
    fn bicgstab_solves_small_nonsymmetric_system() {
        let mut tri = TriMatI::<f64, usize>::new((2, 2));
        tri.add_triplet(0, 0, 4.0);
        tri.add_triplet(0, 1, 1.0);
        tri.add_triplet(1, 0, 2.0);
        tri.add_triplet(1, 1, 3.0);
        let a = tri.to_csr();

        let b = DVector::from_vec(vec![1.0, 1.0]);
        let x0 = DVector::from_vec(vec![0.0, 0.0]);
        let m_inv_diag = DVector::from_vec(vec![0.25, 1.0 / 3.0]);

        let result = solve_bicgstab_with_guess(&a, &b, &m_inv_diag, &x0, 1e-10, 100);
        assert!(
            result.converged,
            "BiCGSTAB should converge for a small nonsymmetric system"
        );
        assert!((result.solution[0] - 0.2).abs() < 1e-8);
        assert!((result.solution[1] - 0.2).abs() < 1e-8);
    }
}
