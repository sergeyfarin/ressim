# OPM diagnostic oracles

The patches and commands in this directory are observation-only aids for the tracked FIM/Flow
comparison. They are not production OPM dependencies.

## WATER-013 same-policy materialized `system_cpr` matrix

`water012-system-cpr-materialized-dump.patch` applies to Flow source commit
`062cb19986aa8f11cffc30351fd2fee355d0ccb4`. At linear-solver verbosity above 10 it preserves
the live `system_cpr` matrix-free solve, while additionally writing
`reports/*_flow_materialized_matrix_istl.mm`. The file is the ordinary reservoir matrix plus the
same `WellModelAsLinearOperator` (`-C D^-1 B`) used by Flow's actual solve; it is generated only
for observation and is never passed back to the solver.

Apply, rebuild Flow, then run the unmodified water deck with its ordinary linear policy:

```sh
git apply /path/to/water012-system-cpr-materialized-dump.patch
flow CASE.DATA --linear-solver-verbosity=11 --output-extra-convergence-info=steps,iterations
```

Do not use `--matrix-add-well-contributions=true` or switch to `cpr_quasiimpes` for this oracle:
those alter the `system_cpr` contract. Compare `nit_2`'s materialized matrix with the normal RHS
and the held ResSim capture using `solver_lab_water012_eval2_reservoir_well_decomposition`.

## G4c0 reservoir partition

Apply `g4c0-reservoir-partition.patch` to exact OPM `release/2026.04/final` commit
`b82f21dba405286c4c4446614dd3bf9cdebf7a2c`. The patch instruments the TPFA fast linearizer used
by the reference case and prints accumulation, signed faces, source, and assembled reservoir row.

## G4c2 evaluation-0 matrix

Flow's default MatrixMarket export omits matrix-free well contributions. A coupled evaluation-0
oracle must materialize them explicitly and use a compatible linear solver:

```sh
flow CASE.DATA \
  --matrix-add-well-contributions=true \
  --linear-solver=cpr_quasiimpes \
  --linear-solver-verbosity=11 \
  --solver-verbosity=3 \
  --time-step-verbosity=3
```

Use only evaluation 0 for comparison: the changed linear solver can alter the later trajectory.
Map Flow equations `[oil, water, gas]` to ResSim `[water, oil, gas]`, and Flow primaries
`[Sw, pressure(Pa), composition-switch]` to ResSim `[pressure(bar), Sw, hydrocarbon]`. Multiply
Flow reservoir rows and RHS by `21600 s`; additionally multiply the Flow pressure column by
`1e5 Pa/bar`. Flow solves `J dx = R` and subtracts `dx`, while ResSim solves `J dx = -R` and adds
`dx`, so negate the mapped Flow RHS/correction when comparing signs.

The tracked Flow deck has no `DRSDT`, while the historical ResSim exact-test preset forced
DRSDT0. Use `FIM_Y1J_GAS_REDISSOLUTION=1` on the ResSim diagnostic when comparing the deck as
written; otherwise the engines begin with different hydrocarbon primaries.

For WATER-002's two-phase one-day deck, map Flow rows `[oil, water]` and primaries
`[Sw, pressure(Pa)]` to ResSim rows `[water, oil]` and primaries `[pressure(bar), Sw]`. Multiply
Flow rows/RHS by `86400 s` and its pressure column by `1e5 Pa/bar`; negate the Flow RHS and raw
correction for ResSim's update sign. ResSim's third cell row must first be verified as a zero-RHS,
no-feedback structural pin, then explicit well rows are Schur-eliminated.

Flow's true raw maximum update can be cross-checked without modifying OPM by setting tiny positive
solution-change tolerances and reading `CASE.DBG`:

```sh
flow CASE.DATA --matrix-add-well-contributions=true \
  --linear-solver=cpr_quasiimpes \
  --tolerance-max-dp=1e-300 --tolerance-max-ds=1e-300 \
  --solver-verbosity=3 --time-step-verbosity=3
```

For the corrected water deck it reports evaluation-1 `DP=1.107e7 Pa, DS=1.604e2`, matching the
evaluation-0 MatrixMarket solve. Run `solver_lab_water002_matched_first_correction` with
`FIM_CAPTURE_SEQUENCE_DIR`, `FIM_WATER002_FLOW_MATRIX`, and `FIM_WATER002_FLOW_VECTOR` to reproduce
the exact correction and ranked well-Schur attribution.

WATER-003 source trace: `PiecewiseLinearTwoPhaseMaterial::evalAscending_` in opm-common returns
`yValues.front()` when `x <= xValues.front()`, making the AD derivative zero at the lower SWOF
endpoint. Replay the coherent ResSim FIM property contract with
`FIM_WATER003_ENDPOINT_REPLAY=1`; add `FIM_WATER003_FIRST_UPDATE=1` to the full-target probe to
lock injector `p=390 bar, Sw=0.3` and evaluation-1 water MB `0.313756`. The flag is native-only
and default false; it does not remove the well Schur or change nonlinear policy.

