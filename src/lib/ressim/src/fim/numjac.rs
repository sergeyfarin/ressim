//! Test-only numerical-Jacobian harness.
//!
//! Central-difference reference Jacobian for validating the automatic-
//! differentiation assembly. Every AD phase is gated on its analytic block
//! matching this reference to a tight tolerance on a small canonical case, so
//! Jacobian errors are caught per-primitive instead of after the whole solver
//! is wired.

/// Dense Jacobian of `residual` at `x` by central differences.
///
/// `residual` maps an `n`-vector of unknowns to an `m`-vector of equations.
/// Returns row-major `m x n` entries: `jac[eq][unknown]`.
pub(crate) fn central_difference_jacobian<F>(x: &[f64], m: usize, residual: F) -> Vec<Vec<f64>>
where
    F: Fn(&[f64]) -> Vec<f64>,
{
    let n = x.len();
    let mut jac = vec![vec![0.0; n]; m];
    let mut probe = x.to_vec();

    for j in 0..n {
        // Scale the step to the magnitude of the variable for good conditioning.
        let h = 1e-6 * (1.0 + x[j].abs());

        probe[j] = x[j] + h;
        let f_plus = residual(&probe);
        probe[j] = x[j] - h;
        let f_minus = residual(&probe);
        probe[j] = x[j];

        let inv = 0.5 / h;
        for i in 0..m {
            jac[i][j] = (f_plus[i] - f_minus[i]) * inv;
        }
    }

    jac
}

/// Compare two dense Jacobians entrywise with a combined relative/absolute
/// tolerance. Panics with the worst mismatch if they disagree.
pub(crate) fn assert_jacobian_matches(
    analytic: &[Vec<f64>],
    numerical: &[Vec<f64>],
    rel_tol: f64,
    abs_tol: f64,
) {
    assert_eq!(
        analytic.len(),
        numerical.len(),
        "row count mismatch: {} vs {}",
        analytic.len(),
        numerical.len()
    );

    let mut worst_abs = 0.0_f64;
    let mut worst_at = (usize::MAX, usize::MAX);
    let mut worst_pair = (0.0, 0.0);

    for (i, (a_row, n_row)) in analytic.iter().zip(numerical.iter()).enumerate() {
        assert_eq!(a_row.len(), n_row.len(), "col count mismatch in row {i}");
        for (j, (a, n)) in a_row.iter().zip(n_row.iter()).enumerate() {
            let diff = (a - n).abs();
            let allowed = abs_tol + rel_tol * a.abs().max(n.abs());
            if diff > allowed && diff > worst_abs {
                worst_abs = diff;
                worst_at = (i, j);
                worst_pair = (*a, *n);
            }
        }
    }

    assert!(
        worst_at == (usize::MAX, usize::MAX),
        "Jacobian mismatch at [{}][{}]: analytic={:.6e} numerical={:.6e} (|diff|={:.3e})",
        worst_at.0,
        worst_at.1,
        worst_pair.0,
        worst_pair.1,
        worst_abs,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn central_difference_recovers_known_jacobian() {
        // f0 = x0^2 * x1
        // f1 = sin(x0) + x1^3
        // At x = (1.3, 0.7):
        //   df0/dx0 = 2 x0 x1,          df0/dx1 = x0^2
        //   df1/dx0 = cos(x0),          df1/dx1 = 3 x1^2
        let x = [1.3_f64, 0.7_f64];
        let residual = |v: &[f64]| vec![v[0] * v[0] * v[1], v[0].sin() + v[1].powi(3)];

        let numerical = central_difference_jacobian(&x, 2, residual);
        let analytic = vec![
            vec![2.0 * x[0] * x[1], x[0] * x[0]],
            vec![x[0].cos(), 3.0 * x[1] * x[1]],
        ];

        assert_jacobian_matches(&analytic, &numerical, 1e-6, 1e-9);
    }

    #[test]
    #[should_panic(expected = "Jacobian mismatch")]
    fn assert_detects_mismatch() {
        let analytic = vec![vec![1.0, 2.0]];
        let numerical = vec![vec![1.0, 5.0]];
        assert_jacobian_matches(&analytic, &numerical, 1e-9, 1e-12);
    }
}
