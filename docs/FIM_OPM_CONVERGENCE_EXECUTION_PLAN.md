# FIM–OPM Convergence Execution Plan

Status: **Y2d6c capture-v3 corpus regeneration is complete; the exact test-only DUNE BiCGSTAB
recurrence is next (2026-07-15)**. This document turns the evidence in
`FIM_OPM_PARITY_PLAN.md` into a bounded sequence that can be executed without choosing a new
solver lever by intuition. The parity plan remains the Bundle Y evidence record; this file owns
the current order of work, gates, and handoff instructions.

## 1. Strategic objective and current truth

The goal is not to make one ResSim case “converge somehow.” It is to remove demonstrated
semantic or algorithmic divergences from OPM Flow until the same physical problem follows a
comparable Newton trajectory, without weakening convergence acceptance.

Primary oracle: tracked `gas-rate-10x10x3` deck, six 0.25-day report steps.

| Solver | Accepted substeps | Newton iterations | Cuts |
| --- | ---: | --- | ---: |
| OPM Flow 2026.04 | 6 | `7, 5, 4, 3, 4, 3` | 0 |
| ResSim baseline `OpmAligned` | hundreds | many at the 20-iteration cap | many/fragmented |
| ResSim lifecycle candidate (default-off) | 6 | `8, 5, 4, 4, 4, 4` | 0 |

The established causal chain is:

1. The live linear stack is not the primary cause on the exact first rung: forced direct and
   live CPR produce the same plateau.
2. Removing wells or retaining only the producer converges at the full trial step; retaining the
   injector reproduces or worsens the failure. Injector coupling is necessary and sufficient on
   that rung.
3. At `Sw = Swc`, AD uses the above-bound derivative while a raw Newton correction points below
   the bound; ResSim then projects the candidate back to `Swc`. The linear model predicts motion
   that the state update discards.
4. Local OPM source shows its normal Newton update limits saturation increments by `ds-max`, but
   saturation projection is optional and defaults off. ResSim hard-clamps `Sw >= Swc` after every
   update. This is a source-confirmed implementation divergence.
5. The Y2b2 live raw-state probe materially improved the accepted first rung. Its exact raw-state
   capture has 120 structurally empty gas-primary columns, so ordinary Sparse and dense LU both
   correctly reject the singular matrix. A rank-revealing dense SVD solves its compatible system
   to `1.10e-12` relative residual and tracks CPR closely. The raw-state probe therefore remains
   inconclusive, not refuted; it also establishes that raw retention without OPM-style variable
   adaptation cannot supply a valid nonsingular direct-LU oracle.
6. The completed tagged lifecycle removes the gas plateau across all six report steps and at
   `20x20x3`, validating that mechanism. It is not the complete OPM stack: one bounded water
   control regresses against Legacy and the heavy-water case still takes seven substeps to Flow's
   one.

This evidence supersedes the older claims that the gas failure was primarily CPR quality, that
OPM itself grinds through the same stagnation, or that AMG is the next convergence lever.

## 2. Rules that apply to every slice

- Read `FIM_EXPERIMENT_REGISTRY.md` before editing solver behavior.
- Change one falsifiable causal question per commit. Do not combine a boundary-policy experiment
  with unrelated controller, tolerance, or damping tuning. When OPM source requires coupled
  lifecycle semantics, keep them in a dependency-aware bundle and explicitly list which pieces
  are matched, held constant, or still missing.
- Before treating a direct/live comparison as an independent gate, require both paths to report
  backend-neutral full-system `rhs_norm`, final `||rhs-J dx||`, reduction, finite correction, and
  reservoir/well row partitions. `reduction=n/a`, reduced-only norms, or backend-specific failure
  payloads make the result `INCONCLUSIVE`; they do not refute solver physics or state policy.
- Preserve Legacy behavior unless a later promotion gate explicitly authorizes an unconditional
  change. First behavior probes are `OpmAligned`-only and flag-gated.
- Do not broaden stagnation or final-state acceptance (`FIM-NEWTON-004`/`005`). A state that OPM
  reaches by a consistent Newton path must not be replaced by accepting an inconsistent plateau.
- Do not globally change generic clamp helpers or patch only the injector well row. Reservoir and
  well residuals must use one coherent state/update convention.
- Do not start AMG, G4 primary-variable restructuring, G5 variable substitution, or controller
  parity until a dependency table scopes the missing OPM phase-presence/primary-variable lifecycle
  exposed by Y2b2c.
- Record exact commit, commands, case dimensions, flags, output artifact paths, and before/after
  metrics in the worklog. Update the registry even for a negative result.
- Use capped first-rung diagnostics before full runs. A full multi-step run is a promotion gate,
  not a discovery tool.
- A green ResSim test, AD/legacy parity check, or finite-difference match proves its named local
  contract only. It is not an OPM-parity oracle unless the compared state, residual, variables,
  and update lifecycle are sourced from OPM.

## 3. Y2b0 — establish OPM and ResSim bound/update semantics (no behavior change)

Objective: decide whether the immediate mismatch is state feasibility/projection, derivative
selection at a kink, or both.

Tasks:

1. Trace the exact OPM update path used by the reference deck:
   - saturation primary-variable update and `ds-max` scaling;
   - default and deck/runtime value of saturation projection;
   - any post-update normalization;
   - material-law behavior below connate/residual endpoints;
   - primary-variable switching and phase-presence updates relevant to this deck.
2. Trace ResSim from raw Newton correction through damping, candidate construction,
   `FimState::enforce_cell_bounds`, phase/regime handling, and the next residual assembly.
3. Build this table for `Swc`, `Sg=0`, and the upper saturation constraint:

   | Boundary | OPM admissible state/update | ResSim state/update | AD derivative | legacy derivative | feasible directions |
   | --- | --- | --- | --- | --- | --- |

4. Add source file/line citations to the parity plan. Do not infer OPM behavior from comments in
   ResSim.

**Result (2026-07-13, commit `5c29a9d`, no behavior change): PASS.** The tracked deck has no
`project-saturations` or `ds-max` override; the actual Flow binary reports defaults
`project-saturations=false` and `ds-max=0.2`. The tracked harness re-ran Flow successfully:
six `TStep=0.25` rows, `NewtIt=7,5,4,3,4,3`, and `Conv=1` throughout. OPM keeps raw saturation
primary variables through the normal update and passes them to accumulation; endpoint material
laws clamp relperms separately. ResSim applies the same nominal per-cell `ds-max` chop under
`OpmAligned`, then hard-clamps the stored state before the next assembly. OPM also adapts primary
variables after every update; ResSim freezes the regime until accepted-state evaluation. Full
source table: `FIM_OPM_PARITY_PLAN.md` §15.1.

Pass condition: the complete update lifecycle is sourced and the exact deck's effective options
are known. **Met.** Stop condition: if OPM does project at `Swc` for this deck, discard the provisional
“projection divergence” hypothesis and proceed with a coherent active-set derivative audit only.

