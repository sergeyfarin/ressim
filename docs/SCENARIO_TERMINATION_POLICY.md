# Scenario Termination Policy

Use scenario termination policies to stop a WASM run early when a runtime production condition is reached, while still returning the final state and full accumulated rate history to the frontend through the normal completion path.

## Where It Lives

- Scenario contract: `src/lib/catalog/scenarios.ts`
- Example scenario usage: `src/lib/catalog/scenarios/sweep_areal.ts`
- Worker evaluation: `src/lib/workers/terminationPolicy.ts`
- Worker run-loop integration: `src/lib/workers/sim.worker.ts`

## Shape

```ts
terminationPolicy: {
  mode: 'any' | 'all',
  conditions: ScenarioTerminationCondition[],
}
```

`mode` semantics:

- `any`: stop as soon as one condition matches
- `all`: stop only after every condition matches on the same accepted step

## Supported Condition Kinds

### `watercut-threshold`

Stops when water cut reaches or exceeds a fractional threshold.

```ts
{
  kind: 'watercut-threshold',
  value: 0.01,
  scope: 'producer',
}
```

Fields:

- `value`: fractional water cut, not percent
- `scope`: currently `producer` or `any-producer`

Examples:

- `0.01` = 1% water cut
- `0.10` = 10% water cut
- `0.98` = 98% water cut

Current evaluation:

- water cut is computed from runtime production totals as
  `waterRate / liquidRate`
- water rate is derived as `total_production_liquid - total_production_oil`

### `phase-rate-threshold`

Stops when a phase rate crosses a threshold.

```ts
{
  kind: 'phase-rate-threshold',
  phase: 'oil',
  relation: 'lte',
  value: 0,
  scope: 'producer',
}
```

Fields:

- `phase`: `oil`, `water`, or `gas`
- `relation`: `lte` or `gte`
- `value`: threshold in surface-rate units
- `scope`: `producer`, `injector`, or `any`

Examples:

- Oil rate drops to zero:

```ts
{
  kind: 'phase-rate-threshold',
  phase: 'oil',
  relation: 'lte',
  value: 0,
  scope: 'producer',
}
```

- Gas production exceeds 5000:

```ts
{
  kind: 'phase-rate-threshold',
  phase: 'gas',
  relation: 'gte',
  value: 5000,
  scope: 'producer',
}
```

Current evaluation notes:

- producer phase rates are read from aggregate runtime totals
- injector phase-rate evaluation is only meaningful for injected phases the model reports through aggregate injection totals
- aggregate injection is currently interpreted from `total_injection` together with `injectedFluid`

### `gor-threshold`

Stops when producing GOR crosses a threshold.

```ts
{
  kind: 'gor-threshold',
  relation: 'gte',
  value: 500,
  scope: 'producer',
}
```

Fields:

- `relation`: `lte` or `gte`
- `value`: threshold in `Sm3/Sm3`
- `scope`: `producer` or `any`

Example:

```ts
{
  kind: 'gor-threshold',
  relation: 'gte',
  value: 1200,
  scope: 'producer',
}
```

## Example Policies

Stop at first water breakthrough:

```ts
terminationPolicy: {
  mode: 'any',
  conditions: [
    { kind: 'watercut-threshold', value: 0.01, scope: 'producer' },
  ],
}
```

Stop when oil rate is depleted or GOR becomes too high:

```ts
terminationPolicy: {
  mode: 'any',
  conditions: [
    { kind: 'phase-rate-threshold', phase: 'oil', relation: 'lte', value: 0, scope: 'producer' },
    { kind: 'gor-threshold', relation: 'gte', value: 1200, scope: 'producer' },
  ],
}
```

Stop only after both high water cut and high GOR are present:

```ts
terminationPolicy: {
  mode: 'all',
  conditions: [
    { kind: 'watercut-threshold', value: 0.90, scope: 'producer' },
    { kind: 'gor-threshold', relation: 'gte', value: 1500, scope: 'producer' },
  ],
}
```

## Runtime Behavior

- The worker evaluates the policy after each accepted simulator step.
- If a condition matches, the worker emits the final `state` message for that step.
- The worker then finishes through the normal `batchComplete` path rather than the manual-stop path.
- The frontend therefore receives all history and rate data normally, just with fewer steps than originally requested.

## Current Limits

- Policies are currently configured from scenario definitions, not from UI controls.
- Evaluation is based on aggregate reported producer or injector totals, not per-well selection.
- `scope` values are forward-looking in part; today the runtime mostly evaluates aggregate producer-side metrics.
- If finer per-well stopping is needed later, extend runtime reporting first rather than changing scenario syntax.