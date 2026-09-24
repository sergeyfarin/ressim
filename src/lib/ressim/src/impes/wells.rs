//! Well terms of the IMPES pressure equation.
//!
//! Two kinds of completion enter the pressure matrix as `PI·(p − p_conn)` rather than as a fixed
//! source:
//!
//! - A **BHP-controlled** completion, whose connection pressure `bhp + head` is known.
//! - A completion of a **rate-controlled well with more than one completion**. Its BHP is an
//!   extra unknown of the pressure system, fixed by the well's rate:
//!   `Σ_k PI_k·(p_k − bhp − head_k) = Q`. The split of `Q` between completions is then implicit.
//!   It used to be computed from the beginning-of-substep pressures and held fixed, and with
//!   poorly communicating layers that explicit split went bang-bang: a layer taking 37 m³/day in
//!   one substep took none in the next, and the reversals it drove near the well destroyed water
//!   at the saturation clamps, 42 % of pore volume on the shipped `wf_gravity` tight-`k_v` case
//!   (#10). A single-completion rate well keeps its fixed source: its split is trivially exact.
//!
//! Either kind is **active** only while it flows the way its well does, the same sign test
//! transport and FIM's connection law apply. The pressure equation used to keep a completion
//! flowing the wrong way, crossflowing back into the formation, while transport shut it (#10).

use nalgebra::DVector;

use crate::ReservoirSimulator;
use crate::well_control::{ResolvedWellControl, WellControlDecision, WellControlGroupKey};

/// Re-solves allowed while active completions and BHP-limit switches settle; the last is kept.
pub(crate) const MAX_ACTIVE_SET_PASSES: usize = 8;

/// Where a completion's BHP comes from.
#[derive(Clone, Copy, Debug, PartialEq)]
enum CompletionBhp {
    Fixed(f64),
    /// Index into [`WellSystem::implicit_wells`].
    Implicit(usize),
}

#[derive(Clone, Debug)]
struct CompletionTerm {
    /// Index into `ReservoirSimulator::wells`.
    entry: usize,
    cell: usize,
    productivity_index: f64,
    head_offset_bar: f64,
    injector: bool,
    bhp: CompletionBhp,
}

#[derive(Clone, Debug)]
struct ImplicitRateWell {
    /// Indices into [`WellSystem::terms`].
    terms: Vec<usize>,
    /// Signed total reservoir rate [m³/day]: positive out of the formation.
    target_m3_day: f64,
    bhp_limit_bar: f64,
    injector: bool,
    bhp_bar: f64,
    /// The solved BHP breached the limit this substep, so the well is held at the limit.
    limited: bool,
}

impl ImplicitRateWell {
    fn breaches_limit(&self, bhp_bar: f64) -> bool {
        if !self.bhp_limit_bar.is_finite() {
            return false;
        }
        if self.injector {
            bhp_bar > self.bhp_limit_bar
        } else {
            bhp_bar < self.bhp_limit_bar
        }
    }
}

/// Every well term of one substep's pressure system.
pub(crate) struct WellSystem {
    n_cells: usize,
    terms: Vec<CompletionTerm>,
    implicit_wells: Vec<ImplicitRateWell>,
    active: Vec<bool>,
    /// Entries (indices into `ReservoirSimulator::wells`) whose rate is a fixed source.
    fixed_rate_entries: Vec<usize>,
}

impl WellSystem {
    /// Unknowns of the extended system: cell pressures, then one BHP per implicit well.
    pub(crate) fn unknowns(&self) -> usize {
        self.n_cells + self.implicit_wells.len()
    }

    pub(crate) fn has_fixed_rate(&self, entry: usize) -> bool {
        self.fixed_rate_entries.contains(&entry)
    }

    fn connection_pressure(&self, term: &CompletionTerm) -> f64 {
        let bhp = match term.bhp {
            CompletionBhp::Fixed(bhp) => bhp,
            CompletionBhp::Implicit(g) => self.implicit_wells[g].bhp_bar,
        };
        bhp + term.head_offset_bar
    }

    fn flows(&self, term: &CompletionTerm, cell_pressure_bar: f64) -> bool {
        let drawdown = cell_pressure_bar - self.connection_pressure(term);
        if term.injector {
            drawdown < 0.0
        } else {
            drawdown > 0.0
        }
    }

