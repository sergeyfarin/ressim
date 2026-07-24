use std::f64;

use nalgebra::DVector;

use sprs::CsMat;

use super::{LinearSolveParams, LinearSolveResult, apply_jacobi_preconditioner, cs_mat_mul_vec};

#[derive(Clone, Debug)]
struct Ilu0Factors {
    l_rows: Vec<Vec<(usize, f64)>>,
    u_diag: Vec<f64>,
    u_rows: Vec<Vec<(usize, f64)>>,
}

impl Ilu0Factors {
    fn apply(&self, rhs: &DVector<f64>) -> DVector<f64> {
        let mut y = DVector::zeros(rhs.len());
        for row in 0..rhs.len() {
            let mut sum = rhs[row];
            for &(col, value) in &self.l_rows[row] {
                sum -= value * y[col];
            }
            y[row] = sum;
        }

        let mut x = DVector::zeros(rhs.len());
        for row in (0..rhs.len()).rev() {
            let mut sum = y[row];
            for &(col, value) in &self.u_rows[row] {
                sum -= value * x[col];
            }
            x[row] = sum / self.u_diag[row];
        }
        x
    }
}

fn row_entry(row: &[(usize, f64)], column: usize) -> f64 {
    row.binary_search_by_key(&column, |&(index, _)| index)
        .map(|index| row[index].1)
        .unwrap_or(0.0)
}

/// Scalar ILU(0) for the IMPES pressure matrix.  The matrix is an ordered
/// Cartesian stencil, so retaining its original sparsity captures the nearest-
/// neighbor coupling without the superlinear fill cost of a direct LU.
fn factorize_ilu0(matrix: &CsMat<f64>) -> Option<Ilu0Factors> {
    if matrix.rows() == 0 || matrix.rows() != matrix.cols() {
        return None;
    }

    let n = matrix.rows();
    let mut l_rows: Vec<Vec<(usize, f64)>> = vec![Vec::new(); n];
    let mut u_diag = vec![0.0_f64; n];
    let mut u_rows: Vec<Vec<(usize, f64)>> = vec![Vec::new(); n];

    for row_index in 0..n {
        let row = matrix.outer_view(row_index)?;
        let mut diagonal = 0.0;

        for (column, value) in row.iter().filter(|(column, _)| *column < row_index) {
            let mut updated = *value;
            for &(k, l_ik) in &l_rows[row_index] {
                if k >= column {
                    break;
                }
                updated -= l_ik * row_entry(&u_rows[k], column);
            }
            let pivot = u_diag[column];
            if !pivot.is_finite() || pivot.abs() <= f64::EPSILON {
                return None;
            }
            l_rows[row_index].push((column, updated / pivot));
        }

        for (column, value) in row.iter().filter(|(column, _)| *column >= row_index) {
            let mut updated = *value;
            for &(k, l_ik) in &l_rows[row_index] {
                updated -= l_ik * row_entry(&u_rows[k], column);
            }
            if column == row_index {
                diagonal = updated;
            } else {
                u_rows[row_index].push((column, updated));
            }
        }

        if !diagonal.is_finite() || diagonal.abs() <= f64::EPSILON {
            return None;
        }
        u_diag[row_index] = diagonal;
    }

    Some(Ilu0Factors {
        l_rows,
        u_diag,
        u_rows,
    })
}

// BiCGSTAB with ILU(0) preconditioning and a Jacobi fallback when factorization
// is unavailable. Unlike PCG, this remains valid for the mildly non-symmetric
// pressure matrices produced by upwinded multiphase flow.
pub(super) fn solve(params: &LinearSolveParams<'_>) -> LinearSolveResult {
    let ilu0 = factorize_ilu0(params.matrix);
    let apply_preconditioner = |rhs: &DVector<f64>| {
        ilu0.as_ref().map_or_else(
            || apply_jacobi_preconditioner(params.preconditioner_inv_diag, rhs),
            |factors| factors.apply(rhs),
        )
    };

    let mut x = params.initial_guess.clone();
    let mut r = params.rhs - &cs_mat_mul_vec(params.matrix, &x);
    let rhs_norm = params.rhs.norm().max(f64::EPSILON);
    if r.norm() / rhs_norm <= params.tolerance {
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
        if r.norm() / rhs_norm <= params.tolerance {
            converged = true;
            break;
        }
        iter_count = it + 1;

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

        let p_hat = apply_preconditioner(&p);
        v = cs_mat_mul_vec(params.matrix, &p_hat);
        let r_hat_dot_v = r_hat.dot(&v);
        if !r_hat_dot_v.is_finite() || r_hat_dot_v.abs() < f64::EPSILON {
            converged = false;
            break;
        }
        alpha = rho / r_hat_dot_v;
        let s = &r - alpha * &v;
        if s.norm() / rhs_norm <= params.tolerance {
            x += alpha * p_hat;
            converged = true;
            break;
        }

        let s_hat = apply_preconditioner(&s);
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

    #[test]
    fn bicgstab_reports_completed_iterations_at_loop_boundary_convergence() {
        let mut tri = TriMatI::<f64, usize>::new((2, 2));
        tri.add_triplet(0, 0, 4.0);
        tri.add_triplet(0, 1, 1.0);
        tri.add_triplet(1, 0, 2.0);
        tri.add_triplet(1, 1, 3.0);
        let matrix = tri.to_csr();

        let rhs = DVector::from_vec(vec![1.0, 1.0]);
        let initial_guess = DVector::from_vec(vec![0.0, 0.0]);
        let preconditioner_inv_diag = DVector::from_vec(vec![0.25, 1.0 / 3.0]);

        // The first full BiCGSTAB correction reduces the residual below 1e-2,
        // but the intermediate s residual remains above it. Convergence is
        // therefore recognized at the next loop boundary, after one completed
        // iteration rather than two.
        let result = solve(&LinearSolveParams {
            matrix: &matrix,
            rhs: &rhs,
            preconditioner_inv_diag: &preconditioner_inv_diag,
            initial_guess: &initial_guess,
            tolerance: 1e-2,
            max_iterations: 100,
        });

        assert!(result.converged);
        assert_eq!(result.iterations, 1);
        let relative_residual =
            (&rhs - cs_mat_mul_vec(&matrix, &result.solution)).norm() / rhs.norm();
        assert!(relative_residual < 1e-2);
    }
}
