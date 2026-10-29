//! Linear solves for the compositional Newton system (compositional plan, C10; #49).
//!
//! Three routes over one row-scaled system, all reporting the same observables:
//!
//! - [`LinearRoute::DenseLu`] — Gaussian elimination with partial pivoting on the densified
//!   matrix. The **oracle**: obviously correct, `O((cells*N)^3)` time and `(cells*N)^2` memory,
//!   so only for small grids. Every other route is measured against it.
//! - [`LinearRoute::SparseLu`] — faer's sparse LU on the block pattern. Agrees with the oracle to
//!   roundoff, but its fill-in grows faster than the grid: at SPE3 size it took the wasm heap to
//!   67 MB where CPR needed 8.
//! - [`LinearRoute::FgmresCpr`] — restarted flexible GMRES, right-preconditioned by a two-stage
//!   CPR designed for this formulation (below).
//!
//! [`LinearRoute::Auto`], the default, takes sparse LU up to [`DIRECT_ROW_THRESHOLD`] rows and CPR
//! above it. The measurements behind that are in `docs/COMPOSITIONAL_VALIDATION.md`'s C10
//! linear-route record.
//!
//! # The report contract
//!
//! Every route returns a [`LinearSolveReport`] carrying the RHS norm, the **true** full-system
//! residual norm `||b - A x||` recomputed from the returned correction (never a Krylov recurrence
//! estimate), their ratio and whether the correction is finite. That is the precondition
//! `docs/COMPOSITIONAL_VALIDATION.md` §6 item 4 set on this adapter: a direct-vs-iterative
//! comparison is only admissible when both reports carry the same observables measured the same
//! way.
//!
//! # CPR for the compositional system
//!
//! The plan's warning applies and is followed: nothing here is black-oil's quasi-IMPES. The
//! pressure restriction is derived from this formulation's own accumulation derivatives.
//!
//! **Restriction weights.** Primaries are `u = [p, z_0..z_(N-2)]` and each cell's accumulation
//! block is `D_iv = dn_i/du_v`. The weights `w` of a cell solve
//!
//! ```text
//! D^T w = e_p      i.e.   sum_i w_i dn_i/dp = 1,   sum_i w_i dn_i/dz_k = 0   for every k
//! ```
//!
//! so the weighted sum of the cell's component equations has an accumulation term that depends on
//! pressure alone — the true-IMPES pressure equation. For this mole-balance formulation it is the
//! volume balance: the fluid volume `V_f(p, n(p, z))` equals the pore volume `PV(p)` for every
//! `z`, so partial molar volumes `w_i = dV_f/dn_i` annihilate every `dz` column, and dividing by
//! `dPV/dp - dV_f/dp|_n` (pore volume times total compressibility, positive for any cell holding
//! fluid) normalizes the pressure column to one. `D` is therefore nonsingular whenever the cell is
//! compressible at all, and a singular one is a preconditioner build failure reported as such,
//! not patched. The weights are computed from the **row-scaled** block, so they belong to the
//! system the Krylov method actually sees.
//!
//! **Wells.** V1 wells have no unknowns; each completion's derivative sits in its cell's diagonal
//! block. The coarse operator is `C = W^T A P` over the full scaled matrix, so a well's pressure
//! derivative — the BHP feedback — enters the pressure equation exactly as it enters the cell's
//! equations, and its composition derivatives are left to the second stage. No separate well
//! policy is needed because there is no well equation to restrict.
//!
//! **Coarse operator and solve.** `P` injects a cell's pressure correction into its pressure column,
//! so `C` is `cells x cells` with the face graph's pattern. It is factorized once per linear solve
//! by sparse LU — at the sizes this engine runs, an exact coarse solve is cheaper than tuning an
//! approximate one, and it keeps the first stage deterministic.
//!
//! **Second stage.** Block-ILU(0) on the full scaled matrix, with `N x N` blocks: the composition
//! coupling the pressure stage leaves behind is local to a cell and its neighbours, which is what
//! ILU(0) on the block pattern captures.
//!
//! The two stages compose multiplicatively: `x1 = P C^-1 W^T r`, then `x = x1 + M^-1 (r - A x1)`.

use faer::Col;
use faer::linalg::solvers::Solve;
use faer::sparse::{SparseRowMat, Triplet};

