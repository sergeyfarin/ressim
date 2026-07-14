# Y2b3 — OPM Primary-Variable Lifecycle and ResSim Dependency Design

Status: **Y2b3c Gate C green; Y2b is a promotion candidate and Y2c is next (2026-07-14)**

This document closes the two prerequisites created by Y2b2c:

1. map the OPM Flow phase-presence/primary-variable lifecycle used by the tracked
   `gas-rate-10x10x3` deck; and
2. define how ResSim must preserve a live dependency for every fixed-layout cell unknown before
   another raw-state behavior run.

It began as a source/design checkpoint rather than convergence evidence. Gates A-C in §6 now
pass, and the first-rung result makes Y2b a promotion candidate; Y2c owns the remaining complete
target and non-regression decision.

## 1. Scope and exclusions

The tracked deck enables `OIL`, `WATER`, `GAS`, and `DISGAS`. It does not enable `VAPOIL`.
Therefore the first coherent behavior bundle covers only the gas composition-switch slot with
meanings `Sg` and `Rs`. OPM's `Rv` route, vaporized water, dissolved gas in water, solvent, salt,
and one-phase special cases are not part of this deck-scoped probe. They must remain guarded as
unsupported, not silently approximated.

The bundle must not change well unknowns/equations, CPR or direct solvers, Newton acceptance,
timestep control, nonlinear tolerances, or Legacy behavior. Sparse LU remains an oracle; adding a
diagonal to hide an empty column is forbidden.

## 2. OPM lifecycle map for the tracked deck

OPM does not add and remove algebraic columns as gas appears or disappears. Each cell retains one
composition-switch slot, and a per-cell tag determines whether that slot means free-gas saturation
`Sg` or dissolved-gas ratio `Rs`.

| Lifecycle stage | OPM behavior relevant to this deck | Source |
| --- | --- | --- |
| Read current meanings | Saturation increments are derived only for slots currently tagged as saturations. The composition increment contributes to the common saturation chop only when its current meaning is `Sg`. | `OPM/opm-simulators/opm/models/blackoil/blackoilnewtonmethod.hpp:231-268` |
| Apply Newton update | Pressure has a relative chop; `Sw` and `Sg` use the common `dsMax` saturation factor. When the composition slot means `Rs`, its update is not saturation-chopped; it is limited only to prevent a negative ratio. | `blackoilnewtonmethod.hpp:270-309` |
| Adapt candidate meaning | After the update, every candidate cell calls `adaptPrimaryVariables`. A candidate `Sg < -eps`, with oil present and `DISGAS` enabled, switches to `Rs` and initializes the slot to `min(RsMax, RsSat)`. A candidate `Rs > min(RsMax, RsSat*(1+eps))` switches to `Sg` and initializes `Sg=0`. | `OPM/opm-models/opm/models/blackoil/blackoilprimaryvariables.hh:774-858` |
| Prevent immediate switch-back | If the cell switched on the preceding update, the next adaptation call uses `priVarOscilationThreshold` as `eps`; otherwise it uses zero. The switched state is remembered per cell. | `blackoilnewtonmethod.hpp:436-455` |
| Store candidate state | Saturation normalization/projection occurs only when `projectSaturations` is enabled. It is disabled by default and is not enabled by the tracked deck, so raw endpoint-crossing saturations remain stored. | `blackoilnewtonmethod.hpp:456-457`; `blackoilnewtonmethodparams.hpp:37-42` |
| Interpret the next iterate | The intensive-quantity path reads `Sg` from the composition slot only when the tag is `Sg`; otherwise free-gas saturation is zero for the `Rs` state. `So=1-Sw-Sg`, and these raw saturations populate the fluid state used by residual assembly. | `OPM/opm-models/opm/models/blackoil/blackoilintensivequantities.hh:216-254` |
| Evaluate endpoint properties | Material laws protect relative permeability and capillary-pressure evaluation at table endpoints independently of the stored raw primary state. They do not replace the stored Newton state with endpoint-clamped saturations. | `OPM/opm-common/opm/material/fluidmatrixinteractions/EclDefaultMaterial.hpp:375-424`; `PiecewiseLinearTwoPhaseMaterial.hpp:232-255` |
| Assemble accumulation | The current Newton fluid state is built from the candidate's adapted meaning; the previous time level remains the accepted previous state. Thus changing the current tag changes which physical quantity the fixed slot contributes without retagging history. | `blackoilintensivequantities.hh:216-254`; update order above |

The dependency-critical ordering is:

```text
current tagged variables
  -> meaning-aware Newton chop/update
  -> adapt candidate meaning and value
  -> optional projection (off for this deck)
  -> meaning-aware fluid state, properties, accumulation, fluxes, and wells
  -> next Jacobian
```