Checkpoint: docs/registry commit; no solver validation suite is required because behavior is
unchanged.

## 4. Y2b1 — boundary characterization (test/diagnostic only)

Objective: measure first-order consistency on both sides of each relevant boundary.

Tasks:

1. Add focused fixtures at `bound-eps`, `bound`, and `bound+eps` for `Swc`, `Sg=0`, and the upper
   saturation constraint.
2. For connected reservoir component rows and injector well rows, record residual values, AD,
   legacy derivative, forward FD, backward FD, and central FD.
3. Extend the exact first-rung trace with raw correction, damping/chop, candidate before bounds,
   candidate after bounds, predicted residual change, and realized next-assembly residual change.
4. Confirm direct and live linear backends still agree on the correction.

Decision gate:

- If hard projection discards the correction and breaks the linear prediction while OPM does not
  project, authorize Y2b2 as a bound-policy probe.
- If AD/FD disagreement persists away from a projected boundary, fix the residual/Jacobian
  formula first; do not change state bounds.
- If neither occurs, close the bound hypothesis and return to a measured G4 row/unknown mapping.

Validation: the focused tests plus the `assembly_ad` parity surface selected by
`ressim-validation/SKILL.md`. Commit diagnostic/test infrastructure separately from behavior.

**Result (2026-07-13, diagnostic/test-only): PASS — authorizes Y2b2.** The new fixture sweeps
`bound-eps`, `bound`, and `bound+eps` for `Swc`, `Sg=0`, `Sw` upper, and `Sg` upper with an
injector perforation plus its connected water/oil/gas rows. It records AD, legacy, and all three
finite differences. At a kink, AD agrees with the active one-sided derivative and central FD
straddles the discontinuity; this is not evidence for derivative averaging. The exact 10x10x3
live trace shows each observed `Swc` crossing proposes `Sw < 0.15`, then projects it back before
the next assembly. At `dt=9.78384825e-4`, the water row's raw next residual is
`-8.53e-11` versus its linear prediction `-8.53e-11`, while the projected state restores
`4.859e-3`; the same correction and result occur with forced-direct linear solve. OPM does not
project this deck. The hard projection is therefore a measured first-order consistency break,
not merely a source divergence. Y2b2 may now test exactly one reversible, default-off,
`OpmAligned` policy; no acceptance widening or G4/G5 change is authorized.

## 5. Y2b2 — smallest reversible behavior probe

Only enter after Y2b1 authorizes it.

Implementation constraints:

- Introduce one `OpmAligned`-only, default-off policy at the state-update boundary.
- Match the sourced OPM behavior exactly: saturation increment limiting, endpoint extension, and
  normalization must be treated as one coherent policy.
- Leave Legacy unchanged. Do not alter `Scalar::max_floor`/`min_ceil` globally and do not special
  case a well row.
- Add focused tests showing both old Legacy and candidate OPM-aligned semantics.

Run the capped matrix first: both wells/rate, injector only, producer only, no wells,
pressure-controlled wells; live and forced-direct where relevant. Compare accepted first-rung
`dt`, retry count/classes, Newton iterations, plateau length, bound excursions, CNV/MB/well
residual history, and direct/live correction agreement.

Pass condition: the exact injector case obtains a materially larger accepted first step and/or
removes the repeated 20-iteration plateau, with residuals decreasing consistently. Revert the
behavior probe if it creates NaN/non-finite states, invalid phase totals, new failures in controls,
or an unchanged plateau. A direct/live disagreement is a stop condition only after the
backend-neutral correction and full-system reduction contract has been verified. Preserve tests,
traces, and an accurately classified registry verdict when useful.

**Result (2026-07-13): INCONCLUSIVE; implementation removed.** The native-only raw-saturation candidate improved
the live capped first rung from `dt=0.000978384825` (five nonlinear retries) to `0.00898425`
(three linear-classified retries), but forced-direct accepted no substep and exhausted all 16
retries. The two backends agree through the first three cutbacks and diverge at `0.00898425`:
live performs a third iteration and accepts, while direct fails its well row at iteration two.
The original slice treated this as a valid direct/live refutation. The subsequent report-path
audit shows that conclusion was unsound:

- both paths reach the same iteration-1 state at `dt=0.00898425`;
- live CPR returns `converged=false` with a measured reduction of `5.299e-3`, which satisfies
  OPM's relaxed `<1e-2` criterion; the applied correction leads to strict nonlinear convergence
  on the next iteration;
- Sparse LU returns a finite solution but always leaves `failure_diagnostics=None`;
- well Schur forwards those missing diagnostics, and Newton derives reduction only from that
  optional payload, producing `reduction=n/a` and a hard abort;
- at later tiny retries, the direct trace reports `would_accept=strict` before another mandatory
  linear solve, then retries solely because the same report has no usable reduction.

Therefore the direct run did not measure whether its iteration-1 correction was good or bad. The
9.2x live improvement is provisional positive evidence, not a promotion, while the direct result
is an oracle defect, not a physics refutation. The behavior code remains deleted until the oracle
is repaired and the exact probe is replayed.

The probe was also narrower than the phrase "coherent OPM policy" implied. It skipped ResSim's
post-update projection but retained frozen hydrocarbon regimes and flash-side hydrocarbon clamps;
it did not implement OPM's per-iteration `adaptPrimaryVariables`. It can test the local `Swc`
accumulation mechanism, but cannot refute the complete OPM raw-state/property/variable lifecycle.

### 5.1 Y2b2a — repair and validate the experimental oracle (complete 2026-07-13)

Objective: determine whether the Y2b2 direct/live split is a real correction-quality difference
or only report/wrapper semantics. Do not reintroduce raw-state behavior, alter G4/G5 structure, or
tune acceptance in this slice.

Implementation sequence:

1. In `fim/linear/mod.rs`, make the linear report expose backend-neutral full-system quantities
   without consulting `failure_diagnostics`: initial/RHS norm, final residual norm, and reduction.
   Keep backend-specific failure details optional.
2. In `fim/linear/sparse_lu_debug.rs`, populate those quantities on matrix-build failure,
   factorization failure, finite non-strict solutions, and converged solutions.
3. In `fim/linear/well_schur.rs`, publish the recovered **full-system** residual norm and original
   full RHS norm. Never forward a reduced solver's norm as though it described the full system.
4. In `fim/newton.rs`, make OPM relaxed linear acceptance consume the backend-neutral reduction,
   not `failure_diagnostics`. Do not change the `<1e-2` threshold or any nonlinear acceptance.
5. Add focused report-contract tests with declared names or equivalent coverage:
   - `sparse_lu_non_strict_report_has_reduction`;
   - `well_schur_report_uses_full_system_norms`;
   - `opm_relaxed_linear_acceptance_is_backend_neutral`.