use super::block_matrix::BlockSparseMatrix;

/// Systems up to this many rows solve by sparse LU under [`LinearRoute::Auto`]; larger ones by
/// FGMRES-CPR.
///
/// Measured (wasm, one solve, `0ab2af3`): at 882 rows CPR already took 12–14 ms against sparse
/// LU's 36–46 ms, and at 3,240 rows 14–15 ms against 251–256 ms. Below a few hundred rows both are a few milliseconds and the
/// direct solve is exact, which is worth more than the time. Where between the two the crossover
/// sits was not measured; 512 is the black-oil direct threshold, kept for symmetry rather than
/// because the compositional system was shown to share it.
pub const DIRECT_ROW_THRESHOLD: usize = 512;

/// Which linear solve a Newton iteration uses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LinearRoute {
    /// Sparse LU up to [`DIRECT_ROW_THRESHOLD`] rows, FGMRES-CPR above. A report never carries
    /// this; it names the route that ran.
    #[default]
    Auto,
    /// Dense LU with partial pivoting. The oracle; small grids only.
    DenseLu,
    /// Sparse LU on the block pattern.
    SparseLu,
    /// Restarted flexible GMRES with the compositional CPR preconditioner.
    FgmresCpr,
}

impl LinearRoute {
    /// Stable name for reports.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::DenseLu => "dense-lu",
            Self::SparseLu => "sparse-lu",
            Self::FgmresCpr => "fgmres-cpr",
        }
    }

    /// The route that runs on a system of `rows` rows.
    pub const fn resolve(self, rows: usize) -> Self {
        match self {
            Self::Auto if rows <= DIRECT_ROW_THRESHOLD => Self::SparseLu,
            Self::Auto => Self::FgmresCpr,
            other => other,
        }
    }
}

/// Settings for the linear solve. Only the iterative route reads the tolerances and budgets; a
/// direct route always reports its residual and is never rejected on it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LinearOptions {
    pub route: LinearRoute,
    /// Required `||b - A x|| / ||b||` on the scaled system.
    pub relative_tolerance: f64,
    /// Krylov vectors kept before a restart.
    pub restart: usize,
    /// Total Krylov iterations across restarts.
    pub max_iterations: usize,
}

impl Default for LinearOptions {
    fn default() -> Self {
        Self {
            route: default_route(),
            // Relative to the RHS, which is the Newton residual itself: a forcing term this small
            // keeps Newton's convergence quadratic until the residual is far below its 1e-8
            // acceptance, so the iterative route is never what holds Newton back. CPR reached it
            // in 7 iterations on both measured sizes.
            relative_tolerance: 1e-11,
            restart: 40,
            max_iterations: 200,
        }
    }
}

#[cfg(not(test))]
fn default_route() -> LinearRoute {
    LinearRoute::default()
}

/// Under test, `RESSIM_COMP_LINEAR_ROUTE=dense-lu|sparse-lu|fgmres-cpr` replaces the default for
/// every fixture that does not name a route, so the whole compositional gate can be replayed on
/// each route without a copy of each test.
#[cfg(test)]
fn default_route() -> LinearRoute {
    match std::env::var("RESSIM_COMP_LINEAR_ROUTE").as_deref() {
        Ok("dense-lu") => LinearRoute::DenseLu,
        Ok("sparse-lu") => LinearRoute::SparseLu,
        Ok("fgmres-cpr") => LinearRoute::FgmresCpr,
        _ => LinearRoute::default(),
    }
}

/// Why a linear solve produced no usable correction.
#[derive(Clone, Debug, PartialEq)]
pub enum LinearFailure {
    /// A direct factorization found the matrix singular. `min_pivot` is reported by the dense
    /// route only; faer's sparse LU does not expose its pivots.
    Singular { min_pivot: Option<f64> },
    /// The correction contains a NaN or infinity.
    NonFinite,
    /// The preconditioner could not be built, naming the cell whose block was singular.
    Preconditioner { cell: usize, stage: &'static str },
    /// The iterative route spent its budget above tolerance.
    NotConverged {
        iterations: usize,
        relative_residual: f64,
    },
}

impl core::fmt::Display for LinearFailure {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Singular { min_pivot: Some(p) } => {
                write!(f, "the Jacobian is singular: smallest pivot {p:e}")
            }
            Self::Singular { min_pivot: None } => write!(f, "the Jacobian is singular"),
            Self::NonFinite => write!(f, "the linear correction is not finite"),
            Self::Preconditioner { cell, stage } => {
                write!(f, "CPR {stage} block of cell {cell} is singular")
            }
            Self::NotConverged {
                iterations,
                relative_residual,
            } => write!(
                f,
                "the iterative solve did not converge in {iterations} iterations \
                 (relative residual {relative_residual:e})"
            ),
        }
    }
}

