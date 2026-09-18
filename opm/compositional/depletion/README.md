# Single-cell depletion reference case

C12's **second** fixture. The compositional execution plan asks for several matched fixtures and
names *phase-changing single-cell depletion* among them; `1D_COMP` is a displacement and nothing in
it takes a cell down through its own saturation pressure.

| | |
| --- | --- |
| Deck | `DEPLETION.DATA`, derived from [`OPM/opm-tests`](https://github.com/OPM/opm-tests) `compositional/1D_COMP.DATA` — the PROPS section is that deck's verbatim, © 2024 SINTEF Digital / TNO, Open Database License |
| Simulator | `flowexp_comp`, the same build as the 1D case |
| Regenerate | `bash tools/opm_compositional/run-depletion.sh` |
| Verify unchanged | `bash tools/opm_compositional/run-depletion.sh --check` |
| Consumed by | `comp_depletion_*` in `src/lib/ressim/src/compositional/reference_tests.rs` |

## Two decks

`DEPLETION.DATA` is on a surface **oil rate**; `bhp/DEPLETION.DATA` is the same cell against an
**80 bar BHP**. The second exists because the first turned out to be unusable as a trajectory
oracle — `flowexp_comp` meters 30 sm³/day of surface oil as 3.8% fewer moles than its own flash
says that stream is, so a rate-controlled comparison inherits a discrepancy that is the reference's
and not the model's. Nothing in the BHP deck goes through a surface volume.

They also cover different things. The rate-controlled deck brackets the **phase appearance** (and
then the reference aborts); the BHP deck runs the full twenty days two-phase and is where the
**composition drift** and the **connection rate** are compared.

## The case

One cell, 100 m × 100 m × 10 m, 100 mD, 10% porosity — a 10 000 m³ pore volume. Initially
single-phase liquid at 150 bar and 150 °C with `z = [0.1, 0.3, 0.6]`, the same mixture as the 1D
case, which saturates between 100 and 125 bar at this temperature. One producer on **rate** control:
30 sm³/day of surface oil, with a 20 bar BHP floor that never binds.

Rate control rather than BHP control is the point. With the withdrawal imposed identically on both
sides, the comparison is of the pressure path and the phase-appearance point, and it does not
inherit the conditioning that makes a near-shut-in well's cumulative a small difference of large
numbers — the thing that keeps `1D_COMP` from meeting the plan's 1% cumulative target.

## The reference run aborts, and that is the finding

`flowexp_comp` depletes the cell cleanly to 111.55 bar single phase, produces **one** report step at
109.10 bar with gas just appeared (`Sg = 0.0036`), and then its own Rachford–Rice stops converging
and the run is abandoned. Every two-phase flash method it offers — `ssi`, `newton`, `ssi-newton` —
fails the same way, and halving the production rate only moves where.

So the fixture is seven report steps rather than twenty, and `run-depletion.sh` checks the **step
count** rather than the exit status: an expected abort must not be indistinguishable from a real
failure. Those seven steps cover the whole single-phase depletion path and the phase appearance
itself, which is what this fixture is for.

ResSim runs the same case past that point without difficulty — its stability test and extended
(negative) Rachford–Rice window exist precisely for near-boundary states.

## What the BHP deck found

**C12's largest disagreement.** Both simulators start at 150 bar and settle at the well's 80 bar,
agreeing there to 0.003 bar, but ResSim depletes faster in between and its time constant is about
**1.29×** the reference's. Measured directly at the reference's own reported states, the producer
connection rate ratio is flat at 1.29 from the third step on.

Every term of `q = WI · Σ_P (kr_P/μ_P) · Δp · c_P` was checked and none accounts for it: the
drawdown is exact, the saturation agrees to 1e-4, the viscosities to 2% against OPM's own
PTFlash + LBC, and the accumulation to 0.02% on the difference that matters. Relative permeability
cannot produce it either — `λ_total` bottoms out at 7.1 over all saturations and the reference's
rate needs 5.8.

The **produced compositions agree to 1%**, so the phase split is right and what differs is a single
multiplicative constant on the connection. The one comparable check that passes is the 1D case's
injector, at 0.4%, and that cell is single phase.

See the C12 record in [`docs/COMPOSITIONAL_VALIDATION.md`](../../../docs/COMPOSITIONAL_VALIDATION.md).

## What the rate deck establishes, and what it opens

| | |
| --- | --- |
| Surface gas/oil **ratio** | agrees to **1.4e-8** — the reference's single-precision floor. The first head-to-head comparison of the two surface flashes on a mixture that genuinely splits |
| Phase appearance | agrees at all seven steps: single phase through 111.55 bar, two-phase at 109.10 |
| Surface volume **scale** | ResSim 236 926 mol/day for 30 sm³/day of surface oil; OPM's own PTFlash at `STCOND` says 236 582 — **0.15%**, the two fluid systems' critical constants |
| The oracle's self-consistency | `flowexp_comp`'s own trajectory implies **227 888** mol/day for that same 30 sm³/day — **3.8%** from its own flash. **Open, and not ResSim's** |
| Pressure path | separates by 0.23 bar/day, which is that 3.8% integrated. Timestep-independent to 0.001 bar over a 16-fold change in sub-step |

## What the pressure paths actually caught

They separate, and material balance says why — but the answer is not on ResSim's side.

While the cell is single phase its composition cannot change, so the moles it holds are `PV · c(p)`,
and both implementations agree on `c` to 0.12% along this very path (checked against OPM's own
`ParameterCache::molarVolume`). So the reference really withdrew about 227 900 mol/day for what it
reported as 30 sm³/day of surface oil. Asking OPM's **own** PTFlash what 30 sm³/day of surface oil
is — `ternary_stcond_depletion`, added to the C0 fixture for exactly this — gives 236 582 mol/day.

Two artefacts of the same simulator disagree with each other by 3.8%, and ResSim agrees with the one
that is a flash. The consequence for C12 is that no surface-metered cumulative on a *mixture* can be
held to the plan's 1% against this oracle. The 1D case is unaffected: its injected stream is pure
CO2, for which the two agree to about 0.01%.

`comp_depletion_reference_metering_disagrees_with_its_own_flash` is where all three numbers are
measured side by side, and it is bounded in both directions so that reconciling them fails rather
than passes silently.
