# FIM dense/sparse review and interaction-aware repair plan

Review date: 2026-09-22. Code reviewed and tested: clean
`e121aeaac618cb3d59a0a335a7758745792479e7`, native release, rustc 1.98.1.
**Deliverable: code review and executable investigation plan; no solver change or backend
promotion.** Registry: `FIM-DIRECT-001`. Related issue: [#23](https://github.com/sergeyfarin/ressim/issues/23).

The reported thousands of small-grid substeps justify investigation. They do not establish that
both LU implementations are wrong, or that dense is accurate because it is fast. Both routes
share assembly, primary-state handling, nonlinear acceptance and timestep control. A defect in
any shared component can affect both, and a backend change can expose or mask it. The next step
is to locate the first divergent decision on identical inputs, not choose another default.

This plan supersedes the **backend recommendation**, not the historical measurements, in
[the cross-target investigation](FIM_CROSS_TARGET_DIVERGENCE_2026-09-22.md) §8. Keep the existing
production routing until a candidate meets the gates below. Do not infer that keeping it means
sparse has been vindicated. The completed F0–F8 repair and parked WATER/G4/G5 work remain intact.

## 1. Review findings

Paths below are relative to `src/lib/ressim/src/`; line numbers refer to the reviewed commit.

| Finding | Evidence in code | Consequence / confidence |
|---|---|---|
| Target-dependent backend selection is real | `fim/linear/mod.rs:330,355` selects sparse LU natively and dense LU on wasm; `fim/newton.rs:320` separately selects the direct fallback. `should_force_direct_solve` also overrides an explicit small GMRES request on wasm. | Confirmed behavior defect for cross-target consistency. It identifies a trigger, not why the correction trajectories differ. |
| The generic offline comparison does not reproduce current production options | `fim/linear/solver_lab.rs:1434` constructs `FimLinearSolveOptions::default()` (`use_true_fgmres=false`) and supplies captured `Some(equation_scaling)`. `frontend.rs:102` defaults the simulator to true FGMRES; `timestep.rs:1062` forwards it; the live calls in `newton.rs:1914–1948` pass `None` for linear equation-family checks. | Confirmed oracle mismatch. `solver_lab_compare_backends` also omits dense LU, can route its named CPR case to direct LU, and asserts historical failure rates. Do not use it unmodified to adjudicate this incident. This is a separate harness from the pasted Python benchmark. |
| Full and reduced routing can differ | `fim/linear/well_schur.rs:229` sends the reduced system through the public dispatcher, re-enabling forced-direct selection. | Confirmed control flow. A system with 170 cells and four well/perforation rows has 514 full rows but 510 reduced rows: it can reach direct LU. After a failed full direct solve, the reduced solve can also attempt direct again. “Above 512 rows both are always iterative” is too broad. It does not invalidate the stated 900/1200-row examples. |
| The existing grid test is not a separation of spatial and temporal error | `tests/physics/depletion_grid_convergence.rs:23–24,122` uses 20 calls at five days on every grid, with adaptive internal stepping, and checks contraction of field averages. | Confirmed limitation. A change to `DT_DAYS` alone changes the final time from 100 to 20 days when testing `dt=1`. The pasted finer-dt run has not documented whether `STEPS` was adjusted. Neither result proves the test is merely an artifact. |
| The pasted benchmark has additional measurement gaps | Recovered `bench_backend.py` counts `len(rate_history())`, estimates rows as `3*cells`, does not assert completed time or solver warnings, and compares each run with its own backend's single `dt/50` result. | Confirmed harness limitations. Actual well rows, real/replayed accepts, failed horizons, and agreement of the two refined limits are unmeasured. A 50-fold refinement is not proof of a converged reference. |
| Direct solves are guarded by residuals, but rejection provenance is lost | Both direct modules compute the residual against the input matrix. Sparse distinguishes conversion/factorization only in its test diagnostic; dense uses a zero vector when LU returns `None`. The public report has no explicit preparation status. | No demonstrated CSR-conversion or LU-arithmetic defect from this review. A tiny residual does not establish a unique or well-conditioned correction; a finite zero vector is not proof a factorization succeeded. |
| Well elimination is not always exact despite its general description | `well_schur.rs:96` calls `gmres_block_jacobi.rs::invert_tail_block`, which substitutes a diagonal approximation when inversion fails and drops some small couplings during elimination. | Confirmed conditional hazard, already partly covered by the report-contract test. The full-system residual guard rejects the tested singular-tail result. Whether this mechanism occurs in the reported cases is unmeasured; do not attribute the incident to it yet. |

The review also followed the live AD assembly, raw/tagged state update, accepted-state writeback,
Newton convergence and retry path. Existing tests protect several of these contracts (§2).
They are shared by dense and sparse and must remain in scope if same-system corrections agree.
No new common physics defect was demonstrated by this review.

## 2. Evidence collected in this review

These commands ran on the clean reviewed commit, before documentation edits. Each timeout
returned exit 0; these are focused checks, not the full promotion matrix:

```bash
timeout 120 cargo test --release --manifest-path src/lib/ressim/Cargo.toml report_contract -- --nocapture
timeout 120 cargo test --release --manifest-path src/lib/ressim/Cargo.toml fim_repair_ -- --nocapture
timeout 120 cargo test --release --manifest-path src/lib/ressim/Cargo.toml assembly_ad -- --nocapture
FIM_CAPTURE_SEQUENCE_DIR=/tmp/fim-review-e121aea-sequence timeout 120 cargo test --release --manifest-path src/lib/ressim/Cargo.toml native_single_step_fim_probe_case_a_24_cells -- --ignored --nocapture
```

Summary output:

```text
report_contract: test result: ok. 7 passed; 0 failed; 0 ignored
fim_repair_:    test result: ok. 6 passed; 0 failed; 0 ignored
assembly_ad:   test result: ok. 13 passed; 0 failed; 0 ignored
native probe:  {"nx":24,"ms":8.598,"time":0.250000,"warning":"","history":1}
native probe:  test result: ok. 1 passed; 0 failed; 0 ignored
```

The 24-cell probe exercises capture plumbing; it is **not** a reproduction of the 50/160-cell
or 12×12 pathology. Its time is a single observation, not a performance baseline. Logs are
`/tmp/fim-review-{report-contract,lifecycle,assembly,capture}.log`. Captures use v2 and cannot
alone restore nonlinear state/options. Use a fresh directory for every rerun.

Recovered benchmark source/results live under
`/tmp/claude-1000/-home-coder-Repos-ressim/dab26d45-79e8-4e2c-ae9d-a541939873a6/scratchpad/`:

| Artifact | SHA-256 |
|---|---|
| `bench_backend.py` | `bf6686425d8a5db1b0211752e2d33cfe89504cce02cf79d4ad17333f80073aab` |
| `bench_dense.json` | `04d2134ecd253b6bdacbf6527656741816a5f04ca0edcc6793155b4b2576fb17` |
| `bench_sparse.json` | `604142704f50f193847f10f5bdd4425ca704c6c63b0e8c17374a630b628cf8ea` |

They substantiate what the pasted harness measured, but do not pin the loaded native extension
to a source commit. The 4,697/9,021 counts and runtime ratios remain **reported, not reproduced
here**. Do not promote them into the committed convergence baseline. Temporary artifacts may
disappear; S0 below must produce durable fixtures and evidence.

## 3. Execute S0–S5 in order

Each stage ends in a scoped commit and an evidence record. A local mechanism test and a full
simulation answer different questions. A control regression can require reverting a patch
without refuting a coupled hypothesis. Never change a tolerance to get past a stage.

### S0 — Make the comparison faithful and bounded

**Deliverable:** a committed small-case driver and a backend-neutral capture/replay test.
Reuse the capture parser and report checks; replace the historical lab assumptions for this
new experiment rather than rewriting old experiment evidence.

1. Pin the source revision, build profile, target, compiler, binary hash and complete case
   configuration. Rebuild each binary and keep separate target/output directories. Start with
   the committed `crates/ressim-py/parity/cases.json` 50-cell first-day fixture, then reproduce
   the recovered 160-cell and 12×12 two-day cases. Preserve the distinction between those
   fixtures and the existing 24-cell/0.25-day probe.
2. Add a test-only backend selection that reaches **actual** dense LU, sparse LU and true
   FGMRES/CPR without target overrides. Also expose the unchanged production dispatcher as a
   separate variant. Carry “forced direct allowed” through Schur recursion explicitly; assert
   the actual backend at each level. A requested name is not an observation.
3. Record the options actually passed at the call site, including flexible recurrence,
   full-residual/family acceptance, elimination, tolerances, restart and iteration budget.
   Capture both full and reduced dimensions, CSR entries/RHS, row/column units and meanings,
   variable/equation scales, full current and previous state, trial dt, primary tags, well
   topology/control, retry/controller memory and the raw/applied correction. Existing v2
   matrix files do not contain all of this; use a versioned extension or sidecar.
4. Stop at the first different factorization result, correction, chop, phase switch, well
   post-processing, acceptance or retry. Capture iteration 0 too: a roundoff-size correction
   at an endpoint can change the derivative branch at iteration 1. No need to finish 9,000
   substeps before investigating the first split. Bound each exploratory live run by 120 s
   and a declared attempt limit; save partial progress as a timeout, not a successful run.
5. Prove capture/replay fidelity by comparing the live and replayed report **and correction**
   with identical options; check full-system residuals independently. Diagnostic mode must
   leave the uninstrumented trajectory unchanged.

**Exit:** at least one reproducing small fixture with its first-divergence artifact, replay
fidelity, explicit preparation status and effective routing. Otherwise verdict is
`INCONCLUSIVE — reproduction/oracle incomplete`; do not change solver physics.

### S1 — Classify the first different linear system

Run all candidates on each identical full system captured just before and at the split. Reuse
`sparse_lu_debug::diagnose`, direct kernels, the true-FGMRES kernel and the test-only dense SVD
helper, but make every option explicit. The SVD is an investigation tool, not a new production
backend or an OPM Newton-update oracle.

For every candidate record:

- factorization/conversion status, actual full/reduced backend and fallbacks;
- `||b||`, finite correction, independently computed `||b-J dx||`, reduction, reservoir/well
  partitions and equation-family scaled peaks;
- normwise backward error `||b-J dx||∞ / (||J||∞ ||dx||∞ + ||b||∞)` and componentwise backward
  error using `|J||dx|+|b|` (define zero-denominator behavior);
- correction differences by pressure, saturation, Sg/Rs, BHP and rate, normalized by declared
  physical variable scales; a single norm over bar and saturation is insufficient;
- nonfinite/duplicate/empty rows and columns, singular values/rank under stated row/column
  scaling and a cutoff sweep. An unscaled SVD cutoff on mixed units is not a physical verdict.

| Observation | Next action |
|---|---|
| CSR/dense reconstruction differs, or one method has large backward error on a well-conditioned nonsingular system | Isolate conversion, permutation, solve or refinement with a small regression fixture. Fix that contract before live experiments. |
| Both have small residuals but corrections differ substantially | Test conditioning and the approximate nullspace: measure `J*(dx_dense-dx_sparse)`, variable support and branch sensitivity. Do not call either exact solution the truth by name. |
| Sparse rejects and dense accepts a near-singular system | Determine rank/compatibility and whether dense used tiny pivots; test correction stability under controlled perturbations. Do not infer that sparse rejection is a library bug or dense acceptance is sound. |
| Both direct paths reject a compatible singular system | Audit inactive-variable/endpoint lifecycle and iterative behavior. Do not insert an arbitrary diagonal or relperm slope to make LU invertible. |
| Raw corrections agree but applied states diverge | Proceed directly to S2; investigate chop, branch selection, well update and acceptance. |
| Recovered Schur correction is wrong or tail inverse is approximate | Compare unreduced and reduced systems. Separate approximate preconditioning from exact elimination; a singular tail must be explicit and use a justified full-system solve/rejection. |

**Exit:** a minimal distinguishing contract or a documented, bounded hypothesis. Source algebra,
FD consistency, and a well-conditioned synthetic solve have different evidential roles.

### S2 — Audit the shared nonlinear lifecycle where the trace points

Check residual/Jacobian consistency at the first divergent state, including accumulation,
flux/upwinding and connected well rows. Use central directional FD over an epsilon sweep in
smooth regions and active one-sided differences at table knots, endpoints and phase/control
switches. AD/legacy agreement alone is not independent physics validation.

Compare `R(x)+J dx` with the residual at the raw candidate, then at each subsequent state:
pressure/saturation chop, primary adaptation, well inner solve/relaxation, accepted-state flash
and simulator writeback. Record `actual_dx`, not only the proposed correction. Require the
accepted residual to describe the committed state. Verify independent component inventories
and accepted-dt source integration, and that a rejected attempt cannot change physical state
or cumulative accounting. Extend the existing `fim_repair_` tests at the demonstrated defect.

Before any state/property change, write a dependency table:

| Coupled semantics | Required treatment |
|---|---|
| Stored raw/tagged primaries and Sg/Rs adaptation | Mark matched/held/missing for the exact fixture; two-phase and three-phase are separate cases. |
| Endpoint property values and active derivatives | Keep scalar and AD consumers consistent; preserve validated physical curves. |
| Accumulation, flux and well component sources | Use the same primary-state and quantity conventions. |
| Well unknowns, local solve, control switching and Schur recovery | Keep the full connection/control lifecycle coherent. |
| Linear quality and nonlinear acceptance | Measure both separately; preserve existing acceptance tolerances. |
| Commit, rollback and timestep memory | Distinguish physical rollback from intentional controller state. |

**Exit:** an independently demonstrated defect, or a valid nonlinear sensitivity explanation.
Missing Flow state/update diagnostics remain `INCONCLUSIVE`; do not substitute a direct solve
of its exported matrix for its actual update. Read the OPM pipeline skill before a new reference
run. Source-pin any claimed OPM lifecycle rather than copying an old comment.

### S3 — Separate time error, grid error and backend error

First keep the original grid-convergence gate intact. Add a diagnostic with final time fixed at
**100 days**, grids `5,10,20,40` (add 80 only if needed), and requested dt
`5,2.5,1.25,0.625,...`. Change the number of steps with dt. Assert final time, no warning,
finite state and actual accepted-time sums. Record real/replayed accepts and retries explicitly.

For each grid/backend, refine time until successive changes are small relative to the spatial
difference being adjudicated. A useful predeclared diagnostic budget is temporal uncertainty
below 10% of that spatial difference; when the spatial difference is near zero, use an absolute
quantity budget declared before the run. This is a diagnostic error budget, not permission to
alter the existing acceptance tolerances. Refine the nonlinear/linear solve only as a labeled
accuracy experiment to establish that algebraic error is below the same budget.

Compare dense and sparse **fine-time limits with each other**, as well as successive refinements
within each backend. Track pressure, Sw, Sg, Rs, Bo, cumulative production and all component
balances. For the Buckley-style cases, use the same two-day horizon across all dt levels and
retain final fields; max pressure error alone can hide transport errors.

Add a test-only prescribed substep schedule for paired runs. If either backend fails a prescribed
step, report that failure; do not silently cut and still claim equal timesteps. It is permissible
to refine the schedule for both and restart both from the same initial state. Report adaptive
production runs separately. Audit that spatial refinement holds domain, rock/fluid data,
boundary conditions and physical well specification constant; well-index/discrete-completion
effects are part of the error budget, not a backend preference.

**Exit:** distinguish (a) common limit with different adaptive time errors, (b) persistent
backend-dependent limit/failed step, and (c) unresolved temporal/spatial error. Fixed 0.8
contraction at coarse grids is not a theorem at a phase transition. Replace a scientific test
only with a documented asymptotic/reference argument and explicit tolerance justification,
never because one backend fails it. The pasted `dt=1` experiment does not supply this gate.

### S4 — Test interactions deliberately

Do not immediately discard a demonstrated fix because one full-run metric worsens. Name two
mechanisms A and B from S1/S2 (for example, valid linear-direction selection and the matching
primary/property lifecycle), then run baseline, A, B and A+B from the **same committed base**:

| Variant | Local contract A | Local contract B | Paired-step correction / residual | Full-horizon physics error | Real accepts / retries / time |
|---|---|---|---|---|---|
| baseline | measure | measure | measure | measure | measure |
| A | measure | held | measure | measure | measure |
| B | held | measure | measure | measure | measure |
| A+B | measure | measure | measure | measure | measure |

For a scalar metric M, report `M(A+B)-M(A)-M(B)+M(base)` to expose interaction, alongside the
underlying outputs. Iteration counts are discontinuous, so this contrast is descriptive; it
does not replace a causal trace or physics gate. Repeat at a neighboring dt and a second case
to avoid promoting an isolated favorable trajectory.

If A and B are inseparable pieces of one valid lifecycle, the partial corners are explicitly
incomplete probes, not admissible production candidates. Evaluate the coherent bundle with
targeted ablations and local invariants. `REVERTED` for control regression does not imply
`REFUTED` for the complete mechanism. No unguided Cartesian sweep of historical flags.

**Exit:** a minimal coherent candidate that fixes the demonstrated contracts and meets both
accuracy and work criteria. If no candidate does, retain the first-divergence evidence and the
specific missing contract; do not choose dense solely on speed or sparse solely on one pass.

### S5 — Promote and unify only after the evidence

Use one target-independent routing policy for the validated candidate, with explicit backend
requests honored and tested through reduction and fallback. Test 169/170/171-cell fixtures with
and without well tails, and synthetic dispatcher boundaries at 511/512/513 rows; the separate
CPR pressure-coarse threshold must not be confused with the full-system threshold.

Run the exact pathological small fixtures, original depletion gate, both refined-limit checks,
the capillary `wf_capillary` fallback regression, and the affected physics tests. Also run:

- FIM locked three-test baseline, curated FIM/shared buckets, AD parity and Buckley benchmarks;
- rebuilt WASM bounded matrix **and** long water/gas horizons from `fim-solver-debug`;
- `pnpm run validate:full`, native-binding parity and `pnpm run test:deployed` as prescribed by
  the repository (native-binding parity currently includes non-strict FIM cases, so a green
  script alone is not proof the target difference was removed);
- relevant ignored release gates: SPE1 full horizon, SPE1 areal refinement, depletion FIM;
- an independently matched small-case physical/reference oracle that actually exercises the
  changed route. Existing 900-row OPM/SPE1 results are useful controls, not small-LU arbitration.

Make cross-target tolerances explicit for pressures, saturations, rates and cumulative totals;
justify them against the accuracy budget. Do not silently turn a known difference into a loose
pass. No nonfinite state, incomplete horizon, hidden acceptance widening or unexplained
conservation loss is promotable. Require the pathological real-substep explosion to be removed,
not merely renamed in the ledger. Record wall times with build cost excluded and repeat timings.

Rerun the affected shortlist on the final clean implementation commit, record exact commands,
options and key output, update the registry/worklog/status and the owning issue, then remove or
document the remaining diagnostic switches. Do not push or publish without authorization.

## 4. Immediate handoff

**Next task is S0**, not a production backend swap, endpoint regularization, acceptance change
or restart of the old Flow research sequence. S1 follows only after a faithful first-divergence
replay exists. S3's harness can be prepared independently, but no accuracy verdict precedes
its completed-horizon and temporal-convergence checks.

Each handoff must give: source/binary identity; fixture; exact command; full/reduced route;
oracle validity; first divergent state/decision; hypothesis; confirming/refuting observations;
coupled semantics matched/held/missing; before/after physics and work; verdict; next stage.

This review changes documentation only. It does not claim the fragmentation is fixed, either
backend is correct on the reported cases, or all promotion gates passed. Remote issue publication
is deferred under the user's no-publication instruction; the local plan records the related
issue and evidence for that update.