/// What one linear solve did, measured the same way on every route.
#[derive(Clone, Debug, PartialEq)]
pub struct LinearSolveReport {
    pub route: LinearRoute,
    /// `||b||_2` of the row-scaled system.
    pub rhs_norm: f64,
    /// `||b - A x||_2` recomputed from the returned correction on the full row-scaled system.
    pub residual_norm: f64,
    /// Krylov iterations; zero for a direct route.
    pub iterations: usize,
    /// Smallest pivot magnitude, dense route only.
    pub min_pivot: Option<f64>,
    /// Every entry of the correction is finite.
    pub finite: bool,
}

impl LinearSolveReport {
    /// `residual_norm / rhs_norm`; zero when the RHS is.
    pub fn reduction(&self) -> f64 {
        if self.rhs_norm > 0.0 {
            self.residual_norm / self.rhs_norm
        } else {
            0.0
        }
    }
}

fn norm(v: &[f64]) -> f64 {
    v.iter().map(|x| x * x).sum::<f64>().sqrt()
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn true_residual_norm(matrix: &BlockSparseMatrix, rhs: &[f64], x: &[f64]) -> f64 {
    let mut ax = vec![0.0; rhs.len()];
    matrix.multiply(x, &mut ax);
    rhs.iter()
        .zip(&ax)
        .map(|(b, a)| (b - a) * (b - a))
        .sum::<f64>()
        .sqrt()
}

/// Solve `A x = b` on the row-scaled system.
///
/// `accumulation_blocks` are each cell's row-scaled accumulation block, row-major; only the CPR
/// route reads them. Returns the correction when one was produced, and the report either way.
pub fn solve(
    options: &LinearOptions,
    matrix: &BlockSparseMatrix,
    accumulation_blocks: &[Vec<f64>],
    rhs: &[f64],
) -> (Result<Vec<f64>, LinearFailure>, LinearSolveReport) {
    let route = options.route.resolve(matrix.rows());
    let rhs_norm = norm(rhs);
    let mut report = LinearSolveReport {
        route,
        rhs_norm,
        residual_norm: rhs_norm,
        iterations: 0,
        min_pivot: None,
        finite: false,
    };

    let outcome = match route {
        LinearRoute::DenseLu => {
            let mut dense = matrix.to_dense();
            let mut x = rhs.to_vec();
            let min_pivot = solve_dense(&mut dense, &mut x);
            report.min_pivot = Some(min_pivot);
            if min_pivot == 0.0 || !min_pivot.is_finite() {
                Err(LinearFailure::Singular {
                    min_pivot: Some(min_pivot),
                })
            } else {
                Ok(x)
            }
        }
        LinearRoute::SparseLu | LinearRoute::Auto => sparse_lu(matrix, rhs),
        LinearRoute::FgmresCpr => match Cpr::build(matrix, accumulation_blocks) {
            Ok(cpr) => {
                let (x, iterations) = fgmres(options, matrix, &cpr, rhs);
                report.iterations = iterations;
                Ok(x)
            }
            Err(e) => Err(e),
        },
    };

    let x = match outcome {
        Ok(x) => x,
        Err(e) => return (Err(e), report),
    };
    report.finite = x.iter().all(|v| v.is_finite());
    if !report.finite {
        return (Err(LinearFailure::NonFinite), report);
    }
    report.residual_norm = true_residual_norm(matrix, rhs, &x);
    #[cfg(test)]
    shadow::check(route, matrix, rhs, &x);

    if route == LinearRoute::FgmresCpr
        && report.residual_norm > options.relative_tolerance * rhs_norm
    {
        let failure = LinearFailure::NotConverged {
            iterations: report.iterations,
            relative_residual: report.reduction(),
        };
        return (Err(failure), report);
    }
    (Ok(x), report)
}

/// Solve `A x = b` in place by Gaussian elimination with partial pivoting, returning the smallest
/// pivot magnitude seen.
///
/// The oracle every other route is measured against: no structure exploited, nothing to get wrong.
pub fn solve_dense(matrix: &mut [Vec<f64>], rhs: &mut [f64]) -> f64 {
    let n = matrix.len();
    let mut min_pivot = f64::INFINITY;

    for k in 0..n {
        let mut pivot_row = k;
        for r in (k + 1)..n {
            if matrix[r][k].abs() > matrix[pivot_row][k].abs() {
                pivot_row = r;
            }
        }
        matrix.swap(k, pivot_row);
        rhs.swap(k, pivot_row);

        let pivot = matrix[k][k];
        min_pivot = min_pivot.min(pivot.abs());
        if pivot == 0.0 || !pivot.is_finite() {
            return min_pivot;
        }
        for r in (k + 1)..n {
            let factor = matrix[r][k] / pivot;
            if factor == 0.0 {
                continue;
            }
            for c in k..n {
                matrix[r][c] -= factor * matrix[k][c];
            }
            rhs[r] -= factor * rhs[k];
        }
    }

    for k in (0..n).rev() {
        let mut acc = rhs[k];
        for j in (k + 1)..n {
            acc -= matrix[k][j] * rhs[j];
        }
        rhs[k] = acc / matrix[k][k];
    }
    min_pivot
}

fn faer_matrix(
    rows: usize,
    triplets: impl Iterator<Item = (usize, usize, f64)>,
) -> Option<SparseRowMat<usize, f64>> {
    let triplets: Vec<Triplet<usize, usize, f64>> =
        triplets.map(|(r, c, v)| Triplet::new(r, c, v)).collect();
    SparseRowMat::<usize, f64>::try_new_from_triplets(rows, rows, &triplets).ok()
}

fn sparse_lu(matrix: &BlockSparseMatrix, rhs: &[f64]) -> Result<Vec<f64>, LinearFailure> {
    let singular = LinearFailure::Singular { min_pivot: None };
    let a = faer_matrix(matrix.rows(), matrix.triplets()).ok_or(singular.clone())?;
    let lu = a.sp_lu().map_err(|_| singular)?;
    let b = Col::from_fn(rhs.len(), |i| rhs[i]);
    let x = lu.solve(&b);
    Ok((0..rhs.len()).map(|i| x[i]).collect())
}

/// Invert a row-major `n x n` block in place by Gauss-Jordan with partial pivoting. Returns false
/// when it is singular.
fn invert_block(block: &mut [f64], n: usize) -> bool {
    let mut inverse = vec![0.0; n * n];
    for i in 0..n {
        inverse[i * n + i] = 1.0;
    }
    for k in 0..n {
        let mut pivot_row = k;
        for r in (k + 1)..n {
            if block[r * n + k].abs() > block[pivot_row * n + k].abs() {
                pivot_row = r;
            }
        }
        let pivot = block[pivot_row * n + k];
        if pivot == 0.0 || !pivot.is_finite() {
            return false;
        }
        if pivot_row != k {
            for c in 0..n {
                block.swap(k * n + c, pivot_row * n + c);
                inverse.swap(k * n + c, pivot_row * n + c);
            }
        }
        for c in 0..n {
            block[k * n + c] /= pivot;
            inverse[k * n + c] /= pivot;
        }
        for r in 0..n {
            if r == k {
                continue;
            }
            let factor = block[r * n + k];
            if factor == 0.0 {
                continue;
            }
            for c in 0..n {
                block[r * n + c] -= factor * block[k * n + c];
                inverse[r * n + c] -= factor * inverse[k * n + c];
            }
        }
    }
    block.copy_from_slice(&inverse);
    true
}

/// `out = a * b` for row-major `n x n` blocks.
fn block_product(a: &[f64], b: &[f64], out: &mut [f64], n: usize) {
    for i in 0..n {
        for j in 0..n {
            let mut acc = 0.0;
            for k in 0..n {
                acc += a[i * n + k] * b[k * n + j];
            }
            out[i * n + j] = acc;
        }
    }
}

/// The true-IMPES pressure weights of one cell: `D^T w = e_p` for its row-scaled accumulation
/// block `D`. `None` when `D` is singular.
pub fn pressure_weights(accumulation_block: &[f64], n: usize) -> Option<Vec<f64>> {
    let mut transposed = vec![vec![0.0; n]; n];
    for (i, row) in transposed.iter_mut().enumerate() {
        for (v, entry) in row.iter_mut().enumerate() {
            *entry = accumulation_block[v * n + i];
        }
    }
    let mut w = vec![0.0; n];
    w[0] = 1.0;
    let min_pivot = solve_dense(&mut transposed, &mut w);
    (min_pivot > 0.0 && min_pivot.is_finite() && w.iter().all(|x| x.is_finite())).then_some(w)
}

/// Block-ILU(0) of the full scaled matrix: `L` and `U` share the matrix's pattern, and the
/// diagonal blocks of `U` are stored inverted.
struct BlockIlu0 {
    factors: BlockSparseMatrix,
}

impl BlockIlu0 {
    fn build(matrix: &BlockSparseMatrix) -> Result<Self, LinearFailure> {
        let n = matrix.block_size();
        let mut f = matrix.clone();
        let mut product = vec![0.0; n * n];
        for row in 0..f.block_rows() {
            let diag = f.diagonal_position(row);
            for position in f.row_range(row).start..diag {
                let k = f.block_column(position);
                // L_ik = A_ik * U_kk^-1 (the diagonal of row k is already inverted).
                let inv_kk = f.block(f.diagonal_position(k)).to_vec();
                let a_ik = f.block(position).to_vec();
                block_product(&a_ik, &inv_kk, &mut product, n);
                f.block_mut(position).copy_from_slice(&product);
                let l_ik = product.clone();
                // A_ij -= L_ik * U_kj for every j > k in both row i's and row k's pattern.
                for kj in (f.diagonal_position(k) + 1)..f.row_range(k).end {
                    let j = f.block_column(kj);
                    if let Some(ij) = f.position(row, j) {
                        let u_kj = f.block(kj).to_vec();
                        block_product(&l_ik, &u_kj, &mut product, n);
                        for (a, p) in f.block_mut(ij).iter_mut().zip(&product) {
                            *a -= p;
                        }
                    }
                }
            }
            if !invert_block(f.block_mut(diag), n) {
                return Err(LinearFailure::Preconditioner {
                    cell: row,
                    stage: "block-ILU(0) diagonal",
                });
            }
        }
        Ok(Self { factors: f })
    }

    /// `x = (LU)^-1 r`.
    fn apply(&self, r: &[f64], x: &mut [f64]) {
        let f = &self.factors;
        let n = f.block_size();
        x.copy_from_slice(r);
        // Forward: y_i = r_i - sum_{k<i} L_ik y_k (unit lower blocks).
        for row in 0..f.block_rows() {
            for position in f.row_range(row).start..f.diagonal_position(row) {
                let k = f.block_column(position);
                let block = f.block(position);
                for i in 0..n {
                    let mut acc = 0.0;
                    for j in 0..n {
                        acc += block[i * n + j] * x[k * n + j];
                    }
                    x[row * n + i] -= acc;
                }
            }
        }
        // Backward: x_i = U_ii^-1 (y_i - sum_{j>i} U_ij x_j).
        let mut tmp = vec![0.0; n];
        for row in (0..f.block_rows()).rev() {
            let diag = f.diagonal_position(row);
            tmp.copy_from_slice(&x[row * n..(row + 1) * n]);
            for position in (diag + 1)..f.row_range(row).end {
                let j = f.block_column(position);
                let block = f.block(position);
                for i in 0..n {
                    let mut acc = 0.0;
                    for c in 0..n {
                        acc += block[i * n + c] * x[j * n + c];
                    }
                    tmp[i] -= acc;
                }
            }
            let inv = f.block(diag);
            for i in 0..n {
                x[row * n + i] = dot(&inv[i * n..(i + 1) * n], &tmp);
            }
        }
    }
}

/// The two-stage compositional CPR preconditioner. See the module docs.
struct Cpr<'a> {
    matrix: &'a BlockSparseMatrix,
    weights: Vec<Vec<f64>>,
    coarse: faer::sparse::linalg::solvers::Lu<usize, f64>,
    smoother: BlockIlu0,
}