    /// A starting guess for the extended unknown vector.
    pub(crate) fn initial_guess(&self, cell_pressures: &[f64]) -> DVector<f64> {
        let mut x = DVector::<f64>::zeros(self.unknowns());
        for (i, p) in cell_pressures.iter().enumerate() {
            x[i] = *p;
        }
        for (g, well) in self.implicit_wells.iter().enumerate() {
            x[self.n_cells + g] = well.bhp_bar;
        }
        x
    }

    /// Add the active well terms to a copy of the cell operator.
    ///
    /// `vals`/`rhs` hold the cell rows. Returns the extended triplets, right-hand side and
    /// Jacobi preconditioner. Well rows are scaled so their diagonal `Σ PI` is positive:
    /// `Σ PI_k·(bhp + head_k − p_k) = −Q`.
    pub(crate) fn assemble(
        &self,
        rows: &[usize],
        cols: &[usize],
        vals: &[f64],
        rhs: &DVector<f64>,
        diag_positions: &[usize],
    ) -> (Vec<usize>, Vec<usize>, Vec<f64>, DVector<f64>, DVector<f64>) {
        let n = self.unknowns();
        let mut rows = rows.to_vec();
        let mut cols = cols.to_vec();
        let mut vals = vals.to_vec();
        let mut b = DVector::<f64>::zeros(n);
        for i in 0..self.n_cells {
            b[i] = rhs[i];
        }
        let mut well_diag = vec![0.0f64; self.implicit_wells.len()];

        for (term, _) in self.terms.iter().zip(&self.active).filter(|(_, on)| **on) {
            let pi = term.productivity_index;
            vals[diag_positions[term.cell]] += pi;
            match term.bhp {
                CompletionBhp::Implicit(g) if !self.implicit_wells[g].limited => {
                    let well_row = self.n_cells + g;
                    rows.push(term.cell);
                    cols.push(well_row);
                    vals.push(-pi);
                    b[term.cell] += pi * term.head_offset_bar;
                    rows.push(well_row);
                    cols.push(term.cell);
                    vals.push(-pi);
                    well_diag[g] += pi;
                    b[well_row] -= pi * term.head_offset_bar;
                }
                _ => b[term.cell] += pi * self.connection_pressure(term),
            }
        }
        for (g, well) in self.implicit_wells.iter().enumerate() {
            let well_row = self.n_cells + g;
            rows.push(well_row);
            cols.push(well_row);
            if well.limited || well_diag[g] <= 0.0 {
                // Held at a known BHP: the row just pins the unknown.
                vals.push(1.0);
                b[well_row] = well.bhp_bar;
            } else {
                vals.push(well_diag[g]);
                b[well_row] -= well.target_m3_day;
            }
        }

        let mut diag = vec![0.0f64; n];
        for idx in 0..vals.len() {
            if rows[idx] == cols[idx] {
                diag[rows[idx]] += vals[idx];
            }
        }
        let diag_inv = DVector::from_iterator(
            n,
            diag.iter()
                .map(|d| if d.abs() > f64::EPSILON { 1.0 / d } else { 1.0 }),
        );
        (rows, cols, vals, b, diag_inv)
    }

    /// Take the solved BHPs, apply BHP-limit switches and re-derive the active set. Returns
    /// whether anything changed, i.e. whether the system must be solved again.
    pub(crate) fn update(&mut self, solution: &DVector<f64>) -> bool {
        let mut changed = false;
        for g in 0..self.implicit_wells.len() {
            let well = &mut self.implicit_wells[g];
            if well.limited {
                continue;
            }
            let bhp = solution[self.n_cells + g];
            if well.breaches_limit(bhp) {
                well.limited = true;
                well.bhp_bar = well.bhp_limit_bar;
                changed = true;
            } else {
                well.bhp_bar = bhp;
            }
        }
        for t in 0..self.terms.len() {
            let flows = self.flows(&self.terms[t], solution[self.terms[t].cell]);
            if flows != self.active[t] {
                self.active[t] = flows;
                changed = true;
            }
        }
        changed
    }

    /// Set the implicit BHPs from a Newton iterate without touching the active set.
    pub(crate) fn set_bhps(&mut self, x: &DVector<f64>) {
        for g in 0..self.implicit_wells.len() {
            if !self.implicit_wells[g].limited {
                self.implicit_wells[g].bhp_bar = x[self.n_cells + g];
            }
        }
    }

