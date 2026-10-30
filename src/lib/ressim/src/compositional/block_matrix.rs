//! The compositional Jacobian's storage: block-sparse rows, one `N x N` block per cell coupling
//! (compositional plan, C10).
//!
//! Each cell carries `N` equations and `N` primaries, and a cell's equations depend only on its own
//! primaries and on those of the cells it shares a face with. Wells add only to the connected
//! cell's own block — a rate-controlled well's BHP unknown is eliminated locally before the
//! linear solve, so there is no well row and no well column here. The pattern is therefore the
//! face graph plus the diagonal, fixed for the whole run, and storing it
//! as blocks rather than scalars keeps the cell structure that the CPR preconditioner and the
//! block-ILU smoother both work on.
//!
//! **Why not dense.** A dense `cells*N` square is the right oracle on a three-cell column and the
//! wrong storage at SPE scale: SPE3's 324 cells at ten components are 3,240 rows, 84 MB of `f64`
//! before a single factorization, inside a browser heap. The block pattern of the same grid is a
//! few hundred kilobytes.
//!
//! **Structural zeros are stored.** A block is in the pattern because a face makes it so, not
//! because its value happens to be non-zero at this iterate. A face whose flux is zero today (equal
//! pressures, no gravity) still couples its cells tomorrow, and a pattern that changed with the
//! values would make every factorization rebuild its symbolic structure.

/// A square block-sparse matrix in block-CSR form, with sorted block columns in every block row and
/// the diagonal block always present.
#[derive(Clone, Debug, PartialEq)]
pub struct BlockSparseMatrix {
    block_size: usize,
    block_rows: usize,
    /// Start of each block row in `block_columns`; length `block_rows + 1`.
    row_start: Vec<usize>,
    /// Block column of each stored block, sorted within a row.
    block_columns: Vec<usize>,
    /// Position of each block row's diagonal block in `block_columns`.
    diagonal: Vec<usize>,
    /// Stored blocks, each row-major `block_size x block_size`, in `block_columns` order.
    values: Vec<f64>,
}

impl BlockSparseMatrix {
    /// An all-zero matrix whose pattern is the diagonal plus both off-diagonal blocks of every
    /// connection. Repeated connections share one block.
    ///
    /// Returns `None` when a connection names a block row outside the matrix or connects a block
    /// to itself — both are caller errors the assembler reports by face.
    pub fn from_connections(
        block_rows: usize,
        block_size: usize,
        connections: impl IntoIterator<Item = (usize, usize)>,
    ) -> Option<Self> {
        let mut neighbours: Vec<Vec<usize>> = (0..block_rows).map(|row| vec![row]).collect();
        for (i, j) in connections {
            if i >= block_rows || j >= block_rows || i == j {
                return None;
            }
            neighbours[i].push(j);
            neighbours[j].push(i);
        }

        let mut row_start = Vec::with_capacity(block_rows + 1);
        let mut block_columns = Vec::new();
        let mut diagonal = Vec::with_capacity(block_rows);
        row_start.push(0);
        for (row, columns) in neighbours.iter_mut().enumerate() {
            columns.sort_unstable();
            columns.dedup();
            let start = block_columns.len();
            let offset = columns.binary_search(&row).ok()?;
            diagonal.push(start + offset);
            block_columns.extend_from_slice(columns);
            row_start.push(block_columns.len());
        }

        let values = vec![0.0; block_columns.len() * block_size * block_size];
        Some(Self {
            block_size,
            block_rows,
            row_start,
            block_columns,
            diagonal,
            values,
        })
    }

    /// `N`, the rows and columns per block.
    pub fn block_size(&self) -> usize {
        self.block_size
    }

    /// Number of block rows (cells).
    pub fn block_rows(&self) -> usize {
        self.block_rows
    }

    /// Number of scalar rows.
    pub fn rows(&self) -> usize {
        self.block_rows * self.block_size
    }

    /// Number of stored blocks, structural zeros included.
    pub fn stored_blocks(&self) -> usize {
        self.block_columns.len()
    }

