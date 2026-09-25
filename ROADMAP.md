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

## 0. Limited public release — delivered

[#8](https://github.com/sergeyfarin/ressim/issues/8) is closed: the site is deployed to GitHub Pages
from CI (`publish-dist.yml`) and smoke-tested on the deployed revision (`pnpm run test:deployed`).
The [milestone](https://github.com/sergeyfarin/ressim/milestone/1) has no open issues.

## 1. Scientific validation and closure

- [#12 — SPE1 and black-oil scenario validation gaps](https://github.com/sergeyfarin/ressim/issues/12)
- [#22 — One cross-solver harness and scorecard against OPM Flow](https://github.com/sergeyfarin/ressim/issues/22)
- [#35 — Bubble-point PVT conventions cost ~30 % Newton (FIM-KINK-001 J1)](https://github.com/sergeyfarin/ressim/issues/35)

Closed since the last revision of this list: #10 (gravity wells), #11 (FIM/IMPES depletion,
an unstable PVT table), #13 (CI), #21 (Flow oil bias, no solver defect), #36–#39, #42–#44.

Current numbers: [`docs/BENCHMARKS.md`](docs/BENCHMARKS.md). Authoritative evidence:
`docs/BLACK_OIL_VALIDATION.md`, `docs/THREE_PHASE_VALIDATION.md` and
`docs/P4_TWO_PHASE_BENCHMARKS.md`.

## 2. Product and chart architecture

- [#15 — Remaining chart correctness and presentation](https://github.com/sergeyfarin/ressim/issues/15)
- [#26 — Second sensitivity dimension for the withheld `dep_pvt`](https://github.com/sergeyfarin/ressim/issues/26)

#14 (typed output selection) is closed. The current design audit is
`docs/CHART_ARCHITECTURE_REVIEW_2026-08-02.md`. Preserve the analytical method registry, declared
reference sources, scenario-agnostic routing, and single-property panel contracts while
simplifying orchestration.

## 3. Scenario and workflow enablers

- [#16 — Field permeability and multi-well patterns](https://github.com/sergeyfarin/ressim/issues/16)
- [#17 — Declarative schedules and deferred physics](https://github.com/sergeyfarin/ressim/issues/17)
- [#18 — Ensemble bands and curated pre-run exhibits](https://github.com/sergeyfarin/ressim/issues/18)

These capabilities land only with a consuming case and a validation source. Detailed case admission,
Tier 7 IDs, and enabler dependencies remain in `docs/CASE_LIBRARY_ROADMAP.md`.

## 4. Analytical and reference expansion

- [#19 — Assumption-checked analytical cases](https://github.com/sergeyfarin/ressim/issues/19)

#20 (OPM references and artifact provenance) is closed; eleven parsed artifacts ship. Preferred
order: gravity-modified fractional flow; gravity-capillary equilibrium; Koval; gas-cap blowdown;
then interaction/uncertainty cases after their enablers. Dataset licensing and provenance are
admission requirements, not cleanup after publication.

## 4a. Compositional engine

The native engine is validated against OPM `flowexp_comp` (`docs/COMPOSITIONAL_VALIDATION.md`).
Next, in order:

- [#29 — C13: compositional chart sourcing, then product integration](https://github.com/sergeyfarin/ressim/issues/29)
- [#52 — Run SPE5, then SPE3](https://github.com/sergeyfarin/ressim/issues/52), which needs
  [#45](https://github.com/sergeyfarin/ressim/issues/45)–[#51](https://github.com/sergeyfarin/ressim/issues/51)
  (components, immiscible water, 3-D and gravity, rate control, a sparse linear route, schedules,
  SPE3 characterization)

## 5. FIM research frontier

- [#23 — Parked OPM-parity research backlog](https://github.com/sergeyfarin/ressim/issues/23)

The registry and worklog own experiment detail. Missing backend-neutral diagnostics produce an
`INCONCLUSIVE` verdict; partial OPM ports cannot refute the coupled lifecycle they omit.

## 6. Maintenance

- [#24 — UI-audit and developer-maintenance debt](https://github.com/sergeyfarin/ressim/issues/24)
- [#31 — `scripts/debug-spe1-*.ts` do not run under Node ESM](https://github.com/sergeyfarin/ressim/issues/31)

Maintenance work should remain causally scoped and must not bundle speculative solver or chart
redesigns.

## Delivered capability record

Completed implementation history is not repeated here. Use:

- `.archive/docs/DELIVERED_WORK_2026_Q1.md`
- `.archive/docs/TODO_HISTORY_2026-07-24.md`
- Git history through the tracker migration point `991b19d`
- the validation and architecture documents in `docs/DOCUMENTATION_INDEX.md`
