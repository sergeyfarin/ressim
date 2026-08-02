# ResSim work tracker

GitHub Issues is the source of truth for actionable work:

- [Open issues](https://github.com/sergeyfarin/ressim/issues)
- [Limited public release milestone](https://github.com/sergeyfarin/ressim/milestone/1)
- [Prioritized roadmap](ROADMAP.md)

This file remains as a stable landing page for older links. It is deliberately not a second
checkbox tracker. Do not add task narratives or completed-work history here.

## Now

### Limited public release

- [#8 — Deploy ResSim to GitHub Pages and smoke-test the public URL](https://github.com/sergeyfarin/ressim/issues/8)

The catalog-integrity blocker found during the release audit was completed by `991b19d`; its
migration issue is retained as the closed record [#9](https://github.com/sergeyfarin/ressim/issues/9).

### High priority

- [#10 — Support fully perforated wells under gravity](https://github.com/sergeyfarin/ressim/issues/10)
- [#11 — Resolve the FIM/IMPES black-oil depletion disagreement](https://github.com/sergeyfarin/ressim/issues/11)
- [#12 — Close remaining SPE1 and black-oil validation gaps](https://github.com/sergeyfarin/ressim/issues/12)
- [#13 — Strengthen pull-request CI](https://github.com/sergeyfarin/ressim/issues/13)
- [#14 — Consolidate chart architecture and output selection](https://github.com/sergeyfarin/ressim/issues/14)

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
