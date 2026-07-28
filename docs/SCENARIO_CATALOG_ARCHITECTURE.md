# Scenario Catalog Architecture

The predefined scenario is ResSim's auditable product-content unit. A scenario definition owns
every decision that is specific to that case; shared code implements reusable mechanisms and must
not identify a case by its key.

## Scenario-owned declarations

Each module under `src/lib/catalog/scenarios/` declares:

- catalog group, content role, application mode, label, descriptions, and concise picker summary;
- complete simulator parameters and solver policy, including the user-facing rationale;
- analytical method selection, validity summary, literature reference, and external sources;
- chart-layout preset, scenario-local panel/curve/formatting overrides, and live panel definitions;
- sensitivity dimensions, variants, analytical-overlay policy, and default selection;
- optional termination policy and history/forecast window.

Scenario-local messages, warnings, curve choices, panel expansion, or formatting belong in that
module as data. A consumer must not contain `if (scenario.key === ...)`, a switch on a canonical
case key, or a label-based equivalent. `scenarioAgnosticArchitecture.test.ts` enforces the key rule.

## Legitimately shared mechanisms

Some behavior is not scenario-specific and remains shared:

- `analyticalMethodRegistry.ts` implements each analytical method once. A scenario selects the
  method in its capabilities; chart consumers ask the registry what that method produces.
- `chartLayouts.ts`, `panelDefs.ts`, and chart-panel builders provide reusable presentation
  primitives. A scenario selects a preset and owns any case-specific patch.
- the warning policy contains parameter-validity and runtime rules that apply equally to Custom
  Mode and predefined cases. A warning relevant only to one scenario belongs in its definition.
- catalog assembly may apply a declared policy mechanically, such as adding the generic
  FIM-vs-IMPES sensitivity when `comparisonSensitivityAvailable` is true. It must not decide the
  policy from gas flags, analytical method, or the scenario key.

## Catalog taxonomy

The visible group is explicit metadata, not a physics inference:

1. **Buckley–Leverett Displacement** — 1D displacement fundamentals and departures from BL.
2. **Sweep Efficiency** — areal, vertical, and combined waterflood contact.
3. **Depletion & Decline** — bounded depletion, decline interpretation, and depletion ambiguity.
4. **Pressure-Transient Analysis** — drawdown now; buildup, Horner, interference, and pulse tests later.
5. **Gas-Dominated Recovery** — gas injection and solution-gas drive mechanisms.
6. **Validation Benchmarks** — published or external comparative solutions such as SPE1 and future SPE10/SPE9 cases.
7. **Other** — cross-cutting interpretation cases awaiting a permanent family.

`catalog.role` is a separate axis (`simulation`, `interpretation`, or `benchmark`). This prevents
the navigation hierarchy from conflating physical domain with the purpose of the content.

## Admission rule for future cases

A new top-level scenario needs a distinct engineering question and a plot capable of answering it.
A new reference implementation belongs in `referenceSources`; a parameter variation belongs in a
sensitivity dimension; neither alone justifies another picker entry. Numerical grid, timestep, and
solver variants should remain secondary validation sensitivities rather than new scenarios.
