---
name: shape-issue
description: Turn a rough idea, bug report, benchmark signal or review finding into an implementation-ready GitHub issue for ResSim - evidence, bounded scope, measurable acceptance, the validation gate, real blockers, and the decisions the owner must make. Use when asked to shape, draft, write up, refine, split or groom an issue, or to make an existing issue ready for work. Writes no code; use work-issue to implement.
---

# Shape An Issue

The output is one GitHub issue (or a parent with slices) that another agent could implement with
the `work-issue` skill without asking a question. You write no code, create no branch and open
no pull request. Tracking rules: `docs/DEVELOPER_WORKFLOW.md` → Tracking.

## 1. Find out what already exists

- Search open and closed issues: `gh issue list --search "<terms>" --state all`. A duplicate
  gets a comment with the new evidence, not a second issue.
- Check whether it already ships (`README.md` → Implemented Capabilities and Scenario
  Inventory), where it sits in `ROADMAP.md` and `docs/CASE_LIBRARY_ROADMAP.md`, and whether it
  was deliberately not done (`docs/OPEN_ITEMS_2026-09-21.md`).
- For a FIM convergence idea, search `docs/FIM_EXPERIMENT_REGISTRY.md` by mechanism name. A lever
  already listed is not reopened unless its `Retry only if` condition holds.
- Check it against the roadmap's principles and `docs/SCIENTIFIC_LIMITATIONS.md`. An idea that
  widens the validated envelope without a consuming case and an independent oracle is a proposal
  for the owner, not a work item; say so in the issue.

## 2. Gather evidence

- A bug: reproduce it, or trace it in the code. Record the scenario or command, the revision,
  the wrong result, the expected result and `file:line`. Say plainly when you could not
  reproduce it.
- A number: give the commit hash, the exact command and the output. A dirty-tree measurement is
  provisional and is labelled so.
- A feature or improvement: the gap a user meets, how the current code behaves, and the existing
  owner the work would extend.

## 3. Choose the form and bound it

- Implementation with a known outcome uses `.github/ISSUE_TEMPLATE/work-item.yml`. Work whose
  outcome depends on a hypothesis uses `scientific-investigation.yml` and the `research` label:
  hypothesis, reproduced baseline, oracle and observables, coupled semantics held constant,
  verdict criteria.
- One issue is one reviewable pull request. Larger work becomes a parent issue with slices; each
  slice leaves every gate green and is useful by itself.
- State what is deliberately held constant or left out, and why.
- A blocker is a real prerequisite with its reason. A later roadmap position is not a blocker.
  Use the `blocked` label only when you can name the missing prerequisite and what would
  unblock the work.

## 4. Separate decisions from facts

List every decision that is the owner's: scope of the validated envelope, a tolerance or
acceptance band, a scientific claim shown to users, a trade-off with a lasting cost, a change to
a rule in `AGENTS.md`. Give each one your recommendation and the reason. Do not settle one
silently by wording the acceptance around it. Decide the small things yourself and say what you
chose.

## 5. Write it

For a work item, the template's fields:

- **Outcome** — the observable result when the issue is complete.
- **Current behavior and evidence** — from step 2.
- **Scope and constraints** — in scope, held constant, blockers. Link the plan or validation
  document; do not copy its narrative.
- **Acceptance criteria** — checkboxes, each observable and measurable. "Matches OPM" is not
  acceptance; "final cumulative oil within 1 % of the Flow artifact on `wf_bl1d`, gated by a
  named test" is.
- **Validation gate** — the exact commands from the `ressim-validation` decision table.

Add **Decisions for the owner** when step 4 found any.

## 6. File it

- `gh issue create` with one `priority:P0`–`priority:P3` label and at least one `area:*` label
  (`area:engine`, `area:frontend`, `area:scenario`, `area:validation`, `area:docs`,
  `area:release`); add `bug`, `enhancement`, `research` or `blocked` when they apply.
- No milestone, and no edit to the order in `ROADMAP.md`: creating an issue never changes
  release scope or sequence. Both are the owner's.
- To make an existing brief issue ready, rewrite its body with
  `gh api -X PATCH repos/{owner}/{repo}/issues/<n> -F body=@<file>` and add a comment saying
  what changed. Keep the original problem statement.
- Report the issue URL, the decisions waiting on the owner, and whether the issue is ready for
  `work-issue` now or only after those decisions.

## Ready means

A different agent with no access to this conversation can pick the issue up, knows what to
build, knows what not to build, knows which gate proves it, and can tell when it is done.