impl<'a> Cpr<'a> {
    fn build(
        matrix: &'a BlockSparseMatrix,
        accumulation_blocks: &[Vec<f64>],
    ) -> Result<Self, LinearFailure> {
        let n = matrix.block_size();
        let cells = matrix.block_rows();

        let mut weights = Vec::with_capacity(cells);
        for (cell, block) in accumulation_blocks.iter().enumerate() {
            weights.push(
                pressure_weights(block, n).ok_or(LinearFailure::Preconditioner {
                    cell,
                    stage: "pressure-weight accumulation",
                })?,
            );
        }

        // C_cd = sum_i w_c,i A_(c,d)[i][p]
        let mut coarse_triplets = Vec::with_capacity(matrix.stored_blocks());
        for (cell, w) in weights.iter().enumerate() {
            for position in matrix.row_range(cell) {
                let block = matrix.block(position);
                let value: f64 = (0..n).map(|i| w[i] * block[i * n]).sum();
                coarse_triplets.push((cell, matrix.block_column(position), value));
            }
        }
        let coarse = faer_matrix(cells, coarse_triplets.into_iter())
            .and_then(|c| c.sp_lu().ok())
            .ok_or(LinearFailure::Preconditioner {
                cell: 0,
                stage: "coarse pressure operator",
            })?;

        let smoother = BlockIlu0::build(matrix)?;
        Ok(Self {
            matrix,
            weights,
            coarse,
            smoother,
        })
    }

