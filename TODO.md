# ResSim work tracker

GitHub Issues is the source of truth for actionable work:

- [Open issues](https://github.com/sergeyfarin/ressim/issues)
- [Limited public release milestone](https://github.com/sergeyfarin/ressim/milestone/1)
- [Prioritized roadmap](ROADMAP.md)

This file remains as a stable landing page for older links. It is deliberately not a second
checkbox tracker. Do not add task narratives or completed-work history here.

## Now

The limited public release ([#8](https://github.com/sergeyfarin/ressim/issues/8)) is deployed and
its milestone is closed. See [ROADMAP.md](ROADMAP.md) for the current order.

### High priority

- [#12 — Close remaining SPE1 and black-oil validation gaps](https://github.com/sergeyfarin/ressim/issues/12)
- [#15 — Fix remaining chart correctness and presentation defects](https://github.com/sergeyfarin/ressim/issues/15)
- [#26 — Give the withheld dep_pvt case a second sensitivity dimension](https://github.com/sergeyfarin/ressim/issues/26)
- [#29 — Compositional model: C13 chart sourcing, then C14](https://github.com/sergeyfarin/ressim/issues/29)
- [#52 — Run SPE5, then SPE3, on the compositional engine](https://github.com/sergeyfarin/ressim/issues/52)

## Tracking rules

1. Create or update a GitHub Issue for an actionable bug, feature, validation gap, or maintenance
   task. Give it an outcome, acceptance criteria, priority, and area label.
2. Put stable scientific evidence and replay commands in the owning validation document. Put FIM
   experiments and negative results in `docs/FIM_EXPERIMENT_REGISTRY.md` and
   `docs/FIM_CONVERGENCE_WORKLOG.md`.
3. Keep strategic sequencing in `ROADMAP.md` and detailed case sourcing in
   `docs/CASE_LIBRARY_ROADMAP.md`. Do not duplicate those narratives in issue bodies.
4. Close the issue when its acceptance criteria and required validation gate are complete. Link the
   implementing commit or pull request.
5. Discoveries made while working belong in the current issue when in scope; otherwise open a new
   issue before declaring the work complete.

Use the repository issue forms for implementation work and scientific investigations. Apply one
priority label and at least one area label; use `research` only when the outcome depends on a
hypothesis/oracle rather than a predetermined implementation.

## Migration record

The former 1,294-line tracker is preserved in Git history at commit `991b19d`. The issue mapping and
the reason for the change are recorded in
`.archive/docs/TRACKER_MIGRATION_2026-08-02.md`.
