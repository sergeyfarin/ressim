# Developer workflow

How work moves through this repository: tracking, branches and pull requests, commits, review,
and where agent guidance lives. It applies to people and to coding agents. `AGENTS.md` restates
the parts an agent must hold in every session; where the two differ, this document governs and
`AGENTS.md` is corrected in the same change.

## Owner decisions, 2026-11-15

Four decisions by the owner, recorded here because they change how work was done before.

1. **Implementation work goes through a short-lived branch and a pull request**, opened without
   being asked. This replaces committing directly to `master`, which was the usual practice
   until early 2026-11. The reason is coverage: `.github/workflows/pr-tests.yml` runs only on
   pull requests, so a direct commit is checked by no FIM bucket, no native-binding parity gate
   and no benchmark check (`docs/FIM_CROSS_TARGET_DIVERGENCE_2026-09-22.md` records a regression
   that reached `master` that way).
2. **Whoever implements also looks for further defects**: bugs, weak tests, code smells and
   improvement opportunities. A small, local one is fixed in the same pull request as its own
   commit; anything larger becomes a GitHub issue. Nothing stays only in a conversation.
3. **A pull request review is review only.** The reviewer judges everything, the owner's
   decisions included, and fixes nothing.
4. **`AGENTS.md` is the only repo-wide instruction file.** No harness-specific copies.
   `CLAUDE.md` is a one-line import of it and holds nothing else.

On the same date the tracking rules moved here from `TODO.md`, which is now only a landing page
for older links.

## Tracking

GitHub Issues is the source of truth for actionable work.

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

- Priority labels: `priority:P0` (must be resolved before the next public release),
  `priority:P1` (high-priority user-facing or correctness work), `priority:P2` (important planned
  work, not release-blocking), `priority:P3` (deferred, polish, or maintenance).
- Area labels: `area:engine`, `area:frontend`, `area:scenario`, `area:validation`, `area:docs`,
  `area:release`.
- `blocked` requires a named prerequisite.
- Creating an issue sets no milestone and does not change the order in `ROADMAP.md`. Both are
  the owner's.
- A deliberate "not doing this" goes in `docs/OPEN_ITEMS_2026-09-21.md` with the reason.

## Branches and pull requests

- Branch from the latest canonical default branch, `master`: `upstream/master` when an
  `upstream` remote exists (a fork, whose own `master` can be behind), otherwise
  `origin/master`.

  ```sh
  base=$(bash scripts/canonical-repo.sh --remote) &&
  git fetch "$base" &&
  git switch -c <type>/<issue>-<short-name> "$base/master"
  ```

- Every `gh` call names that canonical repository in the same command
  (`repo=$(bash scripts/canonical-repo.sh) && gh … -R "$repo"`). `gh`'s default repository in a
  fork clone is usually the fork, where pull request and issue numbers mean something else, and
  an empty `-R ""` falls back to it.
- One outcome per branch. Never branch from another feature branch, and never add unrelated
  work to a branch that has an open pull request.
- Push the branch and open a ready-for-review pull request against `master`, following
  `.github/pull_request_template.md`, without waiting to be asked. This overrides a
  machine-wide agent default to wait for a request.
- The owner merges. An agent never merges and never enables auto-merge.
- Pull requests are merged with a merge commit, which is the practice so far. Benchmark stamps
  (`docs/benchmarks/benchmarks.json`) and the cross-solver scorecard name the branch commit that
  produced them; a squash or rebase merge would leave them naming a commit that is not on
  `master`.
- A direct commit to `master` needs the owner's say-so.
- A baseline, scorecard or benchmark record is re-recorded only on a committed tree, as its own
  commit after the change that earned it (`AGENTS.md` → Evidence discipline).

## Commits

Conventional Commits, with the smallest honest scope and the issue number when there is one:

```
fix(compositional): reject partial checkpoint surface reporting (#88)
```

- Types in use: `feat`, `fix`, `docs`, `test`, `refactor`, `chore`, `ci`, and `experiment` for
  test-only or reverted solver experiments.
- Scopes name the area: `fim`, `impes`, `compositional`, `fluid`, `spe5`, `benchmarks`,
  `charts`, `catalog`, `deps`, `skills`.
- Do not bundle unrelated changes. A fix found while doing something else is its own commit.
- A changed benchmark tolerance carries its written justification in the same commit.

## What a request may change

- The requested outcome sets the side effects. "Check", "find", "list" or "assess" returns
  findings and changes nothing. "Draft" or "propose" returns text. Only "fix", "build",
  "update", "sweep" or "file", or a task skill invoked by name with no limiting word, runs a
  write workflow: branch, commits, pull request, issues, labels, comments.
- An explicit scope limit wins over a skill invoked by name. A word limits the request when it
  scopes it ("check docs", "draft an issue", "report only", "read-only", "dry run"), not when it
  names something to deliver with an authorized write ("fix #104 and report the results").
- A probe that needs code changed to prove a finding runs in a scratch worktree under
  `.claude/worktrees/` (git-ignored), is never committed, and is removed afterwards.

## Security

- A suspected vulnerability is never described publicly before it is fixed: not in an issue, a
  pull request, a commit message or a comment. It is reported privately as `SECURITY.md` says
  (GitHub private vulnerability reporting, which is enabled on this repository).
- Advisories GitHub has already published (Dependabot alerts) are tracked publicly by advisory
  ID and dependency only. Whether the vulnerable code is reachable here, and code-scanning
  findings on this repository's own code, are discussed privately.
- Dismissing an alert is the owner's decision.

## Review

- A review reports a verdict and verified findings ranked `P0`–`P3` with `file:line` and a
  concrete failure. It makes no edits, commits, pushes, pull request comments or issues unless
  the owner asks for that specific thing.
- Findings go back to the implementing session, which fixes them on the same branch and answers
  each one in a pull request comment.
- A second opinion is worth most from a different model in a fresh session.

## Agent guidance

- `AGENTS.md` is the only repo-wide instruction file for every harness in use (Claude Code,
  Codex, OpenCode). `CLAUDE.md` contains only `@AGENTS.md`. Claude Code reads `AGENTS.md` by
  itself only from v2.1.277 and only while no `CLAUDE.md` or `CLAUDE.local.md` exists; the import
  makes it load in every version, under the default *Project instructions* setting, under
  `claude-md` and `claude-md-and-agents-md`, and beside a personal `CLAUDE.local.md`. The
  `managed-only` setting skips every project instruction file, the import included.
- Task and area skills live under `.claude/skills/`. `.agents/skills` is a symlink to that
  directory, because Codex scans only `.agents/skills`. `.claude/skills/README.md` is the
  maintenance guide.
- `docs/AGENT_TASK_PLAYBOOK.md` is the owner's guide to starting each kind of task.
- When a change moves a fact that `AGENTS.md` or a skill states, update that file in the same
  pull request.
