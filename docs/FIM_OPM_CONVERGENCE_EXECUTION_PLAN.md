# FIM–OPM Convergence Execution Plan

Status: **Y2b0-Y2b2c complete; Y2b3a Gate A green; Y2b remains INCONCLUSIVE and Y2c is blocked
(2026-07-14)**. This document turns the evidence in
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
| ResSim `OpmAligned` | hundreds | many at the 20-iteration cap | many/fragmented |

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
   update. This is a source-confirmed implementation divergence, but it is not yet proven to be
   the sole cause of the live plateau.
5. The Y2b2 live raw-state probe materially improved the accepted first rung. Its exact raw-state
   capture has 120 structurally empty gas-primary columns, so ordinary Sparse and dense LU both
   correctly reject the singular matrix. A rank-revealing dense SVD solves its compatible system
   to `1.10e-12` relative residual and tracks CPR closely. The raw-state probe therefore remains
   inconclusive, not refuted; it also establishes that raw retention without OPM-style variable
   adaptation cannot supply a valid nonsingular direct-LU oracle.

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
acceptance tuning. Gate A is now implemented below. The next executable slice is Y2b3b's
derivative/structure Gate B; only then may the exact-capture Gate C run. Y2c remains blocked.

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

## 7. Choose exactly one next branch from post-Y2 evidence

- **Y2b3a Gate A is complete.** Execute Y2b3b Gate B before another behavior run. Gate C must
  show zero empty cell-primary columns and viable direct/live corrections before Y2b can be
  promoted, refuted, or used to select a structural branch.
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