Porting raw saturation retention without the adaptation step is not an OPM lifecycle port.

## 3. Current ResSim mapping and the failure mechanism

ResSim already has the correct high-level storage shape: `FimCellState` contains
`[pressure_bar, sw, hydrocarbon_var]` plus `HydrocarbonState::{Saturated, Undersaturated}`. The
third slot means `Sg` in `Saturated` cells and `Rs` in `Undersaturated` cells
(`fim/state.rs:29-94`). The AD assembly also keeps a fixed three-equation/three-unknown cell block
and sends both the current and previous tagged states to accumulation
(`fim/assembly_ad.rs:452-475`).

The live update ordering differs:

1. `apply_newton_update_frozen` deliberately retains the old regime tag throughout the Newton
   solve (`fim/state.rs:401-412`).
2. Normal updates then clamp `Sg` or `Rs`; the Y2b2 probe skips stored saturation bounds but still
   freezes the tag (`fim/state.rs:250-290,414-429`).
3. In a frozen `Saturated` cell, both the scalar flash and differentiable property path evaluate
   the gas slot through a zero/total-hydrocarbon clamp (`fim/flash.rs:75-105` and
   `fim/properties.rs:74-82`). Once a raw candidate crosses below zero, the assembled residual and
   Jacobian can become locally independent of the still-labelled `Sg` slot.
4. `classify_regimes` can change `Sg`/`Rs`, but it is not called by the live frozen update; the
   reclassifying path shown at `fim/state.rs:377-398` is test-only/outside the Newton loop.

This code path explains how a fixed, frozen gas slot can become inactive. Y2b2c measured the
resulting algebraic symptom—120 empty local-variable-2 columns—but its matrix capture does not
contain the per-cell tagged state needed to prove the crossing history of each individual column.
The implementation must therefore add the state/switch trace in §6 rather than treating this
source inference as a completed causal replay.

## 4. Parity classification before implementation

| Coupled semantic | Classification | Required disposition |
| --- | --- | --- |
| Fixed per-cell composition-switch slot plus meaning tag | **Matched in representation** | Keep the fixed matrix dimension; do not add/remove cell columns. |
| Meaning-aware `dsMax` update for `Sg`; nonnegative update for `Rs` | **Partially matched** | Verify the existing OPM chop consumes the old tag and add focused transition tests. |
| Raw saturation storage with projection default off | **Diagnostic-only** | Keep the existing native, default-off `OpmAligned` probe until promotion. Never alter Legacy here. |
| Endpoint-clamped material properties separate from raw stored state | **Partial/mismatched** | Preserve endpoint-safe properties, but remove state-flattening clamps from the dependency path used by an active tagged variable. |
| Per-iteration `Sg -> Rs` and `Rs -> Sg` adaptation | **Missing** | Add immediately after the candidate update and before the next assembly. |
| Previous-switch hysteresis | **Missing** | Add per-cell switched memory and the OPM threshold semantics in the same bundle. |
| Current accumulation/flux/well interpretation after switching | **Missing as a coupled lifecycle** | All consumers must derive from the newly adapted tag; no well-row-only patch. |
| Previous accepted time-level state | **Intentionally held constant** | Do not mutate or retag it during Newton iterations. |
| Well unknowns and equations | **Intentionally held constant** | G4 remains blocked. |
| Linear solve, acceptance, tolerances, and timestep controller | **Intentionally held constant** | Preserve the current oracle and controller behavior. |
| `Rv` and other non-deck switch families | **Out of scope and guarded** | Fail explicitly if the deck-scoped probe encounters them. |

## 5. ResSim dependency design

### 5.1 State contract

Keep one fixed cell tuple `[p, Sw, z]` and one tag `meaning(z) in {Sg, Rs}`. The tag and value form
one primary variable; neither may be updated independently. Add per-cell `was_switched` memory for
the immediately preceding Newton update.

The candidate lifecycle must be a single operation:

1. Read the current tag.
2. Apply the existing per-cell OPM pressure/saturation chop using that tag.
3. Store raw candidate `Sw` and `z` under the default-off OPM-parity policy.
4. Adapt the candidate tag/value using the deck-scoped OPM rules:
   - `Sg -> Rs` when raw `Sg < -eps`, oil saturation is positive, and `DISGAS` is enabled; set
     `Rs=min(RsMax,RsSat)`.
   - `Rs -> Sg` when raw `Rs > min(RsMax,RsSat*(1+eps))`; set `Sg=0`.
   - use zero `eps` normally and the OPM oscillation threshold when `was_switched` is true.
