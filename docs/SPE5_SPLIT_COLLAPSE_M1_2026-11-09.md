# SPE5 split collapse at the #116 crossings: M1

**Status: diagnostic result. N1 as scoped is not sufficient, and neither N1 nor N2 is approved.
Production acceptance is unchanged at `1e-8`; no solver behavior changed.**

M1 follows the physical two-phase split found in M0 in 50-digit arithmetic, from the label switch
to its actual collapse and through the production trivial-solution cutoff
(`sum ln^2 K < 1e-5`). It then asks the engine what would change if a split-based phase decision
(N1) returned that split where the standard flash returns a single phase. The answer depends on
which side of the phase envelope the split collapses:

- **Cells 25/31 and 16 collapse at the dew point** (`beta -> 1`). For 25/31 the composition
  contrast is finite (`sum ln^2 K = 3.1e-5` at the endpoint), so the trivial cutoff is **not**
  reached. N1 moves the switch to the dew point, but the remaining residual jump is the
  single-phase identity: Li's rule labels the vapour-side single phase **liquid**. That is N2's
  question, not N1's.
- **Cells 33/39 collapse at the bubble side** (`beta -> 0`). The trivial cutoff fires first, at
  `beta = 0.3929`, far from the collapse. N1 would therefore return a two-phase state with a
  vapour saturation of 0.393 where the standard flash returns single liquid. The jump is smaller
  than at the original switch, but it is **moved, not removed**; the cutoff is the limiter.

## Provenance and method

The measurement was run on `08f90e34` (branch `docs/116-m1-split-collapse`), release build, with
the five committed M0 crossing seeds. The M0 branch-comparison test was rerun first, then:

```bash
SPE5_TRACE_OUT=DIR cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib \
    comp_spe5_m0_branch_comparison -- --ignored --nocapture
uv run --no-project --with mpmath python -I tools/jutul/spe5_m1_split_mp.py \
    DIR/m0_gibbs_points.json DIR/m1_split_mp.json
SPE5_M1_STATES=DIR/m1_split_mp.json SPE5_TRACE_OUT=DIR \
    cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib \
    comp_spe5_m1_cutoff_jump -- --ignored --nocapture
```

**High-precision continuation (`tools/jutul/spe5_m1_split_mp.py`).** The input is the five
`switch -1e-15` points of `m0_gibbs_points.json`: the two-phase side of each adjacent-float label
change. The script continues the split in 50-digit arithmetic along a **fixed-composition pressure
path**, in the direction that raises pressure towards the single-phase side. The equilibrium
equations are solved for `(ln K, p)` at fixed `beta`; near these states the small singular value of
the standard `(ln K, beta)` system is almost pure `beta`, and fixing it keeps the branch solvable
through the region where the reservoir flash stalls.

At every step the script records `beta`, `sum ln^2 K`, `g_split - g_feed` against the lower feed
root, phase molar volumes and densities, the mixture molar volume, the vapour saturation and the
implied component inventory per m3 of pore volume. It locates:

- the **trivial cutoff** `sum ln^2 K = 1e-5`, by bisection in `beta`, when the branch crosses it;
- the **collapse**, the endpoint where `beta` reaches 0 or 1 and the phase amount vanishes.

The pressure path is the relevant one: across the M0 Newton path the composition changes by
`<= 1e-14`, and re-solving the split with the two endpoints' compositions moves the fixed-beta
pressure by `<= 0.1 Pa`, against the 29-49 Pa distances measured below.

**Engine-side jump (`comp_spe5_m1_cutoff_jump`).** Each 50-digit state is rebuilt through ResSim's
own EOS (`two_phase_state_from_ln_k_for_test`, test-only), and compared with the standard flash at
the same `(p, z)`. The test reports the cell's component inventory per pore volume, every incident
face's component molar flux with the neighbour held at the seed state, and the assembled scaled
residual. This is the same branch-comparison instrument as M0, now evaluated at the collapse and
cutoff states rather than at the switch.

## Result

High-precision endpoints. `Delta p` is measured from the M0 switch point.