6. Run the existing forced-direct first-rung driver with raw-state behavior still absent. Confirm
   that every finite non-strict report now emits a numeric full-system reduction rather than
   `n/a`. This is a report-contract check, not a new convergence baseline.

Required validation while implementing:

```text
cargo test --manifest-path src/lib/ressim/Cargo.toml sparse_lu_non_strict_report_has_reduction -- --nocapture
cargo test --manifest-path src/lib/ressim/Cargo.toml well_schur_report_uses_full_system_norms -- --nocapture
cargo test --manifest-path src/lib/ressim/Cargo.toml opm_relaxed_linear_acceptance_is_backend_neutral -- --nocapture
cargo test --manifest-path src/lib/ressim/Cargo.toml well_schur -- --nocapture
FIM_FORCE_DIRECT_LINEAR=1 FIM_MAX_SUBSTEPS=1 cargo test --release --manifest-path src/lib/ressim/Cargo.toml repro_gas_rate_10x10x3_y1j -- --ignored --nocapture
bash scripts/validate-solver-coverage.sh fim
bash scripts/validate-solver-coverage.sh shared
```

Y2b2a exit gate: focused tests prove failure diagnostics are optional metadata, every wrapper
publishes norms for the system represented by its report, well Schur overwrites them with original
full-system values after recovery, and the forced-direct trace has no finite `reduction=n/a`
outcome. Commit this oracle change and its docs before restoring any raw-state behavior.

**Result.** `FimLinearSolveReport` now publishes `rhs_norm`, `final_residual_norm`, and a
backend-neutral `reduction()`. Sparse LU populates the RHS norm even when its optional failure
diagnostics are absent; well-Schur replaces reduced-system values with residual/RHS norms from the
recovered original system; and the `OpmAligned` relaxed decision consumes the report reduction.
Focused Sparse-LU, well-Schur, and acceptance-contract tests passed, as did the AD assembly parity
bucket (10/10) and the existing well-elimination direct-solve check. A forced-direct baseline with
the raw-state behavior still absent retained its ordinary first-rung path and emitted no
`reduction=n/a`. The shared validation script still stops at the already-recorded unrelated
closed-system public-step assertion (`2` versus `1`); it is not a result of this change.

The original broad FIM script invocation did not reach a result in the tool harness, so it is not
recorded as passing. Y2b2b must rerun its locked FIM coverage along with its capture/replay gate.

### 5.2 Y2b2b — restore and replay the same narrow probe (complete 2026-07-13)

Only enter after the Y2b2a commit passes its exit gate. **Completed 2026-07-13.**

1. Restore the previously deleted native, default-off, `OpmAligned`-only raw-saturation flag
   without adding primary-variable adaptation, G4/G5 changes, acceptance tuning, or controller
   changes. This is intentionally the same narrow probe so the old result is comparable.
2. Add capture at the exact `dt=0.00898425`, iteration-1 decision point. Record matrix/RHS
   dimensions, state/well-vector checksum, and stable captured-artifact checksum.
3. Replay CPR and Sparse LU through the same production dispatch, including well Schur.
4. For each backend record `||rhs-J dx||/||rhs||`, finite status, correction norms, maximum
   componentwise correction difference, and reservoir/well row residual partitions. Compare the
   actual BHP and perforation-rate corrections, not only `converged` flags.
5. Rerun capped live and direct first-rung commands with the flag. Record retry classes, accepted
   `dt`, Newton iterations, CNV/MB/well histories, and the next state after any relaxed solve.

Decision table:

| Observation at the captured iteration | Verdict | Next authorized action |
| --- | --- | --- |
| CPR and Sparse LU corrections agree to numerical tolerance and both reduce the full residual below `1e-2` | Oracle defect confirmed | Classify the corrected capped live/direct result; if both accept consistently, proceed to the coherent OPM lifecycle bundle, not G4 |
| Sparse LU correction is materially worse and its full residual reduction is `>=1e-2` | Linear/Schur defect localized | Diagnose the direct factorization/recovery path; Y2b remains inconclusive |
| Corrections differ but both reductions are `<1e-2` | Backend trajectory sensitivity, not failure | Apply the same OPM relaxed rule, then compare the next nonlinear state; keep verdict inconclusive until replayed |
| Report contract cannot produce comparable full-system quantities | Oracle still invalid | Stop; do not issue a Y2b/G4 physics verdict |

Only after Y2b2b may raw-state retention be classified. If it remains beneficial, the next
behavior bundle must source and cover raw stored state, raw accumulation, endpoint-clipped
properties, and per-iteration primary-variable
adaptation together. G4 becomes eligible only if that coherent trace still localizes the plateau
to well equations or well primary variables.

**Result.** The restored probe is native-only, default-off, and `OpmAligned`-only. It retains raw
`Sw`/hydrocarbon variables while preserving the pressure floor and well/control bounds; it skips
only the candidate basic-bounds check that would reject that intentional raw state. It does not
change primary-variable adaptation, controllers, G4/G5 structure, or acceptance thresholds.

The capped live driver accepted `dt=0.00898425` after three linear retries and three Newton
iterations. Its one claimed decision artifact is `904x904`, `5540` nonzeros, RHS norm
`2.786530e1`, state/well checksum `5be96f7f56374d91`, and SHA-256
`c503d9cb1781eab942d7621fb45f0baada3c40db7e32ffc297d17d1d35611561`
(`/tmp/ressim-y2b2b-live-capture-final/fim_capture_00000.txt`). CPR replay of that exact artifact
has reduction `4.830552e-3` and a finite correction, including BHP
`[0.22494971034013922, -0.031642581144180365]` and perforation-rate
`[-5.37113701915633, 0.0013166107429607712]`. The same production dispatch with Sparse LU returns
the all-zero correction, reduction `1.0`, and no solved iterations; its maximum correction
difference from CPR is `5.371137e0`.

The capped forced-direct driver accepts no substeps and records 16 `linear_bad` retries. Its
separately captured iteration-1 state differs (`5626` nonzeros, checksum `ceb8e32d8ac4285e`,
SHA-256 `9575387e32442d78aac2fd9d4e3c8e03adaa16e3d6c4c8df0fbe1ba186597f2e`) because the zero
direct correction never applies live iteration 0. Replaying that direct-path artifact confirms
the same zero Sparse-LU correction; CPR on it reduces only to `1.902285e-2`, as expected for the
earlier state. The replay prints full residuals and reservoir/well partitions, not flags alone.

**Classification at this checkpoint.** Explicit Sparse LU bypasses well-Schur and returned zero,
so no conclusion about raw state could be drawn from this replay alone. Y2b remained
**inconclusive** pending the bounded Y2b2c structural closeout below.

### 5.3 Y2b2c — time-boxed direct-oracle closeout (complete 2026-07-14)

