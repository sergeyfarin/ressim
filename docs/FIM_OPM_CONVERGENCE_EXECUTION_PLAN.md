# FIM–OPM Convergence Execution Plan

Status: **Y2b0 complete; Y2b1 active (2026-07-13)**. This document turns the evidence in
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

This evidence supersedes the older claims that the gas failure was primarily CPR quality, that
OPM itself grinds through the same stagnation, or that AMG is the next convergence lever.

## 2. Rules that apply to every slice

- Read `FIM_EXPERIMENT_REGISTRY.md` before editing solver behavior.
- Change one causal mechanism per commit. Do not combine a boundary-policy change with well
  primary variables, controller settings, tolerances, or damping constants.
- Preserve Legacy behavior unless a later promotion gate explicitly authorizes an unconditional
  change. First behavior probes are `OpmAligned`-only and flag-gated.
- Do not broaden stagnation or final-state acceptance (`FIM-NEWTON-004`/`005`). A state that OPM
  reaches by a consistent Newton path must not be replaced by accepting an inconsistent plateau.
- Do not globally change generic clamp helpers or patch only the injector well row. Reservoir and
  well residuals must use one coherent state/update convention.
- Do not start AMG, G4 primary-variable restructuring, G5 variable substitution, or controller
  parity while Y2b is unresolved.
- Record exact commit, commands, case dimensions, flags, output artifact paths, and before/after
  metrics in the worklog. Update the registry even for a negative result.
- Use capped first-rung diagnostics before full runs. A full multi-step run is a promotion gate,
  not a discovery tool.

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
direct/live disagreement, or an unchanged plateau. Preserve tests, traces, and the negative
registry verdict when useful.

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

- **G4 well structure:** choose only if the bound-consistent trace still localizes the plateau to
  well/perforation rows or the per-perforation `q` formulation.
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
3. State one hypothesis, one observable that would confirm it, and one observable that would
   refute it.
4. Run the cheapest capped diagnostic capable of deciding it.

At the end of every slice, report:

```text
Commit tested:
Hypothesis:
Files changed:
Exact commands:
Before -> after metrics:
Controls unchanged/moved:
Verdict: PROMOTED | REVERTED | REFUTED | DIAGNOSTIC | OPEN
Registry/worklog/parity/TODO updates:
Next authorized checkpoint:
```

Commit code and its focused tests together. Commit the evidence/docs checkpoint after the result
is known. Never proceed to the next branch merely because a run improved one headline count.
