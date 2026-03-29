use std::f64;

use nalgebra::DVector;

use super::{LinearSolveParams, LinearSolveResult, apply_jacobi_preconditioner, cs_mat_mul_vec};

// BiCGSTAB with Jacobi preconditioning. Unlike PCG, this remains valid for the
// mildly non-symmetric pressure matrices produced by upwinded multiphase flow.
pub(super) fn solve(params: &LinearSolveParams<'_>) -> LinearSolveResult {
    let mut x = params.initial_guess.clone();
    let mut r = params.rhs - &cs_mat_mul_vec(params.matrix, &x);
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
    let mut v = DVector::<f64>::zeros(params.rhs.len());
    let mut p = DVector::<f64>::zeros(params.rhs.len());
    let mut converged = false;
    let mut iter_count = 0;
    for it in 0..params.max_iterations {
        iter_count = it + 1;
        if r.norm() / r0_norm < params.tolerance {
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

        let p_hat = apply_jacobi_preconditioner(params.preconditioner_inv_diag, &p);
        v = cs_mat_mul_vec(params.matrix, &p_hat);
        let r_hat_dot_v = r_hat.dot(&v);
        if !r_hat_dot_v.is_finite() || r_hat_dot_v.abs() < f64::EPSILON {
            converged = false;
            break;
        }
        alpha = rho / r_hat_dot_v;
        let s = &r - alpha * &v;
        if s.norm() / r0_norm < params.tolerance {
            x += alpha * p_hat;
            converged = true;
            break;
        }

        let s_hat = apply_jacobi_preconditioner(params.preconditioner_inv_diag, &s);
        let t = cs_mat_mul_vec(params.matrix, &s_hat);
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
    use nalgebra::DVector;
    use sprs::TriMatI;

    use super::*;

    #[test]
    fn bicgstab_solves_small_nonsymmetric_system() {
        let mut tri = TriMatI::<f64, usize>::new((2, 2));
        tri.add_triplet(0, 0, 4.0);
        tri.add_triplet(0, 1, 1.0);
        tri.add_triplet(1, 0, 2.0);
        tri.add_triplet(1, 1, 3.0);
        let matrix = tri.to_csr();

        let rhs = DVector::from_vec(vec![1.0, 1.0]);
        let initial_guess = DVector::from_vec(vec![0.0, 0.0]);
        let preconditioner_inv_diag = DVector::from_vec(vec![0.25, 1.0 / 3.0]);

        let result = solve(&LinearSolveParams {
            matrix: &matrix,
            rhs: &rhs,
            preconditioner_inv_diag: &preconditioner_inv_diag,
            initial_guess: &initial_guess,
            tolerance: 1e-10,
            max_iterations: 100,
        });
        assert!(
            result.converged,
            "BiCGSTAB should converge for a small nonsymmetric system"
        );
        assert!((result.solution[0] - 0.2).abs() < 1e-8);
        assert!((result.solution[1] - 0.2).abs() < 1e-8);
    }
}