The capture remains the one-shot live artifact above (SHA-256
`c503d9cb1781eab942d7621fb45f0baada3c40db7e32ffc297d17d1d35611561`). Native test-only
`SparseLuDiagnostics` records construction separately from factorization and checks empty
rows/columns, duplicates, non-finite entries, all-zero rows, and missing/exact-zero diagonal
candidates before handing the matrix to faer. Exact replay command:

```text
FIM_Y2B2_CAPTURE_DIR=/tmp/ressim-y2b2b-live-capture-final cargo test --release --manifest-path src/lib/ressim/Cargo.toml replay_y2b2_exact_capture -- --ignored --nocapture
```

Matrix construction succeeds; `sp_lu()` factorization fails. There are no empty rows, duplicate
entries, non-finite entries, or all-zero rows, but exactly **120 empty columns** and 120
missing/zero diagonal candidates. All 120 are cell-local variable 2 (the gas-component primary);
no water, oil-component, BHP, or perforation-rate column is empty. Ordinary dense LU independently
rejects the same rank-deficient matrix. The test-only rank-revealing dense SVD has rank `784/904`
at cutoff `1.138287e-6` and solves the compatible full system to relative residual `1.101044e-12`
(reservoir `3.068078e-11`, wells `9.032212e-14`). Its correction differs from CPR by at most
`1.657241e-2`, whereas Sparse LU's zero correction differs by `5.371137e0`.

**Classification.** This is not a Sparse-LU implementation/convergence problem and not a
well-Schur problem: the raw-state capture is structurally rank-deficient because it retains 120
inactive gas primary variables. CPR's `4.830552e-3` correction is independently corroborated by
the rank-revealing direct oracle, but ordinary direct LU cannot be the Y2b promotion gate on this
partial lifecycle. The missing OPM per-iteration phase-presence/`adaptPrimaryVariables` semantics
are now a measured dependency, not a generic future enhancement. Do not tune Sparse LU or
acceptance. Y2b and Y2c remain blocked pending a sourced, dependency-complete lifecycle scope.

### 5.4 Y2b3 — lifecycle mapping and dependency design (complete 2026-07-14)

The deck-scoped OPM lifecycle and ResSim implementation contract are now source-mapped in
`FIM_Y2B3_PRIMARY_VARIABLE_LIFECYCLE_DESIGN.md`. OPM keeps a fixed composition-switch slot but
adapts its meaning and value (`Sg -> Rs` or `Rs -> Sg`) after every Newton update, before the next
residual/Jacobian assembly. It also carries per-cell switch hysteresis. ResSim has the equivalent
tagged storage representation, but its live Newton path freezes the tag; the raw-state probe can
then pass a negative `Sg` through a property/flash clamp that removes the slot's residual
dependency. This is the source-level mechanism consistent with Y2b2c's 120 empty gas-primary
columns; the next capture must directly record each cell's switch history to confirm it.

The design keeps the fixed three-variable cell block and changes the tag/value atomically. It
requires zero empty cell-primary columns before any behavior verdict and forbids diagonal patches,
well-row-only changes, inventory-flash substitutions for OPM's switch initialization, and solver or
acceptance tuning. Gates A-C are now implemented below. The next executable slice is Y2c's bounded
promotion matrix.

**Y2b3a result (2026-07-14): Gate A PASS; no convergence run.** The native/default-off raw-state
path now performs the deck-scoped atomic `Sg <-> Rs` adaptation before well post-processing and
carries OPM's per-cell previous-switch hysteresis within the Newton solve. Five transition tests
are green, along with all 13 state tests and 10 AD/legacy assembly parity tests. The locked FIM
tests pass. The curated `fim`/`shared` run reaches only the known pre-existing closed-system
`rate_history` assertion (`2` vs `1`) already recorded in `TODO.md`; preceding buckets pass.

Gate A deliberately does not close dependency structure. Its hysteresis test proves OPM may keep
a small negative `Sg` for one iteration, while ResSim's current AD property clamp can flatten that
variable. Y2b3b must provide the meaning-aware accumulation/property derivative and zero-empty-
column tests before the exact case is allowed.

**Y2b3b result (2026-07-14): Gate B PASS; no convergence run.** Tagged three-phase `Sg`/`Rs`
variables now remain raw through phase construction and component accumulation, while endpoint
material properties and the two-phase/no-PVT paths retain their previous bounded behavior. The
scalar and AD implementations changed together. Within-meaning one-sided FD covers the
hysteresis-retained `Sg=-5e-6` case and both endpoint transition meanings. One-cell transitions
and a mixed-regime gas-injector assembly have live hydrocarbon columns, finite entries, no empty
rows or columns, scalar/AD residual parity, and successful diagnostic Sparse-LU factorization.
The broader assembly, property, state, locked FIM, and curated FIM gates pass; `shared` reaches the
same pre-existing closed-system `rate_history` mismatch (`2` versus `1`). Gate B is structural and
does not promote Y2b. Gate C must now regenerate and classify the exact first-rung capture.

**Y2b3c result (2026-07-14): Gate C PASS; Y2b promotion candidate.** On clean commit `1a6460d`,
the capped live driver accepts the full `0.25` day target in one substep, 8 reported Newton
iterations, and zero retries, versus the historical bounded baseline's five cuts to
`0.000978384825` and the partial raw-state probe's three cuts to `0.00898425`. OPM accepts the
same first report step in 7 Newton iterations with no cut.

The historical `dt=0.00898425`, iteration-1 system was regenerated through a test-only exact-dt
selector because the fixed trajectory no longer visits that retry rung. All 300 tagged-primary
columns are traced and live (141 `Sg`, 159 `Rs`, minimum occupancy 2). The 904-row, 6815-nonzero
matrix has no structural defects and factorizes in both Sparse LU and dense LU. Their reductions
are `2.837291e-16` and `7.935809e-16`; CPR reduces by `4.911209e-7`. Direct corrections agree to
`7.285839e-15`, and CPR differs from direct by at most `5.557618e-7` across correction families.
The full-step accepted MB is `1.683337e-8`, and the final diagnostic-only trace is byte-identical
to the clean-commit trace. This closes the oracle and first-rung gate; it does not replace Y2c's
six-step/control/physics promotion decision.

## 6. Y2c — promotion matrix

Before the first behavior run, record a clean-current-commit baseline; do not reuse the historical
`459`, `238`, or `695` numbers across code revisions.

Promotion order:

1. Validate the tracked fixture and run the exact six-step ResSim target.
2. Re-run Flow for the final promotion checkpoint and confirm the tracked oracle remains six
   substeps with `7,5,4,3,4,3` Newton iterations.
3. Run the heavy water first-step oracle (OPM: one substep, 11 Newton iterations).
4. Run the bounded control matrix under Legacy and `OpmAligned`.
5. Run focused AD/legacy parity, locked smoke, FIM/shared solver buckets, and Buckley–Leverett
   gates required by `ressim-validation/SKILL.md`.
6. Check mass balance, finite state, saturation totals, reporting, and the accepted fine-dt
   production reference; iteration counts alone are insufficient.