| Seed | Cell | Switch `beta` / `sum ln^2 K` | Trivial cutoff | Collapse | `Delta p` | Vapour saturation at collapse/cutoff |
|---|---|---|---|---|---|---|
| `cap5` | 25, 31 | 0.7309 / 1.45e-4 | none | dew, `beta -> 1`, `sum ln^2 K = 3.1e-5` | 29.3 Pa | 1 |
| `cap5_att12` | 25, 31 | 0.7292 / 1.45e-4 | none | dew, `beta -> 1`, `sum ln^2 K = 3.0e-5` | 29.4 Pa | 1 |
| `cap05` | 16 | 1 - 2.8e-9 / 0.791 | none | dew, `beta -> 1`, `sum ln^2 K = 0.791` | 0.003 Pa | 1 |
| `cap05_att12` | 16 | 1 - 2.8e-9 / 0.791 | none | dew, `beta -> 1`, `sum ln^2 K = 0.791` | 0.003 Pa | 1 |
| `memory` | 33, 39 | 0.4767 / 2.01e-4 | `beta = 0.3929`, `sum ln^2 K = 1e-5` | bubble side, `beta -> 0` | 46.8 Pa to cutoff; 49.1 Pa to collapse | 0.393 at cutoff |

Engine-side jump. "Switch" is the M0 fixed-input branch comparison; "event" is the collapse (25/31,
16) or the trivial cutoff (33/39). Inventory is the largest relative component difference; face
flux is the largest relative component change on any incident face.

| Seed | Cell | Scaled residual at switch | Scaled residual at event | Inventory at event | Face flux at event |
|---|---|---:|---:|---:|---:|
| `cap5` | 25 | 8.4e-5 | 9.0e-6 | 6e-16 | 6.5% |
| `cap5_att12` | 25/31 | 1.1e-5 | 1.1e-6 | <= 2e-16 | 6.5% |
| `cap05` | 16 | 1.5e-6 | 1.5e-6 | 1e-13 | 10.4% |
| `cap05_att12` | 16 | 1.9e-7 | 1.9e-7 | 1e-13 | 10.4% |
| `memory` | 33/39 | 8.9e-7 | 2.9e-7 | 2.8e-8 | 77% |

The standard flash at each event is **single liquid** in every case. At the dew endpoints the
mixture's single cubic root is the same on both branches, so the inventory difference is at
round-off (<= 1e-13); the flux difference is the relative permeability and viscosity attached to
Li's liquid label. At the memory cutoff the split is still a genuine two-phase state
(`S_v = 0.393`), so the flux changes are large even though the compositions are close
(`sum ln^2 K = 1e-5`).

## Verdict

- **N1 alone only moves the jump.** It lowers the scaled residual at the switch by about 9x for
  `cap5` and about 3x for `memory`, but leaves 1e-6 to 1e-5 at the event, far above `1e-8`.
- **Dew-side cells need N2.** For 25/31 and 16 the split collapses smoothly to all vapour; the
  remaining 6.5-10.4% flux jump is entirely the single-phase identity. N1 without N2 cannot remove
  it. N1 by itself also does nothing measurable at cell 16 (0.003 Pa move).
- **The memory cells need a cutoff decision, not just a phase decision.** Their physical split
  continues past the production trivial cutoff to `beta -> 0`; the cutoff stops it at
  `beta = 0.3929`. Following the split further would make the phase amount vanish continuously, but
  that is a change to the trivial-solution termination, not N1. Lowering the threshold alone is not
  enough: the cutoff is crossed while the split is still physically distinct.
- **Acceptance stays at `1e-8`.** No held-label flux or relaxed acceptance is involved; N1/N2 are
  changes to the standard flash's phase decision, and this measurement does not approve either.

## Limits

- The path is at fixed composition, not the exact M0 Newton path; the composition sensitivity
  above bounds that difference at 0.1 Pa.
- The continuation follows the branch connected to the M0 split. Other split roots may exist near
  these states; the measurement does not claim the branch is unique.
- At the `memory` collapse (`beta ~ 1e-11`) the engine's equilibrium Jacobian is singular, so the
  assembled residual and face flux are not available there. They are available at the cutoff,
  which is the decision point.
- The M0 `switch -1e-15` state is an adjacent-float bracket; a small fixed-beta refinement can move
  its pressure by a fraction of a pascal and put the engine's standard flash on either side of the
  label change. The event distances above are measured from that refined switch point.
- Nothing here establishes that N2 removes the dew-side jump. That is the next design and
  measurement.

## Next

1. **N2, designed separately:** a deterministic single-phase identity at the dew point, reproducible
   from cold inputs, using the collapsing split (`beta -> 1` means vapour) rather than Li's
   `T_pc` rule.
2. **The memory cutoff, designed separately:** decide whether the standard flash should follow a
   physical split below `sum ln^2 K = 1e-5` until the phase amount is negligible, and how that
   interacts with the trivial-solution guard. N1 does not cover this.
3. Re-run the from-scratch month-240 replay and the #98 probe only after a candidate is approved;
   acceptance remains the original equations at `1e-8`.
