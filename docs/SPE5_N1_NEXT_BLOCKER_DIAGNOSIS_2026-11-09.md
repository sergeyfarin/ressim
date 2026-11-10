# SPE5 N1 next blocker: branch selection, seed loss or cutoff?

**Status: diagnostic, no production behavior change. The next change is not chosen or implemented
here.** This note uses the failing call captured after N1 + difference refinement to separate the
three mechanisms before picking the next candidate. Measured on `2433d31e` (branch
`diag/116-n1-next-blocker`), release build; the seeds, the HP switch point and the diagnostic tests
are committed with it.

## Inputs

The bounded continuation in [SPE5_SPLIT_PHASE_DECISION_N1_2026-11-09.md](SPE5_SPLIT_PHASE_DECISION_N1_2026-11-09.md)
fails at:

| Seed | Day | Failing call `dt` | Previous attempt |
|---|---:|---:|---|
| `stall_n1_6612_836.json` | 6612.836114153414 | 0.04402126141911289 d | N1 + refinement, month 218, 10 attempts |
| `stall_n1_6612_869.json` | 6612.869119352101 | 0.0439567981404 d | same, `SPE5_MAX_ATTEMPTS=12` |

The seeds are committed from the driver's `SPE5_REPLAY_CHECKPOINT_OUT`; provenance is in
`opm/compositional/spe5/README.md`. Both are frozen failing-call states, not validated references.

## What the failing call does

At the first seed, the last two Newton iterates for cells 33/39 are:

| Iterate | Pressure [bar] | Standard flash | N1 flash | `sum ln^2 K` | Notes |
|---|---|---:|---|---:|---|
| 19 | 227.551436370 | TwoPhase, `beta = 0.517570` | TwoPhase | 0.2362 | far below the switch |
| 20 | 228.156518193 | SingleLiquid | SingleLiquid | 488 (Wilson K) | far above the switch |

The residual alternates between `3.328e-3` and `5.041e-3` for iterations 3-20. Both **far**
endpoints are individually reasonable, but they do not bracket the intervening N1 switch.

### The intervening switch is premature

Bisecting the actual 19→20 path with the same problem, previous inventory, BHP interpolation and
N1/refinement settings gives, for cell 33 (cell 39 is analogous):

| Side | Pressure [bar] | Standard flash | N1 flash | Margin seed | Trials |
|---|---:|---|---|---|---|
| lower | 228.12668313699209 | SingleLiquid | TwoPhase, `beta = 0.4601675` | present | Wilson vapour non-trivial |
| upper | 228.1266831369921 | SingleLiquid | SingleLiquid | absent | both trivial |

The assembled scaled residual jumps by `3.3069e-3` across the adjacent inputs (cell 39:
`3.2777e-3`). The 50-digit oracle finds a **physical split on both sides**: `beta = 0.458178303`,
`sum ln^2 K = 1.645e-5`, `g_split - g_feed = -3.144e-15 RT`. The N1 switch at `228.126683` is therefore
premature; the current composition's connected bubble endpoint is near `228.126707 bar`. M1's old
`228.124845 bar` collapse belongs to a different composition and does not apply here.

### Seed / trial-cutoff loss

This is causal at the switch. At the upper input both Michelsen trials are trivial, so N1 loses its
eligible seed and returns the single-phase answer. The physical split's `sum ln^2 K = 1.645e-5` is
just **above** N1's `1e-5` acceptance cutoff, so the cutoff would not reject it if a seed were
available. Seeding from the lower side's split at the upper state converges to `beta = 0.458176`,
`sum ln^2 K = 1.64531e-5`, `g_split - g_feed = -3.77e-15`, a physical split. The blocker is **seed
eligibility/trial classification**, not the acceptance cutoff and not the far liquid endpoint.

### Cutoff

The acceptance cutoff is not the limiter: a carried seed produces `sum ln^2 K = 1.64531e-5`, above
`1e-5`. The **trial** trivial classification (`k_norm < 1e-5` in the stability test) is what removes
the seed. Lowering the acceptance cutoff alone would not help; lowering the trial trivial cutoff
would need its own validation and still leaves the no-seed path unresolved.