A candidate is promotable only if the exact gas case is materially closer to OPM, no control case
gains new cuts/failures, and physics/validation gates pass. If it is beneficial but incomplete,
keep it isolated behind the flag and diagnose the next demonstrated mismatch; do not declare the
entire OPM stack ready.

### 6.1 Y2c result (2026-07-14): beneficial but incomplete; do not promote

The completed lifecycle is a large, reproducible improvement and matches the primary gas oracle's
substep count, but it fails the predeclared control non-regression gate. Keep
`FIM_Y2B_RAW_SATURATION=1` native/default-off.

| Case / flavor | Accepted substeps | Newton iterations / retries | Reference |
| --- | ---: | --- | --- |
| gas `10x10x3`, six steps, candidate | **6** | `8,5,4,4,4,4`; zero retries | Flow: **6**, `7,5,4,3,4,3`, zero cuts |
| gas `10x10x3`, six steps, Legacy | 14 | first step 4 substeps; 7 nonlinear retries total | candidate is materially closer |
| gas `20x20x3`, first step, candidate | **1** | `8`; zero retries | Legacy 2; baseline `OpmAligned` 238 |
| water `20x20x3`, candidate | 5 | `8,5,5,5,3`; 1 linear + 1 nonlinear retry | Legacy 8; baseline `OpmAligned` 24 |
| water `22x22x1`, candidate | **11** | mostly 3-5; **8 linear retries** | Legacy **4**; baseline `OpmAligned` 24 |
| water `23x23x1`, candidate | 3 | `11,5,5`; 1 linear retry | Legacy 4; baseline `OpmAligned` 12 |
| heavy water `12x12x3`, candidate | 7 | 1 nonlinear retry | Flow: **1**, 11 Newton |

Fresh Flow 2026.04 replay reconfirmed `6` substeps and `7,5,4,3,4,3`. The candidate's six-step
state is finite, saturation closure is at most `2.22e-16`, and the accepted `0.25`-day result is
close to a 24-step `0.0625`-day reference (oil/gas rates within `0.036%`, gas inventory within
`0.228%`, injection within `0.876%`). Focused assembly/state tests, the locked DRSDT0 test,
Buckley-Leverett benchmarks, and the curated FIM bucket pass. The shared bucket reaches the known
pre-existing closed-system `rate_history` mismatch (`2` versus `1`).

Classification: **VALIDATED POSITIVE, NOT PROMOTED**. The `22x22x1` regression against Legacy is
the direct promotion blocker; the heavy-water gap shows that the lifecycle is not the complete OPM
stack. Neither result refutes the sourced lifecycle mechanism. They constrain the next slice.

## 7. Choose exactly one next branch from post-Y2 evidence

- **Y2d1 is complete and selects Y2d2.** Production-faithful restriction replay proves strong
  corpus-dependent sensitivity but no existing restriction clears bounded and current gas gates
  together. Keep production quasi-IMPES fixed and isolate fine smoother versus Krylov budget on
  the same corpora before considering any live change.
- **G4 well structure:** choose only if the bound-consistent trace still localizes the plateau to
  well/perforation rows or the per-perforation `q` formulation after Y2b2a and the corrected Y2b2
  replay.
- **G5 variable substitution:** choose only if failures correlate with phase-presence or
  primary-variable switch events after bound policy is coherent.
- **Y1c heavy oscillation:** re-run after Y2 because the heavy injector also moves across a
  saturation boundary. Treat it as a separate cause only if its two-cycle remains.
- **Y3 controller parity:** choose only when Newton can hold the full report-step trial; then try
  the full target `dt` first and retain OPM-compatible failure cutback.
- **AMG/CPR scale-up:** remains deferred unless linear diagnostics on a larger problem show the
  Newton direction is limited by coarse-solve error. Current exact-case direct/live equivalence
  refutes it as the next convergence fix.

Only one branch may be active. Write a new registry row with a falsifiable hypothesis and gate
before implementation.

### 7.1 Y2d0 bounded-control linear-oracle gate

Hypothesis: the candidate's excess `22x22x1` cuts are caused by a localized full-system linear
correction-quality failure on the first `linear-bad` retry, rather than by the primary-variable
lifecycle itself.

- Capture the exact first candidate `linear-bad` system without altering its trajectory.
- Replay the same artifact through live CPR and one viable independent direct backend.
- Require comparable initial/RHS norm, final full-system residual norm, reduction, finite status,
  and correction-family deltas. A missing quantity makes the result `INCONCLUSIVE`.
- **Confirm** only if the direct correction is finite and materially better on the same full
  system while CPR fails the common residual contract.
- **Refute** if CPR and direct agree to the established correction/reduction scale; then classify
  the retry as nonlinear/controller trajectory evidence and select one new bounded branch.
- No acceptance widening, well-equation edits, timestep tuning, G4/G5 work, or production Sparse
  LU project is authorized in Y2d0.

**Y2d0 result (2026-07-14): hypothesis confirmed.** Clean commit `2030996` reproduced 11
substeps/8 linear retries and captured eight corresponding systems. The first artifact is
`1456x1456`, 4764 nonzeros, SHA-256
`725cbbc2cc06f1d31ef090c7b7f11e6374ce7b70a3feaf06ccc90f18309e786b`; it has no empty,
duplicate, non-finite, all-zero, or zero-diagonal-candidate rows/columns, and Sparse LU factorizes.
On the identical full system (`rhs_norm=2.541597987e3`), CPR reproduces the 30-iteration failure
with finite correction and reduction `1.441123105e-2`; Sparse LU is finite and converged with
reduction `5.546336962e-15`. Reported and independently recomputed residuals match, the CPR
residual is entirely in reservoir rows, and correction-family deltas are material (pressure
`181.498`, water saturation `0.351096`, perforation rate `1550.931`). This confirms a real
iterative correction-quality gap and excludes matrix build/factorization, report semantics,
well-row recovery, lifecycle, and nonlinear acceptance as explanations for this first retry.

### 7.2 Y2d1 production-faithful CPR component discrimination

The existing lab-only restriction comparison is a clue, not yet an oracle for a live change: on
the eight frozen failures, `row0-schur`/`local-schur-balanced` solve 4/8 while current
`quasi-impes` solves 0/8, but that helper bypasses production well-Schur elimination and captured
equation scaling. The next slice must repair that comparability gap rather than flip restrictions.

- Add a test-only way to replay an explicit restriction through the same well-Schur reduction,
  equation scaling, smoother, tolerance, and iteration budget as production.
- Run all existing restriction variants on all eight Y2d0 artifacts and report full-system
  residual/reduction, finite status, reservoir/well partitions, and direct-correction deltas.
- Re-run the established gas failure corpus (where quasi-IMPES previously won 336/337) as the
  mandatory counter-control; no one-case adaptive selector is authorized.
