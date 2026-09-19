# FIM repair audit — 2026-09-18

Audited `origin/master` at `f5838eb`, fetched on the audit date, in an isolated worktree.
The user's `compositional-modelling` checkout was not modified. This review stops at the
FIM repair; it does not assess the compositional branch or C12 forensics.

## Verdict

The delivered repairs are useful and the targeted regression gates pass. No new production
numerical failure was reproduced. However, **the original F0–F8 plan is not fully demonstrated**:
F5's tests do not establish its actual simulator commit/retry contract, F6's shared-geometry
conclusion is unsupported, and some planned accuracy/interface work was deferred while the
handoff still declares completion. These are validation and readiness findings, not evidence
that the current solver necessarily commits incorrect states.

**Follow-up status (2026-09-19):** A1–A4 are addressed by `a8e553b` and `8d7bfec`. The new
outer-commit test rejects the audit's deliberate write-back mutation. The independent #10 oracle
and #21 waterflood accuracy study remain open work; the documentation no longer presents them as
completed or causally resolved.

## Findings

### A1 — P1: F5 does not test the commit/retry boundary it claims to protect

Evidence:

- `src/lib/ressim/src/fim/newton/tests.rs:2008–2039` reassembles
  `report.accepted_state` and reclassifies a clone. Despite the test name and comments, it
  never calls `write_back_to_simulator` or reads a committed simulator state back.
- `src/lib/ressim/src/fim/tests/repair_lifecycle.rs:76–154` calls `run_fim_timestep`
  directly for a failed attempt and another direct call for the retry. It does not execute
  the outer timestep controller's rejection, dt reduction, acceptance, history or ledger
  updates. The retry comparison checks Newton iterations and accepted cell values.
- The rollback snapshot omits cumulative source ledgers, full history contents and gas
  balance. Its fixture is two-phase, and the options use the default `Legacy` flavor;
  the public default `OpmAligned` retry lifecycle is not covered by those tests.
- The new closed-system test checks water and oil in a two-phase system. It does not
  establish the plan's gas inventory or integrated nonzero accepted-source contract.

**Mutation check:** temporarily changed the three-phase assignment in
`fim/state.rs` from `sim.rs[idx] = derived.rs` to
`sim.rs[idx] = derived.rs + if cfg!(test) && sim.three_phase_mode { 1.0 } else { 0.0 }`.
All five `fim_repair_` tests still passed. The mutation was removed and the Rust tree
verified clean. This demonstrates a blind spot in this test group, not in every repository
test, and is not a defect present in the audited production source.

Required follow-up:

1. Keep the inner-Newton checks, but name their scope accurately.
2. Add a deterministic test through the real outer timestep path that commits a three-phase
   accepted result. Reconstruct physical state and component inventory from the simulator;
   compare with the accepted solution and independently re-evaluate the applicable residual.
   The write-back mutation above must fail this test.
3. Inject one deterministic rejected attempt at the outer attempt boundary, then permit the
   normal smaller-dt retry. Compare with a clean run using the same accepted substep sequence.
   Check pressure/saturations/Rs, well state, time, full history, component inventories and
   cumulative accepted sources. Separate intentional controller memory from physical state.
4. Run both nonlinear flavors and a gas/phase-transition case with nonzero well sources.
   Integrate sources using actual accepted dt, independently of the reporting ledger.
5. Update the F5/F8 envelope only after these tests pass; do not change solver policy merely
   because coverage is missing.

### A2 — P2: shared-code agreement cannot exclude a shared geometry error

`docs/BLACK_OIL_VALIDATION.md:241,273–285` and
`docs/FIM_REPAIR_HANDOFF_2026-09-15.md:95` conclude that shared well geometry is ruled out
and #10 is IMPES-scoped. The evidence is a reconstructed case in which FIM and IMPES agree.
Both solvers reuse the geometry under investigation: a shared error can affect both and
preserve agreement. The original shipped case was not replayed.

The supported conclusion is **not reproduced in the reconstruction; cause/applicability
INCONCLUSIVE**. This does not establish that geometry is defective, either.

Required follow-up: correct the causal claim, retain #10 as unresolved, and use the exact
scenario plus an independent Peaceman/hydrostatic/source oracle or matched OPM reference
before excluding the shared path. The corresponding GitHub closeout discussion should use
the same qualification. #10 is already open; no reopening is needed.

### A3 — P2: F6–F8 completion exceeds the delivered scope

The original plan explicitly requires a matched #21 case and a three-level timestep study
(`FIM_REPAIR_EXECUTION_PLAN_2026-09-14.md:288–298`). The validation table instead says
“not attempted” and “out of F6 scope.” Deferral is reasonable if documented as a scope change;
it does not satisfy that planned acceptance criterion. The unexplained #11 9% gas-saturation
difference also remains an accuracy limitation. A behavior-preserving extraction can be safe
without establishing the physical accuracy of the foundation it preserves.

F7 delivered useful named black-oil layout constants and linear layout metadata. The plan's
short interface design for model-specific trial/admissibility, accepted-state evaluation and
snapshot/commit/rollback is not supplied by those constants or the pre-existing damping and
convergence modules. Newton remains coupled to `ReservoirSimulator` and `FimState`.
This is a narrower delivery than the full boundary design requested at plan lines 314–319;
it does not imply that a large generic-framework rewrite was required.

