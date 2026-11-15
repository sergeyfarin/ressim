# Compositional engine

Reference for the `engine-physics-change` skill: read this before changing anything under
`src/lib/ressim/src/compositional/` or `src/lib/ressim/src/fluid/`, or `tools/jutul/`. The
unit system, the `crate::math` rule, the payload-boundary rule and the test-placement rules in
`SKILL.md` apply here unchanged.

## What it is

A separate fully implicit engine for isothermal Peng–Robinson compositional flow. It shares no
solver code with black-oil FIM; the black-oil gates (`validate-solver-coverage.sh`) do not select
any of its tests.

| Where | What |
|---|---|
| `fluid/` | Thermodynamics only: `specification.rs`, `units.rs`, `pinned.rs` (the pinned fluids), `eos.rs`, `stability.rs`, `flash.rs`, `derivatives.rs`, `transport.rs` |
| `compositional/` | The simulator: `layout.rs`, `state.rs`, `accumulation.rs`, `flux.rs`, `assembly.rs`, `block_matrix.rs`, `linear.rs`, `newton.rs`, `timestep.rs`, `wells.rs`, `water.rs`, `relperm.rs` |
| `compositional/api.rs`, `compositional/frontend.rs` | Its payload boundary and wasm shim, the pattern `api.rs` / `frontend.rs` follow for black-oil |
| `schedule.rs` (crate root) | The well-event envelope both engines consume |
| `tools/opm_compositional/`, `opm/compositional/` | The `flowexp_comp` oracle: fixtures and how they are built |
| `tools/jutul/` | JutulDarcy references and the SPE5 diagnostic tools; its tests run in PR CI |

Tests sit beside the code as `*_tests.rs` and are named `comp_<area>_*` (`comp_flash_*`,
`comp_well_*`, `comp_spe5_*`). The gate selects them by that prefix, so keep it.

## Governing documents

- `docs/COMPOSITIONAL_VALIDATION.md` owns the scope, the pinned fluid, the oracles, the numerical
  acceptance contract (§5) and the gate status (§7). Read §5 before changing a tolerance or an
  acceptance band; the benchmark-tolerance rule in `AGENTS.md` covers them.
- `docs/COMPOSITIONAL_C12_FORENSICS.md` §6 owns the rules for comparing against a reference.
  **Read it before any comparison with `flowexp_comp` or JutulDarcy.**
- `docs/SPE5_VALIDATION.md` and `docs/SPE3_VALIDATION.md` own those cases: definition, what the
  case is and is not, reproducer commands, and the dated experiment notes they link.
- `docs/COMPOSITIONAL_WELL_DESIGN.md` owns the well unknowns, connection law and controls.

## Rules

- **Never compare a converged solution against an unconverged one.** Match the discretisation on
  both sides or refine both. Check the reference's own timestep convergence first, and refine
  twice: once cannot tell converging from diverging. No rate-controlled `flowexp_comp` fixture
  converges in the timestep, which is an oracle limit and not a ResSim defect.
- Evaluate a rate where the scheme evaluates it. An instantaneous rate and a
  `Δinventory/Δt` from an implicit step are different quantities.
- A term-by-term agreement with a product disagreement means the comparison is wrong, not the
  terms.
- An experiment that changes acceptance, a tolerance or a phase decision stays test-only until the
  owner approves it. Record its verdict in the owning case document with the same words the FIM
  work uses (`INCONCLUSIVE` is not `REFUTED`; see the `fim-solver-debug` skill).
- `comp_spe5_wag` and `comp_spe3_gas_cycling` are in `WITHHELD_SCENARIOS`
  (`src/lib/catalog/scenarios.ts`) until their benchmark comparisons exist. Do not offer them in
  the picker as a side effect of other work.

## Gates

```bash
bash scripts/validate-compositional.sh thermo     # standalone thermodynamics, ~20 s; what PR CI runs
bash scripts/validate-compositional.sh records    # record comparison against committed fixtures; no OPM needed
bash scripts/validate-compositional.sh native     # thermo + fixture + reference; needs OPM headers and flowexp_comp
bash scripts/validate-compositional.sh all        # native + wasm compile check
bash scripts/validate-compositional.sh refinement # C12 timestep and grid refinement, release, ~3 min
```

The header of `scripts/validate-compositional.sh` describes every mode. Run `all` before touching
the well model, the flux or the timestep lifecycle. The long SPE5 and SPE3 replays are
`#[ignore]`d and run with `--release ... -- --ignored`; the exact commands are under *Reproducer
and gates* in `docs/SPE5_VALIDATION.md`. A change that touches code shared with black-oil
(`schedule.rs`, `solvers/`, `math.rs`, `ad.rs`) also runs the black-oil gates from the
`ressim-validation` skill.