- **Confirm restriction mismatch** only if one variant materially improves the bounded corpus
  without losing the gas corpus under the production-faithful wrapper.
- **Refute restriction mismatch** if no variant improves both; then isolate smoother/Krylov
  behavior on the same artifacts without a live solver edit.
- No production behavior or convergence run belongs in Y2d1.

**Y2d1 result (2026-07-14): restriction sensitivity confirmed; no universal variant.** Test-only
restriction injection is byte-for-byte equivalent to production for quasi-IMPES and retains the
real well-Schur reduction/recovery, equation scaling, block-ILU0 smoother, tolerance, and
30-iteration budget. All direct references converge and every report matches an independently
recomputed full-system residual.

| restriction | bounded strict / relaxed (8) | current gas strict / relaxed (5) | bounded median reduction | gas median reduction |
| --- | ---: | ---: | ---: | ---: |
| row0-schur | `0 / 3` | `4 / 5` | `1.699e-2` | `2.814e-4` |
| sum-rows | `2 / 2` | `5 / 5` | `1.718e-2` | `1.472e-4` |
| diag-balanced-sum | **`8 / 8`** | **`3 / 3`** | `1.340e-3` | `4.867e-3` |
| dominant-diag-row | `0 / 3` | `5 / 5` | `1.355e-2` | `1.603e-4` |
| local-schur-balanced | `0 / 1` | `4 / 5` | `1.608e-2` | `2.814e-4` |
| quasi-impes (production) | `0 / 0` | `4 / 4` | `1.455e-2` | **`3.646e-5`** |

`diag-balanced-sum` resolves every bounded artifact and reduces its median direct-correction delta
from quasi-IMPES `508.9` to `4.23`, but loses one current gas artifact and catastrophically misses
two (`1.803e1` and `4.704e-1` relative residual). `sum-rows` preserves/improves gas but resolves
only 2/8 bounded systems. Therefore restriction choice is one contributor, not a complete global
remedy; no restriction flip or case-adaptive selector is authorized.

The historical 337-system gas corpus is unavailable and arose before later capture/report fixes.
Y2d1 cleanly regenerated the current `20x20x3` baseline on commit `e143c19`: 238 accepted
substeps, one linear and four nonlinear retries, and five captured final-near-miss/failure systems.
These five current-head artifacts are the valid counter-control; the historical 336/337 result is
context, not a reproducible gate.

### 7.3 Y2d2 fixed-restriction smoother/Krylov isolation

Hypothesis: with production quasi-IMPES held fixed, the bounded failures are caused by the
block-ILU0 fine smoother and/or the 30-iteration Krylov budget rather than pressure restriction
alone.

- Extend only the test-only production-faithful wrapper to select the existing fine smoother.
- On all eight bounded and five current gas artifacts compare block-ILU0 (production), full ILU0,
  and block Jacobi at the same tolerance/budget.
- Then vary only the Krylov budget (`30`, `60`, `150`) for the best no-regression smoother while
  keeping restart, restriction, scaling, and well Schur fixed.
- Require the same full-system norms, partitions, finite status, and direct-correction deltas.
- Confirm a smoother/budget cause only if one row resolves all bounded artifacts without losing
  any currently usable gas artifact. Otherwise close that component and inspect CPR coarse-stage
  quality; do not combine restriction and smoother changes to manufacture a win.
- No production behavior or live convergence run is authorized in Y2d2.

**Y2d2 result (2026-07-14): smoother refuted; 30-iteration truncation confirmed.** The
test-only production-faithful wrapper held quasi-IMPES, well Schur elimination/recovery,
equation scaling, tolerance, and production restart fixed. At effective budget 30, production
block-ILU0 is already the best existing smoother: bounded block-ILU0/full-ILU0/block-Jacobi are
`0/8`, `0/8`, and `0/8`, with median full reductions `1.455e-2`, `1.578e-2`, and `1.578e-2`;
current gas is `4/5` for all three, with block-ILU0 retaining the best median reduction
(`3.646e-5` versus `4.513e-4`). Full-ILU0 and block Jacobi are bit-identical on these corpora.

Keeping block-ILU0 fixed and raising only the effective budget gives a discrete boundary result:
all eight bounded systems stop unconverged at iteration 30, then converge at iteration 31 or 32.
The current gas miss likewise converges at iteration 31, improving gas `4/5 -> 5/5`; the other
four gas systems remain bit-identical. Budgets 60 and 150 return bit-identical corrections and
reports on every artifact. All systems are finite, all bounded residual is reservoir-only, and
the full report norms match independent residual recomputation.

Verdict: **CONFIRMED OFFLINE COMPONENT CAUSE; NO PRODUCTION CHANGE.** The existing smoother is
not the defect. The effective 30-iteration cap truncates nine hard systems one or two iterations
before their first post-restart convergence. A higher cap is a diagnostic workaround, not yet an
OPM-alignment mechanism: Flow's production path converges the reference Newton systems within
its 20-iteration limit. Do not promote `60`, run live convergence, combine restrictions, or open
AMG from this result alone.

### 7.4 Y2d3 restart-boundary convergence-history audit

Hypothesis: the sharp `30 -> 31/32` transition is caused by useful progress across the first
FGMRES restart boundary, and its trajectory will distinguish a budget bookkeeping mismatch from
weak first-cycle CPR quality.

- Add test-only per-iteration true full residual and preconditioned residual history to the same
  production-faithful replay; production dispatch remains unchanged.
- Replay the eight bounded artifacts and the one hard gas artifact at effective budgets 30 and
  60 with block-ILU0/quasi-IMPES fixed. Record the iteration-29 through convergence window and
  restart-cycle boundaries.
- First prove whether iteration 30 represents 30 completed corrections or a boundary check before
  the next correction. If it is bookkeeping, correct and unit-test that contract before any live
  run. If iteration 31/32 supplies genuinely new Krylov directions, quantify first-cycle versus
  post-restart true-residual reduction and inspect the fixed CPR coarse-stage solve next.
- Preserve full-system norms, reservoir/well partitions, direct deltas, and the five-system gas
  counter-control. Do not alter tolerance, smoother, restriction, scaling, well Schur, nonlinear
  acceptance, or timestep control.
- No production budget increase or live convergence run is authorized in Y2d3. AMG is authorized
  only as a later isolated diagnostic if the recorded history localizes the loss to coarse-stage
  quality rather than iteration accounting.

**Y2d3 result (2026-07-15): accounting and coarse tolerance refuted; Krylov contract
localized.** Test-only history proves that iteration 30 is a completed correction identical to
the production result. The bounded systems retain median true reduction `1.455e-2` through that
cycle, while the independently reapplied preconditioned residual is a median `1.169e19` times
larger than the internal Givens estimate. A fresh restart direction then reduces the
true residual by a median factor `1.294e-4` and converges at iteration 31 or 32. The hard gas
artifact behaves the same way: `4.897e-2` at iteration 30 to `3.440e-5` at iteration 31.

