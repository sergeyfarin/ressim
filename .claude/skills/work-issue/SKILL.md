---
name: work-issue
description: End-to-end procedure for implementing a GitHub issue, bug fix, feature or experiment in ResSim - branch from the canonical master, build, validate, look for further defects nearby, then open a pull request without being asked. Use when asked to work on, implement, fix, build or deliver an issue, bug, feature or slice. Not for reviewing a pull request; use review-pr for that.
---

# Work An Issue

An implementation task ends with a pushed branch and an open, ready-for-review pull request. Do
not stop to ask whether to commit, push or open it: that is the standing instruction for this
repository, and it overrides any machine-wide default to wait for a request. Never merge and
never enable auto-merge; the owner merges. Governing rule: *Branches and pull requests* in
`docs/DEVELOPER_WORKFLOW.md`.

## 1. Understand before changing

- Read the issue with its comments
  (`repo=$(bash scripts/canonical-repo.sh) && gh issue view <n> -R "$repo" --json number,title,state,labels,body,comments`;
  a plain `gh issue view` fails in the `gh` installed here), the plan or validation document it
  links, and where it sits in `ROADMAP.md`.
- Implementation needs an outcome, bounded scope, measurable acceptance and a named validation
  gate (the fields of `.github/ISSUE_TEMPLATE/work-item.yml`). A `research` issue also needs its
  oracle and verdict criteria (`scientific-investigation.yml`). Stop and ask one question, with
  your recommendation, when acceptance is missing, when the issue contradicts `AGENTS.md` or an
  authoritative document, or when it waits on an owner decision. Decide anything smaller yourself
  and record it in the pull request.
- Load the area skills for the code you will touch from the table in `AGENTS.md`.
  `ressim-validation` applies to every task.

## 2. Branch

```sh
base=$(bash scripts/canonical-repo.sh --remote) &&
git fetch "$base" &&
git switch -c <type>/<issue>-<short-name> "$base/master"
```

The `&&` chain stops at the first failure in any shell, so a failed fetch never leaves you on a
branch cut from a stale `master`. Every `gh` call in this task names the canonical repository
in the same command, `repo=$(bash scripts/canonical-repo.sh) && gh … -R "$repo"`: `gh`'s
default repository in a fork clone is the fork, and each tool call starts a new shell.

Always cut from the latest canonical `master`. In a fork that is `upstream/master`, because the
fork's own `master` can be behind; in a direct clone it is `origin/master`. Never cut from another
feature branch, and never onto a branch that has an open pull request for other work. If the
working tree holds changes that are not yours, stop and ask. `<type>` is the commit type
(`fix`, `feat`, `docs`, `test`, `chore`, `ci`, `exp` for an experiment).

## 3. Build

- Make the smallest root-cause change that meets the acceptance. One outcome per pull request.
- For a bug: write the failing named test, see it fail, then fix.
- New physics needs an oracle (analytical solution, conservation check or finite-difference
  check), not "runs without crashing".
- A new Rust test must be selected by a filter in `scripts/validate-solver-coverage.sh` or
  `scripts/validate-compositional.sh`, or it runs in no gate.

## 4. Look around — required, not optional

While you are in the code, look for defects beyond the task: bugs, tests that cannot fail, a
test no gate selects, missing tests on a risky path, duplicated helpers, code smells, stale
documentation, an easier design. Look at what you read, not only at what you change. Then give
every finding one of two outcomes.

**Fix it in this pull request** when all of these hold:

- It is in, or directly beside, the code this task already touches.
- It is small: a reviewer can verify it at a glance, roughly 30 changed lines in one module.
- It needs no new dependency, no change to a payload shape (`api.rs`, worker messages, the
  `Scenario` type, saved state) and no owner decision.
- The gate you are already running covers it, with a named test added when it changes behaviour.
- **It cannot move a number.** It does not change what the engine computes in a run, an
  analytical formula, a scenario's `params`, a tolerance or acceptance band, a benchmark record,
  the cross-solver scorecard, a reference deck or an OPM artifact.

Commit it separately from the task (`fix(...)`, `refactor(...)`, `test(...)`, `docs(...)`) and
list it under *Also fixed* in the pull request.

**File a GitHub issue** for everything else:

- Search first
  (`repo=$(bash scripts/canonical-repo.sh) && gh issue list -R "$repo" --search "<terms>" --state all --limit 100`). Add new evidence
  to an existing issue as a comment; do not open a duplicate.
- Write it with the fields of `.github/ISSUE_TEMPLATE/work-item.yml`: outcome, current behavior
  and evidence (`file:line`, revision, reproduction), scope, acceptance, validation gate. A
  finding whose outcome depends on a hypothesis uses `scientific-investigation.yml` and the
  `research` label.
