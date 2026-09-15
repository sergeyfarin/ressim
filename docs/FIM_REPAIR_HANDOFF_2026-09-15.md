# FIM repair handoff — F0–F8

Date: 2026-09-15. Series: `de4f23d`..`9e3fff1` on `master`, plus this record.
Execution plan: [`FIM_REPAIR_EXECUTION_PLAN_2026-09-14.md`](FIM_REPAIR_EXECUTION_PLAN_2026-09-14.md).
Applicability evidence: [`BLACK_OIL_VALIDATION.md`](BLACK_OIL_VALIDATION.md) section 5.

This is the record of what was repaired, what is validated, and what a compositional executor may
rely on. It is deliberately explicit about what it does **not** establish.

## Milestones

- **FIM-REPAIR-READY** — declared at `57ecb8e` (F6). Evidence: `BLACK_OIL_VALIDATION.md` §5.
- **FIM-COMPOSITIONAL-SEAM-READY** — declared here. F7 delivered the layout seam C7 consumes and
  proved black-oil behaviour unchanged; F8 replayed G0–G5 on the final tree.

## Repaired contracts

| # | Contract | Was | Commit |
|---|---|---|---|
| 1 | Well control stencil is the connected cell only | stale nine-cell oracle failing since `FIM-BUNDLE-X` | `de4f23d` |
| 2 | `total_mobility` terminates without a 3-phase SCAL table | infinite recursion → stack overflow, reachable via `addWell` | `925658c` |
| 3 | Injector pressure derivatives incl. the water branch | both `#[cfg(test)]` hand derivatives returned `0.0` for water; gas fixture was never a gas injector | `31a176a` |
| 4 | Repaired contracts actually execute in a gate | `fim::wells::tests::`, `assembly_ad`, `wells_ad` ungated; ignored-only filters counted as execution | `dddc54d` |
| 5 | Linear report describes the returned correction | sound, but only 1 of 10 table cells tested | `13a0e25` |
| 6 | Accepted residual belongs to the committed state; rejected steps leave no trace; closed systems conserve | asserted nowhere | `5ebdc78` |
| 7 | Row/column meaning is named, not inferred from `index % 3` | bare `3` / `% 3` / `== 0` literals across five modules | `e346b15` |
| 8 | `fim::flow_resv::` green and gated | stale 1×1×1 fixture failing on a clean tree, module ungated | `9e3fff1` |

## Validated envelope

- Two-phase and three-phase black oil with a PVT table, on both `OpmAligned` (default) and
  `Legacy` nonlinear flavors.
- Phase lifecycle covering `Undersaturated → Saturated` liberation, including the case where the
  two flavors commit **different parameterizations of the same physical state** (see below).
- Rejected-step rollback, closed-system component conservation (drift ~1e-9 relative over 8
  accepted steps), and the linear report contract across iterative/direct backends, with/without
  a well tail, finite/non-finite corrections, and singular/rejected solves.
- Multi-completion gravity on both solvers (reconstruction, not the shipped `wf_gravity` deck).

## Reusable interfaces for the compositional executor

Consume these; do not re-derive them.

- **`fim/layout.rs`** — `CELL_BLOCK_SIZE`, `CellPrimary { Pressure, WaterSaturation, Hydrocarbon }`
  (matrix **column** order), `CellEquation { Water, OilComponent, GasComponent }` (matrix **row**
  order). Discriminants are load-bearing: CPR restricts its coarse system onto local column 0.
- **`FimLinearBlockLayout`** (`fim/linear/mod.rs`) — `cell_unknown`, `cell_equation`,
  `split_cell_unknown`, `split_cell_equation`, `is_cell_pressure_column`, computed from the
  layout's own `cell_block_size` so a reduced system stays self-describing. This is C7's
  `EquationLayout`.
- **`FimLinearSolveReport`** (`fim/linear/mod.rs`) — `rhs_norm`, `final_residual_norm`,
  `reduction()`, `backend_used`, `used_fallback`, `failure_diagnostics`. F4 verified and tested
  that these always describe **the original full system and the correction actually returned**,
  including after well-Schur recovery.
- **`EquationScaling` / `EquationFamilyPeaks`** (`fim/scaling.rs`) — row-space family partition;
  `family_peaks` now indexes via `CellEquation` rather than `+1`/`+2`.

### Conventions a compositional model must preserve

- Row order per cell: `Water, OilComponent, GasComponent`. Column order: `Pressure,
  WaterSaturation, Hydrocarbon`. Cell blocks first, then well-BHP rows, then perforation-rate rows.
- Residuals are dt-integrated surface volumes (Sm³), **not** rates — unlike OPM, whose CNV carries
  an explicit `* dt`. See `convergence.rs`'s `CnvMbDiagnostics`.
- The hydrocarbon primary's *meaning* switches (Sg ↔ Rs) while its column index does not.
- A `FimLinearSolveReport` that returns no correction reports a zero correction with
  `final_residual_norm == rhs_norm`, i.e. `reduction() == 1`. That is the true residual of what it
  returned, not a placeholder.

