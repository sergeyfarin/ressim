# Documentation Index

Which documents are authoritative, which are active working notes, and where historical material
lives. Tracker ownership moved to GitHub Issues on 2026-08-02; superseded plans and snapshots remain
under `.archive/`.

## Start here

| Document | Use it for |
|----------|------------|
| `README.md` | Product overview, current feature state, quick start, doc map |
| [GitHub Issues](https://github.com/sergeyfarin/ressim/issues) | Actionable work, priority, acceptance criteria, and status |
| `ROADMAP.md` | Strategic priority order and links to active issues |
| `TODO.md` | Stable landing page for issue links; not a second tracker |
| `docs/DOCUMENTATION_INDEX.md` | This map |

## Architecture & stable reference

| Document | Use it for |
|----------|------------|
| `docs/ARCHITECTURE_NOTES.md` | Current architecture direction and unresolved design decisions |
| [Open items](OPEN_ITEMS_2026-09-21.md) | Deliberate deferrals and known gaps in one place, each with why it was not done and what would close it. Starts with the wasm32/x86-64 FIM substepping divergence |
| [Engine payload boundary design](ENGINE_PAYLOAD_BOUNDARY_DESIGN_2026-09-21.md) | Pre-implementation note: how the engine's 12 remaining `JsValue`-bound functions become target-agnostic, by generalising the `compositional/api.rs` pattern to black-oil; phases, alternatives rejected, and the gates that keep it true |
| [Architecture split plan](ARCHITECTURE_SPLIT_PLAN_2026-09-19.md) | S0–S6 engine/frontend separation: measured WASM and module coupling at `93c8c5c`, the four leaf extractions that zero it, workspace packaging, a non-WASM engine target, and why one repository rather than several |
| [Compositional readiness assessment](COMPOSITIONAL_READINESS_ASSESSMENT_2026-09-14.md) | Rust/OPM reuse map, FIM prerequisites, reproduced well-contract gaps, and staged compositional scope |
| [Compositional fluid execution plan](COMPOSITIONAL_FLUID_EXECUTION_PLAN_2026-09-14.md) | C0–C15 implementation tasks, EOS/flash contracts, component equations, wells, native/WASM admission and immiscible-water extension |
| [Compositional validation](COMPOSITIONAL_VALIDATION.md) | Pinned fluid dataset and EOS constants, oracle availability and measured domain limits, numerical acceptance contract, and C0–C15 gate status |
| [Upstream OPM rate-control issue (draft)](UPSTREAM_OPM_RATE_CONTROL_DRAFT.md) | Unfiled draft for OPM/opm-simulators: rate-controlled compositional wells do not converge under timestep refinement, with the reproducer and the ruled-out list |
| [Compositional C12 forensics](COMPOSITIONAL_C12_FORENSICS.md) | Why the reported producer connection-rate defect was not one: the two methodological errors, the settled term-by-term register, and the rules for comparing against an unconverged reference |
| [Compositional well design](COMPOSITIONAL_WELL_DESIGN.md) | C11's pre-implementation design note: unknowns, connection law, injection rule, controls, provenance and gates |
| `docs/UNIT_SYSTEM.md` | Unit conventions, equations, solver / PVT notes |
| `docs/UNIT_REFERENCE.md` | Quick unit lookup card |
| `docs/TRANSMISSIBILITY_FACTOR.md` | Derivation of the transmissibility conversion factor |
| `docs/BENCHMARK_MODE_GUIDE.md` | Benchmark workflow semantics and comparison behavior |
| `docs/P4_TWO_PHASE_BENCHMARKS.md` | Buckley-Leverett benchmark methodology, tolerances, results |
| `docs/BLACK_OIL_VALIDATION.md` | SPE1 acceptance criteria, depletion grid convergence, black-oil solver safeguards |
| `docs/THREE_PHASE_IMPLEMENTATION_NOTES.md` | Three-phase implementation details and remaining validation gaps |
| `docs/SCENARIO_TERMINATION_POLICY.md` | Early-stop policy syntax, conditions, runtime behavior |
| `docs/SCENARIO_CATALOG_ARCHITECTURE.md` | Scenario ownership boundary, catalog taxonomy, and no-key-branching rule |
| `docs/OPM_FLOW_MINIMAL_MAPPING.md` | Minimal OPM Flow → ResSim solver mapping and CPRW-first plan |

## FIM — current truth

FIM ships in the user path since `b88ee28` (2026-07-24). Each scenario now declares its solver
policy and rationale explicitly; catalog assembly only applies that declaration and adds the generic
comparison sensitivity when requested. Convergence work on FIM remains developer-driven
(`docs/FIM_DEFERRED_BACKLOG.md`) — search
the registry **by mechanism name** before proposing any convergence change.

| Document | Use it for |
|----------|------------|
| `docs/FIM_STATUS.md` | Consolidated FIM state, blockers, validation entry points, canonical source map. Its **2026-09-15 banner** carries the current measured convergence baseline; the "Known Open Gaps" list below it is 2026-07-05 provenance |
| [FIM repair execution plan](FIM_REPAIR_EXECUTION_PLAN_2026-09-14.md) | F0–F8 repair order, issue mapping, exact gates and the solver-interface handoff. Completed record, preserved as issued — read its documentation-boundary note before citing it |
| [FIM repair handoff](FIM_REPAIR_HANDOFF_2026-09-15.md) | What F0–F8 repaired, the validated envelope, reusable layout surface, final regression replay and release-equivalence measurement; see its 2026-09-18 qualification |
| [FIM repair audit](FIM_REPAIR_AUDIT_2026-09-18.md) | Independent review of `f5838eb` and 2026-09-19 follow-up: repaired commit/retry coverage, corrected evidence claims, and named open scientific gates |
| [FIM model/solver boundary](FIM_MODEL_SOLVER_BOUNDARY_2026-09-18.md) | Minimal assembly/update/accept/commit/rollback adapter contract requested by F7; current reuse map and extraction admission tests |
| `docs/FIM_EXPERIMENT_REGISTRY.md` | Searchable anti-repeat ledger of levers, verdicts, retry conditions |
| `docs/FIM_CONVERGENCE_WORKLOG.md` | Active investigation log: current-head traces, temporary hypotheses |
| `docs/SOLVER_COMPARISON_SUMMARY.md` | Current OPM/FIM/IMPES timing + convergence re-baseline (clean tree `663e380`, 2026-07-24) |
| `docs/FIM_OPM_ALIGNMENT_STRATEGY_2026-04-26.md` | The 95%-track-OPM policy and Bundle A/B/C sequencing |
| `docs/FIM_OPM_CONVERGENCE_EXECUTION_PLAN.md` | Decision-frontier execution plan (oracle repair, raw-state replay, promotion matrix) |
| `docs/FIM_OPM_PARITY_PLAN.md` | Bundle Y evidence record and original Y0–Y4 roadmap. **Frontier superseded 2026-09-15** — the 695-substep gas-rate stall it tracks is fixed; read its top banner before acting on anything inside |
| `docs/FIM_DEFERRED_BACKLOG.md` | Deferred FIM convergence/validation work and the gates that remain open |

## FIM — design frontier (paused behind the WATER track)

Source-pinned, mostly default-off designs for the OPM gas-RESV injector / primary-variable
lifecycle. Paused while the product/validation track is active (see `FIM_STATUS.md` and roadmap
issues #21–#23). Kept because they are prescriptive and not yet superseded.

| Document | Scope |
|----------|-------|
| `docs/FIM_G4_INJECTOR_RESV_LIFECYCLE_DESIGN.md` | G4a single-perforation gas-RESV injector lifecycle design + oracle |
| `docs/FIM_G4B2_ATOMIC_ROUTE_READINESS_AUDIT.md` | G4b2 coupled-path audit + pre-Newton safety block |
| `docs/FIM_G4B2A_ATOMIC_ROUTE_IMPLEMENTATION_DESIGN.md` | G4b2a typed-u AD/legacy/Schur/trace implementation contract |
| `docs/FIM_Y2B3_PRIMARY_VARIABLE_LIFECYCLE_DESIGN.md` | Deck-scoped OPM `Sg`/`Rs` lifecycle map + fixed-layout contract |
| `docs/FIM_Y2D6_FLOW_LINEAR_LIFECYCLE_DESIGN.md` | Source-pinned Flow 2026.04 linear lifecycle + IMPES applicability audit |

## Frontend / scenarios / comparison roadmaps

| Document | Use it for |
|----------|------------|
| `docs/CHART_ARCHITECTURE_REVIEW_2026-08-02.md` | Current chart audit and migration boundary |
| `docs/FRONTEND_EXECUTION_PLAN_2026-07.md` | Delivered frontend Waves 0–3 and historical rationale |
| `docs/CASE_LIBRARY_ROADMAP.md` | Sourcing map for new scenarios: SPE benchmarks, field datasets, textbook cases |
| `docs/MULTI_SOURCE_COMPARISON_ROADMAP.md` | Comparison-axis roadmap across analytical/IMPES/FIM/OPM/published sources |
| `docs/COMPARISON_TOOLBOX_REVIEW_2026-07-01.md` | 2026-07 comparison-architecture findings and forward plan |
| `docs/WAVE4_REVIEW_2026-07-19.md` | Open post-Wave-4 review findings (ranked). **Untracked** — its findings were pointed at `TODO.md` checkboxes that no longer exist after the 2026-08-02 tracker migration, and finding 1 is an unverified BLOCKER. Needs an issue or a verification pass before it can be archived |
| `.claude/skills/README.md` | Workflow skill library index |

## Archived material

Moved out of the active tree 2026-07-24, git-tracked and reversible (`.archive/README.md` lists
each file and why). Includes: closed FIM experiment plans (Bundle N/P/W/X, DIAG-003), the
March–April design/audit/investigation docs and test-plan snapshots, the pre-existing FIM
convergence archives + March history, and dated review snapshots. Their verdicts remain summarized
in `FIM_EXPERIMENT_REGISTRY.md`; the docs themselves are provenance, not live specs.

- `.archive/docs/` — archived `docs/` files (incl. `REFACTOR_PLAN.md`, historical refactor plan)
- `.archive/docs/TRACKER_MIGRATION_2026-08-02.md` — GitHub Issues migration and initial issue map
- `.archive/PLAN.md` — historical scenario-first rewrite plan (superseded by landed work)
- `.archive/CODEX_FIM_DIALOGUE_03.07.2026.md` — historical design dialogue

## Current repo-level facts

- `src/lib/catalog/scenarios/` is the primary scenario registry: **17 scenarios are offered in the
  picker**. A further definition, `dep_pvt`, is resolvable and tested but explicitly withheld in
  `scenarios.ts` pending a second sensitivity dimension.
- `ScenarioPicker.svelte` is the only live case-selection surface, driven entirely by
  `scenarios.ts`. The legacy benchmark-family data and Custom Mode's preset/facet entries were
  archived in 2026-07 (`.archive/README.md`); compatibility types and empty stubs remain, but
  production case definitions live only in `src/lib/catalog/scenarios/`.
- Public simulations execute in browser-side WASM through scenario-declared **IMPES or FIM**
  policies. Offline OPM Flow artifacts are precomputed reference data, not live simulation. The
  **8 committed artifacts** are `status: "parsed"` and are rendered only when a scenario declares
  them as a reference source.
- Black-oil and three-phase modes are implemented and exposed. SPE1 has published reference
  overlays, OPM artifacts, and
  quantitative acceptance criteria (`src/lib/ressim/src/tests/spe1_acceptance.rs`, closed 2026-07-24;
  see `docs/BLACK_OIL_VALIDATION.md`).
- Case-library planning is owned by `docs/CASE_LIBRARY_ROADMAP.md` (Tier 7 = the 2026-07-24 gap
  audit; stable case and enabler IDs). `ROADMAP.md` carries strategic ordering and GitHub Issues
  carry execution state; neither should restate the detailed case rationale.

## Maintenance rule

If a document stops describing the current implementation or plan, update it immediately or move it
to `.archive/` (and note it in `.archive/README.md`). Do not leave half-current working documents
in the authoritative set.