- Label it with one `priority:P0`–`priority:P3` and at least one `area:*`. Do not set a
  milestone and do not edit the order in `ROADMAP.md`.
- Link it under *Follow-up issues* in the pull request.

A suspected security vulnerability is the exception to both outcomes: do not fix it in a public
pull request and do not file it. Do not describe it in a commit message or a comment either.
Report it to the owner in the session and follow `SECURITY.md`.

A finding that changes a simulated or analytical result, solver convergence behaviour, a
tolerance, a benchmark record or a scientific claim in a scenario **always gets an issue, even
when the fix is one line**. Fix it in this pull request only when the task cannot be completed
without it; then it is part of the task and carries the task's full gates. Report any P0 or P1
finding to the owner in your final message as well.

Nothing stays only in the conversation: every finding ends as a commit in this pull request or
as an issue. A deliberate "not doing this" goes in `docs/OPEN_ITEMS_2026-09-21.md` with the
reason. Do not start a second feature or a broad refactor.

## 5. Validate and document

- Run the gates the `ressim-validation` decision table names for what changed. Full
  `cargo test` is not a gate. Anything that can move an answer also runs
  `bash scripts/validate-cross-solver.sh` and `bash scripts/benchmarks.sh check`; say so plainly
  when OPM Flow is not installed and they could not run.
- A baseline, scorecard or benchmark record is re-recorded only on a committed tree
  (`AGENTS.md` → Evidence discipline): commit the change, then run `--update` / `update` and
  commit the result as its own commit on the same branch.
- Put evidence where it belongs: the owning validation document, the FIM registry and worklog,
  the scenario inventory in `README.md`, `docs/DOCUMENTATION_INDEX.md` for a new document. When
  the change moves a fact a skill states, update the skill in the same pull request.

## 6. Ship

- Commit with Conventional Commits (`docs/DEVELOPER_WORKFLOW.md` → Commits), then push and open
  the pull request against the canonical repository, in one strict subshell:

  ```sh
  (
    set -euo pipefail
    repo=$(bash scripts/canonical-repo.sh)
    dest=$(bash scripts/canonical-repo.sh --push origin)
    ids='[.id, .owner.id, (.source.id // .id), .owner.login] | @tsv'
    r=$(gh api "repos/$repo" --jq "$ids")
    d=$(gh api "repos/$dest" --jq "$ids")
    read -r repo_id repo_owner repo_network _ <<<"$r"
    read -r dest_id dest_owner dest_network dest_login <<<"$d"
    if [ "$dest_network" != "$repo_network" ]; then
      echo "not pushing: $dest is neither $repo nor a fork of it" >&2; exit 1
    fi
    if [ "$dest_id" != "$repo_id" ] && [ "$dest_owner" = "$repo_owner" ]; then
      echo "not pushing: $dest is a fork owned by $dest_login; ask the owner" >&2; exit 1
    fi
    branch=$(git branch --show-current); [ -n "$branch" ]
    git push -u origin "$branch"
    gh pr create -R "$repo" --base master --head "$dest_login:$branch" \
      --title "<title>" --body-file <file>
  )
  ```

  Everything is checked before the push. `dest` is the one repository `git push origin`
  writes to: it honours `remote.origin.pushurl` and `pushInsteadOf`, and the helper fails when
  there are several push URLs naming different repositories. The checks compare GitHub's
  numeric IDs, never names, because GitHub names are case-insensitive (`sergeyfarin/ResSim` is
  `sergeyfarin/ressim`). `dest` must be the canonical repository or a fork in its network (same
  `source` ID), because a pull request can only come from there. A different repository owned
  by the canonical repository's owner (same owner ID) is refused: GitHub then needs the head
  repository by name, which `gh pr create` cannot pass. The head owner is the login GitHub
  reports, in GitHub's own spelling. In a fork, `origin` is the fork; the
  branch goes there and the pull request is opened on the canonical repository, from
  `<fork-owner>:<branch>`. Issues are read and filed there too
  (`repo=$(bash scripts/canonical-repo.sh) && gh issue create -R "$repo" …`).
- The pull request is ready for review and follows `.github/pull_request_template.md`. The body
  states:
  - what changed and why;
  - `Closes #<n>` only when every acceptance item is met, otherwise `Refs #<n>` with what
    remains;
  - each gate you ran with its result, verbatim where it is short, and what you did not run;
  - *Also fixed* and *Follow-up issues* from step 4;
  - each decision you made on the owner's behalf.
- Finish by reporting the pull request URL, the validation result and the list of findings with
  their outcome.

## Review rounds

Review findings come back through the owner. Fix them on the same branch, push, and add one pull
request comment saying what changed for each finding and which tests now cover it. Disagree in
that comment, with evidence, rather than silently skipping a finding.