    /// This substep's controls with every implicit well's completions on its solved BHP.
    pub(crate) fn step_controls(
        &self,
        well_controls: &[Option<ResolvedWellControl>],
    ) -> Vec<Option<ResolvedWellControl>> {
        let mut controls = well_controls.to_vec();
        for well in &self.implicit_wells {
            for &t in &well.terms {
                if let Some(control) = controls[self.terms[t].entry].as_mut() {
                    control.decision = WellControlDecision::Bhp {
                        bhp_bar: well.bhp_bar,
                    };
                    control.bhp_limited = control.bhp_limited || well.limited;
                    control.flowing_bhp = Some(well.bhp_bar);
                }
            }
        }
        controls
    }

    /// Newton residual `Σ q_k − Q` [m³/day] of each implicit well's rate, in unknown order,
    /// where `q_k` is the rate transport applies. A well held at its limit has none.
    pub(crate) fn rate_residuals(
        &self,
        sim: &ReservoirSimulator,
        cell_pressures: &DVector<f64>,
    ) -> Vec<f64> {
        self.implicit_wells
            .iter()
            .map(|well| {
                if well.limited {
                    return 0.0;
                }
                let produced: f64 = well
                    .terms
                    .iter()
                    .map(|&t| {
                        let term = &self.terms[t];
                        sim.completion_rate_for_bhp(
                            &sim.wells[term.entry],
                            cell_pressures[term.cell],
                            well.bhp_bar,
                        )
                        .unwrap_or(0.0)
                    })
                    .sum();
                produced - well.target_m3_day
            })
            .collect()
    }
}

impl ReservoirSimulator {
    /// Collect the substep's well terms from its resolved controls.
    pub(crate) fn build_well_system(
        &self,
        well_controls: &[Option<ResolvedWellControl>],
    ) -> WellSystem {
        let n_cells = self.nx * self.ny * self.nz;
        let mut terms = Vec::new();
        let mut implicit_wells: Vec<ImplicitRateWell> = Vec::new();
        let mut implicit_keys: Vec<WellControlGroupKey> = Vec::new();
        let mut fixed_rate_entries = Vec::new();

        for (entry, well) in self.wells.iter().enumerate() {
            let Some(control) = well_controls[entry] else {
                continue;
            };
            let cell = self.idx(well.i, well.j, well.k);
            match control.decision {
                WellControlDecision::Disabled => {}
                WellControlDecision::Bhp { bhp_bar } => {
                    if well.productivity_index.is_finite() && bhp_bar.is_finite() {
                        terms.push(CompletionTerm {
                            entry,
                            cell,
                            productivity_index: well.productivity_index,
                            head_offset_bar: well.head_offset_bar,
                            injector: well.injector,
                            bhp: CompletionBhp::Fixed(bhp_bar),
                        });
                    }
                }
                WellControlDecision::Rate { q_m3_day } => {
                    let key = self.well_control_group_key(well);
                    let completions = self
                        .wells
                        .iter()
                        .enumerate()
                        .filter(|(other, candidate)| {
                            self.well_control_group_key(candidate) == key
                                && matches!(
                                    well_controls[*other].map(|c| c.decision),
                                    Some(WellControlDecision::Rate { .. })
                                )
                        })
                        .count();
                    let bhp_guess = control.flowing_bhp.unwrap_or(well.bhp);
                    if completions < 2
                        || !well.productivity_index.is_finite()
                        || !bhp_guess.is_finite()
                    {
                        fixed_rate_entries.push(entry);
                        continue;
                    }
                    let g = match implicit_keys.iter().position(|k| *k == key) {
                        Some(g) => g,
                        None => {
                            implicit_keys.push(key);
                            implicit_wells.push(ImplicitRateWell {
                                terms: Vec::new(),
                                target_m3_day: 0.0,
                                bhp_limit_bar: self.well_control_config(well).bhp_limit,
                                injector: well.injector,
                                bhp_bar: bhp_guess,
                                limited: false,
                            });
                            implicit_wells.len() - 1
                        }
                    };
                    implicit_wells[g].terms.push(terms.len());
                    implicit_wells[g].target_m3_day += q_m3_day;
                    terms.push(CompletionTerm {
                        entry,
                        cell,
                        productivity_index: well.productivity_index,
                        head_offset_bar: well.head_offset_bar,
                        injector: well.injector,
                        bhp: CompletionBhp::Implicit(g),
                    });
                }
            }
        }

        let mut system = WellSystem {
            n_cells,
            terms,
            implicit_wells,
            active: Vec::new(),
            fixed_rate_entries,
        };
        system.active = system
            .terms
            .iter()
            .map(|term| system.flows(term, self.pressure[term.cell]))
            .collect();
        system
    }
}