Finally, handoff line 14 says “F8 replayed G0–G5,” but its G5 provenance lists only three
release tests. The plan's G5 also lists the OPM gas comparison. There is no final replay of
that comparison recorded there. G5 is conditional in the plan: explicitly marking this
comparison inapplicable/deferred would be valid; claiming the entire gate was replayed is not.

Required follow-up: distinguish “black-oil regression repairs verified” and “layout extraction
verified” from the outstanding accuracy/lifecycle/interface gates. Complete the gates or
record an explicit revised scope with named dependencies. Keep #21 and #22 open for their
actual remaining work. This audit makes no conclusion about whether the later compositional
branch supplies any of that work.

### A4 — P3: finite-difference sweep checks the best sample, not a stable region

`fim/wells.rs:2253–2282` tries four perturbation sizes and selects the minimum relative error.
This is useful corroboration, but one lucky sample can pass; it does not enforce the stable
finite-difference region requested by the plan. Assert agreement across adjacent usable step
sizes and include the sweep values on failure. The current derivative was not shown wrong;
this is a test-strength improvement and does not invalidate the main #27 repair.

## Repairs supported by review

- **F1 / #28:** the connected-cell stencil test now matches the implemented well contract;
  heterogeneous neighbors and multiple completions strengthen the oracle.
- **F2 / #27:** the old gas fixture was not configured as gas, and test-only derivative helpers
  omitted the water branch. The new fixture exercises gas, the helpers use production
  derivatives, and local finite differences corroborate them. This was not a production
  Jacobian repair.
- **Missing-SCAL recursion:** extracting the two-phase total-mobility fallback removes the
  recursive three-phase fallback cycle. Its focused regression test passes.
- **F3 / #13:** curated gates cover the previously omitted modules and require executed passing
  tests, so an ignored-only filter cannot masquerade as execution. Remote CI evidence is real.
- **F4:** the backend/Schur report tests materially improve coverage of the returned full-system
  correction. They protect an existing residual-recomputation safeguard.
- The longer water/gas controls reproduce the improved convergence baseline; there is no
  evidence here to restart the historical stall investigation.

## Independent validation on `f5838eb`

| Check | Result |
|---|---|
| `bash scripts/validate-solver-coverage.sh all` | exit 0; 38 curated gate lines |
| `cargo test --manifest-path src/lib/ressim/Cargo.toml benchmark_buckley -- --nocapture` | 3 passed |
| Three locked FIM filters below | 1 passed per filter |
| Missing-SCAL fallback and gas-injector source derivative filters below | 1 passed per filter |
| `bash scripts/build-wasm.sh` | source build succeeded |
| Four short WASM controls below | all completed; no warnings |
| Water-pressure, 12×12×3, 20 reports × 1 day | 23 accepted substeps; retries 0/0/0; no warnings |
| Gas-rate, 10×10×3, 24 reports × 0.25 day | 27 accepted substeps; retries 0/1/0; no warnings |
| Temporary write-back mutation, `cargo test … fim_repair_ -- --nocapture` | all 5 passed, demonstrating A1; mutation reverted |

The locked filters were `drsdt0_base_rs_cap_flashes_excess_dissolved_gas_to_free_gas`,
`spe1_fim_first_steps_converge_without_stall`, and `spe1_fim_gas_injection_creates_free_gas`.
Additional filters were `three_phase_mode_without_scal_table_falls_back_to_two_phase_total_mobility`
and `gas_injector_source_row_has_exact_pressure_conversion_derivative`.

WASM replay command template:

```sh
node scripts/fim-wasm-diagnostic.mjs --preset PRESET --grid GRID --steps N --dt DAYS --diagnostic outer --no-json
```

Short controls: `water-pressure` at `20x20x3`, `22x22x1`, `23x23x1`, and `gas-rate` at
`20x20x3`; each one report at `dt=0.25`. Long controls use the table parameters.
Retries retain the diagnostic's three-counter ordering.

[CI run 35013467285](https://github.com/sergeyfarin/ressim/actions/runs/35013467285)
was independently checked through GitHub: success at
`02a5c015ba3e81e51fe317e99af6302bd7387223`, including rebuilt WASM, Rust gates, BL,
lint/cycles/typecheck, full Vitest and production build. This is historical candidate CI,
not a fresh full-product run on the audit head.

Local logs are temporary audit artifacts in `/tmp/ressim-fim-audit-*.log`; commands and results
are recorded here so the conclusions do not depend on retaining those files. The full cargo
suite was deliberately not used. This audit did not rerun the release-only G5 tests, the #25
TypeScript scenario, or a fresh OPM trajectory comparison. Their historical records were
reviewed, not independently re-established.

## Handoff

Follow-up implementation on this branch addresses A1 with outer commit/retry tests, A2 by
correcting the shared-code inference, A3 with an explicit boundary contract and qualified gate
claims, and A4 with a stable-adjacent-region FD assertion. #21 and the independent #10 oracle
remain open scientific work; they are now described as such. No compositional review is part of
this follow-up.