Two bounded controls separate coarse quality from the outer recurrence. Temporarily using the
existing exact dense pressure inverse for all 484 pressure rows makes every bounded system
converge in one outer iteration (median true reduction `3.225e-14`). Tightening the production
iterative pressure solve from relative tolerance `1e-6` to `1e-10` does not change the plateau:
bounded remains `0/8` at 30 and the gas corpus `4/5`; the misses still pass only after restart.
Both temporary constants were restored.

The implementation applies the preconditioner to the initial residual and to `A v`, then combines
the preconditioned Arnoldi basis directly into the solution. That is a fixed, left-preconditioned
GMRES recurrence, despite the FGMRES name. Above 300 pressure rows its CPR application contains a
stopping-tolerance-driven BiCGSTAB solve, so the map depends on its input and is not a fixed linear
operator. The Givens residual identity is therefore not valid; the iteration-one estimate/actual
split and restart reset are expected consequences. Verdict: **CONFIRMED ALGORITHM-CONTRACT GAP;
NO PRODUCTION CHANGE.**

### 7.5 Y2d4 true flexible-GMRES offline oracle

Hypothesis: a mathematically valid right-preconditioned flexible GMRES recurrence, storing each
applied search direction, removes the false residual collapse and reaches the direct solution
inside the existing 30-iteration budget without changing CPR components.

- Implement a test-only solver-lab path first. Use unpreconditioned Arnoldi residual basis
  `v_j`, compute and store `z_j = M_j^{-1} v_j`, form `w = A z_j`, and construct candidates as
  `x_0 + Z_j y`. Recompute the true residual for acceptance; do not reuse the current left-GMRES
  Givens estimate as a production convergence oracle.
- Hold quasi-IMPES restriction, block-ILU0, iterative pressure solver (`50`, `1e-6`), well Schur,
  scaling, restart `30`, maximum `30`, and nonlinear behavior fixed. No dense-pressure shortcut,
  tolerance sweep, budget increase, or AMG.
- Replay all eight bounded artifacts and all five current gas artifacts. Record iterations, true
  and estimated residual histories, full reservoir/well partitions, finite status, and direct
  correction deltas. Confirm only if bounded is `8/8`, gas is `5/5`, every hard system is within
  30 iterations, and no previously passing correction materially regresses.
- Include a small synthetic variable-preconditioner test that fails the old residual-estimate
  assumption but passes the flexible recurrence, plus a fixed-linear-preconditioner equivalence
  control. This prevents another corpus-only implementation error.
- If the offline oracle fails, classify the first divergence against the exact-dense bounded
  control before changing another component. If it passes, source-check OPM's production
  flexible-solver/preconditioner lifecycle and design a separately gated production candidate;
  do not run a live Newton comparison in this slice.

**Y2d4 result (2026-07-15): CONFIRMED OFFLINE; production unchanged.** The test-only solver uses
the required right-preconditioned recurrence (`v_j`, stored `z_j=M_j^-1v_j`, `A z_j`, `Zy`). A
synthetic nonlinear-preconditioner control demonstrates the old fixed-left estimate defect while
the flexible estimate matches the true residual; a fixed-linear control matches the direct
solution.

With production quasi-IMPES, block-ILU0, iterative pressure solve, well Schur, scaling, tolerance,
and effective budget 30 fixed, flexible GMRES resolves bounded `8/8` in exactly two iterations
versus production `0/8` at 30. Median bounded true reduction is `1.205e-8` and median maximum
family correction delta from Sparse LU is `1.641e-4`. Gas improves from `4/5` to `5/5` in one to
three iterations; the former hard gas system passes in one iteration at reduction `3.813e-3` and
direct delta `5.729e-6`. No production pass is lost. Estimate/independent-true disagreement stays
below `1.04e-8` bounded and `8.48e-11` gas.

The exact tracked Flow 2026.04 `CASE.DBG` prevents overclaiming OPM parity: that run selects outer
`bicgstab`, `maxiter=20`, `tol=0.005`, with `cprw`, true-IMPES weights, `paroverilu0`, and one
AMG-backed coarse loop. OPM's generic `FlexibleSolver` separately supports genuine `flexgmres`,
but it is optional rather than the reference configuration. Y2d4 therefore repairs the
mathematical contract of ResSim's chosen outer method; it does not reproduce OPM's full linear
stack.

### 7.6 Y2d5 default-off true-FGMRES production-path candidate

Hypothesis: routing ResSim's existing `FgmresCpr` configuration through the validated true
flexible recurrence removes the bounded and gas linear retries in live Newton solves without
regressing the other controls.

- Move the validated recurrence into production-capable code behind one explicit default-off
  solver option. Preserve the old path for A/B comparison. Do not change CPR restriction,
  smoother, coarse solver, tolerance, effective budget, well Schur, scaling, Newton acceptance,
  or timestep control.
- Unit-test dispatch/default-off behavior and exact equivalence between the promoted core and the
  Y2d4 oracle. Re-run both captured corpora before any live test.
- Run the capped candidate first on `22x22x1` water and current `20x20x3` gas. If both improve or
  remain stable, run the exact six-step `10x10x3` gas target and the full bounded/heavy control
  matrix. Record nonlinear iterations, linear retries/iterations, accepted substeps, cuts, and
  runtime—not only final convergence.
- Confirm only if the candidate preserves bounded `8/8` and gas `5/5` offline, removes the
  identified live linear retry class, does not worsen any control, and keeps physics gates green.
  Otherwise revert the production routing but retain the Y2d4 oracle and classify the first
  moved system.
- Keep the OPM distinction explicit. A passing Y2d5 is a ResSim algorithm-correctness promotion.
  Literal OPM linear parity (BiCGSTAB + true-IMPES CPRW + AMG coarse application) remains a later
  coupled design and must not be inferred from a flexible-GMRES win.

**Y2d5 result (2026-07-15): CONFIRMED DEFAULT-OFF; masked positive recovered, not default
promotion.** The exact Y2d4 recurrence is now production-capable behind `use_true_fgmres=false`
and the separate `setFimTrueFgmres`/`--true-fgmres` diagnostic switch. Default routing remains the
historical solver. Dispatch/default tests pass, and production dispatch is bit-exact with the
Y2d4 oracle on both corpora: bounded `8/8` in two iterations and gas `5/5` in one to three.

The first Legacy live gates remain stable: `22x22x1` stays at four substeps/two nonlinear retries
while accepted-rung linear iterations change `3,4,3,4 -> 2,3,3,3`; `20x20x3` gas stays at two
substeps/one nonlinear retry with `3,3 -> 2,2`. Under the complete Y2 primary-variable lifecycle,
the previously blocking `22x22x1` result improves decisively from 11 substeps/eight linear retries
to three substeps/zero linear plus one nonlinear retry. The other Y2 water controls preserve
substeps while converting their lone linear retry to nonlinear (`20x20x3`: five substeps,
`1L+1N -> 0L+2N`; `23x23x1`: three, `1L -> 0L+1N`). Heavy remains seven/one nonlinear.