WATER-004 uses Flow's exported `nit_1` matrix/vector and ResSim capture sequence `00001` after
that endpoint replay. Run `solver_lab_water004_matched_second_correction` with
`FIM_CAPTURE_SEQUENCE_DIR`, `FIM_WATER004_FLOW_MATRIX`, and `FIM_WATER004_FLOW_VECTOR`. Its
matrix/RHS mapping is the WATER-002 mapping; do not call the comparison same-state merely from
the matched global MB, because Flow's export has no intermediate cell-state payload. At `Sw=.3`,
the corrected deck's rounded SWOF slopes (`.469`, `-2.031`) also differ from ResSim's analytic
Corey slopes (`.625`, `-1.875`), so the next valid mechanism gate is property replay, not a
controller or iteration-limit change.

WATER-005's default-false `FIM_WATER005_SWOF_REPLAY=1` replaces the entire two-phase FIM mobility
law with that corrected deck's rounded nine-row SWOF table, in both scalar and AD paths. Combine
it with `FIM_WATER_FULL_TARGET_PROBE=1`, `FIM_NESTED_WELL_SOLVE=1`, and a sequential capture to
repeat the `nit_1` lab. It is deliberately fixture-specific and native-only; it does not change
the product Corey model or authorize controller/iteration-limit changes.

## G5a post-switch contract

The explicit comparator contract is `DISGAS` with `DRSDT` omitted: Flow therefore leaves the
per-step dissolved-gas increment unbounded, and the matching ResSim native diagnostic must set
`FIM_Y1J_GAS_REDISSOLUTION=1`. The deck and its hash remain unchanged.

At evaluation 0 both engines use `Rs`; after the overshooting correction both adapt to saturated
`Sg=0`. Compare evaluation 1 only after confirming all four observables: primary tag/value, BHP,
connection rate, and surface source. A zero ResSim source is not a reservoir-row comparison: it
means the selected injector has fallen onto its clamped non-injecting connection branch.

OPM's `StandardWell_impl.hpp` shows that an injecting perforation uses total reservoir mobility.
Together with Flow retaining its injecting source after the switch, this supports (by inference)
recovering the selected ResSim well on that same injecting branch before its local Newton solve.
It does not establish parity for the complete StandardWell lifecycle, BHP control switching, or
multi-perforation allocation.

## G4c3 derivative counterfactual

Capture the matched ResSim evaluation sequence with `FIM_CAPTURE_SEQUENCE_DIR` and the G5a exact
native command, then run the ignored
`solver_lab_g4c3_water_storage_counterfactual` test with the same directory. The test solves the
unchanged full RHS four ways: original, missing water-compressibility storage diagonal only,
mapped non-water cell-block deltas only, and both. Every reported correction is checked against
the full patched matrix with relative residual below `1e-10`.

The water delta is source-derived rather than fitted: OPM's
`ConstantCompressibilityWaterPvt::inverseFormationVolumeFactor()` uses the PVTW reference pressure
and compressibility, making the initial storage derivative `PV*Sw*(c_r+c_w)`. OPM
`LiveOilPvt` stores/evaluates `1/Bo` and `1/(Bo*mu_o)` tables. ResSim presently keeps constant
`b_w` in FIM component inventory and directly interpolates oil `Bo/mu_o`; the exact native fixture
also maps only PVTO saturated rows. These coupled differences require a coherent lifecycle, not
a storage-diagonal-only production edit.

## G4c4 production lifecycle

The ResSim matched native fixture now maps every PVTO row in the deck, including the four
undersaturated endpoints. Production interpolation uses `1/Bo` and `1/(Bo*mu_o)` in pressure and
Rs. Water uses the PVTW reference pressure set by the case's initial pressure and OPM's quadratic
constant-compressibility inverse-FVF expression in all storage/flux/well/reporting consumers.

Run the matched six-step native oracle with `FIM_Y1J_GAS_REDISSOLUTION=1`,
`FIM_Y2B_RAW_SATURATION=1`, `FIM_FLOW_RESV_INJECTOR=1`, and
`FIM_NESTED_WELL_SOLVE=1`. Add `FIM_FORCE_DIRECT_LINEAR=1` for the one-step exact correction
oracle. Do not treat this as complete OPM extrapolation parity: only in-table reciprocal PVTO
segments are implemented in G4c4.

## G4c5 guided PVTO and coarse-factorization replay

OPM `LiveOilPvt.cpp` selects
`UniformXTabulated2DFunction::InterpolationPolicy::LeftExtreme`. The implementation in
`UniformXTabulated2DFunction.hpp` evaluates adjacent Rs branches at guided pressure coordinates:
`p_low=p-t*shift`, `p_high=p+(1-t)*shift`. ResSim applies this to both `1/Bo` and
`1/(Bo*mu_o)` in the scalar and AD paths.

Capture sequential matched systems into one directory with the G4c4 environment selectors, then
run `solver_lab_coarse_factorization_comparison` with `FIM_CAPTURE_DIR` pointing at that directory.
The G4c5 seven-system oracle has 300 coarse rows, zero ILU0/BiCGSTAB failures, median relative
residual `5.197e-7`, and maximum `7.879e-7`; this is the evidence for routing exactly 300 rows away
from dense inversion. The production six-step oracle must retain 23 updates, 55 Krylov
iterations, and zero cuts while timing the setup improvement.