    /// `z = M^-1 r`.
    fn apply(&self, r: &[f64], z: &mut [f64]) {
        let n = self.matrix.block_size();
        let cells = self.matrix.block_rows();

        let restricted = Col::from_fn(cells, |c| dot(&self.weights[c], &r[c * n..(c + 1) * n]));
        let pressure = self.coarse.solve(&restricted);
        let mut x1 = vec![0.0; r.len()];
        for c in 0..cells {
            x1[c * n] = pressure[c];
        }

        let mut ax1 = vec![0.0; r.len()];
        self.matrix.multiply(&x1, &mut ax1);
        let r2: Vec<f64> = r.iter().zip(&ax1).map(|(a, b)| a - b).collect();
        self.smoother.apply(&r2, z);
        for (zi, xi) in z.iter_mut().zip(&x1) {
            *zi += xi;
        }
    }
}

/// Restarted flexible GMRES, right-preconditioned, from a zero initial guess. Each cycle restarts
/// from the **true** residual, and the solve stops only when the true residual meets tolerance —
/// a Givens recurrence that drifts from it cannot declare success (the black-oil lesson of
/// `efde1f4`).
fn fgmres(
    options: &LinearOptions,
    matrix: &BlockSparseMatrix,
    preconditioner: &Cpr<'_>,
    rhs: &[f64],
) -> (Vec<f64>, usize) {
    let size = rhs.len();
    let mut x = vec![0.0; size];
    let target = options.relative_tolerance * norm(rhs);
    let restart = options.restart.max(1);
    let mut iterations = 0;

    let mut r = rhs.to_vec();
    let mut ax = vec![0.0; size];
    while iterations < options.max_iterations {
        let beta = norm(&r);
        if !beta.is_finite() || beta <= target {
            break;
        }

        let mut basis: Vec<Vec<f64>> = vec![r.iter().map(|v| v / beta).collect()];
        let mut preconditioned: Vec<Vec<f64>> = Vec::with_capacity(restart);
        let mut hessenberg: Vec<Vec<f64>> = Vec::with_capacity(restart);
        let mut cs: Vec<f64> = Vec::with_capacity(restart);
        let mut sn: Vec<f64> = Vec::with_capacity(restart);
        let mut g = vec![beta];

        for j in 0..restart {
            if iterations >= options.max_iterations {
                break;
            }
            iterations += 1;

            let mut z = vec![0.0; size];
            preconditioner.apply(&basis[j], &mut z);
            let mut w = vec![0.0; size];
            matrix.multiply(&z, &mut w);
            preconditioned.push(z);

            let mut h = vec![0.0; j + 2];
            for (i, v) in basis.iter().enumerate() {
                h[i] = dot(&w, v);
                for (wk, vk) in w.iter_mut().zip(v) {
                    *wk -= h[i] * vk;
                }
            }
            h[j + 1] = norm(&w);

            for i in 0..j {
                let (a, b) = (h[i], h[i + 1]);
                h[i] = cs[i] * a + sn[i] * b;
                h[i + 1] = -sn[i] * a + cs[i] * b;
            }
            let denom = crate::math::hypot(h[j], h[j + 1]);
            let (c, s) = if denom == 0.0 {
                (1.0, 0.0)
            } else {
                (h[j] / denom, h[j + 1] / denom)
            };
            let breakdown = h[j + 1] == 0.0;
            if !breakdown {
                basis.push(w.iter().map(|v| v / h[j + 1]).collect());
            }
            h[j] = c * h[j] + s * h[j + 1];
            h[j + 1] = 0.0;
            cs.push(c);
            sn.push(s);
            g.push(-s * g[j]);
            g[j] *= c;
            hessenberg.push(h);

            if g[j + 1].abs() <= target || breakdown {
                break;
            }
        }

        // Back-substitute the small triangular system and update x.
        let m = hessenberg.len();
        let mut y = vec![0.0; m];
        for i in (0..m).rev() {
            let mut acc = g[i];
            for k in (i + 1)..m {
                acc -= hessenberg[k][i] * y[k];
            }
            y[i] = acc / hessenberg[i][i];
        }
        for (yi, z) in y.iter().zip(&preconditioned) {
            for (xk, zk) in x.iter_mut().zip(z) {
                *xk += yi * zk;
            }
        }

        matrix.multiply(&x, &mut ax);
        for k in 0..size {
            r[k] = rhs[k] - ax[k];
        }
        if !x.iter().all(|v| v.is_finite()) {
            break;
        }
    }
    (x, iterations)
}

