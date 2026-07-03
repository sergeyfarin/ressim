# FIM Experiment Registry

This is the fast pre-flight index for FIM convergence work. Before proposing
or implementing a FIM solver change, search this file for the mechanism, files,
target case, and failure family. If an equivalent experiment is already listed,
do not repeat it unless the `Retry only if` condition is satisfied.

Use this file for short, searchable verdicts. Keep detailed traces, tables, and
reasoning in the linked source documents.

## Registry Rules

- One row per attempted lever or diagnostic branch.
- `Verdict` values:
  - `PROMOTED` - kept in the solver or workflow.
  - `REVERTED` - tried and removed; do not repeat casually.
  - `REFUTED` - measured as a no-op or wrong mechanism.
  - `DIAGNOSTIC` - instrumentation or read-only result.
  - `OPEN` - promising but not yet attempted or closed.
- Record exact commands and baseline numbers in the source document, not in
  this index.
- When a future experiment supersedes a row, update the old row instead of
  adding a conflicting duplicate.

## High-Risk Repeats

| ID | Area | Hypothesis / Lever | Main Files | Main Cases | Result | Verdict | Retry only if | Source |
|---|---|---|---|---|---|---|---|---|
| FIM-AD-001 | Assembly | Switch live assembler from legacy hand derivatives to AD | `fim/assembly_ad.rs`, `fim/properties.rs`, `relperm.rs`, `fim/wells_ad.rs` | FIM control matrix, heavy water, gas 6-step, full suite | Found/fixed two-phase structural singularity plus Stone2 and connection-rate clamp kinks; gas shelf improved, heavy water initially mixed | PROMOTED | Future AD/legacy divergence touches assembly/properties/wells/relperm | `docs/FIM_CONVERGENCE_WORKLOG.md` "Phase 5 AD-assembler cutover" |
| FIM-AD-002 | Assembly | Delete legacy assembler after AD cutover | `fim/assembly.rs`, `pvt.rs`, `mobility.rs`, `wells.rs` | Production build, full suite, wasm size, shortlist | Kept legacy assembler as test-only parity oracle; gated dead-code helpers with `#[cfg(test)]` | PROMOTED | Only if a replacement parity oracle exists | `docs/FIM_CONVERGENCE_WORKLOG.md` "Phase 6 completed" |
| FIM-NEWTON-001 | Newton | OPM-style oscillation detection and relaxation scalar | `fim/newton.rs` | Full control matrix, heavy water | Heavy water improved `34 -> 31` substeps, controls unchanged | PROMOTED | Tune constants only with fresh control-matrix evidence | `docs/FIM_CONVERGENCE_WORKLOG.md` "Phase 7" |
| FIM-NEWTON-002 | Newton | Remove `nonlinear_history_stabilization_decision` after OPM relaxation | `fim/newton.rs` | Heavy water step trace | Redundancy hypothesis false; history cap tighter in 29/29 firings | REFUTED | OPM relaxation is retuned and shown tighter at those sites | `docs/FIM_CONVERGENCE_WORKLOG.md` "Sub-phase 7.3" |
| FIM-NEWTON-003 | Newton | Remove producer-hotspot stagnation bailout | `fim/newton.rs` | Heavy water, water-rate 20x20x3, gas controls | Zero fires across shortlist; mechanism removed | PROMOTED | New topology proves the removed mechanism is needed | `docs/FIM_CONVERGENCE_WORKLOG.md` "Sub-phase 7.4" |
| FIM-NEWTON-004 | Newton | Loosen or remove residual-stagnation bailout / above-tolerance stagnation acceptance | `fim/newton.rs` | Heavy water, medium water | Bailout still load-bearing; prior widening attempts regressed or no-oped | REVERTED | A new root cause explains the residual plateau and has a guarded fix | `docs/FIM_CONVERGENCE_WORKLOG.md` "Sub-phase 7.5"; `TODO.md` bounded front-local water follow-up |
| FIM-DIAG-001 | Diagnostics | Add nonlinear state detail at linear failure sites | `fim/newton.rs` | Heavy water, 23x23x1 | Pure instrumentation; controls bit-identical; exposed well-source-dominated cell rows | PROMOTED | N/A | `docs/FIM_CONVERGENCE_WORKLOG.md` "Phase 8" |
| FIM-LINEAR-001 | Linear/CPR | Blanket per-row infinity-norm scaling before iterative CPR/ILU0 | `fim/linear/gmres_block_jacobi.rs` | Heavy water and full control matrix | Heavy water regressed `31 -> 241` substeps; controls mostly unchanged | REVERTED | Scaling is equation-family-aware or CPR-pressure-only, not blanket full-system row scaling | `docs/FIM_CONVERGENCE_WORKLOG.md` "Hypothesis C attempt" |
| FIM-LINEAR-002 | Linear | Row/column scaling probe (`D_r J D_c`) | `fim/linear/gmres_block_jacobi.rs` | 4-case shortlist | No clean win; medium 6-step stalled; heavy/gas regressions | REFUTED | Preconditioner chain changes first, especially pressure weighting | `docs/FIM_LINEAR_SOLVER_AUDIT.md` "Finding 2" |
| FIM-LINEAR-003 | Linear | Intra-rung Jacobian / sparse-LU factorization reuse | `fim/newton.rs`, `fim/linear/mod.rs`, `fim/linear/sparse_lu_debug.rs` | Medium water, heavy water, gas shortlist | Strict gate added overhead; permissive gate diverged trajectory | REVERTED | Bypass architecture changes so hot iterations solve same-shape systems | `docs/FIM_JACOBIAN_REUSE_INVESTIGATION.md` |
| FIM-LINEAR-004 | Linear/Newton | Post-fail short-circuit after iterative CPR failure | `fim/newton.rs` | 4-case shortlist | Reduced wasted repeated CPR attempts; later work showed it is cost-saving, not substep-closing | PROMOTED | Revisit only if CPR backend behavior changes materially | `docs/FIM_BYPASS_AUDIT.md` "Lever 3 Stage 2" |
| FIM-LINEAR-005 | Linear/CPR | CPR pressure restriction should not be unconditional water row | `fim/linear/gmres_block_jacobi.rs` | Medium water, OPM comparison | Audit identified row-0 pressure restriction as a likely weakness; no current-head replacement attempted | OPEN | Use measurement-only comparison of row-0 Schur vs summed/dynamic/quasi-IMPES restrictions first | `docs/FIM_LINEAR_SOLVER_AUDIT.md` "Finding 3"; `docs/FIM_CPR_IMPROVEMENT_PLAN.md` |
| FIM-LINEAR-006 | Linear/CPR | Stronger coarse solver / AMG path | `fim/linear/gmres_block_jacobi.rs` | OPM ablations, over-threshold probes | OPM CPRW gives large linear-iteration savings; ResSim still uses dense <=512 else BiCGSTAB+ILU0 | OPEN | Scope as CPRW/AMG design, not another generic CPR heuristic | `docs/FIM_LINEAR_SOLVER_AUDIT.md`; `docs/OPM_FLOW_MINIMAL_MAPPING.md` |
| FIM-TIME-001 | Timestep | Cross-outer-step hard dt-cap carryover shared across water/gas | `fim/timestep.rs` | Heavy water, gas 6-step | Helped one gas shelf but regressed water guard; reverted | REVERTED | Family-specific policy with water guard evidence | `docs/FIM_CONVERGENCE_WORKLOG.md` "Water shelf" / "Gas shelf" |
| FIM-TIME-002 | Timestep | Gas-only outer-step first-trial carryover with fixed clean-step budget | `fim/timestep.rs` | Gas-rate 10x10x3 6-step, control matrix | Stabilized shipped gas replay after startup; bounded controls unchanged | PROMOTED | Change persistence only with full sequential gas replay and water controls | `docs/FIM_CONVERGENCE_WORKLOG.md` 2026-04-12 gas carryover |
| FIM-TIME-003 | Timestep | Accepted-site-aware carryover persistence | `fim/timestep.rs`, `fim/newton.rs` | Gas-rate 10x10x3 6-step | Narrower site-aware variants failed to hold full replay | REVERTED | New evidence beats fixed 3-clean-step budget | `docs/FIM_CONVERGENCE_WORKLOG.md` 2026-04-12 comparison branch |
| FIM-TIME-004 | Timestep | Post-cooldown hotspot regrowth caps | `fim/timestep.rs` | 20x20x3 water, controls | Added fragmentation without reducing retries | REVERTED | New front-local mechanism identifies why regrowth fails | `docs/FIM_CONVERGENCE_WORKLOG.md` 2026-04-11 water trace follow-up |
| FIM-TIME-005 | Timestep | Let exact no-op accepts decay hotspot memory earlier | `fim/timestep.rs` | Heavy water | Reopened `water@1020` ladder; severe retry/runtime regression | REVERTED | New release policy prevents same hotspot refail after memory clears | `docs/FIM_CONVERGENCE_WORKLOG.md` 2026-04-11 heavy guard |
| FIM-TRIAL-001 | Newton initial guess | Global/per-cell Newton extrapolation | `fim/timestep.rs`, `fim/newton.rs` | Heavy water, SPE1, shortlist | Three rounds failed: replay regression, gas regression, SPE1 collapse | REVERTED | Fundamentally different cell-selection anchor than Step-0 top-risk cells | `docs/FIM_SLICE_A_EXTRAPOLATION.md`; `docs/FIM_CONVERGENCE_IMPROVEMENTS.md` |
| FIM-TRIAL-002 | Replay/acceptance | Dt-aware replay acceptance tolerance | `fim/timestep.rs` | 6-case shortlist | Measured no-op; gate requires exactly zero update/diff | REFUTED | Replay gate precondition changes, but that overlaps failed extrapolation path | `docs/FIM_CONVERGENCE_IMPROVEMENTS.md` "Direction B attempt" |
| FIM-TRIAL-003 | Timestep | Water cross-outer-step first-trial carryover | `fim/timestep.rs` | Medium-water 6-step, shortlist | Fired but behavior bit-identical; boundary cold-start was not cost source | REVERTED | Evidence shows boundary first trial, not in-step ladder, is dominant | `docs/FIM_CONVERGENCE_IMPROVEMENTS.md` "Direction E attempt" |
| FIM-DAMP-001 | Newton damping | Ignore zero-move effective-floor iterations in stagnation counter | `fim/newton.rs` | 4-case shortlist, OPM comparison | Medium-water substeps improved strongly; heavy case moved closer to fine-dt physical reference | PROMOTED | Revisit only with fine-dt/OPM reference, not raw substep count alone | `docs/FIM_LINEAR_SOLVER_AUDIT.md` "Fix A1 Stage 2" |
| FIM-DAMP-002 | Newton damping | Remove Wang-Tchelepi fractional-flow inflection chop entirely | `fim/newton.rs` | Case 3 correctness discriminator | FOPT/physics degraded; full removal refuted | REVERTED | New correctness reference shows no basin-jump risk | `docs/FIM_LINEAR_SOLVER_AUDIT.md` "Fix A3 Stage 2" |
| FIM-DAMP-003 | Newton damping | Widen fractional-flow inflection chop threshold (`k=1.2`) | `fim/newton.rs` | 4-case shortlist, fine-dt references | Net lin_ms improved, correctness preserved vs fine-dt/OPM references | PROMOTED | Retune only with k-sweep and fine-dt reference | `docs/FIM_CHOP_WIDEN_EXPERIMENT.md` |

## How To Add A Row

Copy this template and keep it terse:

```md
| FIM-AREA-NNN | Area | Hypothesis / Lever | `file.rs` | target cases | measured result | VERDICT | required new evidence | source doc |
```

If the experiment changes behavior, the source doc must include:

- commit hash or clearly marked provisional dirty-tree status
- exact commands
- before/after summary lines
- validation controls that moved or stayed fixed
