---
name: drift-sweep
description: Maintenance sweep for things in ResSim that have gone stale - blocked GitHub issues whose prerequisite is now met, roadmap and tracking entries that no longer match reality, docs and agent guidance that disagree with the code, and tests that are ignored, skipped, vacuous or selected by no gate. Use when asked to check for stale or drifted docs, roadmap, comments, tests or blocked issues, or to run a maintenance or hygiene sweep.
---

# Drift Sweep

Compare what the repository claims with what is true, and close the gap. Run the scopes named in
the prompt (`blocked`, `roadmap`, `docs`, `tests`); with none named, run all four in that order.

The request decides how far this goes (`AGENTS.md` → Task modes). Asked only to check, list or
report: make no changes anywhere — no branch, no edit to the checkout, no label change, no
comment, no issue — and report what a full sweep would do. Any probe that needs code changed (a
test broken on purpose, a behaviour disabled) runs in a scratch worktree under
`.claude/worktrees/`, is never committed, and the worktree is removed afterwards. Invoked by
name, or asked to sweep, fix or clean up: the full workflow below. An explicit limit wins over
the invocation: `$drift-sweep check docs` is a check.

Two rules hold for every scope of a full sweep:

- **Correct facts, never decide.** A sweep fixes a status, a link, a path, a count. It does not
  reorder the roadmap, change scope, change a tolerance, or pick between a document and the code
  when they disagree about behaviour or about a number. Those go to the owner as an issue or a
  question.
- **Changes to tracked files go in one pull request; the rest become issues.** A sweep that
  changes files is a `work-issue` task: branch from the canonical `master` and finish with an
  open pull request. A sweep whose only changes are on GitHub (the `blocked` scope's comments
  and label removals, issues it filed) opens no branch and no pull request, and finishes with
  the link to each comment, label change and issue. A sweep that changes nothing says what it
  checked. Something that looks like a
  security vulnerability is neither fixed publicly nor filed; it goes to the owner in the
  session under `SECURITY.md`.

## `blocked` — blocked issues

```sh
repo=$(bash scripts/canonical-repo.sh) && gh issue list -R "$repo" --label blocked --state open --limit 500 --json number,title,updatedAt
```

Every `gh` command in this skill is one self-contained line that resolves the canonical
repository first (`scripts/canonical-repo.sh`), so each scope can run on its own, in a new
shell. Every `gh ... list` defaults to 30 results. Pass `--limit` (or `--paginate` for `gh api`) on
every survey, compare the count you got with the limit, and judge the call by `gh`'s own exit
status: a list that failed is unchecked, not empty.

For each one, read the prerequisite it names and check it: did the issue it waits on close, did
the enabler merge, has the evidence arrived, is the upstream release published?

- **Unblocked, verifiably** (a closed issue, a merged pull request, a published version,
  recorded evidence): comment with the evidence and its link, remove the `blocked` label, and
  list it in your report. Do not start the work.
- **Probably unblocked, a judgement call**: comment with what changed, leave the label, and ask
  the owner in your report.
- **Still blocked**: no comment. Silence is not noise.
- **Names no prerequisite**: comment asking for one; the label requires it.

## `roadmap` — tracking against reality

- `ROADMAP.md`: entries in the current execution order whose issue is closed or whose pull
  request merged; "blocked on" statements whose blocker closed; dated statements that expired.
- `README.md` → Near-Term Focus and `TODO.md` against the roadmap they point at.
- `docs/OPEN_ITEMS_2026-09-21.md`: a deferral that has since been done.
- `docs/DOCUMENTATION_INDEX.md` → Current repo-level facts (scenario, artifact and withheld
  counts) against `src/lib/catalog/scenarios.ts` and `src/lib/catalog/opm-flow-results/`.
- Open issues whose acceptance is fully met, and closed issues with an unmet acceptance item.
  Open issues missing a `priority:` or `area:` label.
- Issue numbers cited in active docs that point at closed or retitled issues.

Fix statuses, counts and links. Report anything that implies a change of order.

## `docs` — documents and guidance against the code

- Commands in `README.md` → Validate and in `docs/DEVELOPER_WORKFLOW.md`: every script and
  `pnpm` script they name exists and does what the text says.
- File paths, function names and test names cited in `AGENTS.md` and under `.claude/skills/`
  (reference files included) still exist. This is mechanical: extract the backticked names and
  look each one up.
- Counts and lists a skill states (gates per bucket, diagnostic presets, CI steps, OPM cases)
  against the script or workflow that owns them.
- A rule a skill restates still matches `AGENTS.md` and its owning document.
- Each `SKILL.md` is still lean (`.claude/skills/README.md`).
- Generated pages are in sync: `bash scripts/benchmarks.sh render --check`. Never hand-edit
  `docs/BENCHMARKS.md` or copy a number out of it.

Leave these alone: anything under `.archive/`, dated plans, reviews and experiment notes
(`*_2026-MM-DD.md`), `docs/FIM_CONVERGENCE_WORKLOG.md`, and past rows of
`docs/FIM_EXPERIMENT_REGISTRY.md`. They are records, and being out of date is their job. A
measured baseline in a current document is never "corrected" by a sweep: a number is replaced
only by a re-measurement on a committed tree (`AGENTS.md` → Evidence discipline).

## `tests` — tests that no longer earn their place

- **Rust tests no gate selects.** A test that matches no filter in
  `scripts/validate-solver-coverage.sh` or `scripts/validate-compositional.sh` runs nowhere,
  because full `cargo test` is not a gate. List the test names
  (`cargo test --manifest-path src/lib/ressim/Cargo.toml -- --list`) and compare them with the
  filters. This is the omission class that hid two failures here (#13).
- `#[ignore]`d Rust tests and skipped vitest tests (`it.skip`, `test.skip`, `describe.skip`,
  `.todo`): is the stated reason still true, is the issue it cites still open? A slow release
  replay is ignored on purpose and stays ignored. File an issue when a test that should run now
  fails.
- Tests that cannot fail: an assertion that holds for an empty result, a tolerance wider than
  the effect, a check on a builder's output that never reaches the engine.
- Tests that fail intermittently in CI:
  `repo=$(bash scripts/canonical-repo.sh) && gh run list -R "$repo" --workflow pr-tests.yml --status failure --limit 100`,
  then group by failing step.
- Comments and test names that cite an issue as pending when it is closed.

A test you believe is vacuous is proved so before you change it: break the behaviour it claims
to cover and watch it still pass. Do that in a scratch worktree, never in the checkout, and
remove it afterwards:

```sh
git worktree add --detach .claude/worktrees/probe-<name> HEAD
```

Work only inside that worktree, with `git -C` or absolute paths, and commit nothing. When you
are done, and only if the `add` above succeeded, remove it:
`git worktree remove --force .claude/worktrees/probe-<name>`. If the `add` failed because the
path exists, pick another name; never remove a worktree you did not create.
 A test whose fix would tighten or loosen a tolerance is an
issue, not a sweep fix.

## Report

One table per scope: what was checked, what was fixed in the pull request, what was filed, what
needs the owner. "Nothing found" is a valid result for a scope when the report says what was
checked.
