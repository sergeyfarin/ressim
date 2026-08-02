# ResSim roadmap

This roadmap owns strategic order. GitHub Issues owns execution state; stable scientific evidence
belongs in the validation documents indexed by `docs/DOCUMENTATION_INDEX.md`.

## Principles

1. Validate existing behavior before expanding the model envelope.
2. Make every scenario claim measurable and every reference explicit about its assumptions.
3. Remove architectural duplication before adding new chart or workflow primitives.
4. Add physics through a named consuming case and an independent oracle, not as an isolated switch.
5. Keep browser release quality—deployment, reproducibility, warnings, privacy, and licensing—part
   of the product definition.

## 0. Limited public release

The only remaining release blocker is
[#8: deploy and smoke-test GitHub Pages](https://github.com/sergeyfarin/ressim/issues/8).
The scenario-integrity audit was resolved by `991b19d` and is retained as closed issue
[#9](https://github.com/sergeyfarin/ressim/issues/9).

Release requires a clean committed revision, the documented product gate, one IMPES and one FIM
smoke run on the deployed site, working WASM/worker/charts/3D assets, and valid social metadata.

## 1. Scientific validation and closure

- [#10 — Fully perforated gravity wells](https://github.com/sergeyfarin/ressim/issues/10)
- [#11 — FIM/IMPES black-oil depletion disagreement](https://github.com/sergeyfarin/ressim/issues/11)
- [#12 — SPE1 and black-oil validation gaps](https://github.com/sergeyfarin/ressim/issues/12)
- [#13 — CI aligned with the product gate](https://github.com/sergeyfarin/ressim/issues/13)

Authoritative evidence: `docs/BLACK_OIL_VALIDATION.md`, `docs/THREE_PHASE_VALIDATION.md`,
`docs/P4_TWO_PHASE_BENCHMARKS.md`, and `docs/SOLVER_COMPARISON_SUMMARY.md`.

## 2. Product and chart architecture

- [#14 — Typed output selection and chart consolidation](https://github.com/sergeyfarin/ressim/issues/14)
- [#15 — Remaining chart correctness and presentation](https://github.com/sergeyfarin/ressim/issues/15)

The current design audit is `docs/CHART_ARCHITECTURE_REVIEW_2026-08-02.md`. Preserve the analytical
method registry, declared reference sources, scenario-agnostic routing, and single-property panel
contracts while simplifying orchestration.

## 3. Scenario and workflow enablers

- [#16 — Field permeability and multi-well patterns](https://github.com/sergeyfarin/ressim/issues/16)
- [#17 — Declarative schedules and deferred physics](https://github.com/sergeyfarin/ressim/issues/17)
- [#18 — Ensemble bands and curated pre-run exhibits](https://github.com/sergeyfarin/ressim/issues/18)

These capabilities land only with a consuming case and a validation source. Detailed case admission,
Tier 7 IDs, and enabler dependencies remain in `docs/CASE_LIBRARY_ROADMAP.md`.

## 4. Analytical and reference expansion

- [#19 — Assumption-checked analytical cases](https://github.com/sergeyfarin/ressim/issues/19)
- [#20 — Missing OPM references and artifact provenance](https://github.com/sergeyfarin/ressim/issues/20)

Preferred order: gravity-modified fractional flow; gravity-capillary equilibrium; Koval; gas-cap
blowdown; then interaction/uncertainty cases after their enablers. Dataset licensing and provenance
are admission requirements, not cleanup after publication.

## 5. FIM research frontier

- [#21 — Report-step sensitivity and systematic Flow oil bias](https://github.com/sergeyfarin/ressim/issues/21)
- [#22 — Newton production-seam refactor](https://github.com/sergeyfarin/ressim/issues/22)
- [#23 — Parked OPM-parity research backlog](https://github.com/sergeyfarin/ressim/issues/23)

The registry and worklog own experiment detail. Missing backend-neutral diagnostics produce an
`INCONCLUSIVE` verdict; partial OPM ports cannot refute the coupled lifecycle they omit.

## 6. Maintenance

- [#24 — UI-audit and developer-maintenance debt](https://github.com/sergeyfarin/ressim/issues/24)

Maintenance work should remain causally scoped and must not bundle speculative solver or chart
redesigns.

## Delivered capability record

Completed implementation history is not repeated here. Use:

- `.archive/docs/DELIVERED_WORK_2026_Q1.md`
- `.archive/docs/TODO_HISTORY_2026-07-24.md`
- Git history through the tracker migration point `991b19d`
- the validation and architecture documents in `docs/DOCUMENTATION_INDEX.md`
