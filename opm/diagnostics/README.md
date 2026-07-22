# OPM diagnostic oracles

The patches and commands in this directory are observation-only aids for the tracked FIM/Flow
comparison. They are not production OPM dependencies.

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
