# Tracker migration — 2026-08-02

ResSim moved actionable work from repository checkboxes to GitHub Issues on 2026-08-02.

## Why

The live `TODO.md` had grown to 1,294 lines and mixed current defects, completed delivery records,
scientific measurements, rejected experiments, future cases, and maintenance notes. It was hard to
prioritize, easy to conflict during parallel work, and duplicated `ROADMAP.md`, validation docs,
the case-library roadmap, and the FIM registry/worklog.

## Preserved source

The complete pre-migration tracker and roadmap are preserved in Git history at commit `991b19d`.
Earlier tracker history remains in `TODO_HISTORY_2026-07-24.md`.

## New ownership

| Information | Owner |
|---|---|
| Actionable bugs, features, validation gaps, maintenance | [GitHub Issues](https://github.com/sergeyfarin/ressim/issues) |
| Release readiness | [Limited public release milestone](https://github.com/sergeyfarin/ressim/milestone/1) |
| Strategic ordering | `ROADMAP.md` |
| Stable documentation map | `docs/DOCUMENTATION_INDEX.md` |
| Scenario sourcing and admission | `docs/CASE_LIBRARY_ROADMAP.md` |
| FIM experiment verdicts and retry conditions | `docs/FIM_EXPERIMENT_REGISTRY.md` |
| Active FIM traces and hypotheses | `docs/FIM_CONVERGENCE_WORKLOG.md` |
| Completed historical narrative | `.archive/docs/` and Git history |

## Initial issue map

- Release: #8–#9
- Scientific validation: #10–#13
- Chart/product architecture: #14–#15
- Scenario enablers: #16–#18
- Analytical/reference expansion: #19–#20
- FIM research: #21–#23
- Maintenance: #24

Issue #9 was closed immediately because commit `991b19d` had already completed the scenario redesign
that surfaced during the migration. Its remaining follow-ups were routed to active issues.

Two issue forms were added under `.github/ISSUE_TEMPLATE/`: a general work item requiring an
outcome, evidence, acceptance criteria, and validation gate; and a scientific investigation form
requiring a clean baseline, valid oracle, coupled-semantics audit, and predeclared verdict criteria.