5. Assemble the next iterate from the adapted tag. Leave the previous time-level state untouched.

Do not substitute ResSim's inventory-flash reclassification for these initial values in the first
parity probe. OPM explicitly initializes saturated `Rs` on disappearance and zero `Sg` on
appearance; a mass-preserving alternative would be a different mechanism and needs its own
experiment. Conservation is checked through the assembled component residual, not imposed by
silently changing the switch rule.

### 5.2 Residual and property dependency contract

Every active fixed-layout variable must influence at least one assembled equation at the state
where Newton is solved. In particular:

- an `Sg` tag must expose raw `Sg` to accumulation and phase-state construction; endpoint property
  functions may be flat, but the component-storage dependency must remain;
- an `Rs` tag must expose raw `Rs` to dissolved-gas accumulation; it must not be flattened by a
  generic `max(0)` before differentiation; and
- reservoir accumulation, face fluxes, perforation phase properties, and well equations must all
  consume one meaning-aware derived state.

The implementation may use endpoint-safe values inside PVT/material-property functions where OPM
does, but must not clamp the primary variable before its component accumulation dependency is
formed. If a physically unsupported state cannot be interpreted, switch its meaning or reject the
candidate explicitly; never retain a column whose derivative is identically zero.

### 5.3 Structural invariant

Before every diagnostic direct solve, inspect the assembled matrix by variable family. For this
tracked case the required invariant is:

```text
empty cell-primary columns = 0
```

On violation, stop the behavior probe and print, for each empty column: cell index, old/new tag,
raw `z`, derived `Sg`, derived `Rs`, `RsSat`, switch epsilon, switch decision, and nonzero counts by
reservoir/well row family. Do not continue via CPR and call the lifecycle valid; do not patch the
diagonal. The fixed system should remain `3*Ncell + Nwell + Nperf` square throughout a switch.

## 6. Prescriptive implementation and test gates

Implement this as one dependency-complete, native, default-off `OpmAligned` behavior bundle. A
partial port cannot refute the complete lifecycle.

### Gate A — transition unit tests

- positive `Sg` remains `Sg`; the third column is nonempty;
- negative raw `Sg` under `DISGAS` switches to `Rs=min(RsMax,RsSat)`;
- sub-saturated `Rs` remains `Rs` and stays a live accumulation variable;
- over-saturated `Rs` switches to `Sg=0`;
- a preceding switch applies hysteresis and prevents immediate threshold chatter;
- the previous time-level cell is unchanged by current-iterate switching;
- raw `Sw < Swc` remains stored while endpoint properties are finite and component accumulation
  still changes; and
- the deck-scoped path rejects unsupported `Rv`/non-deck meanings explicitly.

**Result (2026-07-14): PASS for the representable deck scope.** The native/default-off
`FIM_Y2B_RAW_SATURATION` path now retains raw saturation and atomically adapts the fixed third
slot after each candidate update, before well post-processing. Five focused tests cover positive
`Sg`, raw `Sw`, previous-state immutability, `Sg -> min(RsMax,RsSat)`, sub-saturated `Rs`,
`Rs -> Sg=0`, and both hysteresis directions. `HydrocarbonState` has only `Saturated` (`Sg`) and
`Undersaturated` (`Rs`) meanings, so `Rv` is unrepresentable rather than silently approximated.

Gate A also exposes a required Gate B case: after a switch, OPM's `eps=1e-5` hysteresis can retain
a slightly negative `Sg` for one iteration. ResSim's current differentiable property path applies
`max_floor(0)` to `Sg`, which can erase that column's derivative. Therefore the state machine is
not yet a dependency-complete behavior probe and must not be run on the convergence case.

### Gate B — derivative and structure tests

- compare AD with a one-sided finite difference within each fixed meaning; never central-difference
  across a semantic switch;
- assemble one-cell `Sg -> Rs` and `Rs -> Sg` fixtures and assert no empty primary columns;
- assemble a mixed-regime multi-cell injector fixture and assert no empty rows/columns, finite
  entries, and direct factorization success; and
- confirm reservoir and well terms use the same adapted meaning by checking their AD/legacy
  parity fixtures.

**Result (2026-07-14): PASS.** Three-phase tagged primaries now remain raw through current-state
phase construction and component accumulation. Endpoint extension remains owned by the existing
relative-permeability/capillary functions; the two-phase and no-PVT bounded paths are unchanged.
The scalar reference flash was changed in lockstep with the AD property path.

The new gates establish the following local contracts:

- within each fixed meaning, the active hydrocarbon accumulation column matches a one-sided
  finite difference for positive, zero, and hysteresis-retained `Sg=-5e-6`, and for sub-saturated
  and just-disappeared-gas `Rs` states;
- both one-cell transition directions assemble a live third column after adaptation and factor
  with the independent diagnostic Sparse LU;
- a three-cell mixed `Sg`/`Rs` gas-injector fixture has finite entries, zero empty rows, zero empty
  columns, and a successful direct factorization; and
- scalar reservoir/well residuals and AD residuals agree on the transition and injector fixtures,
  while the existing full reservoir/well AD-vs-scalar-FD gates remain green.

These tests prove derivative consistency and structural viability only. They do not establish
that the lifecycle improves the tracked nonlinear trajectory, so Y2b remains `INCONCLUSIVE`.

### Gate C — exact first-rung diagnostic

Regenerate the Y2b2 `dt=0.00898425` iteration-1 capture with switch tracing enabled. Require:

1. zero empty cell-primary columns;
2. ordinary Sparse LU and dense LU both return finite corrections;
3. direct and CPR full-system reduction and correction partitions are reported and agree closely
   enough to classify the behavior; and
4. the trace directly ties every previously endangered local-variable-2 column to a valid `Sg`
   or `Rs` dependency after adaptation.

If Gate C is structural-green, rerun only the capped first rung and compare accepted `dt`, retries,
Newton history, CNV/MB/well partitions, and mass balance against the clean baseline. Do not run the
six-step promotion matrix yet.

**Result (2026-07-14): PASS; promotion candidate, not promoted.** The completed lifecycle no
longer cuts down to the historical decision rung: on clean committed `1a6460d`, the capped live
driver accepts the full `0.25` day report step in one substep, 8 reported Newton iterations, and
zero retries. The trace ends at CNV `[6.695e-12,6.711e-4,2.622e-4]`, MB
`[4.597e-12,1.648e-8,6.734e-8]`, with accepted scalar MB `1.683337e-8`. OPM's corresponding
first report step takes 7 Newton iterations with no cut, so this is materially closer but not yet
the six-step promotion result.

A test-only `FIM_Y1J_DT_DAYS=0.00898425` selector cleanly regenerated the historical iteration-1
system without forcing the timestep controller to manufacture a retry. Its 904-by-904 matrix has
6815 nonzeros, zero empty rows/columns, zero non-finite/duplicate/all-zero rows, and zero missing
or zero diagonal candidates. The companion trace covers all 300 cell primaries: 141 are tagged
`Sg`, 159 are tagged `Rs`, the same 159 record a preceding switch, every column is live, and the
minimum local-variable-2 occupancy is 2.

Backend-neutral replay is consistent:

- CPR: reduction `4.911209e-7`, full residual `1.368523e-5`, maximum correction difference from
  Sparse LU `5.557618e-7` by family;
- Sparse LU: reduction `2.837291e-16`, full residual `7.906198e-15`;
- dense LU: reduction `7.935809e-16`, full residual `2.211337e-14`; and
- Sparse/dense correction-family disagreement is at most `7.285839e-15`.

The exact capture is `/tmp/ressim-y2b3c-exact-b/fim_capture_00000.txt`, SHA-256
`13f5f6aa14ae218679b866bb236293801ad81f5d75eba1110b3083f90ea1b61a`; its switch trace is
`/tmp/ressim-y2b3c-exact-b.log`, SHA-256
`3cc23f85789a3be6be64a21b0eb4475265dadf0b3d5bafc6508c9afc12499224`. The post-instrumentation
full-rung trace is byte-identical to the clean-commit trace. Gate C therefore selects the
"structure passes, first rung improves" branch: proceed to Y2c, without opening G4, acceptance,
or direct-solver tuning.

### Decision after Gate C

- **Structural gate fails:** implementation defect or incomplete lifecycle; fix it without a
  convergence verdict.
- **Structure passes, first rung improves:** Y2b becomes a promotion candidate; proceed to Y2c.
- **Structure passes, first rung is neutral/worse:** Y2b is validly refuted for this plateau; use
  the new trace to choose exactly one later branch.
- **Direct/live or diagnostics disagree:** Y2b remains `INCONCLUSIVE`; repair the oracle only.

## 7. Next executable slice

The next slice is **Y2c: bounded promotion matrix** in execution-plan §6. Start by committing this
Gate C diagnostic checkpoint, then reproduce the exact six-step ResSim target on that clean
revision and re-confirm the Flow oracle. Continue through the prescribed heavy/control/physics
gates only if the six-step target remains materially closer. Do not open G4 or tune acceptance,
direct solvers, wells, or timestep control in parallel.