### Branch selection and Taylor

The far endpoints are on opposite sides of the physical boundary, but the cycle is not evidence
that the far single-liquid state is wrong: it is evidence that the switch between them happens too
early. The Taylor analysis is now run with refinement and N1 enabled for the attempt, its previous
inventory and its residual/Jacobian evaluations. It remains quadratic within the branch
(`error/h^2` about `1.7e-4`) and breaks at the full step, which crosses the premature switch. That
is consistent with a seed/trial-switch discontinuity; it is not a controller-only boundary.

### N2

Not the first blocker for this call. The far liquid state is bubble-side, where single-liquid
identity is correct. N2's dew-side problem remains a separate candidate for the other crossing
cells.

## Timestep evidence

The same failing-call checkpoint was traced over a dt ladder with N1 + refinement:

| Call | Last failing `dt` | First converging `dt` | Behaviour |
|---|---:|---:|---|
| first seed (`6612.836`) | 0.044021 d | 0.022011 d | 20-iteration 33/39 two-cycle at the failing dt; 4 iterations, `6.45e-10` at half |
| second seed (`6612.869`) | 0.043957 d | 0.000687 d | two-cycle persists through `1.37e-3 d`; 12 iterations, `1.57e-9` at `6.87e-4 d` |

A larger budget alone is not sustained progress: `SPE5_MAX_ATTEMPTS=16` advances seven accepted steps
from the first seed to day `6612.878047`, then fails another month-218 call at `2.74e-7`; the N1
flash-override count rises from 186 to 3,468. A fixed Newton step damping probe reaches `5.20e-5` at
factor 0.5 but still stalls at 20 iterations (and at 40 iterations, `5.20e-5`). An early two-cycle
detector could save Newton work, but it does not address the premature switch itself.

## Decision

The evidence now points to **seed eligibility/continuation at the premature switch**, not a
controller-only fix:

1. **Recommended next:** carry or recover a split seed through the switch. At the upper input a
   non-trivial split exists but both stability trials have collapsed to trivial; use the adjacent
   iterate's/previous flash's `K` (or another validated fallback) to solve the two-phase branch.
   The committed switch regression already demonstrates the contract: the carried seed gives
   `beta = 0.458176`, `sum ln^2 K = 1.64531e-5`, passing the existing `1e-5` cutoff and Gibbs guard
   without changing residual acceptance.
2. **Evaluate separately:** a phase-boundary-aware timestep cutback / early two-cycle detector for
   cost and minimum-step behaviour. The budget-16 run shows timesteps collapsing; a faster replay
   of the same ladder is not sustained progress.
3. **Not next for this call:** N2 (far liquid identity is correct), acceptance-cutoff lowering
   (carried split passes at `1.645e-5`), and blanket damping (insufficient).
4. The immediate causal experiment is the premature switch, not the far endpoints. Keep the far
   endpoint labels and the new seeds as regressions.

## Reproduce

```bash
# actual-path switch, trial sums, carried seed and residual jump
cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib \
    comp_spe5_n1_next_call_switch_probe -- --ignored --nocapture

# live-state flash/stability and Taylor under the same N1+refinement operator
cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib \
    comp_spe5_n1_next_call_phase_diagnostics -- --ignored --nocapture
cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib \
    comp_spe5_n1_next_call_step_damping_probe -- --ignored --nocapture

# 50-digit split at the upper switch input
uv run --no-project --with mpmath python -I tools/jutul/spe5_m0_gibbs_mp.py \
    opm/compositional/spe5/n1_switch_6612_cell33.json DIR/n1_switch_mp.json

SPE5_TRACE_SEED=opm/compositional/spe5/stall_n1_6612_836.json \
    SPE5_FLASH_REFINE=1 SPE5_N1=1 SPE5_TRACE_HALVINGS=12 \
    cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib \
    comp_spe5_trace_seed_ladder -- --ignored --nocapture
```

The first two tests are ignored diagnostics and change no production path; the Python command uses
the committed switch point.
