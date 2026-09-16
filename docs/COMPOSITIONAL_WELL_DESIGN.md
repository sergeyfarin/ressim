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

**The rule adopted here: the injected stream's own total mobility, at connection conditions.**
Flash the prescribed composition `z_inj` at `p_conn` and the reservoir temperature, then

```text
lambda_inj = sum_P (kr_P(S_P^inj) / mu_P^inj)               [1/cP]
q_total    = WI * lambda_inj * (-dp)                        [m3/day], positive into the cell
source_i   = q_total * c_mixture^inj * z_(inj,i)            [mol/day]
```

The injected fluid's own saturations sum to one, so no phase is being evaluated at a saturation
belonging to a different fluid and the injectivity cannot vanish for the wrong reason. It is a
**modelling choice**, made explicitly: it says the near-wellbore region is occupied by the
injected fluid, which is the usual assumption for an injector at steady injection and is wrong
during the first moments after startup.

The injected composition is prescribed and is what enters the cell exactly — `sum_i source_i`
distributed in the proportions `z_inj`. No flash of the *cell's* fluid enters the injected stream's
composition, which is the property that makes injection auditable.

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
2. **Total molar rate**, `mol/day`, signed: positive injects. Solved for the `bhp` that achieves
   it, by a bracketed scalar solve on a monotone function of `bhp`, with a BHP limit applied
   afterwards — if the limit binds, the well reverts to BHP control at the limit and the achieved
   rate is reported rather than the target. A target that cannot be reached at any admissible BHP
   is reported as such, not silently truncated.
3. **Surface volumetric rate**, `m3/day` at the pinned surface conditions, through C5's
   `surface_separation`. **This is not a reservoir rate and not a black-oil RESV target**: the
   conversion is a single-stage equilibrium flash at 1 atm and 288.71 K, and the two differ by the
   formation volume factor of a fluid whose composition is changing.

Every control carries its units in the API and in its tests, because "rate" is three different
quantities here.

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
  and the reported rate is the achieved one.

## Explicitly out of scope for this commit

Crossflow (rejected, per the policy above), wellbore friction, multisegment wells, separator
trains beyond C5's single stage, and any claim of OPM parity.
