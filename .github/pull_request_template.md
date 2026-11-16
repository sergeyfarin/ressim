<!-- Do not describe a suspected security vulnerability here or in a commit message. See SECURITY.md. -->

## Summary

- **What changed?**
- **Why?**

Closes # <!-- only when every acceptance item is met; otherwise "Refs #" and say what remains -->

## Validation

Gates from `.claude/skills/ressim-validation/SKILL.md` for what changed, each with its result.

- [ ] `pnpm run validate:product` — frontend, catalog or worker-payload change
- [ ] `bash scripts/validate-solver-coverage.sh <bucket>` — Rust engine change
- [ ] `bash scripts/validate-cross-solver.sh` and `bash scripts/benchmarks.sh check` — anything that can move an answer
- [ ] Docs, skills and generated pages updated where this change moved a fact

Not run, and why:

## Numbers

Does this change a simulated or analytical result, solver work counts, a tolerance or band, a
benchmark record or the scorecard? If yes: which, by how much, the justification, and the commit
that re-recorded it. If no: say "none".

## Also fixed

<!-- small, local findings fixed here, each as its own commit -->

## Follow-up issues

<!-- everything else that was found, as issue links -->

## Decisions made on the owner's behalf