## Two results that are easy to misread

**The two flavors commit different parameterizations of the same state.** After a three-phase
liberation step:

```text
OpmAligned  regime=Undersaturated  hydrocarbon_var=Rs=31.42   19 Newton iterations
Legacy      regime=Saturated       hydrocarbon_var=Sg=0.1196   7 Newton iterations
derived by both: so=0.680299  sg=0.119612  rs=20.5441  p=101.360
```

This is sound: `resolve_cell_flash` treats an `Undersaturated` cell whose `Rs` exceeds the
saturation cap as already flashed. Assert on the **derived** state, never on the regime label — a
label-equality assertion would wrongly fail `OpmAligned` while proving nothing. The 19-vs-7
difference is **not** a convergence finding: the flavors accept on different criteria and
`OpmAligned` reached the tighter residual (6.55e-8 vs 2.37e-7).

**`invert_tail_block` degrades silently.** It falls back to a diagonal approximation when the well
tail has no true inverse, which makes the Schur complement wrong with no signal inside the
elimination. The only thing that catches it is `recover_full_system_report` recomputing the
residual on the original full system. That safety net is load-bearing and is now tested.

## Outstanding, explicitly excluded

| Issue | Status after this series |
|---|---|
| [#10](https://github.com/sergeyfarin/ressim/issues/10) | Did not reproduce in reconstruction; IMPES-scoped. Shared well geometry ruled out by cross-solver agreement. Shipped deck not replayed. |
| [#11](https://github.com/sergeyfarin/ressim/issues/11) | **Reproduces** at 9.0 % (`Sg(nx=40)` FIM `0.030179` vs IMPES `0.033171`). Independent of the seam; measured, not explained. |
| [#12](https://github.com/sergeyfarin/ressim/issues/12) | Scenario/frontend scope. Both SPE1 release replays pass. |
| [#21](https://github.com/sergeyfarin/ressim/issues/21) | Not attempted. Needs source-pinned OPM Flow artifacts. |
| [#22](https://github.com/sergeyfarin/ressim/issues/22) | Damping/convergence extraction was already done; layout seam added. `flow_lifecycle.rs` still uses a literal `col % 3 == 0`. **Not closed.** |
| [#25](https://github.com/sergeyfarin/ressim/issues/25) | **Did not reproduce.** Recovery `0.9280` vs the issue's `1.006`; closure `+0.0109` vs `+0.0885`. Recommend closing. |

## What this handoff does not establish

- No OPM iteration-count parity claim is made anywhere in this series.
- #11's 9 % gap is measured; no mechanism is identified.
- #10 was tested through a reconstruction of the `wf_gravity` geometry, not its shipped deck.
- The committed `src/lib/ressim/pkg/` bindings regenerate with non-semantic ordering churn; that
  is tracked separately and was deliberately not committed during this series.
- CI workflow changes (F3) are locally equivalent-verified only; the remote job has never run.

## Provenance

Toolchain: `rustc 1.98.1 (48a229cea 2026-09-01)`, `cargo 1.98.1`, `node v24.18.0`, `pnpm 12.3.4`.

Final replay, run on the committed tree at `9e3fff1` with a clean worktree:

| Gate | Command | Result |
|---|---|---|
| G0 | 7 filters (see plan §G0) | all ok — 56 tests |
| G1 | 3 locked FIM contracts | 3/3 ok |
| G3 | `scripts/build-wasm.sh` + 6 `fim-wasm-diagnostic.mjs` controls | 6/6 exit 0 |
| G4 | `cargo fmt --check`, `build-wasm.sh`, `pnpm run validate:full`, `benchmark_buckley`, `git diff --check` | all exit 0; `validate:full` 42 gate lines |
| G5 | 3 `--release --ignored` replays | 3/3 exit 0 |

**Release-equivalence measurement.** The WASM was built from `origin/master` (`ffaf18f`) in a
detached worktree and the same six G3 controls were replayed against the `9e3fff1` build. Every
physics field is **identical in 6/6 cases** — trajectories, pressure envelopes, saturation ranges,
Newton counts, linear-solve counts and Krylov counts. Only wall-clock fields differ, which the
plan states are not bitwise data.

That is the whole behavioural delta of this series: the only shipped change that alters any
outcome is `925658c`, and it alters only a path that previously aborted the process. The
`CELL_BLOCK_SIZE` / `CellPrimary` / `CellEquation` substitutions in `assembly.rs`, `scaling.rs`
and `newton.rs` are literal-for-named-constant with identical values.

## First unblocked task in the fluid plan

**C0** — pin the fluid, consuming case and independent oracle
([`COMPOSITIONAL_FLUID_EXECUTION_PLAN_2026-09-14.md`](COMPOSITIONAL_FLUID_EXECUTION_PLAN_2026-09-14.md)).
C0–C6 are standalone thermodynamics and were never blocked by this series. **C7** (component
layout and geometry boundary) is the first task that consumes F7's interfaces, and it is now
unblocked.
