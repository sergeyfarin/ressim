# Compositional well design note

Written **before** the implementation, as C11 of the
[compositional fluid execution plan](COMPOSITIONAL_FLUID_EXECUTION_PLAN_2026-09-14.md) requires.
Owning issue: [#29](https://github.com/sergeyfarin/ressim/issues/29). Results and gate status:
[`COMPOSITIONAL_VALIDATION.md`](COMPOSITIONAL_VALIDATION.md).

## Provenance: what this was derived from, and what it was not

C11's reading list names OPM's `CompWellModel` and `wells/CompWellFlash.hpp`. **Neither is
available in this environment** — they live in `opm-simulators/flowexperimental/comp/`, which
Debian does not package, and `find /usr/include/opm -path '*comp*'` returns no well headers. This
is [plan correction 3](COMPOSITIONAL_FLUID_EXECUTION_PLAN_2026-09-14.md#plan-corrections--2026-09-16).

The plan offers two routes and requires saying which was taken. **This design is derived from
first principles plus the repository's own well geometry.** No OPM well header was read, and
nothing here paraphrases one. Specifically:

- The connection law is Peaceman's, in the form this repository already computes it
  (`well_control.rs::calculate_well_productivity_index`).
- The component source is the phase rate times the upstream phase's molar density and composition,
  which is the same construction C9's face flux uses, applied at a well connection instead of a
  face.
- The injection rule is stated and justified below rather than imported.

**Consequence for validation.** Agreement with OPM cannot be claimed for the well model, because
no OPM well model was consulted. The gates are internal invariants — component-source closure,
AD/FD derivative agreement, and inventory accounting — which is what the plan asks for in the
absence of the external oracle. See also
[`COMPOSITIONAL_VALIDATION.md`](COMPOSITIONAL_VALIDATION.md) §3b: the compositional *trajectory*
oracle is blocked, so nothing here is checkable against a reference simulator either.

## Geometry reuse

The well index is the **geometric** part of the repository's existing productivity index:

```text
WI = DARCY_METRIC_FACTOR * 2 pi k_avg h / (ln(r_eq / r_w) + skin)      [m3.cP/(day.bar)]
```

`well_control.rs` folds the *total mobility* into its `productivity_index` and writes
`rate = PI * dp`. The compositional model needs per-phase mobilities, so it factors that mobility
back out and keeps only the geometry. The two are the same number with the same Darcy constant;
splitting them is what lets one connection carry two phases with different mobilities.

Physical well identity, cell connection, radius, skin and datum depth are the existing ones. Fluid
splitting is not: that is the new semantics, and it is below.

## Unknowns and equations

A well has one or more completions sharing a **single BHP**, each with its own connected cell,
well index and head offset. Multiple completions landed in their own commit, as the plan requires.

**Crossflow is explicitly rejected, not clipped**, which is one of the two options the plan
permits and the one it insists must not be silent. A producer whose connection would take fluid
*from* the wellbore, or an injector whose connection would draw formation fluid *into* it, returns
a typed `Crossflow` error naming the completion, the cell and the potential.

The reason for rejecting rather than implementing is that crossflow is the one thing in this model
that genuinely needs a wellbore state. Without it a producer's stream is the sum of its
connections' and an injector's is prescribed, so there is nothing to mix; with it, the fluid
re-entering the formation is the wellbore mixture, which depends on every other connection — a
coupled wellbore equation that would have to be posed, solved and validated, and which nothing in
V1's scope needs.

A **single-completion** producer at or above its cell pressure is a different case and is shut in
rather than rejected: there is no other connection for wellbore fluid to have come from, so
nothing is being silently invented. The distinction between those two cases is the whole content
of the policy.

| Control | Unknowns | Equations |
| --- | --- | --- |
| BHP | none; `bhp` is data | the `N` component sources enter the cell's balances |
| Molar rate | `bhp` | one rate equation: total component molar rate equals the target |
| Surface volumetric rate | `bhp` | one rate equation: surface volume from C5's flash equals the target |

Under BHP control the well adds no unknown: it is a pure source term on the cell block, and the
`N x 1` derivative with respect to `bhp` is still produced, because rate control needs it and a
derivative that only exists in one code path is a derivative that disagrees with itself.

### C14: the rate row is part of the Newton system

**Decision: the rate constraint is an equation with the BHP as its unknown, solved by local Schur
elimination** — the OPM `StandardWell` structure — rather than a bisection outside the Jacobian
loop. The reason is the C14 measurement itself: a single-cell, three-report-step case took 17 s in
release because a 41-step BHP bisection ran inside every Jacobian evaluation. Moving the same
bisection outside the loop would still leave the BHP frozen within a Newton iteration, so the
Jacobian would omit the well's strongest feedback and the constraint would be satisfied only by
an outer fixed point. The equation is both smaller and the formulation OPM uses.

For one rate-controlled well the system is

```text
[ J_cc  J_cw ] [ dx_c ]   [ -R_c ]
[ J_wc  J_ww ] [ dx_w ] = [ -R_w ]      J_ww is scalar and diagonal across wells
```

`J_cw = -dt * d(source_i)/d(bhp)` already existed on every `WellSource`; `J_wc` is
`d(achieved rate)/d(cell primaries)` and `J_ww = d(achieved rate)/d(bhp)`. The assembler
eliminates the BHP locally,

```text
(J_cc - J_cw J_ww^-1 J_wc) dx_c = -R_c + J_cw J_ww^-1 R_w
dx_w = J_ww^-1 (-R_w - J_wc dx_c)
```

so the linear solve and CPR still see the same cell-block shape they saw before. The surface-rate
row's derivative is analytic for the single-stage separation every pinned fluid uses: it chains
the existing `flash_derivatives` at surface conditions through `d(beta)/d(z)` and
`d(rho)/d(z)`. A liquid-cascade train, whose derivative only affects Newton's rate, uses a
finite difference of the same public `surface_separation`.

Cost measured on this commit (release, one core, four runs): **three 0.25 d single-cell report
steps cost 1.23–1.47× the BHP-controlled case** (`comp_cost_rate_controlled_against_bhp_controlled`);
the same ORAT seven-step rate-controlled depletion fell from **3.5 s to 0.13 s** after the change.
The
remaining factor is one limit-capacity check per assembly, which control selection needs.

### Control switching and hysteresis

A rate target near what the BHP limit can deliver is switched to BHP control at the limit. The
margin is the black-oil engine's (`well_control.rs`): the limit must deliver the target by **5% of
the target** before the rate equation takes over again. Without the deadband a well sitting exactly
at capacity flips mode on every Newton iterate. This is stateless, so a retry and a fresh run make
the same decision from the same state. The switch is per assembly, and `WellResult.on_bhp_limit`
reports the active mode; a BHP-limited well contributes no rate row and no BHP unknown.

## Connection law and sign convention

```text
p_conn   = bhp + head_offset                                [bar]
dp       = p_cell - p_conn                                  [bar]
```

**Positive `source_i` means moles entering the cell.** A producer therefore has negative sources
and an injector positive ones, matching C8's residual `R_i = n_i(new) - n_i(prev) - dt source_i`.

### Producer

Draws the cell's own fluid, so the connection is upstream of the cell in the same sense C9's face
flux means it:

```text
q_P      = WI * (kr_P / mu_P)|cell * dp                     [m3/day], for each phase present
source_i = -sum_P q_P * c_P|cell * x_(P,i)|cell             [mol/day]
```

An absent phase contributes no term at all — not a zero mobility multiplied by an undefined
density. A producer with `dp <= 0` is flowing backwards, which for one completion is not
production; the rate is held at zero rather than allowed to become injection of whatever the
wellbore is assumed to contain, because no wellbore composition has been specified.

### Injector

The plan warns specifically that "merely using kr of an absent injected phase can incorrectly
eliminate injectivity" — if the injected stream is a vapour and the cell holds only liquid, the
vapour relative permeability at the cell's saturation is zero, and a naive rule would make the
well unable to inject at all.

**The rule adopted here: the prescribed stream, moving at the perforation cell's total
mobility.** Flash `z_inj` at `p_conn` for what goes in; flash the cell's own `z` at its own
pressure for the resistance it goes through.

```text
lambda_cell = sum_P (kr_P(S_P^cell) / mu_P^cell)            [1/cP]
q_total     = WI * lambda_cell * (-dp)                      [m3/day], positive into the cell
source_i    = q_total * c_mixture^inj * z_(inj,i)           [mol/day]
```

The plan's failure mode is avoided because the mobility is a **sum over the phases the cell
actually holds**, not the injected phase's relative permeability evaluated at the cell's
saturation. A vapour injector into a single-phase liquid cell still injects, at the liquid's
mobility, which is the physically right answer rather than a special case.

The injected **composition** is prescribed and is what enters the cell exactly — `sum_i source_i`
distributed in the proportions `z_inj`. No flash of the cell's fluid enters the injected stream's
composition. That, and not independence of the rate, is the property that makes injection
auditable: the mobility is one scalar multiplying every component, so it changes how much enters
and never what enters.

#### This rule replaced an earlier one, and C12 is why

The first implementation used the **injected stream's own** total mobility, on the reasoning that
its saturations sum to one and so no phase is evaluated at a saturation belonging to a different
fluid. That reasoning is sound as far as it goes, and the rule is a defensible modelling choice —
it says the near-wellbore region is already occupied by the injected fluid. It is also wrong in
exactly the regime it was meant to cover.

C12's comparison against `flowexp_comp` measured the cost. While the perforation cell is still
unswept, pure supercritical CO2 is about four times more mobile than the mixture it is displacing,
so injecting at the stream's own mobility over-injects by roughly that factor until the cell is
swept. On `1D_COMP` it showed as a 27% excess across the interval where the injector hands over
from rate control to BHP control, decaying to about 3% once cell 1 had become essentially pure
CO2. The cell-mobility rule is also what OPM's `StandardWell` does, so this is not ResSim choosing
between two defensible conventions — it is ResSim agreeing with the reference.

See `docs/COMPOSITIONAL_VALIDATION.md`'s C12 record for the measurement.

### Wellbore flash

The formulation above needs **no** wellbore flash. The injector's stream composition is prescribed
data, and the producer's is the sum of its connections', so there is no wellbore state variable
whose equilibrium has to be solved. The plan asks whether the selected formulation requires one
and, if so, that it be solved coherently rather than half-ported; the answer here is that it does
not, and the reason is the crossflow policy above.

Summing mole rates across completions is all the mixing a producing wellbore needs, and the
surface separation flashes that sum. What would require a wellbore equilibrium is a mixture
flowing back *into* the formation — which is precisely the case that is rejected.

## Controls

1. **BHP.** Data. The source follows directly.
2. **Total molar rate**, `mol/day`, signed: positive injects. The rate equation and the BHP
   unknown are part of the Newton system (above). If the limit binds, the well reverts to BHP
   control at the limit and reports the achieved rate rather than the target. A target that
   cannot be reached at any admissible BHP is therefore reported as the limit's achieved rate,
   not silently truncated. `well_source` remains a direct scalar-solve convenience for callers
   that ask one well for a self-consistent source; the timestep driver does not use it.
3. **Surface volumetric rate**, `m3/day` at the pinned surface conditions, through C5's
   `surface_separation`. **This is not a reservoir rate and not a black-oil RESV target**: the
   conversion is a single-stage equilibrium flash at 1 atm and 288.71 K, and the two differ by the
   formation volume factor of a fluid whose composition is changing.

Every control carries its units in the API and in its tests, because "rate" is three different
quantities here.

## Schedules (#50, shared with #17)

A case carries an absolute-time `schedule` of well events. The envelope is engine-neutral
(`src/lib/ressim/src/schedule.rs`): **a time, a well id, and a flattened change**. The
compositional half of the change is a control and/or an injected stream; a black-oil event will
carry its own control type in the same envelope (#17), so scenario code sees one schedule contract
rather than two.

```json
{
  "time_days": 1.0,
  "well": "INJ",
  "control": { "control": "bhp", "target_bar": 250.0 },
  "injection": { "injection": "hydrocarbon", "composition": [1.0, 0.0] }
}
```

- **Exact time.** The engine caps a requested step at the next event, accepts that step, and only
  then applies the change. A well's source over `[t_i, t_i+1)` is therefore governed by the last
  event at or before `t_i`; a switch cannot happen part-way through an accepted step. A caller that
  expects a report at a later time sees the smaller accepted `dt` and continues on the remainder —
  the worker session already steps on the remainder rather than counting engine calls as report
  steps.
- **Short boundary intervals are attempted.** An event at day 1.0000005 after a daily report step
  leaves a 5e-7 d interval, below the normal 1e-6 d minimum timestep. The schedule cap lowers the
  retry floor for that explicitly requested interval, so Newton runs; refusing it would end the
  run with `Budget` before any solve. The symmetric case is an event at day 0.9999995: the 5e-7 d
  report remainder *after* the event is not capped by anything, so an explicit "this state is on
  an event boundary" flag grants the same allowance for that one remainder. A caller's own tiny
  `dt` away from a boundary is still governed by the configured minimum.
- **Order is data, not a sort.** Events must be in nondecreasing time order and a well may not have
  two changes at any one instant — including when another well's event sits between them. An
  ambiguous schedule is refused rather than silently reordered, because two readings would
  otherwise give two answers.
- **The schema is the capability guard.** A scheduled case uses
  `ressim-compositional-case/2`, not the unscheduled `/1`. A build that predates schedules refuses
  `/2` as an unsupported schema instead of deserializing the payload, ignoring its events and
  running a different physical case. The checkpoint envelope stays `/1`, but its embedded `config`
  carries `/2`, so an older build rejects a scheduled checkpoint through that config.
- **`t = 0` is the initial condition.** Events at zero are applied when the case is built, so the
  base control need not be repeated as a first no-op event.
- **Shut-in is a control.** `{"control":"shut-in"}` zeroes the source, keeps the completions, and
  keeps the well's row in the cumulative ledger so later events can reopen it. A shut-in well
  reports a flowing BHP of zero, and a well reopened by an event keeps zero until its first
  accepted step supplies a solved value; every well that has flowed under its current control
  reports the pressure its accepted solve ran at.
- **Restart carries the cursor.** A checkpoint stores `schedule_position` — the number of events
  applied — because two events at one time for different wells are not recoverable from the state
  alone, and `schedule_boundary_pending` — whether the next step is the remainder after an event.
  The latter is execution state: at day 1000 a consumed 5e-7 d remainder is smaller than any
  practical clock-roundoff tolerance, so inferring it would regenerate an allowance the original
  run had used. A checkpoint from before the flag existed is inferred with a roundoff-scale
  `event_at(checkpoint.time, last event)` test (`1e-11 * time`), not the old `1e-9 * time` band.
  Restore still requires the stored cursor to agree. Checkpoints written before schedules existed
  are replayed by time instead of refused.
- **One event can change both halves.** A WAG switch changes the control target and the injected
  stream together, so there is no instant at which the well is injecting the old fluid at the new
  target (or the new fluid at the old one).

The gate is `comp_schedule_wag_cycle_switches_exactly_in_time_and_conserves_mass`: five cells, a
pinned binary, immiscible water and one producer; the injector alternates water → gas → water → gas,
shuts in, and reopens on integer days. Each step stops exactly on its event without a retry, and
after every switch the inventory change equals the summed per-well cumulative row, water included.

Out of scope here: black-oil consumption of the envelope (#17 owns the wiring and E2), relative
permeability hysteresis (E4), and an OPM `SCHEDULE` deck parser. The gate is an internal
conservation/timing contract, not an OPM trajectory comparison.

## What the gates are

Internal, since no external well oracle exists:

- **Component-source closure.** What the well removes from the cell equals what appears in the
  well's own accounting, per component and exactly.
- **Derivatives.** The `N x N` cell block and the `N x 1` BHP column against finite differences.
- **Inventory.** Over several accepted steps, the grid's loss equals the well's cumulative
  production, which is C10's closure test extended to a real well rather than an imposed source.
- **Injection composition.** The composition entering the cell is the prescribed one, not the
  cell's.
- **Injectivity.** An injector delivering a vapour into a liquid-filled cell still injects — the
  specific failure the plan names.
- **Control switching.** A rate target that exceeds what the BHP limit allows reverts to the limit,
  and the reported rate is the achieved one. The 5% hysteresis band is tested in both directions.
- **Rate-row derivatives.** `d(rate)/d(cell primaries)` and `d(rate)/d(bhp)` against finite
  differences, for molar, surface-liquid/vapour and surface-total targets
  (`comp_well_rate_constraint_jacobian_matches_finite_differences`).
- **Cost.** An active rate control evaluates two well sources per assembly — one limit check and
  one rate evaluation — not a bisection; a BHP-controlled well evaluates one
  (`comp_well_rate_control_uses_two_source_evaluations_not_a_bisection`). The release timing is
  in `COMPOSITIONAL_VALIDATION.md`'s C14 record.

## Explicitly out of scope for this commit

Crossflow (rejected, per the policy above), wellbore friction, multisegment wells, separator
trains beyond C5's single stage, and any claim of OPM parity.