/// The dense oracle, run beside every sparse-LU solve small enough to afford it, under test only.
///
/// This is what makes "the sparse route agrees with dense to roundoff on every existing
/// compositional fixture" a property of the whole gate rather than of a few hand-picked systems:
/// every fixture that solves a Newton system checks it, through whatever route that fixture takes.
#[cfg(test)]
pub(crate) mod shadow {
    use super::{BlockSparseMatrix, LinearRoute, solve_dense};

    /// Largest system the shadow solves, in rows. Dense LU is cubic; above this a fixture would be
    /// paying for the oracle rather than testing anything.
    pub const MAX_ROWS: usize = 300;

    /// Largest admissible `||x_sparse - x_dense||_inf / ||x_dense||_inf`.
    ///
    /// Two backward-stable factorizations of the same matrix differ by about `cond(A) * eps`.
    /// Across the 6,896 systems the compositional gate solves the worst measured is 1.7e-12; this
    /// leaves more than two orders of conditioning headroom and still fails loudly on any wrong
    /// entry.
    pub const TOLERANCE: f64 = 1e-9;

    pub fn check(route: LinearRoute, matrix: &BlockSparseMatrix, rhs: &[f64], x: &[f64]) {
        if route != LinearRoute::SparseLu || matrix.rows() > MAX_ROWS {
            return;
        }
        let mut dense = matrix.to_dense();
        let mut oracle = rhs.to_vec();
        let pivot = solve_dense(&mut dense, &mut oracle);
        if pivot == 0.0 || !pivot.is_finite() {
            return;
        }
        let scale = oracle.iter().fold(0.0f64, |m, v| m.max(v.abs()));
        let diff = x
            .iter()
            .zip(&oracle)
            .fold(0.0f64, |m, (a, b)| m.max((a - b).abs()));
        let relative = if scale > 0.0 { diff / scale } else { diff };
        if std::env::var_os("RESSIM_COMP_LINEAR_SHADOW_LOG").is_some() {
            eprintln!(
                "comp-linear-shadow rows={} relative={relative:e}",
                matrix.rows()
            );
        }
        assert!(
            relative <= TOLERANCE,
            "sparse LU disagrees with the dense oracle on a {}-row system: {relative:e}",
            matrix.rows()
        );
    }
}
