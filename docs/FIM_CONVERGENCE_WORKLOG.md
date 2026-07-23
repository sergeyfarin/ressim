# FIM Convergence Worklog

This file is the active investigation log for live FIM convergence work.
Use `docs/FIM_STATUS.md` for the current consolidated solver status.
Use this worklog only for active observations, reproductions, traces, and next hypotheses while an issue is still live.

Historical narrative was trimmed out of this file twice:
- March 2026 tracker history from `TODO.md`: `docs/FIM_HISTORY_2026-03.md`
- Full live worklog snapshot through 2026-04-06: `docs/FIM_CONVERGENCE_ARCHIVE_2026-03_to_2026-04-06.md`
- Water/gas shelf investigations, Phase 5 AD-assembler cutover, Phase 6 (legacy Jacobian
  retirement), Phase 7 (OPM-style Newton globalization), Phase 8 (hotspot state
  characterization), and the Hypothesis C row-scaling attempt (2026-04-08 through 2026-07-03):
  `docs/FIM_CONVERGENCE_ARCHIVE_2026-04-08_to_2026-07-03.md`

## Active Scope
- Keep this file limited to current-head repros, latest measurements, and next solver questions.
- Treat resolved correctness hardening and old exploratory branches as archival unless they reopen on current head.
- Current active repro set:
  - hard water shelf: `water-pressure --grid 12x12x3 --steps 1 --dt 1`
  - shipped gas shelf: `gas-rate --grid 10x10x3 --steps 6 --dt 0.25`
  - over-threshold CPR probe: `water-pressure --grid 23x23x1 --steps 1 --dt 0.25`

### Strategy reconciliation after Y2a (2026-07-13; documentation only)

Reviewed the status, registry, Bundle Y parity plan, historical OPM strategy/gap documents, TODO,
and local OPM source before choosing the post-Y2a slice. The current TODO's direction was too
narrow: it proposed selecting an active-bound derivative convention before establishing whether
OPM treats `Swc` as a hard Newton-state bound.

Source evidence changes the order. ResSim calls `FimState::enforce_cell_bounds` after Newton
updates (`fim/state.rs:250,390,436`) and clamps `Sw` to `Swc`. OPM scales saturation increments
with `dsMax` (`OPM/opm-simulators/opm/models/blackoil/blackoilnewtonmethod.hpp:266-267`), while
post-update `chopAndNormalizeSaturations` is conditional (`:456-457`) and
`ProjectSaturations::value` defaults false
(`blackoilnewtonmethodparams.hpp:42`). This is a demonstrated code-path divergence, not yet a
causal verdict: exact-deck options, endpoint behavior, normalization, and variable switching still
need a complete audit.

Created `FIM_OPM_CONVERGENCE_EXECUTION_PLAN.md` as the active decision-frontier plan. It orders the
work as source audit (Y2b0), test-only boundary characterization (Y2b1), one reversible
`OpmAligned` behavior probe only if authorized (Y2b2), and the oracle/control promotion matrix
(Y2c). G4/G5, heavy-case branching, controller parity, AMG, and acceptance changes are explicitly
deferred until that evidence selects one branch. No solver behavior or test code changed.

### Bundle Y Y2b0 — bound/update source audit (2026-07-13; complete)

**Commit:** `5c29a9d` (documentation-only predecessor; solver tree clean). **Hypothesis:** the
exact injector plateau could be caused by a difference between OPM's saturation-state update and
ResSim's hard `Swc` projection, independently of the Y2a whole-row derivative mismatch.

Confirmed the effective reference options, rather than assuming them from source: the tracked deck
contains no projection or `ds-max` override; installed `flow --help` reports
`project-saturations=false`, `ds-max=0.2`; the tracked harness was replayed with
`scripts/opm-ressim-compare.sh --opm-only --no-build-wasm --out-dir
/tmp/ressim-y2b0-opm-audit`. It passed fixture verification and `CASE.INFOSTEP` records six
uncut 0.25-day steps with `NewtIt 7/5/4/3/4/3`, all `Conv=1`. The first sandbox run failed before
Flow initialization due to MPI socket restrictions; the identical elevated replay passed, so this
is an environment constraint rather than a solver result.

Source verdict: OPM limits the raw saturation update to `dsMax` and passes raw primary-variable
saturations into accumulation; its optional normalization is off. Endpoint material-law values are
clamped separately. ResSim also uses the 0.2 per-cell chop in `OpmAligned`, but immediately
hard-clamps stored `Sw`, `Sg`, and oil complement before the next assembly. Additionally, OPM
adapts primary variables after every update, while ResSim freezes the hydrocarbon regime until
accepted-state evaluation. Full citations and the three-boundary table are in
`FIM_OPM_PARITY_PLAN.md` §15.1.

**Verdict:** Y2b0 PASS. The state-policy divergence is real and sufficiently specified to justify
Y2b1, but it does not prove that removing the hard bound fixes the plateau. Next authorized work
is test-only boundary characterization; no acceptance, clamp, G4/G5, controller, or AMG change.

### Bundle Y Y2b1 — boundary characterization (2026-07-13; complete, test-only)

**Hypothesis:** ResSim's hard post-update projection, rather than the linear backend or a need to
average a kink derivative, destroys the Newton step in the exact injector failure path.

Added only `#[cfg(test)]` machinery: an unbounded candidate view in `fim/state.rs`, a four-bound
guard (`Swc`, `Sg=0`, upper `Sw`, upper `Sg`), a one-cell gas-injector AD/legacy/FD fixture, and
the exact driver's `FIM_Y2B_AUDIT=1` trace. The fixture exercises `bound-eps`, `bound`, and
`bound+eps`, with injector `rate_consistency` plus connected water/oil/gas rows; it prints
residuals, AD/legacy entries, and forward/backward/central FD. It passes and makes the kink
semantics unambiguous: AD equals the active one-sided derivative, while central FD crosses the
branch. This is evidence against derivative averaging, not against AD.

Exact command (live):
`FIM_Y2A_AUDIT=1 FIM_Y2B_AUDIT=1 FIM_MAX_SUBSTEPS=1 FIM_TRACE_FILE=/tmp/ressim-y2b-live.log
FIM_TRACE_DT_BELOW=1 cargo test --release --manifest-path src/lib/ressim/Cargo.toml
repro_gas_rate_10x10x3_y1j -- --ignored --nocapture`. The matching forced-direct command adds
`FIM_FORCE_DIRECT_LINEAR=1` and writes `/tmp/ressim-y2b-direct.log`; it accepted the same capped
first rung (`dt=0.000978384825`, five nonlinear retries). At a representative late rung
(`dt=9.78384825e-4`, iter 5), both backends propose `Sw=0.15→0.1499514237572`. Raw next water
residual follows the assembled linear prediction (`-8.53e-11` live; direct is within trace roundoff),
but the normal candidate projects `Sw` back to `0.15` and its next water residual is
`4.8591978e-3`, essentially the pre-step `4.8591979e-3`. Rate-consistency, oil, and gas rows also
show a large projection effect; water is the cleanest first-order discriminator.

**Verdict:** Y2b1 PASS; Y2b2 is authorized. This proves that the hard projection breaks the
Newton prediction on the exact plateau and direct/live equivalence rules out an iterative-linear
explanation. It does not prove that a raw-state policy alone converges the full case. Next work is
one default-off, `OpmAligned`-only coherent policy probe; no acceptance widening, generic clamp
change, derivative averaging, G4/G5 restructuring, or controller work is in scope.

**Validation:** focused Y2b state and injector fixtures pass; `fim::assembly_ad::tests` passes
4/4; `validate-solver-coverage.sh fim` passes 9/9. Its `shared` follow-on has one repeatable
unrelated failure: `tests::runtime_api::closed_system_public_step_keeps_same_water_inventory_on_both_solvers`
expects one FIM substep and sees two (`runtime_api.rs:81`). The new paths are `#[cfg(test)]` and
disabled absent `FIM_Y2B_AUDIT`, so they cannot affect that normal public-step trajectory; an
isolated rerun fails identically. Recorded as a pre-existing baseline issue, not changed here.

### Bundle Y Y2b2 — raw saturation-state policy probe (2026-07-13; inconclusive and removed)

**Hypothesis:** retaining OPM-style raw `Sw`/`Sg` after the already-matching `ds-max` chop would
turn the measured first-order closure into a larger accepted exact first rung, consistently for
live and forced-direct linear paths. The native-only, default-off probe kept pressure/control
bounds and applied only when `OpmAligned` was already enabled; Legacy remained projected.

Clean committed baseline (`8865af6`): `FIM_MAX_SUBSTEPS=1 cargo test --release
--manifest-path src/lib/ressim/Cargo.toml repro_gas_rate_10x10x3_y1j -- --ignored --nocapture`
accepted `dt=0.000978384825` after five nonlinear retries. Probe live command:
`FIM_Y2B_RAW_SATURATION=1 FIM_MAX_SUBSTEPS=1 FIM_TRACE_FILE=/tmp/ressim-y2b2-live.log
FIM_TRACE_DT_BELOW=1 cargo test --release --manifest-path src/lib/ressim/Cargo.toml
repro_gas_rate_10x10x3_y1j -- --ignored --nocapture`; it accepted `dt=0.00898425` after three
linear-classified retries. Forced-direct with the same flag wrote
`/tmp/ressim-y2b2-direct.log` and accepted nothing: 16 linear-classified retries.

The traces agree exactly through `dt=0.027225`; at `0.00898425`, live has a third Newton
iteration and accepts (`res=4.737838e-5`, `mb=5.153094e-10`), while direct stops at iteration two
with dominant `well@900`, then repeats that failure down to the retry floor. The original slice
treated this as violating a valid direct/live gate. The field, setter, update branch,
candidate-validity exception, and test hook were deleted in the same working slice; no behavior
change remains.

**Forensic correction:** the predeclared gate was not valid. At `dt=0.00898425`, both paths reach
the same iteration-1 state (`bhp=[222.6115941403441,191.75240471787257]`, perforation rates equal
to roundoff). Live CPR then returns a finite, non-strict solve whose report supports measured
reduction `5.299e-3`; `OpmAligned` relaxed-accepts it, applies the correction, and iteration 2 is
`would_accept=strict`. Sparse LU also returns a finite solution, but
`fim/linear/sparse_lu_debug.rs` supplies `failure_diagnostics=None` on all outcomes. Well Schur
forwards that absence and `newton.rs` computes relaxed reduction only through the optional
failure payload, so forced-direct prints `reduction=n/a` and aborts. Later direct cutbacks show
`would_accept=strict` before another solve and still retry through the same missing-report path.
The traces never measured the direct iteration-1 correction or its full-system residual
reduction. This is the missing-diagnostics shape already identified in the earlier Y1b/Y1e
well-Schur audit, now shown to invalidate the Y2b2 verdict.

The probe also retained ResSim's frozen hydrocarbon regime and flash-side hydrocarbon clamps; it
did not include OPM's per-update `adaptPrimaryVariables`. It tested a narrow raw-`Sw` mechanism,
not the complete sourced OPM state/property/primary-variable lifecycle.

**Corrected verdict:** INCONCLUSIVE. The hard-projection inconsistency is real and the live 9.2x
first-rung improvement is provisional positive evidence. The forced-direct outcome is an oracle
defect, not a state-policy refutation. Next execute Y2b2a: add backend-neutral full-system linear
norms/reduction and prove the forced-direct report contract without restoring behavior. Commit
that first; Y2b2b then restores the same narrow probe, captures/replays the exact iteration-1
system, and compares corrections and row partitions. G4 is not authorized by the current direct
trace.

### Y2b2a checkpoint (2026-07-13) — backend-neutral direct oracle repaired

Before restoring the behavior probe, the linear report contract was repaired in the path that
invalidated Y2b2: `FimLinearSolveReport` now reports `rhs_norm`, `final_residual_norm`, and
`reduction()` independently of `failure_diagnostics`. Sparse LU publishes a RHS norm for its
finite non-strict result, and well-Schur replaces any reduced-system values with the norms of the
recovered original system. `OpmAligned` relaxed acceptance now consumes that common reduction.

Focused checks passed: `sparse_lu_non_strict_report_has_reduction`,
`well_schur_report_uses_full_system_norms`,
`opm_relaxed_linear_acceptance_is_backend_neutral`, the ten-test `assembly_ad` bucket, and the
existing direct well-elimination check. The forced-direct, raw-state-absent first-rung baseline
had no finite `reduction=n/a`; it did not itself exercise a non-strict solve, which is why the
synthetic contract test is retained. The shared validation script stopped at the previously
recorded unrelated closed-system public-step assertion (`2` versus `1`). A broad FIM-script run
did not complete in the execution harness and is not claimed as passing.

**Next.** Restore exactly the deleted native, default-off, `OpmAligned` raw-saturation probe;
capture the historical `dt=0.00898425`, iteration-1 system; replay CPR and direct production
dispatch on that one artifact; then classify the result. No G4/G5 or lifecycle behavior is
authorized by this checkpoint.

### Y2b2b checkpoint (2026-07-13) — direct Sparse LU returns the zero correction

The exact narrow raw-saturation probe was restored behind native-only, default-off
`FIM_Y2B_RAW_SATURATION`. It retains raw saturation/hydrocarbon state only; pressure and
well/control bounds remain active. A one-shot atomic claim prevents retry revisits from mixing
the replay corpus. The capped live run accepted `dt=0.00898425` after three linear retries and
three Newton iterations. Its captured decision system is `904x904`, 5540 nnz, RHS norm
`2.786530e1`, state checksum `5be96f7f56374d91`, SHA-256
`c503d9cb1781eab942d7621fb45f0baada3c40db7e32ffc297d17d1d35611561`.

Replay through the production selections is decisive: CPR produces reduction `4.830552e-3`,
full residual `1.346048e-1`, reservoir residual `1.346048e-1`, well residual `2.711113e-17`,
BHP correction `[0.22494971034013922, -0.031642581144180365]`, and perforation-rate correction
`[-5.37113701915633, 0.0013166107429607712]`. Explicit Sparse LU produces all zeros, reduction
`1.0`, full residual `2.786530e1`, reservoir residual `2.780911e1`, and well residual
`1.768765e0`; the maximum correction difference is `5.371137e0`. The forced-direct live run
therefore has 16 linear retries and no accepted substep. Its capture differs (5626 nnz) because
the zero direct iteration never advances the state; it reproduces the same all-zero direct solve.

**Checkpoint classification:** the zero direct result was not a state-policy verdict. Since
explicit direct does not use well Schur, the bounded next slice was sparse-matrix construction
versus factorization diagnosis on this exact capture. G4/G5 and nonlinear policy changes remained
blocked.

Validation: the raw-saturation state fixture, both release replay artifacts, both capped release
drivers, and the 10-test `assembly_ad` bucket pass. Wasm rebuilt successfully and all six required
control diagnostics completed without a runtime warning; the native-only flag remains unreachable
in those controls.

### Y2b2c checkpoint (2026-07-14) — exact raw-state matrix is rank-deficient

Scope stayed diagnostic-only. The preserved live artifact
`/tmp/ressim-y2b2b-live-capture-final/fim_capture_00000.txt` (SHA-256
`c503d9cb1781eab942d7621fb45f0baada3c40db7e32ffc297d17d1d35611561`) was replayed with a
test-only structural audit. It cleanly separates sparse conversion from factorization and checks
empty/duplicate/non-finite/zero structure before any solve.

The `904x904`, 5540-nnz matrix builds in faer; `sp_lu()` then fails. It has no empty rows,
duplicates, non-finite coefficients, or all-zero rows. It has exactly 120 empty columns and 120
missing/zero diagonal candidates. Every empty column is local variable 2 of a cell (gas primary),
with none in water, oil component, BHP, or perforation-rate families. Ordinary dense LU also
returns no solution, so this is not an implementation-specific sparse factorization failure.

The independent direct oracle is test-only rank-revealing dense SVD, not a production-path change.
At cutoff `1.138287e-6`, rank is `784/904`; its correction solves the compatible full system to
relative residual `1.101044e-12` (`||r||=3.068091e-11`, reservoir `3.068078e-11`, well
`9.032212e-14`). Its maximum component difference from CPR is only `1.657241e-2`, compared with
`5.371137e0` for Sparse LU's zero vector. CPR is therefore independently corroborated at this
decision point.

**Classification:** Y2b stays **INCONCLUSIVE**, not refuted or promoted. The partial raw-state
probe leaves inactive gas unknowns in ResSim's fixed three-variable layout. This makes ordinary
direct LU an invalid Y2b promotion oracle and turns OPM's per-iteration phase-presence/
`adaptPrimaryVariables` lifecycle into a measured prerequisite for the next behavior scope. No
Sparse-LU work, G4/G5, acceptance widening, or Y2c promotion follows from this result.

### Y2b3 design checkpoint (2026-07-14) — source lifecycle and dependency contract complete

The tracked deck is `OIL/WATER/GAS/DISGAS` without `VAPOIL`, so the bounded OPM lifecycle is the
fixed composition-switch slot tagged as either `Sg` or `Rs`. OPM applies the meaning-aware Newton
chop, then calls `adaptPrimaryVariables` after every candidate update: negative `Sg` becomes
saturated `Rs`, over-saturated `Rs` becomes `Sg=0`, and a per-cell previous-switch flag supplies
hysteresis. The next residual/Jacobian uses that adapted meaning; raw saturation storage and
endpoint-safe property evaluation remain separate concerns.

ResSim already has the fixed slot/tag representation, but live Newton freezes the tag. In the
raw-state probe, the saturated flash path can clamp a crossed `Sg` to zero before residual/property
evaluation, which is the source mechanism consistent with Y2b2c's empty gas-primary columns. The
old capture lacks state/tag history, so the next capture must confirm this per cell rather than
promoting the inference to a measured fact.

The design in `FIM_Y2B3_PRIMARY_VARIABLE_LIFECYCLE_DESIGN.md` requires an atomic in-loop tag/value
switch, OPM switch initial values and hysteresis, unchanged previous-time state, and a hard
diagnostic invariant of zero empty cell-primary columns. Transition tests and mixed-regime
derivative/structure tests must pass before regenerating the exact first-rung capture. No solver,
well, acceptance, timestep, or Legacy behavior changed at this checkpoint.

### Y2b3a checkpoint (2026-07-14) — Gate A transition lifecycle green

Implemented the deck-scoped state machine only behind native `OpmAligned` plus the existing
default-off `FIM_Y2B_RAW_SATURATION` flag. The fixed third unknown changes tag/value atomically
after the raw candidate update and before well post-processing. A Newton-local boolean per cell
tracks whether the preceding accepted candidate switched, selecting OPM's `eps=1e-5` hysteresis.
No persistent state shape, matrix dimension, Legacy path, wasm path, solver, acceptance, or
timestep behavior changed.

Focused results:

- `cargo test ... y2b3_opm_lifecycle -- --nocapture`: 5/5 pass;
- `cargo test ... fim::state::tests -- --nocapture`: 13/13 pass;
- `cargo test ... assembly_ad -- --nocapture`: 10/10 pass;
- locked `drsdt0_base_rs_cap_flashes_excess_dissolved_gas_to_free_gas`,
  `spe1_fim_first_steps_converge_without_stall`, and
  `spe1_fim_gas_injection_creates_free_gas`: pass; and
- curated FIM/shared gates pass through their preceding buckets, then stop at the known
  pre-existing `closed_system_public_step_keeps_same_water_inventory_on_both_solvers`
  `rate_history` assertion (`left=2`, `right=1`) already recorded in `TODO.md`.

No flagged convergence run was performed. Gate A's hysteresis test shows why: with
`was_switched=true`, `Sg=-5e-6` correctly remains tagged `Sg`, but `cell_props_generic` currently
floors it to zero and may erase its Jacobian dependency. Y2b3b must first add the raw
meaning-aware accumulation dependency and prove one-sided FD plus zero empty columns.

### Y2b3b checkpoint (2026-07-14) — Gate B dependency and structure green

Changed only the tagged three-phase property/accumulation lifecycle: `Sg` and `Rs` remain raw in
the scalar and AD phase-state construction, while relative permeability/capillary endpoint
extension remains separate. Two-phase and no-PVT bounded behavior is unchanged. This removes the
specific derivative flattening exposed by Gate A without adding a diagonal, changing matrix
shape, or touching wells, linear solvers, Newton acceptance, or timestep control.

Evidence:

- `cargo test ... y2b3b_ -- --nocapture`: 3/3 pass. The accumulation column matches one-sided FD
  within each meaning, including `Sg=-5e-6`; both one-cell switch directions and the mixed-regime
  gas injector have live hydrocarbon columns, finite/nonempty matrices, and successful diagnostic
  Sparse-LU factorization.
- `cargo test ... fim::assembly_ad::tests:: -- --nocapture`: 6/6 pass, including scalar residual
  parity and AD-vs-scalar-FD with reservoir and well terms.
- `cargo test ... fim::properties::tests:: -- --nocapture`: 7/7 pass;
  `cargo test ... fim::state::tests::y2b3 -- --nocapture`: 5/5 pass.
- Locked `drsdt0_base_rs_cap_flashes_excess_dissolved_gas_to_free_gas` passes; the two SPE1 locked
  cases pass in the curated FIM bucket. `bash scripts/validate-solver-coverage.sh fim` passes.
- `bash scripts/validate-solver-coverage.sh shared` passes its first three contracts and then
  stops at the known pre-existing closed-system `rate_history` assertion (`left=2`, `right=1`).

Classification remains **Y2b INCONCLUSIVE**. These are local derivative and structural oracles,
not evidence of improved OPM trajectory parity. No flagged convergence run was performed. The
next slice is Y2b3c Gate C: regenerate the exact first-rung capture with switch tracing, require
zero empty cell-primary columns, and compare backend-neutral direct/CPR corrections and full-system
reductions before running only the capped first rung.

### Y2b3c checkpoint (2026-07-14) — exact system full-rank, first rung improves

The first clean-commit behavior run on `1a6460d` made the historical retry point disappear: the
completed lifecycle accepted the full `dt=0.25` capped rung in one substep, 8 reported Newton
iterations, and zero retries. Its accepted scalar MB is `1.683337e-8`; final CNV is
`[6.695e-12,6.711e-4,2.622e-4]` and final per-family MB is
`[4.597e-12,1.648e-8,6.734e-8]`. The post-diagnostic replay trace is byte-identical to that clean
trace. This moves ResSim from the historical five-cut `dt=0.000978384825` rung and incomplete
raw-state three-cut `dt=0.00898425` rung to Flow's no-cut scale (Flow reports 7 iterations).

To preserve the predeclared oracle, the ignored native driver gained a test-only
`FIM_Y1J_DT_DAYS` override. At exactly `0.00898425`, iteration 1, it writes the same capture format
plus 300 `Y2B3-PRIMARY` trace lines tying current tag/value, derived `Sg`/`Rs`, previous-switch
state, epsilon, column index, and nonzero count together. Results:

- 141 cells are `Sg`, 159 are `Rs`, and the same 159 report a preceding switch;
- all 300 local-variable-2 columns are nonempty (minimum occupancy 2);
- matrix `904x904`, 6815 nonzeros, no empty/duplicate/non-finite/all-zero rows or columns, no
  zero-diagonal candidates; Sparse LU preparation is `factorized`;
- CPR reduction `4.911209e-7`, residual `1.368523e-5`;
- Sparse LU reduction `2.837291e-16`, residual `7.906198e-15`;
- dense LU reduction `7.935809e-16`, residual `2.211337e-14`;
- Sparse/dense maximum correction-family delta `7.285839e-15`; CPR/Sparse maximum delta
  `5.557618e-7`.

Artifact: `/tmp/ressim-y2b3c-exact-b/fim_capture_00000.txt`, SHA-256
`13f5f6aa14ae218679b866bb236293801ad81f5d75eba1110b3083f90ea1b61a`; trace SHA-256
`3cc23f85789a3be6be64a21b0eb4475265dadf0b3d5bafc6508c9afc12499224`.

Validation: Y2b3 focused tests 8/8, AD/scalar assembly tests 6/6, capture round-trip tests 3/3,
the exact release capture/replay tests, the locked DRSDT0 test, and the curated FIM bucket all
pass. The shared bucket passes its first three contracts and stops at the unchanged pre-existing
closed-system `rate_history` mismatch (`left=2`, `right=1`).

**Classification: Y2b PROMOTION CANDIDATE.** Gate C is structural-green and the bounded behavior
is materially closer to Flow. This is not final promotion: Y2c must now run the six-step target,
fresh Flow oracle, heavy case, controls, and physics validation on the committed checkpoint.

### Y2c checkpoint (2026-07-14) — validated gas improvement, control regression blocks promotion

The ignored native drivers were extended only to make the predeclared matrix reproducible: the gas
fixture accepts grid/flavor/step/dt selectors and preserves state across report steps; a water
driver selects the existing `20x20x3`, `22x22x1`, and `23x23x1` fixtures. They print accepted
substeps, Newton counts, retry classes, state closure, inventory, rates, and cumulative reporting
balance. Production behavior is unchanged.

Primary target and Flow oracle:

- candidate command: `FIM_Y1J_STEPS=6 FIM_Y2B_RAW_SATURATION=1 cargo test --release
  --manifest-path src/lib/ressim/Cargo.toml --lib repro_gas_rate_10x10x3_y1j -- --ignored
  --nocapture`;
- result: six accepted substeps total, one per `0.25`-day report step, Newton
  `8,5,4,4,4,4`, zero linear/nonlinear/mixed retries;
- fresh `scripts/opm-ressim-compare.sh --opm-only` with Flow 2026.04: six substeps,
  `7,5,4,3,4,3`, zero cuts (INFOSTEP SHA-256
  `25f1275c29b3bd95972e56ea266826ccbc0c7605d4e9512716654824625ac047`, INFOITER
  `c196dc25615be0532ed8c27776ec4fecec347c1880dbb09449888934546cda0d`);
- Legacy six-step control: 14 accepted substeps and 7 nonlinear retries;
- candidate `20x20x3` gas first step: one substep/8 Newton/zero retries, versus Legacy 2 and
  baseline `OpmAligned` 238.

Bounded water matrix (accepted substeps; retry counts in parentheses):

| fixture | Legacy | baseline `OpmAligned` | candidate |
| --- | ---: | ---: | ---: |
| `20x20x3` | 8 (0L/3N) | 24 (5L/1N) | 5 (1L/1N) |
| `22x22x1` | **4** (0L/2N) | 24 (8L/0N) | **11** (8L/0N) |
| `23x23x1` | 4 (0L/2N) | 12 (1L/0N) | 3 (1L/0N) |

The heavy `12x12x3` candidate first step accepts 7 substeps with one nonlinear retry; its Flow
oracle is one substep/11 Newton. Thus the lifecycle fixes the gas boundary mechanism but does not
complete the water/controller/linear stack.

Physics/reference checks: the six-step candidate is finite with maximum saturation-closure error
`2.220e-16`. At 1.5 days its oil/gas rates are `1.623659472e2` / `1.298927578e4`, injection is
`1.130866968e5`, and water/oil/gas inventories are `4.481397932e3`, `2.017550243e4`, and
`1.767983459e6`. A 24-step `dt=0.0625` reference differs by about `0.036%` in rates, `0.228%` in
gas inventory, `0.876%` in injection, and `0.015%` in oil inventory. These are accepted-reference
checks, not a substitute for Flow trajectory parity.

Validation: fixture checker, `git diff --check`, focused Y2b3 (8/8), AD assembly (6/6), locked
DRSDT0, Buckley-Leverett (3/3), and curated FIM (all buckets) pass. The shared bucket passes its
first three contracts and stops at the known pre-existing closed-system `rate_history` assertion
(`left=2`, `right=1`).

**Classification: VALIDATED POSITIVE, NOT PROMOTED.** The default-off flag is retained. The direct
promotion blocker is the candidate's 11 versus Legacy's 4 substeps on `22x22x1`, with eight
`linear-bad` retries. Next execute only Y2d0: capture the first such system and compare live CPR
with one independent direct replay under the backend-neutral full-system norm contract. Do not
change acceptance, timestep policy, wells, or the lifecycle while establishing that oracle.

### Y2d0 checkpoint (2026-07-14) — first bounded retry is a genuine CPR correction failure

Baseline commit: `20309969a4ba58844b00f19b0f7191577f9b3f55`. The exact capture command was:

```text
FIM_CAPTURE_DIR=/tmp/ressim-y2d0-2030996 FIM_Y2B_RAW_SATURATION=1 \
FIM_Y2C_WATER_GRID=22x22x1 FIM_Y2C_FLAVOR=opm cargo test --release \
--manifest-path src/lib/ressim/Cargo.toml --lib repro_water_pressure_y2c_control \
-- --ignored --nocapture
```

It reproduced Y2c exactly: 11 accepted substeps, Newton `3,3,3,3,3,3,4,4,4,5,3`, eight
`linear-bad` retries, and no nonlinear/mixed retries. The failure hook wrote exactly eight
artifacts. The prescribed first artifact is `/tmp/ressim-y2d0-2030996/fim_capture_00000.txt`,
SHA-256 `725cbbc2cc06f1d31ef090c7b7f11e6374ce7b70a3feaf06ccc90f18309e786b`.

The new ignored `replay_y2d0_first_bounded_control_failure` test is diagnostic-only and requires
an isolated one-file `FIM_CAPTURE_DIR`. It computes backend-neutral norms independently from the
reports, partitions the residual into reservoir and well rows, and compares correction families.
Results:

- matrix `1456x1456`, 4764 nonzeros; no empty rows/columns, duplicates, non-finite entries,
  all-zero rows, or zero-diagonal candidates; Sparse LU preparation `factorized`;
- common RHS norm `2.541597987e3`;
- CPR: finite, not converged, 30 iterations, report/recomputed residual `3.662755584e1`, reduction
  `1.441123105e-2`, reservoir residual identical, well residual zero;
- Sparse LU: finite/converged, one iteration, residual `1.409655886e-11`, reduction
  `5.546336962e-15`, reservoir `1.409653480e-11`, well `2.604409651e-14`;
- CPR/direct correction delta peaks: pressure `181.4979654`, Sw `0.3510961842`, hydrocarbon slot
  `5.58e-14`, well BHP `2.53e-14`, perforation rate `1550.930879`.

The generic backend replay independently reproduces the same CPR failure and direct success;
GMRES-ILU0 also fails (`7.031e-3` strict relative residual). The existing well-elimination lab
shows both default CPR with and without well Schur stop at 30 iterations, so Schur recovery is not
the sole cause.

An existing lab-only restriction sweep was run as a bounded clue, not a behavior verdict. On the
first artifact, `row0-schur`/`local-schur-balanced` converge at `1.257e-4`, while quasi-IMPES is
non-converged at `7.598e-3`. Across all eight artifacts, row0/local-Schur converge 4/8 and
quasi-IMPES 0/8. But this helper bypasses production well-Schur elimination and equation scaling,
and historical gas evidence favored quasi-IMPES 336/337. It is therefore an invalid basis for a
live restriction flip.

**Classification: Y2d0 CONFIRMED.** The first promotion-blocking retry is a real iterative
correction-quality gap, not matrix build/factorization, reporting, lifecycle, or nonlinear
acceptance. Next execute Y2d1 offline only: route explicit restriction variants through the real
well-Schur/scaling wrapper and require both the eight bounded artifacts and the established gas
corpus before choosing or refuting restriction mismatch.

Validation: exact release capture/replay and the generic backend/well-elimination/restriction labs
pass; capture round trips 3/3, Sparse-LU report reduction and well-Schur full-system norm contracts
pass; locked DRSDT0 and Buckley-Leverett 3/3 pass; the curated FIM bucket passes. The shared bucket
again passes its first three contracts and stops at the unchanged pre-existing closed-system
`rate_history` mismatch (`left=2`, `right=1`).

### Y2d1 checkpoint (2026-07-14) — production-faithful restriction tradeoff, no live change

Baseline commit: `e143c19d17eb416e1c7f6df9fb02de177d064d04`. The eight Y2d0 bounded artifacts
remain `/tmp/ressim-y2d0-2030996`. The historical 337-system gas corpus was not preserved and was
created before later zero-iteration/report fixes, so it is not a current reproducible gate. A
clean-head counter-corpus was regenerated with:

```text
FIM_CAPTURE_DIR=/tmp/ressim-y2d1-gas-e143c19 FIM_Y1J_GRID=20 \
FIM_Y1J_FLAVOR=opm FIM_Y1J_STEPS=1 cargo test --release \
--manifest-path src/lib/ressim/Cargo.toml --lib repro_gas_rate_10x10x3_y1j \
-- --ignored --nocapture
```

It reproduces the current baseline: 238 accepted substeps, one linear/four nonlinear retries,
minimum accepted `dt=0.00018974736`, maximum `0.0029648025`, and 81.306 s elapsed. Current capture
semantics yield five final-near-miss/failure artifacts, with SHA-256 values:

```text
d24fcb069f919a6b3240c8eb5a4b0582b10222afa647d805342f17dd8922459f  00000
8381a5ba45396e23c7c9611ea959aec17ab65ac504836341cff78d1b50f39938  00001
4764172f68162c448f7f1045cd7a01530a8add0395db96f5a879128259c83ab0  00002
2d5eece7a7002e38926e34d1416ea4ece615004cd4aa0007092ac3524fea05bc  00003
a09db1261721c835b5e54607674c4fb3bda76f108d42c396e9d8de8ab11c4fba  00004
```

Y2d1 refactored well-Schur recovery into a shared private helper and added a test-only explicit
restriction entry point. A focused synthetic test and per-real-artifact assertions prove injected
quasi-IMPES is identical to production dispatch. Every row retains production well elimination,
captured equation scaling, block-ILU0 smoother, `5e-3` strict tolerance, 30-iteration budget, and
full-system recovery. All 13 Sparse-LU references converge; all iterative report residuals equal
independently recomputed residuals, with reservoir/well partitions and direct-correction deltas.

Production-faithful aggregate (`strict`; relaxed means finite and full reduction `<1e-2`):

| restriction | bounded strict/relaxed (8) | gas strict/relaxed (5) | bounded median rel | gas median rel | bounded median direct delta |
| --- | ---: | ---: | ---: | ---: | ---: |
| row0-schur | `0/3` | `4/5` | `1.699e-2` | `2.814e-4` | `418.8` |
| sum-rows | `2/2` | `5/5` | `1.718e-2` | `1.472e-4` | `198.8` |
| diag-balanced-sum | **`8/8`** | **`3/3`** | `1.340e-3` | `4.867e-3` | **`4.23`** |
| dominant-diag-row | `0/3` | `5/5` | `1.355e-2` | `1.603e-4` | `604.1` |
| local-schur-balanced | `0/1` | `4/5` | `1.608e-2` | `2.814e-4` | `609.8` |
| quasi-impes | `0/0` | `4/4` | `1.455e-2` | **`3.646e-5`** | `508.9` |

Diagonal-balanced decisively repairs bounded (`8/8`) but loses two gas artifacts: relative
residuals `1.803e1` and `4.704e-1`. Sum-rows/dominant-diagonal preserve all gas artifacts but
leave six/eight bounded systems or all eight unresolved. Thus the old unwrapped row0 clue was not
production-faithful, and the corrected result is not authority for a restriction flip.

**Classification: DIAGNOSTIC TRADEOFF; NO UNIVERSAL RESTRICTION.** Restriction mismatch is a real
part of bounded CPR weakness, but no existing global choice clears both current corpora. No live
solver or convergence run was changed. Next Y2d2 keeps quasi-IMPES fixed and uses the same wrapper
to isolate block-ILU0 versus other existing smoothers, then 30/60/150 Krylov budgets without
combining levers.

Validation: production/injected quasi-IMPES equivalence and all four well-Schur tests pass; the
exact Y2d0 replay is unchanged; both production-faithful corpus labs, capture round trips 3/3,
Sparse-LU report reduction, locked DRSDT0, Buckley-Leverett 3/3, and the curated FIM bucket pass.
The shared bucket again passes its first three contracts and stops at the unchanged pre-existing
closed-system `rate_history` mismatch (`left=2`, `right=1`). Rustfmt and diff checks pass for all
changed Rust/docs files.

### Phase 9 (revised 2026-07-04) — component-isolation lab built and validated

User reviewed `CODEX_FIM_DIALOGUE_03.07.2026.md` (an independent parallel investigation) and an uncommitted
experimental commit (`db3bdaf`, "Experiment - not completed"), and directed a structural fix: stop testing
linear-solver hypotheses by changing the live solver and replaying full simulations — that conflates linear-
solve quality with Newton-trajectory and timestep-controller feedback, which is exactly why the session's
own row-scaling attempt (Hypothesis C above) took a full wasm-rebuild-and-replay cycle to falsify. Directed
instead: build the ability to test components — matrix builder, matrix solver, CPR — separately. Plan file
Phase 9 rewritten accordingly (see `/home/coder/.claude/plans/graceful-splashing-micali.md`).

**Step 9.0 — rejected commit `db3bdaf` (revert `55d6dcf`).** That commit added an in-situ CPR-restriction-
variant probe gated on `options.verbose`; confirmed by direct code read that the only production caller
(`step_internal_fim_impl`, `timestep.rs:849`) hardwires `verbose=false`, so the probe could never fire through
the canonical wasm diagnostic runner — dead code from the moment it landed. It also measured only one
preconditioner *application* per variant, which cannot answer the only question that matters ("would a full
solve with variant X converge where the current row0-schur restriction dead-states?"). Reverted cleanly;
`cargo build` + `fim::linear` tests (29) green after. `FIM-LINEAR-005` registry row reset to `OPEN` with a
note pointing at the offline lab as the correct vehicle; the variant math (`CprPressureRestrictionKind`,
5 variants) remains recoverable from git history for Step 9.2 continuation.

**Step 9.1 — capture harness built.** New `fim/linear/capture.rs` (`#[cfg(not(target_arch = "wasm32"))]`,
std-only text format, no new dependencies): dumps `jacobian`/`rhs`/`layout`/failure-metadata for every failed
iterative linear solve to disk, gated on the `FIM_CAPTURE_DIR` env var. Hooked at the existing linear-failure
branch in `newton.rs` (same site as `FAIL-SITE-DETAIL`, Phase 8's Step 8.1 addition). Because the whole module
is `#[cfg(not(target_arch = "wasm32"))]`, it is not merely inert but **entirely absent** from wasm builds —
confirmed via clean `bash scripts/build-wasm.sh` and a heavy-case replay bit-identical to the Phase 8 baseline
(`31` substeps, `accepts=30+3+1678`, `retries=0/12/0`, identical `real_accept_rungs`/`retry_rungs` sequence).
Locked smoke set green (3/3, 381.04s). Refreshed the stale doc comment on `repro_water_pressure_12x12x3`
(described the pre-`ffd965a` non-terminating singularity, no longer true — the case completes at 31 substeps)
and repurposed it as the capture driver; added a sibling `repro_water_pressure_23x23x1` mirroring the bounded
control case. Captured two corpora: **54 systems** from the heavy case, **13 systems** from the bounded case
(both native `--release`, `--ignored`, a few seconds each).

**Step 9.2 — offline solver lab built, both validity gates passed on both corpora.** New
`fim/linear/solver_lab.rs` (`#[cfg(test)]`, `#[ignore]`d): loads a captured corpus and runs **full solves**
through the existing `solve_linearized_system` entry point with each backend (sparse-LU, `GmresIlu0`,
`FgmresCpr`) on identical input — a new hypothesis is now one enum arm and a few-seconds rerun, no wasm
rebuild, no simulation. Both plan-mandated validity gates (asserted, not just printed) passed on both corpora:

- **Stop-condition-1 gate (assembly-level sanity):** sparse-LU reference converged on **all 54 heavy-case and
  all 13 bounded-case systems**, residual ~1e-13 to 1e-16 relative — the captured systems are genuinely
  solvable; no evidence of an assembly-level problem at these states.
- **Stop-condition-2 gate (capture fidelity):** the current `FgmresCpr` (row0-schur) path failed to converge
  on **54/54 heavy-case and 13/13 bounded-case** systems offline — it reproduces its live failure with 100%
  fidelity on both corpora (well above the ≥50% bar), confirming the capture is complete enough to trust for
  comparison.

**First substantive finding, and an important nuance between the two corpora:**
- **Heavy case (54 systems): uniform signal.** Plain `GmresIlu0` (full ILU(0), no CPR pressure-correction
  stage at all) landed 1-4 orders of magnitude closer to converged than the current `FgmresCpr` on **every
  single one** of the 54 systems (e.g. capture 00037: ILU0 relative residual `3.24e-5` vs. CPR `1.00e0`;
  capture 00053: ILU0 `3.74e-7` vs. CPR `1.00e0`). On this case, the CPR pressure-correction stage is not
  merely insufficient — adding it on top of ILU0 makes convergence uniformly and substantially *worse*.
- **Bounded case (13 systems): mixed signal, not the same pattern.** Here CPR sometimes helps a lot (capture
  00000: ILU0 rel `1.93e-3` vs. CPR `6.82e-5`, ~28x better) and sometimes hurts a lot (capture 00010: ILU0 rel
  `2.34e-3` vs. CPR `4.58e-1`, ~200x worse) — no uniform direction. **Do not over-generalize the heavy case's
  "CPR is actively harmful" finding as universal** — it is state-dependent, and the lab has now demonstrated
  that cleanly across two independent corpora in seconds, something the prior live-replay-only workflow could
  not have surfaced this precisely.

**Status:** lab validated and ready for its intended use — testing the salvaged `CprPressureRestrictionKind`
variants (`sum-rows`, `diag-balanced-sum`, `dominant-diag-row`, `local-schur-balanced`) and an OPM-style
quasi-IMPES weighting (per-cell solve of `A_ii^T w = e_pressure` from the diagonal accumulation block,
normalized) as full-solve variants against both corpora, per Step 9.2's remaining scope. Not yet implemented;
presented to the user as the next increment before proceeding.

### Restriction-variant comparison — decisive, consistent result across both corpora

Salvaged the variant math from `db3bdaf`'s history (kept out of the live path — `solve()`, the only production
entry point, hardcodes `Row0Schur` explicitly, unchanged from current behavior) and parameterized
`build_pressure_transfer_weights`/`build_block_jacobi_preconditioner`/`solve_with_cpr_fine_smoother` with a
`CprPressureRestrictionKind` enum: `Row0Schur` (current), `SummedRows`, `DiagBalancedRows`,
`DominantDiagonalRow`, `LocalSchurBalanced` (all salvaged), plus a new `QuasiImpes` matching OPM's
`getQuasiImpesWeights.hpp` (`w = A_ii^{-1}.row(pressure_index)`, normalized — reuses the block inverse already
computed for the same cell). Added a lab-only `solve_with_restriction_kind` entry point (`#[cfg(test)]`,
never reachable from production) and a new `solver_lab_compare_restriction_variants` test. All 31 pre-existing
`fim::linear` tests pass unchanged; production build has zero new warnings.

**Full-solve comparison, both captured corpora (numbers are `converged/total`, median relative residual):**

| Variant | Heavy (54 systems) | Bounded (13 systems) |
|---|---|---|
| `row0-schur` (current, live) | `0/54`, median `1.00e0` | `0/13`, median `2.61e-3` |
| `sum-rows` | `50/54`, median `3.32e-8` | `12/13`, median `2.73e-8` |
| `diag-balanced-sum` | `8/54`, median `1.00e0` | `0/13`, median `1.04e-1` |
| `dominant-diag-row` | `39/54`, median `7.11e-6` | `12/13`, median `4.42e-8` |
| `local-schur-balanced` | `0/54`, median `1.00e0` | `0/13`, median `2.60e-3` |
| `quasi-impes` | `50/54`, median `3.64e-8` | `12/13`, median `6.55e-8` |

**Reading:** the current live restriction (`row0-schur`) never converges on either corpus — consistent with
everything found earlier this session. `local-schur-balanced` (row0-schur plus normalization) doesn't fix
this either — the problem isn't scaling of the row0-schur weights, it's the row0-schur construction itself.
Three variants dramatically outperform current on **both** independent corpora: `sum-rows` and `quasi-impes`
both converge on ~92-93% of systems on each corpus; `dominant-diag-row` is strong on bounded (12/13) but
weaker on heavy (39/54). `quasi-impes` is the principled choice — it's literally OPM's own production CPR
construction, not an ad-hoc heuristic, and it matches `sum-rows`'s convergence rate almost exactly on both
corpora while being derived from the actual physics (the diagonal accumulation block) rather than treating
all rows as interchangeable.

**This is now Step 9.3-eligible evidence** (the plan's promotion bar: "convincingly wins offline on the clear
majority of captured dead-state systems, solution accurate vs the direct reference," on both independent
corpora) — presented to the user for the promotion decision rather than promoted autonomously, since Step 9.3
still requires the live control-matrix gate and Newton-trajectory feedback is a real, separate risk (stop
condition 4) that this offline lab cannot rule out by itself.

### Step 9.3 live gate result — mixed, not a clean promote or revert

User approved proceeding to Step 9.3. Changed `solve()`'s hardcoded restriction from `Row0Schur` to `QuasiImpes`
(`gmres_block_jacobi.rs`, the only production entry point) — production build clean, zero new warnings, all 31
pre-existing `fim::linear` tests pass unchanged. Full control matrix + locked smoke on the rebuilt wasm:

| Case | Baseline (`row0-schur`) | After (`quasi-impes`) |
|---|---|---|
| water-pressure 20x20x3 | `8` substeps, `0/3/0` | `8` substeps, `0/3/0` (unchanged) |
| water-pressure 22x22x1 | `4` substeps, `0/2/0` | `4` substeps, `0/2/0` (unchanged) |
| water-pressure 23x23x1 | `4` substeps, `0/2/0` | `4` substeps, `0/2/0` (unchanged) |
| gas-rate 20x20x3 | `2` substeps, `0/1/0` | `2` substeps, `0/1/0` (unchanged) |
| gas-rate 10x10x3, 6 steps | `14` total substeps | `14` total substeps (unchanged) |
| **water-pressure 12x12x3 dt=1 (heavy)** | **`31` substeps, `0/12/0`, `26.5s`** | **`26` substeps, `0/13/0`, `78.4s`** |

Locked smoke set green (3/3, 445.81s). All 5 control-matrix cases bit-identical. The heavy case (the actual
target) is genuinely mixed: substeps improved `31→26` (16% fewer, the primary metric this whole effort has
tracked) but retries went up by 1 and wall-clock nearly tripled (`26.5s→78.4s`), driven by `pc_ms` jumping from
~6-13s to `66.2s` — `quasi-impes` converges far more often per Newton iteration than the old restriction, but
each convergent solve now runs many more GMRES iterations to reach the old, very tight `1e-7`-relative target,
instead of the old restriction's fast-fail-at-30-iterations-then-cheap-direct-solve pattern.

**Decision: neither promoted nor reverted.** Presented to the user as a genuine trade-off; user's response was
to reject further isolated-lever tuning and instead directed a systematic replication of OPM's whole recipe as
one bundle (Phase 10, below) — this is exactly the item-by-item testing trap the user flagged: a real
improvement (restriction choice) tested against the wrong tolerance philosophy produces a misleading verdict.
`solve()` is currently left on `QuasiImpes` (uncommitted) pending Phase 10's bundle re-test, which will
supersede this isolated result either way.

## Phase 10 (2026-07-04) — adopt OPM's CPR recipe as a bundle, not item by item

Full context, OPM's actual shipped defaults (researched from `OPM/opm-simulators/` source), ResSim's confirmed
current state, and the Rust-ecosystem AMG constraint are in
`/home/coder/.claude/plans/graceful-splashing-micali.md` "Phase 10." Summary: OPM's `cprw` default pairs a
**loose** linear tolerance (`0.005` relative reduction, `maxiter: 20` for the CPR path) with block-ILU0 and
TrueIMPES/QuasiIMPES weighting — tested and shipped as one matched set, not tuned as independent levers. AMG is
explicitly out of scope (no wasm32-compatible pure-Rust crate exists; hand-rolling is ~1500-2000 LOC and not
needed at current benchmark scale per the existing `docs/FIM_CPR_IMPROVEMENT_PLAN.md` finding).

### Step 10.0 — tolerance/budget translation, derived from real data (not pasted from OPM)

ResSim's linear solve always starts from `x_0 = 0` (confirmed: `solution = DVector::zeros(rhs.len())` at the
top of `solve_with_cpr_fine_smoother`, `gmres_block_jacobi.rs:1290`), so `r_0 = rhs` exactly on every solve.
This means OPM's relative-reduction target `||r_k||/||r_0|| <= 0.005` translates **exactly** (not
approximately) to ResSim's own absolute-residual check as `relative_tolerance = 5e-3` (with the old
`absolute_tolerance: 1e-10` term becoming vestigial — confirmed below, not assumed).

Pulled the actual `rhs_norm` values observed across both captured corpora (54 heavy-case systems, 13
bounded-case systems, via a fresh `solver_lab_compare_backends` rerun on each): range is **`5.2e-2` to
`2.5e3`** across both corpora combined. Substituting the extremes and a mid-range sample:

| `rhs_norm` | old tolerance (`1e-10 + 1e-7·rhs`) | new tolerance (`5e-3·rhs`) | new/old ratio |
|---|---|---|---|
| `5.202e-2` (smallest observed) | `5.302e-9` | `2.601e-4` | `~49,057x` |
| `2.079e-1` | `2.089e-8` | `1.040e-3` | `~49,761x` |
| `9.140e0` | `9.141e-7` | `4.570e-2` | `~49,994x` |
| `3.026e1` (largest observed in bounded corpus) | `3.026e-6` | `1.513e-1` | `~49,998x` |

The new tolerance is consistently **~50,000x looser** than the current one across the entire observed range —
this is not a marginal adjustment, it's a wholesale change in what "converged" means for a linear solve. The
old absolute floor (`1e-10`) is confirmed vestigial at every observed scale (at the smallest `rhs_norm` it
still contributes only ~2% of the old tolerance; at larger scales it's negligible) — dropping it (or keeping a
tiny floor purely to avoid a degenerate zero-tolerance edge case) matches OPM's own pure-relative criterion.

**`max_iterations`**: per the plan, OPM's `maxiter: 20` is the coarse-AMG-solve budget *inside* one CPR
preconditioner application — a different axis from ResSim's outer FGMRES iteration budget — so it is
deliberately **not** pasted as `150 → 20`. Step 10.3's offline lab will sweep `max_iterations ∈ {150, 50, 30,
20}` at the new `5e-3` tolerance and pick the smallest budget that holds the ~92-93% convergence rate already
established for `quasi-impes`, as a corpus-derived choice.

### Step 10.2 — block-ILU0 implemented

Added `FimBlockIlu0Factors`/`factorize_block_ilu0` alongside the existing scalar `FimIlu0Factors` in
`gmres_block_jacobi.rs`: standard block-IKJ ILU(0) over the natural `cell_block_size × cell_block_size`
reservoir-cell blocks (dense `nalgebra::DMatrix` arithmetic, reusing the `try_inverse()` pattern already
established by `cell_block_inverses`), with the well-BHP/perforation tail factorized independently as a
scalar sub-ILU(0) (Option A from the plan — no cross-block fill between cell region and tail, matching OPM's
own architecture of handling wells via a separate operator rather than folding them into the block-ILU0
reservoir smoother). New `CprFineSmootherKind::BlockIlu0` variant, explicit (not a silent redefinition of
`FullIlu0`) so the offline lab can A/B scalar-vs-block ILU0 independently. Two new unit tests: exact-solve on
an uncoupled block-diagonal system, and residual-reduction (not bit-equivalence — the correct gate per the
plan) on a coupled system with a scalar tail. All 33 pre-existing `fim::linear` tests pass unchanged;
production build has zero new warnings.

### Step 10.3 — offline bundle lab: decisive, consistent win on both corpora

Extended `solver_lab.rs` with `solver_lab_compare_bundle_tolerance_iterations`, testing 8 combinations of
`(relative_tolerance, max_iterations, smoother)` — all using `QuasiImpes` (already the live restriction) —
against both captured corpora via the new `solve_with_smoother_and_restriction` lab-only entry point:

**Heavy corpus (54 systems):**

| Row | Converged | Median iters | Mean iters |
|---|---|---|---|
| baseline: `tol=1e-7 iter=150 ilu0` (= today's live config) | `50/54` | `24` | `25.1` |
| `tol=5e-3 iter=150 ilu0` | `50/54` | `18` | `27.3` |
| `tol=5e-3 iter=50/30/20 ilu0` | `50/54` | `18` | `18.4-19.9` |
| **`tol=5e-3 iter=50/30/20 block-ilu0`** | **`54/54`** | **`4`** | **`4.4`** |

**Bounded corpus (13 systems):**

| Row | Converged | Median iters | Mean iters |
|---|---|---|---|
| baseline: `tol=1e-7 iter=150 ilu0` | `12/13` | `42` | `49.7` |
| `tol=5e-3 iter=150 ilu0` | `13/13` | `12` | `12.8` |
| `tol=5e-3 iter=50/30/20 ilu0` | `13/13` | `12` | `12.8` |
| **`tol=5e-3 iter=50/30/20 block-ilu0`** | **`13/13`** | **`5`** | **`5.2`** |

**Reading, consistent across both corpora:** loosening the tolerance alone is a real but modest win (closes
some of the convergence gap, cuts iterations by roughly 2x on heavy / 3.5x on bounded) — but reducing
`max_iterations` down to `20` costs **nothing** at either corpus once the tolerance is loose (identical
converged-count and median iterations at 150/50/30/20), confirming OPM's `maxiter: 20` budget is safe here,
derived from data rather than pasted. The decisive lever is **block-ILU0**: paired with the loose tolerance
it reaches **100% convergence on both corpora** (up from 50/54 and 12/13) with median iteration counts of just
**4 and 5** — a 5-6x reduction from baseline, and most systems now converge within a single 30-iteration
restart cycle. This is exactly the kind of result the bundle approach was meant to surface: block-ILU0's
value was invisible when only tolerance/restriction were being varied one at a time.

**Winning combination for Step 10.4 live promotion**: `relative_tolerance=5e-3`, `absolute_tolerance=1e-12`
(degenerate-case guard only), `max_iterations=20`, `restart=30` (unchanged), `CprFineSmootherKind::BlockIlu0`,
`CprPressureRestrictionKind::QuasiImpes` (already live). This clears Step 10.3's gate decisively — proceeding
to Step 10.4's live control-matrix promotion test.

### Step 10.4 — live gate: decisive REGRESSION, whole bundle REVERTED

Wired the offline-winning combination live: `FimLinearSolveOptions::default()` (`mod.rs`) changed to
`relative_tolerance=5e-3`, `absolute_tolerance=1e-12`, `max_iterations=20`; `solve()`'s CPR fine-smoother
selection (`gmres_block_jacobi.rs`) changed to `CprFineSmootherKind::BlockIlu0`. Production build clean, zero
new warnings; all 33 `fim::linear` tests pass (2 label-assertion tests updated to expect `"block-ilu0"`,
matching the intentional smoother change). Full control matrix on the rebuilt wasm:

| Case | Baseline (post-Step-9.3, `quasi-impes`+old tolerance+`ilu0`) | Phase 10 bundle |
|---|---|---|
| water-pressure 20x20x3 | `8` substeps, `0/3/0` | `8` substeps, `0/3/0` (unchanged) |
| water-pressure 22x22x1 | `4` substeps, `0/2/0` | `4` substeps, `0/2/0` (unchanged) |
| water-pressure 23x23x1 | `4` substeps, `0/2/0` | `4` substeps, `0/2/0` (unchanged) |
| gas-rate 20x20x3 | `2` substeps, `0/1/0` | `2` substeps, `0/1/0` (unchanged) |
| gas-rate 10x10x3, 6 steps | `14` total substeps | `14` total substeps (unchanged) |
| **water-pressure 12x12x3 dt=1 (heavy)** | **`26` substeps, `0/13/0`, `78.4s`** | **`59` substeps, `0/16/0`, `152.5s`** |

All 5 control-matrix cases bit-identical (substep/retry counts; timing noise only). The heavy case — the
one this whole recipe was built for — **regressed decisively**: substeps more than doubled relative to the
pre-quasi-impes original baseline (`31`) and grew further from the Step 9.3 state (`26→59`); retries worsened
(`0/13/0→0/16/0`); wall-clock nearly doubled again from the already-bad Step 9.3 number (`78.4s→152.5s`),
worse than every prior state recorded this session. Note this is a genuinely distinct decision point from
Step 10.3: the offline lab's promotion bar ("wins on the clear majority of both corpora") was met decisively
(100% convergence, 5-6x fewer iterations, both corpora) — this is not a case of the offline evidence being
weak or ambiguous; it is a case of the offline lab measuring the right thing (linear-solve quality on frozen
states) while missing something real about live Newton-trajectory feedback.

**Root cause, as far as this session can characterize it without further investigation:** the offline lab
tests linear solves on *frozen* captured Jacobian/RHS pairs — it cannot see how a *less accurate* linear
correction (satisfied at `5e-3` relative residual instead of `1e-7`) feeds back into the *next* Newton
iteration's trajectory. Apparently, in this codebase's Newton/timestep-controller interaction, accepting a
much less precise linear correction leads to Newton needing measurably *more* outer iterations and hotspot
re-visits to recover — even though each individual linear solve got dramatically cheaper. This is exactly the
"Newton-trajectory feedback is a real, separate risk the offline lab cannot rule out by itself" caveat flagged
in the Phase 9 plan before Step 9.3 was even attempted — now confirmed concretely, on a larger and more
decisive scale, by Phase 10's bundle.

**Reverted per the plan's explicit no-piecemeal-retry rule**: both `mod.rs`'s tolerance/budget defaults and
`gmres_block_jacobi.rs`'s smoother selection reverted together to their Step 9.3 state (`1e-7`/`1e-10`/`150`,
`FullIlu0`) in the same pass — not picked apart to isolate "was it the tolerance or the smoother." Two test
label assertions reverted to match. Rebuilt wasm; heavy-case replay confirmed exact restoration to the Step
9.3 state (`26` substeps, `accepts=25+4+2082`, `retries=0/13/0`, ~`70-78s`). **The validated `FimBlockIlu0Factors`/
`factorize_block_ilu0` implementation, its 2 unit tests, and the offline-lab bundle-comparison infrastructure
(`solver_lab_compare_bundle_tolerance_iterations`, `solve_with_smoother_and_restriction`) are kept** — they
remain correct, tested, unused-in-production capability for any future systematic attempt at this territory;
only the *live wiring* that caused the regression was reverted.

**Per the plan's discipline: do not now try picking the bundle apart on a hunch** (e.g. "keep block-ILU0,
revert only the tolerance" or vice versa) in this same session — that is precisely the piecemeal-retry pattern
this whole phase existed to avoid, and the offline/live divergence found here means any such probe would need
a *live* control-matrix cycle to mean anything (the offline lab has now been shown not to predict this
failure mode), which is exactly the expensive, noisy, one-lever-at-a-time loop Phase 10 was designed to
replace with something better. A legitimate next attempt would come from a fresh systematic analysis of *why*
the Newton trajectory needs more outer iterations under the looser tolerance specifically in this codebase —
not from retuning constants.

### Step 10.4 (reopened) — bundle re-applied per explicit user direction; Step 10.1 done with real evidence

User pushback on the Step 10.4 revert above: reverting the whole bundle after a single live regression,
without first executing the plan's own Step 10.1 (Newton-side reconciliation), was too quick — the offline
lab's win was decisive and the plan already called for finishing the Newton-side half before judging the
bundle. Re-applied both `mod.rs` (`relative_tolerance=5e-3`, `absolute_tolerance=1e-12`, `max_iterations=20`)
and `gmres_block_jacobi.rs` (`CprFineSmootherKind::BlockIlu0` for `FgmresCpr`) to their Step-10.4 state; fixed
the 2 stale test-label assertions back to `"block-ilu0"`. All 33 `fim::linear` tests pass.

**Step 10.1 measurement (real trace data, heavy case, `--diagnostic step`):** of 16 total retries, 9 are
`post-loop: NOT CONVERGED after 20 iterations`. 8 of those 9 are dominated by `perf@1299` (a single well
perforation equation) with a strikingly consistent pattern: `mb` 3-5 orders of magnitude inside its own
tolerance (`1e-5`), `upd` well inside `update_tolerance` (`1e-3`), but scaled `res` only `2-6x` over
`residual_tolerance` (`1e-5`) — e.g. `res=2.063e-5 mb=5.257e-9 upd=9.903e-5`. The 1 outlier (`oil@430`) is a
genuine large miss (`res=4.974e-3`, ~500x over), not a near-miss. Separately: `OSC-DETECT osc_phases=0
relax=1.00` fires on every logged iteration even while `STAGNATION-ATTRIB class=real-bump` shows the scaled
residual genuinely *increasing* between consecutive iterations — the Phase 7 oscillation detector (tuned for
alternating zig-zag residuals, `d1<0.2<d2` on a 3-iteration window) does not recognize this monotonic-growth
signature as oscillation, so it never engages extra relaxation here. Conclusion: the dominant live failure
mode under the loosened bundle is a **persistent near-miss at the well/perforation row**, not the mid-loop
stagnation-bailout or oscillation-relaxation machinery over-reacting — those mechanisms are largely inert at
this specific hotspot.

**Hypothesis tested and REFUTED by a real run (`FIM-NEWTON-005`):** added a bounded, post-loop-only
near-convergence acceptance (`should_accept_near_converged_newton_final_state`: accept the final iterate when
`max_newton_iterations` is exhausted, residual is within `10x` of `residual_tolerance`, and both `mb` and
`upd` already satisfy their own unrelaxed tolerances) — deliberately scoped narrower than, and distinct from,
the previously-reverted mid-loop stagnation-acceptance widening (`FIM-NEWTON-004`: that one gave up *early*
with budget remaining; this one only fires *after* the full budget is spent). 4 focused unit tests added,
all passed; `cargo build`/`cargo test --lib fim::newton` clean (52/52). Rebuilt wasm, ran the heavy case live:
the run did **not** finish in over 8 minutes of wall-clock (vs. `152.5s` for the bundle alone, `~26s`/`78.4s`
pre-Phase-10 baselines) and was killed. **This is decisive negative evidence, not a hunch**: accepting a
marginally under-converged perforation-rate constraint as "good enough" does not fix anything — the residual
well/perforation error carries forward as the next substep's initial condition and compounds, apparently
explosively, rather than being absorbed. This is exactly the failure mode the codebase's "known-reverted lever
class" entry for widening stagnation acceptance warns about, now confirmed to also apply to a differently-
scoped (post-loop, not mid-loop) variant of the same idea. **Reverted cleanly** (constant, function, live
wiring, 4 tests) back to the pre-hypothesis state; `fim::newton` back to 48/48 passing; wasm rebuilt to the
bundle-only state (matches the Step 10.4 numbers above: heavy case still `59` substeps / `152.5s`).

**Root-cause refinement (updates the Step 10.4 "root cause" note above):** the problem is not general
Newton-mechanism over-reaction to a noisier linear correction; it is specifically that `perf@1299`'s row is
persistently and repeatedly left a small-but-real multiple over tolerance by the global relative-residual
stopping criterion, and forcing acceptance of that state cascades forward rather than resolving. A single
scalar relative-residual criterion applied to the whole system (reservoir + well + perforation rows together)
can leave a numerically small subsystem systematically under-resolved even while the overall norm is
satisfied — plausible here because perforation-row magnitudes are small relative to the dominant
pressure/saturation rows, so a large *relative* error there barely moves the *global* norm. This is consistent
with why OPM keeps wells in a separate well-operator with its own convergence handling rather than folding
them into one global norm — an architectural distinction Phase 10's own context notes ResSim does NOT
currently have (wells are explicit unknowns, matching OPM, but they share one global stopping criterion with
the reservoir rows, which does not match OPM). Registered as `FIM-NEWTON-005` (REFUTED) and `FIM-LINEAR-008`
left `OPEN` (bundle still live, no longer reverted, no verdict yet) pending a decision on whether to pursue a
per-family/per-block linear stopping criterion (a real architectural change, not yet offline-lab-validated)
or a different reconciliation path.

### Step 10.1 follow-up — per-family linear convergence built and offline-lab-tested; WEAK support, root cause redirected to Newton damping

Designed and built the per-family/per-block linear stopping criterion floated above, systematically rather
than as another live guess: `EquationScaling::family_peaks`/`within_relative_reduction` (new,
`fim/scaling.rs`, reuses the same per-equation-family scale factors Newton already computes — no new scaling
scheme invented), threaded as an opt-in `equation_scaling: Option<&EquationScaling>` parameter through
`solve_linearized_system` → `gmres_block_jacobi::solve_with_cpr_fine_smoother`'s three convergence-decision
sites (top-of-loop, restart-completion, final post-loop fallback) via a `family_ok` closure — every family's
own scaled residual must clear its own relative-reduction target, not just the whole-system norm. `None`
everywhere except the live Newton call site (which now passes `Some(&assembly.equation_scaling)`) and the
new offline-lab test, so every pre-existing synthetic-matrix test is unaffected (34/34 `fim::linear` still
pass unchanged). New `fim::scaling` unit tests (3) and 6 threading edits, all green.

Extended the capture format to `fim-capture-v2` (adds the `EquationScaling` 5 arrays) so the offline lab
could actually test this on real systems, since the old v1 corpora couldn't carry it. Also found and fixed a
real gap in the capture *trigger*: the existing hook only fires when the linear solve itself fails, but under
the loosened Phase 10 tolerance the linear solve now almost always succeeds — only the outer Newton loop
fails (exhausts `max_newton_iterations`). Added a second, unconditional capture at the final Newton iteration
(`newton.rs`, gated on `iteration + 1 == options.max_newton_iterations`) so the lab has real near-miss systems
at all. Recaptured both corpora fresh against the live bundle: heavy case 35 systems, bounded case 3 systems
(both native, `--release`, `FIM_CAPTURE_DIR` set, `repro_water_pressure_12x12x3`/`_23x23x1`).

**Offline result (new `solver_lab_compare_family_aware_convergence` test, both corpora):**

| Corpus | Non-family-aware | Family-aware | Per-family overshoot |
|---|---|---|---|
| Heavy (35 systems) | converged 35/35, mean_iters 3.1 | converged 34/35, mean_iters 3.9 | only **1/35** systems shows overshoot > 1.0 (12.14x); the rest are ≤ 1.0 |
| Bounded (3 systems) | converged 0/3 | converged 0/3 (no change) | overshoot ratios in the billions — a measurement artifact, not a real signal (see below) |

`worst_family=perforation_flow` dominates the overshoot ranking on every system in both corpora (confirms
the earlier live-trace finding that this family is the tightest-constrained), but the *magnitude* of the
effect on the heavy corpus's captured final-iteration systems is small: 34/35 already satisfy their own
family target with the existing global criterion, and enforcing the stricter per-family check costs ~25% more
mean linear iterations for essentially no convergence-rate gain (35→34/35, i.e. it can occasionally make a
system *harder* within the same iteration budget). The bounded corpus's astronomical ratios are a division-
by-near-zero artifact: at a captured near-converged Newton iteration, the *raw RHS* perforation-row entry
(the ratio's denominator basis) can itself be tiny, so any non-trivial residual — even a perfectly reasonable
one — reports as billions-of-multiples over target; both configurations fail identically (0/3) regardless of
family-awareness, meaning these 3 systems are hard for an unrelated reason, not informative for this
hypothesis. **Verdict: the offline evidence does not support per-family linear-convergence swamping as the
dominant cause of the heavy case's near-miss retries.** Per the project's own "don't rebuild wasm on hope"
discipline (Step 10.5's stop-condition philosophy), this was NOT wired live and no control-matrix rerun was
attempted — the offline gate itself already says no.

**Redirected root-cause lead, found while investigating the above:** re-examined the same `DAMP-BREAKDOWN`
trace lines from the earlier heavy-case `--diagnostic step` capture (substep 22's second retry, the one that
exhausts all 20 Newton iterations at `perf@1299`). After the first 3 iterations, the applied damping factor
alternates **perfectly** between `1.0` and `0.5` for the remaining 17 iterations (`1.0, 0.5, 1.0, 0.5, ...`) —
a textbook two-step oscillation signature. Yet `OSC-DETECT osc_phases=0 relax=1.00` fires on every one of
these iterations: the Phase 7 oscillation detector's `family_is_oscillating` test (`d1 = |f0-f2|/f0 < 0.2 <
d2 = |f0-f1|/f0`) is evaluated on the *residual* history and evidently isn't tripping even though the
*damping factor* it's supposed to help moderate is visibly bouncing. This is a more concrete, better-
supported lead than the family-aware linear criterion: the mechanism that's supposed to catch exactly this
pattern appears to have a real gap between what it measures (residual ratios) and what's actually happening
(a damping-factor bounce), rather than the linear solver leaving a family under-resolved. **Not yet
implemented or further measured — flagged as the next Step 10.1 angle, pending the user's direction**, since
another live speculative change without full measurement first is exactly the pattern this phase exists to
avoid (see the `FIM-NEWTON-005` lesson above: a plausible-sounding mechanism doesn't earn a live promotion
without decisive offline/measured support).

**Kept as validated, tested, currently-inert infrastructure** (matches the project's `FIM-AD-002` precedent —
don't delete correct, tested capability just because this specific application didn't pan out):
`EquationScaling::family_peaks`/`within_relative_reduction`, the `family_ok` threading in
`gmres_block_jacobi.rs`, the `fim-capture-v2` format, and the new offline-lab test. All are opt-in
(`Option<&EquationScaling>` defaulting to `None`) and do not change production behavior when unused.

## Phase 11 (2026-07-04) — Well Schur elimination + OSC-DETECT scope fix (`FIM-LINEAR-010`, `FIM-NEWTON-006`)

User directive after the Step 10.1 follow-up: stop testing individual mechanisms in isolation; implement OPM's
architecture *consistently* — ResSim solving well-BHP/perforation-rate as ordinary global Newton unknowns
(rather than eliminating them via Schur complement every iteration, as OPM's `StandardWellEquations` does) is
itself an inconsistency with OPM that could be masking or interacting with other fixes. Full plan in
`/home/coder/.claude/plans/graceful-splashing-micali.md`, "Phase 11".

### Step 11.1-11.2 — Well Schur elimination built, proven exact, offline-decisive

New module `fim/linear/well_schur.rs`: eliminates well-BHP + perforation-rate rows from the linear system via
Schur complement before the iterative CPR/GMRES solve, recovering them exactly afterward — pure sparse/dense
linear algebra on the existing `FimLinearBlockLayout` row partition, no well-specific physics needed. Gated
behind `FimLinearSolveOptions::eliminate_wells` (opt-in, default `false` initially). **Correctness proven**:
a synthetic cell+well+perforation system solved via elimination matches a direct full-system solve to `1e-9`
(`well_elimination_matches_direct_full_system_solve`). **Offline lab** (`solver_lab_compare_well_elimination`,
new test) on the 35 real captured heavy-case systems: convergence `34/35 → 35/35`, mean linear iterations
`3.9 → 1.1` — a decisive win, passing this project's own offline-gate bar.

### Step 11.3 — Live gate: correct and faster per-solve, but does NOT fix the target oscillation

Promoted `eliminate_wells: true` as default, rebuilt wasm. Control matrix (5 non-target cases): bit-identical
substeps/retries, no regression. **Heavy case: regressed** (`59→160` substeps, `16→40` retries,
`hotspot_newton_caps` `20→58`, wall-clock `~65s→166s`), `retry_dom` still `nonlinear-bad:perf@1299`.

**Decisive diagnostic**: pulled the `perf@1299` residual trace after elimination — **the oscillation pattern is
unchanged** (`2.137e-5 ↔ 3.419e-5`, damping still alternating `1.0/0.5`, essentially the same numbers as before
elimination). This is the key insight: Schur elimination is an *exact* reformulation of the same linear system
— it doesn't change what Newton direction gets computed, only how efficiently. It cannot fix an oscillation
that lives in the nonlinear/damping layer, because that layer sees essentially the same direction either way.
**This rules out well-architecture-as-linear-system-structure as the oscillation's cause** — the well/perforation
oscillation is a genuine nonlinear phenomenon, not an artifact of solving wells jointly with reservoir cells.

### Step 11.3 (continued) — OSC-DETECT scope was the actual gap, now measured and fixed

Pulled the exact per-family peak values during the oscillation: `water` flat at `2.733e-7`, `oil_component`
wobbling ±8% (both deeply converged, no real oscillation), `perforation_flow` swinging `2.137e-5 ↔ 3.419e-5`
(±60%, driving the global residual peak). Applying OPM's own `d1 = |f0-f2|/f0 < 0.2 < d2 = |f0-f1|/f0` test by
hand to the perforation values: `d1 ≈ 0`, `d2 ≈ 0.6` — a textbook positive match. **The OPM-ported oscillation
detector's algorithm is correct; it was just never given the row family where this oscillation actually lives**
(`PerFamilyNorms`, Phase 7, scoped to `water`/`oil_component`/`gas_component` only, with an explicit "revisit
only with evidence" note — that evidence now exists). Separately confirmed the *active* damping mechanism at
this site was never the OPM-ported detector (`osc_phases=0` throughout, confirmed again after elimination) but
`nonlinear_history_stabilization_decision` (home-grown, site-keyed): its `repeated_site_streak` requires
`current_residual >= previous_residual * 0.98` to count as weak progress, and the clean 2-period bounce makes
every *other* iteration look like a genuine improvement, resetting the streak before its cap can escalate past
the first tier (`0.5`) — it never breaks the cycle.

**Fix**: widened `PerFamilyNorms`/`detect_oscillation` (`newton.rs`) to include `well_constraint` and
`perforation_flow` (both `Option<ResidualFamilyPeak>` in the existing `ResidualFamilyDiagnostics`, mapped to
`f64::INFINITY` when absent so "no wells present" never registers as oscillating). No new mechanism — folds
into the same `compose_damping` composition point Sub-phase 7.2 already established. 4 new/updated unit tests
(`detect_oscillation_flags_perforation_flow_two_step_relative_change`,
`detect_oscillation_ignores_missing_well_and_perforation_families`, plus the 2 existing tests updated for the
wider struct). `cargo test --lib fim::newton::` 50/50 pass.

**Live result**: heavy case `160→62` substeps, `40→20` retries, `hotspot_newton_caps` `58→16`, and — the actual
target signature — `retry_dom` shifted from `nonlinear-bad:perf@1299` to `nonlinear-bad:water@387`. Pulling
that new dominant retry's trace shows a **completely different failure mode**: `DAMPING FAILED — invalid
bounded Appleyard candidate` at `cell129` — a hard damping failure, not a residual oscillation. This confirms
the fix worked for its actual target (the `perf@1299` oscillation is gone as the bottleneck) and has uncovered
a separate, pre-existing issue underneath, out of scope for this phase. Control matrix (5 non-target cases):
bit-identical, no regression. Locked smoke (`spe1_fim_first_steps_converge_without_stall`,
`spe1_fim_gas_injection_creates_free_gas`, `drsdt0_base_rs_cap_flashes_excess_dissolved_gas_to_free_gas`):
3/3 pass.

**Net verdict**: heavy case still not back to the pre-Phase-10 baseline (`26` substeps) — genuine, verified
progress (`160→62`, and the specific targeted pathology resolved), not a full fix. Both changes kept live
(well elimination for its own offline-proven correctness/efficiency value and OPM-architecture consistency;
the OSC-DETECT widening for its now-decisive live evidence). `water@387`'s "invalid bounded Appleyard
candidate" failure is the next open item — a distinct, unrelated mechanism, not a continuation of this phase's
work.

### Step 11.4 (continued) — `water@387`'s Appleyard-inflection stall investigated; fix REFUTED after three variants

Direct diagnosis (`--diagnostic step` on the post-Phase-11 heavy case): the "invalid bounded Appleyard
candidate" failure at `cell129`/`water@387` traces to `appleyard_damping_breakdown`'s fw-inflection
trust-region chop (`bind=sw_inflection@cell115` in the trace — a *different* cell than the reported hotspot,
since the binding constraint is whichever cell's raw step is tightest). The chop formula
`chop = dist_to_inflection / |dsw_signed|` degenerates to exactly `0.0` when a cell's current water
saturation sits essentially at the fw inflection point (`dist ≈ 0`), and the existing "let marginal
crossings through" overshoot check (`FW_INFLECTION_OVERSHOOT_FACTOR * dist`) cannot rescue this, since a
threshold computed from a near-zero `dist` is itself near-zero and trivially exceeded by any real step. With
`damping = 0.0` exactly, `candidate_is_valid` fails (`damping > 0.0` check), and `zero_move_appleyard_
acceptance_allows` also rejects the state (its threshold is `residual_tolerance * 1e-3 * 2.0 = 2e-8`, far
below the observed `8.569e-6` residual) — Newton has no valid move in either direction and the substep fails
outright.

Three variants of a fix were built and live-tested, each targeting the same degenerate case differently:
1. **Additive margin** (`chop = (dist + max_saturation_change) / |dsw|`): heavy case `62→263` substeps,
   `retry_dom` reverted to `perf@1299`.
2. **Max-based floor** (`chop = dist.max(max_saturation_change) / |dsw|`, chosen to be more surgical — a
   floor only changes behavior when `dist` is already below the floor, unlike an unconditional additive
   margin): **bit-identical regression to variant 1** (`263` substeps, same `accepts`/`retries` breakdown) —
   `max_saturation_change` (`0.1`) turned out to be larger than `dist` on nearly every crossing in this
   problem, so the "floor" engaged almost universally, not just in the truly-degenerate case.
3. **Skip below a degenerate-range threshold** (reuse `fw_inflection_point_sw`'s own `MIN_RANGE = 1e-4`
   convention; skip the inflection chop entirely when `dist <= 1e-4`, leaving the ordinary unconditional
   `sw_appleyard` cap as the sole saturation-change bound at that state): heavy case `62→238` substeps,
   `retry_dom` again reverted to `perf@1299`.

All three variants passed their own focused unit tests (each correctly fixes the isolated degenerate case
they were tested against) and left the 5 non-target control-matrix cases unaffected — but each caused a
substantial regression specifically on the heavy case, and each regression brought back the *already-fixed*
`perf@1299` oscillation pattern rather than any new failure mode. This is a genuine, repeated negative
signal, not a single false start: the heavy case's Newton trajectory is evidently sensitive enough to this
exact site's damping value that essentially any change here perturbs the accept/retry path into re-visiting
a different, already-addressed problem, rather than net-improving anything. **Reverted cleanly** to the
original chop formula (all three variants collapse to the same reversion); heavy case reconfirmed back to
the known-good `62` substeps / `0/20/0` retries / `hotspot_newton_caps=16` / `retry_dom=water@387` state.
Recorded as `FIM-NEWTON-007` (REFUTED) — do not re-attempt a local chop-formula change at this exact site
without new evidence explaining *why* it is this sensitive (e.g. characterizing what specifically links
cell115's damping decision to cell1299's/perf1's oscillation many iterations and possibly substeps later).

### Task #37 — the sensitivity mechanism found and fully explained; no further local fix pursued

Traced substep 59's exact retry sequence (`--diagnostic step`, same post-Phase-11 heavy case). What actually
happens is precise and deterministic, not chaotic:

- Substep 59 starts (`iteration 0`, before any update) with residual `8.569e-6` — comfortably inside the
  *ordinary* Newton tolerance (`1e-5`) but not inside the much stricter "already converged, skip the update
  entirely" entry guard, which requires `residual <= residual_tolerance * NOOP_ENTRY_EXACT_FACTOR = 1e-5 * 1e-3
  = 1e-8` (`newton.rs:2555-2556`) when the state hasn't materially changed — which it hasn't, since iteration 0
  always starts from the unmodified previous state. `8.569e-6` is `~857x` too large for this gate, so Newton
  must attempt a real update.
- That update gets `cell115`'s fw-inflection trust-region chop applied (a *different* cell from the reported
  hotspot `cell129`, since the binding constraint is whichever cell's own raw step is tightest that iteration),
  which computes exactly `damping = 0.0` — legitimate protection, not a bug on its own (a cell genuinely at the
  fw inflection point has `dist ≈ 0`, so any real step "crosses" it and the chop that would land exactly at the
  boundary is itself ≈0).
- With `damping = 0.0`, the candidate is invalid, and the rescue path (`zero_move_appleyard_acceptance_allows`)
  is *also* gated at `residual_tolerance * NOOP_ENTRY_EXACT_FACTOR * ENTRY_RESIDUAL_GUARD_FACTOR = 1e-5 * 1e-3 *
  2.0 = 2e-8` (`newton.rs:2241-2246`) — similarly far below `8.569e-6`. Newton has no valid move in either
  direction; the substep fails and the retry ladder halves `dt`.
- The residual scales down almost exactly **quadratically** with `dt` across the 5 retries this substep needed
  (`8.569e-6 → 2.142e-6 → 5.356e-7 → 1.339e-7 → 3.347e-8 → 8.368e-9`, each halving of `dt` roughly quartering
  the residual) until it finally crosses the `1e-8` entry-guard threshold and gets accepted — confirmed in the
  trace as a genuine, exact local plateau: every cell in the `cell129` neighborhood shows literally `dP=+0.00
  dSw=+0.0000` (no movement at all, at any precision shown). This is not a bug; it is the retry ladder correctly
  discovering that this region of the reservoir has reached local steady-state partway through the outer
  timestep, and mechanically shrinking `dt` until the residual is small enough to accept that as fact.

**Why the three `FIM-NEWTON-007` variants backfired, precisely**: `cell115`'s zero damping and the tight
entry/zero-move thresholds are each individually legitimate and — for the thresholds — already an explicit,
previously-litigated project decision (`FIM-NEWTON-004`: do not widen acceptance above tolerance). Both are
*single global scalars* applied to the *entire* Newton update vector, not per-cell values. Loosening either one
doesn't just let `cell115`/`cell129`'s own local plateau resolve faster — it lets the *whole* global Newton step
move further in the *same* iteration, including at whatever other site happens to be marginal that iteration
(here, `perf1`, whose oscillation `FIM-NEWTON-006` had just fixed). The "sensitivity" is not a special
`cell115`↔`perf1` relationship; it is the ordinary consequence of coupling every cell's allowed movement through
one shared scalar, which is an inherent property of scalar-damped Newton globalization generally (OPM's own
persistent relaxation scalar has the same global-coupling character) rather than something specific to this
codebase's implementation.

**No further local fix pursued at the time.** The two candidate levers (loosen the inflection chop; loosen the
entry/zero-move acceptance gates) are each already-explored territory with a clear negative verdict, and a fix
that actually addresses this without the same side effect would need per-cell or per-region damping/acceptance
criteria — a materially larger architectural change than a chop-formula tweak. Recorded as understood at the
time; `62` substeps stood as the interim heavy-case baseline (down from the pre-Phase-11 `160`, still short of
the pre-Phase-10 `26`) — **superseded by Task #38 below**, which found a materially better `k` value for the
*existing* `FW_INFLECTION_OVERSHOOT_FACTOR` mechanism rather than trying to change its formula.

### Task #38 — user pointed at prior art (`FIM-DAMP-002`/`003`); re-swept `k` under the current bundle, found a new stable point

User recollection, confirmed by a docs search: the "loosen the inflection chop" direction was already explored
in depth in April 2026, well before this session — `docs/FIM_LINEAR_SOLVER_AUDIT.md` "Fix A3" and
`docs/FIM_CHOP_WIDEN_EXPERIMENT.md`. Two directly relevant prior results:

- **`FIM-DAMP-002` (REVERTED)**: removing the inflection chop entirely — full alignment with OPM, which has no
  equivalent mechanism — was tried on a dedicated branch (`experiment/fim-no-inflection-chop`) and failed on
  *both* axes: substeps got worse (`27→162` on that era's case 3) and physics accuracy got worse (`FOPT
  3883→3019`, a genuine `-22%` loss vs. the converged fine-dt reference `3826`). The chop is doing real
  correctness work, not "OPM-inconsistent extra conservatism" — it compensates for ResSim's linear solver
  (no full AMG-CPR, unlike OPM) producing wilder raw Newton directions than OPM's.
- **`FIM-DAMP-003` (PROMOTED)**: the live `FW_INFLECTION_OVERSHOOT_FACTOR=1.2` came from a deliberate k-sweep
  (`k ∈ {1.0, 1.2, 1.5, 2.0, ∞}`) on that era's linear solver, showing a *monotonic* trend — more loosening,
  worse on both substeps and FOPT. `k=1.2` was the identified sweet spot, with an explicit retry condition:
  "retune only with k-sweep and fine-dt reference."

This session's `FIM-NEWTON-007` (three variants relaxing the chop's degenerate-`dist≈0` case) was, in
retrospect, more points along that same already-swept axis — the regression found there is a re-confirmation
of the April trend, not new information, and cross-referencing this prior art first would have saved three
live-test cycles. **The linear solver has changed substantially since the April sweep** (Phase 10's loosened
tolerance/budget/block-ILU0, Phase 11's well elimination) — `FIM-DAMP-003`'s own retry condition is satisfied,
so a fresh k-sweep under the *current* bundle is legitimate, not another attempt at the same refuted direction.

**Sweep result on the heavy case (`k`, substeps):**

| `k` | substeps | retries | `hotspot_newton_caps` | `retry_dom` |
|-----:|---------:|--------:|----------------------:|---|
| 1.0 | 248 | 51 | 105 | `perf@1299` |
| 1.1 | 32  | 13 | 8   | `water@1215` |
| 1.15 | 214 | 49 | 91  | `perf@1299` |
| 1.2 (April sweet spot, now stale) | 62 | 20 | 16 | `water@387` |
| **1.25 (chosen)** | **32** | **13** | **7** | `water@1215` |
| 1.3 | 32 | 13 | 7 | `water@1215` (identical to 1.25) |
| 1.5 | 204 | 48 | 95 | `perf@1299` |
| 2.0 | 134 | 34 | 51 | `water@819` |

**The `k`↔substep relationship is genuinely chaotic, not smooth** — `k=1.15` (214 substeps) sits *between* two
good values (`1.1`, `1.25`/`1.3`, all `32`), a hallmark of Newton-trajectory bifurcation (a retry/accept
decision flips discretely at some critical iteration depending on the exact `k`), not a tunable trend. This
means picking a value because it "looks good" in one measurement would be exactly the kind of trial-and-error
this project's discipline exists to avoid — the reason `k=1.25` is defensible is that `k=1.25` and `k=1.3`
produce **bit-identical** trajectories (same `accepts`/`retries`/`hotspot_newton_caps`/production numbers),
a genuine stable plateau, unlike the isolated single points at `1.1` or `1.2`.

**Promoted `k=1.25`** (middle of the demonstrated `[1.25, 1.3]` stable range). Full control matrix (5
non-target cases) bit-identical; locked smoke 3/3 (`spe1_fim_first_steps_converge_without_stall`,
`spe1_fim_gas_injection_creates_free_gas`, `drsdt0_base_rs_cap_flashes_excess_dissolved_gas_to_free_gas`).
Checked the new dominant retry site (`water@1215`/`cell405`) directly — same benign, already-understood
local-Sw-plateau retry-ladder mechanism from Task #37 (`DAMPING FAILED`, residual scaling quadratically with
`dt` across retries, genuine zero-movement plateau on acceptance), not a new failure mode, just occurring less
often (13 retries vs. 20) and at a different cell. Recorded as `FIM-DAMP-004`.

**Net**: heavy case now at `32` substeps — down from `160` at the Phase-11 low point, `62` at the interim
Task #37 baseline, and close to (though not exactly at) the pre-Phase-10 `26`. Production numbers (`oil`,
`inj`) stayed in the same ballpark across all tested `k` values (no gross physics breakdown at any point in
the sweep), though a proper fine-dt reference re-derivation under the current bundle (matching the April
methodology) has not been done and would be needed before treating `26` itself as the target to chase further.

### Task #38 (continued, 2026-07-06) — fine-dt FOPT reference: `k=1.25` has a real accuracy cost

Closed the gate this task's own writeup flagged as skipped: re-derived the April `FIM-DAMP-003` fine-dt
methodology (`docs/FIM_CHOP_WIDEN_EXPERIMENT.md` "case 3") under the current bundle, commit `43c6a1d` plus
the local `FW_INFLECTION_OVERSHOOT_FACTOR` edits described below (each rebuilt via `bash scripts/build-wasm.sh`
before its run):

```
node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 12x12x3 --steps 16 --dt 0.0625 --diagnostic outer --no-json
```

| Configuration | fine-dt FOPT (`oil` @ step 16, `time=1.0000d`) | vs. OPM converged (`3826.12`) |
|---|---:|---:|
| April, old (tight-tolerance) bundle, `k=1.2` (`FIM-DAMP-003`, historical) | 3826.36 | +0.01% |
| **Current bundle, `k=1.2`** (isolation run, this task) | 3845.38 | +0.50% |
| **Current bundle, `k=1.25`** (`FIM-DAMP-004`, live) | 3883.47 | +1.50% |

Full 16-step trace for `k=1.25` (the live value): steps 1-11 climb smoothly (`oil` 3349→3902), step 12 turns
over (3902.25, a small decline begins), steps 13-14 continue declining under retry fragmentation
(`retry_dom` shifts to `nonlinear-bad:water@1215`), and steps 14-16 freeze bit-identically at `oil=3883.47`/
`inj=3872.87` — confirmed as the documented benign local-plateau replay mechanism (`accepts=1+5+1018`, the
`1018` being replayed hotspot-plateau bookkeeping per the `fim-solver-debug` skill's reading guide), not a
stall bug. Both `k=1.2` and `k=1.25` isolation runs (rebuilt from a temporarily-edited constant, then
restored to `1.25` and rebuilt again to confirm bit-identical reproduction — `3883.47` reproduced exactly)
show this same tail shape; only the final magnitude differs.

**Isolation result**: rerunning the identical fine-dt command at `k=1.2` under the *unchanged* current bundle
(same tolerance/budget/block-ILU0/well-elimination) gives `3845.38`, almost 2x closer to the OPM reference
than `k=1.25`'s `3883.47`. This cleanly separates two effects:

1. The Phase 10/11 bundle itself (not touched by this row) already costs ~0.5% FOPT drift vs. April's
   validated 0.01% match — a previously unquantified, unstated cost of the tolerance-loosening/block-ILU0/
   well-elimination changes, not attributable to `k` at all.
2. `k=1.25` specifically adds a further ~1.0 percentage point of drift on top of that (`0.50%→1.50%`) — a
   real, measured accuracy cost from letting more "marginal" fw-inflection crossings through unchopped, not
   a bug or measurement artifact.

**Conclusion**: the `62→32` substep win from `FIM-DAMP-004` is real, but it is **not accuracy-neutral**. The
promotion stands (registry verdict remains `PROMOTED`, updated with this caveat) because reverting outright
would only buy back partial accuracy (`k=1.2` here is still `0.5%` off, not `0.01%`) at the cost of doubling
substeps (`32→62`) — not an unambiguous win either way. This is a genuine, open trade-off, not a settled
question; flagged to the user rather than resolved unilaterally. Candidates for a real fix, not yet
attempted: (a) determine whether the bundle-level `0.5%` drift is itself fixable (tolerance-loosening
accuracy cost was never checked against a fine-dt reference when `FIM-LINEAR-008` was promoted — this may be
the bigger, more foundational gap); (b) a `k` value between `1.1` and `1.25` that hasn't been fine-dt-checked
(the chaotic `k`↔substep relationship means this isn't a simple bisection); (c) revisit whether the inflection
chop's role should shift once/if AMG (Bundle C) lands, per the existing `docs/FIM_OPM_ALIGNMENT_STRATEGY_2026-04-26.md`
guidance that per-cell damping and dropping the chop are deferred until after AMG.

## Validation Shortlist
- Water shelf summary:
  - `node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 12x12x3 --steps 1 --dt 1 --diagnostic summary --no-json`
- Gas shelf outer replay:
  - `node scripts/fim-wasm-diagnostic.mjs --preset gas-rate --grid 10x10x3 --steps 6 --dt 0.25 --diagnostic outer --no-json`
- Over-threshold coarse probe:
  - `node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 23x23x1 --steps 1 --dt 0.25 --diagnostic step --no-json | rg -m 8 "cpr=\[|FIM retry summary|FIM step done|Newton: dt="`
- Exact-dense threshold control:
  - `node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 22x22x1 --steps 1 --dt 0.25 --diagnostic step --no-json | rg -m 8 "cpr=\[|FIM retry summary|FIM step done|Newton: dt="`
- Bounded control matrix additions (added 2026-07-02, part of the fim-solver-debug skill's routine gate set):
  - `node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 20x20x3 --steps 1 --dt 0.25 --diagnostic summary --no-json`
  - `node scripts/fim-wasm-diagnostic.mjs --preset gas-rate --grid 20x20x3 --steps 1 --dt 0.25 --diagnostic summary --no-json`

### Task #41 (2026-07-07) — Gap factor budget: heavy case, ResSim vs OPM Flow side-by-side

User directive: stop optimizing individual mechanisms; attribute the full 2-3-orders-of-magnitude
wall-clock gap to OPM Flow before designing the fix. Both sides measured on this machine, same
day, commit `468a103` (clean tree; wasm rebuilt from exactly this source).

**ResSim side** (exact command, verbatim summary):

```
node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 12x12x3 --steps 1 --dt 1 --diagnostic step --no-json
step=  1 | time=1.0000d | outer_ms=36705.8 | history+=32 | substeps=32 | accepts=31+4+1764 | retries=0/13/0 | avg_p=353.55 | oil=3893.94 | inj=3883.24 | gor=0.00 | dt=[4.003e-5,6.866e-2] | growth=hotspot-repeat | hotspot_newton_caps=7 | retry_dom=nonlinear-bad:water@1215 | fim_ms=36658.0 | lin_ms=34732.0 | pc_ms=32867.0 | retry_ms=11803.0
```

Counted from the step trace: **336 real Newton iterations** across **44 Newton solve attempts**
(31 real accepted substeps + 13 retry rungs), 12 linear-solver failures, per-Newton linear
iterations 1-4 (median 3). Wall-clock 36.9 s. A `--capillary false` rerun is bit-identical
(`oil=3893.94`, same trajectory), confirming the preset's effective Pc is nil here.

**OPM Flow side** (Flow 2025.10, `/usr/bin/flow`, default options): adapted the tracked parity
deck `origin/fim-opm-continuation-plan:opm/reference-decks/water-medium-step1/CASE.DATA` to the
heavy case (`DIMENS 12 12 3`, 432 cells, producer moved to 12 12, `TSTEP 1.0`, array counts
rescaled — pure `sed`, no physics edits). Verbatim result:

```
 Newton its=11, linearizations=12 (0.0sec), linear its= 13 (0.0sec)
Number of timesteps:             1
Simulation time:                 0.05 s
  Linear solve time:             0.03 s  (Linear setup: 0.03 s)
Overall Newton Iterations:      11   Overall Linear Iterations:      13   (Wasted: 0; 0.0%)
```

**Caveat (recorded, not hidden):** the deck is a *cost-class* reference, not a physics reference —
its FOPT (2609.5) does not match ResSim (3893.9); porosity/viscosity/relperm/Pc/wells/perm all
verified matching, so the residual parity gap is inherited from the branch's Phase-0 deck lineage
(whose own doc says numerics-parity tolerances were never stated). The April OPM converged FOPT
(3826.12) remains the physics reference. An 11-Newton/zero-cut/0.05 s cost class is robust to
this level of physics mismatch.

**The factor budget (36.9 s / 0.05 s = 738x), multiplicative decomposition:**

| Factor | OPM | ResSim | Ratio |
|---|---:|---:|---:|
| Newton iterations for the 1-day step | 11 | 336 | **30.5x** |
| Wall-clock per Newton iteration | 4.5 ms | 109 ms | **24x** |

`30.5 x 24 ≈ 730x` — the decomposition closes. Within each factor:

- **Newton-count factor (30.5x)** is nonlinear-layer architecture: 32 substeps + 13 retry rungs
  where OPM takes ONE step with zero cuts. Per solve attempt ResSim averages 7.6 iterations —
  the same order as OPM's 11 for the whole day. The multiplication comes from acceptance +
  timestep control, not from Newton being weak per-attempt. 32% of wall-clock (`retry_ms=11803`)
  is spent on discarded retry-rung work (OPM wasted: 0%).
- **Per-iteration cost factor (24x)** is almost entirely preconditioner build: `pc_ms=32867` =
  95% of `lin_ms` = **89% of total wall-clock**. ResSim rebuilds quasi-IMPES weights + block-ILU0
  + the O(n³) dense coarse inverse at every Newton iteration; OPM's default `--cpr-reuse-setup=4`
  reuses the CPR setup and fully recreates it only every 30 linear solves.

**OPM installed-binary defaults captured for the design work** (from `/usr/bin/flow --help-all`,
Flow 2025.10 — no memory-derived numbers): `tolerance-mb=1e-7` (relative to total mass in place,
relaxed `1e-6`), `tolerance-cnv=0.01` (LOCAL max saturation error, relaxed `1`),
`relaxed-max-pv-fraction=0.03` (3% of pore volume may violate CNV even during strict iterations),
`min-strict-cnv-iter=-1` (relaxed kicks in when the Newton budget is exhausted),
`tolerance-wells=1e-4`, `ds-max=0.2`, `dp-max-rel=0.3`, `newton-max-iterations=20` /
`newton-min-iterations=2`, `time-step-control=pid+newtoniteration` with
`target-newton-iterations=8`, growth `1.25`, decay `0.75`, restart factor `0.33`, max restarts
`10`; `linear-solver-reduction=0.01`, `linear-solver-max-iter=200`, `cpr-reuse-setup=4` /
`cpr-reuse-interval=30`.

Contrast with ResSim's acceptance: `scaled_residual_inf_norm` (`newton.rs:1425`) at
`residual_tolerance=1e-5` — the single worst cell/equation in the grid must individually pass
`1e-5`, roughly **1000x stricter locally than OPM's CNV `1e-2`**, with no relaxed tier, no
pore-volume exemption, and no volume-averaged criterion. The `water@1215` plateau ladders that
dominate the heavy case's retry burden are single cells sitting above `1e-5` that OPM's criteria
would have accepted many iterations earlier.

**Conclusion → design doc:** the gap splits cleanly into a nonlinear-architecture half (30x:
acceptance criteria + controller + global-scalar damping) and a per-iteration-cost half (24x:
preconditioner rebuild). Both are addressed as two coherent bundles in
`docs/FIM_BUNDLE_N_DESIGN.md` (this measurement is its motivating section). Per the standing
user directive, neither bundle is to be implemented or judged mechanism-by-mechanism against
current-architecture baselines.

### Task #43 (2026-07-07) — Bundle N step 0: port-fidelity pass against OPM source, done

Cloned `opm-simulators` at `release/2025.10/final` (commit `b8b2b9e`, the exact release of the
installed `/usr/bin/flow`) and extracted the verbatim formulas for every Bundle N item into
`docs/FIM_BUNDLE_N_DESIGN.md` §9: CNV/MB convergence (incl. the per-cell pore-volume
normalization, `B_avg` FVF weighting, and the 3%-PV relaxed-CNV rule that fires at ANY
iteration), per-cell `dsMax`/`dpMaxRel` chopping (incl. the implied `dSo = -(dSw+dSg)` term),
the `pid+newtoniteration` controller (`min(PID, iteration-target)` with damping factors
1.0/3.2 — NOT the 1.25/0.75 rates, which belong to the non-default simple controller; this
corrected a real error in the design doc's original sketch), substep failure/growth clamps
(0.33 restart factor, 3x/2x growth clamps), and linear-failure handling (`reduction ≤ 0.01`
accepted with a warning; no direct-solver fallback exists in OPM's path). Also verified
`convergence-monitoring` is default-off → excluded. Design doc updated in place; §9 is now the
implementation contract for checkpoints 1-5.

### Bundle N checkpoint 1 (2026-07-07) — inert CNV/MB measurement: criteria are NOT the direct waste; the damping stall is

Implemented the OPM CNV/MB convergence measures (design doc §9.1) as a read-only per-iteration
diagnostic in `fim/newton.rs` (`cnv_mb_from_parts` pure core + `cnv_mb_diagnostics` wrapper +
`CNV-MB` trace line; 2 focused unit tests incl. the 3%-PV-rule case). One unit adaptation,
recorded in code comments: ResSim's residual is already dt-integrated (surface m³), so OPM's
`* dt` factor is intentionally absent; also noted that ResSim's existing `build_equation_scaling`
is structurally `pv/(dt·B)` — i.e. the current scaled inf-norm is already a per-cell-B CNV times
dt, which explains task #37's "residual scales quadratically with dt" observation.

**Behavioral no-op gate (all passed, commit to follow this entry):** locked smoke 3/3; full
control matrix bit-identical (20x20x3 `8, 0/3/0`; 22x22x1 `4, 0/2/0`; 23x23x1 `4, 0/2/0`;
gas 20x20x3 `2, 0/1/0`; gas 10x10x3 x6 steady `2/step`); heavy case bit-identical
(`substeps=32 | accepts=31+4+1764 | retries=0/13/0`, `oil=3893.94`, `hotspot_newton_caps=7`).

**Measurement** (heavy case `--diagnostic step`, 44 Newton solve attempts, 358 traced
iterations):

- Iterations if OPM's criteria had decided acceptance on these same trajectories: **357 of 358 —
  no saving.** OPM's test would accept earlier in only a handful of blocks (9), later or never in
  the rest.
- **35 of 44 blocks never pass OPM's criteria at all** within their attempted iterations. Binding
  criterion: **MB(1e-7) alone in 32 blocks, both in 4, CNV alone in 0.** The "1000x looser local
  CNV" framing is measured to be a non-factor on ResSim's own trajectories — CNV is comfortably
  met whenever ResSim's trajectories settle.
- The signature (longest block, dt=0.037, 20 iterations): MB contracts `1.7e-2 → 2e-3` over 9
  iterations, drops to `1.1e-5`, then **stalls oscillating at ~2e-6 for 10 straight iterations**
  with CNV at `8e-4` (passing) and violating-PV at 0. Neither ResSim's nor OPM's criteria accept
  a stalled state; ResSim then dt-halves. Distribution across the 32 MB-blocked blocks: final MB
  ranges `1.3e-7` (1x over) to `1e-3` (10000x, blowing-up rungs), median `2.2e-6` (~22x over).

**Interpretation — this refutes the simple half of the design's §1 narrative and confirms the
bundle thesis:**

1. Porting OPM's *acceptance criteria* alone (N1) would fix nothing on this case and would
   likely regress: OPM's MB `1e-7` is effectively TIGHTER than ResSim's exit states (23 of 31
   ResSim-accepted substeps end at OPM-MB between `1.3e-7` and `~2e-6`). The criteria are not
   where the 30x Newton-count factor lives *given ResSim's current update dynamics*.
2. The waste lives in the *trajectory*: the damped-Newton stall at ~2e-6 (global damping scalar
   collapsing the update at local plateaus — task #37's mechanism) burns half of every capped
   block and forces the dt-halving ladder. OPM's per-cell chop (N2) is what lets its Newton walk
   through the same plateau to `1e-7` in 11 iterations flat.
3. Accuracy note: ResSim's accepted states sitting at `1-20x` of OPM's strict MB tolerance means
   current physics acceptance is roughly OPM-comparable — no accuracy scandal in either
   direction from the criteria themselves.

**Consequence for the build order** (design doc §6 updated): checkpoint 2 becomes N2 (per-cell
chopping) — the measured load-bearing item — with N1's acceptance flip moving after it. The
bundle's end-state gates are unchanged; this is a development-order change only, fully within
the "judge only at the end" principle.

Replay: `node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 12x12x3 --steps 1
--dt 1 --diagnostic step --no-json`, grep `CNV-MB`; analysis script inline in the session log.

### Bundle N checkpoint 2 (2026-07-07) — `OpmAligned` flag + N2 per-cell chopping live behind it

Implemented `FimNonlinearFlavor::{Legacy, OpmAligned}` on `FimNewtonOptions` (default `Legacy`)
and OPM's per-cell update chopping (`opm_per_cell_chopped_update`, design doc §9.2: per-cell
`satAlpha = dsMax/maxSatDelta` incl. the implied `dSo = -(dSw+dSg)`, Rs-meaning non-negativity
guard, `±0.3·p` relative pressure clamp; oscillation-relaxation scalar pre-multiplies the raw
update matching OPM's dampen-then-chop order). Under `OpmAligned` the Legacy update-limiting
layer (history stabilization + global Appleyard scalar + inflection chop +
`candidate_respects_update_bounds`) is bypassed; everything else (acceptance, entry guards,
retry ladder, controller) is still Legacy at this checkpoint. Plumbing:
`setFimOpmAlignedNonlinear` wasm setter, `--opm-aligned` runner flag. 4 focused unit tests
(implied-So chop, per-cell independence, relative dp clamp, Rs guard, relax-then-chop order).

**No-op gate (default Legacy), all passed:** locked smoke 3/3; control matrix + heavy case
bit-identical (heavy: `substeps=32 | accepts=31+4+1764 | retries=0/13/0`, `oil=3893.94`,
`hotspot_newton_caps=7`).

**Informational first `--opm-aligned` heavy-case run** (NOT a gate — intermediate bundle state
under Legacy acceptance/controller, judged only at bundle end per the design principle):

```
node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 12x12x3 --steps 1 --dt 1 --opm-aligned --diagnostic step --no-json
step= 1 | substeps=226 | accepts=224+5+1568 | retries=0/48/0 | oil=3804.38 | hotspot_newton_caps=132 | retry_dom=nonlinear-bad:perf@1299   (141.5 s)
```

End-to-end worse under Legacy gates, exactly as the bundle thesis predicts for a mismatched
intermediate — but the CNV-MB probe (checkpoint 1, still live) shows the *Newton quality
underneath* moved decisively in the right direction:

| CNV-MB verdict per Newton solve attempt | Legacy damping (44 blocks) | Per-cell chop (272 blocks) |
|---|---:|---:|
| OPM-acceptable mid-solve (strict/pv-relaxed) | 9 | 7 |
| OPM-acceptable at exhaustion (relaxed MB `1e-6` / CNV tier) | 12 | **251** |
| Truly failing under full OPM rules | 23 (52%) | **14 (5%)** |
| Median final MB in non-accepted blocks | `2.2e-6` | **`2.9e-7`** |

The MB stall that checkpoint 1 identified as the binding failure (damped Newton frozen at
`~2e-6`) is gone — per-cell chopping brings 95% of solve attempts to a state OPM's full
acceptance rules would take (vs 48% before). The 226-substep fragmentation is now almost
entirely the *Legacy acceptance layer* (inf-norm `1e-5` + entry guards) rejecting
OPM-acceptable states, plus the Legacy hotspot controller reacting to those rejections.
Checkpoints 3-4 (N1 acceptance incl. relaxed tiers + N5 linear handling, then N3 controller)
are precisely the harvest step. Caveat recorded: "acceptable at exhaustion" means ~20-iteration
substeps; the N3 controller's 8-iteration target should settle on fewer, larger substeps —
end metrics (§5) remain the only judgment.

### Bundle N checkpoint 3 (2026-07-07) — N1 acceptance criteria + N5 linear handling live behind `OpmAligned`

Implemented, under `OpmAligned` only: (N1) the per-iteration entry check now decides
acceptance purely via `opm_conv.would_accept` (design doc §9.1's CNV/MB, including the
final-iteration relaxed tiers via a new `relax_final_iteration: bool` param on
`cnv_mb_from_parts`/`cnv_mb_diagnostics`), gated on `iteration >= 1` (OPM's
`newton-min-iterations=2` translated to this loop's 0-indexing); (N5) the linear-solve
failure branch is replaced entirely — no direct-LU fallback ladder, no
dead-state/restart-stagnation/zero-move bypass bookkeeping; a solve is used as-is if
converged, accepted with a trace if it achieved OPM's relaxed reduction (`< 0.01` relative to
`rhs_norm`, both already present on `FimLinearFailureDiagnostics`), else the Newton iteration
fails immediately (returns, no rescue) exactly like OPM's `NumericalProblem` throw. 1 new unit
test (`cnv_mb_relax_final_iteration_applies_relaxed_tiers_unconditionally`).

**A real bug was found and fixed during this checkpoint's own gating, before any live-run
measurement was trusted** (worth recording since it nearly produced a false read): two
*additional* Legacy-only acceptance shortcuts inside the loop were not yet gated on
`opm_aligned` — a mid-iteration "raw update is already tiny" check (`update_tolerance` +
`ENTRY_RESIDUAL_GUARD_FACTOR`, no OPM analog) and the zero-move-Appleyard rescue on an invalid
candidate — plus the post-loop exhaustion check (which must use OPM's final-iteration-relaxed
CNV/MB, not Legacy's strict tolerances, since it genuinely corresponds to OPM's
`iteration == maxIter` case). Caught because a first small-case (`22x22x1`) test run showed
zero `OPM-CONVERGED`/`LINEAR-ACCEPT`/`LINEAR FAILED` trace lines fire across 37 iterations —
i.e. the new mechanisms were silently never reached, exactly the kind of "looks fine, isn't"
result the project's own baseline discipline exists to catch before trusting a number. Fixed
all three sites; re-ran the same small case and confirmed `OPM-CONVERGED` now fires exactly
once per accepted substep.

**No-op gate (default Legacy), all passed:** locked smoke 3/3; full control matrix + heavy
case bit-identical to the recorded baseline (heavy: `substeps=32 | accepts=31+4+1764 |
retries=0/13/0`, `oil=3893.94`, `hotspot_newton_caps=7`).

**Informational `--opm-aligned` runs (not gates — intermediate bundle state, N3 controller
still Legacy):**

- `22x22x1` (bounded control-matrix case, 484 cells, comparable size to the heavy target):
  `substeps=14 | retries=0/7/0 | hotspot_newton_caps=9`, 15.8s — vs Legacy's `substeps=4 |
  retries=0/2/0` at well under 1s. Worse, as expected for a mismatched intermediate: the
  Legacy retry ladder (hotspot-repeat cooldown, retry factor selection) was tuned against
  Legacy's own acceptance/damping behavior and doesn't yet know how to drive dt for the new
  per-cell-chopped, CNV/MB-accepted trajectory efficiently.
- Heavy case (`12x12x3`, the actual target): **timed out past 280s** (previously 141s under
  checkpoint 2's chop-only state). Not investigated further live — burning more wall-clock on
  a known-intermediate, known-mismatched state contradicts the bundle's own "judge only at the
  end" principle, and `MAX_SUBSTEPS=100_000` means this is a slow, not infinite, run (a
  genuine safety-valve ceiling, not evidence of a hang). The retry ladder is very likely
  driving dt down repeatedly against `water@1215`'s local plateau without an N3-shaped
  mechanism to recognize "OPM would already call this converged" and stop shrinking — exactly
  the gap checkpoint 4 (N3, the `pid+newtoniteration` controller) closes.

**Consequence for the build order:** checkpoint 4 (N3, timestep controller) is now the clear
next step — it is very likely load-bearing for making the heavy case tractable again, not an
optional polish item, since the Legacy retry ladder is now actively working against the
already-fixed N1/N2/N5 mechanics rather than neutral to them.

### Bundle N checkpoint 4 (2026-07-07) — N3 timestep controller live behind `OpmAligned`

Implemented, under `OpmAligned` only (design doc §9.3/9.4): OPM's `pid+newtoniteration`
controller (`opm_accepted_step_growth_decision` = `min(dt_pid, dt_iter)` then the two growth
ceilings — always `solver-max-growth=3.0`, further `solver-growth-factor=2.0` if this substep
needed any retries), backed by a new `opm_relative_change` (OPM's `BlackoilModel::relativeChange()`
— sum-of-squares of pressure/saturation deltas between consecutive ACCEPTED substep states,
normalized by the new state's own sum-of-squares, implied `So` included) feeding a 3-value
rolling error history reset each outer step. On retry, the flat `solver-restart-factor=0.33`
replaces both ResSim's failure-classified `retry_factor` and the repeated-hotspot acceleration
on top of it. The Legacy cooldown/gas-carryover trial-dt clamps and the "retry-hold" growth
override are skipped entirely for `OpmAligned` (no OPM analog); their bookkeeping keeps
running unconditionally (harmless — nothing reads it under `OpmAligned`). 9 new unit tests
(`opm_relative_change_*`, `opm_pid_dt_*`, `opm_iteration_count_dt_*`,
`opm_accepted_step_growth_decision_*`).

**Pre-existing, unrelated test failures found and ruled out before trusting the smoke gate**:
`fim::timestep::tests::changing_hotspot_resets_extra_growth_cooldown_budget`,
`repeated_same_hotspot_extends_growth_cooldown_budget`,
`fim_enabled_step_advances_time_and_records_history_for_closed_system` fail on a clean checkout
of commit `41d45f2` (checkpoint 3, before any of this checkpoint's edits) — confirmed via
`git stash` + rerun. Unrelated to Bundle N; logged in `TODO.md`, not investigated further here.

**No-op gate (default Legacy), all passed:** locked smoke 3/3; full control matrix + heavy case
bit-identical to baseline (heavy: `substeps=32 | accepts=31+4+1764 | retries=0/13/0`,
`oil=3893.94`, `hotspot_newton_caps=7`).

**Informational `--opm-aligned` run (not a gate — full end-metric evaluation is §5, after N4):**

`22x22x1` (same case tracked at checkpoints 2/3 for a consistent trend):

| Checkpoint | substeps | retries | attempts |
|---|---:|---:|---:|
| Legacy | 4 | 2 | 6 |
| 2 (chop only) | — (not separately run) | — | — |
| 3 (chop + N1 + N5) | 14 | 7 | 21 |
| 4 (+ N3 controller) | 20 | 12 | 32 |

**Honest finding: N3 alone made this small case worse, not better.** Growth-decision trace
breakdown across the 20 accepted substeps: 7 at the `3.0` ceiling (clean accepts), 11 at the
`2.0` post-retry ceiling (i.e. the large majority of substeps needed at least one retry before
accepting), 2 at smaller PID/iteration-bound values. The retry_dom stays pinned at the same
site (`oil@415`) throughout. The most likely explanation: this case's stubborn site needs a
*more aggressive* dt cut than OPM's flat `0.33` to get past — exactly what Legacy's
repeated-hotspot-acceleration (shrinking to `0.2` after repeats) was tuned to do, and which N3
deliberately does not replicate (OPM's own retry backoff really is failure-class- and
site-agnostic). This is a genuine, not-yet-resolved open question the design's checkpoint list
did not anticipate: N3's simplicity may trade away a real capability Legacy had for navigating
specific repeated-failure sites. Not chased further live (heavy case not re-attempted this
checkpoint either, for the same "don't chase intermediate mismatches" reason as checkpoint 3) —
recorded honestly rather than declared a win.

**Consequence for the build order**: proceed to N4 (mechanism deletion) as planned — deleting
the compensating mechanisms is what actually tests whether the *combined* bundle (not each
piece in isolation against a Legacy-shaped baseline) resolves the heavy case. If the full
bundle's end-metric evaluation (§5) shows the retry-navigation gap is real and material, the
recorded fallback is revisiting N3's retry-factor choice specifically (e.g., OPM's own
`solver-max-restarts`-scaled backoff, or accepting this as a genuine, small, permanent
trade-off versus OPM if the alternative — reintroducing site memory — reopens the
architectural inconsistency this whole bundle exists to remove).

### Bundle N checkpoint 5 (2026-07-07) — N4 mechanism deletion sweep: the forensic prediction holds

Deleted, under `OpmAligned` only, exactly the three mechanisms checkpoint 4's log forensics
identified as having no OPM analog and actively causing the observed regressions:

1. **`candidate_materially_changed` dropped from `candidate_is_valid`** (the dominant fix,
   confirmed by checkpoint 4's forensics: 11 of 12 failures on the tracked small case were
   this exact exit firing on a near-zero-move update). OPM has no "was the raw update
   materially small" validity check at all — a near-zero update is still a normal Newton step;
   the loop keeps iterating and the entry check (or post-loop exhaustion) decides.
2. **Residual-stagnation bailout** (`stagnation_count >= 3` → accept-or-bail, both Legacy-scaled)
   gated behind `!opm_aligned`. OPM never inspects residual *trend* mid-solve, only its
   absolute value against CNV/MB at each entry check.
3. **Preemptive direct-solve bypass ladder** (`any_preexisting_bypass`, gating dead-state/
   restart-stagnation/zero-move/repeated-zero-move flags) gated behind `!opm_aligned` as a
   whole, closing a latent gap checkpoint 3 didn't fully cover: `repeated_zero_move_direct_bypass`
   doesn't depend on `used_fallback` (unlike the other three, already inert for `opm_aligned`
   since `used_fallback` never becomes true under N5) and could still have forced a direct
   solve ahead of any FGMRES-CPR attempt, silently reintroducing exactly the "no direct-solve
   fallback in the loop" violation N5 (checkpoint 3) was built to remove.

`repeated_hotspot_streak` checked and confirmed already safe (only read inside the
already-`opm_aligned`-gated `nonlinear_history_stabilization_decision` call from checkpoint 3).

**No-op gate (default Legacy), all passed:** locked smoke 3/3; full control matrix + heavy case
bit-identical to baseline (heavy: `substeps=32 | accepts=31+4+1764 | retries=0/13/0`,
`oil=3893.94`, `hotspot_newton_caps=7`).

**Informational `--opm-aligned` runs — the forensic prediction confirmed directly:**

`22x22x1` (the case tracked since checkpoint 2, full trend):

| Checkpoint | substeps | retries | attempts | DAMPING FAILED |
|---|---:|---:|---:|---:|
| Legacy | 4 | 2 | 6 | n/a |
| 3 (chop + N1 + N5) | 14 | 7 | 21 | (not counted) |
| 4 (+ N3 controller) | 20 | 12 | 32 | 11 |
| **5 (+ N4 sweep)** | **12** | **1** | **13** | **0** |

`DAMPING FAILED` dropped to exactly 0 as predicted; `post-loop CONVERGED` still never fires
(0) but `post-loop NOT CONVERGED` is now only 1 (a single genuine end-of-budget failure,
`retry_dom` shifted from the stubborn `oil@415` to `water@0`) — the checkpoint-4 diagnosis was
correct and the fix resolved it directly, not incidentally. 13 attempts is close to Legacy's 6
and a completely different order of magnitude from checkpoint 4's 32.

**Two honest findings that are NOT yet resolved:**

- **Heavy case (`12x12x3`) still times out** — attempted at both `--diagnostic step` (280s) and
  the cheaper `--diagnostic summary` (400s), both killed with zero output produced. The heavy
  case's dominant site (`water@1215`, the genuine local-saturation-plateau from Task #37) is a
  different pathology class from the small case's `oil@415`/`water@0` sites; N4's fix (aimed at
  a spurious VALIDITY exit) does not obviously address a genuine physical plateau. Not
  chased further live this checkpoint — still consistent with "the real test is §5," but this
  is the first checkpoint where a *comparable* case (not just a differently-shaped small case)
  showed a real, measured win, so the heavy case's continued intractability is now a specific,
  named open risk rather than a generic "intermediate state" excuse.
- **A third case (`23x23x1`, 529 cells, part of the control matrix) surfaced a DIFFERENT
  failure mode**: `substeps=26 | retries=9/0/0` (all `linear-bad`, not `nonlinear-bad`) at
  `retry_dom=linear-bad:oil@361` — fast in wall-clock (3s, this case's linear systems are cheap)
  but structurally worse than Legacy's `4/0/2/0`. This is the first sign that N5's linear-failure
  handling (accept if reduction `<0.01`, else fail outright, no rescue) may itself be a material
  cost on some cases, distinct from the nonlinear-layer issues N1-N4 have been addressing. Not
  investigated further this checkpoint — flagged for the N4/end-metric evaluation stage.

**Consequence for the build order:** N4's sweep list (design doc, the remaining "site-keyed
history stabilization remnants"/"plateau-replay bookkeeping"/"retry-family classification as a
control input" items) is not yet fully exhausted — `history_stabilization` is already `None`
under `opm_aligned` (checkpoint 3) and plateau-replay is Legacy-only bookkeeping already unread,
but the newly-surfaced `linear-bad` finding on `23x23x1` suggests N5 itself may need a second
look before §5's end-metric evaluation, not just N1-N4's mechanisms. Recommend one more
targeted pass at the `linear-bad` finding before declaring N4 complete and moving to §5.

### Bundle N checkpoint 6 (2026-07-07) — N5 bug fix: wrong residual field, not a design gap

Investigated checkpoint 5's open item (`23x23x1`'s `linear-bad` retries) directly. Root cause:
N5's reduction check (`docs/FIM_BUNDLE_N_DESIGN.md` §9.5) used
`failure.outer_residual_norm / failure.rhs_norm`, but `outer_residual_norm` is computed at the
TOP of `gmres_block_jacobi.rs`'s restart loop from the last COMMITTED solution — on a solve
that never converges, this can stay pinned at the seed value (`rhs_norm`, i.e. the residual at
`x_0=0`) even when later restarts produced a materially better, already-*returned* candidate.
Confirmed directly from the `23x23x1` trace: every one of the 9 `LINEAR FAILED` lines reported
`reduction=1.000e0` regardless of wildly different residual magnitudes (`2.542e3` down to
`1.696e-4`) — a dead giveaway, not a coincidence. The correct quantity (matching Dune ISTL's own
`result.reduction`, computed from the solution actually returned, not an intermediate
diagnostic) is `FimLinearSolveReport::final_residual_norm`, which `gmres_block_jacobi.rs`
already sets correctly to the candidate's true residual at the max-iterations return site.
One-line fix: read `linear_report.final_residual_norm` instead of
`failure.outer_residual_norm`.

**No-op gate (default Legacy):** trivially preserved — this code path only executes under
`opm_aligned`; re-ran the full control matrix + heavy case + locked smoke 3/3 anyway (all
bit-identical to baseline) since the fix touched a function called from the shared linear
report handling.

**Informational re-runs, dramatic and directly attributable improvement:**

| Case | Before fix | After fix |
|---|---|---|
| `23x23x1` | `substeps=26, retries=9/0/0` (35 attempts), all `linear-bad`, `reduction=1.000e0` always | `substeps=12, retries=1/0/0` (13 attempts) — `LINEAR-ACCEPT relaxed reduction=9.928e-3` fires once (correctly, just under the `0.01` bar), one genuine `LINEAR FAILED` remains (`reduction=1.520e-2`) |
| `22x22x1` | `substeps=12, retries=0/1/0` | unchanged (its one remaining retry was already `nonlinear-bad`, untouched by this fix) |
| Heavy (`12x12x3`) | times out >280s | **still times out >280s** (tried again after the fix) — its dominant `water@1215` failure is `nonlinear-bad` (Task #37's genuine physical plateau), a different pathology class this fix does not touch |

**Consequence:** N5 is now correctly implemented per design, not just "inert by default and
plausible." Two of three tracked comparable-size cases (`22x22x1`, `23x23x1`) are now close to
or better than Legacy in attempt count. The heavy case remains the one unresolved item, and its
failure class (`nonlinear-bad`, not `linear-bad`) means this specific fix class is exhausted for
it — the next lead, if pursued, would need to look at the `nonlinear-bad` retry path itself
(effectively re-opening the N2/chop or N1/acceptance question specifically for that site,
which the design's own build order already treats as the terrain N1-N4 were meant to cover).
Recommend proceeding to the §5 end-metric evaluation next; further live probing of the heavy
case's `nonlinear-bad` failures without a fresh forensic angle risks repeating the same
"chase an intermediate state" pattern already flagged as unproductive at checkpoints 3-4.

### Bundle N §5 end-metric evaluation (2026-07-09) — heavy case FAILS decisively; root cause identified

Ran the heavy case (`water-pressure 12x12x3`, `dt=1`) natively in `--release` mode under
`OpmAligned` (new test `repro_water_pressure_12x12x3_opm_aligned`, added specifically because
the wasm diagnostic runner's I/O buffering made this case's timeout at checkpoints 3-6
inconclusive — the previous "times out at 280-400s" data points were never resolved to a real
number). Result, verbatim (`176m25s` wall-clock):

```
FIM step done: 18002 substeps, advanced 1.000000 of 1.000000 days
FIM retry summary: linear-bad=7 nonlinear-bad=2 mixed=0
```

**18,002 substeps** — vs Legacy's `32`, vs the §5 gate's `≤35` Newton-iteration target. This is
not a "close but not quite" result; it is 2-3 orders of magnitude over every efficiency gate in
§5. Per §5's own rule ("any gate failing → the bundle as a whole is reworked or reverted"),
**Bundle N as currently implemented does not promote.**

**Root cause identified — a specific, narrow architectural gap, not a diffuse problem across
N1-N5.** The trace tail shows the run entering a compounding dt-collapse: consecutive accepted
substeps with `max_dSat=0.0000 max_dP=0.00` (essentially zero physical change — the reservoir
has reached steady state, injector/producer balanced) but `iters=20` (hit the Newton cap) and
`growth=0.400` — repeating substep after substep. The detail trace pinpoints the cause:
`perf1 well1 ... bhp=100.000 frozen_bhp=100.000 ... dq=1.802e-1` — a producer pinned exactly at
its BHP limit, its rate-vs-BHP complementarity residual not fully resolved. This residual does
not shrink with dt (it is a discrete control-mode condition, not a smooth PDE term), so a
smaller substep does not make it converge faster — yet `opm_iteration_count_dt` (checkpoint 4)
applies its full penalty regardless: `its=20 > target=8 → dt_iter = dt/(1+(20-8)/8*1.0) = 0.4·dt`.
Because the SAME well-pinned state recurs across many consecutive substeps (nothing about it
changes as dt shrinks), the `0.4×` penalty compounds: `0.4^N → 0` rapidly, and no ordinary
substep is "lucky" enough to escape since the underlying cause is structural, not transient.

**Verified directly against the OPM source** (not assumed) that this is a genuine architecture
mismatch, not a formula bug: OPM's well equations are resolved via a **dedicated inner
iteration loop** — `WellInterface::iterateWellEquations` /
`StandardWell::iterateWellEqWithControl` / `iterateWellEqWithSwitching`
(`opm/simulators/wells/{WellInterface,StandardWell}.hpp`), called *within* a single outer
reservoir Newton iteration. OPM's `total_newton_iterations` (the exact quantity fed to
`computeTimeStepSize`, confirmed at `BlackoilModel_impl.hpp:270` and
`AdaptiveTimeStepping_impl.hpp:790` `getNumIterations_`) counts only the **outer reservoir**
iterations — well-control-switching cost is resolved inside that inner loop and is invisible
to the timestep controller. ResSim's FIM solver has no such two-level split: it is one flat
Newton loop over reservoir + well + perforation unknowns together (well-BHP/perforation-rate
rows are Schur-eliminated at the *linear* level per Phase 11, `FIM-LINEAR-010`, but not at the
*nonlinear* level — the outer Newton loop still iterates until the recovered well variables
also satisfy nonlinear convergence). Porting OPM's iteration-count growth formula literally,
using ResSim's own combined count, therefore punishes well-control-switching cost as if it were
"physics moving too fast" — a category error that OPM's own architecture never exposes because
the two costs are structurally separated there.

**This is exactly "Hypothesis A" from the original Phase 8/9 well-coupling investigation**
(`docs/FIM_CONVERGENCE_ARCHIVE_*`), independently re-derived here from a completely different
angle (a timestep-controller pathology instead of a linear-solver pathology) — a second,
convergent line of evidence that ResSim's flat well/reservoir coupling, not any single
mechanism tuning, is the real remaining architectural gap to OPM.

**What this does and does not indict:**
- N1 (acceptance), N2 (chopping), N4 (mechanism deletion), and N5 (once the checkpoint-6 bug
  was fixed) each showed real, measured, positive results in isolation on the `22x22x1` and
  `23x23x1` cases (checkpoints 5-6). This finding does not undo those.
- N3's specific formula — using the *combined* reservoir+well iteration count — is the
  identified defect. It is a narrow, well-understood gap, not a reason to distrust the whole
  bundle's design.
- The heavy case is disproportionately exposed to this gap because its geometry (a single
  producer/injector pair) reaches a well-pinned steady state partway through the 1-day step and
  stays there — exactly the condition that triggers the compounding shrink. Cases without a
  well hitting a hard control limit (most of the control matrix) would not exhibit this at all,
  which is consistent with `22x22x1`/`23x23x1` both showing genuine improvements.

**Recommendation:** do not promote Bundle N in its current form. Two concrete, scoped remediation
paths, in order of preference:
1. **Decouple the well/perforation iteration count from N3's growth formula** — track reservoir-
   cell Newton iterations separately from well/perforation-driven retries within the same
   substep, and feed only the reservoir-cell count to `opm_iteration_count_dt`. This is a
   bounded, well-scoped fix consistent with Bundle N's existing architecture (no new nonlinear
   well layer needed) and directly targets the identified mechanism.
2. **Build a genuine nested well-equation solve** (matching OPM's `iterateWellEquations`) —
   correct architecturally, but a materially larger undertaking, effectively a new sub-phase of
   its own; not scoped for this bundle.
Path 1 is recommended as the next step: cheap to try, directly targets the confirmed mechanism,
and testable on the same heavy-case repro without another 176-minute wait if a bounded substep
cap is added to the test for iteration purposes.

### Bundle N §5 follow-up (2026-07-09) — well-BHP update chop implemented, heavy-case rerun pending

Implemented the recommended fix from the §5 evaluation: OPM's well-BHP update clamp
(`--dbhp-max-rel`, default `1.0`), verified from `StandardWellPrimaryVariables.cpp::updateNewton`
at the pinned tag (not invented): `dBHP_limited = sign(dBHP) * min(|dBHP|, |bhp_current| *
dbhp_max_rel)`, then floored so the next BHP stays `>= 1 bar` (OPM's own
`bhp_lower_limit = 1 bar - 1 Pa`, simplified to `1.0` here). Added to `opm_per_cell_chopped_update`
alongside the existing per-cell reservoir chop. Perforation-rate deltas remain deliberately
unchopped — confirmed directly from the same OPM source that `WQTotal` (well rate) has no
magnitude clamp at all, only a post-hoc sign-consistency check (injector can't produce, producer
can't inject) — so adding a rate clamp would have been inventing a limit OPM itself doesn't have.

**Note on the originally-proposed "decouple well iteration count from N3's growth formula" fix**:
re-examined before implementing and found it would be a no-op. N1's acceptance check
(`opm_conv.would_accept`) already excludes well/perforation rows entirely (matching OPM's
`getReservoirConvergence`), and under `OpmAligned` it is the *only* path to acceptance
(checkpoint 5 removed every other exit) — so `report.newton_iterations` fed to N3's growth
formula already reflects "how long reservoir-only convergence took," by construction. There was
no well-iteration count mixed into it to decouple. The trace instead showed the reservoir's own
MB residual genuinely stalling (`stagnation_count=17`) while the well/perforation family
dominated the residual mix — consistent with an unchopped, oscillating well update perturbing
the coupled linear solve and dragging out the reservoir's own convergence, not "extra iterations
tacked on for the well's sake." This is why the BHP chop (not an iteration-count change) was
implemented instead.

**Gates so far:** 3 new unit tests (`opm_per_cell_chop_clamps_well_bhp_relative_when_increasing`,
`_well_bhp_within_cap_is_untouched`, `_well_bhp_floors_above_lower_limit`); locked smoke 3/3;
full control matrix + heavy case bit-identical under default Legacy (no-op preserved). `22x22x1`
and `23x23x1` under `--opm-aligned` are unchanged (`12/1` each, both previously used to validate
checkpoints 5-6) — expected, since neither case has a well pinned at its BHP limit.

**Heavy-case rerun**: in progress (native `--release`, no-trace variant, background). Given the
previous full run took 176 minutes, this is the actual test of whether the fix addresses the
identified mechanism — result to be appended once available, not assumed from the mechanism
analysis alone.

### Bundle N §5 follow-up — trace-overhead isolation result (2026-07-09)

The no-trace native repro (old/unfixed code, run concurrently with the BHP-chop fix's own
confirmation run below — the two competed for CPU, inflating this run's wall-clock to
`304m59s` real / `178m23s` user) confirms: **`accepted_substeps=18002`, exactly matching** the
original `step_with_diagnostics` run. Tracing overhead was not inflating the substep count —
the `18,002`-substep pathology is a genuine solver/controller behavior, not a diagnostics
artifact. User CPU time (`178m23s`) is also close to the original traced run's wall-clock
(`176m25s`), confirming the `fim_trace!` macro's unconditional `format!()`/detail-computation
calls (not the trace-string storage) account for most of the tracing-adjacent cost, as
suspected — but that cost is small next to the substep count itself, which is the real problem.

### Bundle N §5 follow-up — well-BHP chop fix REFUTED (2026-07-09)

Result, verbatim (native `--release`, no-trace, `298m13s` wall-clock running solo):

```
accepted_substeps=18002 advanced_dt=1.000000/1.000000 linear_bad=7 nonlinear_bad=2 mixed=0
min_dt=Some(1.0868125188689959e-7) max_dt=Some(0.1850314752) last_dt=Some(1.0868125188689959e-7)
```

**Identical to both prior runs — `accepted_substeps=18002`, and `min_dt`/`max_dt`/`last_dt`
match to the exact same floating-point bits.** The well-BHP chop had zero effect and, given the
bit-identical `min_dt`/`max_dt`, most likely never engaged at all: the well's raw per-iteration
BHP delta apparently never exceeded the `dbhp-max-rel=1.0` (100% of current BHP) cap in this
scenario. **The hypothesis that an unchopped, oscillating BHP update was perturbing the coupled
reservoir residual is REFUTED.** BHP itself is not the oscillating variable.

**Consequence — do not guess again.** Two well-reasoned, OPM-verified fixes have now been tried
(iteration-count decoupling, ruled out by code inspection before implementation; BHP chop, ruled
out empirically at a cost of ~5 hours of compute across two runs). Continuing to guess at a
third fix without better visibility into what is *actually* oscillating (candidates not yet
ruled out: the perforation-rate variable itself, deliberately left unchopped to match OPM's own
lack of a `WQTotal` clamp; or ResSim's own `relax_well_state_toward_local_consistency`
post-processing step — a RESSIM-SPECIFIC mechanism with no direct OPM counterpart, run after
every Newton update, which is a plausible oscillation source the OPM-fidelity review has not
yet examined) would repeat exactly the trial-and-error pattern flagged as unproductive earlier
in this session. Recorded as an open item requiring cheaper diagnostic tooling (e.g. a modified
native test that writes the full trace to a file for the specific late-time window, since the
pathology's substep count implies it is concentrated very close to the end of the simulated
day — `max_dt=0.185` days means the "healthy" phase likely covers the bulk of the day in a
handful of substeps, with virtually all 18,002 substeps spent crawling through a tiny residual
sliver of time) before attempting a further live fix.

### Bundle N disposition (2026-07-10) — parked; retrospective written

Consolidated retrospective + recommended sequencing written to `docs/FIM_BUNDLE_N_DESIGN.md`
§10 (what was established with evidence, disposition, and the P → diagnostic → W plan);
`docs/FIM_STATUS.md` updated with a Bundle N section and reprioritized "Known Open Gaps"
(Bundle P first, then the late-window diagnostic, then the nested well solve "Bundle W");
`TODO.md` FIM next steps refreshed to match. Bundle N's code stays behind the `OpmAligned`
flag, default `Legacy`, fully no-op gated — inert, not deleted (its pieces are the building
blocks the eventual OPM-shaped solver still needs) and not promoted (§5 failed).

### Coarse-factorization cost lever (2026-07-10) — offline decisive, live promoted, `FIM-LINEAR-011`

Follow-up to `FIM-BUNDLE-P`'s P0 (REFUTED for reuse). P0.1's build-cost breakdown was re-run
with `coarse_factorization_ms` split into `dense_inverse_ms`/`coarse_ilu0_ms` (they were
conflated in one timer) to confirm precisely which piece dominates: on the heavy corpus,
`dense_inverse_ms=48.8` vs `coarse_ilu0_ms=0.12` — **400x**, confirming the dense inverse
(`invert_pressure_block`'s `try_inverse()`) alone is the cost, not the ILU0 setup that runs
alongside it.

**Offline 3-way comparison** (new `coarse_factorization_lab_compare` in `gmres_block_jacobi.rs`,
new `solver_lab_coarse_factorization_comparison` test; recaptured both corpora identically —
185 bounded + 414 heavy systems, exact counts matching the original P0 run):

| | dense inverse | LU factorization | BiCGStab+ILU0 |
|---|---:|---:|---:|
| bounded (529 coarse rows) | 90.7ms | 20.5ms (4.4x cheaper) | 0.54ms (**168x cheaper**) |
| heavy (432 coarse rows) | 45.3ms | 10.6ms (4.3x cheaper) | 0.45ms (**101x cheaper**) |

LU reproduces the inverse's solution to `~1e-10` (machine precision, exact). BiCGStab —
already the production coarse-solve path above the 512-row threshold — converges on **every
one of 599 captured systems with zero failures**, residual reduction ratio median `~4e-7`, max
`~1e-6` (far tighter than Newton's own tolerance needs). BiCGStab strictly dominates LU here:
cheaper by another ~20-25x, and already proven production code (no new solver path).

**Live promotion**: `PRESSURE_DIRECT_SOLVE_ROW_THRESHOLD` lowered `512→300` (coarse rows =
cell count post-well-elimination: heavy=432, `22x22x1`=484, `23x23x1`=529 already above 512,
`20x20x3`x2=1200 already above; `gas-rate 10x10x3`=300 stays on dense, exactly at the new
threshold, untested at that size but trivially cheap regardless). Gates:

- `cargo test --lib -- fim::linear::`: 36/36 pass (0 changes needed — thresholds referenced
  symbolically everywhere).
- Locked smoke 3/3; Buckley-Leverett benchmarks 3/3.
- Full control matrix: **bit-identical on all 5 non-heavy cases**, including `22x22x1` (newly
  flipped from dense to BiCGStab) — `substeps=4 | accepts=4+0+0 | retries=0/2/0`, unchanged.
- Heavy case (`--dt 1`): wall-clock `36.9s → 6.8s` (**5.4x**). Substep count/trajectory DID
  shift (`32→52` substeps, `retries=0/13/0→0/8/7` — a new "mixed" retry classification appears
  for the first time), consistent with this system's already-established chaotic sensitivity to
  linear-solve perturbations (Task #37, the `k`-sweep) — the coarse solve is now approximate
  (`~4e-7` residual) rather than exact, and this system's Newton trajectory is known to bifurcate
  on changes this small.
- **Fine-dt FOPT** (`--steps 16 --dt 0.0625`, the physics gate that actually matters):
  `oil=3847.59` vs OPM's converged `3826.12` — **+0.56% drift, better than the currently-accepted
  bundle's own +1.50%** (`3883.47`, `k=1.25`'s accepted result). The coarse-solve swap did not
  cost accuracy; if anything the trajectory it lands on tracks OPM slightly more closely.

**Verdict: PROMOTED as `FIM-LINEAR-011`.** Net: dramatic, validated per-solve cost win (100-170x
on the coarse stage, 5.4x heavy-case wall-clock) with no physics-accuracy cost — the opposite of
`FIM-DAMP-004`'s trade-off. The heavy-case substep-count change is not itself a regression signal
(this system's substep counts have never been directly comparable across bundle configurations —
`docs/FIM_STATUS.md`'s own historical trajectory note says as much); the fine-dt FOPT check is
the metric that was actually at risk, and it improved.

**Not investigated (candidate follow-ups, not required for this promotion):** whether `k=1.25`
(tuned against the OLD dense-inverse coarse solver) is still the best value under this new
config — the "mixed" retry class appearing for the first time is worth a first-principles look
if a future k-resweep happens, but per the user's own 2026-07-06 guidance not to chase small
accuracy deltas, this is deferred, not urgent.

### Late-window trace diagnostic on the 18k pathology (2026-07-11)

Per `docs/FIM_BUNDLE_N_DESIGN.md` §10's recommended sequencing (Bundle P → this diagnostic →
Bundle W) and `TODO.md` "FIM next steps" #2: two fixes for the heavy-case `OpmAligned`
18,002-substep pathology were already honestly refuted (iteration-count decoupling — no-op by
inspection; verbatim `dbhp-max-rel` BHP chop — bit-identical 18,002 rerun). Two refuted fixes is
the guessing budget; this builds the cheap diagnostic visibility instead of a third blind guess.

**Instrumentation** (`src/lib/ressim/src/fim/trace_sink.rs`, new module; wiring in
`fim/timestep.rs`/`fim/newton.rs`): a native-only, env-gated file trace sink
(`FIM_TRACE_FILE`), mirroring `fim/linear/capture.rs`'s pattern. `FIM_TRACE_FILE` alone gets a
per-substep `LEDGER` line (BHP, perforation rates, iters, growth, dt) for the whole run, cheap
enough to run unconditionally. `FIM_TRACE_DT_BELOW`/`FIM_TRACE_SUBSTEP_START` narrow full
per-iteration tracing (every existing `fim_trace!` line, plus a new `WELLTRACE` line with
per-iteration well/perforation state) to just the collapse window. `FIM_MAX_SUBSTEPS` overrides
the hardcoded 100,000 cap so a windowed rerun can abort shortly after capturing the window.
All four are no-ops when unset — verified: full control matrix (all 6 commands) bit-identical,
locked smoke 3/3, `assembly_ad` parity 10/10, wasm build green (module compiles for wasm like
`capture.rs` but every call site is `#[cfg(not(target_arch = "wasm32"))]`, so it's dead code
there — confirmed harmless, same as the existing `capture.rs` precedent).

**Re-baseline** (commit `9554e9f` + this no-op-verified instrumentation on top; working tree
otherwise had only an unrelated pre-existing `docs/CASE_LIBRARY_ROADMAP.md` edit — treated as
non-provisional since the instrumentation's no-op behavior was independently proven via the
bit-identical control-matrix check above, not merely assumed):

```
FIM_TRACE_FILE=<path> cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib \
  fim::timestep::phase5_repro::repro_water_pressure_12x12x3_opm_aligned_no_trace -- --ignored --nocapture --exact
→ accepted_substeps=17990 advanced_dt=1.000000/1.000000 linear_bad=7 nonlinear_bad=1 mixed=1
  min_dt=Some(1.032244883492794e-6) max_dt=Some(0.1850314752) last_dt=Some(3.5426427140716754e-6)
  wall-clock: 1288.708s (~21.5 min)
```

**The pathology persists post-`FIM-LINEAR-011`**: `17,990` substeps (vs. the prior `18,002`,
both catastrophically over the `≤35` gate) — the small shift is the same chaos-sensitivity to
linear-solve precision already documented for this case (cf. Legacy's `32→52` shift under the
same lever), not a meaningful change. Wall-clock dropped from the cleanest prior solo run's
`298m13s` (`17893s`) to `1288.7s` — **~13.9x faster**, confirming `FIM-LINEAR-011`'s own framing
("makes every future heavy-case experiment ~an order of magnitude cheaper") and making this kind
of diagnostic run tractable within a session for the first time.

**Windowed rerun** (`FIM_TRACE_SUBSTEP_START=25 FIM_MAX_SUBSTEPS=530`, same driver): completed
in `40.14s` (`accepted_substeps=530`, capped as expected). The run is bit-for-bit deterministic
across invocations — early substeps (dt, iters, bhp, q) match the uncapped re-baseline run
exactly at the same indices — so this window is representative of the full pathology, not an
artifact of a different trajectory.

**Finding — the oscillating/stuck variable, with a mechanism, not just a name:**

BHP is independently reconfirmed, more strongly than the earlier chop refutation, as not the
culprit: `raw_dbhp=[0.0, 0.0]` for both wells, **exactly** (not merely small) on every single
Newton iteration across every substep inspected — the well-constraint row is trivially satisfied
every iteration because BHP is pinned at its control limit (`bhp=[500.0, 100.0]` bit-identical
across the entire window).

The actual culprit is the **producer well's perforation rate**, and the mechanism is a
**persistent disagreement between the raw Newton correction and
`relax_well_state_toward_local_consistency`** ([state.rs:307](../src/lib/ressim/src/fim/state.rs:307)),
not a classical back-and-forth oscillation. Inspected 5 independent `iters=20` substeps (27, 30,
32, 33, 35) via the new `WELLTRACE` line — same signature every time:

- The raw (pre-relax) Newton correction to the producer's perforation rate (`raw_dq[1]`)
  settles into a **non-vanishing plateau** within the first few iterations and stays there
  through iteration 18+ (e.g. substep 27: settles at `≈+0.581 m³/day`; substep 30: `≈+0.405`;
  substep 32: `≈+0.641`; substep 33: `≈+0.278`; substep 35: `≈+0.381` — different per substep,
  constant *within* a substep).
- `relax_well_state_toward_local_consistency`'s contribution (`relax_dq_approx`, computed as
  `candidate − (state + damping·raw_update)`) tracks the **near-exact negative** of that same
  plateau every iteration (e.g. substep 27: `≈−0.581`), so the two nearly cancel — net movement
  per iteration is tiny, which is why `q` and the reservoir-side CNV/MB both *look* converged
  (flat) from iteration ~2 onward.
- Despite that, the `perforation_flow` residual family (`res_pf`) never drops to zero — it
  plateaus at a small-but-nonzero floor that scales linearly with the same `raw_dq[1]` plateau
  (ratio `≈8.63e-5`, consistent across all 5 substeps to 3 significant figures — i.e. `res_pf`
  and `raw_dq[1]` are literally measuring the same underlying disagreement).
- The injector well is unaffected throughout (`raw_dq[0]`/`relax_dq_approx[0]` both stay at
  `~1e-7`/`~1e-12`, negligible) — this is specific to the BHP-limited producer.

Mechanistic read: `relax_well_state_toward_local_consistency` recomputes its own "consistent"
perforation rate from the current candidate reservoir state each iteration
(`connection_rate_for_bhp`) and blends toward it — a rate formula independent of, and evidently
not agreeing with, the AD-linearized Jacobian's implicit perforation-flow equation, by a
persistent offset. Every iteration: the raw Newton step corrects toward *its* zero, relax
immediately pulls back toward a *different* implied value by nearly the same magnitude, and the
next iteration's Newton correction (computed against the post-relax state) reproduces the same
disagreement — a standoff, not a convergence, and not a 2-period oscillation either (`res_pf`'s
own iteration-to-iteration delta stays small throughout, which is precisely why `FIM-NEWTON-006`'s
OSC-DETECT widening — tuned to the classical `d1≈0, d2≥0.2` signature — doesn't and structurally
can't see this: the large raw/relax components cancel before they ever show up as a residual
swing). This is consistent with, and sharpens, the original §5 finding ("a well/perforation
residual that does NOT shrink with dt forces `iters=20`") — the missing piece is *why* it doesn't
shrink: not the perforation-rate unknown itself (which converges fine on its own, per the
raw-Newton-only trajectory), but its forced disagreement with `relax_well_state_toward_local_consistency`.

**Consequence for Bundle W**: the nested well-equation solve (`docs/FIM_STATUS.md` gap #3)
should replace `relax_well_state_toward_local_consistency` outright with a converged per-well
inner solve (matching OPM's `iterateWellEquations`), rather than trying to patch the relax
step's blend factor or trust radius — the diagnosis is a structural disagreement between two
independently-derived rate formulas, not a tuning parameter. `WELL_RATE_MANIFOLD_BLEND`/
`WELL_RATE_TRUST_RADIUS_*` in `state.rs` are the relax step's current constants, kept for
reference but not a promising tuning target given this finding.

**Scope**: read-only diagnostic, no solver-behavior change. Instrumentation kept in the tree,
gated off by default (verified no-op above). Raw ledger/window trace files are scratch output,
not committed.

### Bundle W plan written (2026-07-11)

Follow-up to the diagnostic above: full checkpointed implementation plan for the nested
well-equation solve written to `docs/FIM_BUNDLE_W_PLAN.md`; registry row `FIM-BUNDLE-W` (OPEN)
added. Key plan decisions, all downstream of `FIM-DIAG-002`'s finding: (1) the inner solve must
drive the same assembled well residual rows as the global assembly (W1's bit-match agreement
test encodes this), (2) the previously-refuted `dbhp-max-rel` chop is re-homed inside the inner
loop where OPM actually applies it, (3) the cheap WELLTRACE mechanism gate runs before the full
§5 re-run so a wrong design fails in minutes not hours, (4) Legacy adoption is explicitly
deferred to its own experiment (plan §7). Also corrected the "Hypothesis A" citation history in
the plan's evidence section: Phase 8's original FB-crossover hypothesis found no supporting
evidence — what survived was the well-source-dominance pattern plus Bundle N §5's controller
pathology, now sharpened by the diagnostic's standoff mechanism.

### Bundle W checkpoint W0: OPM source verification (2026-07-11, commit `2f0f284`)

Full findings live as an appendix in `docs/FIM_BUNDLE_W_PLAN.md` ("Appendix: W0 OPM source
verification"); this entry is the worklog-discipline summary with the numbers that matter for
later checkpoints. Verified against the pinned local checkout `OPM/opm-simulators`
(`062cb19986aa8f11cffc30351fd2fee355d0ccb4`, tag `interim_release/2024.12-4152-g062cb1998`).

**Correction to prior citations**: the reservoir Newton model class was renamed upstream from
`BlackoilModel`/`BlackoilModel_impl.hpp` (what earlier Bundle N docs cite) to
`NonlinearSystemBlackOilReservoir`/`NonlinearSystemBlackOilReservoir_impl.hpp`, between whatever
checkout Bundle N's session used and this one. All citations below are against the file names
actually present in this checkout — don't trust old file-name citations without re-verifying.

**Loop structure confirmed**: `WellInterface::iterateWellEquations`
(`WellInterface_impl.hpp:532`) is called from `prepareWellBeforeAssembling`
(`WellInterface_impl.hpp:1018`, call site `:1066`), itself invoked once per outer Newton
iteration from `BlackoilWellModel::assemble()`, **before** `assembleWellEqWithoutIteration(dt)`
(`BlackoilWellModel_impl.hpp:1186`) — wells converge first, then get linearized into the global
system without further iteration that same outer step. Gated (not literally unconditional) by
`shouldRunInnerWellIterations` (`NewtonIterationContext.hpp:95`): true while
`globalIteration_ < max_niter_inner_well_iter_` (`MaxNewtonIterationsWithInnerWellIterations` =
**99**, effectively always-on for realistic outer iteration counts).

**Inner loop body** (`iterateWellEqWithSwitching`, `StandardWell_impl.hpp:2458` — the default
path since `LocalWellSolveControlSwitching` defaults `true`): `do {...} while (it < max_iter)`,
`max_iter = MaxInnerIterWells` = **50**. Control-mode switching (rate↔BHP↔THP, open↔stop) is
checked via periodic discrete re-evaluation every 4 iterations
(`min_its_after_switch`, `StandardWell_impl.hpp:2482`) — structurally different from ResSim's
continuous Fischer-Burmeister complementarity row. Documented as a deliberate divergence, not
something Bundle W ports; ResSim's FB row stays as the existing assembled equation.

**Convergence test** (`StandardWellEval::getWellConvergence`, `StandardWellEval.cpp:156`): two
*separately*-toleranced checks, correcting the "tolerance-wells=1e-4" shorthand used loosely
elsewhere in these docs — that number is only exactly right for the flux/mass-balance rows and
for BHP-controlled wells specifically:
- flux rows: `ToleranceWells` = **1e-4** (relaxed to `RelaxedWellFlowTol` = **1e-3** after
  `StrictInnerIterWells` = **40** iterations); hard fail above `MaxResidualAllowed` = **1e7**.
- control-equation row (`checkConvergenceControlEq`, `WellConvergence.cpp:39`): tolerance
  depends on the well's *current active control mode* — `{rates: 1e3, grup: 1e4, bhp: 1e-4,
  thp: 1e-6}` (`StandardWellEval.cpp:211`), four orders of magnitude looser for rate control
  than for BHP control.
- a hard `WrongFlowDirection` sign-consistency check for pressure-controlled wells (not a
  tolerance) — producer flux must not be negative, injector flux must not be positive.

**`updateNewton` chop** (`StandardWellPrimaryVariables.cpp:262`, called every inner iteration):
BHP capped at `DbhpMaxRel` = **1.0** (100% relative), floored at `1 bar − 1 Pa`. **This is the
exact value the refuted Bundle N §5 BHP-chop follow-up ported verbatim** — confirms that fix was
tested in the wrong place (the outer Newton loop) rather than built with the wrong formula; its
correct home is the inner well loop this bundle builds. `WQTotal` (total well rate) has **no
magnitude clamp at all**, only a post-hoc sign floor — reconfirms the 2026-07-09 finding with a
fresh citation.

**Parametrization note** (informative, doesn't change the design): OPM's `StandardWell` uses a
*lumped* `[WQTotal, WFrac, GFrac, Bhp]` unknown set, not one rate unknown per perforation the
way ResSim's `FimState::perforation_rates_m3_day` does. Bundle W targets ResSim's own assembled
rows (plan §2), not OPM's exact variable choice — only the *structural* pattern (nested bounded
Newton, converged before global assembly, invisible to the outer iteration count) transfers.

**Failure policy**: non-convergent wells get `stopWell()` + `solveWellWithZeroRate(...)`
(`ShutUnsolvableWells` = **true** default) — OPM does not silently accept an under-converged
well state, it forces a well-defined degraded state. Flagged as an open design question for W2
(resolved there: keep the last iterate, report not-converged, let the outer retry ladder decide
— did not implement OPM's stop-the-well escalation, which is a larger behavior change than this
bundle's scope).

**One real correction to the standing Bundle N narrative**: the *iteration count* fed to the
timestep controller is confirmed outer-only (`getNumIterations_`,
`AdaptiveTimeStepping_impl.hpp:1186`, reads `total_newton_iterations` which increments once per
call to `nonlinearIterationNewton`, `NonlinearSystemBlackOilReservoir_impl.hpp:237` — wells'
internal iterations never separately increment it). But the outer **convergence check** is NOT
well-blind: `NonlinearSystemBlackOilReservoir::getConvergence`
(`NonlinearSystemBlackOilReservoir_impl.hpp:1008`) computes `report = getReservoirConvergence(...)`
then `report += wellModel().getWellConvergence(...)` — the aggregate outer report does include a
well term. In practice this rarely blocks anything extra (wells already converged via the inner
loop by the time this runs), but "N1's acceptance excludes well/perforation rows entirely"
(`docs/FIM_BUNDLE_N_DESIGN.md` §5.1) describes ResSim's `OpmAligned` simplification, not
literally OPM's structure — this is exactly the gap plan §5 step 3 proposes closing, now backed
by a precise citation instead of an inference.

### Bundle W checkpoint W1: local well system + agreement tests (2026-07-11, commit `7509289`)

New `fim/wells_inner.rs`: `assemble_well_local_system(sim, state, topology, well_idx) ->
FimWellLocalSystem` builds one physical well's local residual (`well_constraint` row, then one
`rate_consistency` row per perforation) and Jacobian w.r.t. `[bhp, q_perf...]`, with the
reservoir cell state held frozen (read as input, not solved-for). Built by calling the exact
same shared AD primitives `assembly_ad.rs`'s `add_well_residual_terms`/`add_well_jacobian_terms`
call for these rows (`well_constraint_residual_fb_generic`,
`well_constraint_bhp_column_and_fb_gradient`, `well_constraint_own_perforation_rate_jacobian`,
`connection_rate_generic`, `rate_consistency_cell_bhp_jacobian`, `producer_fractions_generic`)
— not a reimplementation. Two small `assembly_ad.rs` helpers (`well_cell_input`,
`well_control_generic`) promoted from private to `pub(crate)` for reuse; verified zero behavior
change via `assembly_ad` parity (10/10 unaffected).

**Agreement tests** (4, all passing) directly encode the design constraint from plan §2: for a
constructed state, the local system's rows/columns must exactly match the corresponding
rows/columns of a full `assemble_fim_system_ad` call.
- `local_system_matches_global_assembly_bhp_controlled` / `..._rate_controlled`: both control
  modes, two-well fixtures mirroring `assembly_ad.rs`'s own `two_phase_bhp_controlled_wells`/
  `two_phase_rate_controlled_wells` structural-parity fixtures.
- `local_system_matches_global_assembly_away_from_convergence`: perforation rates perturbed
  `+500` before comparing — deliberately not a near-zero-residual state, where a formula bug
  could hide behind both sides evaluating to ~0.
- A no-cross-coupling check: one perforation's `rate_consistency` row never touches another
  perforation's `q` column, matching the global assembler's `tri.add_triplet(perf_row, q_col,
  1.0)` (each row only ever writes its own `q_col`).

Residuals compare bit-identical (`assert_eq!`). Jacobian entries needed a `1e-12` tolerance, not
exact equality — one test failure at `local=-9.414691248821328e-16 global=0.0` before the fix
traced to the global sparse assembler's `add_if_nonzero` dropping `|value| <= 1e-14` as an
implicit zero, while the dense local Jacobian stores the raw computed value. Confirmed as a
sparse-storage convention difference, not a formula divergence, by inspecting `assembly_ad.rs`'s
`add_if_nonzero` directly before accepting the tolerance fix (not just widening the assertion
until it passed).

**Closed-form observation** (not exploited yet at W1, confirmed empirically at W2 below):
`connection_rate_generic` takes `(bhp, cell)` and does not depend on `q` at all — the
`rate_consistency` row's dependence on `q` is the trivial identity (`tri.add_triplet(perf_row,
q_col, 1.0)`). For a BHP-controlled well (`well_constraint = bhp − bhp_target`, no `q`
dependence either — exactly why `FIM-DIAG-002` measured `raw_dbhp` at exactly `0.0` every
iteration) with frozen reservoir cells, `q = connection_rate_generic(bhp_target, cell)` is a
one-shot closed-form evaluation, not an iterative fixed point. This reframes the `FIM-DIAG-002`
standoff: very likely an artifact of the *coupled global iterative linear solve*'s imprecision
on that specific row/unknown pair (fgmres-cpr/BiCGStab solving the FULL system approximately),
not genuine nonlinear difficulty in the isolated well subsystem.

Gates: `assembly_ad` parity 10/10, `fim::wells` 18/18, locked smoke 3/3, wasm build green (new
code unreachable in production until W3 wires it in — same "kept in the tree, no-op verified"
pattern as `trace_sink.rs`).

### Bundle W checkpoint W2: inner Newton loop (2026-07-11, commit `5765f28`)

`solve_well_locally`/`solve_wells_locally` (`fim/wells_inner.rs`): a bounded, chopped Newton
loop over W1's `assemble_well_local_system`, mutating `state.well_bhp[well_idx]`/
`state.perforation_rates_m3_day[perf_idx]` in place — same call shape as
`relax_well_state_toward_local_consistency`, so it is a drop-in replacement at that single call
site (`state.rs:424`) once W3 flips the flag. Defaults per W0's numbers:
`max_iterations = 50` (`MaxInnerIterWells`), `tolerance = 1e-4` (`ToleranceWells`),
`dbhp_max_rel = 1.0` (`DbhpMaxRel`).

**`chop_bhp_update`**: OPM's exact chop formula (`raw_delta_bhp.clamp(-cap, cap)` where
`cap = |bhp| * dbhp_max_rel`, then floored at `BHP_LOWER_LIMIT_BAR = 1.0 − 1e-5` bar) — the
formula the refuted Bundle N follow-up ported to the wrong (outer) loop; this is its correct
home. No magnitude clamp on `q`, matching OPM's `WQTotal` update exactly.

**Convergence check reuses the exact scaling formula the global assembly's own convergence test
uses** — a small refactor of `fim/scaling.rs` extracted `well_constraint_scale(bhp_bar,
control_slacks)` and `perforation_flow_scale(rate_m3_day)` out of `build_equation_scaling`'s
inline logic into standalone `pub(crate)` functions, with `build_equation_scaling` itself
updated to call them (zero-behavior-change refactor: verified via `fim::scaling` 3/3 and
`assembly_ad` parity 10/10 unaffected). `wells_inner.rs` calls these same two functions for its
own scaled-residual-peak convergence check, so "inner converged" and "outer sees zero" cannot
silently drift into two hand-matched copies of the same formula.

**`perforation_flow_direction_ok`**: OPM's `WrongFlowDirection` check (W0), scoped to
pressure-controlled (non-rate-controlled) wells, applied per-perforation since ResSim has no
single aggregate `WQTotal` the way OPM's lumped parametrization does. A state whose residual is
within tolerance but has the wrong sign correctly reports `converged: false`, not silently
accepted.

**Failure handling**: a singular local Jacobian (`.lu().solve()` returns `None`) or exhausted
iteration budget both keep the last iterate and report `converged: false` — no acceptance
widening, per the `FIM-NEWTON-005` lesson (don't paper over an inner failure by loosening what
counts as "close enough").

**10 tests, all passing — the closed-form observation from W1 confirmed empirically, not just
in theory**: `bhp_controlled_well_converges_from_perturbed_state` starts from perforation rates
perturbed `+800` m³/day away from consistency and converges in **exactly 1 iteration**, final
scaled residual peak `2.3e-16`/`8.5e-16` (verified via a temporary debug print, removed before
commit) — machine epsilon, exactly matching the "one Newton step lands on the closed-form
solution" prediction. `rate_controlled_well_converges_to_slack_feasible_state` (genuinely
nonlinear FB case, since the constraint row *does* depend on `q` there) converges and lands on a
feasible `(bhp, q)` per the well's own slack tolerances. `exhausted_budget_reports_not_converged_
without_panicking` uses a deliberate `max_iterations: 0` to exercise the give-up path
deterministically — standing in for the plan's "deliberately infeasible case" wording, since the
FB reformulation is specifically designed to avoid genuine physical infeasibility rather than
produce a clean pathological test fixture.

**Regression gate**: full `fim::` test suite, 277 passed / 3 failed / 20 ignored (197.53s). The
3 failures were confirmed byte-identical *by exact test name* to the pre-existing 2026-07-07
known failures recorded in `TODO.md` (`fim::timestep::tests::
changing_hotspot_resets_extra_growth_cooldown_budget`, `repeated_same_hotspot_extends_growth_
cooldown_budget`, `fim_enabled_step_advances_time_and_records_history_for_closed_system`) — not
a new regression, verified by rerunning the full suite with output saved to a file and grepping
the `failures:` block, not assumed from the count alone. `assembly_ad` parity 10/10, wasm build
green.

### Bundle W checkpoint W3: flag wiring + no-op gates (2026-07-11)

**`state.rs`**: `apply_raw_update`'s last parameter changed from `relax_well_state: bool` to a
new `WellStateUpdateMode` enum (`None`/`Relax`/`NestedSolve`), matched in a 3-way `match` at the
single call site where well post-processing happens (right after the raw Newton update and
`enforce_control_bounds`). `NestedSolve` calls `wells_inner::solve_wells_locally` then
`enforce_control_bounds` again, mirroring `Relax`'s existing shape exactly. The only other
caller (`apply_newton_update`, `#[cfg(test)]`) passes `None`, and one existing test
(`apply_newton_update_frozen_limits_well_overshoot_toward_local_consistency`, which specifically
tests the Legacy relax trust-radius behavior) was updated to pass `Relax` explicitly so it keeps
testing what it always tested.

**`newton.rs`**: `FimNewtonOptions` gained `nested_well_solve: bool` (default `false` in the
`Default` impl — the two existing literal-construction sites both already use
`..FimNewtonOptions::default()`, so neither needed updating). The single
`apply_newton_update_frozen` call site picks `NestedSolve` vs `Relax` based on the flag,
independent of `nonlinear_flavor` (plan §5: "an independent flag... evaluable under both
flavors" — confirmed live by the flag-on sanity check below, which ran `--nested-well-solve`
under *Legacy*, not just `OpmAligned`).

Plan §5 item 3 (the outer-criteria addition): the `converged_on_entry` computation's
`opm_aligned` branch gained a `wells_ok` term —
```rust
let wells_ok = !opm_aligned
    || !options.nested_well_solve
    || wells_inner::all_wells_converged(sim, &state, &topology, &FimWellInnerSolveOptions::default());
let converged_on_entry = if opm_aligned {
    iteration >= OPM_NEWTON_MIN_ITERATION_INDEX && opm_conv.would_accept && wells_ok
} else { ... };
```
`wells_ok` is trivially `true` whenever either flag is off, so this is provably a no-op unless
*both* `opm_aligned` and `nested_well_solve` are set — verified by the control-matrix bit-
identity gate below, not just by inspection. This was the only acceptance decision site in the
whole file that reads `opm_conv.would_accept` under `opm_aligned` (confirmed by grep before
writing the change — a single site, not the many-return-points shape some other Bundle N
mechanisms have had to thread through).

**`wells_inner.rs`**: refactored the per-iteration scaled-residual-peak + flow-direction check
out of `solve_well_locally`'s loop body into a standalone `well_convergence_status` helper
(takes an already-assembled `FimWellLocalSystem`, returns `{converged, scaled_residual_peak}`).
Two new public functions built on it: `well_is_converged` (one well, read-only — assembles
once, checks, returns) and `all_wells_converged` (all wells, `.all(...)`) — together these are
the `getWellConvergence` analog from W0 appendix G, a pure *check*, not a solve. Two new tests:
`well_is_converged_matches_solve_result_before_and_after` (asserts the read-only check agrees
with `solve_well_locally`'s own verdict, both before solving on a perturbed state and after) and
`all_wells_converged_requires_every_well` (converges only one of two wells, confirms the
aggregate check still fails). `fim::wells_inner` now 12/12.

**Diagnostic/API surface** (mirrors the existing `fim_opm_aligned_nonlinear`/
`setFimOpmAlignedNonlinear`/`--opm-aligned` triple exactly): `ReservoirSimulator.fim_nested_well_solve: bool`
field (`lib.rs`), initialized `false` in the constructor (`frontend.rs`), `setFimNestedWellSolve`
wasm setter, threaded into `newton_options.nested_well_solve` in `timestep.rs`'s
`step_internal_fim_impl`, `--nested-well-solve` CLI flag in `scripts/fim-wasm-diagnostic.mjs`
(help text + parsing + `sim.setFimNestedWellSolve(true)` call, alongside the existing
`--opm-aligned` wiring). The native `repro_water_pressure_12x12x3_opm_aligned_no_trace` driver
(the exact `FIM-DIAG-002` re-baseline vehicle) gained a `FIM_NESTED_WELL_SOLVE` env-var toggle
so it doubles as the W4 §5 re-run vehicle without a new test function.

**No-op gate** (flag off): full `fim::` suite `279 passed / 3 failed / 20 ignored` (192.93s) —
the 3 failures are the identical pre-existing 2026-07-07 names (confirmed by grepping the
`failures:` block, not inferred from count), `+2` from this checkpoint's own new tests vs W2's
277. Full 6-command control matrix, rebuilt wasm, bit-identical to documented baselines
including the heavy Legacy case (`substeps=52 accepts=51+3+2060 retries=0/8/7
retry_dom=nonlinear-bad:water@1215` — exact match). Wasm build green (`bash scripts/build-wasm.sh`,
only the pre-existing harmless `dim()`-never-used warning).

**Flag-on sanity check** (informational, not a W4 gate — just confirms the wiring is live code,
not dead): `node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 22x22x1 --steps 1
--dt 0.25 --diagnostic summary --no-json --nested-well-solve` (Legacy flavor) lands on the same
coarse substep/retry counts as the flag-off baseline (`4 substeps, retries=0/2/0`) but with
visibly different per-substep Newton iteration counts (`n10,n9,n8,n6` vs the baseline's
`n13,n9,n7,n7`) — a genuinely different trajectory that happens to match on the coarse metric,
not a silent no-op. Adding `--opm-aligned` on top of `--nested-well-solve` on the same case
changes the outcome much more visibly: `24` substeps (vs `12` for `--opm-aligned` alone,
previously recorded), a new dominant retry class (`linear-bad:oil@1450`, previously
`nonlinear-bad`), and a much lower minimum dt (`2.813e-5` vs whatever `--opm-aligned` alone
reached). This confirms the new `wells_ok` outer-criteria gate is genuinely firing and changing
acceptance decisions under the combined flags — a real, substantial behavior change worth
investigating, deliberately **not evaluated here**: the plan's own W4 ordering runs the cheap
`FIM-DIAG-002` mechanism gate on the *heavy* case first, bounded cases only after that passes.
This `22x22x1` regression (relative to `--opm-aligned` alone) is exactly the kind of finding W4
step 3 ("bounded cases... watch whether the gap narrows") is designed to characterize honestly,
not something to explain away here.

### Bundle W checkpoint W4: evaluation — mechanism fixed, heavy-case gate failed for a different reason (2026-07-11)

**Step 1, mechanism check (PASSED).** Native `--release` capped run,
`FIM_NESTED_WELL_SOLVE=1 FIM_TRACE_FILE=<ledger> FIM_MAX_SUBSTEPS=1000` on
`repro_water_pressure_12x12x3_opm_aligned_no_trace`: `accepted_substeps=1000
advanced_dt=0.916822/1.000000 ... min_dt=6.309373552505448e-6 ... elapsed 67.617s`. Immediately
notable: the pathology's `min_dt` floor (`6.3e-6`) is roughly 6x less extreme than the
post-`FIM-LINEAR-011` baseline's `1.03e-6` and ~60x less extreme than the original
pre-`FIM-LINEAR-011` `1.09e-7` — a hint, confirmed below, that *something* did genuinely
improve even though the substep count did not.

Followed with a windowed rerun (`FIM_TRACE_SUBSTEP_START=980`, same cap) to get full
per-iteration `WELLTRACE` on a stuck (`iters=20`) substep (substep 997, deterministically
reproduced — the ledger's last 5 lines matched the uncapped-cap run bit-for-bit, confirming no
run-to-run nondeterminism). Inspected all 20 iterations in full:

- `res_wc=0.000000e0` every single iteration (well-constraint row trivially satisfied, as
  expected for a BHP-pinned well — matches `FIM-DIAG-002`'s finding that `raw_dbhp` is exactly
  `0.0`, now doubly confirmed).
- `res_pf` (perforation-flow row): `0.0` at iter 0, then `3.56e-12, 4.76e-16, 3.57e-16, 7.14e-16,
  ...` for the rest — **machine epsilon from iteration 1 onward**. Previously (`FIM-DIAG-002`,
  pre-Bundle-W) this same field floored at a non-vanishing `~5e-5`–`5.5e-5` and stayed there for
  all 20 iterations. **This is the standoff, gone.**
- `q_post` for the producer: `3821.179165213835` (iter 0) → `3821.1791704584202` (iter 1) →
  `3821.1791703504177` (iter 2) → ... → `3821.179169910965` (iter 19) — stable to **8 decimal
  places** from iteration 1 onward. The well variable itself converges almost immediately and
  stays put.
- `raw_dq[1]` (producer) is still a persistent, non-vanishing ~0.58 m³/day every iteration, and
  `relax_dq_approx[1]` (the WELLTRACE field's name predates Bundle W and still reads "relax" —
  it is arithmetically the same `post − (pre + raw)` decomposition, now attributing the *nested
  solve's* multi-step internal convergence work rather than the old relax blend) still tracks
  its near-exact negative. **This surface pattern looks unchanged from `FIM-DIAG-002`'s trace at
  first glance — the numbers that actually matter (`res_pf`, `q_post`'s stability) show the
  underlying meaning has completely changed**: before, this cancellation was two mechanisms
  fighting to a non-converging standoff; now it's the nested solve's own internal Newton
  iterations converging smoothly to a fixed point that the outer linear solve's raw proposal
  simply doesn't move away from. A reminder to read the *converged quantity*, not just the
  presence of a cancellation pattern, when judging whether a standoff is real.
- `cnv=[6.100e-5, 6.146e-5, 0.000e0]` (water, oil, gas): **frozen** — unchanged past the 4th
  significant digit across all 19 iterations shown (`6.100e-5`/`6.146e-5` at iter 1 through
  `6.100e-5`/`6.146e-5` at iter 18). `mb=[1.412e-7, 1.423e-7, 0.000e0]` similarly frozen. This is
  the reservoir-side CNV/MB entry criterion — completely unaffected by the well fix, and it is
  what keeps `would_accept=no` for 19 straight iterations before the final-iteration relaxed
  tier (`would_accept=pv-relaxed` at iteration 19) finally accepts.

**Verdict for step 1**: the mechanism check's literal pass condition ("`res_pf` drops below
tolerance within the inner-converged iterations instead of flooring") is unambiguously met.

**Step 2, full uncapped §5 re-run (FAILED).** Same command without the cap, background,
native `--release`:
```
FIM_NESTED_WELL_SOLVE=1 cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib \
  fim::timestep::phase5_repro::repro_water_pressure_12x12x3_opm_aligned_no_trace -- --ignored --nocapture --exact
→ accepted_substeps=18015 advanced_dt=1.000000/1.000000 linear_bad=8 nonlinear_bad=1 mixed=3
  solver_ms=1220457.88 min_dt=1.0337753842559846e-6 max_dt=0.1850314752 last_dt=1.4780226131883012e-6
  elapsed 1235.482s (~20.6 min)
```
**`18,015` vs the `17,990`-substep `OpmAligned`-only baseline (commit `a362e29`) — a `25`-substep
difference, well within this case's already-documented chaos-sensitivity to small perturbations
(cf. `FIM-LINEAR-011`'s Legacy `32→52` shift), i.e. essentially unchanged.** Wall-clock `1235.5s`
vs `1288.7s` — also essentially the same. **Decisively fails the `≤35` gate**, exactly as
severely as before Bundle W.

Root cause, from the same ledger: only **12 retry events** across all 18,015 substeps (`8
linear-bad, 1 nonlinear-bad, 3 mixed`), all with `dominant=oil@430` — none well-related, and far
too few to explain 18k substeps on their own. The substep explosion is entirely the *accepted*-
substep dt cycle: `growth=0.400 limiter=opm-iter` (an `iters=20` substep, shrinking) alternating
with `growth=3.000 limiter=opm-max-growth` (an `iters=2` substep, recovering) — the exact same
alternation pattern `FIM-DIAG-002` originally found, but now driven by the reservoir CNV
plateau confirmed above, not the well standoff. The run's final accepted state at `t=1.000000`
(`q≈[-3628.19, 3627.10]`) matches the *original, pre-Bundle-W* baseline's own final `q` values
closely — the two runs converge to essentially the same physical endpoint via the same
pathological path, just for a different underlying reason at the per-iteration level.

This pattern — a small residual (`cnv≈6e-5`) that won't shrink further and only clears via a
relaxed final-iteration tier — is consistent with (though not proven identical to; different
cell, different controller mechanism) the phenomenon `docs/FIM_STATUS.md` already documents as
"understood and benign" for Legacy's own `water@1215` plateau: "a genuine local steady-state
region colliding with intentionally-strict entry/zero-move acceptance gates." Bundle W's scope
never included this — plan §5's "Explicitly NOT in Bundle W" already excludes any acceptance-
criteria change.

**Step 3, bounded cases (mixed).**
```
node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 23x23x1 --steps 1 --dt 0.25 \
  --diagnostic summary --no-json --opm-aligned --nested-well-solve
→ substeps=12 retries=1/0/0 retry_dom=linear-bad:oil@1585
```
**Identical** to `--opm-aligned` alone (same substep count, same retry count, same dominant
retry cell — differs only in wall-clock `outer_ms`, expected run-to-run timing noise). The
nested solve is a genuine no-op on this case: `23x23x1`'s own bottleneck (`linear-bad:oil@1585`)
was never well-related, so fixing wells here changes nothing, for or against.

`22x22x1` (first measured in W3's flag-on sanity check, re-cited here since it's directly
relevant to this evaluation): `24` substeps vs `12` for `--opm-aligned` alone — a real
regression. **Not root-caused with the same rigor as the heavy case** — plausible by analogy
(fixing wells could equally expose a reservoir-side plateau on this smaller grid) but not
confirmed by a dedicated windowed trace. Recorded honestly as an open, unresolved data point,
not assumed to match the heavy case's story just because the shape rhymes.

**Step 4, fine-dt FOPT: deliberately not run.** With the primary gate failing decisively and one
bounded case regressing, running the more expensive physics-accuracy check would not change the
disposition — mirrors Bundle N §5's own precedent (moved straight to root-cause analysis once
its gate failed decisively, rather than completing every remaining checklist item first). Revisit
only if a future fix for the reservoir-CNV plateau reopens the heavy-case gate.

**Step 5, control matrix**: already done and gated at W3 (flag-off bit-identity); nothing in W4
touched the flag-off path, so not repeated.

### Bundle W checkpoint W5: verdict — NOT PROMOTED, mechanism kept (2026-07-11)

Applying the original Bundle N `docs/FIM_BUNDLE_N_DESIGN.md` §5 promotion rule (end metrics
only, heavy-case substep/cut behavior in the `≤35` class or nothing) to the heavy case with W
in: **FAILS** (`18,015` substeps). This is the same disposition shape as Bundle N itself: the
targeted mechanism is real, independently validated three separate ways (W1's bit-exact
agreement tests against the global assembly, W2's empirical 1-iteration/machine-epsilon
convergence on synthetic perturbed states, and W4's windowed trace confirming the exact
diagnosed standoff is gone on the real heavy-case trajectory) — but insufficient alone, because
fixing it exposed a second, independent, previously-masked architecture gap (the reservoir CNV
plateau) that Bundle W was never scoped to address.

**Disposition**: `nested_well_solve` stays in the tree, default `false`, fully no-op verified
(W3's control-matrix bit-identity gate). Not deleted — it is validated, correct, and the
`getWellConvergence`-equivalent well-convergence checking it introduces is exactly the building
block a future combined fix would still need. `FIM-BUNDLE-N`'s own registry status
(REWORK REQUIRED) is unaffected — Bundle N is evaluated independently of this flag, which
defaults off, so Bundle N's own heavy-case number is unchanged by this work.

**New open item** (not Bundle W's to fix): the reservoir-side CNV plateau at near-steady-state
under `OpmAligned`'s entry criterion, now clearly exposed as the heavy case's dominant remaining
blocker. Recommend a fresh, dedicated diagnostic pass — reusing `FIM-DIAG-002`'s own
`WELLTRACE`/`LEDGER` tooling, which already incidentally captured this signature while
investigating wells — targeting `cnv`/`mb` per-iteration evolution and the exact conditions
under which the final-iteration relaxed tier is needed, before proposing any fix. The same
discipline that produced `FIM-DIAG-002` (evidence before a third guess) applies here by direct
extension: this is effectively a *first* look at a newly-exposed mechanism, not a well-trodden
one, so there is no guessing budget to spend carelessly.

### Week retrospective (2026-07-11): the heavy-case failure is a conjunction, and the chain is nearly exhausted

Prompted by a user review question: three consecutive NOT-PROMOTED verdicts (Bundle N, Bundle P,
Bundle W) under end-metric-only gating — if the heavy case fails on a *combination* of errors,
element-by-element evaluation can never promote anything and never "solves" the case. Analysis
of the week's own recorded traces, plus two fresh source checks, confirms the combination
structure directly and sharpens the remaining problem to a single measurable quantity.

**1. The conjunction is real and is directly visible in the traces already recorded.**
The `FIM-DIAG-002` window trace (substep 27, pre-Bundle-W) shows TWO criteria frozen above
tolerance simultaneously: the perforation-flow residual floored at `~5e-5` (the diagnosed
standoff) AND `mb=[1.858e-7, 4.668e-7]` frozen from iteration 2 onward — both > `1e-7`, both
immobile. Bundle W's W4 trace (substep 997) shows exactly ONE remaining: `res_pf` now at machine
epsilon, `mb=[1.412e-7, 1.423e-7]` still frozen. The end gate (substep count) is a step function
over a conjunction: it cannot move until the LAST frozen criterion clears, which is why fixing
the well standoff produced `17,990 → 18,015` (no change) despite being a genuine fix. The user's
critique is confirmed by the data — with the caveat that the *construction* side of the week was
already combination-aware (N1-N5 were deliberately built as one bundle; W was evaluated stacked
on N) — the gap is on the *measurement* side: end-metric gating is conjunction-blind, and the
NOT-PROMOTED bookkeeping makes cumulative progress read as serial failure.

**2. The hidden progression the substep count can't show.** Re-reading the week's numbers as a
sequence of binding-constraint margins instead of substep counts:
- pre-Bundle-N: MB stalls at `≈2e-6` (20x over the `1e-7` tolerance) — fixed by N2 per-cell chop
  (95% vs 48% OPM-acceptable attempts, checkpoint 2);
- post-N: perforation-flow residual floored at `≈5e-5` scaled (the `FIM-DIAG-002` standoff) —
  fixed by Bundle W (machine epsilon, W4);
- post-W: MB frozen at `1.41e-7` — **1.4x over tolerance**, the sole survivor.
The binding margin has tightened >100x across the week. The chain is nearly exhausted, which
argues for finishing the serial-peeling approach rather than abandoning it — but with the
measurement and bookkeeping changes below.

**3. The remaining bottleneck, sharpened from already-recorded data + two fresh checks.**
Fresh check 1 (OPM pinned source): `ToleranceMb=1e-7`, `ToleranceMbRelaxed=1e-6`,
`ToleranceCnv=1e-2`, relaxed MB applies at the final iteration only by default
(`MinStrictMbIter=-1`, `NonlinearSystemBlackOilReservoir_impl.hpp:751`) — our port's constants
and tier logic match exactly (`newton.rs:1794-1797`). Fresh check 2: on the stuck substeps,
CNV passes by **160x** (`6.1e-5` vs `1e-2`); **MB alone binds, at 1.41x over strict**, for 18
straight iterations until the final-iteration relaxed tier (`1e-6`) accepts — then N3 sees
`iters=20` and collapses dt. The entire 18k-substep catastrophe now rests on one number: why
does our MB freeze at `1.41e-7` when OPM solves this same case at ~2.5 iters/step (i.e., its MB
genuinely drops below `1e-7`, since its acceptance tiers are identical to ours)?

Frozen at 4 significant figures across 18 iterations means the state is an **invariant point of
the modified iteration map** (Newton step + per-cell chop + nested well solve + bounds), not
slow convergence. The trace shows the mechanism: each iteration the coupled linear solve
proposes `dq≈+0.58` for the producer (its way of zeroing the well-cell mass-balance rows via the
source term) and the nested well solve vetoes it back to the perforation-consistent value.
Three ranked hypotheses, each with a cheap decisive test:
- **H1 (displaced standoff / constrained fixed point)**: enforcing the perforation equation
  exactly displaces the same underlying inconsistency into the well-cell mass-balance rows —
  the `1.41e-7` MB *is* the old standoff wearing a different family label. Test: locate the
  peak-MB cell during the freeze (extend the CNV-MB trace line to name the binding cell/family
  — the FAIL-SITE-DETAIL machinery already exists); if it's the producer's perforation cell
  (cell 143) or its column, H1 confirmed. ~70s capped run.
- **H2 (linear-precision floor)**: the loose `5e-3` outer linear tolerance (`FIM-LINEAR-008`)
  caps achievable MB reduction near steady state. Test: force the direct linear backend on a
  capped heavy run — if the freeze breaks, H2. Also testable offline against captured frozen
  substeps (`FIM_CAPTURE_SEQUENCE_DIR` + solver lab, exact-vs-iterative post-step MB).
- **H3 (MB formula fidelity)**: our MB formula runs ~1.4x hot vs OPM's at the same state — a
  units/pore-volume/dt-factor discrepancy would explain everything, including the unexplained
  3x bounded-case gap (Bundle N §10 obs. 6: `12/1` vs Legacy `4/2` — if our MB reads hot,
  *every* `OpmAligned` case pays extra iterations chasing `1e-7`). Test: W0-style formula audit
  of `cnv_mb_diagnostics` against `NonlinearSystemBlackOilReservoir_impl.hpp`'s
  `getReservoirConvergence` (B_avg construction, pore-volume weighting, dt factor).

**4. The unexploited asset: OPM Flow is installed (`/usr/bin/flow`).** All week we compared
against OPM's *source* (static fidelity, W0-style) and its *end physics* (fine-dt FOPT) — never
its *per-iteration runtime trajectory* on the pathological window. A differential run — OPM Flow
on the heavy-case deck (the FOPT reference deck already exists per the opm-reference-pipeline)
with convergence logging, diffed against our ledger through the steady-state tail (t≈0.83-1.0) —
answers H2/H3 from the oracle side: what are OPM's actual MB values at the same simulated times,
and does it ever need its relaxed tier there? This should become a standing method, not a
one-off: trajectory-level differential testing is the direct form of "learn from OPM Flow".

**5. Combination coverage gaps found in the review** (now cheap post-`FIM-LINEAR-011`: capped
heavy runs are ~70s, full runs ~21 min — a factorial that would have cost 40+ hours a week ago
costs an afternoon now):
- **Legacy + W on the heavy case: never run** (only the `22x22x1` sanity check). Legacy's own
  heavy-case issues (the `perf@1299` mixed retries, the `water@1215` plateau ladders) are
  well-adjacent; if Legacy+W beats Legacy's current `52`, that is a *promotable Legacy-side win*
  independent of the whole `OpmAligned` question (own full gate per plan §7).
- OpmAligned+W ± forced-direct linear backend (the H2 test).
- OpmAligned+W with `min_strict_mb_iter` set positive — OPM's own shipped knob for "use relaxed
  MB after N iterations" — recorded as a *fallback only*: OPM's defaults solve this case without
  it, so reaching for the knob before understanding H1-H3 would be acceptance-widening in OPM
  clothing (`FIM-NEWTON-005` lesson applies).
- The `22x22x1` OpmAligned+W `12→24` regression and the `23x23x1` first-substep
  `linear-bad:oil@1585` — both uninvestigated, both cheap windowed traces.

**6. Bookkeeping reframe (the direct answer to "element by element you never promote"):**
declare the candidate stack (`OpmAligned` + `nested_well_solve`) a first-class tracked
configuration with its own registry identity and baseline (`18,015` @ this commit), measure
per-fix progress by **binding-constraint margin** (currently MB `1.41e-7` vs `1e-7`) and
substeps-to-t=0.9 on capped runs, and make the promotion decision once, for the stack, when the
chain is exhausted. Mid-chain mechanisms get "validated-in-stack" dispositions rather than
reading as failures. One porting nit to fold into the next verification pass: OPM's
`NewtonMinIterations` default is **2**; our `OPM_NEWTON_MIN_ITERATION_INDEX` is 1 — re-verify
the intended off-by-one semantics against `iterCtx.iteration()`.

**Recommended order**: (1) the FIM-DIAG-003 binding-cell trace + forced-direct capped run
(hours, discriminates H1/H2); (2) MB formula audit (hours, H3, also explains the bounded-case
3x if it hits); (3) OPM Flow differential trajectory on the heavy deck (a day, decisive from
the oracle side); (4) Legacy+W heavy case (minutes, possible independent win); (5) only then
decide whether the stack promotes or the approach changes.

### FIM-DIAG-003 checkpoint D0: instrumentation (2026-07-11)

Per `docs/FIM_DIAG_003_PLAN.md` D0. No-op gated; both additions are diagnostic-only and neither
touches the accept/retry decision.

**1. Binding-criterion trace.** `cnv_mb_from_parts` (`fim/newton.rs`) now also computes, per
component: `cnv_peak_cell[c]` (the cell realizing `max_coeff[c]`, tracked inline in the existing
loop) and `mb_peak_cell[c]` (the largest `|r_i,c|` among cells whose sign agrees with the summed
imbalance `r_sum[c]` — the cell(s) actually driving the MB error rather than one that cancels
against it). A new `binding: Option<BindingCriterion>` field names the single failing
criterion (CNV or MB, which component) with the largest `value/tolerance` overshoot ratio. The
`CNV-MB` trace line gained a trailing `binding=[...]` field, e.g.
`binding=[mb[water]=1.412e-07/1.000e-07 cell=143]`, or `binding=[none]` when `would_accept`.
`fim_trace!` always runs (writes to the sim's trace buffer unconditionally); this is pure
diagnostic output, no control-flow change.

**2. Forced-direct-linear switch.** New `fim_force_direct_linear: bool` field on
`ReservoirSimulator` (`lib.rs`, default `false`), set only via a `pub(crate)`
`set_fim_force_direct_linear` (added to the existing plain `impl ReservoirSimulator` in
`timestep.rs`, alongside `append_fim_trace_line` — no `#[wasm_bindgen]`, no wasm surface, same
as the pattern established for `fim_nested_well_solve`). When set, `step_internal_fim_impl`
forces `newton_options.linear.kind = FimLinearSolverKind::SparseLuDebug` — every Newton
iteration solves exactly via the direct backend instead of the default iterative CPR/GMRES
stack. Wired into the native repro driver
(`repro_water_pressure_12x12x3_opm_aligned_no_trace`, same env-gated pattern as
`FIM_NESTED_WELL_SOLVE`): `FIM_FORCE_DIRECT_LINEAR=1` sets the flag before `sim.step()`.

**Gate results** (clean tree, commit pending this checkpoint):
- `cargo build --manifest-path src/lib/ressim/Cargo.toml` (lib only): clean, no new warnings.
- `cargo build --manifest-path src/lib/ressim/Cargo.toml --tests`: clean (the `set_...` setter
  warning under lib-only build is expected — it's only called from the `#[cfg(test)]` repro
  driver).
- `cargo test assembly_ad`: 10/10 pass (parity gates untouched).
- Locked smoke: `drsdt0_base_rs_cap_flashes_excess_dissolved_gas_to_free_gas`,
  `spe1_fim_first_steps_converge_without_stall` (124.5s), `spe1_fim_gas_injection_creates_free_gas`
  (425.1s) — all pass.
- `bash scripts/build-wasm.sh`: succeeds (`release` profile, `wasm-opt` optimized).
- Control matrix (`fim-solver-debug` skill, all 6 commands): pending — the sandbox's Bash safety
  classifier went temporarily unavailable mid-session; will complete and record before D1.

### FIM-DIAG-003 checkpoint D2: H3 MB formula audit (2026-07-11)

Per `docs/FIM_DIAG_003_PLAN.md` D2. Static, line-by-line comparison of `cnv_mb_from_parts`
(`fim/newton.rs:1846-` — updated line numbers post-D0) against the pinned OPM checkout at
`OPM/opm-simulators` (verified `git log -1` = `062cb19986aa8f11cffc30351fd2fee355d0ccb4`,
`interim_release/2024.12-4152-g062cb1998`, clean tree — this IS the tag the design doc cites).

**Quantity-by-quantity comparison**, OPM source in
`opm/simulators/flow/NonlinearSystemBlackOilReservoir_impl.hpp`:

| Quantity | OPM (file:line) | ResSim (`fim/newton.rs`) | Verdict |
|---|---|---|---|
| `B_avg[c]` | `getMaxCoeff` (`:1114` etc.): `Σ 1/invB(phase)` per cell, then `/= global_nc_` in `localConvergenceData` (`:621-623`) — current-iterate pressure | `b_avg[c] = Σ fvf[c] / n_cells`, `fvf` from `state.cells[idx].pressure_bar` (current iterate) | **Matches** |
| `R_sum[c]` | `getMaxCoeff` (`:1117`): raw `modelResid[cell][compIdx]` accumulated, no PV weight | `r_sum[c] += residual[i*3+c]` raw | **Matches** |
| `maxCoeff[c]` (CNV) | `getMaxCoeff` (`:1118-1121`): `max_i(|R_i|/pvValue_i)`, `pvValue = referencePorosity(cell,t=0) * dofTotalVolume(cell)` — fixed reference PV | `max_coeff[c] = max_i(|r|/pv_i)`, `pv_i = sim.pore_volume_m3(i) = dx*dy*dz*porosity` (pure geometric reference, no rock-compressibility factor — confirmed via `grid.rs:17-19`, contrasted with the compressibility-adjusted PV the actual accumulation term uses, `properties.rs:136-145`) | **Matches** — both use fixed reference PV for the convergence check, deliberately different from the compressed PV inside the physics itself |
| per-cell max-over-components (CNV-PV-split) | `characteriseCnvPvSplit` (`:646-656`): `maxCnv = max_c(\|r_c\| * B_avg[c])` (inner_product with `max` combine, `multiplies` per-element — **not** a sum, re-checked after an initial misread) | `cell_max_cnv = cell_max_cnv.max(r.abs()*b_avg[c]/pv)` | **Matches** |
| `pv_sum` | `localConvergenceData` (`:607`): same `pvValue` accumulated across all cells | `pv_sum += pv` (same `pore_volumes_m3[i]`) | **Matches** |
| `CNV[c]` | `:827`: `B_avg[c] * dt * maxCoeff[c]` (`dt` explicit — OPM's residual is a **rate**, verified via `fvbaselocalresidual.hh:590-596`: storage `(V_new-V_old)*scvVolume/dt`, flux already a rate via `computeFlux`/`alpha`) | `cnv[c] = b_avg[c] * max_coeff[c]` (no `dt` — ResSim's residual is dt-integrated: accumulation is a raw `current-previous` volume difference with **no** `/dt`, verified `properties.rs::cell_accumulation_generic:199-203`; flux/well terms are `coefficients * q * dt_days`, verified `assembly_ad.rs:144,246,249`) | **Matches structurally** — `ResSim_residual ≡ dt_days * OPM_rate_residual` by construction, so the `dt` factor is legitimately absorbed; this is inherently a dimensionless ratio either way (the `dt` cancels within each system's own formula), not a source of a fixed cross-system scale error |
| `mb[c]` | `:828`: `\|B_avg[c]*R_sum[c]\| * dt / pvSum` | `mb[c] = \|b_avg[c]*r_sum[c]\| / pv_sum` | **Matches** (same dt-absorption argument) |
| `ToleranceMb` / `ToleranceMbRelaxed` / `ToleranceCnv` | `1e-7` / `1e-6` / `1e-2` (already verified week-retrospective) | same constants | **Matches** (re-confirmed) |
| `RelaxedMaxPvFraction` | `BlackoilModelParameters.hpp:50`: `0.03` | `OPM_RELAXED_MAX_PV_FRACTION = 0.03` | **Matches** |
| `min_strict_mb_iter_` relax-final-iteration gate | `:752-753`: `relax_final_iteration_mb = min_strict_mb_iter_ < 0 && iteration() == maxIter` | `relax_final_iteration` passed in as `is_final_newton_iteration = iteration+1 == max_newton_iterations` | **Matches** |

**No `~1.4x`-shaped formula bug found.** Every sub-quantity in the MB/CNV computation is a
faithful, source-verified port. **H3 (MB formula fidelity) is REFUTED** as an explanation for
the `1.41e-7` freeze — the formula itself is correct.

**Independent finding (not H3): confirmed off-by-one in `OPM_NEWTON_MIN_ITERATION_INDEX`.**
Traced the exact semantics of OPM's `iteration() >= minIter` gate:
`NewtonIterationContext::iteration()` (`NewtonIterationContext.hpp:52-55`) is 0-based and starts
at 0 each timestep (`resetForNewTimestep`, `:122-128`); `NonlinearSystemBlackOilReservoir::
nonlinearIteration` (`:203-231`) calls `initialLinearization` (assemble + convergence check
against the **current** `iteration()` value, **before** any update this pass) then, only after
that, `this->simulator_.problem().advanceIteration()` (`:229`) — so `iteration()` at each check
equals the number of Newton **updates already applied** in this timestep, exactly mirroring
ResSim's own `for iteration in 0..max_newton_iterations` loop structure (`newton.rs:2802` region:
assemble → check `converged_on_entry` → apply update only if not converged). `NewtonMinIterations`
default is **2** (`BlackoilModelParameters.hpp:163`), checked as `iteration() >= minIter`
(`NonlinearSystemBlackOilReservoir_impl.hpp:175`) — so OPM requires `iteration() >= 2`, i.e. at
least **2 Newton updates already applied** (3 total residual evaluations) before acceptance is
even possible. ResSim's `OPM_NEWTON_MIN_ITERATION_INDEX = 1` only requires `iteration >= 1` (1
update applied, 2 evaluations) — **one iteration too permissive**, confirmed by direct
correspondence between the two loop structures, not just parameter-name pattern-matching.

This is a real, independently-scoped correctness bug, but **does not explain the heavy-case
plateau**: the stuck substeps run ~18-20 Newton iterations, far past either gate value, so
`minIter=1` vs `minIter=2` is irrelevant once the substep is already this deep. It could plausibly
explain (or contribute to) the still-unexplained bounded-case 3x gap (`OpmAligned` 12/1 vs
Legacy 4/2 — Bundle N §10 obs. 6) for fast-converging cases where the gate is the actual limiter.
**Not fixed in this checkpoint** — flagged as a candidate follow-on fix, own registry row, own
control-matrix + locked-smoke gate per the project's promotion discipline (changes acceptance
behavior under `OpmAligned` everywhere, not just the heavy case). Decision on fix timing deferred
to D5.

**D2 verdict**: H3 refuted via source-cited static audit. No fix, no re-run triggered by this
checkpoint (the plan's "if a discrepancy is found, fix it" branch does not fire for H3 itself).
Effort: ~1.5h (audit) vs the ~2-4h estimate.

### FIM-DIAG-003 checkpoint D1: H1 CONFIRMED, H2 REFUTED (2026-07-12)

Per `docs/FIM_DIAG_003_PLAN.md` D1. Two windowed capped runs, native `--release`, commit
`a4fad1c` (D0+D2 checkpoint): `FIM_TRACE_SUBSTEP_START=980 FIM_MAX_SUBSTEPS=1000
FIM_NESTED_WELL_SOLVE=1 cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib
repro_water_pressure_12x12x3_opm_aligned_no_trace -- --ignored --nocapture`, with/without
`FIM_FORCE_DIRECT_LINEAR=1`, `FIM_TRACE_FILE` pointed at a scratch log.

**Run 1 (baseline, default `fgmres-cpr`)**: `accepted_substeps=1000 advanced_dt=0.916822/1.0
linear_bad=8 nonlinear_bad=1 mixed=3`, wall `75.85s`. Binding-cell census over the whole window
(238 `binding=[mb...]` lines): **198/218 (91%) at cell 143, 20/218 (9%) at cell 130** — zero
lines anywhere else. Cell 143 is `idx(11,11,0)` — the producer's own perforation cell (`nx-1,
ny-1, 0`, confirmed from the repro driver's `add_well` call). Cell 130 is `idx(10,10,0)`, the
producer's immediate diagonal neighbor in the same (only-completed) layer. **100% of the frozen
MB is concentrated at the well or its immediate neighborhood.**

**Run 2 (`FIM_FORCE_DIRECT_LINEAR=1`, every Newton iteration solved exactly via
`SparseLuDebug`)**: `accepted_substeps=1000 advanced_dt=0.922922/1.0 linear_bad=0
nonlinear_bad=13 mixed=0`, wall `171.58s` (2.3x slower, as expected — direct factorization vs
iterative CPR/GMRES, consistent with `FIM-LINEAR-011`'s own cost measurements). **The freeze does
not break**: binding-cell census (220 lines) is **180/200 (90%) at cell 143, 20/200 (10%) at
cell 130** — same cells, same ~91/9 split. MB magnitude at the frozen plateau is `2.301e-7`/
`2.331e-7` in this run's tail window vs `1.412e-7`/`1.423e-7` in the baseline's — **higher**, not
lower, with exact linear solves (different substep/trajectory state, not a strict single-point
comparison, but decisively not "drops below `1e-7`"). Progress metric: `advanced_dt` moved
`0.9168→0.9229`, a **0.6% improvement** — nowhere near the plan's decisive-test bar ("materially
improves").

**This run is also the D1 point-3 cross-check** (forced-direct + binding-cell trace together):
exact linear solves with the residual still parked at the well cells is the plan's own stated
signature for **pure H1**.

**Verdict**:
- **H1 (displaced standoff) CONFIRMED.** The frozen `1.41e-7`-class MB literally lives at the
  producer's perforation cell (and its immediate neighbor) in both runs, regardless of linear
  solve exactness — direct evidence that Bundle W's fix (which drove the perforation-flow
  residual itself to machine epsilon, W4) displaced the same underlying well/reservoir
  inconsistency into the well-cell mass-balance row rather than resolving it. This matches the
  mechanism already on record from the pre-D1 trace: the coupled linear solve proposes `dq≈+0.58`
  each iteration to zero those rows via the source term, and the nested well solve vetoes it back
  — an invariant point of the modified iteration map, now confirmed to be spatially exactly where
  H1 predicted.
- **H2 (linear-precision floor) REFUTED.** Forcing exact linear solves neither breaks the freeze
  nor moves it off the well cells nor materially improves progress — the `5e-3` outer linear
  tolerance is not the limiting factor near this plateau.
- H1 and H3 (refuted at D2) together leave H1 as the sole standing explanation. The fix direction
  is therefore **nonlinear/well-coupling**, not linear-tolerance policy and not a CNV/MB formula
  bug.

Effort: ~15 min setup + 76s + 172s run time + analysis, well under the ~1h estimate.

### FIM-DIAG-003 checkpoint D4: combination coverage (2026-07-12)

Per `docs/FIM_DIAG_003_PLAN.md` D4, both items, using `scripts/fim-wasm-diagnostic.mjs`
(`--opm-aligned`/`--nested-well-solve` flags), commit `e12b95d`.

**1. Legacy + `nested_well_solve` on the heavy case (never run before).** Raw summary line:
`substeps=8 accepts=7+4+14907 dt=[6.104e-5,3.125e-2]`, real accept rungs `s0@3.125e-2 →
s6@6.104e-5` (7 entries, dt collapsing monotonically over just 7 real Newton-solved steps).
**Read carefully, not at face value**: `substeps=8` looks dramatically better than Legacy's own
baseline `substeps=52` (`real=51,cooldown=3,hotspot_plateau=2060`, real accept rungs run
`s0@3.125e-2` through the high-30s/40s before the tail collapses) — but the `accepted_substeps`
ledger field collapses an entire hotspot-plateau replay block into ~1 history entry regardless of
its size (`51 real + 1 collapsed block ≈ 52`; `7 real + 1 collapsed block ≈ 8` — the arithmetic
matches exactly in both cases). The real signal is `real_accepted_substeps`: Legacy alone does
**51** genuine Newton-solved dt advances before the tail-end plateau; Legacy+`nested_well_solve`
does only **7** before permanently stalling at `dt=6.1e-5` and having the remaining ~99.8% of the
timestep auto-filled by cheap plateau-replay bookkeeping (14,907 replayed units, vs Legacy's own
2,060). **This is a regression, not a win** — `nested_well_solve` under Legacy causes a much
*earlier and more severe* stall (dt collapses to the plateau floor after 7 real attempts instead
of ~50), the opposite of the `docs/FIM_DIAG_003_PLAN.md` "possible independent win" framing
(condition explicitly required "physics intact," which this fails). This is exactly the
measurement trap the skill (`.claude/skills/fim-solver-debug/SKILL.md` "Reading the summary
line") and the week retrospective (§2, "the measurement was blind") both warn about — do not
read `accepted_substeps` alone as the gate metric when a plateau-replay block is present; check
`real_accepted_substeps` first. **Not a promotable win. No further action.**

**2. The `22x22x1` OpmAligned+`nested_well_solve` "`12→24` regression" and the `23x23x1`
`linear-bad:oil@1585` ride-along.** Re-derived at current HEAD (baseline discipline: "do not
trust expected counts written in old docs"):

| Case | OpmAligned alone | OpmAligned + `nested_well_solve` | Delta |
|---|---|---|---|
| `22x22x1` | `substeps=24`, `retry_dom=linear-bad:oil@1450`, `avg_p=319.00` | `substeps=24`, same `retry_dom`, same `avg_p` | **bit-identical** |
| `23x23x1` | `substeps=12`, `retry_dom=linear-bad:oil@1585`, `avg_p=317.18` | `substeps=12`, same `retry_dom`, same `avg_p` | **bit-identical** |

**Does not reproduce.** Both bounded cases are confirmed no-ops for `nested_well_solve` at the
current tree (matches every other field checked: `oil`, `inj`, `gor`, `dt` bounds). Note `23x23x1`
OpmAligned-alone's own number (`12`) matches the "`12`" the retrospective attributed to
`22x22x1` — most likely a mislabel/stale reading from an earlier commit, not a real regression
that has since been fixed (no intervening commit touched the `nested_well_solve`-off path). The
week-retrospective row is superseded: **no `22x22x1` regression exists on the current tree; both
bounded no-ops reconfirmed.**

**D4 verdict**: heavy Legacy+W = confirmed regression (not promotable); both bounded no-ops
reconfirmed clean, prior "regression" claim was stale/non-reproducible. No fixes triggered, no
new registry rows needed beyond this record (kept as this checkpoint's own evidence).

### FIM-DIAG-003 checkpoint D3: OPM Flow differential trajectory (2026-07-12)

Per `docs/FIM_DIAG_003_PLAN.md` D3, commit `c9d041e`. `/usr/bin/flow` (confirmed installed);
`origin/fim-opm-continuation-plan` has the deck harness but is stale relative to current master
(pre-dates most of `.claude/skills/`, several docs) — did **not** merge that branch; instead
recreated the specific deck adapted from its `water-medium-step1` template and tracked it fresh
on this branch as `opm/reference-decks/water-heavy-step1/CASE.DATA` (`DIMENS 12 12 3`, perms
2000/2000/200 mD matching `water-medium-step1` exactly, corner wells BHP 500/100, `TSTEP 1.0`).
One deliberate deviation from the template: `COMPDAT` well radius `0.1`, not the template's
`0.2` — verified against `frontend.rs::add_well` call sites that the actual repro driver
(`repro_water_pressure_12x12x3_opm_aligned_no_trace`) and the `fim-wasm-diagnostic.mjs` preset
both pass `well_radius=0.1`; the medium-step1 template's `0.2` does not match either and appears
to be its own pre-existing discrepancy (out of scope to fix here — noted for future cleanup).

Ran: `flow CASE.DATA --output-extra-convergence-info=steps,iterations` in a scratch working
directory (matching the harness's own copy-before-run pattern, keeping the tree clean).

**Result: OPM solves the entire `t=0→1.0` interval in ONE Newton solve, 11 iterations, ZERO
timestep cuts.** `CASE.INFOSTEP`: `Time=0 TStep=1 ... NewtIt=11 LinIt=14 Conv=1`. Total wall
time `0.03s`. This alone is a stark contrast with ResSim: Legacy needs 52 substeps (51 real),
Legacy+`nested_well_solve` collapses to 7 real substeps before stalling (D4), and
`OpmAligned`+`nested_well_solve` needs 18,015 substeps to cover the same interval.

**`CASE.INFOITER` per-iteration trajectory** (the decisive piece — answers the plan's D3
question #2 directly): at **iteration 10**, `MB_Oil=6.947e-7 MB_Water=1.970e-7` — the SAME
order of magnitude ResSim is frozen at (`1.41e-7`-`2.33e-7` across D1's runs), both still above
OPM's own strict `ToleranceMb=1e-7`. At **iteration 11** (the very next, and final, iteration),
`MB_Oil=1.088e-9 MB_Water=1.130e-8` — a clean **2-3 order-of-magnitude drop in a single Newton
step**, comfortably below tolerance (matching `CNV_Oil`/`CNV_Water`'s own `~250x` one-step drop,
the classic quadratic-convergence tail of a well-posed Newton iteration). `WellStatus=CONV` and
`PenaltyWellRes=0` at every iteration — no well-side distress recorded anywhere in the OPM
trajectory.

This directly answers the plan's own framing: **"is its MB at these states `~1e-8`, or
`~1.4e-7`-but-still-converging, or does it also touch its relaxed tier?"** — the answer is
**still-converging**: OPM transiently occupies the exact same residual-magnitude neighborhood
ResSim is stuck at, and takes one clean further Newton step through it. This is oracle-side proof
that `1e-7`-to-`2e-7` MB is not an inherent numerical floor for this physics/grid/well
configuration — it is a ResSim-specific structural stall, independently confirming H1 (D1) and
reinforcing H3's refutation (D2): a correctly-behaving Newton iteration passes through this exact
zone in one step, so there is nothing "hot" about the tolerance comparison itself, only about
ResSim's own iteration map having an invariant point there that OPM's doesn't.

`CASE.PRT` well-solver defaults (context, not a new lever): OPM's own well-equation inner solve
tolerance is `ToleranceWells=1e-4` — two orders looser than the reservoir `ToleranceMb=1e-7` — and
its own inner well iteration budget is `MaxInnerIterWells=50`/`StrictInnerIterWells=40`, far more
generous than ResSim's `nested_well_solve` inner solve. Consistent with (not new evidence beyond)
the already-recorded Bundle W design intent; not investigated further here, out of D3's scope.

**Not attempted in this checkpoint** (time-boxed per the user's "do D3 now, full plan" but the
single-shot result already being decisive): a matching multi-`TSTEP` OPM run replaying ResSim's
own accepted-dt sequence to compare dt-by-dt through the `t≈0.83-1.0` steady tail specifically.
The single-shot per-iteration trajectory already answers the load-bearing question (H1
independent oracle confirmation); the dt-sequence comparison would be corroborating, not
decisive, and is left as a candidate follow-on if a future fix needs finer trajectory-level
verification (this checkpoint doubles as the "adopt trajectory-level differential comparison as
a standing method" pilot per the week retrospective §4 — the method works and is cheap: one deck
+ one `flow` invocation + reading two small text files, seconds not hours).

**D3 verdict**: OPM oracle confirms H1 independently — a well-posed Newton iteration passes
cleanly through the exact MB magnitude ResSim is frozen at. No fix attempted (out of scope for a
diagnostic checkpoint); fix direction guidance for D5 is now well-triangulated from three
independent angles (D1's binding-cell census, D2's formula-fidelity audit, D3's oracle
trajectory): nonlinear/well-coupling, not linear tolerance, not CNV/MB formula fidelity, not an
inherent numerical floor at this residual magnitude.

### FIM-DIAG-003 checkpoint D5: verdict + `FIM-NEWTON-008` (2026-07-12)

Per `docs/FIM_DIAG_003_PLAN.md` D5, commit `08fe69b`.

**Verdict on H1/H2/H3**: the plan's three-way discrimination is complete and unanimous across
three independent methods:

- **H1 (displaced standoff into well-cell MB rows) — CONFIRMED.** D1's binding-cell census: 100%
  of the frozen-MB iterations bind at the producer's own perforation cell (91%) or its immediate
  neighbor (9%), in both the default and forced-exact-linear runs. D3's OPM oracle: a well-posed
  Newton step transits the exact same MB magnitude in one clean iteration with a 2-3
  order-of-magnitude drop, `WellStatus=CONV`/`PenaltyWellRes=0` throughout — proving the frozen
  magnitude is not intrinsically hard, only ResSim's own well-coupled iteration map has an
  invariant point there.
- **H2 (linear-precision floor) — REFUTED.** D1: forcing every Newton iteration through the
  exact `SparseLuDebug` backend neither breaks the freeze, moves it off the well cells, nor
  materially improves progress (`advanced_dt` `+0.6%` only).
- **H3 (MB formula fidelity) — REFUTED.** D2: line-by-line audit of `cnv_mb_from_parts` against
  the pinned OPM source found every sub-quantity a faithful, source-cited port; no `~1.4x`-shaped
  translation bug exists.

This is plan case **(a)**: "a scoped fix bundle for the confirmed mechanism." The mechanism is
now precisely located (the well-cell mass-balance rows, under `nested_well_solve`'s handling of
the coupled linear solve's `dq≈+0.58` proposal each iteration, vetoed back by the nested solve —
the exact signature already on record from before D1 even ran). **Designing and building that fix
bundle is out of scope for this diagnostic plan** (D0-D5 was explicitly instrumentation +
discrimination, "zero guessing budget spent on this mechanism" per the plan's own framing) — it
is the natural next unit of work, to be scoped as its own plan/bundle document with the same
checkpoint discipline as Bundles N/P/W, once opened.

**`min_strict_mb_iter` still explicitly out of scope**, now for a sharper reason than before: H1
is a genuine structural fixed point (the residual is bit-identical across 18 iterations, not
slowly converging), so relaxing WHEN the relaxed tolerance kicks in would not fix anything — it
would only widen acceptance around an unfixed defect, the exact `FIM-NEWTON-005` anti-pattern.

**Independent fix, PROMOTED this checkpoint (`FIM-NEWTON-008`)**: D2's off-by-one
(`OPM_NEWTON_MIN_ITERATION_INDEX` `1→2`) is small, well-understood, and orthogonal to H1 — fixed
and gated here rather than left open. Full control matrix (all 6 standard commands): **bit-identical**
(the constant is `OpmAligned`-only, so the Legacy/flag-off path is provably untouched — confirmed,
not just argued). `OpmAligned`-flavored re-runs of the three fast bounded cases:
`22x22x1`/`23x23x1` substep counts **unchanged**, `20x20x3` shows a small **expected increase**
(`15→16`, matching the fix's direction — stricter acceptance, closer to OPM's actual default,
which requires more not fewer forced iterations). Heavy-case pathology is unaffected by
construction (the floor only gates iterations 0-1; the heavy case fails at iteration ~18-20) — not
re-run for this fix, the code-path argument is exact. Locked smoke 3/3, `assembly_ad` parity
10/10. **This fix does NOT explain or close the bounded-case 3x gap** (`OpmAligned` `12/1` vs
Legacy `4/2`, Bundle N §10 obs. 6) — if anything it moves `OpmAligned` iteration counts up
slightly, the opposite direction from "explaining away the gap." That gap remains open and
unexplained; H3's refutation (D2) already ruled out the leading hypothesis for it (a hot MB
formula), so it is now itself a small standing question, not urgent enough to warrant its own
diagnostic plan yet.

**Stack promotion status: still OPEN.** The candidate stack (`OpmAligned` + `nested_well_solve`,
baseline `18,015` substeps @ `c916c87`) has not been re-evaluated end-to-end — that gate is the
original Bundle N §5 gate (heavy `≤35`-substep class + fine-dt FOPT + control matrix + bounded
cases not worse than Legacy) and stays closed until a fix for the now-precisely-located H1
mechanism exists and is evaluated. `FIM-DIAG-003` itself is **closed as a diagnostic** (D0-D5
complete, unanimous verdict, zero unresolved hypotheses) even though the underlying pathology is
not yet fixed — this is the correct disposition per the retrospective's own bookkeeping reframe
("mid-chain mechanisms get 'validated-in-stack' dispositions rather than reading as failures").

**Summary of this diagnostic's yield**: one independently promoted correctness fix
(`FIM-NEWTON-008`), one confirmed regression averted (`nested_well_solve` under Legacy, D4 — would
have been a false "win" without the `real_accepted_substeps` correction), one stale claim
retracted (the `22x22x1` "regression," D4), one refuted formula-fidelity concern closed (H3, D2),
one refuted linear-tolerance concern closed (H2, D1), and the mechanism precisely located for a
future fix bundle (H1, D1+D3) — all without a single guess spent on the mechanism itself before
the evidence was in hand.

## Bundle X (`docs/FIM_BUNDLE_X_PLAN.md`): well-update ordering / well-fraction fidelity

### Bundle X checkpoint X0: stage-by-stage forensics — a DIFFERENT root cause than planned (2026-07-12)

Per `docs/FIM_BUNDLE_X_PLAN.md` X0. Commit base `1fdd157`. Extended the D0 window instrumentation
with a new `WELLJAC` trace line (`fim/newton.rs`, window-gated, no-op when inactive) dumping,
per perforation, the `rate_consistency` row's own residual/diagonal plus its cell's water/oil/gas
row residuals and their `d/dq`, `d/dp`, `d/dsw` couplings — read directly from `assembly.jacobian`
via the same `CsMat::get(row, col)` accessor the W1 agreement test uses — plus a `WELLJAC-WATER`
line with the full face-by-face flux/accumulation/well-source breakdown
(`cell_equation_residual_breakdown`, the same helper `FAIL-SITE-DETAIL` uses). Windowed capped run
(`FIM_TRACE_SUBSTEP_START=980 FIM_MAX_SUBSTEPS=1000 FIM_NESTED_WELL_SOLVE=1`, same window as D1).

**Finding 1 — the planned question is answered, and it eliminates the planned suspects.**
`res_pf=0.000000e0` and `d(res_pf)/dq=1.000000e0` (a trivial, always-1 diagonal — the row is
definitional, `residual = q − f(p_cell, bhp, mobility)`) at every frozen iteration. A row with
zero residual and a unit diagonal contributes **zero** pull toward any `dq` change on its own —
the observed `raw_dq≈+0.70` is not coming from the well's own equation at all. Ruled out by
direct measurement, not inference: relaxation (`PERCELL-CHOP relax=1.00`, already on record),
chop (`sat_chopped_cells=0`, already on record; confirmed `opm_per_cell_chopped_update` never
touches perforation-rate entries by code inspection), `enforce_cell_bounds`/`enforce_control_bounds`
(confirmed by code inspection: neither function references `perforation_rates_m3_day` at all).

**Finding 2 — cell 143's own mass-balance residual is real and large relative to its only
effective lever.** Raw (unscaled) water/oil residuals at cell 143: `water=+1.696e-3`,
`oil=−1.721e-3` — genuinely nonzero (the `~1e-9`-scale figures seen earlier were
`EquationScaling`-normalized, not raw). Their Jacobian sensitivities: `d/dsw≈±20` (water `+20.00`,
oil `−20.04`, nearly exact opposites — a saturation move trades almost 1:1 between the two,
the expected mass-conservation identity), `d/dp≈2.7e-6` (water) `/5.55e-3` (oil), `d/dq≈4.5e-7`
(water) `/2.59e-5` (oil). **`dsw` is 3-4 orders of magnitude the dominant lever** — a fix via `dq`
alone would need `dq≈66` (vs the observed `0.70`); a fix via `dsw` alone needs only `≈-8.5e-5`.

**Finding 3 — `dsw` is legitimately clamped and cannot move.** `sw=0.100000` exactly, every
iteration — matching `s_wc=0.1` exactly (the repro's connate/irreducible water saturation, also
the reservoir's uniform initial condition — cell 143 is the producer, farthest corner from the
injector in a 12x12 grid, and genuinely has not yet seen breakthrough at `t≈0.92d` of `1.0d`).
`raw_dsw≈-7.6e-5` (the coupled linear solve's own proposed step) would push `sw` to `≈0.09992`,
**below** the connate floor — `enforce_cell_bounds`'s `cell.sw = cell.sw.clamp(sim.scal.s_wc, ...)`
clamps it straight back to exactly `0.1` every iteration, discarding the correction. This clamp
runs **unconditionally**, before the `WellStateUpdateMode` branch (`state.rs:435-438`) — i.e. it
would fire identically under `Relax`, `NestedSolve`, or `None`. **The `nested_well_solve`
ordering (the original Bundle X hypothesis) is not the primary mechanism** — the `dq` veto is
real (confirmed in D1/W's own traces) but it is downstream of, and secondary to, a saturation-bound
collision that has nothing to do with which well-update mode is active.

**Finding 4 — where the persistent residual actually comes from, and why it's structural, not
numerical noise.** `WELLJAC-WATER`: `accum≈-9.7e-9` (negligible — `sw` isn't moving, consistent
with Finding 3), `x-`/`y-` flux ≈`-1.7e-5` each (negligible — near-zero cross-cell water mobility,
as expected this close to the front), **`well=+1.730e-3`** — the well source term alone accounts
for essentially the entire residual (`total=1.695941e-3 ≈ well`). The producer's phase-split
(`producer_fractions_generic`, `fim/wells_ad.rs:52-81`) is **not** computed from the perforated
cell's own mobility alone — `perforation_control_cells` (`fim/wells.rs:822-838`, the single
shared call site for both the coupled assembly, `assembly_ad.rs:126/192/305`, and the nested
solve, `wells_inner.rs:82-92`) builds a **3x3 areal window** around every *producer* perforation
(injectors already get the single-cell treatment, `if perforation.injector { return
vec![perforation.cell_index]; }`) and sums mobility across it. Cell 143's own `krw` is exactly `0`
at `sw=0.1` (`SWOF` table endpoint), but its 3x3 neighborhood includes cell 130 (D1's secondary
binding cell, 9% of iterations) and other near-front neighbors with slightly elevated `sw` — their
nonzero water mobility leaks into the *well's* aggregate `water_fraction`, producing a small but
persistent nonzero water withdrawal that gets debited entirely against cell 143's own water
balance (the well source term is applied at the perforated cell, not distributed across the
neighborhood that supplied the mobility estimate). This is a structural mismatch, not noise — it
recurs identically every iteration because both the neighborhood-averaged fraction and cell 143's
own zero local mobility are themselves stable/converged; there is no lever in the system that can
zero it (Finding 2/3).

**Finding 5 — OPM does not do this.** Read `WellInterface<TypeTag>::getMobility`
(`WellInterface_impl.hpp:2105-2143`, pinned `062cb1998`): `mob[activeCompIdx] =
intQuants.mobility(phaseIdx)` at `intQuants = simulator.model().intensiveQuantities(cell_idx, 0)`,
where `cell_idx = this->well_cells_[local_perf_index]` — **the single connected cell only**, for
every well type (no producer/injector distinction, no neighborhood). `StandardWell::getMobility`
(`StandardWell_impl.hpp:706-756`) layers polymer/solvent/`WINJMULT` adjustments on top of that
same single-cell mobility — still no neighborhood averaging anywhere in the connection-rate path.
OPM's producer at the equivalent state would compute `water_fraction=0` **exactly** (its own
`krw=0` at the connate floor, nothing to blend in), withdrawing pure oil with no residual to
create. This directly explains D3's oracle finding (OPM transits the same MB magnitude cleanly in
one more iteration): OPM's local-only formulation never manufactures this residual in the first
place.

**Origin of the 3x3 design**: `git log -S "producer_control_state"` traces it to `d824f4f`
("Add producer control state management and enhance well control logic"), part of the original
pre-FIM, pre-OPM-alignment simulator design. `wells_ad.rs::producer_fractions_generic` is
documented as "a generic mirror of `wells::producer_control_state`'s fraction computation" — the
FIM/AD layer faithfully carried this convention forward without re-examining it against OPM. Not
a deliberate OPM-motivated design choice; an inherited, never-revisited divergence.

**Revised diagnosis for `FIM-BUNDLE-X`**: the well-update-ordering hypothesis (X1/X2 as originally
planned) is downgraded from primary to secondary/contingent. The primary, now-precisely-located
candidate fix is narrower and different in kind: **restrict `perforation_control_cells`'s producer
branch to the single perforated cell**, matching both the injector branch's existing behavior and
OPM's `getMobility` exactly. This is a single shared function (confirmed one call site pattern
across `assembly_ad.rs` and `wells_inner.rs`), scoped to the FIM engine only (`fim/wells.rs` is
separate from the legacy/IMPES `well_control.rs::producer_control_state_for_pressures`, confirmed
by grep — no legacy/public-simulator blast radius). Higher-leverage and more surgical than an
ordering change, but broader-reaching (it changes producer water-cut/GOR physics for every FIM
case with a producer, not just the heavy case) — needs the full control-matrix + locked-smoke +
BL-benchmark gate, not just a capped heavy-case check, before any promotion decision.

Plan doc (`docs/FIM_BUNDLE_X_PLAN.md`) updated to record this pivot; X1 retargeted from "pure
coupled well update" to "single-cell producer fraction," the original ordering probe kept as a
secondary/fallback item.

### Bundle X checkpoint X1: single-cell producer fraction — the fix, decisive result (2026-07-12)

Per `docs/FIM_BUNDLE_X_PLAN.md` X1 (retargeted per X0). Commit base `bb23c81`.

**Implementation**: `perforation_control_cells` (`fim/wells.rs:822`) gained a dev flag
(`fim_single_cell_producer_fraction`, default `false` = unchanged 3x3-window behavior). When set,
producer perforations get the same single-cell treatment injectors already have —
`vec![perforation.cell_index]` — matching OPM's `WellInterface::getMobility` exactly (X0
Finding 5). Threaded as a proper wasm-exposed dev flag (`setFimSingleCellProducerFraction`,
`frontend.rs`, mirroring `setFimNestedWellSolve`'s pattern) rather than native-only, since X1's
own gate needs the standard wasm control matrix on the bounded cases — and because this is a
strong enough candidate to be worth promoting all the way, not just probing. Native repro driver
gained the matching `FIM_SINGLE_CELL_PRODUCER_FRACTION` env var. One correction during
implementation: the native-only setter was initially given the same name as the wasm one,
which doesn't compile (duplicate inherent method across `impl` blocks) — resolved by keeping
only the wasm-exposed `pub` version, callable from the native test driver too.

**Heavy-case result — decisive.** Native repro driver
(`repro_water_pressure_12x12x3_opm_aligned_no_trace`), clean uncapped runs (no windowing, no
trace file, verified twice for reproducibility):

| Configuration | `accepted_substeps` | `advanced_dt` | wall time |
|---|---|---|---|
| `OpmAligned` + `nested_well_solve` (baseline, `c916c87`) | `18,015` | `1.0/1.0` | `1235.5s` |
| `OpmAligned` + `nested_well_solve` + `single_cell_producer_fraction` | **`16`** | `1.0/1.0` | **`3.05s`** |
| `OpmAligned` alone (no `nested_well_solve`) + `single_cell_producer_fraction` | **`16`** | `1.0/1.0` | **`2.95s`** |

**A ~1126x reduction in substep count, ~400x wall-clock speedup, and the fix works identically
with or without `nested_well_solve`** — confirming X0's finding that the well-update-ordering
mechanism was never the primary defect. `linear_bad=0 nonlinear_bad=1 mixed=0` in the fixed run
(vs baseline's `linear_bad=8 nonlinear_bad=1 mixed=5`-class retry pattern even within a
1000-substep capped window) — the fix doesn't just shorten the run, it makes it clean. `16`
substeps for a 1-day interval is in OPM's own efficiency class (OPM: 11 Newton iterations in a
*single* substep for the same interval, D3) — ResSim now needs a handful of substeps rather than
tens of thousands.

**Isolation and no-regression checks, all wasm-based (`fim-wasm-diagnostic.mjs`)**:
- Full 6-command standard control matrix, flag OFF: **bit-identical** to the recorded baseline on
  every field (`20x20x3`=`8`, `22x22x1`=`4`, `23x23x1`=`4`, `gas-rate 20x20x3`=`2`, `gas-rate
  10x10x3` 6-step outer = `4,2,2,2,2,2`, heavy `12x12x3`=`52`/`51+3+2060`) — the new code path is
  fully inert when the flag is unset, as expected (`perforation_control_cells`'s new branch is an
  `||` addition to the existing injector check, structurally cannot fire when the flag is false).
- `22x22x1`/`23x23x1` under `--opm-aligned`, flag ON vs OFF: **bit-identical**
  (`24`/`24` substeps, `12`/`12`, matching `avg_p`/`oil`/`inj`/`retry_dom` exactly) — the fix
  changes nothing on cases that don't hit this specific pre-breakthrough-corner-producer
  scenario, no regression risk apparent on the cases already exercised regularly.
- `assembly_ad` parity: 10/10.
- Locked smoke, 3/3: `drsdt0_base_rs_cap_flashes_excess_dissolved_gas_to_free_gas`,
  `spe1_fim_first_steps_converge_without_stall` (`218.5s`), `spe1_fim_gas_injection_creates_free_gas`
  (`432.5s`) — all pass with the code change in the tree (flag defaults off, but this exercises
  the changed `perforation_control_cells` function's injector/producer branch structure).

**Remaining gates, completed** (the sandbox environment was severely CPU-throttled during this
checkpoint — observed ~1:60 CPU-time-to-wall-clock ratio on the background `cargo test` process,
several hours wall-clock for the `fim` bucket alone — but all gates below did complete and pass):
- `bash scripts/validate-solver-coverage.sh fim`: **8/8 pass** — `fim::tests::spe1::` (the same 2
  tests already verified individually above, plus this run confirmed no others in that filter),
  `fim::tests::wells::` (3 tests, including `single_cell_producer_reporting_matches_local_source_state`
  — directly relevant to this change, passes), 3 `dep_pss_fim_*` depletion tests.
- `bash scripts/validate-solver-coverage.sh shared`: hit a **pre-existing, unrelated** failure
  partway through (`closed_system_public_step_keeps_same_water_inventory_on_both_solvers`, `assert_eq!
  (fim.2, 1)` — left `2`, right `1`; the script's `set -euo pipefail` stops at first failure).
  Verified pre-existing via `git stash` (reproduces identically on the clean `bb23c81` tree,
  before any Bundle X X1 changes) — and structurally cannot be caused by this change regardless:
  the test constructs a well-less closed system (`ReservoirSimulator::new(4,4,1,0.2)`, no
  `add_well` calls), so `perforation_control_cells` is never invoked (zero perforations). Ran the
  remaining 11 tests in the bucket individually past that point: **11/11 pass**
  (`simple_pressure_control_public_step_has_same_stable_contract_on_both_solvers`,
  `shared_block_multiwell_public_step_remains_finite_on_both_solvers`, 4
  `physics_depletion_*`/`physics_waterflood_*`/`physics_gas_flood_*` contract tests,
  `physics_gas_cap_vertical_column_fim_matches_impes_hydrostatic_benchmark`,
  `physics_wells_sources_gas_injection_surface_totals_match_target_on_both_solvers`, 2
  `physics_geometry_*` tests). The pre-existing failure itself is a new discovery worth a TODO
  entry (its symptom — an extra `rate_history` entry on the FIM path for a well-less closed
  system — closely resembles the already-known-and-tracked "3 pre-existing failures found
  2026-07-07" class in `TODO.md`, though not an exact name match; not investigated further here,
  out of scope for Bundle X).
- `benchmark_buckley`: **3/3 pass** (`benchmark_buckley_leverett_case_a_favorable_mobility` rel_err
  `0.041`, `case_b_more_adverse_mobility` rel_err `0.090`, `smaller_dt_improves_coarse_alignment`
  — all within the existing validated tolerances, untouched).

**X1 verdict: PROMOTABLE.** Every gate in the plan's X3 promotion checklist that doesn't require
the full uncapped heavy re-run (done above, `16` substeps) or the D3 oracle comparison (X3's own
remaining item) is green. This is the rare case of a single-function fix (`perforation_control_cells`,
one `||` condition added) resolving what had been, across the whole `FIM-BUNDLE-N`/`FIM-BUNDLE-W`/
`FIM-DIAG-002`/`FIM-DIAG-003` arc, an ~18,000-substep catastrophic failure — because the arc had
been chasing a downstream symptom (the well-cell MB freeze) of an upstream formula-fidelity gap
that had never been compared against OPM's actual source until D3/X0 did so directly.

### Bundle X checkpoint X3: D3 oracle re-comparison, generality checks, stack promotion (2026-07-12)

Per `docs/FIM_BUNDLE_X_PLAN.md` X3. Commit base `9bb4925`.

**D3 oracle re-comparison.** Full unwindowed `LEDGER` trace of the fixed run
(`OpmAligned`+`nested_well_solve`+`single_cell_producer_fraction`, `FIM_TRACE_SUBSTEP_START=0`,
no cap needed — only 16 substeps): dt schedule climbs cleanly from `0.0825` to `0.259` days,
reaching the exact `dt≈0.185`-class step the original D3 plan asked about
("does OPM hold `dt=0.185`-class steps at 2-3 iterations where we collapse?") — substep 12 covers
`t=0.313→0.498` at `dt=0.185031` in **7 iterations**, substep 13 covers `t=0.757→1.0`-ish at
`dt=0.259044` in **12 iterations**. `mb` values throughout: `1e-8`-`1e-10` range, comfortably
under strict tolerance — no plateau anywhere in the trace. Total Newton iterations across all 16
substeps: **168** (summing the `iters` column) vs OPM's **11** in its single one-substep solve
(`docs/FIM_CONVERGENCE_WORKLOG.md` "FIM-DIAG-003 checkpoint D3"). ResSim is now firmly in a
*functional* regime — clearing the `≤35`-substep gate by more than 2x — but still ~15x less
iteration-efficient than OPM per unit of simulated time, consistent with `OpmAligned`'s known,
separately-tracked per-iteration cost gap (Bundle P's own wall-clock attribution work) and not
something this fix was aimed at closing.

**Generality checks (not required by the plan, run because the finding was too clean not to
stress further):**

1. **Legacy flavor benefits too.** `water-pressure 12x12x3 --dt 1` under Legacy (no
   `--opm-aligned` at all) + `single_cell_producer_fraction`: **`52 → 25` substeps**. Smaller
   improvement than `OpmAligned`'s `18,015 → 16` (Legacy's Appleyard-damping retry ladder was
   already tolerating the old formula's residual, just at higher cost — `OpmAligned`'s strict
   CNV/MB gate could not), but a real, unconditional improvement — confirms the fix is a genuine
   physics-formula correction, not something that only matters under a specific Newton-loop
   flavor. Reported production numbers shift (`oil=3887.33 → 2900.00` at this snapshot) — expected
   and correct, not a regression: the old numbers included a manufactured water-fraction leak at
   the producer that this fix removes.
2. **`water-medium-6step` (`water-pressure 20x20x3 --steps 6 --dt 0.25`), a second,
   independently-discovered broken case.** Not part of the standard 6-command control matrix, but
   checked per the plan's own X3 item 4 ("a producer that sees breakthrough mid-run"). Baseline
   (flag off): steps 1-4 clean (`8/3/4/5` substeps), but **steps 5-6 exhibit the same
   plateau-replay-explosion pathology** as the heavy case (`accepts=3+5+1018`, then
   `accepts=1+5+2042` — `real_accepted_substeps` collapsing while `accepts` balloons via the same
   ledger-collapsing mechanism D4 diagnosed for `FIM-DIAG-003`). Reported `oil` **freezes** at
   `3560.89` identically across both stuck steps — itself evidence the run wasn't making genuine
   progress. With the fix: steps 5-6 resolve cleanly (`8`/`3` real substeps, `accepts=8+0+0`/
   `3+0+0`), and reported `oil` **continues climbing** (`3543.90 → 3578.05 → 3599.27`) instead of
   freezing. Steps 1-4 (pre-breakthrough, before the fix's mechanism is even exercised) are
   near-identical between the two runs (`oil` differs by `<0.1` at every step) — the fix changes
   nothing until the physics it corrects actually matters, exactly as expected.

**Stack-level Bundle N §5 promotion decision.** The original gate (heavy `≤35`-substep class +
fine-dt FOPT + control matrix + bounded cases not worse than Legacy) had TWO independent parts,
and this fix resolves only one of them cleanly:
- **Heavy-case substep class: PASSED, decisively** (`16 ≤ 35`, and `16` is also *better* than
  Legacy's own `52` — the fix doesn't just clear the bar, it makes `OpmAligned` the better choice
  on this specific case).
- **Bounded cases "not worse than Legacy": still open, unrelated to this fix.** Re-confirmed
  unchanged by `single_cell_producer_fraction` (bit-identical on `22x22x1`/`23x23x1` with the flag
  on or off): `OpmAligned` alone was already costlier than Legacy on the bounded cases *before*
  this fix (`docs/FIM_STATUS.md` "Bundle N" section, recorded at the time as "close on attempts,
  not yet better") and remains so — `20x20x3` `8→15`, `22x22x1` `4→24`, `23x23x1` `4→12`,
  `gas-rate 20x20x3` `2→459`. This is a pre-existing, separately-tracked characteristic of
  `OpmAligned`'s more conservative per-cell chopping / stricter CNV-MB acceptance vs Legacy's
  Appleyard-damping ladder — not something `FIM-BUNDLE-X` was ever scoped to fix, and this fix
  neither helps nor hurts it (structurally cannot, per X0's finding that the mechanism only fires
  near a pre-breakthrough producer's saturation-bound collision, which these bounded cases don't
  hit in a way that changes their substep count).

**Verdict, split into two independent decisions**:
1. **`single_cell_producer_fraction` itself: PROMOTABLE now, independent of the
   `OpmAligned`/`nested_well_solve` stack question.** It is a physics-fidelity bug fix (matches
   OPM's actual formula exactly), strictly improves every case tested under every flavor
   combination, and has zero observed regression. This is the kind of fix the "systemic steer"
   guidance favors (fix the OPM-inconsistent base, not another mechanism layered on top of it).
2. **The `OpmAligned`+`nested_well_solve` stack (the original Bundle N §5 question — should this
   *become the default*, replacing Legacy): still NOT closed.** The heavy-case blocker that
   stalled it across `FIM-BUNDLE-N`/`FIM-BUNDLE-W`/`FIM-DIAG-002`/`FIM-DIAG-003` is now removed,
   but the bounded-case cost tradeoff (never this bundle's target) remains as the standing
   obstacle to full stack promotion. That is a distinct, pre-existing question, appropriately
   left for its own future work rather than folded into this bundle's scope.

**Open product question, not a technical one: should `single_cell_producer_fraction` become
unconditional (delete the flag, always match OPM) rather than stay an opt-in dev flag?** It
changes reported production numbers on any FIM scenario where a producer's 3x3 neighborhood
differs from its own cell's saturation (i.e. once water is near but hasn't reached a producer) —
this is more physically correct, but is a default-behavior change for the public-facing FIM path
(FIM is currently dev-only per `docs/FIM_DEFERRED_BACKLOG.md`, which lowers the stakes, but
worth an explicit decision rather than a silent default flip). Deferred to the user.

**User decision: make it unconditional.** Delete the flag; the OPM-matching single-cell formula
always applies.

### Bundle X checkpoint X4: make the fix unconditional, second duplicate found and fixed (2026-07-12)

Commit base `473f754`.

**Implementation.** `perforation_control_cells` (`fim/wells.rs:822`) simplified to always return
`vec![perforation.cell_index]` — the 3x3-window branch, `i_min`/`i_max`/`j_min`/`j_max` loop, and
the `fim_single_cell_producer_fraction` condition all deleted. Removed the flag entirely: the
`ReservoirSimulator` field (`lib.rs`), its constructor initialization (`frontend.rs`), the
wasm-exposed setter `setFimSingleCellProducerFraction` (`frontend.rs`), the native repro driver's
env-var wiring (`fim/timestep.rs`), and the `--single-cell-producer-fraction` CLI flag
(`scripts/fim-wasm-diagnostic.mjs`) — confirmed zero remaining references via `grep` across
`src/` and `scripts/`.

**A second, independent duplicate found and fixed.** Rebuilding immediately surfaced an
`assembly_ad` parity failure (5/10 tests, e.g. `residual[3] diverged: old=-3.499... new=-3.589...`)
— the legacy assembler (`assembly.rs`, the frozen bit-parity oracle, Phase 6) and the live AD
assembler (`assembly_ad.rs`) now disagreed. Root cause: `producer_control_state`
(`fim/wells.rs:786`, called by the legacy assembler's `perforation_component_rates_sc_day` *and*
by `reporting.rs`'s water-cut reporting) was a **second, wholly independent copy** of the same
pre-fix 3x3-areal-neighborhood mobility-summing logic that X0/X1 found and fixed in
`perforation_control_cells` — never touched by the X1 flag, since it doesn't call through that
function at all. Fixed in lockstep (same single-cell formula). This is exactly the kind of
divergence the AD/legacy parity gate exists to catch (`engine-physics-change` skill: "physics
helpers often have both a legacy and an AD implementation that must be changed together") — and
it caught it on the first rebuild, before any live run.

**Full re-gate, all green** (commands and counts recorded verbatim per baseline discipline):
- `assembly_ad` parity: **10/10** (restored after the `producer_control_state` fix).
- Locked smoke, 3/3: `drsdt0_base_rs_cap_flashes_excess_dissolved_gas_to_free_gas`,
  `spe1_fim_first_steps_converge_without_stall` (`85.7s`), `spe1_fim_gas_injection_creates_free_gas`
  (`113.5s`).
- `validate-solver-coverage.sh fim`: **9/9** (`fim::tests::spe1::` 3 tests including
  `spe1_fim_producer_gas_breakthrough_smoke`, `fim::tests::wells::` 5 tests including
  `single_cell_producer_reporting_matches_local_source_state`, 1 `dep_pss_fim_*` — full bucket,
  `202.8s`).
- `validate-solver-coverage.sh shared`: all 14 non-pre-existing-failure tests run individually
  (the script's `set -euo pipefail` stops at the known, unrelated
  `closed_system_public_step_keeps_same_water_inventory_on_both_solvers` failure — still
  reproduces, still structurally unrelated, `TODO.md` entry stands) — **14/14 pass**.
- `benchmark_buckley`: **3/3 pass**, tolerances and `rel_err` values unchanged from the pre-fix
  baseline (`0.041`/`0.090`/dt-sweep) — this benchmark's producer configuration doesn't exercise
  the fixed mechanism.

**New unconditional baseline, control matrix** (supersedes the pre-`FIM-BUNDLE-X` baseline
recorded throughout `FIM-DIAG-003`/prior bundles — commit `473f754`'s tree plus this checkpoint's
changes, wasm rebuilt):

| Case | Old baseline (pre-`FIM-BUNDLE-X`) | New baseline (this checkpoint) |
|---|---|---|
| `water-pressure 20x20x3 --dt 0.25` | `8` substeps | **`8`** (unchanged; `oil` drifts `3340.56→3340.66`, 4th sig fig) |
| `water-pressure 22x22x1 --dt 0.25` | `4` | **`4`** (unchanged, `oil` bit-identical `1473.47`) |
| `water-pressure 23x23x1 --dt 0.25` | `4` | **`4`** (unchanged; `oil` drifts `1454.56→1454.60`) |
| `gas-rate 20x20x3 --dt 0.25` | `2` | **`2`** (unchanged, bit-identical) |
| `gas-rate 10x10x3 --steps 6 --dt 0.25` | `4,2,2,2,2,2` | **`4,2,2,2,2,2`** (unchanged substep counts; step 2's dominant retry cell shifts `oil@64→oil@37`, a non-blocking retry-site relabel, not a regression) |
| `water-pressure 12x12x3 --dt 1` (heavy) | `52` | **`25`** (superseded — the fix's own result, `oil` drifts substantially `3887.33→3182.85` since removing the spurious water withdrawal changes total oil accounting materially for the case that goes deepest into the fixed mechanism) |

Every non-heavy case's substep count is bit-identical to the pre-fix baseline; only cosmetic
(4th-significant-figure) production-number drift appears where a producer sits anywhere near a
saturation front, confirming the fix's blast radius is exactly and only what X0/X1/X3 predicted.
The heavy case's `52 → 25` is the fix applying to the *default* (Legacy) flavor for the first
time (X1/X3 measured this behind the flag; this checkpoint makes it the recorded baseline).

**`FIM-BUNDLE-X` fully closed.** No dev flag remains — `perforation_control_cells` and
`producer_control_state` unconditionally match OPM's single-connected-cell mobility formula for
every well, injector or producer, under every code path (legacy assembler, AD assembler, nested
well solve, reporting).

## Bundle Y planned: OPM convergence parity (2026-07-12)

Plan doc: `docs/FIM_OPM_PARITY_PLAN.md`. Post-X4 `OpmAligned` baselines re-derived for the plan
on the clean committed tree at `53cae5c` (wasm rebuilt in the X4 commit), exact commands +
verbatim key fields:

```
node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 12x12x3 --steps 1 --dt 1 --opm-aligned --nested-well-solve --diagnostic summary --no-json
  substeps=17 | accepts=17+0+0 | retries=2/1/0 | dt=[5.280e-3,2.387e-1] | retry_dom=linear-bad:oil@1099 | oil=2853.81
node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 20x20x3 --steps 1 --dt 0.25 --opm-aligned --diagnostic summary --no-json
  substeps=16 | retries=6/1/0 | retry_dom=linear-bad:oil@2716
node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 22x22x1 --steps 1 --dt 0.25 --opm-aligned --diagnostic summary --no-json
  substeps=24 | retries=8/0/0 | dt=[2.813e-5,5.591e-2] | retry_dom=linear-bad:oil@1450
node scripts/fim-wasm-diagnostic.mjs --preset water-pressure --grid 23x23x1 --steps 1 --dt 0.25 --opm-aligned --diagnostic summary --no-json
  substeps=12 | retries=1/0/0 | retry_dom=linear-bad:oil@1585
node scripts/fim-wasm-diagnostic.mjs --preset gas-rate --grid 20x20x3 --steps 1 --dt 0.25 --opm-aligned --diagnostic summary --no-json
  substeps=459 | retries=337/0/0 | dt=[7.032e-5,9.539e-4] | retry_dom=linear-bad:oil@1261 | wall 83.9s
```

Notable observations feeding the plan:
- Heavy `OpmAligned`+`nested_well_solve` reads `17` on this re-run vs X1's `16` — within this
  case's known ±1 chaos band, recorded honestly; `16` stays the X1-checkpoint number, `17` is
  the plan-baseline number at `53cae5c`.
- **The gas-rate 459 catastrophe is untouched by the X fix** (bit-identical substep count and
  retry profile before/after) — and its dominant failure class is `linear-bad` (337 of 459
  substeps end in linear retries, dt pinned at `~1e-4`). Combined with `FIM-LINEAR-005`'s
  offline finding (current `row0-schur` restriction converges 0/54 heavy systems as full
  solves; `sum-rows`/`quasi-impes` ~92%), the linear stack is now the leading suspect for both
  the bounded-case overhead and the remaining transient inefficiency — Y0 measures, Y1
  promotes the restriction variant per the standing OPEN registry row.
- Every `OpmAligned` bounded case's `retry_dom` is now `linear-bad` — under Legacy the same
  cases fail `nonlinear-bad` first. `OpmAligned`'s stricter acceptance has shifted the binding
  constraint from the nonlinear layer to the linear layer across the whole matrix, which is
  exactly the regime OPM's `cprw` (quasi-IMPES CPR) is built for.

### Bundle Y checkpoint Y0: transient + gas-rate differential diagnostics (2026-07-12)

Full writeup with source citations, replay commands, and raw trace excerpts:
`docs/FIM_OPM_PARITY_PLAN.md` §6. Tree `b2dd34a` plus one additive native-test-only diff (new
`repro_gas_rate_20x20x3_opm_aligned_no_trace` driver in `fim/timestep.rs`, needed because the
wasm runner can't host `fim::trace_sink`'s file trace) — validated bit-identical to the
recorded gas-rate baseline (`459/337/0/0`) before being trusted.

Summary of the two findings (both diagnostic only, no fix attempted):
- **Heavy substep 0** never converges at trial `dt=0.25` — the injector-connected cell's water
  saturation genuinely 2-period-oscillates (`sw` bouncing `0.1↔0.5`-ish every iteration) rather
  than converging slowly. Checked the OSC-DETECT/per-cell-chop composition against OPM's real
  pinned source (`blackoilnewtonmethod.hpp`, `NonlinearSolver.cpp`,
  `NonlinearSystemBlackOilReservoir_impl.hpp`) line by line: order, constants
  (`dsMax=0.2`/`relaxMax=0.5`/`relaxIncrement=0.1`/`relaxRelTol=0.2`), and scope all match —
  the damping/chop port is faithful, not the culprit. Divergence is upstream (raw pre-damping
  Newton step magnitude at the well-connected cell), not yet localized to G2 vs G4.
- **Gas-rate 459** is a steady-state grow→fail→retry→accept limit cycle spanning the *entire*
  run (not a transient window), zero `nonlinear-bad` retries, dominant retry site drifting
  across near-front/well rows over time (histogram in the plan doc). Windowed trace shows
  `linear-bad` firing on iterations whose nonlinear state (`CNV-MB`) is *already* strictly
  converged — the linear solve's own internal `converged` flag
  (`classify_retry_failure_with_site`, `newton.rs:345`) is stricter than the nonlinear need,
  not a bad search direction. This is FIM-DIAG-003's "H2 — linear-precision floor" hypothesis,
  live again on a *different* case/site/symptom than where it was refuted — that refutation
  does not transfer.
- **Masking caution (explicit, per standing program lesson)**: both traces show the CPR linear
  solve running very few applications (`apps=2-4`) per Newton iteration on both cases. Flagged
  a plausible-but-unconfirmed unifying hypothesis — one under-resourced CPR linear solve could
  produce both symptoms (poor raw direction under large dt; over-strict internal gate on
  otherwise-good solves) — without asserting it. Y1 must re-measure both traces before/after
  the CPR restriction swap, not just its own target metrics, before declaring G1 resolved.

Verdict: Y1 (`FIM-LINEAR-005` promotion) remains the correct next lever, now with two
mechanism-level reasons instead of one plausibility argument. No code changes this checkpoint.

### Bundle Y checkpoint Y1 attempt: promotion already live, evidence-gathering gap found (2026-07-12)

Full writeup: `docs/FIM_OPM_PARITY_PLAN.md` §7. Set out to execute Y1 ("promote `sum-rows` or
`quasi-impes` to the live CPR path") and found, via direct source read + `git blame`, that
`quasi-impes` has been the live restriction since commit `77ec900e` (2026-07-05) — a week
before this plan was written — and that the loosened-tolerance/block-ILU0 bundle
(`FIM-LINEAR-008`) and well-Schur elimination (`FIM-LINEAR-010`) are also both live defaults.
`docs/FIM_EXPERIMENT_REGISTRY.md`'s `FIM-LINEAR-005` row said `OPEN`; corrected to `PROMOTED`.
Every number in this plan's §1/§6 baseline tables was already measured with the full bundle
active — Y0's findings describe what's still wrong *after* all three linear-stack levers, not
before.

Tried to do the honest version of Y1 anyway (re-capture fresh `OpmAligned` corpora on the
current tree, per the plan's own "re-capture since trajectories changed" instruction) and hit a
structural wall: `fim/newton.rs`'s `FIM_CAPTURE_DIR` write call lives entirely inside `if
!opm_aligned { ... }`. `OpmAligned`'s own linear-failure path (a separate, no-fallback-ladder
branch matching OPM's `NumericalProblem`-throw semantics) has no capture call at all. Confirmed
empirically: capturing the gas-rate case (337 `linear-bad` retries under `OpmAligned`) produced
**zero** files. Every prior linear-stack promotion (`FIM-LINEAR-005/008/010`) was validated
exclusively against `Legacy`-flavor captures — the flavor Bundle Y is not targeting. Separately,
the lab's comparison harness bypasses `well_schur::solve_with_well_elimination` (calls
`gmres_block_jacobi` directly), so it wouldn't reflect the live code path even on a same-flavor
rerun today.

One loose thread noticed, not chased: an `OpmAligned` abort trace shows `reduction=n/a`
(`failure_diagnostics: None` with `converged: false`) — every `None`-diagnostics site found in
`gmres_block_jacobi.rs` is actually on a `converged: true` path, so this system likely went
through `well_schur::solve_with_well_elimination` instead; that wrapper's failure/recovery path
was not read this session. Possibly the literal mechanism behind §6.2's "linear-bad fires on an
already-converged nonlinear state" — flagged for whoever picks this up.

No code promoted. Options for the real next step (not decided): (1) add an `OpmAligned`-side
capture hook and re-run the offline lab through `solve_linearized_system` on fresh corpora
before trusting any further restriction/tolerance tuning; (2) chase the `reduction=n/a` thread
directly; (3) set the linear stack aside and work `G1` (heavy-case raw-Newton oscillation, which
has almost no linear failures to capture in the first place) via a different route. Two scratch
capture directories exist (`/tmp/.../fim-y1/{heavy,gasrate}-capture`), not durable.

### Bundle Y checkpoint Y1a/Y1b: the gas-rate catastrophe is not a linear-solve problem (2026-07-12)

Full writeup: `docs/FIM_OPM_PARITY_PLAN.md` §8. Registry: `FIM-LINEAR-012` (DIAGNOSTIC). Picked
option 1 from the prior checkpoint's list. Two additive, no-op-verified changes: an
`OpmAligned`-side capture call in `fim/newton.rs` (mirrors the existing Legacy-only one), and an
`FIM_WELL_SCHUR_DEBUG` diagnostic print in `fim/linear/well_schur.rs`.

Captured fresh `OpmAligned` corpora on the current tree: heavy `1` system, bounded `23x23x1` `1`
system (both consistent with §6.1 — those cases barely fail on the linear axis anymore),
gas-rate **`337`** (matches its live `linear_bad` count exactly).

Re-ran `solver_lab_compare_restriction_variants` on the fresh gas-rate corpus: `quasi-impes`
still wins decisively (converged `336/337`, wins `220/337`, median relative residual `1.13e-4`,
5-6x better than the next-best) — the already-live restriction choice holds up on real
`OpmAligned` data for the first time.

Then ran `solver_lab_compare_backends`, which calls the real `solve_linearized_system`
dispatcher (well-Schur elimination included) — i.e. tests **today's exact live configuration**
end to end. Result: **`fgmres-cpr` converges 337/337 offline**, `iters=2` each, relative
residuals `~2e-5`-`7e-5`. Every system captured at the exact moment it was classified as a live
failure solves cleanly when replayed in isolation with identical inputs. The lab's own "stop
condition 2" assertion (built for the old Legacy corpora, expecting captured failures to
reproduce offline) panics on this corpus — correctly, since the premise it was checking no
longer holds for this flavor. **The gas-rate catastrophe is not a linear-solve-quality problem.**

Mechanism, localized but not fully pinned: `FIM_WELL_SCHUR_DEBUG` on a live windowed trace shows
a repeating pattern — the *reduced* (well-eliminated) solve reports `converged=true` at
`reduced_iters=0` (the `x_0=0` trivial guess spuriously satisfies its own tolerance check) while
the recovered full-system residual stays at essentially the original `rhs_norm` (200x over
tolerance). `well_schur.rs`'s safety-net check correctly downgrades this to `converged=false`,
but forwards `failure_diagnostics: reduced_report.failure_diagnostics` unchanged — `None`,
because the reduced report claimed success. `OpmAligned`'s abort path can't compute a
`reduction` without `failure_diagnostics`, so it hard-aborts unconditionally — discarding a
linear solve that (per the offline replay) was actually fine. Not yet correlated line-by-line to
a specific live abort (the two trace streams weren't cross-referenced by iteration this
session), so treat as strongly evidenced, not exhaustively proven — flagged per the standing
masking caution. Root cause of the spurious zero-iteration "convergence" itself is unresolved:
either the reduced system's own tolerance check uses a mismatched RHS-norm basis, or the
elimination/recovery arithmetic produces a degenerate near-zero `dx_tail`.

Does not explain §6.1 (heavy-case oscillation — that case has almost no linear failures to
trigger this path at all). §6.4's "one root cause wearing two masks" hypothesis is weakened by
this finding, not confirmed — G1 and G2 look like genuinely separate problems now.

No fix implemented — deliberately. Two candidate fixes named in the registry row; both need the
correlation gap closed first before changing live acceptance behavior, since a wrong fix here
would change what `OpmAligned` accepts across every bounded/gas case, not just the target one.

### Bundle Y checkpoint Y1d: correlation closed, root cause re-localized — both original candidates wrong (2026-07-12)

Full writeup: `docs/FIM_OPM_PARITY_PLAN.md` §8.6. Registry: `FIM-LINEAR-012` updated. Closed
both items §8.4 left open: the correlation gap, and the (a) tolerance-basis-mismatch vs (b)
degenerate-elimination-arithmetic discrimination.

Made `FIM_WELL_SCHUR_DEBUG`'s print also write through `trace_sink::write_line` (was
`eprintln!`-only, an unordered separate stream), and added a new `CPR-ACCEPT-DEBUG` print at
`gmres_block_jacobi.rs:1651` (the `beta <= tolerance && family_ok(&residual)` branch — the
preconditioned-residual accept check, gated to fire only when the raw residual is >10x
tolerance so clean convergences don't spam the trace). Both additive, no-op-verified
(`assembly_ad` 10/10). Ran the native gas-rate repro test once with `FIM_TRACE_FILE` +
`FIM_WELL_SCHUR_DEBUG=1` set together so both prints interleave with the existing `LEDGER
retry` lines in one file, in true call order. Reproduced `459/337` exactly, confirming the
instrumented run is representative.

**Correlation, checked programmatically over all 3,066 trace lines**: 337 `CPR-ACCEPT-DEBUG`,
337 `WELL-SCHUR-DEBUG reduced_iters=0`, 337 `LEDGER retry retry_class=linear-bad` — and every
single `LEDGER retry` line is immediately preceded by its matching `WELL-SCHUR-DEBUG
reduced_iters=0` line, immediately preceded by its matching `CPR-ACCEPT-DEBUG` line. Zero
exceptions. One mechanism accounts for every gas-rate `linear-bad` retry in this run — the
§8.4 correlation gap is closed exhaustively, not just strongly evidenced.

**Discrimination: both original candidates refuted.**
- (b) elimination/recovery arithmetic: refuted by inspection (exact Schur-complement linear
  algebra, independently checked by the very safety net that catches this bug).
- (a) tolerance-basis mismatch (reduced vs full `rhs.norm()`): real but negligible — measured
  across all 337 pairs, relative difference is median 0.00%, max 3.69%. Cannot explain a 200x
  gap; downgraded from "favored" to "minor contributor."
- **Actual mechanism**: `gmres_block_jacobi.rs:1651` accepts convergence on the *preconditioned*
  residual `beta` alone, at `iterations == 0` (one preconditioner application to the untouched
  `x=0` residual, before any Krylov step) — no raw-residual floor guards this branch. Measured:
  `tolerance/beta` median 12.5x, max 31.2x (comfortably "passes"); `residual_norm/tolerance` is
  a constant `200.0x` in every one of the 337 cases (`= 1/relative_tolerance`, confirming this
  is a fixed structural ratio, not noise). The preconditioner isn't malfunctioning — it
  genuinely shrinks the preconditioned norm of these particular residuals — but `beta` alone is
  not a trustworthy solution-quality proxy for well-Schur-*reduced* systems specifically. Why
  "reduced" and not the ordinary full system triggers this is not established (same generic
  function serves both paths); flagged as an open follow-up, not investigated this checkpoint.

No fix implemented — `gmres_block_jacobi.rs:1651` is shared by every `FgmresCpr`/`GmresIlu0`
solve in the codebase, not just well-Schur-reduced ones, so a guard added there needs its own
measurement pass before being written.

### Bundle Y checkpoint Y1e: the accept-check fix, measured and promoted (2026-07-12)

Full writeup: `docs/FIM_OPM_PARITY_PLAN.md` §9. Registry: `FIM-LINEAR-013` (PROMOTED). The
measurement pass Y1d's own writeup called for before touching `gmres_block_jacobi.rs:1651`.

Fix: `beta <= tolerance && family_ok(&residual)` → `iterations > 0 && beta <= tolerance &&
family_ok(&residual)`. At `iterations == 0`, `solution` is provably still the untouched `x_0 =
0` initial guess — no Krylov correction has run yet — so accepting there on the preconditioned
residual alone returns the zero vector as "the solution" regardless of whether `x_0 = 0` is
actually close to correct. One-condition change, generic (not well-Schur-specific), correctness-
neutral at `iterations >= 1` (unchanged there).

Added a synthetic regression test (`beta_only_accept_never_fires_on_the_untouched_initial_guess`,
`fim/linear/gmres_block_jacobi.rs`): two independent 3x3 diagonal blocks, block 0's diagonal
inflated to `1e6` vs block 1's `1.0`, all RHS mass in block 0. Block-Jacobi's exact per-block
inverse crushes `beta` to `~1.7e-6` in one application while the raw residual at `x_0=0` stays
at `~1.732` — ~200x over the default tolerance, the same order as the live corpus. Confirmed
by temporarily reverting the guard that the test fails without it and passes with it.

The 337-system gas-rate capture corpus from Y1a/Y1b/Y1d turned out unusable for an offline
before/after comparison: `solver_lab.rs::run_backend` builds `FimLinearSolveOptions::default()`
rather than the live Newton loop's actual options, so it never reproduced the live
tolerance/`beta` relationship that triggers the bug — this resolves Y1d's open question of why
offline replay of the same live-failed systems converged cleanly (`iters=2`) while live didn't:
different options, not a capture-fidelity problem. Validated live instead.

**Live measurement** (native `repro_gas_rate_20x20x3_opm_aligned_no_trace`, `--release`):
`linear_bad` `337 → 1`, `nonlinear_bad` `0 → 4` (new but small and expected — Newton iterations
that previously got a rejected linear solve now get the genuinely-converged one, and a few need
slightly more nonlinear massaging), `accepted_substeps` `459 → 238`. `linear_bad` collapsing to
near-zero is direct confirmation this was the dominant driver of the storm, not correlation.

**Full control matrix, wasm rebuilt, zero regressions**: Legacy `8/4/4/2` bit-identical; Legacy
heavy (`dt=1`) `25` substeps bit-identical, same `retry_dom`/rung breakdown; `OpmAligned`
bounded `16/24/12` bit-identical; `OpmAligned` bounded `12x12x3`+`nested_well_solve` `12`
unchanged — consistent with these cases barely exercising the well-Schur-reduced `linear-bad`
path in the first place (§8.1). Gates: `assembly_ad` 10/10, full `fim::linear` module suite
37/37 (8 offline-lab tests correctly `ignored`, need `FIM_CAPTURE_DIR`), locked smoke 3/3.

Does not close G2 on its own — gas-rate `OpmAligned` is still `238` substeps vs Legacy's `2` —
but removes a measured, non-physical source of retries; the remaining gap is more likely to
reflect genuine behavior now, not this artifact. Does not touch G1 (heavy case's linear failure
count was always too small for this bug to matter there).

### Bundle Y checkpoint Y1f: chasing `nonlinear_bad=4` — closed, benign (2026-07-12)

Full writeup: `docs/FIM_OPM_PARITY_PLAN.md` §9.1. Registry: `FIM-LINEAR-013` updated. Closed
the one open follow-up Y1e's own writeup flagged.

Reran the native gas-rate repro test post-fix with `FIM_TRACE_FILE` set, diffed its `LEDGER`
lines against the pre-fix trace already captured for Y1d (same test, `ccbcf37`, before
`FIM-LINEAR-013`). Reproduced `238/1/4` exactly first.

All 4 `nonlinear_bad` retries turned out to be one event, not four: the very first substep of
the run (`substep=0, t=0`), across its own `retry_count=1..4` ladder. Before the fix, this exact
substep needed **7** retries (all `linear-bad`, `mb=inf` on every one — the signature of the
`FIM-LINEAR-012` bug firing on every single attempt, no real Newton work ever happening) before
accepting at `dt=0.000106546`. After the fix, the same substep needs only **4** retries (now
correctly `nonlinear-bad` — genuine, finite, shrinking `mb`: `7.7e-6 → 8.6e-7 → 1.1e-7 →
1.7e-8` — close to converging but not quite clearing tolerance within the 20-iteration Newton
budget at the larger trial `dt`s) and accepts at `dt=0.002964803` — **28x larger**, in **3 fewer
retries**.

Verdict: not a regression. This is the same known "aggressive initial trial `dt=0.25` needs a
shrink ladder" behavior (G3, controller policy), now visible and correctly classified instead
of being masked by the linear-accept bug firing on every attempt. No further action.

### Bundle Y checkpoint Y1g: the real driver of `238` substeps — a well-cell Newton stall (2026-07-12)

Full writeup: `docs/FIM_OPM_PARITY_PLAN.md` §10. Continuing toward gas-rate parity per explicit
instruction, now that `FIM-LINEAR-013` has removed the linear-accept noise.

Ran the gas-rate case through `--diagnostic step` (17,918 lines, `243` substep attempts,
matching the native `238/1/4` baseline exactly) and found the `238`-substep count is a
persistent limit cycle spanning the *entire* run: `129/238` accepted substeps hit the full
`20`-iteration Newton budget, each triggering OPM's own iteration-count growth throttle
(`growth=0.400`, faithfully reproducing `opm_iteration_count_dt`'s formula exactly), alternating
with fast `iters=3` substeps that grow `3.0x`. Net ~`1.2x` per two-step cycle — the growth
policy is not a bug, it's correctly responding to genuinely high iteration counts.

Ruled out G1 (oscillation) directly: `OSC-DETECT osc_phases>0` fires on only `4`/`2,989`
iteration checks in the whole run, all from the initial transient, none from any of the `129`
`iters=20` substeps sampled afterward.

The real mechanism, confirmed on 6 widely-spaced `iters=20` substeps (8, 19, 27, 42, 65, 91,
158): the residual reaches near-tolerance within 2-3 iterations, then **freezes bit-for-bit
identical** for the remaining 14-17 iterations (`upd=5.933e-7` the *exact same value* every
iteration in substep 19's detail trace) — always just over the `1e-7` `mb` target (`1.02x` to
`2.6x` observed), always at a cell touching the injector well (cell `0`/`1`/`20`/`400`). Same
structural shape as `FIM-DIAG-003`'s original H1 "displaced well-cell standoff" finding, now
recurring at the injector under gas-rate `OpmAligned` rather than the producer Bundle X fixed.
The existing (not new) `STAGNATION-ATTRIB`/`STAG-TREND` classifier correctly detects and labels
every frozen iteration, but its only remedy (`would_widen`, requiring `trend_vs_entry < 0.5`) is
explicitly gated `if !opm_aligned` (`newton.rs:3299`) — a standing design comment
(`newton.rs:3293-3298`) states this is intentional, modeling a belief that OPM itself has no
trend-based bailout and would also just grind through the full budget.

That belief is asserted in a code comment, not verified against pinned OPM source or a real run
for this specific well-adjacent-stall scenario. Per standing "measure, don't guess a fix
mechanism" discipline, closing this needs an OPM ground-truth comparison (INFOITER differential
trajectory, the established D3/Y0 method) at one of these well-adjacent stall states — not
attempted this checkpoint, since no gas-rate-comparable OPM reference deck exists in this
working tree (existing decks live on `origin/fim-opm-continuation-plan`, a different branch).

Verdict: DIAGNOSTIC. No code changed. Next step: obtain/author an OPM gas-rate reference deck
and run the differential comparison before touching the `would_widen` `OpmAligned` exclusion.

### Bundle Y checkpoint Y1h: OPM ground truth — the design comment's premise is refuted (2026-07-12)

Full writeup: `docs/FIM_OPM_PARITY_PLAN.md` §11. Closes Y1g's open question per explicit
instruction to pursue it.

Found `opm/reference-decks/gas-rate-10x10x3/CASE.DATA` already exists on
`origin/fim-opm-continuation-plan` (`cacdf767`), already validated to run. Extracted it via `git
show` (no branch checkout, kept this tree clean) and ran real `flow 2026.04` with
`--output-extra-convergence-info=steps,iterations`. Result: **6 report steps, 6 total substeps,
zero cuts, Newton iterations `7/5/4/3/4/3`**, `MB_Oil`/`MB_Gas` decaying smoothly and
monotonically every iteration in `CASE.INFOITER` — no freeze, no plateau, no relaxed-tier
acceptance anywhere.

Ran ResSim on the matching hand-authored geometry/wells/rates/PVT mapping (`--preset gas-rate --grid 10x10x3
--dt 0.25 --steps 6 --opm-aligned`): **695 total accepted substeps** for the same 1.5 days — a
`~116x` gap on the matching input just run through real OPM (closes the grid-size
caveat from Y1g's 20x20x3-only evidence). `retries=0/5/0` — essentially zero linear-bad,
reconfirming `FIM-LINEAR-013` closed the linear side; 100% of the remaining gap is nonlinear.

**Verdict: the `newton.rs:3293-3298` design comment is refuted by direct measurement.** OPM's
real trajectory shows no stagnation of any kind on this case — the claim that `OpmAligned`'s
missing trend-based bailout faithfully models OPM's own behavior does not hold.

**Important connection surfaced**: the stagnation-bailout machinery under examination
(`newton.rs:2689-2790`, `would_widen` at `3275`, gated `if !opm_aligned` at `3299`) is the same
mechanism family as `FIM-NEWTON-004` — REVERTED, "bailout still load-bearing; prior widening
attempts regressed or no-oped," retry condition "a new root cause explains the residual plateau
and has a guarded fix." This checkpoint is that new root cause, freshly measured against real
OPM output — but scope differs from what was tried before (extending an unchanged Legacy
mechanism to `OpmAligned`, not widening it). Given `FIM-NEWTON-004` and the separately-reverted
`FIM-NEWTON-005` (a live run that didn't finish in 8+ minutes after letting an under-converged
state compound forward) both live in exactly this territory, any fix here needs its own careful,
narrowly-scoped design and the full measurement discipline — not a quick patch. Paused for
explicit direction rather than proceeding unilaterally.

No code changed. Registry row not yet filed — pending a concrete fix design to measure.

### Bundle Y checkpoint Y2d2: fixed-quasi smoother/Krylov isolation (2026-07-14)

Scope was offline diagnostic infrastructure only. Production dispatch and behavior remain
unchanged. The test-only well-Schur helper now accepts an existing fine-smoother choice while
retaining production well elimination/recovery and equation scaling. The ignored solver-lab test
replays the same production tolerance/restart/restriction contract and reports full-system norms,
reservoir/well partitions, finite status, direct-correction deltas, and actual iterations.

Corpora:

- bounded: eight preserved `22x22x1` candidate failure artifacts from clean commit `2030996`,
  `/tmp/ressim-y2d0-2030996`;
- gas counter-control: five clean-regenerated current `20x20x3` baseline artifacts from commit
  `e143c19`, `/tmp/ressim-y2d1-gas-e143c19` (238 accepted substeps, one linear and four nonlinear
  retries). The older 337-artifact corpus remains unavailable/stale.

At effective budget 30 with quasi-IMPES fixed:

| Fine smoother | Bounded strict/relaxed | Gas strict/relaxed | Bounded median reduction | Gas median reduction |
| --- | ---: | ---: | ---: | ---: |
| block-ILU0 (production) | `0/8`, `0/8` | `4/5`, `4/5` | `1.455237405e-2` | `3.645953044e-5` |
| full ILU0 | `0/8`, `0/8` | `4/5`, `4/5` | `1.578222455e-2` | `4.512725001e-4` |
| block Jacobi | `0/8`, `0/8` | `4/5`, `4/5` | `1.578222455e-2` | `4.512725001e-4` |

Production block-ILU0 is the best no-regression choice. Full ILU0 and block Jacobi are
bit-identical on these artifacts and do not explain the gap.

Holding block-ILU0 fixed:

| Corpus / effective budget | Strict/relaxed | Iterations | Median full reduction | Median direct-correction delta |
| --- | ---: | --- | ---: | ---: |
| bounded / 30 | `0/8`, `0/8` | all `30` | `1.455237405e-2` | `5.089236461e2` |
| bounded / 60 | `8/8`, `8/8` | `31,31,31,31,32,32,32,32` | `2.200737272e-6` | `9.575253468e-2` |
| bounded / 150 | `8/8`, `8/8` | identical to 60 | `2.200737272e-6` | `9.575253468e-2` |
| gas / 30 | `4/5`, `4/5` | hard artifact `30` | `3.645953044e-5` | `8.433077588e-7` |
| gas / 60 | `5/5`, `5/5` | hard artifact `31` | `3.439884116e-5` | `7.340688465e-7` |
| gas / 150 | `5/5`, `5/5` | identical to 60 | `3.439884116e-5` | `7.340688465e-7` |

The bounded failures are reservoir-only. All residuals/corrections are finite, production-path
equivalence for the block-ILU0 wrapper passes, and report norms match independent full residual
recomputation. The sharp transition is therefore real: the effective cap of 30 stops nine hard
systems one or two iterations before first convergence, while 150 adds nothing beyond 60.

Exact diagnostic commands:

```text
FIM_CAPTURE_DIR=/tmp/ressim-y2d0-2030996 FIM_Y2D1_CORPUS=bounded-22x22x1 \
  FIM_Y2D2_MODE=smoother cargo test --release --manifest-path src/lib/ressim/Cargo.toml \
  --lib solver_lab_compare_production_smoother_and_budget -- --ignored --nocapture
FIM_CAPTURE_DIR=/tmp/ressim-y2d1-gas-e143c19 FIM_Y2D1_CORPUS=gas-20x20x3-current \
  FIM_Y2D2_MODE=smoother cargo test --release --manifest-path src/lib/ressim/Cargo.toml \
  --lib solver_lab_compare_production_smoother_and_budget -- --ignored --nocapture
FIM_CAPTURE_DIR=<each directory above> FIM_Y2D1_CORPUS=<matching label> \
  FIM_Y2D2_MODE=budget FIM_Y2D2_SMOOTHER=block-ilu0 \
  cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib \
  solver_lab_compare_production_smoother_and_budget -- --ignored --nocapture
```

Verdict: **CONFIRMED OFFLINE COMPONENT CAUSE; NO PRODUCTION CHANGE**. Raising the budget would
improve these replay results but would not align the approach with OPM Flow, whose reference
Newton solves stay inside 20 iterations. The next authorized slice is Y2d3: record comparable
true/preconditioned residual histories across iterations 29-32 and determine whether the boundary
is iteration accounting or genuinely useful post-restart progress. No live run, budget promotion,
combined restriction/smoother experiment, nonlinear change, or AMG implementation is authorized.

Validation on the final Y2d2 tree: rustfmt/diff checks; all four well-Schur tests; production
block-ILU0 wrapper equivalence on every replay; exact Y2d0 replay unchanged; capture round trips
2/2; Sparse-LU report reduction; DRSDT0; both locked SPE1 tests; Buckley-Leverett 3/3; curated FIM
bucket 11/11. The shared bucket passes its first three contracts and stops at the same documented,
pre-existing closed-system `rate_history` mismatch (`left=2`, `right=1`). No wasm control matrix
was run because Y2d2 adds only `#[cfg(test)]` plumbing and makes no solver-behavior change.

### Bundle Y checkpoint Y2d3: restart history and Krylov-contract localization (2026-07-15)

Scope remained diagnostic. Added test-only per-iteration snapshots of the Givens residual
estimate, independently reapplied preconditioned residual, CPR pressure reduction, true full
residual, restart/inner indices, and candidate correction. The well-Schur lab wrapper recovers
every reduced candidate and recomputes its original-system residual. Production dispatch,
acceptance, budget, CPR configuration, and nonlinear behavior are unchanged.

The production-budget candidate at iteration 30 exactly matches the existing report on all eight
bounded failures and the hard gas failure. Thus the boundary is not iteration accounting. For the
bounded corpus, the median first-cycle true reduction is `1.455237405e-2`; the true residual is
already nearly flat after the first correction while the internal Givens estimate continues to
collapse. At iteration 30 the median ratio between actual and estimated preconditioned residual
is `1.169223907e19`. Restart two then cuts the true residual by median factor
`1.293965443e-4` and converges in one or two corrections, with median final direct delta
`9.575253468e-2`. The hard gas capture shows the same discontinuity: true reduction
`4.896722176e-2` at iteration 30 and `3.439884116e-5` at iteration 31, final direct delta
`6.8472e-8`. Residual partitions, independent full residuals, and direct references remain valid.

Two temporary coarse-stage controls were run and reverted:

| Control | Bounded at 30 | Gas at 30 | Classification |
| --- | ---: | ---: | --- |
| production iterative pressure solve (`50`, `1e-6`) | `0/8` | `4/5` | first-cycle false residual collapse |
| exact dense pressure inverse for 484 rows | `8/8`, all iteration 1 | not applicable | median true reduction `3.224743915e-14`; matrix and CPR composition can reach direct answer |
| iterative pressure tolerance `1e-10` | `0/8` | `4/5` | same 31/32 and 31 post-restart passes; coarse tolerance alone refuted |

The exact-dense control's median direct-correction delta is `2.601154847e-10`. The tighter
iterative bounded run still has median iteration-30 reduction `1.455230947e-2` and passes at
31/32; the hard gas run remains `4.896820185e-2` at 30 and passes at 31. The pressure threshold
is restored to `300` and the iterative relative tolerance to `1e-6`.

Code inspection closes the mechanism classification. The routine named FGMRES builds an Arnoldi
basis from `M^-1 r`, applies `M^-1 A` to each basis vector, and combines that same basis into the
solution. This is fixed left-preconditioned GMRES. For pressure systems above 300 rows, `M^-1`
contains a tolerance-terminated BiCGSTAB solve and is input-dependent. The fixed-operator
Hessenberg/Givens residual identity therefore does not apply. This accounts for the estimate/
actual split from the first iteration and the fresh progress only after restart.

Diagnostic commands:

```text
FIM_CAPTURE_DIR=/tmp/ressim-y2d0-2030996 FIM_Y2D1_CORPUS=bounded-22x22x1 \
  cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib \
  solver_lab_audit_restart_boundary_history -- --ignored --nocapture
FIM_CAPTURE_DIR=/tmp/ressim-y2d1-gas-e143c19 FIM_Y2D1_CORPUS=gas-20x20x3-current \
  cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib \
  solver_lab_audit_restart_boundary_history -- --ignored --nocapture
```

Verdict: **ALGORITHM-CONTRACT GAP CONFIRMED; NO PRODUCTION CHANGE**. Y2d4 must implement a
test-only true right-preconditioned flexible recurrence (`v_j`, stored `z_j=M_j^-1v_j`,
`w=A z_j`, candidate `x_0+Zy`) with all CPR components and the 30-iteration cap fixed. It requires
synthetic variable/fixed-preconditioner controls and bounded `8/8`, gas `5/5` before any live
candidate. Do not promote dense pressure solving, a tighter coarse tolerance, budget 60, or AMG.

Final-tree validation: targeted well-Schur `4/4`, capture round-trip `2/2`, Sparse-LU report
contract, exact Y2d0 direct/CPR replay, final production-budget replays on bounded and gas, and
final Y2d3 history replays pass. `cargo check` has no new warnings. DRSDT0, both locked SPE1
tests, Buckley-Leverett `3/3`, and curated FIM `11/11` pass. The shared bucket passes its first
three contracts and stops at the unchanged pre-existing closed-system `rate_history` mismatch
(`left=2`, `right=1`). No wasm control matrix was run because Y2d3 adds only `#[cfg(test)]`
recording and makes no solver-behavior change.

### Bundle Y checkpoint Y2d4: true flexible-GMRES offline oracle (2026-07-15)

Implemented a test-only right-preconditioned flexible-GMRES oracle with raw Arnoldi basis `v_j`,
stored independently preconditioned directions `z_j`, operator products `A z_j`, and `Zy`
candidates. Production dispatch and behavior remain unchanged. Synthetic controls prove both
sides of the algorithm contract: an input-dependent nonlinear preconditioner breaks the old
fixed-left residual estimate, while the flexible estimate tracks the true residual and matches
direct; a fixed-linear preconditioner also matches direct.

Production-faithful captured replay kept quasi-IMPES restriction, block-ILU0, iterative pressure
solve (`50`, `1e-6`), well Schur, equation scaling, relative tolerance `0.005`, and effective
budget/restart 30 fixed:

| Corpus | Production converged | Flexible converged | Iterations | Median reduction | Median direct delta | Max estimate/true disagreement |
| --- | ---: | ---: | --- | ---: | ---: | ---: |
| bounded 8 | `0/8` | `8/8` | all `2` | `1.204527535e-8` | `1.641232493e-4` | `1.031351975e-8` |
| gas 5 | `4/5` | `5/5` | `1-3` | `6.986957784e-5` | `1.871345444e-6` | `8.472080369e-11` |

The former hard gas artifact passes in one iteration at reduction `3.813067988e-3` and direct
delta `5.728602178e-6`, versus production failure at iteration 30/reduction
`4.896722176e-2`. Every pre-existing gas pass remains a flexible pass. Full report norms,
independent residuals, reservoir/well partitions, finite corrections, and direct references are
valid on all 13 systems.

Exact commands:

```text
cargo test --manifest-path src/lib/ressim/Cargo.toml true_flexible_gmres -- --nocapture
FIM_CAPTURE_DIR=/tmp/ressim-y2d0-2030996 FIM_Y2D1_CORPUS=bounded-y2d4 \
  cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib \
  solver_lab_compare_true_flexible_gmres -- --ignored --nocapture
FIM_CAPTURE_DIR=/tmp/ressim-y2d1-gas-e143c19 FIM_Y2D1_CORPUS=gas-y2d4 \
  cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib \
  solver_lab_compare_true_flexible_gmres -- --ignored --nocapture
```

Source/configuration closeout used both the upstream OPM implementation and the preserved exact
Flow 2026.04 output. `FlexibleSolver_impl.hpp` defaults to `bicgstab`, while separately supporting
`gmres` and genuine `flexgmres`. The exact reference `CASE.DBG` resolves ambiguity: outer
`bicgstab`, `maxiter=20`, `tol=0.005`; `cprw` with true-IMPES weights, well contributions,
`paroverilu0`, and a one-iteration AMG coarse loop. Thus Y2d4 confirms a mathematical defect in
ResSim's named FGMRES implementation and a decisive correction with its present nonlinear CPR;
it does not claim to reproduce Flow's outer or coarse algorithm.

Verdict: **CONFIRMED OFFLINE — NO PRODUCTION CHANGE**. Y2d5 is a separately gated default-off
production integration of only the recurrence. It must preserve both replay corpora before live
`22x22x1` water/current gas gates, then exact gas and the full control matrix. Literal OPM
BiCGSTAB/true-IMPES/AMG work remains separate.

Final-tree validation: synthetic Y2d4 controls `2/2`, final bounded and gas Y2d4 replays, exact
Y2d0 production replay, well-Schur `4/4`, capture round-trip `2/2`, Sparse-LU report contract,
`cargo check` (no new warnings), DRSDT0, Buckley-Leverett `3/3`, and curated FIM `11/11` pass.
The shared bucket passes its first three contracts and stops at the unchanged pre-existing
closed-system `rate_history` mismatch (`left=2`, `right=1`). No wasm matrix was run because all
Y2d4 solver and wrapper entry points are `#[cfg(test)]` and production behavior is unchanged.

### Bundle Y checkpoint Y2d5: default-off production true-FGMRES (2026-07-15)

Scope held fixed: quasi-IMPES, block-ILU0, tolerance-terminated pressure BiCGSTAB (`50`, `1e-6`),
well Schur, scaling, `restart=30`, effective budget 30, Newton acceptance, timestep control, and
the Y2 lifecycle. Only the outer recurrence routing changed. `FimLinearSolveOptions` gains
`use_true_fgmres=false`; `setFimTrueFgmres` and `--true-fgmres` provide independent live A/B.

Oracle and dispatch gates:

```text
FIM_CAPTURE_DIR=/tmp/ressim-y2d0-2030996 FIM_Y2D1_CORPUS=bounded cargo test --release \
  --manifest-path src/lib/ressim/Cargo.toml solver_lab_compare_true_flexible_gmres \
  -- --ignored --nocapture
FIM_CAPTURE_DIR=/tmp/ressim-y2d1-gas-e143c19 FIM_Y2D1_CORPUS=gas cargo test --release \
  --manifest-path src/lib/ressim/Cargo.toml solver_lab_compare_true_flexible_gmres \
  -- --ignored --nocapture
```

Promoted dispatch exactly equals the Y2d4 oracle for solution, convergence, iteration count, RHS,
and final residual. Bounded remains `8/8` in two iterations; gas remains `5/5` in one to three.
Default-off and FGMRES-only dispatch tests pass.

First Legacy live gates are stable and cheaper in outer linear iterations. `22x22x1` remains four
substeps/two nonlinear retries, with accepted linear iterations `3,4,3,4 -> 2,3,3,3` and measured
linear time `963 -> 372 ms`. Gas `20x20x3` remains two/one nonlinear, `3,3 -> 2,2`, linear time
`1059 -> 658 ms`. Timings are single-run diagnostic observations, not committed performance
baselines.

The complete Y2 matrix is decisive:

```text
FIM_Y2B_RAW_SATURATION=1 FIM_TRUE_FGMRES=1 FIM_Y2C_WATER_GRID=<grid> \
FIM_Y2C_FLAVOR=opm cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib \
  repro_water_pressure_y2c_control -- --ignored --nocapture
FIM_Y1J_STEPS=6 FIM_Y2B_RAW_SATURATION=1 FIM_TRUE_FGMRES=1 cargo test --release \
  --manifest-path src/lib/ressim/Cargo.toml --lib repro_gas_rate_10x10x3_y1j \
  -- --ignored --nocapture
FIM_Y2B_RAW_SATURATION=1 FIM_TRUE_FGMRES=1 cargo test --release \
  --manifest-path src/lib/ressim/Cargo.toml --lib \
  repro_water_pressure_12x12x3_opm_aligned_no_trace -- --ignored --nocapture
```

- `22x22x1`: `11 substeps/8L -> 3/0L+1N`, Newton `11,5,6`;
- `20x20x3`: five stays five, `1L+1N -> 0L+2N`, Newton `9,5,5,5,4`;
- `23x23x1`: three stays three, `1L -> 0L+1N`, Newton `11,5,6`;
- heavy: seven/one nonlinear unchanged;
- exact gas: six/zero unchanged, Newton `8,5,4,4,4,4 -> 9,6,5,5,4,4` versus Flow
  `7,5,4,3,4,3`.

Legacy option-on controls preserve substeps/retries: water `20x20x3=8/3N`, `22x22x1=4/2N`,
`23x23x1=4/2N`; gas `20x20x3=2/1N`; six-step gas totals 14 substeps/seven nonlinear; heavy is
21/three nonlinear versus the documented 25-substep Legacy baseline.

Verdict: **CONFIRMED — VALIDATED DEFAULT-OFF CORRECTNESS PATH, NOT DEFAULT OR FULL OPM
PROMOTION.** The invalid recurrence masked the positive Y2 lifecycle on the blocking water case.
True FGMRES removes the linear retry class but modestly increases accepted Newton work on several
otherwise-stable cases. The next slice is Y2d6's source-complete design for Flow's actually
selected BiCGSTAB/true-IMPES CPRW/paroverilu0/one-loop-AMG lifecycle, not more FGMRES tuning.

Final-tree validation: dispatch/default and synthetic true-FGMRES tests; both final capture
corpora; well-Schur `4/4`; capture round-trip `2/2`; Sparse-LU backend-neutral report contract;
`cargo check` with only the four pre-existing native warnings; successful WASM rebuild; diagnostic
CLI syntax; typecheck; lint; Vitest `648/648`; DRSDT0; Buckley-Leverett `3/3`; and curated FIM
`11/11` all pass. The FIM bucket's three SPE1 tests took `838.52s` in debug but completed. The
shared bucket again passes its first three contracts and stops at the unchanged pre-existing
closed-system `rate_history` assertion (`left=2`, `right=1`). The package manager's background
version check reports restricted-network `ERR_PNPM_META_FETCH_FAIL`, but typecheck/lint/Vitest
commands themselves exit successfully.

### Bundle Y checkpoint Y2d6: exact Flow linear-lifecycle design (2026-07-15)

Pinned the design to installed `libopm-simulators-bin 2026.04-1~noble`, the exact OPM source tag
`release/2026.04/final` (`b82f21d...`), and DUNE-ISTL 2.11.0. The source confirms raw two-norm
BiCGSTAB stopping at `0.005`, twenty full alpha/omega pairs, storage-derived true-IMPES weights,
zero CPR pre-sweeps, one coarse pressure correction, and one post `paroverilu0` sweep. The coarse
`loopsolver(maxiter=1)` clears its correction and applies AMG once, so it is a fixed application
inside one outer solve rather than ResSim's RHS-dependent tolerance-terminated pressure
BiCGSTAB.

The audit found a new coupled mismatch that must be held atomic. Flow's outer operator includes
eliminated well effects, but its fine ILU factors the reservoir matrix without them and CPRW adds
well pressure contributions explicitly to the coarse matrix. ResSim currently Schur-eliminates
wells before CPR and factors that modified matrix. The present 13 captures also lack the raw
storage blocks required for true-IMPES. Therefore an outer-only BiCGSTAB run, an AMG-only run, or
a “true-IMPES” weight derived from the diagonal Jacobian would all be `INCONCLUSIVE`.

Design and prescriptive gates: `docs/FIM_Y2D6_FLOW_LINEAR_LIFECYCLE_DESIGN.md`. Next is Y2d6a
capture payload only: raw storage/weight proof plus separate reservoir/well blocks, strict
round-trip validation, then one bounded and one gas artifact. No FIM solver behavior changed.

IMPES applicability was audited separately. Bundle X's connected-cell producer fractions already
benefit the shared product path. Y2 primary switching, variable CPR, well-Schur, and CPRW do not
map to IMPES's explicit transport and pressure-only direct system. The iteration-contract review
did expose one IMPES fallback diagnostic bug: loop-boundary BiCGSTAB convergence over-counted one
not-executed iteration. A focused regression failed `2 != 1`, the counter placement was corrected,
and the complete solver test bucket passes `6/6`; the numerical pressure correction is unchanged.

### Bundle Y checkpoint Y2d6a: source-complete capture payload (2026-07-15)

Implemented a native/default-off `FIM_Y2D6_CAPTURE_DIR` first-system capture using format v3. The
full unscaled Jacobian, RHS, block layout, and equation scaling remain intact; the companion adds
the exact per-cell accumulation derivative blocks used by assembly, Flow's normalized true-IMPES
weights using the equivalent 50-bar pressure scale, and the four pre-elimination partitions
`J_rr/J_rw/J_wr/J_ww`. The source fingerprint is fixed to Flow
`release/2026.04/final@b82f21d...` and DUNE-ISTL 2.11.0.

The payload oracle does more than parse: it recomputes every weight, checks cardinalities and
partition dimensions, and reconstructs every full-J entry bit-for-bit. Focused round-trip and
mismatched-weight rejection tests pass. Two isolated release artifacts then passed:

- bounded `22x22x1`: rows `1456`, cells `484`, well rows `4`, full nnz `4764`, partition nnz
  `[4752,2,4,6]`;
- exact gas `10x10x3`: rows `904`, cells `300`, well rows `4`, full nnz `5372`, partition nnz
  `[5360,3,2,7]`.

This proves capture sufficiency only; it is not a convergence result. No solver dispatch,
preconditioner, nonlinear acceptance, timestep control, or IMPES equation was changed. Next is
Y2d6b: prove the seven component identities on both artifacts before any 13-capture verdict.

### Bundle Y checkpoint Y2d6b: component identities (2026-07-15)

Added a test-only component oracle around the v3 payload, without an outer Krylov solver or live
dispatch. It forms the standard-well effect both matrix-free and as an independent explicit Schur
matrix, builds the CPR restriction/prolongation from captured true-IMPES weights, factors the
existing block ILU only on raw `J_rr`, and applies zero pre-sweeps, one coarse correction, then one
post correction against the matrix-free outer residual.

DUNE 2.11's hierarchy builder stops immediately when unknowns are at or below
`coarsenTarget=1200`; its sequential coarse path then selects the direct solver. The captured
pressure systems have only 484 and 300 rows. Therefore D6b stores one direct coarse factor map and
tests its repeatability/linearity. This is the complete source-bounded AMG surface for these
artifacts, not a partial aggregation port and not authorization for a general AMG project.

All seven identities pass on both release artifacts:

- bounded: outer/coarse disagreement `0/0`, coarse well norm `1.268891369e1`, fine/coarse/CPR
  linearity `2.0997e-16/9.6570e-15/1.9729e-15`, independent residual-norm disagreement
  `1.8882e-16`;
- gas: outer/coarse disagreement `8.1572e-19/5.8715e-18`, nonzero coarse well norm
  `1.355152165e-12`, linearity `1.3004e-16/2.0332e-15/4.4227e-16`, and zero independent
  residual-norm disagreement.

A self-contained coupled two-cell/two-well-unknown regression proves the same identity gate
without external artifacts. These are algebraic correctness results only. Next is Y2d6c: build
the exact raw-norm/twenty-pair DUNE BiCGSTAB recurrence around this fixed map, regenerate/extend
the v3 corpus to bounded eight plus gas five, and compare without live routing.

Validation on the final D6b tree: lifecycle unit/oracle `3/3`, AD/legacy structural parity
`12/12`, DRSDT0, both locked SPE1 tests, curated FIM bucket (SPE1 `3/3`, wells `5/5`, depletion
`3/3`), and Buckley-Leverett `3/3` pass. The shared bucket passes its first three public contracts
and stops at the unchanged pre-existing closed-system `rate_history` assertion (`left=2`,
`right=1`). `cargo check` passes; after gating parser-only helpers to tests it retains only the
four pre-existing native dead-code warnings.

### Bundle Y checkpoint Y2d6c.1: capture-v3 corpus regeneration (2026-07-15)

Added `FIM_Y2D6_CORPUS_DIR` at exactly the two selectors that produced the preserved Y2d4 corpus:
final Newton-iteration near miss and OPM-aligned linear abort. The v3 companion is assembled from
the same state/Jacobian before solver dispatch; older v2 capture paths and D6a's atomic first-system
trigger are unchanged.

Release regeneration reproduces the historical controls and cardinalities:

- bounded `22x22x1`: 11 substeps, eight linear retries, exactly eight v3 `max-iters` artifacts;
- gas `20x20x3`: 238 substeps, one linear/four nonlinear retries, exactly four v3
  `final-iteration-near-miss` plus one `max-iters` artifact.

The corpus payload lab passes `8/8` and `5/5`; loading each file recomputes true-IMPES and
reconstructs full J bit-for-bit. Artifact directories are
`/tmp/ressim-y2d6c-bounded-1b3de31` and `/tmp/ressim-y2d6c-gas-1b3de31`. This is selection and
payload evidence only. Next is the exact test-only DUNE BiCGSTAB recurrence.

### Bundle Y checkpoint Y2d6c.2: exact DUNE BiCGSTAB corpus result (2026-07-15)

Implemented the outer solve only in the test oracle, directly following DUNE-ISTL 2.11
`BiCGSTABSolver::apply`: zero correction, raw norm, strict `<0.005`, matching rho/omega/h
breakdown guards, alpha/omega half-step checks, and at most twenty complete pairs. The fixed D6b
CPR is right-applied to `p` and the intermediate residual; every capture reruns all seven
identities before entering the recurrence.

| Corpus | Production | true FGMRES | Flow lifecycle | Max complete pairs | Median full reduction | Median direct delta |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| bounded 8 | `0/8` | `8/8` | **`8/8`** | `1` | `1.977026308e-13` | `1.417902240e-9` |
| gas 5 | `4/5` | `5/5` | **`5/5`** | `1` | `3.813935911e-4` | `2.617263521e-6` |

No solve breaks down or loses an existing pass. Alpha/omega/preconditioner counts are consistent,
outer norms match independent recomputation, and recovered well residuals are zero or roundoff.
Bounded captures `00006`/`00007` stop legitimately at `1.3168e-3`/`2.2524e-4` and retain direct deltas
`7.2245`/`1.1150`; the other six are near direct. This is expected solution-quality latitude
under Flow's loose linear criterion and is not claimed as direct equivalence.

Verdict: **CONFIRMED OFFLINE — D6D DEFAULT-OFF LIVE AUTHORIZED.** This is the first complete
Flow-selected linear lifecycle result on these blockers; it is not yet evidence about Newton
iterations or live OPM parity. IMPES remains unchanged because it has no coupled CPR system.

Final validation: lifecycle unit/oracle `3/3`; both identity-gated release corpora; DRSDT0; both
locked SPE1 tests; curated FIM bucket (SPE1 `3/3`, wells `5/5`, depletion `3/3`); and
Buckley-Leverett `3/3` pass. `cargo check` passes with the four pre-existing native warnings. The
shared bucket passes its first three contracts and stops at the unchanged closed-system
`rate_history` assertion (`left=2`, `right=1`).

### Bundle Y checkpoint Y2d6d: default-off live Flow lifecycle (2026-07-15)

Implemented the complete D6c stack behind native-only `FIM_FLOW_LIFECYCLE=1`, default false:
storage-derived true-IMPES, matrix-free StandardWell outer action, separate `J_rr` block ILU0,
exactly-once CPRW well coarse contribution, one-level direct pressure solve, and the pinned DUNE
BiCGSTAB recurrence. Only the pressure coarse matrix is dense. A focused coupled-system test
proves the live construction matches the independent explicit-Schur oracle.

Committed-tree baseline before the change: exact gas six/zero with reported residual evaluations
`8,5,4,4,4,4`; heavy with the complete Y2 lifecycle seven substeps and `0L/1N`. Live Flow
lifecycle: exact gas remains six/zero but evaluations become `10,5,5,4,4,4`; heavy remains seven
with one mixed retry. Y2d7 later established the comparable applied-update sequences as
`7,4,3,3,3,3 -> 9,4,4,3,3,3`, versus Flow `7,5,4,3,4,3`. Y2 bounded
controls remain three substeps (`22x22x1` `11,4,6`; `23x23x1` `10,5,5`), Legacy `22x22x1`
remains four and Legacy exact gas remains fourteen total. Heavy runtime rises roughly
`0.54s -> 3.47s` because this bounded oracle refactors a dense coarse matrix every iteration.

Verdict: **COMPLETE, DEFAULT-OFF, NOT PROMOTED.** The coherent linear mechanism is validated but
does not close the nonlinear trajectory gap. No IMPES port applies: IMPES has no coupled
well-tail/CPRW system. Next compare Flow and ResSim nonlinear iteration observables and locate the
first divergence before choosing G4/G5; no further linear tuning is authorized by this result.

Final gates: lifecycle/default dispatch tests, DRSDT0, locked SPE1 `3/3`, curated FIM wells `5/5`
and depletion `3/3`, Buckley-Leverett `3/3`, rebuilt-wasm Legacy control matrix, and full
`validate:product` (typecheck, lint, frontend `648/648`, IMPES bucket, wasm and Vite build) pass.
The shared bucket again passes its first three contracts and stops at the unchanged closed-system
`rate_history` assertion (`left=2`, `right=1`).

### Bundle Y checkpoint Y2d7: nonlinear trajectory and count-accounting audit (2026-07-15)

Scope: diagnostics plus test-driver access to the existing nested-well option; no production
default or solver behavior changed. Commit under test before the driver plumbing was `40f366d`.
Fresh Flow was generated with:

```text
bash scripts/opm-ressim-compare.sh --opm-only --out-dir /tmp/ressim-y2d6e-40f366d
```

The fixture check passes. `CASE.INFOSTEP` reports applied updates `7,5,4,3,4,3`, while each
report step has one additional `CASE.INFOITER` residual row. ResSim reports `iteration+1` when
entry convergence is observed, so its historical `8,5,4,4,4,4` sequence is residual evaluations,
not comparable applied updates. Trace entry indices give Y2 updates `7,4,3,3,3,3` and D6d
updates `9,4,4,3,3,3`. Flow and D6d both total 26; per-step L1 mismatch is 3 for Y2 and 4 for
D6d. This supersedes the earlier `29 -> 32 versus 26` comparison.

The trajectory anchor is strong. At evaluation 0, Flow/ResSim oil CNV/MB are
`0.5109/1.667e-3`; gas CNV is `1.2457/1.245`, and gas MB is
`3.5069e-3/3.470e-3`. After update 1, Flow remains `WellStatus=CONV` with oil MB
`1.8375e-3`. Default ResSim moves injection `-500 -> -526.8532`, produces
well/perforation residuals about `5.747e-2/1.699e-2`, and oil MB rises to `4.311e-3`, binding at
saturated injector cell 0.

The native repro driver now accepts `FIM_NESTED_WELL_SOLVE=1` and prints the selected state. On
the same first report step, nested solve keeps injection at `-499.999999999`, reduces the well
residual to roundoff and the perforation residual to `1.88e-6`, but leaves oil MB exactly
`4.311e-3`. It accepts after 6 applied updates versus Flow's 7. Across six reports its applied
updates are `6,5,3,3,3,3`; this redistribution is diagnostic, not a promotion claim.

Verdict: **DIAGNOSTIC; G4 injector reservoir/well source formulation is next.** Hold nested solve,
Y2 state lifecycle, linear routing, acceptance, and controller fixed. Compare component source
and rate/unit conversion at injector cell 0 after the first `ds-max` update against Flow
`StandardWell`. G5 is not first because that binding cell remains saturated with active `Sg`.
IMPES has no equivalent nested Newton well solve; audit it only if G4 locates a defect in shared
well/component source physics.

Final focused gates on the edited tree: the exact one-step repro passes with
`nested_well_solve=false`, one accepted substep, 8 residual evaluations and zero retries; the same
repro with `FIM_NESTED_WELL_SOLVE=1` passes with one accepted substep, 7 residual evaluations and
zero retries. All `fim::wells_inner::tests` pass (`12/12`). `cargo fmt` was run; unrelated
format-only changes in already-existing Rust files were excluded from this slice.

### Bundle Y checkpoint Y2d8/G4: report-step-frozen RESV injection conversion (2026-07-15)

Scope: source audit and native trace instrumentation only; no production source, rate, control,
or acceptance behavior changed. The `WELLSOURCE` line uses the live
`perforation_component_rates_sc_day` helper and records component rate, dt-weighted source, and
assembled `d(residual)/dq`; therefore it is an assembler-consistent observation rather than a
second formula.

The exact repro commands were:

```text
FIM_TRACE_FILE=/tmp/ressim-y2d8-default.trace FIM_TRACE_DT_BELOW=1 \
FIM_Y1J_GRID=10 FIM_Y1J_FLAVOR=opm FIM_Y1J_STEPS=1 FIM_Y2B_RAW_SATURATION=1 \
cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib \
fim::timestep::phase5_repro::repro_gas_rate_10x10x3_y1j -- --ignored --nocapture --exact

FIM_TRACE_FILE=/tmp/ressim-y2d8-nested.trace FIM_TRACE_DT_BELOW=1 \
FIM_Y1J_GRID=10 FIM_Y1J_FLAVOR=opm FIM_Y1J_STEPS=1 FIM_Y2B_RAW_SATURATION=1 \
FIM_NESTED_WELL_SOLVE=1 cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib \
fim::timestep::phase5_repro::repro_gas_rate_10x10x3_y1j -- --ignored --nocapture --exact
```

Both pass. At evaluation 0, `q=-500`, `B_g=0.0065`, and source is
`-76,923.076923 Sm3/d` (`-19,230.769231` in the 0.25-day gas row). After the first update,
the nested trace holds q at `-500` but has cell-0 `p=242.6790872`, `B_g=0.005219627384`, and
source `-95,792.278488 Sm3/d` (`-23,948.069622` in the row). Default also changes q to
`-526.8532245`, giving `-100,936.941616 Sm3/d`. The assembled gas `dres/dq` changes from
`38.46153846` to `47.89613924`, exactly `dt/B_g`.

Flow source inspection pins the contrasting semantics: `RateConverter::defineState` creates the
regional average at `BlackoilWellModel::beginReportStep`; `WellAssemble::assembleControlEqInj`
uses the resulting `calcInjCoeff` for RESV control; `StandardWell` supplies its surface component
rate to `BlackoilWellModel::addReservoirSourceTerms`. The regional coefficient is refreshed after
an accepted step, not per Newton evaluation. With the deck's uniform initial 200-bar state,
Flow therefore retains the `B_g=0.0065` conversion during report step 1; its evaluation-1
`WellStatus=CONV` maps the 500 m3/d target to `-76,923.076923 Sm3/d`. The
`18,869.201565 Sm3/d` nested discrepancy is a valid source-comparable oracle and remains after
the separate well-row relaxation is removed.

Verdict: **CONFIRMED G4 LIFECYCLE MISMATCH; DESIGN REQUIRED.** Do not apply a source-only frozen
`B_g` patch. Flow's surface component unknown, frozen regional RESV coefficient, connection
equation, and source term are coupled. Next write G4a's matched/held/missing dependency table and
a default-off coherent probe/oracle. No IMPES change is implied yet.

Focused validation: both exact native replays pass (default: 8 residual evaluations; nested: 7),
`fim::wells_ad::tests::parity_injector_rate_controlled_reservoir_target` passes, the gas-injector
source-pressure finite-difference test passes, and `fim::wells_inner::tests` passes `12/12`.
The locked FIM baseline passes `3/3` (DRSDT0 plus both SPE1 smokes), and `assembly_ad` parity
passes `12/12`.

### Bundle Y checkpoint G4a: coherent Flow gas-RESV injector design (2026-07-15)

Scope: source/design closeout only. No Rust solver path, parsed deck behaviour, IMPES path,
controller, acceptance rule, or convergence measurement changed.

The prescriptive design is `docs/FIM_G4_INJECTOR_RESV_LIFECYCLE_DESIGN.md`. It resolves a
representation issue that could otherwise create a misleading partial patch. ResSim q is a
negative local reservoir connection rate, constrained by `q-q_connection_res(p,bhp)=0`, with
source `q/B_g(cell)`. Flow's injector `WQTotal` is positive surface gas rate, RESV control is
`B_g,ref*WQTotal-Q_resv=0`, and the reservoir source is the same standard-condition rate with
reservoir sign. The current-state connection calculation still depends on local properties.

For the exact pure-gas one-perf case the coherent probe is:

```text
R_perf = -q_res(p,bhp)/B_g(cell) - u
R_ctrl = B_g,ref*u - Q_resv
S_gas  = q_res(p,bhp)/B_g(cell)
```

At observed evaluation 1, `B_g,ref=0.0065` and `u=76,923.076923 Sm3/day`, so source must remain
`-76,923.076923 Sm3/day` because the perforation row is converged (`-q_res/B_g=u`). Local
`B_g(cell)=0.005219627384` belongs in both current connection/source derivatives, not a frozen
source coefficient. The design holds Flow retry lifetime, multi-perf allocation, active BHP
switching, and the existing q-coordinate nested local solve outside the probe. Calling that
nested solve after changing only global rows would recreate the independently-derived-row defect
Bundle W was built to avoid. IMPES stays out of scope because this is a FIM-tail representation
change.

**Next authorized checkpoint: G4b0 only.** Add explicit RESV control representation and immutable,
default-off report-step reference context with strict unsupported-case guards and unit tests. Do
not route assembly, freeze a source, or run a live convergence comparison in that commit.

### G4b0: inert RESV representation and report-step reference context (2026-07-15)

Implemented the deliberately non-behavioral first slice. `WellScheduleControl::Resv` parses the
existing serialized control string, while native-only `FIM_FLOW_RESV_INJECTOR` enables capture of
one `FlowResvReportStepContext`. Its `FlowResvReference` is the gas/hydrocarbon-PV-weighted
pressure and PVT-derived `B_g,ref` at report-step entry. The context is copied unchanged on every
Newton retry and rebuilt only after accepted-state write-back. It is carried through
`FimNewtonOptions`, but no assembly code reads it.

The guard is intentionally narrower than the production frontend: it rejects no/excess RESV
wells, non-gas or disabled wells, multi-perforation topology, surface-rate target, explicit BHP
limit, missing/nonpositive reservoir target, unsupported PVT, and q-coordinate nested well solve.
Current ResSim has no FIP/PVT region mapping, so the probe explicitly permits one implicit region
only. It does not expose RESV through the public schedule setter, route it into `well_control`, or
alter IMPES. This prevents a representation-only commit from silently creating a partial runtime
path.

Focused `flow_resv` tests initially exposed a test-fixture error rather than an implementation
error: all fixture PVT rows had the same `Rs`, so the project's PVT grouping selected an
unexpected branch. Giving the three rows distinct `Rs` values restored the intended gas-table
interpolation. The final focused suite passes four tests: capture, disabled default, retry versus
accepted-step refresh, and nested/BHP rejection.

Validation: focused `flow_resv` (4), `well_controls` (9), and `assembly_ad` (12) tests passed;
the three locked FIM smokes passed; `validate-solver-coverage.sh fim` and all three
`benchmark_buckley` checks passed. `validate-solver-coverage.sh shared` passed its first three
public well-control checks, then reproduced the separately documented closed-system
`rate_history` assertion at `runtime_api.rs:81` (`2` versus `1`). It is pre-existing and outside
this inert path, so it is recorded as **INCONCLUSIVE for shared bucket completion**, not evidence
for or against G4b0.

Verdict: **G4b0 complete; no convergence claim.** Next authorized step is G4b1 only: build the
shared AD/f64 residual contract for `c_s=-q_res/B_g`, `R_perf=c_s-u`,
`R_ctrl=B_g,ref*u-Q_resv`, and `S=-c_s`, with value, derivative, and finite-difference gates.
Neither assembler, state update, nested solve, IMPES, nor live convergence is in scope.

### G4b1: shared current-FVF residual/source contract (2026-07-15)

Added `flow_resv_injector_residual<S: Scalar>` in `fim/flow_resv.rs`. It is a pure local bundle:
the current reservoir connection `q_res` and current `B_g` produce
`c_s=-q_res/B_g`, then `R_perf=c_s-u` and `S_gas=-c_s`; only the control term receives the frozen
report-step constants, `R_ctrl=B_g,ref*u-Q_resv`. The helper returns `c_s` separately so any
future trace can prove the perforation equality before it compares source to `-u`.

Two fixtures intentionally use current `B_g` different from `B_g,ref=0.0065`. Their f64 values
give `c_s=u=76,923.076923` and source `-76,923.076923`; local AD verifies that the connection and
perforation pressure derivatives match, source has their negative, source has no u derivative,
and control's u derivative is exactly `0.0065`. A central finite difference independently
matches the AD pressure derivatives away from a clamp. This proves the intended local math, not
an OPM trajectory or production route.

The helper has no callers outside its tests. No legacy/AD assembler, source helper, rate
unknown/update, local solve, scaling, diagnostics, reporting, IMPES, or live convergence path
changed. **Next authorized slice: G4b2 readiness audit only.** It must enumerate and gate those
coupled routes before an atomic implementation; a partial assembler connection remains invalid.

Validation on this checkpoint: the focused `flow_resv` suite passes all six context/contract
tests, including the two new value/AD/FD tests; `assembly_ad` passes all 12 parity/numerical
tests. A locked FIM smoke and the curated FIM bucket were started but produced no completion
result in this terminal after several minutes and were interrupted rather than credited. Because
this helper is uncalled by production code, that is **INCONCLUSIVE regression coverage**, not a
solver/convergence observation; the completed focused and assembly gates are the only claimed
validation for G4b1.

### G4b2: RESV execution safety block and atomic-route readiness audit (2026-07-15)

The audit found that G4b0's valid RESV context did not itself select a RESV well formulation.
`physical_well_control` accepts only the literal `rate` as rate-controlled, so a live invocation
would otherwise continue through the historical BHP/q control, source, and update path while the
new context/helper was inert. That is a more dangerous outcome than an explicit unsupported
error: it could produce a plausible trajectory that never evaluated the intended Flow rows.

Added a pre-Newton block after successful scoped-context capture. The valid 1x1x1 gas/PVT/RESV
fixture confirms `time_days` is unchanged and the warning says atomic routing is not ready. This
replaces the prior operational assumption that the context could safely travel through a live
Newton attempt; context lifetime is still unit-tested, but live execution is now intentionally
unavailable until the whole lifecycle is routed.

`docs/FIM_G4B2_ATOMIC_ROUTE_READINESS_AUDIT.md` inventories the missing package: explicit u
state/control, both assembly/Jacobian paths, current-FVF source helper, q-relax exclusion,
scaling, Schur, diagnostics/reporting, and retry semantics. **Next authorized step: G4b2a atomic
route design only.** No individual assembler/source/update edit and no live convergence run is
valid before that design provides one coupled implementation/gate list.

### G4b2a: atomic typed-u implementation design (2026-07-15)

No Rust behaviour changed. The completed handoff is
`docs/FIM_G4B2A_ATOMIC_ROUTE_IMPLEMENTATION_DESIGN.md`. The critical implementation decision is
to mechanically replace the ambiguous `perforation_rates_m3_day: Vec<f64>` representation with a
typed primary (`ReservoirConnectionQ` or `FlowResvGasSurfaceU`) before routing a RESV run. A
metadata-only exception would leave too many q-based source, reporting, scaling, relaxation, and
test call sites able to silently misread u.

The selected route retains the existing BHP/perforation tail dimensions but initializes
`u=Q_resv/B_g,ref` and solves its initial BHP against `q_res=-Q_resv`. It scatters the shared
G4b1 value contract into gas source, perforation, and control rows together. In particular the
source keeps current-FVF cell/BHP derivatives and has no u column; the control is a plain
`B_g,ref*u-Q_resv` row with no BHP/cell column. AD and legacy must be changed together, but
legacy retains an independent analytic derivative for `-q_res/B_g(current)`.

The execution block remains until full default-off parity, five-variable FD, typed update/floor,
scaling, exact Schur, and evaluation-0/1 trace gates pass. Retry lifetime, BHP switching,
multi-perf allocation, nested u solve, IMPES, acceptance, and linear/controller policy remain
unimplemented or held fixed. **Next authorized slice: one atomic code route plus non-live gates;
no convergence replay yet.**

### G4b2b0: assembler-context scaffold, deliberately still blocked (2026-07-15)

The first implementation pass threads `FlowResvReportStepContext` through `FimAssemblyOptions`
and both assemblers. The selected residuals now use the G4b1 helper: gas source/current-FVF
connection/perforation/control rows are scattered together; the u source column is absent, the
control u column is `B_g,ref`, and route-aware scales are present. A valid one-cell fixture
initializes `u=Q_resv/B_g,ref`, solves BHP against `q_res=-Q_resv`, verifies full AD/legacy
residual and matrix agreement, checks the u-column central difference, and preserves historical
assembly parity (12/12). The FIM timestep still returns before Newton for the native option.

This is **not G4b2b completion**. The stored selected value is still the historical
`perforation_rates_m3_day` slot interpreted by the route as u, so it has not met G4b2a's typed
state requirement. The legacy route currently uses a separate scatter but shares generic AD
property differentiation, not the required independent analytic `-q/B_g` linearization. A
first attempt to compare the complete gas reservoir row against a pressure FD was also an invalid
oracle because the row includes accumulation; G4b1's local current-FVF FD remains valid, while
the full-route source FD needs a well-source-isolated residual difference. Keep the safety block,
do not use a live metric, and make those three gaps the next code/design work.

Validation: the new execution-block test passes, the focused `flow_resv` suite remains 6/6,
`assembly_ad` remains 12/12, and `well_controls` remains 9/9. No long-running FIM bucket is
credited in this checkpoint; the safety guard and pure audit do not constitute a convergence
measurement.

### Bundle Y checkpoint Y1i: durable OPM oracle and acceptance-gate audit (2026-07-13)

Scope: measurement infrastructure and source audit only; no FIM production behavior changed.

Promoted the external gas-rate reference input into this branch as
`opm/reference-decks/gas-rate-10x10x3/CASE.DATA`, sourced verbatim from
`origin/fim-opm-continuation-plan@cacdf76701e33bccee6acc127845176be6080858`. Its checksum is
`9b76cbb1190f368d51ecdbaa94cdb4abd091195cef1cc0fb140f0109f02056aa`.
`manifest.json` records the ResSim command, input mapping, and Flow oracle; the new
`scripts/opm-reference-fixture-check.mjs` checks both deck identity/mapped invariants and the
post-run `CASE.INFOSTEP` Newton sequence. `scripts/opm-ressim-compare.sh` runs the same baseline
without creating Flow artifacts in the repository.

Fresh Flow `2026.04` replay of the tracked deck reproduced Y1h exactly: six accepted
`0.25`-day substeps, Newton `7/5/4/3/4/3`, zero cuts. This makes the OPM result durable and
replayable, but the fixture is intentionally described as a *matching hand-authored input*, not
proof of equivalent ResSim/OPM well formulation.

Most importantly, the post-review source audit corrected Y1g/Y1h's mechanism framing:
`would_widen` at `fim/newton.rs:3275` is logged but not used as the Legacy acceptance predicate.
The executing Legacy block is `!opm_aligned && stagnation_count >= 3` at line 3299; its residual
gate allows `10×` the nominal tolerance. Therefore `would_widen=false` on the sampled
`trend_vs_entry=1.0570` stall does **not** make that escape unavailable to Legacy. Extending the
block to OpmAligned would broaden an above-tolerance acceptance class already refuted by
`FIM-NEWTON-004`/`005`, not add a guarded OPM-alignment fix. Do not test that flip.

Next: construct an exact native 10x10x3 direct-vs-iterative, well/control isolation matrix and
identify the first ineffective injector-adjacent Newton update. The outcome determines whether
to inspect linear/update application or the nonlinear well/assembly formulation; acceptance is
out of scope.
### Bundle Y checkpoint Y1j: exact injector-update isolation (2026-07-13, provisional)

Added the ignored native `repro_gas_rate_10x10x3_y1j` driver in `fim/timestep.rs`, matching the
tracked 10x10x3 Flow mapping for one 0.25-day report step. It supports direct-vs-live-linear,
well-layout, and rate-vs-pressure isolation without changing the solver. `FIM_MAX_SUBSTEPS=1`
stops after the first accepted rung.

For both wells/rate/OpmAligned, live FGMRES/CPR and forced exact direct solve both take 5
nonlinear retries and accept exactly `dt=0.000978384825` after 20 Newton iterations, with
`res=1.040889e-5`, `mb=9.668795e-10`. Both traces freeze at `res≈1.041e-5` for iterations 13--18
and have the same `would_widen=false` classification. Thus the plateau is not a live-linear-stack
or update-application artifact.

No wells and producer-only rate control each accept the full `0.25` day with zero retries;
injector-only rate needs 6 nonlinear retries and accepts only `0.00032286699225`; both-well
pressure control is worse (7 nonlinear + 1 linear retry, accepted `0.00003515970997`) and
exposes a perforation-row hotspot. The injector is necessary and sufficient on this first rung;
rate control is not the unique cause. Verdict: G4 injector well/Jacobian/primary-variable audit
next, with no Newton-acceptance change. Replay after committing the newly-added driver before
promoting this as a clean baseline.

### Bundle Y checkpoint Y2a: injector Jacobian audit — active-bound AD kink (2026-07-13)

Added a test-only `FIM_Y2A_AUDIT=1` trace at the first three-count stagnation point. It compares
the live AD matrix, legacy hand-derivative matrix, and central plus one-sided finite differences
for the injector perforation and its cell's component rows. The bounded injector-only native
replay retains Y1j's `6` nonlinear retries and `dt=0.00032286699225`, so the instrumentation did
not alter solver behavior.

The exact command was replayed from clean commit `5a600ae`. At the last captured stalled rung
(`dt=3.2286699225e-4`, iter 5), AD and legacy residuals agree
to `1.137e-13`, but `Sw=Swc=0.15` exposes a decisive derivative kink in `rate_consistency`:
`d(res_pf)/dSw`: AD `-1428.5586`, legacy `0`, central FD `-714.2793`, forward FD `-1428.5585`,
backward FD `0`. The raw Newton `dSw≈-3.13e-5` points below the lower bound and is projected
back, leaving the active AD slope unable to produce an admissible move. Both-well traces show the
same signature. Connected oil/gas entries disagree too, so this cannot safely be fixed only in a
well row. OPM source confirms the different `StandardWell` per-well `WQTotal`/BHP formulation,
but with one perforation the local unknown count matches ResSim and the observed failure is more
immediately an AD boundary-convention inconsistency. Next: scope and test one coherent
active-bound derivative convention before a G4 restructure or acceptance change.

### G4b2b: atomic typed RESV route closeout (2026-07-19)

Implemented the G4b2a contract in four reviewable commits. State now stores
`Vec<FimPerforationPrimary>` rather than parallel value/kind vectors. Historical assembly and
nested-well paths require reservoir q; the selected RESV route requires positive surface u.
The legacy selected route is independently analytic and uses the explicit quotient derivative
for `-q/B_g`; complete `[p, Sw, hc, bhp, u]` central finite differences cover gas, perforation,
and control rows, and full direct versus well-Schur corrections agree.

Two independent defects were uncovered by those gates. First, `PvtTable::d_bg_d_p` used a
centered finite difference across table knots while generic AD interpolation used one active
segment; it now uses the exact interpolation segment (and retains the Boyle-law end branch).
Second, Newton accepted-state and timestep basin-escape residual-only probes omitted the RESV
context, which would reinterpret typed u as historical q after the main assembly succeeded. Both
now carry the immutable report-step context. Route-aware diagnostics/reporting derive current
connection q rather than relabeling u. A non-finite Legacy direct fallback is rejected before
update diagnostics or state mutation instead of allowing NaN/Inf corrections through; finite
historical fallback corrections remain usable even when the debug backend's convergence flag is
false.

Focused validation: `flow_resv` 8/8, `assembly_ad::tests` 6/6, `wells_inner::tests` 12/12,
`pvt::tests` 3/3, the production RESV timestep smoke 1/1, and `cargo check --tests` pass. The
curated shared bucket again reaches its documented pre-existing closed-system assertion
(`rate_history` length 2 versus 1), after its preceding three controls pass; it is not attributed
to G4b2b. The locked FIM coverage is green: SPE1 3/3, wells 5/5, and the three depletion/PSS
checks 3/3. IMPES transport 3/3 and timestep 2/2 are also green. The shared PVT correction changes
only `dB_g/dp`, which is consumed by FIM Jacobian construction; IMPES has no typed perforation
primary or analogous derivative route, so no IMPES implementation change is warranted. No
exact-deck performance metric is claimed here. Next is the capped committed-tree first
report-step trace, held to the no-retry/comparable-field oracle in the G4b2a design.

### G4b2b committed-tree live comparison (2026-07-19)

Added the missing native-driver wiring in `9cdff9b` so `FIM_FLOW_RESV_INJECTOR=1` actually assigns
the injector an explicit RESV 500 schedule and selects the atomic route; producer behavior stays
on the held historical rate path. Default-off remains the historical q route. This was necessary
because the documented environment command previously would have silently measured the old
path.

Fresh exact commands used the ignored release driver with `FIM_Y2B_RAW_SATURATION=1`, six steps,
and with/without `FIM_FLOW_RESV_INJECTOR=1`; the first-step typed run also used
`FIM_MAX_SUBSTEPS=1` and `FIM_TRACE_FILE`. Fresh Flow 2026.04 was regenerated through
`scripts/opm-ressim-compare.sh --opm-only`. Results:

| Case/route | Accepted / retries | Applied Newton updates | Linear iterations | Time |
| --- | --- | ---: | ---: | ---: |
| gas 10x10x3, Flow | `6 / 0` | `26` (`7,5,4,3,4,3`) | `27` | `0.08 s` simulation |
| gas 10x10x3, historical q + Y2 | `6 / 0` | `23` (`7,4,3,3,3,3`) | `61` | `0.576 s` elapsed |
| gas 10x10x3, G4b2b typed u + Y2 | `6 / 0` | `23` (`7,4,3,3,3,3`) | `64` | `0.545 s` elapsed |
| water-heavy 12x12x3, Flow | `1 / 0` | `11` | `14` | `0.04 s` simulation |
| water-heavy 12x12x3, current Y2 | `7 / 1` | `70` (`50` accepted + `20` discarded) | `185` | `0.510 s` elapsed |

The gas rows share the tracked grid/fluid/rate/report mapping, but G4b2b covers only the injector
without BHP switching and leaves the producer historical; they are not yet a full two-well
well-formulation equivalence claim.

Do not interpret the gas count as a G4 improvement: a preliminary default-off run omitted the Y2
raw-primary flag and collapsed to `0.000978 d`; that comparison was rejected and rerun correctly.
With held settings equal, G4b2b changes formulation/trajectory but not nonlinear counts.

Evaluation 1 classifies the result. Flow oil/gas MB are `1.8375e-3/2.8814e-3`; historical q is
`4.311e-3/2.482e-3`; typed u is `3.493e-3/4.526e-3`. More importantly, typed u has
`u=76,923.077`, `c_s=133,639.380`, `R_perf=56,716.303`, whereas Flow reports the well converged.
The source is therefore not yet comparable. **Verdict: G4b2b route COMPLETE; live OPM parity
INCONCLUSIVE due to the deliberately missing u-coordinate inner well solve.** Next is G4b3 with
`c_s≈u` after update 1 as the primary gate. No IMPES port applies; the typed tail/inner well
system is FIM-only, and the IMPES 5/5 gate already passes.

### G4b3 implementation: route-aware u-coordinate inner solve (2026-07-20)

Implemented the missing selected-well local system without a second physics formula. Its f64
residual values and `[p,Sw,hc,bhp,u]` AD derivatives are the same evaluations global assembly
scatters; the local restriction is rows `[control,connection]` by columns `[bhp,u]`. A perturbed
one-cell state confirms exact control and connection restoration. The solve uses Bundle W's
bounded Newton budget, well tolerance, and relative BHP chop, with no u magnitude clamp.

The update seam is route-aware rather than globally bypassing well recovery. The selected well
uses `(bhp,u)` only when nested solve is enabled; every non-selected well retains historical
Relax or `(bhp,q...)` NestedSolve behavior. The OPM-aligned acceptance well check uses the same
route dispatch. This also corrects the G4b2 FlowResv branch's accidental omission of producer
post-processing.

Non-live evidence: `flow_resv` 11/11 and `wells_inner` 12/12 pass; locked FIM coverage passes all
11 curated cases; IMPES 5/5 passes unchanged. Shared coverage passes its first three cases and
reproduces the existing closed-system `rate_history` length mismatch (`2` versus `1`). Next is
the clean committed-tree, one-step exact trace with `FIM_FLOW_RESV_INJECTOR=1`,
`FIM_NESTED_WELL_SOLVE=1`, and the held Y2 raw lifecycle. Do not run six steps unless evaluation
1 has `c_s≈u`, converged control/connection rows, and no retry.

### G4b3 committed-tree oracle: mechanism passes (2026-07-20)

From `4cdea38`, the exact 10x10x3 first step with Y2 raw primary variables, RESV, nested solve,
and one-substep cap accepted the full `0.25 day` with no retry. Seven applied updates precede the
converged entry (driver count eight), the same applied count as Flow's first step and G4b2c.

Evaluation 1 now has `u=76,923.07692`, `c_s=76,923.07692`,
`R_perf=-1.164e-10`, `R_ctrl=5.684e-14`, and source `-76,923.07692`. This closes the G4b2c
incomparability (`c_s=133,639.380`, `R_perf=56,716.303`). Oil MB remains `3.493e-3` versus Flow
`1.8375e-3`; gas MB moves from G4b2c `4.526e-3` to `1.737e-3` versus Flow `2.8814e-3`.
Therefore G4b3 passes its mechanism gate and improves gas-MB distance, but has not demonstrated
a six-step iteration improvement. G4b4 is now authorized with no solver-policy changes.

### G4b4 clean six-step closeout (2026-07-20)

The main workspace was not a valid committed-tree oracle because unrelated user edits include
FIM linear/frontend files. Created a clean detached worktree at `653868e` and ran:

```text
FIM_TRACE_FILE=/tmp/ressim-g4b4.trace FIM_TRACE_DT_BELOW=1 \
FIM_Y1J_GRID=10 FIM_Y1J_FLAVOR=opm FIM_Y1J_STEPS=6 \
FIM_Y2B_RAW_SATURATION=1 FIM_FLOW_RESV_INJECTOR=1 FIM_NESTED_WELL_SOLVE=1 \
cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib \
  fim::timestep::phase5_repro::repro_gas_rate_10x10x3_y1j \
  -- --ignored --nocapture --exact
```

All six `0.25 day` trials accept with zero retry. Applied updates are `7,3,3,4,3,3` (23 total)
and linear iterations `18,9,9,10,8,7` (61 total). Flow is `7,5,4,3,4,3` (26) and
`8,5,4,3,4,3` (27). G4b2c was `7,4,3,3,3,3` (23) and 64 linear iterations. Therefore G4b4
recovers three linear iterations but worsens the nonlinear sequence's L1 distance from Flow
`3 -> 5`; it does not change the total.

The well lifecycle itself remains exact across reference refreshes: every report's evaluation 1
has `c_s=u`, `|R_perf|<=2.01e-9`, `|R_ctrl|<=5.69e-14`, and source `=-u`. Two single-run elapsed
observations are `0.528/0.559 s`, indistinguishable from G4b2c `0.545 s` and still roughly
`6.6-7.0x` Flow's `0.08 s`. Verdict: scoped structural mechanism passes and stays default-off;
standalone convergence improvement is not supported. Next build a comparable OPM-side
injector-cell reservoir-row partition oracle, then audit oil/gas accumulation, face flux, and
the now-matched well source at evaluation 1. Do not start G5 while the tracked cell remains
saturated with `Sg` active, and do not tune acceptance, damping, linear routing, or controller.

### G4c0/G4c1 injector-cell partition and reciprocal PVDG (2026-07-21, provisional dirty tree)

Built an observation-only partition on both production paths. ResSim uses the exact AD
accumulation, face-flux, and routed source helpers and logs `total`, the assembled row, and their
delta. The new all-cell/all-component unit gate reconstructs to machine precision. OPM was built
from exact `release/2026.04/final` commit `b82f21dba405286c4c4446614dd3bf9cdebf7a2c` with the
tracked `opm/diagnostics/g4c0-reservoir-partition.patch`. The first attempted generic-FV hook was
correctly rejected because Flow uses the TPFA fast path and emitted no trace. The final TPFA hook
logs each face, accumulation, source, and assembled row; the run completes with canonical
`26/27` nonlinear/linear counts. OPM rate values are multiplied by `21600 s`.

At cell 0, evaluation 0, Flow versus ResSim before the fix is:

| Term | Flow | ResSim |
| --- | ---: | ---: |
| oil z+ flux | `0.877958` | `0.877861` |
| gas z+ flux | `70.236654` | `70.228917` |
| gas source | `-20,312.500` | `-19,230.769` |
| gas total | `-20,242.263` | `-19,160.540` |

This pins the first mismatch to PVDG/RESV conversion rather than face physics. OPM source read
confirms `DryGasPvt` stores/interpolates `1/Bg` and `1/(Bg*mu_g)`. At 200 bar the exact deck gives
`Bg=1/162.5=0.006153846`, so the `500 rm3/day` target is `81,250 Sm3/day`; ResSim's direct-Bg
interpolation gave `0.0065` and `76,923.077 Sm3/day`.

G4c1 changes shared `PvtTable` f64/AD interpolation and analytic `dBg/dp` to those reciprocal
semantics. Evaluation 0 source now matches exactly. At evaluation 1, ResSim gas terms become
accumulation `2,490.73`, faces `13,076.95/13,076.95/6,578.60`, source `-20,312.5`, total
`14,910.74`; Flow is `1,903.19`, `2,556.34/2,556.34/1,288.48`, `-20,312.5`, total
`-12,008.14`. Because the engines arrived at different states after correction 0, this selects a
matched-state Jacobian/update audit rather than a flux patch.

Release six-step G4c1: residual-evaluation counts `[10,5,4,4,4,4]`, hence applied updates
`[9,4,3,3,3,3]` (25); linear iterations 62; `0.576-0.579 s`. G4b4 was 23/61/
`0.528-0.559 s`; Flow is 26/27/`0.08 s`. Fidelity is improved, but the speed gap is unchanged and
the first report step costs two more updates. Verdict: provisional correctness fix, not a
convergence promotion. Next G4c2 compares evaluation-0 Jacobian/update blocks with all policy and
G5 held.

Validation: PVT `4/4`, PVT behavior `7/7`, AD assembly `13/13`, Flow-RESV `11/11`, IMPES `5/5`,
Buckley-Leverett `3/3`, and owned FIM coverage `11/11`. The `all` coverage script again passes its first three shared controls and
stops at the documented pre-existing closed-system `rate_history` length `2` versus `1`; it is not
attributed to G4c1.

Held/missing coupled semantics remain explicit: this slice proves in-table PVDG segment
interpolation only; OPM-style reciprocal extrapolation beyond the table remains unimplemented.
PVTO reciprocal interpolation, full StandardWell variables, BHP switching, and multi-perforation
allocation are also not claimed by this result.

### G4c2 matched evaluation-0 Jacobian/update oracle (2026-07-22, diagnostic)

The first Flow MatrixMarket capture was an invalid well-coupled oracle: the default export writes
the reservoir matrix before matrix-free well contributions are materialized. The valid capture
uses `--matrix-add-well-contributions=true --linear-solver=cpr_quasiimpes`; changing the linear
preconditioner changes the later solve trajectory, but not the evaluation-0 nonlinear assembly
exported before that solve. Exact commands and unit/sign mappings are recorded in
`opm/diagnostics/README.md`.

The valid oracle exposed a more basic comparator mismatch. The tracked Flow deck has no `DRSDT`,
so `maxGasDissolutionFactor()` is effectively unbounded and cell 0 begins undersaturated with
`Rs=80` as the third primary. The historical ResSim exact driver forced
`gas_redissolution_enabled=false`; its DRSDT0 cap makes the same state saturated with `Sg=0` as
the third primary. The Flow Jacobian confirms this dynamically: its evaluation-0 third column is
an Rs derivative, while evaluation 1 is an Sg derivative after gas appears. Thus the prior claim
that this exact comparator remained in Sg throughout was incorrect.

`FIM_Y1J_GAS_REDISSOLUTION=1` now provides a default-preserving matched diagnostic. With it, the
post-well-Schur cell-0 blocks, after mapping Flow oil/water/gas rows to ResSim water/oil/gas and
Pa/rate units to bar/timestep-integrated units, are:

| Row | Flow `[dp, dSw, dRs]` | ResSim `[dp, dSw, dRs]` |
| --- | --- | --- |
| water | `[0.00135, 100.000008, 0]` | `[0.0006, 100, 0]` |
| oil | `[14.6282695, -80.4048336, -0.1645116]` | `[14.6273067, -80.4010017, -0.1426161]` |
| gas | `[1170.261648, -6432.38712, 56.0611368]` | `[1170.18454, -6432.08013, 57.8094215]` |

The mapped cell-0 RHS is Flow `[0,-0.87795814,20242.2629]` versus ResSim
`[0,-0.87786146,20242.27108]`. A dense solve of the exported Flow matrix gives the cell-0 raw
correction `[dp=9.52669 bar,dSw=-1.28610e-4,dRs=293.43965]`; ResSim's forced-direct solve gives
`[6.903753,-4.142252e-5,293.4539]`. The hydrocarbon correction agrees within about `0.005%`.
Pressure/Sw remain globally sensitive to the water-pressure derivative and smaller PVT-column
differences, but those do not justify a guessed G4 patch.

OPM's `BlackOilPrimaryVariables::adaptPrimaryVariables()` also confirms that an overshooting Rs
switches to Sg and writes exactly `0.0`; ResSim's existing OPM-style adaptation already does the
same. Therefore G4c2 closes as diagnostic, not as a convergence promotion. The next bounded work
is G5a: make DRSDT semantics explicit and identical in both exact-deck paths, then compare the
first post-switch evaluation. Full StandardWell variables, BHP switching, multi-perforation
allocation, reciprocal PVTO/extrapolation, solver acceptance, damping, linear routing, and
timestep control remain held or unimplemented and are not refuted here.

Validation: primary-state lifecycle tests `13/13`, the matched release one-step diagnostic `1/1`,
and the owned FIM coverage bucket `11/11` pass. The shared bucket again passes its first three
controls and stops at the documented pre-existing closed-system `rate_history` length `2` versus
`1`; G4c2 does not touch that path.

### G5a matched primary and first post-switch well lifecycle (2026-07-22)

The oracle contract is now explicit: the Flow deck uses `DISGAS` and intentionally omits
`DRSDT`, so its dissolved-gas increment is unlimited; ResSim selects the same native diagnostic
with `FIM_Y1J_GAS_REDISSOLUTION=1`. Both begin with `Rs=80`, overshoot the saturation limit on
the first correction, and adapt to saturated `Sg=0`. Primary substitution itself therefore
passes its oracle.

The dependency audit before interpreting evaluation 1 was:

| Coupling | G5a status |
| --- | --- |
| stored primary meaning and switch | matched: `Rs -> Sg=0` |
| update/chop and acceptance | held at the existing Y2/OpmAligned route |
| accumulation and endpoint phase state | observed at `Sg=0`; numerical p/Sw states already differ |
| selected well primary/equations | typed surface `u` and shared residual rows matched |
| connection flow-direction lifecycle | mismatch found and corrected on the selected route |
| full StandardWell/BHP switching/multi-perf | unimplemented and not claimed |
| PVTO reciprocal/extrapolation and residual derivative gaps | held; still open |
| linear policy and timestep controller | held |

Before the correction, ResSim's first raw outer update moved cell pressure to about `206.273 bar`
and BHP to `196.905 bar`. Its injector connection clamp then produced `q=0`; that branch has zero
BHP derivative, leaving `c_s=0`, `r_perf=-81250`, and no gas source. A Newton-only local `(bhp,u)`
solve cannot cross that flat branch. Flow remains injecting at evaluation 1. OPM source confirms
that injecting perforations use total reservoir mobility; source persistence plus that code is
evidence, by inference, for restoring the selected ResSim connection to the injecting branch.

`solve_flow_resv_well_locally()` now does that narrowly. When selected `u>0` but the current
connection is not injecting, it evaluates the frozen total-mobility connection one bar above cell
pressure, inverts that exact linear drawdown law for `q=-u*Bg`, and then leaves the ordinary shared
two-row Newton solve and convergence test authoritative. The route is already default-off and
single-perforation/no-BHP-limit by construction. It is not a partial claim about complete OPM
StandardWell behavior.

Afterward evaluation 1 retains `q=-485.9321926 rm3/d`, `u=81250 Sm3/d`, source `-20312.5`, and
both selected rows converge. ResSim's gas partition is accumulation `1781.450`, faces
`2124.782/2124.782/1054.648`, source `-20312.5`, total `-13226.838`; Flow is approximately
`1889.651`, `2548.430/2548.430/1283.847`, `-20312.5`, `-12042.14`. This is not a same-state flux
refutation: evaluation 0 already corrected ResSim to `p≈206.273, Sw≈0.149962`, whereas Flow's
dense correction gives `p≈209.527, Sw≈0.149871`.

On provisional dirty tree `991d6e3`, the matched six-step route needs residual evaluations
`[9,5,5,4,4,4]`, applied updates `[8,4,4,3,3,3]` (25), 59 linear applications, zero cuts, and
`0.588 s`. Flow uses applied updates `[7,5,4,3,4,3]` (26), 27 linear iterations, and `0.08 s`.
The historical DRSDT0 control remains `[10,5,4,4,4,4]`; product defaults are unaffected. The
first-step matched route improves from 14 to 9 residual evaluations, but runtime is unchanged.

Next authorized checkpoint is G4c3, not a broader G5 rewrite: attribute the evaluation-0
pressure/Sw correction difference to the mapped water-pressure and PVTO/Rs-column derivatives.

Validation after the G5a behavior change: Flow-RESV `12/12`, local-well `12/12`, AD assembly
parity `7/7`, locked DRSDT0 lifecycle `1/1`, and the owned FIM bucket `11/11` pass. The shared
bucket again passes its first three controls and stops at the known pre-existing closed-system
`rate_history` length `2` versus `1`. No WASM matrix was run because this selected RESV route and
its environment selector are native-only/default-off and are not exposed in the WASM product.

### G4c3 same-state pressure/Sw derivative attribution (2026-07-22, diagnostic)

The mapped water-pressure mismatch has an exact source-level explanation. With `PV=100 m3`,
`Sw=0.15`, `c_r=4e-5/bar`, and PVTW `c_w=5e-5/bar`, Flow's entry is
`PV*Sw*(c_r+c_w)=0.00135`. ResSim's FIM component inventory divides by constant `b_w`, producing
only `PV*Sw*c_r=0.0006`. IMPES already includes `c_w*Sw` in its pressure compressibility, so a
blind shared scalar change would not be a valid repair.

The existing sequence capture and sparse-LU lab now provide a test-only full-system
counterfactual. On the same evaluation-0 RHS, all variants have finite corrections and full-system
relative residual below `2.12e-16`:

| Variant | Cell-0 `[dp bar, dSw, dRs]` |
| --- | --- |
| ResSim original | `[6.903752614, -4.142251568e-5, 293.4538820]` |
| add only missing `PV*Sw*c_w` | `[6.837656177, -9.230835840e-5, 293.4538820]` |
| apply only mapped non-water cell-block deltas | `[9.627512092, -5.776507255e-5, 293.4396461]` |
| combine both | `[9.469611929, -1.278397610e-4, 293.4396463]` |
| Flow | `[9.52669, -1.28610e-4, 293.43965]` |

Thus water compressibility accounts for most of the Sw gap, while the non-water PVT block
accounts for the pressure and Rs movement; together they explain essentially the full correction.
This is a same-state matrix result, not trajectory inference and not a linear-backend verdict.

Pinned OPM source identifies the likely coherent semantics: constant-compressibility water uses
PVTW reference pressure and evaluates inverse Bw, while `LiveOilPvt` constructs/evaluates
`1/Bo` and `1/(Bo*mu_o)`. ResSim directly interpolates oil Bo/mu and its exact driver includes only
the four saturated PVTO rows, omitting the deck's four undersaturated endpoint rows. The
counterfactual deliberately applies measured deltas rather than claiming those source mechanisms
individually reproduce every entry.

G4c3 closes diagnostic-only. G4c4 must design the coupled PVTW/PVTO lifecycle first: reference
pressure, accumulation, flux, connection/source conversion, reporting/scaling, AD/legacy, and
relevant IMPES/shared consumers. A water-storage-only edit would be a partial OPM port and is not
authorized. G5, StandardWell expansion, acceptance, linear routing, and controller remain held.

Validation: the release exact matched capture completes one report step with zero cuts, and the
ignored `solver_lab_g4c3_water_storage_counterfactual` gate passes with finite corrections and
full-system relative residual `2.12e-16`. `git diff --check` and the reference manifest JSON check
pass. No production assembly or runtime behavior changed in G4c3, so the G5a focused/FIM/shared
behavior gates recorded immediately above were not rerun a second time for this test-only lab.

### G4c4 coherent PVTW/PVTO lifecycle (2026-07-22)

Implemented the source-derived lifecycle as one shared change. `set_initial_pressure()` now also
establishes the water-PVT reference pressure. Water inverse FVF uses OPM's
`(1 + X*(1 + X/2))/Bw_ref`, `X=cw*(p-p_ref)`, in accumulation, face flux, wells, scaling/CNV,
well-control conversion, reporting, and water density, with matching scalar/AD derivatives.
PVTO pressure and Rs interpolation now operates on `1/Bo` and `1/(Bo*mu_o)`. Both exact native
fixtures now contain the deck's four saturated and four undersaturated rows. This is coherent for
the proven in-table segments; OPM's out-of-table reciprocal extrapolation remains unimplemented.

The matched direct first correction is `[9.740529,-1.314971e-4,293.4397]`, versus Flow
`[9.52669,-1.28610e-4,293.43965]`. At evaluation 1, oil accumulation/faces are approximately
`-4.662 / 22.317 / 22.317 / 11.251` versus Flow `-4.616 / 22.075 / 22.075 / 11.121`; gas is
`1909.684 / 2585.267 / 2585.267 / 1303.369` with the exact `-20312.5` source, versus Flow
`1889.65 / 2548.43 / 2548.43 / 1283.85`. The remaining terms are roughly 1-1.5%, not the prior
16-18% divergence.

On the provisional dirty tree, the matched six-step route accepts six full `0.25 day` steps with
zero cuts. Residual evaluations are `[8,5,5,4,3,4]`, applied updates `[7,4,4,3,2,3]` (23), linear
applications 55, and elapsed time `0.597 s`. Flow is `[7,5,4,3,4,3]` updates (26), 27 linear
iterations, and `0.08 s`. The original more-than-tenfold exact-case wall gap is therefore about
7.5x after G4c4, although linear work remains about 2x and per-application cost remains material.

Validation: PVT `6/6`, PVT properties `7/7`, flow-RESV `12/12`, AD assembly parity `7/7`, FIM
SPE1 `3/3`, wells `5/5`, depletion `3/3`, IMPES `5/5`, Buckley-Leverett `3/3`, rebuilt WASM, and
`validate:product` (`644` frontend tests plus build) pass. The shared bucket passes its first three
contracts and stops at the known pre-existing closed-system `rate_history` `2` versus `1`.
The rebuilt browser matrix is bounded but not invariant: water remains `8/4/4`, heavy water moves
`25 -> 23`, gas 20x20x3 moves `2 -> 4`, and the six-step gas control moves `14 -> 28` total
substeps. Next is G4c5 same-state remainder attribution and linear-cost profiling, not G5 or
controller/acceptance tuning.

### G4c5 guided PVTO derivative and coarse linear-cost audit (2026-07-22)

The remaining correction was another source-identifiable part of the same G4c4 lifecycle, not a
new residual family. OPM `LiveOilPvt` configures `UniformXTabulated2DFunction` with
`InterpolationPolicy::LeftExtreme`. Between neighboring Rs branches, fraction `t` and branch
pressure-origin shift `shift` guide the evaluations to `p_low=p-t*shift` and
`p_high=p+(1-t)*shift`. G4c4 evaluated both branches at `p`; that preserved PVTO values at an Rs
knot but changed the Rs derivative. The scalar and generic/AD paths now share the guided rule and
a knot-derivative regression test.

The exact first correction moves from G4c4
`[9.740529,-1.314971e-4,293.4397]` to
`[9.529566823,-1.286491521e-4,293.4396825]`, against Flow
`[9.52669,-1.28610e-4,293.43965]`. At evaluation 1, ResSim oil accumulation/faces are
`-4.647350 / 22.091873 / 22.091873 / 11.134954` and gas is
`1903.316 / 2556.422 / 2556.422 / 1288.512`, versus Flow oil
`-4.616 / 22.0746 / 22.0746 / 11.1207` and gas
`1889.65 / 2548.43 / 2548.43 / 1283.85`. The same-state correction oracle is now closed to about
`0.03%` or better; remaining individual residual terms are about `0.1-0.7%`.

Timing the unchanged dense-threshold route attributed about `417/553 ms` to linear work and
`400 ms` to coarse preconditioner construction. An offline replay of seven sequential 300-row
coarse systems measured median dense inverse `31.733 ms`, sparse LU `6.599 ms`, and the existing
ILU0/BiCGSTAB path `0.193 ms`. ILU0/BiCGSTAB had zero failures, median relative residual
`5.197e-7`, maximum `7.879e-7`; LU and inverse corrections agreed to `8.335e-13`. This satisfies
the earlier FIM-LINEAR-011 retry condition, so the dense cutoff changes from 300 to 299 and routes
the exact 300-cell case through the already-tested iterative implementation.

The final matched six-step route uses residual evaluations `[8,5,5,4,3,4]`, applied updates
`[7,4,4,3,2,3]` (23), 55 Krylov iterations, zero cuts, and `0.191 s`, versus G4c4 `0.597 s`
and Flow `0.08 s`. Total measured solver/linear/preconditioner time is
`186.748/56.035/27.416 ms`. Thus the setup hotspot is removed without loosening convergence, and
the native wall gap is about 2.4x; G4c6 subsequently established that the remaining metric is
55 ResSim Krylov iterations versus Flow's 27, not 55 reservoir solve calls.

Validation passes: PVT `7/7`, PVT properties `7/7`, AD assembly `7/7`, Flow-RESV `12/12`, linear
module `28/28`, BL `3/3`, locked FIM `3/3`, FIM bucket `11/11`, IMPES `5/5`, frontend `644`, WASM
rebuild and product build. The shared bucket passes its first three tests and stops at the known
pre-existing closed-system `rate_history` length `2` versus `1`. Rebuilt-WASM substep counts are
unchanged from G4c4: water `8/4/4`, gas `4`, six-step gas `28`, heavy water `23`. The 300-cell gas
control becomes materially faster, while G4c4's gas-count tradeoff remains. Results are
provisional on dirty tree `1db2db8`.

Next is G4c6: reconcile what ResSim's 55 applications and Flow's 27 iterations count, including
well-inner and repeated reservoir solves per applied Newton update. G5, tolerances, acceptance,
damping, controller and coarse backend remain held.

### G4c6 linear-work semantics and cross-scenario disposition (2026-07-22)

Behavior-neutral counters now distinguish residual/Jacobian evaluations, corrections committed to
the nonlinear iterate, reservoir linear-system solve calls, and the sum of Krylov iterations across
those calls. The matched six-step gas route reports per step:

| Step | Residual evaluations | Applied updates | Reservoir solves | Krylov iterations |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 8 | 7 | 7 | 14 |
| 2 | 5 | 4 | 4 | 9 |
| 3 | 5 | 4 | 4 | 11 |
| 4 | 4 | 3 | 3 | 8 |
| 5 | 3 | 2 | 2 | 5 |
| 6 | 4 | 3 | 3 | 8 |
| **Total** | **29** | **23** | **23** | **55** |

Fresh Flow 2026.04 reports 32 linearizations/residual evaluations, 26 Newton updates, and 27
linear iterations. Its `linearizations` count includes the converged final evaluation and is not
a solve-call count. ResSim has zero retries and exactly one reservoir solve per applied update.
The nested well Newton is executed inside state update and cannot explain the 55. The valid
remaining comparison is therefore `55/23=2.39` versus Flow `27/26=1.04` Krylov iterations per
solve—not 55 versus 27 solve calls and not an outer-Newton deficit.

The already-implemented source-complete Flow linear lifecycle is the required anti-repeat control.
On current G4c5 physics it gives 28 updates, 28 solve calls, 45 Krylov iterations, and `0.646 s`.
It reduces iterations per solve but worsens nonlinear work and is more than three times slower than
the live `0.191-0.207 s` route because of setup cost. This reproduces `FIM-Y2D6`'s no-promotion
verdict; another whole-lifecycle or outer-BiCGSTAB attempt without new evidence would repeat it.

The updated scenario table is `docs/SOLVER_COMPARISON_SUMMARY.md`. Its decision-relevant result is
that exact gas is now roughly 2x Flow with the repository's strongest local correctness match,
whereas water-heavy remains about `5.8 s`/50 FIM substeps against Flow `0.04 s`/one step. G4c6
closes diagnostic-only with no solver policy change. Prioritize the water-heavy nonlinear
trajectory; return to exact-gas Krylov policy only with new same-preconditioner evidence outside
the already-completed Y2D4-D6 experiments. Results remain provisional on dirty tree `1db2db8`.

Validation: focused counter semantics `2/2`, locked FIM `3/3`, curated FIM bucket `11/11`, and
product validation (frontend `644`, IMPES `5/5`, WASM rebuild, Vite build) pass. The rebuilt FIM
control matrix preserves G4c5 counts: water `8/4/4`, gas `4`, six-step gas `28`, heavy water `23`.
The shared bucket again passes its first three checks and stops at the documented pre-existing
closed-system history-length mismatch (`2` versus `1`). `cargo fmt --check`, script smoke, and
`git diff --check` are final handoff gates.

### WATER-001 corrected oracle and FIM/IMPES mechanism split (2026-07-22)

The apparent common slowdown contained an oracle defect. The Flow deck used `COMPDAT 0.1` as if
it were the ResSim `well_radius=0.1`; Eclipse defines that field as diameter. The resulting
radius `0.05` predicts the measured 1.23 PI mismatch. With `COMPDAT 0.2`, Flow evaluation-0
water MB/CNV becomes `0.83078/358.90`, matching ResSim `0.8308/358.9`; oil matches likewise.
The corrected Flow run remains one uncut day, 11 Newton updates, 12 evaluations, 13 linear
iterations, and `0.03 s` reported simulation time.

`FIM_WATER_FULL_TARGET_PROBE=1 FIM_NESTED_WELL_SOLVE=1` bypasses only the outer controller and
runs the unchanged Newton solve at `dt=1`. It fails after 20 updates/20 solves/29 Krylov
iterations with water residual `5.3247e2`, MB `1.2341`, and good CPR reduction near `9e-7`.
The trace localizes a repeatable injector-cell Sw `0.3↔0.5` cycle. Flow instead reduces water MB
`0.83078 -> 0.31375` after its first update; FIM changes `0.8308 -> 0.8277`.

IMPES is a separate explicit-stability mechanism. A diagnostic-only `--max-sat-change` sweep
changes accepted history counts `129 -> 73 -> 46 -> 29` for limits `0.05 -> 0.1 -> 0.2 -> 0.5`;
average pressure, injection and production move materially, so loosening it is not a correctness-
neutral speed fix. WATER-001 therefore refutes a shared active cause. WATER-002 will compare the
corrected-deck evaluation-0 Flow/FIM matrix and raw first correction before any physics change.

### WATER-002 matched first correction and well-Schur attribution (2026-07-22)

Fresh Flow 2026.04 evaluation-0 MatrixMarket output was captured from the corrected-diameter deck
with `matrix-add-well-contributions=true` and `cpr_quasiimpes`. The two-phase map is Flow rows
`[oil,water]`, primaries `[Sw,pressure(Pa)]`, versus ResSim rows `[water,oil]`, primaries
`[pressure(bar),Sw]`, with one-day row scaling. ResSim's third cell slot is a structural two-phase
pin: every pinned row has zero RHS and no off-diagonal feedback, so projecting it is exact.

The Flow matrix solve gives cell-0 raw `[dp=+110.7034824844 bar,dSw=+160.3607888527]`; enabling
Flow's max-update reporting independently gives `DP=1.107e7 Pa, DS=1.604e2`. ResSim's exact
well-Schur solve gives cell 0 `[-196.9323897384,0.7927450128]` and producer cell 143
`[-198.631430969,7.94526e-5]`. Thus the huge Flow `dSw` is real, not export precision failure.
Both engines chop injector saturation to `+0.2`; the decisive applied difference is pressure.
ResSim clips its raw negative correction to `-90 bar` and reaches cell-0 `p=210,Sw=0.3`, where
water MB is still `0.8277`; Flow's raw pressure direction is upward and its MB falls to `0.31375`.

The ranked mapped coefficients identify the first block: after ResSim well elimination,
`water@cell0/Sw=17964.83383` versus Flow `20.0000016`, and
`oil@cell143/Sw=-18018.80926` versus Flow `-20.0000016`. These two entries dominate a mapped
Frobenius delta of `3.3749`; the uneliminated ResSim reservoir block differs by only `0.007323`,
and the RHS by `0.022317`. A diagnostic reservoir-only solve recovers Flow's positive injector
pressure direction and very large raw saturation movement, though the ill-conditioned magnitude
remains sensitive to the smaller reservoir/RHS differences.

WATER-002 closes diagnostic-only. Removing well coupling is not a valid production fix: Flow's
well primary/equations, BHP constraint, connection solve, source application, recovery, and
post-update inner solve must be treated as one lifecycle. WATER-003 is the bounded default-off
source-complete replay. Controller, acceptance, chop limits, nonlinear iteration cap, and IMPES
policy were unchanged. Validation: Flow one-day replay `11/12/13`; exact ResSim capture probe
`1/1`; ignored WATER-002 solver lab `1/1` with both exact-solve residual reductions below
`1e-10`; focused Rust gates and formatting are listed at handoff.

### WATER-003 endpoint-clipped well/property lifecycle (2026-07-22)

The Flow source trace resolves WATER-002's large Schur coefficient without changing the Schur
formula. `PiecewiseLinearTwoPhaseMaterial::evalAscending_` returns `yValues.front()` when the AD
saturation is at or below the first SWOF sample. The returned constant carries zero derivative.
ResSim's scalar `d_k_ro_d_sw()` already returns zero at exact `Swc`, but its generic Corey path
uses an equality-inclusive clamp and retained `dkro/dSw=-2.5`. Because `StandardWell::getMobility`
uses the live reservoir intensive quantities, that derivative entered the BHP connection blocks
and produced the `~1.8e4` post-Schur terms seen in WATER-002.

The replay adds a native-only, default-false simulator flag. It freezes exact two-phase Corey
endpoint derivatives without changing property values and routes through
`phase_mobilities_for_state_generic`, so FIM reservoir fluxes, perforation component equations,
BHP constraint/elimination, recovery and nested well update all use the same contract. This is
not the reservoir-only/no-well counterfactual: full coupling remains assembled and solved.

| Coupled lifecycle semantic | WATER-003 treatment |
| --- | --- |
| State / initial meaning | Same two-phase `p,Sw` state and structural third-row pin as WATER-002 |
| Update and chops | Existing Appleyard/per-cell pressure safeguard and `dSw=0.2` chop held |
| Accumulation | Existing raw iterate and PVTW accumulation held |
| Endpoint-clipped properties | Exact Corey endpoints return constant AD values for every FIM mobility consumer |
| Phase presence / adaptation | Two active phases; no hydrocarbon primary adaptation added or removed |
| Well primaries / equations | Existing explicit BHP and connection-q rows, component sources and BHP constraint retained |
| Elimination / recovery / inner solve | Exact well Schur, recovered well correction and nested post-update solve retained |
| Linear acceptance | Existing CPR/full-system residual contract, tolerance and iteration budget held |

The locked first-update probe reports injector `p=390.0 bar, Sw=0.3` and evaluation-1 water MB
`0.3137562888362`, versus Flow `0.31375`; the baseline state was `p=210, Sw=0.3`, water MB
`0.8277`. The full held-controller comparison on the same release binary is:

| Route | Accepted substeps | Nonlinear retries | Native time | Final avg pressure | Injection | Oil production |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Default | 50 | 2 | `4.385 s` | `376.1268 bar` | `3262.61 Sm3/d` | `2925.04 Sm3/d` |
| WATER-003 | 10 | 0 | `0.775 s` | `374.0939 bar` | `3249.99 Sm3/d` | `2935.16 Sm3/d` |
| Flow | 1 | 0 | `0.03 s` simulation | `352.5328 bar` | `2996.32 Sm3/d` | `2609.51 Sm3/d` |

The direct `dt=1` replay still fails the unchanged 20-update cap, ending with global MB
`8.91e-3`; Flow converges in 11. Therefore WATER-003 passes its first-split mechanism gate and
materially improves controlled work, but does not pass a full-trajectory/output promotion gate.
WATER-004 must compare the matched evaluation-1 matrix/RHS/correction, including the fact that
the Flow oracle uses a rounded piecewise SWOF table while the ResSim fixture uses analytic Corey
curves. Controller, acceptance, chops, iteration cap, linear policy and IMPES remain held.

Validation: endpoint derivative `2/2`, well-Schur full-system `4/4`, locked FIM `3/3`, curated
FIM `11/11`, IMPES `5/5`, Buckley-Leverett `3/3`, first-update gate `1/1`, native default/replay
controls, rebuilt WASM, frontend `644/644`, typecheck/lint and product build pass. The shared
bucket again passes three contracts and stops at the known closed-system history-length mismatch
(`2` versus `1`). The broad AD sweep passes 12/13 and exposes an unrelated rate-controlled-well
column-occupancy mismatch at row 6; the BHP-controlled WATER-003 structural case passes. Default
WASM bounded counts remain water `8/4/4` and gas `4`; the rebuilt heavy run observed `17` history
entries versus the previously recorded `23`. WATER-003 cannot explain that movement because its
selector is native-only and false in WASM, so no causality or promotion claim is attached to it
on this dirty tree.

### WATER-004 evaluation-1 matrix/correction gate (2026-07-22)

Flow was rerun with MatrixMarket output and its `nit_1` pair was compared with the sequential
endpoint-replay ResSim capture `00001`. `solver_lab_water004_matched_second_correction` applies
the established oil/water row, `Sw/pressure` primary, day and Pa-to-bar mapping, retains the
complete ResSim well Schur, and solves both systems directly. It reports mapped matrix/RHS
relative deltas `1.662526792e-1` / `6.280250884e-3`. Injector raw corrections are Flow
`[-38.31824150 bar,+1.893890636]` and ResSim `[-111.2296868 bar,+1.528765205]`; the held
ResSim saturation chop produces `+0.2`.

This does not qualify as a strict same-state comparison: Flow exports its linear system but not
the intermediate cell state. The already-matched global water MB is supporting evidence only.
The rounded SWOF interior slope at `Sw=.3` is `(.469,-2.031)` for `(krw,kro)`, while ResSim's
analytic Corey is `(.625,-1.875)`. Therefore WATER-004 is **INCONCLUSIVE**, and cannot authorize
a controller, cap, chop, linear-policy or IMPES change. WATER-005 is the default-off complete
rounded-SWOF mobility replay followed by this same capture.

### WATER-005 rounded-SWOF complete mobility replay (2026-07-22)

The corrected Flow deck's nine rounded SWOF rows are now a named native-only/default-false FIM
diagnostic. Both f64 and AD mobility routes select the same constant-extended, piecewise-linear
table; therefore reservoir fluxes, BHP/perforation equations, well Schur/recovery, and nested
well updates cannot mix a Corey value with a SWOF derivative. At the `.3` knot, the table selects
the `.2-.3` segment and gives `dkrw=.469`, `dkro=-2.031`; at `.1` its AD derivatives are zero.

| Coupled lifecycle semantic | WATER-005 treatment |
| --- | --- |
| Stored state / primary meaning | Held: two-phase `p,Sw` and structural third-row pin |
| Update / chop | Held: existing pressure safeguard and `dSw=.2` cap |
| Accumulation and PVT | Held: existing PVTW/rock inventory implementation |
| Saturation properties | Matched for this fixture: rounded nine-row SWOF values, endpoint extension and left-knot AD slope |
| Phase presence / adaptation | Held: two active phases, no primary adaptation change |
| Well primary/equations | Matched property input: existing BHP/perforation equations consume the same scalar/AD mobility |
| Schur/recovery/inner solve | Held: complete existing elimination, recovery and nested update remain active |
| Linear acceptance/controller/IMPES | Held: no tolerance, budget, linear-policy, controller or IMPES change |

The held one-update gate reports `p=390`, `Sw=.3`, water MB `.3137499968979` (Flow `.31375`).
With fresh Flow `nit_1` and ResSim sequential capture `00001`, the existing direct solver lab
reports matrix/RHS deltas `.005581345789/.006279194785`, versus WATER-004's
`.1662526792/.006280250884`. Injector raw corrections are Flow
`[-38.31824150,+1.893890636]` and ResSim `[-39.01176062,+1.934341169]`; both keep the `.2`
saturation chop. This is valid evidence that the prior matrix gap was the rounded property law,
not a reason to change solver policy.

Under the held controller, the day accepts five substeps in `.281 s` (WATER-003: ten/`.775 s`),
ending at average pressure `375.6435 bar`, injection `3228.49 Sm3/d`, oil `2792.10 Sm3/d`; Flow
is `352.5328/2996.32/2609.51`. The direct day still exhausts the unchanged 20-update cap, but
reaches residual `8.46e-4` and MB `1.326e-6`. Thus the property mechanism is validated default
off, but no direct-day/output promotion is authorized. WATER-006 is limited to backend-neutral
fixed-policy acceptance/convergence measurement; limits, acceptance, chops, linear policy and
IMPES remain held.

### WATER-006 fixed-policy direct-day decomposition (2026-07-22)

The full WATER-005 direct trace establishes that the last six iterations are not a nonlinear
boundary or well failure. From iteration 14 forward, pressure/saturation chop counts are zero;
upwind flips drop `14,2,11,4,0,0`; well/perforation residuals are negligible; and the binding
water row is cell 419. Its residual decreases monotonically `1.171e0,3.471e-1,1.043e-1,
3.133e-2,9.401e-3,2.821e-3,8.462e-4`. The final MB `1.326e-6` is close but does not meet the
held convergence criterion, so accepting it or increasing the cap is explicitly outside scope.

`solver_lab_water006_late_tail_backend_neutral` re-solves the captured full 1300-row systems.
It publishes one RHS, full `||rhs-Jdx||`, reservoir/well partition and correction difference for
both backends. CPR reductions for iterations 14–19 are `2.210e-3,1.561e-3,1.194e-3,1.058e-3,
1.018e-3,1.007e-3`; sparse LU is `4.108e-15..3.509e-15`. CPR residual is entirely reservoir
rows, while its correction differs from sparse LU by `2.67%..0.618%` infinity-relative. This is
the accuracy floor that prevents the final tail from reaching the held criterion on time.

The forced-direct live control starts with the same WATER-005 first update but ends worse after
20 updates (`res=.0280`, `MB=6.37e-5`). Thus direct solves validate the same-state linear oracle
but cannot be promoted as a trajectory fix. WATER-006 is **DIAGNOSTIC COMPLETE**: do not alter
controller, cap, damping/chops, acceptance or IMPES. WATER-007 is limited to CPR stopping and
post-Schur recovery semantics on the same captured tail.

### WATER-007 CPR stop/recovery contract (2026-07-22)

`solver_lab_water007_cpr_stop_and_recovery_contract` records the production-faithful
well-Schur-recovered Krylov history for the iteration-19 capture, then repeats it with an
offline-only strict CPR option. Production reports convergence after three Krylov iterations:
estimated preconditioned residual `5.454328784e-7`, actual preconditioned residual
`5.469207148e-7`, and recovered full reduction `1.007231199e-3`. The recovered snapshot true
residual (`1.217949474e-4`) agrees with the full report exactly under the test tolerance.

The strict `relative_tolerance=1e-8,max_iterations=60` control is finite and reaches full
reduction `3.860326074e-7` (true residual `4.667927399e-8`), though it correctly remains
unconverged against that stricter target at the 60-iteration budget. Therefore well-Schur
recovery is refuted as the source of the WATER-006 floor; CPR's preconditioned stopping contract
is the measured mechanism. This is an offline diagnostic only. WATER-008 must first source and
measure the equivalent OPM norm/tolerance lifecycle and then perform a same-state sweep; no live
linear tolerance, budget, nonlinear/controller/cap/chop or IMPES change is authorized.

### WATER-008 OPM stopping-norm oracle (2026-07-22)

The source oracle overturns the previous numerical-equivalence assumption. Flow's
`setupPropertyTree.cpp::setupCPRW()` overrides the CPRW defaults to `.005` and 20, and
`FlexibleSolver_impl.hpp::initSolver()` constructs Dune `BiCGSTABSolver` with that `tol` as the
desired residual-reduction factor. The outer Flow criterion is therefore raw residual reduction.
ResSim's current CPR loop can instead accept when the re-applied preconditioned residual is under
the numeric threshold, while its full residual is still above that raw target.

The new `solver_lab_water008_same_state_tolerance_budget_sweep` reruns only the iteration-19
captured system. At `.005`, increasing `maxiter` from 20 to 60 changes nothing: it accepts at 3
iterations with full reduction `1.007231199e-3`. At 60, numeric targets `1e-4`, `1e-5`, and
`1e-6` give full reductions `1.931079858e-5`, `8.952550031e-7`, and `4.200655241e-7` at 5, 6,
and 9 iterations. All corrections are finite and recovered history agrees with the report.

WATER-008 is **DIAGNOSTIC COMPLETE**. It rules out a budget increase and establishes that a
matching Flow numeric tolerance must be enforced against the raw full residual, not reused as a
preconditioned threshold. The next bounded step is WATER-009: default-off raw-full-residual
acceptance, tested only on captured systems before any live trajectory run. Baseline remains
provisional because this is a dirty-tree replay.

### WATER-009 default-off raw acceptance (2026-07-22)

`require_raw_full_residual_acceptance` is default false. When selected it prevents the
preconditioned-`beta` and tiny-tail acceptance routes and bypasses the additional ResSim
per-family acceptance gate. The selected candidate must instead satisfy the raw numeric target;
the well-Schur wrapper still independently recomputes the original full-system residual after
recovery. No timestep, Newton, controller, cap, chop, well or IMPES setting changed.

The captured 14–19 tail contract is complete: finite corrections, RHS, full raw residual,
reduction, reservoir/well partitions, and snapshot/report equality are all available. At the
held Flow `.005` target it accepts every captured system in three iterations, with full reductions
`2.492910139e-3, 1.561255692e-3, 1.194199752e-3, 1.058216751e-3, 1.018074864e-3,
1.007231199e-3`; the residual is wholly reservoir-side. This proves that raw `.005` is a valid
source-aligned criterion for this isolated contract, but also proves it cannot be the missing
accuracy mechanism: `.005` permits the same linear error that leaves the direct nonlinear tail.

WATER-009 is **DIAGNOSTIC COMPLETE** and does not authorize a live A/B. Dune sees Flow's own
StandardWell-reduced operator, while ResSim verifies a recovered full operator; moreover Flow's
nonlinear acceptance, state update/chop and primary-variable adaptation have not been matched.
The next work must compare that coupled nonlinear lifecycle, consistent with the steer to
replicate Flow rather than tune ResSim in isolation. Results remain provisional on this dirty
tree.

### WATER-010 Flow nonlinear lifecycle and trajectory boundary (2026-07-22)

Source mapping shows that the contemplated lifecycle is already present as `OpmAligned`: Flow
computes reservoir CNV/MB and well convergence before solving; on failure it solves, applies the
per-cell update/chop, adapts primaries, refreshes intensives and reassembles. Defaults are MB
`1e-7`, CNV `1e-2`, with final-iteration MB/CNV relaxed to `1e-6`/`1`, and a two-update minimum
before acceptance. ResSim's OpmAligned implementation has the same entry order, minimum,
relaxed tier and per-cell chop. The water fixture is two-phase; no primary switch occurs, and
well residuals are clean under the nested-well mode.

Fresh Flow 2026.04 output (`/usr/bin/flow CASE.DATA --output-extra-convergence-info=steps,iterations
--solver-verbosity=3 --time-step-verbosity=3`) completes one day in 11 updates/12 evaluations.
The native direct ResSim replay used OpmAligned + endpoint/SWOF replay + nested wells and held
every controller/linear setting. Evaluations 0–2 are close; at evaluation 2 Flow oil/water
MB/CNV is `.28605/48.147` and `.45965/102.06`, versus ResSim `.2849/48.26` and `.4621/103.0`.
After update two the trajectories split: evaluation 3 Flow oil/water CNV is `41.516/50.531`,
ResSim is `62.31/27.72`. Flow then reaches final MB oil/water `5.99e-9/7.33e-9`; ResSim reaches
the previously diagnosed, unchopped and well-clean cell-419 tail.

WATER-010 is **DIAGNOSTIC COMPLETE**. It refutes a missing *ordering* or acceptance-rule
explanation within the already-implemented OpmAligned scope, but metrics cannot establish exact
intermediate state identity. WATER-011 must compare the second raw/applied update and the
post-update matrix/state representation. No live policy change is authorized; this dirty-tree
result is provisional.

### WATER-011 second update/matrix boundary (2026-07-22)

With the held direct water replay, Flow `nit_2` and ResSim evaluation 2 are compared after
eliminating ResSim's explicit wells and projecting out its structural third cell slot. In Flow's
physical `[oil, water] x [Sw, pressure]` representation, RHS differs `7.717314731e-3`, while
the matrix differs `1.296976395e-1`. The injector raw updates are Flow
`[+67.59004251 bar,+.1402643934 Sw]` and ResSim `[+69.80108205 bar,+.1377129975 Sw]`; both
water increments remain below `.2`, so saturation chopping cannot produce this boundary. The
producer's small water increments differ `.0002461530` versus `.0006933538`.

Flow does not export intermediate primaries or its applied pressure update in this mode. The
post-update-state comparison is consequently **INCONCLUSIVE**, rather than a reason to infer a
controller or damping defect. The observed matrix difference is nevertheless an earlier,
water-specific representation boundary than the next CNV report. Gas remains the successful
held control, and no live FIM/IMPES policy was changed.

### WATER-012 evaluation-2 reservoir/well decomposition (2026-07-22)

The fixed ResSim capture separates the original physical reservoir block from the same matrix
after explicit-well Schur elimination. Against the available Flow `nit_2` system, reservoir-only
delta is `.4078653104`, the Schur increment norm is `.3867683831`, and reduced delta is
`.1296974455`; RHS delta is `.007713485856`. The ranked residual entries are reservoir
saturation terms, and their Schur increments are zero. This is positive evidence that ResSim's
Schur algebra is corrective rather than the remaining cause.

The Flow side is not yet a strict same-policy oracle: Flow source forbids matrix materialization
under `system_cpr`, and the same-policy attempt aborts. A `cpr_quasiimpes` export exists only as a
diagnostic because that solver selection can change the evaluation-2 state. Result:
**INCONCLUSIVE for full Flow well parity**. The build-oracle detour was removed without changing
either solver. WATER-015 proceeds from the stronger local fact: the dominant remaining terms have
zero ResSim Schur contribution and are reservoir saturation derivatives. Flow source, rather
than a new ResSim-specific model, is the authority for the next implementation comparison.

### WATER-015 Flow two-phase reservoir source audit (2026-07-23)

The apparent exterior-derivative difference is an assembly-layout artifact. Flow treats each cell
as the AD focus, puts its flux derivative into both adjacent residual rows, then obtains the other
matrix column when the neighbor becomes the focus. The scalar exterior mobility/PVT in
`BlackOilLocalResidualTPFA::calculateFluxes_` is required one-sided AD behavior. The focused-pass
regression exactly reproduces ResSim's paired four-block result with rounded SWOF and gravity.

Storage comparison matches water/oil surface-volume inventories. Flow's reference-pressure
quadratic ROCK porosity differs from ResSim's committed-pressure exponential, but this affects
pressure terms and cannot explain the dominant saturation entries. No production policy changed.
WATER-016 will source-replay Flow's first two applied updates and establish a same-state reservoir
comparison without rebuilding OPM.

### WATER-016 Flow applied-state reconstruction audit (2026-07-23)

Source establishes the update semantics but also the hard observability limit. For the held
oil-water deck, Flow subtracts the linear correction, caps pressure at 30% of current pressure,
and scales each water/oil saturation pair to a maximum .2 change. No gas-variable adaptation
applies. However the retained MatrixMarket exports are `J` and RHS only; they do not contain the
CPR/matrix-free-well correction actually passed to the update routine. `solUpd_` is internal and
the public convergence path reports maxima only; `EnableWriteAllSolutions` is substep output,
not Newton-iterate output.

Accordingly an LU solution of the exported system would be a new artificial trajectory, not Flow
state reconstruction. The same-state matrix comparison remains **INCONCLUSIVE**. WATER-017 is
blocked on an observation-only applied-state/update dump from an already compatible Flow binary,
or an explicit decision to rebuild solely for that diagnostic. No runtime policy changed.

### WATER-017 instrumented Flow oracle: build path established, blocked on one dev package (2026-07-23)

Authorized the scoped rebuild WATER-016 asked for. It is not an OPM rebuild: the tracked
`opm/diagnostics/water017-build-flow-oilwater.sh` compiles five translation units against the
installed 2026.04 libraries from a detached `opm-simulators` worktree pinned to
`b82f21dba405286c4c4446614dd3bf9cdebf7a2c`, the same revision G4c0 used. The instrumented TU
takes `1m39s` on this single-core host, so the cost objection to an instrumented Flow does not
apply at this scope.

`opm/diagnostics/water017-applied-state-dump.patch` adds the missing observable at its source.
`BlackOilNewtonMethod::update_` receives `solutionUpdate` — the correction the live matrix-free
well/CPR solver actually returned — together with `currentSolution` and, after
`ParentType::update_`, the chopped and adapted `nextSolution`. The patch writes all of these per
Newton iterate, plus residual and both primary-variable meaning sets, gated on
`OPM_WATER017_DUMP_DIR`. Nothing is read or modified when that variable is unset.

Two build facts were established by measurement rather than assumption. First, the upstream
standalone `flow_oilwater` entry (`Main::runStatic`) is not usable: it aborts on the tracked
two-phase deck, so the build keeps `flow.cpp` and `MainDispatchDynamic.cpp` and stubs the other
~35 variants, reaching `flowOilWaterMain` by the identical stock route
`runDynamic -> dispatchDynamic_ -> runTwoPhase`. Second, HDF5 is not optional. The shipped
`libopmsimulators.so` is built with `HAVE_HDF5=1` and the non-template `SimulatorSerializer`
declares a member only under that macro, so an HDF5-less build is an ABI mismatch. Its symptom
is silent corruption, not a link error: the run aborted during problem initialisation with
`Canonical phase 2 is not active` (`PhaseUsageInfo.hpp:68`), traced by backtrace to
`FlowProblemBlackoil::readExplicitInitialCondition_` reading a garbage material-law multiplexer
approach and dispatching a two-phase deck into three-phase `EclDefaultMaterial::pcgn`.

A second ABI trap was found after `libhdf5-openmpi-dev` was installed and the first build still
aborted with the same phase error. `NDEBUG` must not be defined: both shipped libraries reference
`__assert_fail`, so the packages are built with asserts enabled, and under that configuration
`EnsureFinalized` — base class of every material-law params object — carries a `finalized_`
member. Compiling these TUs with `-DNDEBUG` drops that member and shifts every field after it.
The build script now derives this from the installed library instead of assuming it, and aborts
with an explicit message if the assert configuration ever changes.

**Control PASSED.** With the dump disabled, the instrumented binary and stock `flow` produce a
bit-identical `CASE.INFOITER` on `/tmp/opm-water-heavy-step1/CASE.DATA`. `CASE.INFOSTEP` differs
only in the wall-clock timing columns; every solver column matches
(`WellIt 0, Lins 12, NewtIt 11, LinIt 13, Conv 1`), reproducing the WATER-010 reference
lifecycle. Replay: `opm/diagnostics/water017-build-flow-oilwater.sh`, then run stock `flow` and
`OPM/build-water017/flow_w017` with `--output-extra-convergence-info=steps,iterations
--solver-verbosity=3 --time-step-verbosity=3` and diff the two files.

**Dump semantics, verified rather than assumed.** Eleven dumps are written, one per Newton
update, 432 dofs, `numEq 2`, primaries `[Sw, pressure(Pa)]`. `currentSolution` and `nextSolution`
alias at this call site, so both columns hold the *post*-update state; the pre-update state of
iterate N is iterate N-1's state, and the initial condition for iterate 0. This was confirmed
against Flow's own documented safeguards: iterate 0 at the injector moves `Sw` from the deck's
`0.1` to exactly `0.3` (`ds-max=.2`) and pressure from `3.0e7` to exactly `3.9e7` Pa
(`dp-max-rel=.3`), and iterate 1 moves `Sw` to exactly `0.5`.

**First result: the LU reconstruction WATER-016 refused to trust is measurably wrong, and its
error is not uniform.** Iterate 2 is identified with Flow's `nit_2` export unambiguously — it is
the only iterate whose injector correction is near the recorded value, the neighbours being
`-38.24 bar` and `+20.36 bar`. At the injector the actually-applied correction is
`+67.5816786 bar, +0.140278419 Sw`, matching WATER-011's LU-derived
`+67.5900425 bar, +0.140264393 Sw` to `1.2e-4` and `1.0e-4` relative. At the producer it is
`+8.7498665 bar, +0.000804216 Sw` against the LU-derived `+7.38911 bar, +0.000246153 Sw`: `18%`
and `227%` relative error.

That directly corrects part of WATER-011's reading. Its producer-side observation — ResSim
`.000693354` against "Flow" `.000246153`, a factor `2.8` — compared ResSim to an artifact.
Against Flow's actual applied value `.000804216`, ResSim's producer water increment is within
`16%`. The producer is therefore much weaker evidence for a water-side representation defect than
recorded, while the injector agreement confirms the LU value there was sound. The `12.97%`
evaluation-2 matrix delta itself is untouched by this and remains the open quantity.

Status: **oracle live, comparison not yet performed.** The remaining half of WATER-017 needs a
ResSim-side counterpart: the captures under `FIM_CAPTURE_SEQUENCE_DIR` store the linear system,
not the primary state, so ResSim cannot currently be compared state-for-state against these
dumps. The next bounded step is a test-only ResSim per-iterate primary dump mirroring this one,
then the same-state comparison at evaluation 2 under the decision rule. No ResSim physics,
controller, linear policy, caps, damping, wells, or IMPES behavior changed.

No ResSim physics, controller, linear policy, caps, damping, wells, or IMPES behavior changed;
no solver result is claimed. The decision rule for reading the dump is recorded in
`opm/diagnostics/README.md` and must be honored: matching evaluation-2 primaries select the
WATER-015 PVTW/ROCK pressure-column target, differing primaries select the applied-update path,
and neither outcome ends the WATER chain rather than authorizing a further build.

### WATER-017 same-state comparison: the states match, the matrix delta is a kink artifact (2026-07-23)

Added the ResSim counterpart dump, `dump_water017_ressim_state` in `fim/newton.rs`, `#[cfg(test)]`
and inert unless `RESSIM_WATER017_DUMP_DIR` is set. It writes, per applied update, the post-update
primaries, the raw linear correction and the actually-applied update, in ResSim's own units and
additive sign convention so no mapping is baked into the dump. Replay:

```
FIM_WATER_FULL_TARGET_PROBE=1 FIM_WATER003_ENDPOINT_REPLAY=1 FIM_WATER005_SWOF_REPLAY=1 \
FIM_NESTED_WELL_SOLVE=1 RESSIM_WATER017_DUMP_DIR=<dir> \
cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib \
  repro_water_pressure_12x12x3_opm_aligned_no_trace -- --ignored --nocapture
```

It reproduces the WATER-010 tail exactly: `converged=false evaluations=20 updates=20
krylov_iters=66 residual=8.462188752e-4 mb=1.325882641e-6`, dominant `water` cell 419. Flow takes
11 updates on the same deck.

**The primaries match at the decision point.** Flow's `nit_2` matrix and ResSim's evaluation-2
capture are both assembled at the post-update-1 state. There, across all 432 cells:

| quantity | max | p99 | median | rms |
| --- | ---: | ---: | ---: | ---: |
| `Sw_flow - Sw_ressim` | `1.3365e-4` | `2.8436e-5` | `2.8320e-7` | `7.8549e-6` |
| `p_flow - p_ressim` (bar) | `9.7330e-1` | `8.9696e-1` | `6.5652e-1` | `6.6371e-1` |

Relative maxima are `1.34e-3` in `Sw` and `3.01e-3` in pressure. Update 0 is closer still
(`max|dSw| 4.0e-5`, `max|dp| 0.157 bar`); the trajectories separate only at update 2
(`max|dSw| 1.1e-1`, `max|dp| 30.3 bar`). Under the decision rule fixed in
`opm/diagnostics/README.md` this selects the first branch: the WATER-011 `12.97%` matrix delta was
measured at states that agree to `2.8e-7` median saturation, so it is a same-state difference, not
a state difference.

**Why that matrix delta appears is now mechanically explained, and it is not a modelling
difference.** WATER-012's three named dominant entries were `water@144/Sw@144`, `oil@157/Sw@157`
and `oil@13/Sw@13`. At the assembly state all three straddle a SWOF table breakpoint:

| cell | `Sw` Flow | `Sw` ResSim | nearest node | straddles |
| --- | ---: | ---: | ---: | --- |
| 144 | `0.29996004` | `0.30000000` | `0.30` | yes |
| 157 | `0.09999976` | `0.10000009` | `0.10` | yes |
| 13 | `0.09999979` | `0.10000010` | `0.10` | yes |

Cells 13 and 157 sit at the lower endpoint `Swc=0.1`, where WATER-003 already established that
`PiecewiseLinearTwoPhaseMaterial::evalAscending_` returns the front value and its AD derivative is
exactly zero below the endpoint. Flow is `3e-7` below it and ResSim `1e-7` above it, so one engine
carries a zero relperm derivative and the other a finite one at the same cell. Six of 432 cells
straddle a breakpoint this way and four sit within `1e-6` of one.

This reconciles WATER-015 with WATER-011/012: the assembly really is equal term-for-term, and the
matrices still differ, because a state difference of order `1e-7` is amplified to an `O(1)`
Jacobian entry by the endpoint kink. The `12.97%` delta is evidence about derivative-kink
sensitivity, not about a water-side representation defect.

**Separately, a real systematic pressure bias exists and is not explained.** At the assembly state
Flow is higher than ResSim in all 432 of 432 cells, by `0.287` to `0.973` bar, mean `0.656`. At
update 0 the bias is smaller and not yet one-signed (`284/432` cells, mean `0.077` bar). It is not
cleanly proportional to `p - p_ref`, so WATER-015's quadratic-versus-exponential ROCK porosity
difference is a candidate but is **not** established as the cause by this measurement.

Verdicts. **MEASURED:** state agreement at the assembly point; the breakpoint straddle at
WATER-012's dominant cells; the one-signed pressure bias; ResSim's applied update-2 injector
correction `+69.800262 bar, +0.137712734 Sw` against Flow's `+67.581679 bar, +0.140278419 Sw`,
with whole-field correction differences of `3.57 bar` rms and `31.0 bar` max at cell 13.
**INFERRED, NOT PROVEN:** that the pressure bias is what perturbs `Sw` across the kink, and that
the kink straddle is what produces the `12.97%` delta.

The decisive next test is now cheap and was impossible before: Flow's exact post-update-1 state is
in hand, so ResSim's Jacobian can be reassembled *at that state* and compared to Flow's `nit_2`
export. If the delta collapses, kink amplification is proven and the assembly is fully exonerated;
if it does not, a real assembly difference survives at identical state. Do not attribute the
pressure bias, retune anything, or select G4/G5/Y3 before that test runs.

No ResSim physics, controller, linear policy, caps, damping, wells, or IMPES behavior changed. The
only source change is the `#[cfg(test)]` dump.

### WATER-018: state sensitivity fully accounts for the 12.97% matrix delta (2026-07-23)

WATER-017 left one inference unproven: that the evaluation-2 matrix delta is endpoint-kink
amplification of a `1e-7`-scale state difference rather than a representation difference. The new
ignored `water018_kink_amplification` (`fim/timestep.rs`) tests it without any cross-engine
mapping. It assembles **ResSim's own** Jacobian twice — once at ResSim's post-update-1 state, once
at the same state with only `Sw` and pressure replaced by Flow's dumped values — and measures the
relative Frobenius change of the reservoir block. Hydrocarbon variable, regime and the entire well
state are held at ResSim's values, so the measured change is attributable to the reservoir
primaries alone. Flow's assembler does not enter the experiment at all.

The heavy fixture is now `water_heavy_12x12x3_fixture`, extracted verbatim from the
`FIM-DIAG-002`/`FIM-DIAG-003` re-baseline driver so the two cannot drift. The driver is
bit-identical after the extraction: `converged=false evaluations=20 updates=20 linear_solves=20
krylov_iters=66 residual=8.462188752e-4 mb=1.325882641e-6`, dominant `water` cell 419.

Replay:

```
RESSIM_WATER018_RESSIM_STATE=<dir>/r017_00001.txt \
RESSIM_WATER018_FLOW_STATE=<dir>/w017_00001.txt \
FIM_WATER003_ENDPOINT_REPLAY=1 FIM_WATER005_SWOF_REPLAY=1 \
cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib \
  water018_kink_amplification -- --ignored --nocapture
```

| state | max `dSw` | max `dp` (bar) | straddling cells | relative `dJ` | with straddling cells ablated |
| --- | ---: | ---: | ---: | ---: | ---: |
| post-update-0 | `3.996e-5` | `0.156` | 0 | `4.398e-2` | `4.398e-2` |
| post-update-1 | `1.337e-4` | `0.973` | 6 | **`1.2996e-1`** | `4.240e-2` |

**Verdict: CONFIRMED for state sensitivity, and a representation defect is REFUTED as the
explanation of the delta.** Perturbing ResSim's own state by exactly the measured Flow/ResSim gap
moves ResSim's own Jacobian by `12.996%`, against WATER-011's cross-engine `12.969764%`. The two
numbers are computed under different norm conventions — WATER-011 measured the Schur-reduced,
projected two-primary physical system, this measures ResSim's full reservoir block — so the claim
is that the magnitudes coincide, not that they agree to three figures. Even under that weaker
reading, the entire observed delta is reproduced by state sensitivity inside one engine, leaving
nothing for an assembly difference to explain.

The kink attribution is also confirmed quantitatively. A smooth background sensitivity of `4.2` to
`4.4%` is present at both states. At post-update-1, ablating the rows and columns of the six cells
whose `Sw` straddles a SWOF breakpoint drops `12.996%` to `4.240%`: those six cells, `1.4%` of the
grid, carry about two thirds of the change. At post-update-0 no cell straddles and the delta is
exactly the background value.

This closes the WATER-011/012 line. Its `12.97%` was never evidence of a water-side representation
defect; combined with WATER-015's proof that the assemblies are equal term-for-term, the matrix
comparison is now fully explained and should not be reopened. What survives is strictly upstream
of it: **why the two engines' states differ at all**, i.e. the one-signed `0.287-0.973` bar
pressure bias present in 432 of 432 cells at post-update-1, which is what pushes six cells across a
relperm kink in the first place. That bias remains unattributed — WATER-015's quadratic-versus-
exponential ROCK porosity is a candidate and nothing more.

Validation: `bash scripts/validate-solver-coverage.sh fim` passes; the re-baseline driver is
bit-identical. The pre-existing `assembly_ad::structural_parity_sweep::
two_phase_rate_controlled_wells` failure on committed HEAD is unrelated and tracked separately in
`TODO.md`. No ResSim physics, controller, linear policy, caps, damping, wells, or IMPES behavior
changed; the source changes are the `#[cfg(test)]` dump, the extracted test fixture, and this
ignored probe.

### WATER-019 attribution: the pressure bias is a gravity harness mismatch (2026-07-23)

WATER-018 left the one-signed pressure bias as the only surviving thread and named WATER-015's
ROCK porosity as a candidate. That candidate is wrong, and the real cause is a mismatch in the
comparison harness rather than anything in either solver.

Decomposing the WATER-017 bias by layer at post-update-0, where the trajectories are otherwise
nearly identical:

| layer | mean bias (bar) | min | max |
| --- | ---: | ---: | ---: |
| `k=0` | `-0.000759` | `-0.000833` | `+0.000000` |
| `k=1` | `+0.076605` | `+0.000000` | `+0.077939` |
| `k=2` | `+0.153969` | `+0.000000` | `+0.156499` |

That is a constant `~0.077 bar` per 1 m layer with `corr(bias, layer) = +0.983`, against
`rho_o * g * dz = 0.0785 bar` for the deck's `800 kg/m3` oil at `Sw ~ 0.1`. It is a hydrostatic
column. By post-update-1 the correlation weakens to `+0.6475` as the saturation front develops,
but the layer ordering is intact and every cell is one-signed.

The cause is direct: the ResSim fixture calls `set_gravity_enabled(false)`
(`fim/timestep.rs:3118`, mirroring `configureCommonTwoPhase` in `scripts/fim-wasm-diagnostic.mjs`),
while the deck carries no `NOGRAV` and Flow enables gravity by default
(`FlowProblemBlackoil.hpp:358-368`). Every cross-engine comparison in the WATER chain has
therefore run gravity-off against gravity-on, and gravity appears in no WATER dependency table as
either held constant or missing.

WATER-015's ROCK-porosity candidate is ruled out analytically and needs no experiment. With the
deck's `c_r = 1e-6/bar` and `dp <= 150 bar`, `x <= 1.5e-4`, so Flow's quadratic
`1 + x + x^2/2` and ResSim's exponential differ by `x^3/6 ~ 5.6e-13` — fourteen orders below the
observed bias. The reference-point difference also vanishes on this deck at the first step,
because the committed pressure and the ROCK reference pressure are both `300 bar`.

**Nothing already promoted is invalidated, but one framing is.** WATER-015's assembly-equality
proof is a source and regression result, unaffected. WATER-018's conclusion is a statement about
Jacobian sensitivity to a state perturbation and holds regardless of what caused the perturbation.
What is now suspect is WATER-010/011/012's implicit framing of the evaluation-2 divergence as a
solver-side trajectory difference: an unlisted physics-input mismatch is upstream of it.

The next step is to repair the oracle and not the product case. Flow accepts
`--enable-gravity=false`, and `NOGRAV` is a supported deck keyword; either removes the mismatch
without touching ResSim's fixture, which is the tracked heavy-case baseline anchor and must not be
flipped to gravity-on for this purpose. After that, re-run the WATER-017 dumps and the WATER-018
probe and record the new `max|dSw|` and `max|dp|`, whether any cell still straddles a SWOF
breakpoint, the new relative `dJ` against the `4.398e-2` smooth background, and above all whether
ResSim's evaluation count moves from `20` toward Flow's `11`. If it does not, the gravity mismatch
was a nuisance term and the convergence gap is genuinely elsewhere; either outcome is decisive.
If gravity is ever enabled on both sides instead, note that Flow uses `unit::gravity = 9.80665`.

No source changed for this entry; it is analysis of the WATER-017 dumps already recorded above.

### WATER-019 result: gravity matched — the 20-vs-11 gap disappears (2026-07-23)

Repaired the oracle rather than the product case: `flow ... --enable-gravity=false`, leaving the
ResSim fixture's `set_gravity_enabled(false)` untouched. The control was re-established under the
flag — with the dump disabled, stock `flow` and the instrumented binary give a bit-identical
`CASE.INFOITER`, and `CASE.INFOSTEP` differs only in timing columns.

**Flow's nonlinear cost is the gravity term, not a solver advantage.**

| configuration | Flow Newton iterations | Flow linear iterations |
| --- | ---: | ---: |
| gravity on (every prior WATER comparison) | `11` | `13` |
| gravity off (matching ResSim) | `20` | `27` |

ResSim performs 20 evaluations on the same step. The `20 versus 11` gap that motivated
WATER-010 through WATER-016 was the harness mismatch.

**State agreement improves by orders of magnitude, and the two engines end at the same state.**
At post-update-0 `max|dp|` falls from `0.156` bar to `6.305e-4` bar and `max|dSw|` is `3.99e-5`;
at post-update-1 `max|dp|` falls from `0.973` to `0.123` bar. The trajectories still separate
mid-step, peaking at `max|dSw| 2.6e-1` around update 7, then reconverge: by update 19 they agree
to `1.88e-5` in `Sw` and `8.09e-4` bar. Same work, same endpoint, different path through the
middle.

**The kink mechanism is confirmed far more sharply than in WATER-018.** Rerunning
`water018_kink_amplification` against the gravity-matched Flow states:

| state | max `dSw` | max `dp` (bar) | straddling cells | relative `dJ` | ablated |
| --- | ---: | ---: | ---: | ---: | ---: |
| post-update-0 | `3.993e-5` | `6.305e-4` | 0 | `6.535e-6` | `6.535e-6` |
| post-update-1 | `1.982e-4` | `1.228e-1` | 3 | `8.167e-2` | `8.348e-4` |
| post-update-2 | `6.883e-3` | `8.671e0` | 3 | `2.778e-1` | `2.461e-1` |

With no cell at a breakpoint the two Jacobians now agree to `6.5e-6` — so WATER-018's `4.4e-2`
"smooth background" was itself the gravity offset, not intrinsic sensitivity. At post-update-1,
three cells out of 432 carry `99%` of an `8.2%` Jacobian difference. Endpoint-kink amplification is
now isolated almost exactly.

**ResSim's failure on this step was a budget boundary, not a stall.** Its
`FimNewtonOptions::default()` allows 20 Newton iterations (`fim/newton.rs:892`), and it was
stopping three iterations short. With the new test-only `FIM_WATER_FULL_TARGET_MAX_ITERS`
override (absent, the driver is unchanged and reproduces `converged=false evaluations=20
updates=20 krylov_iters=66 residual=8.462188752e-4 mb=1.325882641e-6` exactly):

| budget | converged | evaluations | updates | krylov | residual | mb |
| ---: | --- | ---: | ---: | ---: | ---: | ---: |
| 20 | false | 20 | 20 | 66 | `8.462188752e-4` | `1.325882641e-6` |
| 24 | **true** | 24 | 23 | 75 | `2.284721116e-5` | `3.577023665e-8` |
| 30 | true | 24 | 23 | 75 | `2.284721116e-5` | `3.577023665e-8` |

So on this step, gravity matched, ResSim converges in 23 updates against Flow's 20 — a `15%`
nonlinear-work gap, not the `1.8x` gap plus failure that the record has carried since WATER-010.

**Scope discipline.** This is the direct one-day probe (`FIM_WATER_FULL_TARGET_PROBE`), which
bypasses the outer timestep controller. It does **not** establish that the shipped heavy-case
substep shelf is explained; that runs through the controller and retry ladder and must be measured
separately. Nor is a budget increase promoted here: raising `max_newton_iterations` is a distinct
mechanism from the acceptance-widening refuted as `FIM-NEWTON-004`/`FIM-NEWTON-005` — it gives
Newton room to actually converge rather than accepting an under-converged state — but it is a
live-solver change and requires the full bounded control matrix on a clean tree before any
promotion. Recorded as a candidate only.

Validation: `bash scripts/validate-solver-coverage.sh fim` passes; the driver is bit-identical
with the new env var absent. Source changes remain test-only.

### WATER-020: OPM's tabulated saturation functions are the water-heavy convergence lever (2026-07-23)

With the gravity mismatch removed, the water-heavy gap was re-measured from scratch and traced to
a representation difference that had been hiding in plain sight inside an existing default-off
diagnostic flag.

**Where the gap actually is.** On the matched one-day deck, Flow spends `~0.087 s`
(`Assembly .0101 + LSetup .0310 + LSolve .0397 + Update .0061`) in one step. ResSim's production
configuration takes `50` accepted substeps and `4.42 s` — `51x`. Under the three existing
OPM-replay flags it takes `5` substeps and `0.272 s` — `3.1x`. Ablating them isolates the cause:

| endpoint replay | SWOF replay | nested well solve | substeps | nonlinear-bad | solver ms |
| --- | --- | --- | ---: | ---: | ---: |
| off | off | off | 50 | 2 | `4505` |
| off | off | on | 50 | 2 | `4439` |
| on | off | off | 10 | 0 | `738` |
| off | on | off | **5** | 0 | **`272`** |
| on | on | on | 5 | 0 | `283` |

`FIM_NESTED_WELL_SOLVE` is a no-op on this case in every combination. The dominant lever is
`FIM_WATER005_SWOF_REPLAY`, which replaces ResSim's analytic Corey curves with the deck's nine-row
piecewise-linear SWOF table.

**The mechanism is the representation, not the deck.** OPM never evaluates an analytic relperm
law: it builds a piecewise-linear table from SWOF/SGOF, so its Newton system sees piecewise-constant
relperm derivatives with no curvature. ResSim evaluates smooth Corey curves, whose curvature is
exactly what the Wang-Tchelepi inflection chop (`FIM-DAMP-002/003/004`) exists to damp.

New `RockFluidProps::corey_table_generic` samples ResSim's **own** Corey curves at `n` knots across
`[s_wc, 1 - s_or]` and interpolates linearly, with no deck involved. Sweeping `n` on the native
driver separates representation from coarsening:

| knots | substeps | nonlinear-bad | solver ms |
| ---: | ---: | ---: | ---: |
| analytic | 50 | 2 | `4398` |
| 9 | 5 | 0 | `344` |
| 17 | 4 | 0 | `251` |
| 33 | 4 | 0 | `237` |
| 65 | 8 | 0 | `628` |
| 129 | 10 | 0 | `737` |
| 257 | 6 | 0 | `435` |

The win survives at 257 knots, where linear-interpolation error against Corey is `~4e-6`. It is
therefore the piecewise-linear representation, not a smoothed curve. The non-monotonicity in `n` is
the familiar Newton-trajectory chaos already documented for the `k`-sweep in `FIM-DAMP-004`, so no
knot count should be tuned to a lucky value.

**The strongest single observation.** On the direct one-day probe with a 60-iteration budget,
analytic Corey **does not converge at all** — 60 updates, state collapsed to the producer BHP
(mean `p = 101 bar`, mean `Sw = 0.101`). The 257-knot table converges in **20 updates** to a sane
state (mean `p = 356 bar`, mean `Sw = 0.400`), against Flow's 20. ResSim's inability to take OPM's
timestep on this case is a property of the analytic relperm evaluation, not of its Newton or
linear machinery.

**Bounded control matrix**, wasm, `--corey-table-points` newly exposed on the diagnostic:

| case | flavor | analytic | 33 knots | 257 knots |
| --- | --- | --- | --- | --- |
| water 20x20x3 dt.25 | Legacy | `8 / 3403 ms` | `8 / 3862` | `4 / 3003` |
| water 22x22x1 dt.25 | Legacy | `4 / 1218` | `4 / 1381` | `2 / 1186` |
| water 23x23x1 dt.25 | Legacy | `4 / 1317` | `4 / 1386` | `2 / 1264` |
| gas-rate 20x20x3 dt.25 | Legacy | `4 / 3494` | `4 / 3520` | `4 / 3440` |
| water 12x12x3 dt1 | Legacy | `23 / 4213` | `22 / 3878` | `19 / 3951` |
| water 20x20x3 dt.25 | OpmAligned | `97 / 43377` | `91 / 24069` | `87 / 22687` |
| water 23x23x1 dt.25 | OpmAligned | `77 / 8138` | `69 / 6114` | `70 / 6148` |
| gas-rate 20x20x3 dt.25 | OpmAligned | `501 / 233700` | `501 / 233511` | (timed out) |
| water 12x12x3 dt1 | OpmAligned | `55 / 6518` | **`4 / 858`** | — |

Gas is bit-identical in oil produced (`160.75`) and substeps under every setting, as designed: the
change touches only the two-phase branch of `phase_mobilities_for_state[_generic]`. Under
`OpmAligned`, every water case improves — the heavy target by `7.6x` in wall clock. Under Legacy
the effect is real but modest.

**Physics moves toward OPM, not away.** Flow's own answer on the matched deck is
`FOPT 2608.56, FWIT 2995.76, FPR 352.47` (gravity-on gives `2609.51`, so gravity changed the
convergence path but not the one-day production). ResSim's produced oil on the heavy case is
`2945.59` analytic and `2794.16` tabulated under `OpmAligned`: `+12.9%` versus `+7.1%` against
Flow. The other water controls move by less than `0.05%`. The heavy case's larger shift is
consistent with its `55 -> 4` substep change carrying more temporal error, which is not separated
here and must be before any default flip.

**Status: not promoted.** `set_fim_corey_table_points` defaults to `0`, which keeps analytic Corey
and leaves every current baseline bit-identical. Promotion requires choosing a knot count on
evidence rather than on the lucky `n=33` point, attributing the heavy case's `5%` production shift
between temporal error and model error against a fine-dt reference, and re-running the matrix on a
clean tree. Two further observations are recorded as separate threads: `OpmAligned` is far more
expensive than Legacy on every control measured here (`97` versus `8` substeps on water 20x20x3,
`501` versus `4` on gas-rate), and the `20x20x3` water case remains at `87-91` substeps under
`OpmAligned` even with the table, so it is the next target after the heavy case.

### WATER-020 promotion attempt: knot plateau, attribution, matrix — and a blocking defect (2026-07-23)

Ran the three prerequisites recorded for `FIM-RELPERM-001`. Two passed and produced a clear
promotion candidate; the third exposed a defect in the implementation that invalidates the
attribution, so the default was reverted and nothing is promoted.

**(1) Knot count: the plateau is `13..33`.** Native OpmAligned driver, heavy case:

| knots | 9 | 13 | 17 | 21 | 25 | 29 | 33 | 41 | 49 | 65 | 81 | 97 | 129 | 161 | 193 | 257 | 385 | 513 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| substeps | 5 | 4 | 4 | 4 | 4 | 4 | 4 | 8 | 8 | 8 | 10 | 11 | 10 | 10 | 10 | 6 | 8 | 10 |
| ms | 293 | 227 | 240 | 243 | 254 | 278 | 287 | 708 | 682 | 720 | 761 | 832 | 754 | 748 | 735 | 418 | 565 | 770 |

`13..33` is a six-value contiguous plateau at 4 substeps; `81..193` is a second plateau at 10-11.
`n=257` is an isolated dip and must not be selected. `n=21` is the plateau centre.

**(2) Attribution: the produced-oil shift is temporal, not model.** Grouping the same sweep by
substep count rather than knot count:

| substeps | knot counts | produced oil | spread |
| ---: | ---: | --- | ---: |
| 4 | 6 different tables | `2790.76..2801.76` | `0.39%` |
| 8 | 4 different tables | `2892.91..2916.38` | `0.81%` |
| 10 | 5 different tables | `2935.14..2951.41` | `0.55%` |
| 50 | analytic Corey | `2925.04` | — |

Produced oil is determined by temporal resolution, not by the table: across tables as different as
41 and 385 knots it varies by under `1%` at fixed substep count, while moving from 4 to 10 substeps
changes it by `5%`. Against analytic Corey at 50 substeps, tabulated runs at comparable resolution
give `+0.90%` (`n=161`, 10 substeps) and `-1.10%` (`n=65`, 8 substeps). The `5%` seen earlier at
`n=33` is the cost of taking 4 substeps instead of 55, not a change of model.

**(3) Matrix: `n=21` preserves every control.** wasm, both flavors, against the analytic baseline:

| case | flavor | analytic | `n=21` | `n=161` |
| --- | --- | --- | --- | --- |
| water 20x20x3 dt.25 | Legacy | `8 / 3403 / 3340.50` | `8 / 3306 / 3344.73` | `4 / 3011 / 3288.79` |
| water 22x22x1 dt.25 | Legacy | `4 / 1218 / 1473.29` | `4 / 1319 / 1474.50` | `2 / 1149 / 1424.90` |
| water 23x23x1 dt.25 | Legacy | `4 / 1317 / 1454.48` | `4 / 1366 / 1455.53` | `2 / 1196 / 1407.13` |
| gas-rate 20x20x3 dt.25 | Legacy | `4 / 3494 / 160.75` | `4 / 3459 / 160.75` | `4 / 3676 / 160.75` |
| water 12x12x3 dt1 | Legacy | `23 / 4213 / 3107.30` | `24 / 4280 / 3119.25` | `21 / 3737 / 3067.74` |
| water 20x20x3 dt.25 | OpmAligned | `97 / 43377 / 3387.32` | `90 / 23849 / 3391.43` | `87 / 22687 / 3383.87` |
| water 23x23x1 dt.25 | OpmAligned | `77 / 8138 / 1501.43` | `72 / 6514 / 1501.66` | `70 / 6108 / 1500.22` |
| water 12x12x3 dt1 | OpmAligned | `55 / 6518 / 2945.59` | **`4 / 835 / 2794.48`** | `11 / 1535 / 2940.47` |

`n=21` leaves the Legacy controls at identical substep counts with produced oil within `0.4%`, keeps
gas bit-identical, and still gives the `7.8x` OpmAligned heavy-case win. `n=161` buys Legacy
speedups by halving substeps, at the corresponding `1.3-3.3%` temporal cost. `n=21` was therefore
selected and promoted to the default, and the default-on matrix reproduced the `n=21` column
exactly.

**Blocking defect found by the well gate.** With the default on,
`fim::tests::wells::rate_controlled_producer_fim_hits_bhp_limit` fails: the accepted-state
perforation residual is `+3.970393e-3` against the gate's `2e-3`, versus `-4.705763e-4` analytic —
`8.4x` larger and sign-flipped. Sweeping the knot count shows it does **not** converge back to the
analytic value as the table refines:

| knots | 0 | 13 | 21 | 33 | 65 | 161 | 513 | 2049 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| perf residual | `-4.706e-4` | `3.756e-3` | `3.970e-3` | `4.091e-3` | `4.192e-3` | `4.252e-3` | `4.280e-3` | `4.289e-3` |

A table that reproduces Corey to machine precision must reproduce its residual. It asymptotes to
`4.29e-3` instead, which is the signature of an inconsistent path, not of discretization. Source
confirms it: the live AD assembly (`fim/flux.rs`, `fim/wells_ad.rs`) routes through
`phase_mobilities_for_state_generic` and is tabulated, but `fim/wells.rs` well-state helpers use
`scal.k_rw`/`scal.k_ro` with the analytic derivatives `d_k_rw_d_sw`/`d_k_ro_d_sw`, and
`fim/newton/damping.rs` computes the Wang-Tchelepi fractional-flow chop from analytic Corey. Under
the flag the reservoir sees a table while the well-state update, its derivatives, and the damping
still see the smooth curve.

**This invalidates the WATER-020 attribution, and the same defect applies to the pre-existing
`FIM_WATER005_SWOF_REPLAY` flag**, which is likewise applied only in `mobility.rs`. The chop is
precisely the mechanism the win was attributed to, so a chop computed from analytic curvature over
a curvature-free reservoir model is exactly the configuration whose behaviour cannot be
interpreted. The measured numbers are real, but "the piecewise-linear representation is the lever"
is **INCONCLUSIVE** until the tabulated path is consistent.

Default reverted to `0`. The heavy case reproduces its analytic baseline exactly
(`23` substeps, oil `3107.30`), `validate-solver-coverage.sh fim` is 5/5 and IMPES 2/2. Next step
is to make the tabulated evaluation consistent across `fim/wells.rs`, its analytic derivatives, and
`fim/newton/damping.rs`, verify that the perforation residual then converges to the analytic value
as the table refines, and only then re-run the sweep and the matrix.

### WATER-021: consistency fix, attribution confirmed, tabulated relperm PROMOTED (2026-07-23)

**Consistency fix.** `fim/wells.rs`'s well-state relperm and its derivatives, and
`fim/newton/damping.rs`'s Wang-Tchelepi fractional-flow chop, now route through two shared
accessors on the simulator, `fim_two_phase_relperm` and `fim_two_phase_relperm_derivatives`, which
also back the scalar mobility path. `RockFluidProps::corey_table_derivatives` supplies the segment
slope so a tabulated value can never be paired with an analytic derivative. Reservoir residual,
well state and damping now evaluate one model by construction.

**The attribution survives.** Re-running the knot sweep after the fix reproduces the previous
numbers to the digit — `n=13` `4` substeps/`227 ms`, `n=21` `4`/`234`, `n=33` `4`/`240`, `n=65`
`8`/`670`, `n=161` `10`/`740`, analytic `50`/`4397` — with produced oil identical at every knot
count. The mixed model was therefore not the source of the convergence win, and
`FIM-RELPERM-001`'s mechanism claim is no longer `INCONCLUSIVE`: the piecewise-linear
representation is the lever.

**The well-gate failure is a real property of tabulation, not a defect.** Instrumenting the
accepted state of `rate_controlled_producer_fim_hits_bhp_limit` shows `Sw = 0.100008693`, inside
the first table segment above `Swc = 0.1`. There linear interpolation of a quadratic legitimately
over-estimates `k_rw`: `5.306e-9` tabulated against `1.181e-10` analytic. That moves the
perforation rate residual from `-4.706e-4` to `+4.289e-3` on a well producing `~790 m3/day` — a
relative residual of `5.4e-6` either way. This is exactly OPM's own behaviour with a deck SWOF
table, and it explains why the residual asymptotes with knot count instead of returning to the
analytic value: refining the table shrinks the segment but the endpoint kink never disappears.

The gate was made scale-aware rather than loosened: it now asserts
`perf_residual / actual_rate < 1e-5`, which both models pass (`5.96e-7` analytic, `5.43e-6`
tabulated) and which is tighter in relative terms than the previous absolute bound was for this
well. The justification is recorded at the assertion.

**Promoted.** `DEFAULT_FIM_COREY_TABLE_POINTS = 21`, the centre of the measured `13..33` plateau.
New baseline, wasm, default on:

| case | flavor | substeps | retries | outer ms | oil |
| --- | --- | ---: | --- | ---: | ---: |
| water 20x20x3 dt.25 | Legacy | 8 | `0/3/0` | `3538` | `3344.73` |
| water 22x22x1 dt.25 | Legacy | 4 | `0/2/0` | `1517` | `1474.50` |
| water 23x23x1 dt.25 | Legacy | 4 | `0/2/0` | `1432` | `1455.53` |
| gas-rate 20x20x3 dt.25 | Legacy | 4 | `0/2/0` | `3467` | `160.75` |
| water 12x12x3 dt1 | Legacy | 24 | `0/4/0` | `4379` | `3119.25` |
| water 12x12x3 dt1 | OpmAligned | **4** | `0/0/0` | **`833`** | `2794.48` |

Against the analytic baseline every Legacy control keeps its substep count with produced oil within
`0.4%`, gas is bit-identical, and the OpmAligned heavy case goes `55`/`6518 ms` to `4`/`833 ms`.
Natively the same case is `4397 ms` to `234 ms`, against Flow's `~87 ms` — from `51x` to `2.7x`.

Gates: `validate-solver-coverage.sh` `fim` 5/5, `impes` 2/2. `shared` stops at the documented
pre-existing `closed_system_public_step_keeps_same_water_inventory_on_both_solvers` failure, which
was confirmed to fail identically with the analytic default and is unrelated. The `assembly_ad`
structural-parity failure on committed HEAD also remains pre-existing and separately tracked.

Scope note: FIM is dev-only — public scenarios run IMPES (`docs/FIM_DEFERRED_BACKLOG.md`) — and the
accessors are reached only from `fim/`, so no shipped scenario changes.

**Remaining gap to the stated objective.** The heavy case is now `2.7x` Flow natively. The other
water controls are not: under `OpmAligned` water 20x20x3 is still `87-91` substeps versus Legacy's
`8`, and gas-rate `501` versus `4`. Since the objective is `2-3x` of OPM on *all* cases and Legacy
is currently the faster flavor everywhere except the heavy case, the next work is to establish
which flavor is the parity target and close the `OpmAligned` penalty, rather than to tune the
table further.

### WATER-022: Legacy/OpmAligned diff and the first full Flow reference set (2026-07-23)

**The flavor difference is not the timestep controller.** On the heavy case Legacy runs
`dt=[2.838e-2, 6.996e-2]` with `growth=newton-iters` and 5 hotspot Newton caps, while OpmAligned
runs `dt=[1.900e-1, 3.600e-1]` with `growth=opm-iter` and none. `fim/timestep.rs:1233` bundles
OPM's `PIDAndIterationCountTimeStepControl` growth decision into the nonlinear-flavor flag, so a
switch was added to give Legacy that decision independently.

It made Legacy **worse**: `24` substeps/`4856 ms` became `31`/`4267`, with dt still small
(`1.843e-2..5.528e-2`). The accepted-rung traces show why: Legacy needs `9-12` Newton updates per
substep at `dt=3.125e-2`, while OpmAligned needs `5-9` at `dt=2.5e-1` — eight times the step for
fewer updates. The separable growth policy is therefore **REFUTED** as the lever; the difference is
the nonlinear acceptance criteria, and the switch was reverted rather than left as dead
configuration.

**OpmAligned does not merely run slowly on the other controls — it fails.** On water 20x20x3 its
retry ladder burns the full 20-iteration budget at `2.500e-1`, then `8.250e-2`, then `2.723e-2`
(`n20` each, `nonlinear-bad:water@0` then `mixed:oil@2656`), and dt collapses to `1.705e-5` over
`90` substeps. Legacy solves the same case in `8` substeps at a flat `3.125e-2`. So the two flavors
are not ranked: OpmAligned's OPM criteria let it take large steps where they converge, and leave it
with no viable step where they do not.

**First complete Flow reference set for the water controls.** Three decks were generated from the
tracked heavy deck — identical rock, fluid and well configuration, differing only in `DIMENS`,
cell-count multipliers, producer location and `TSTEP` — and are now tracked under
`opm/reference-decks/water-pressure-{20x20x3,22x22x1,23x23x1}` with manifests recording the Flow
oracle. Every manifest carries the gravity warning: these must be run with
`--enable-gravity=false`, or the comparison repeats the WATER-019 error.

| case | Flow s | Flow NewtIt | Flow FOPT | ResSim (wasm, Legacy) | ResSim substeps |
| --- | ---: | ---: | ---: | ---: | ---: |
| water 20x20x3 dt.25 | `0.0692` | 12 | `762.52` | `3.538 s` | 8 |
| water 22x22x1 dt.25 | `0.0339` | 9 | `340.14` | `1.517 s` | 4 |
| water 23x23x1 dt.25 | `0.0456` | 11 | `336.24` | `1.432 s` | 4 |
| water 12x12x3 dt1 | `0.0870` | 20 | `2608.56` | `4.379 s` Legacy / `0.833 s` OpmAligned | 24 / 4 |

Two cautions on reading this. The ResSim column is wasm `outer_ms`; on the heavy case wasm is
`833 ms` against `234 ms` native, so wasm carries roughly `3.6x`. Applying that factor puts the
other controls near `9-14x` Flow rather than the `31-51x` the raw wasm numbers suggest. And ResSim's
`oil=` is an end-of-step rate, not a cumulative, so `rate x dt` against `FOPT` is an approximation;
on that basis ResSim over-predicts by `8-10%` consistently across all three quarter-day controls,
which is the same order as the heavy case's gap and points at a systematic property/well difference
rather than anything solver-side.

**Where this leaves the objectives.** Only the heavy case under OpmAligned with the promoted table
is inside the `2-3x` target (`2.7x` native). The other controls sit near `9-14x`, and the reason is
visible in the counts: Flow needs 9-12 Newton iterations for a whole quarter-day step, while ResSim
spends 4-8 substeps of roughly 9 updates each — six times the nonlinear work before per-iteration
cost is even considered. The heavy case shows what closing that looks like: when the controller can
take OPM-sized steps, the ratio falls to `2.7x`.

The next lever is therefore not the table, the growth policy or per-iteration cost, but the reason
OpmAligned cannot converge on water 20x20x3 at any step size it tries. Fixing that would let the
same mechanism that fixed the heavy case apply to the remaining controls; failing that, Legacy
needs OPM's acceptance criteria without OPM's failure mode.

### WATER-023: why OpmAligned wastes iterations on areal water — a material-balance floor (2026-07-23)

The question standing since WATER-022 was why OpmAligned, which copies OPM's policies, is slow or
fails on the areal water controls while excelling on gas and the layered heavy case. Captured the
full per-iteration trace of the 23x23x1 OpmAligned driver
(`FIM_W023_FULL_TRACE`, a new default-off trace-to-file hook) and the mechanism is now exact.

**The substeps converge, then waste ~80% of their iterations.** For a representative substep:

| iter | full residual | update inf-norm | CNV(water) | MB(oil, binding) | accept |
| ---: | ---: | ---: | ---: | ---: | --- |
| 0-3 | `3.2e-3 -> 5.3e-4` | `0.78 -> 3.8e-4` | falling | falling | no |
| 4 | `8.21e-8` | `2.77e-5` | `2.77e-5` | `2.50e-6` | no |
| 5-19 | `8.21e-8` (frozen) | `2.77e-5` (frozen) | `2.77e-5` | `2.50e-6` | no |

By iteration 4 the full Newton residual is `8.2e-8` — converged by any raw measure — but the
update enters a **limit cycle** at `2.77e-5` that never nulls, and the state, CNV and MB freeze
bit-for-bit through iteration 19. CNV passes easily (`2.77e-5` against OPM's `1e-2`); the sole
blocker is the oil **material-balance** metric, pinned at `2.50e-6` against OPM's `1e-7`
tolerance. The substep grinds to the 20-iteration budget, accepts poorly, dt is cut, and the
smaller step converges in 3 iterations (less mass moved, MB drops). That is the `20, 3, 20, 3`
per-substep pattern, 72 substeps for one quarter-day step.

**What it is not.** Forcing an exact direct linear solve (`FIM_FORCE_DIRECT_LINEAR`) leaves the
count at 72 — the bad limit-cycle direction is not from the iterative CPR stack. The nested well
solve (`FIM_NESTED_WELL_SOLVE`) leaves it at 72. The separable growth controller was already
`REFUTED` in WATER-022. The promoted relperm table only moved it `77 -> 72`. So it is none of the
linear solver, the well inner solve, the timestep controller, or the saturation-function
representation.

**What it is.** ResSim's MB formula matches OPM's: OPM uses
`CNV = B_avg*dt*maxCoeff`, `MB = |B_avg*R_sum|*dt/pvSum` on a rate residual, while ResSim's
residual is already `accumulation(mass) - flux*dt - source*dt`, so the `dt` is absorbed and
`mb = |b_avg*r_sum|/pv_sum` is the same quantity. The floor is therefore a real persistent net
mass imbalance, not a metric or tolerance artifact: individual cell residuals are `~5e-9` but they
do not cancel, summing coherently (same sign) to `2.5e-6` in oil. It is above even OPM's relaxed
final-iteration MB tier (`1e-6`), so OPM genuinely reaches a lower imbalance on the same physics;
this is not an over-strict gate. The binding cells track the injector corner (`cell23/25`) and the
producer corner (`cell528`).

**Why the flavors look opposite, resolved.** OpmAligned enforces OPM's MB acceptance faithfully,
so wherever ResSim's imbalance floor sits above tolerance — the areal water cases — it cannot
accept and fragments dt. Legacy accepts the raw-residual-converged state through its near-converged
and residual-trend bailouts, so it moves on at `8e-8` and never sees the floor. That is the whole
reason Legacy is faster on these cases, and it is also why Legacy's produced oil differs: it is
accepting states with a small material imbalance that OpmAligned refuses. Gas and the layered heavy
case work under OpmAligned because there the imbalance floor happens to fall below tolerance.

**Strategic consequence.** The areal-water convergence problem and the `8-10%` produced-oil gap
against Flow (WATER-022) are the same defect seen from two sides: a persistent oil material
imbalance that ResSim cannot drive to OPM's level. Fixing it improves correctness (closing the oil
gap) and speed (removing the wasted iterations and dt fragmentation) at once. The next step is a
same-state material-balance comparison against Flow — the WATER-017 dump apparatus applied to this
case — to attribute the `2.5e-6` oil imbalance to its source: a well surface-rate/reservoir-flux
inconsistency, or an accumulation/FVF inconsistency in the oil equation. This is a correctness
investigation that also happens to be the dominant speed lever, so it supersedes further solver,
controller, or per-iteration-cost work.

No production behavior changed. The only source additions are the default-off `FIM_W023_FULL_TRACE`
trace-to-file hook and `FIM_NESTED_WELL_SOLVE` plumbing on the 23x23x1 driver to match the 12x12x3
one. `validate-solver-coverage.sh fim` 5/5.

### WATER-024: the oil material-balance floor is the saturation front on SWOF breakpoints (2026-07-23)

WATER-023 localized OpmAligned's areal-water slowdown to a persistent oil material-balance floor
(`~2.5e-6`) and named a WATER-017-style same-state comparison as the next step. That comparison is
unnecessary: the imbalance is fully attributable from ResSim's own observability, and it is not a
well defect.

Added `water024_oil_mb_line` (`#[cfg(test)]`, gated on `FIM_W024_OIL_MB`), which decomposes every
cell's oil-equation residual — accumulation, six signed face fluxes, well source — at the accepted
state of each substep the outer controller visits, using the live AD breakdown helper. Direct
single-step reproduction was abandoned because a full-dt step from the uniform initial state exits
at iteration 1 (dt too large) at every dt tried; the in-loop hook instead reads the real
limit-cycle substeps.

**The imbalance is the water front, not the wells.** On the 23x23x1 limit-cycle substeps
(`iters=20`):

| substep | dt | net oil | abs sum | well share | knot share | worst cell | worst accum / faces |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| 0 | `2.96e-3` | `-2.665e-2` | `2.665e-2` | `0.0000` | `1.0000` | 47 | `+0.01395 / -0.01451` |
| 0 | `3.52e-5` | `-2.995e-3` | `2.995e-3` | `0.0000` | `1.0000` | 1 | `+0.01032 / -0.01072` |
| 1 | `1.41e-5` | `-1.194e-3` | `1.194e-3` | `0.0000` | `1.0000` | 23 | `+0.00260 / -0.00270` |
| 3 | `1.69e-5` | `-1.326e-3` | `1.326e-3` | `-0.0000` | `1.0000` | 24 | `+0.00155 / -0.00161` |

Three facts, each decisive:

- **`well_share = 0.0000`.** The two perforated cells contribute nothing to the net oil imbalance.
  The well-coupling hypothesis from WATER-023 is refuted.
- **`knot_share = 1.0000`.** The entire coherent net is carried by cells whose `Sw` sits within one
  table segment of a SWOF breakpoint — the moving saturation front. This is the WATER-018
  endpoint-kink mechanism: the piecewise-linear relperm derivative is discontinuous at a knot, so a
  front cell straddling one cannot have its oil face-flux and accumulation simultaneously nulled by
  Newton. The worst cells (`1, 23, 24, 47`) are all on the injector-corner water front; at each,
  `accumulation ≈ -faces` with a residual of about `4%` left over — the kink inconsistency.
- **`net ≈ abs_sum`** on the areal case: the front cells' errors are one-signed and sum coherently.

**Why the flavors and cases differ, completely resolved.** Contrast the heavy 12x12x3 OpmAligned
step, which converges: `net_oil = 2.25e-4` against `abs_sum = 2.38e-2` — the per-cell front errors
are the same size but here they **cancel** (only `1%` of the absolute sum survives), giving MB
`2.6e-8`, below tolerance. In the 23x23x1 areal single-layer sweep the front is a clean radial band
of cells all at the same `Sw` on the same breakpoint, so the kink errors align and sum to
`2.5e-6`, above tolerance. Whether OpmAligned converges a case reduces to whether its front-cell
kink errors cancel (gas, layered heavy) or align (areal single-layer water). Legacy accepts the
raw-residual-converged state through its bailouts and so never sees the floor — which is why Legacy
is faster on the areal cases and also why its produced oil differs: the coherent front imbalance it
accepts is exactly the displacement-front oil error.

**Unifying finding.** WATER-018 (a `1e-7` state difference flips a relperm derivative at a
breakpoint), WATER-021 (the tabulated relperm win), WATER-023 (the MB floor and limit cycle) and
WATER-024 (the floor is `100%` front cells on breakpoints, `0%` wells) are one phenomenon: the
piecewise-linear saturation-function kink at the moving front. It is also the correctness gap,
since the front controls produced oil.

**Fix direction, not yet attempted.** OPM uses the same piecewise-linear SWOF and reaches `1e-7`,
so the defect is in how ResSim evaluates the tabulated relperm *at a cell crossing a knot during
Newton*, not in tabulation itself. The next step is to compare ResSim's tabulated value/derivative
interval selection at a front cell against OPM's `PiecewiseLinearTwoPhaseMaterial` (which side of
the knot each takes, and whether value and AD derivative stay on the same interval across an
iteration) and make them consistent so a front cell stops limit-cycling across the knot. Candidates
to weigh once that is understood: consistent one-sided knot evaluation, a small saturation-space
regularization of the kink, or the CNV-only "final iteration" acceptance OPM applies when only MB
is marginally violated.

No production behavior changed. The only addition is the `#[cfg(test)]`, default-off
`FIM_W024_OIL_MB` decomposition hook. `validate-solver-coverage.sh fim` 5/5; wasm builds clean.

### WATER-025: the areal-water floor is the hard Swc clamp, not the relperm kink (2026-07-23)

**Correction to WATER-024.** WATER-024 attributed the oil material-balance floor to the
piecewise-linear relperm kink at SWOF breakpoints. That is wrong. Re-running the decomposition with
analytic Corey (`FIM_COREY_TABLE_POINTS=0`, no knots at all) reproduces the floor identically:
`net_oil=-3.0e-3`, `well_share=0.0000`, same injector-corner front cells, `accum≈-faces`, same
`iters=20` limit cycle. The `knot_share=1.0000` figure was an artifact — the ±one-segment window
spans about half the swept saturation range, so "near a knot" was nearly vacuous. The floor is
**relperm-independent**. What survives from WATER-024 is solid and was the real clue:
`well_share=0` (not the wells), the imbalance is on the advancing water front, and forcing an exact
direct linear solve does not help.

**True cause.** If the Jacobian is consistent (the `jacobian_matches_numerical_of_real_residual`
gates pass) and the linear solve is exact, yet the residual floors, the update must be clipped. The
front-adjacent cells sit at exactly `Sw=Swc=0.1` (confirmed in the trace: `sw=0.1000, so=0.9000`),
and `FimState::enforce_cell_bounds` hard-clamps stored `Sw` to `Swc` after every Newton update.
OPM does the opposite: `project-saturations` defaults false, so OPM keeps the raw Newton saturation
as the primary variable and clamps only inside the material-law evaluation. ResSim's hard clamp
pins the ahead-of-front cells every iteration, so the oil balance there can never null — the
coherent sum over the front band is the `2.5e-6` floor.

**Confirmation.** Routing OpmAligned through the existing `apply_newton_update_frozen_raw_saturation`
(raw `Sw`, pressure floor only) drops the reservoir residual from the `2.5e-6` floor to `1.275e-16`
— the floor vanishes. But it then exposed a second, coupled gate: `respects_basic_bounds` requires
`Sw>=Swc`, so raw saturations were rejected as an "invalid bounded Appleyard candidate" and dt
collapsed. The two are one mechanism. Bypassing that bounds check under raw saturations (exactly as
the pre-existing `y2b3_primary_variable_lifecycle` path already does) completes the fix.

**The fix.** Under OpmAligned, keep the raw Newton saturation and bypass the `Swc`-requiring bounds
check, matching OPM's `project-saturations=false`. Two-line change in `fim/newton.rs`, gated to
OpmAligned only; `FIM_W025_DISABLE_RAW_SW` restores the old hard clamp for A/B. Legacy is untouched
by construction.

**Results (native drivers and wasm matrix).**

| case | OpmAligned before | OpmAligned after | Legacy (unchanged) |
| --- | ---: | ---: | ---: |
| water 20x20x3 dt.25 | `90` sub / `22687 ms` | **`3` / `1561`** | `8` / `3479` |
| water 23x23x1 dt.25 | `70` / `6108` | **`6` / `894`** | `4` / `1432` |
| water 12x12x3 dt1 | `4` / `858` | `4` / `864` | `24` / `4379` |

OpmAligned now beats Legacy on the areal cases. Correctness improved in the same motion, exactly as
WATER-023 predicted the shared defect would: produced oil `rate*dt` against Flow's gravity-off
`FOPT` moves from `+11%` to `+6.9%` on 20x20x3 (`815` vs `762.52`) and sits at `+8.0%` on 23x23x1
(`363` vs `336.24`). The remaining `~7%` is a separate correctness item, no longer entangled with
the convergence floor.

**Validation.** `validate-solver-coverage.sh` `fim` 5/5, `impes` 2/2; the shipped gas replay
(Legacy) is unchanged (`8, 4, 4, 4, 4, 4`); gas OpmAligned single-step is clean (`oil=160.76`,
valid `Sg`, `retries=0/0/0`). The A/B override reproduces the old `72` substeps. The `FIM_W024_OIL_MB`
decomposition hook and `FIM_W025_DISABLE_RAW_SW` override are the only diagnostics retained.

This closes the WATER-018→025 arc with a single, OPM-faithful root cause: ResSim was hard-projecting
saturations to `Swc` where OPM keeps them raw, and that projection — not the relperm table, the
wells, the linear solver, or the growth controller — was the areal-water convergence floor and part
of the produced-oil gap.

### WATER-026: OpmAligned promoted to the default FIM nonlinear flavor (2026-07-23)

With WATER-025 removing the last areal-water convergence floor, OpmAligned is now the better path
on every measured case, so `fim_opm_aligned_nonlinear` now defaults to `true`
(`frontend.rs`). FIM is dev-only — public scenarios run IMPES (`FIM_DEFERRED_BACKLOG.md`) and nothing
in `workers`/`catalog` wires the flavor — so this changes only the FIM dev path and the wasm
diagnostic's no-flag default. The diagnostic gains `--legacy` to opt out for A/B; `--opm-aligned` is
now redundant but still accepted.

New default baseline (wasm, no flag = OpmAligned):

| case | substeps | retries | outer ms | vs Legacy |
| --- | ---: | --- | ---: | --- |
| water 20x20x3 dt.25 | 3 | `0/1/0` | `1577` | Legacy `8`/`3479` |
| water 23x23x1 dt.25 | 6 | `4/0/0` | `918` | Legacy `4`/`1432` |
| water 12x12x3 dt1 | 4 | `0/0/0` | `864` | Legacy `24`/`4379` |
| gas-rate 20x20x3 dt.25 | 1 | `0/0/0` | `844` | — |
| gas-rate 10x10x3 6-step | 1/step | `0/0/0` | `~40-410`/step | shipped Legacy `8,4,4,4,4,4` |

**The gas replay is fixed, not deferred.** The gas-rate OpmAligned multi-step that previously ran
away to 501 substeps now completes in one substep per step with correct GOR (`80.0`, matching the
injection GOR) and valid `Sg`. The A/B (`FIM_W025_DISABLE_RAW_SW`) shows this particular case does
not depend on WATER-025 specifically; it was carried by the cumulative WATER-021 relperm-consistency
and default-table work. Either way it is no longer an open item.

Validation: `validate-solver-coverage.sh` `fim` 5/5, `impes` 2/2 under the new default. `shared`
stops only at the pre-existing `closed_system_public_step_keeps_same_water_inventory_on_both_solvers`
failure, confirmed by A/B to fail identically under both flavor defaults (the documented
`rate_history` length case), so it is unrelated to this change. The `assembly_ad`
structural-parity failure on HEAD also remains pre-existing and separately tracked.
