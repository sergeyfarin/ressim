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

The visible group is explicit metadata, not a physics inference. **Groups run on one axis: the
physical question the case answers.** Reference status — analytical overlay, OPM Flow run,
digitized publication — is per-case data (`capabilities.analyticalMethod`, `referenceSources`) and
must never become a group, because it is not a property the members of a physical family share.

1. **1D Displacement — Buckley–Leverett** — one flow path, so displacement efficiency is the whole
   answer. Rule: at most one grid extent > 1, in the base case and in every variant.
2. **Sweep Efficiency** — how much of the rock the flood contacts. Rule: at least two grid extents
   > 1 — the exact complement of the group above.
3. **Flow Regimes & Decline** — one well's pressure history in sequence: infinite-acting transient
   flow, pseudo-steady productivity, boundary-dominated decline, and layered superposition. Rule:
   no injector, and the analytical method is `well-test` or `depletion`.
4. **Material Balance & Drive Mechanism** — where the energy comes from and how much is in place:
   expansion, solution-gas liberation, compaction, PVT representation. Rule: the case shows a
   tank-level result — an `mbe_ooip`, `drive_indices` or `pz` panel.
5. **Published Benchmark Decks** — SPE1 and future SPE9/SPE10. Rule: **deck fixity** — sensitivity
   variants may patch discretization and solver settings only, never a reservoir property.

Every rule above is enforced in `scenarios.test.ts`; rule 4 is present but skipped until the
Havlena-Odeh drive-index panels reach the `gas` chart layout (`TODO.md`).

Replaced 2026-08-02: groups 4 and 5 were **Simulation Only — No Analytical Reference** and
**Validation Benchmarks**, which sorted by epistemic status while groups 1–3 sorted by physics —
two axes in one hierarchy, the exact conflation this section exists to prevent. The names were also
false in both directions: `gas_drive` sat under "No Analytical Reference" while carrying a graded
OPM Flow reference, and `wf_gravity`, `wf_numerics` and `dep_gas_pz` carried external references
from outside "Validation Benchmarks". Group 5 survives because deck fixity is a real property of a
case, not a claim about its quality; group 4 is new, and `dep_gas_pz` moved into it from group 3
because a reserves estimate is a tank answer, not a flow regime.

Removed 2026-07-31: an **Other** group holding only the FIM-vs-IMPES comparison. That case was a
variation of two parameters on an existing scenario rather than a distinct question, and as a
standalone case it declared no analytical method, so it could show that the formulations differ but
never which was closer to truth. It is now `wf_bl1d`'s `solver_formulation` dimension, judged
against the Buckley-Leverett reference. Reinstate a holding group only for a case that genuinely
has no family, not for one that has no reference.

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
