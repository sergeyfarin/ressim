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
- catalog assembly registers scenario definitions without adding case content. Sensitivity
  dimensions and variants are complete in the owning scenario module; shared run code only
  interprets declarative properties such as `variesSolver`.

## Catalog taxonomy

The visible group is explicit metadata, not a physics inference:

1. **Buckley–Leverett Displacement** — 1D displacement fundamentals and departures from BL.
2. **Sweep Efficiency** — areal, vertical, and combined waterflood contact.
3. **Flow Regimes & Decline** — one well's pressure history in sequence: infinite-acting transient
   flow, pseudo-steady productivity, boundary-dominated decline, and layered superposition.
4. **Gas-Dominated Recovery** — gas injection and solution-gas drive mechanisms.
5. **Validation Benchmarks** — published or external comparative solutions such as SPE1 and future SPE10/SPE9 cases.
6. **Other** — cross-cutting interpretation cases awaiting a permanent family.

Merged 2026-07-31: a separate **Pressure-Transient Analysis** group held only the drawdown case,
splitting a single physical continuum in two — the same well, the same geometry, read in a
different flow regime. Reinstate it when buildup, Horner, interference or pulse-test cases arrive
and the interpretation workflow, rather than the flow regime, becomes the organising idea.

`catalog.role` is a separate axis (`simulation`, `interpretation`, or `benchmark`). It is metadata
for the catalog, not a badge: the picker showed it beside benchmark scenarios until 2026-07-31,
where it read as a quality claim about the scenario rather than a statement of its purpose.

This prevents the navigation hierarchy from conflating physical domain with the purpose of the
content.

## Admission rule for future cases

A new top-level scenario needs a distinct engineering question and a plot capable of answering it.
A new reference implementation belongs in `referenceSources`; a parameter variation belongs in a
sensitivity dimension; neither alone justifies another picker entry. A numerical comparison may be
a top-level interpretation scenario only when the formulation itself is the engineering question,
the case parameters make its consequence visible, and the chart is configured to show that result.