    /// Bytes held by the pattern and values, for the memory budget.
    pub fn storage_bytes(&self) -> usize {
        core::mem::size_of::<usize>()
            * (self.row_start.len() + self.block_columns.len() + self.diagonal.len())
            + core::mem::size_of::<f64>() * self.values.len()
    }

    /// Range of stored-block positions in block row `row`.
    pub(crate) fn row_range(&self, row: usize) -> core::ops::Range<usize> {
        self.row_start[row]..self.row_start[row + 1]
    }

    /// Block column of the stored block at `position`.
    pub(crate) fn block_column(&self, position: usize) -> usize {
        self.block_columns[position]
    }

    /// Position of block row `row`'s diagonal block.
    pub(crate) fn diagonal_position(&self, row: usize) -> usize {
        self.diagonal[row]
    }

    /// Position of block `(row, column)` among the stored blocks, if it is in the pattern.
    pub fn position(&self, row: usize, column: usize) -> Option<usize> {
        if row >= self.block_rows {
            return None;
        }
        let range = self.row_range(row);
        self.block_columns[range.clone()]
            .binary_search(&column)
            .ok()
            .map(|offset| range.start + offset)
    }

    /// The stored block at `position`, row-major.
    pub fn block(&self, position: usize) -> &[f64] {
        let size = self.block_size * self.block_size;
        &self.values[position * size..(position + 1) * size]
    }

    /// The stored block at `position`, row-major, for writing.
    pub fn block_mut(&mut self, position: usize) -> &mut [f64] {
        let size = self.block_size * self.block_size;
        &mut self.values[position * size..(position + 1) * size]
    }

    /// Scalar entry `(row, column)`; zero outside the pattern.
    pub fn get(&self, row: usize, column: usize) -> f64 {
        let b = self.block_size;
        match self.position(row / b, column / b) {
            Some(position) => self.block(position)[(row % b) * b + column % b],
            None => 0.0,
        }
    }

    /// Divide scalar row `r` by `scales[r]`.
    pub fn scale_rows(&mut self, scales: &[f64]) {
        let b = self.block_size;
        for row in 0..self.block_rows {
            for position in self.row_range(row) {
                let block = self.block_mut(position);
                for i in 0..b {
                    let scale = scales[row * b + i];
                    for entry in &mut block[i * b..(i + 1) * b] {
                        *entry /= scale;
                    }
                }
            }
        }
    }

    /// `y = A x`.
    pub fn multiply(&self, x: &[f64], y: &mut [f64]) {
        let b = self.block_size;
        for row in 0..self.block_rows {
            let out = &mut y[row * b..(row + 1) * b];
            out.fill(0.0);
            for position in self.row_range(row) {
                let column = self.block_columns[position];
                let block = self.block(position);
                let input = &x[column * b..(column + 1) * b];
                for i in 0..b {
                    let mut acc = 0.0;
                    for j in 0..b {
                        acc += block[i * b + j] * input[j];
                    }
                    out[i] += acc;
                }
            }
        }
    }

    /// The same matrix, dense. For the direct oracle and for tests on small grids only.
    pub fn to_dense(&self) -> Vec<Vec<f64>> {
        let b = self.block_size;
        let n = self.rows();
        let mut dense = vec![vec![0.0; n]; n];
        for row in 0..self.block_rows {
            for position in self.row_range(row) {
                let column = self.block_columns[position];
                let block = self.block(position);
                for i in 0..b {
                    for j in 0..b {
                        dense[row * b + i][column * b + j] = block[i * b + j];
                    }
                }
            }
        }
        dense
    }

    /// Every stored scalar as `(row, column, value)`, structural zeros included.
    pub fn triplets(&self) -> impl Iterator<Item = (usize, usize, f64)> + '_ {
        let b = self.block_size;
        (0..self.block_rows).flat_map(move |row| {
            self.row_range(row).flat_map(move |position| {
                let column = self.block_columns[position];
                let block = self.block(position);
                (0..b * b).map(move |k| (row * b + k / b, column * b + k % b, block[k]))
            })
        })
    }
}