This proves the invalid Krylov recurrence was masking a real Y2 lifecycle benefit. It does not
justify a default switch: the exact six-step gas target keeps six substeps/zero retries but Newton
counts move `8,5,4,4,4,4 -> 9,6,5,5,4,4`, farther from Flow's `7,5,4,3,4,3`; two water controls
also gain one or two accepted Newton iterations. The option is retained as a validated
algorithm-correctness path, default-off. No CPR or nonlinear component changed.

### 7.7 Y2d6 source-complete OPM linear-lifecycle design

Do not tune the true-FGMRES path next. The exact Flow oracle does not use it. Before implementing
another solver experiment, write a dependency-complete design for the actual selected Flow stack:

- source-pin outer BiCGSTAB stopping/budget semantics (`20`, `0.005`) and its residual norm;
- source-pin true-IMPES CPRW weights including well contributions and update frequency;
- source-pin `paroverilu0` block/sweep/relaxation semantics;
- source-pin the one-loop AMG coarse application and show why it is a fixed linear application
  suitable for BiCGSTAB, unlike ResSim's tolerance-terminated inner BiCGSTAB;
- map each item to ResSim as matched, intentionally fixed, or missing, including well Schur,
  scaling, block layout, fallback, and the current effective `20 -> 30` budget promotion;
- define one test-only coupled oracle on the existing 13 captures. A partial outer-method swap
  must be `INCONCLUSIVE`, not a refutation, while its required fixed preconditioner lifecycle is
  absent.

Only after that design may Y2d6 implement the smallest coherent captured-system path. It must
preserve full-system norms, partitions, direct deltas, bounded `8/8`, gas `5/5`, and compare
iteration counts against Flow's actual 20-iteration contract. No live run, default change,
acceptance/controller edit, or standalone AMG project is authorized in the design slice.

**Y2d6 design result (2026-07-15): COMPLETE; implementation not started.** The exact source pin is
`OPM/opm-simulators@release/2026.04/final` commit
`b82f21dba405286c4c4446614dd3bf9cdebf7a2c`, with DUNE-ISTL 2.11.0. The source audit adds two
requirements that make a partial outer swap invalid. First, true-IMPES comes from local storage
derivatives, which the current 13 captures do not contain. Second, Flow applies eliminated well
effects in the outer operator but factors `paroverilu0` on the reservoir matrix without those
effects, then adds well pressure contributions explicitly to the CPRW coarse matrix. ResSim
currently Schur-eliminates first and factors the already-reduced matrix. The complete design,
matched/missing table, coupled oracle, and IMPES applicability audit are in
`FIM_Y2D6_FLOW_LINEAR_LIFECYCLE_DESIGN.md`.

**Y2d6a result (2026-07-15): COMPLETE.** Capture v3 retains the full unscaled system plus exact
local accumulation-derivative blocks, source-pinned normalized true-IMPES weights, and explicit
`J_rr/J_rw/J_wr/J_ww` partitions before well elimination. Parsing recomputes every weight and
reconstructs the full Jacobian bit-for-bit. The isolated `22x22x1` bounded artifact has
`1456` rows, `484` cells, four well rows, and partition nnz `[4752,2,4,6]`; the exact gas artifact
has `904` rows, `300` cells, four well rows, and `[5360,3,2,7]`. Both pass the dedicated payload
oracle with the pinned Flow commit, DUNE version, and 50-bar pressure scale. No solver dispatch or
nonlinear behavior changed.

**Y2d6b result (2026-07-15): COMPLETE.** A test-only oracle proves all seven identities on the
bounded and gas v3 artifacts. Matrix-free well elimination matches an independently formed Schur
matrix; the coarse operator equals reservoir plus exactly one well contribution; block ILU is
factored only on `J_rr`; and the fine, coarse, and complete zero-pre/coarse/one-post CPR maps are
repeatable and linear. Both pressure systems (`484` and `300` rows) are below Flow's
`coarsenTarget=1200`; DUNE therefore creates no aggregation level and its sequential direct
coarse solver is the complete one-level AMG application for this bounded oracle. Independent
outer residual norms agree at machine precision. No outer Krylov solve or production path was
added.

Next is Y2d6c only: regenerate/extend the v3 bounded-eight plus gas-five corpus and compare the
coherent fixed stack with Flow's exact 20-pair BiCGSTAB contract. Preserve per-capture identity
gating; an identity failure is `INCONCLUSIVE`, never a convergence refutation.

**Y2d6c step 1 result (2026-07-15): COMPLETE.** A distinct `FIM_Y2D6_CORPUS_DIR` mirrors only the
two established selection hooks while preserving all older capture behavior: OPM-aligned linear
abort and final-iteration near miss. The current branch exactly regenerates bounded `8/8`
`max-iters` artifacts and gas `4` final-near-miss plus `1` `max-iters` artifact. All thirteen are
capture v3 and pass count, companion, source, weight, and bit-exact full-J reconstruction gates.
Next implement the test-only recurrence; do not route it live.

## 8. Y3 and Y4 end gates

Y3 controller parity starts only after full-target Newton convergence is plausible. Its target is
one initial trial for the whole report interval, then OPM-style cutback on genuine failure. It must
be evaluated on both exact gas and heavy-water references.

Y4 stack promotion may make `OpmAligned` or nested mechanisms default only when:

- the exact target is near the Flow oracle rather than merely better than Legacy;
- bounded controls are not worse than Legacy in cuts, failures, or physics outputs;
- all validation gates pass on a clean commit; and
- inert flags and superseded compensating mechanisms have an explicit keep/delete decision.

## 9. Simple-model handoff protocol

At the start of every slice:

1. Run `git status --short` and do not overwrite unrelated changes.
2. Read this plan, the active registry row, the latest Bundle Y section, and the relevant skill
   files.
3. State the hypothesis, oracle, one confirming observation, one refuting observation, and every
   coupled mechanism held constant or still missing.
4. Prove that the oracle reports comparable quantities before interpreting the target run.
5. Run the cheapest capped diagnostic capable of deciding it.

At the end of every slice, report:

```text
Commit tested:
Hypothesis:
Files changed:
Exact commands:
Before -> after metrics:
Controls unchanged/moved:
Oracle validity: VALID | INVALID (reason)
Coupled semantics omitted/held constant:
Verdict: PROMOTED | REVERTED | REFUTED | INCONCLUSIVE | DIAGNOSTIC | OPEN
Registry/worklog/parity/TODO updates:
Next authorized checkpoint:
```

Commit code and its focused tests together. Commit the evidence/docs checkpoint after the result
is known. Never proceed to the next branch merely because a run improved one headline count, and
never discard an improvement because a second path emitted incomparable diagnostics.
