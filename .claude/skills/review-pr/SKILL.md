---
name: review-pr
description: Review-only procedure for a ResSim pull request - judge direction, owner decisions, approach, implementation, tests, evidence and docs, and report ranked findings without changing anything. Use when asked to review, re-review, check, assess or give an opinion on a pull request, branch or diff. Never fixes, commits or pushes; use work-issue to implement.
---

# Review A Pull Request

Review only. You change nothing: no edits to the branch, no commits, no pushes, no pull request
comments or approvals, no new issues. Do one of those only when the owner asks for that specific
thing in this session. When a fix is obvious, describe it; do not make it. The owner passes your
findings to the agent that wrote the change.

The reason is independence. A reviewer that fixes stops being a second opinion, and its fixes
reach `master` unreviewed.

## Prepare

- Resolve the pull request's repository first, then read it and record the head commit you are
  reviewing. Given a URL, the repository comes from the URL. Given a bare number, it is the
  canonical repository (`upstream` when that remote exists, otherwise `origin`), never `gh`'s
  default, which in a fork clone is the fork:
  ```sh
  pr=<number or pull request URL>
  if [[ $pr == https://github.com/*/pull/* ]]; then
    repo=$(cut -d/ -f4,5 <<<"$pr"); n=$(cut -d/ -f7 <<<"$pr"); n=${n%%[!0-9]*}
  else
    if git remote get-url upstream >/dev/null 2>&1; then base=upstream; else base=origin; fi
    repo=$(git remote get-url "$base" | sed -E 's#^(https://github\.com/|git@github\.com:|ssh://git@github\.com/)##; s#\.git$##')
    n=$pr
  fi
  gh pr view "$n" -R "$repo" --json title,body,baseRefName,commits,files
  sha=$(gh pr view "$n" -R "$repo" --json headRefOid --jq .headRefOid)
  gh pr diff "$n" -R "$repo"
  gh pr checks "$n" -R "$repo" || echo "checks: failing, pending or none (exit $?)"
  ```
  Every later command uses this `repo`, `n` and `sha`. Everything below reviews `$sha`. If the
  pull request moves on during the review, say so and start again from this step.
- Find the issues it closes. `closingIssuesReferences` is not a `gh pr view` field in the `gh`
  installed here (2.46), so ask GraphQL, on the same repository:
  ```sh
  gh api graphql -F owner="${repo%/*}" -F name="${repo#*/}" -F n="$n" -f query='
    query($owner:String!,$name:String!,$n:Int!){repository(owner:$owner,name:$name){
      pullRequest(number:$n){closingIssuesReferences(first:50){nodes{number title state}}}}}' \
    --jq '.data.repository.pullRequest.closingIssuesReferences.nodes[]'
  ```
  Also read the `Closes` / `Refs` lines in the body; they can disagree.
- Read the linked issue with its acceptance, the plan or validation document it cites, and where
  the work sits in `ROADMAP.md`.
- Get the code without disturbing the working tree. Fetch from that same repository, not from
  `origin`, and check that you got the commit GitHub reports:
  ```sh
  if git fetch "https://github.com/$repo.git" "pull/$n/head" \
     && [ "$(git rev-parse FETCH_HEAD)" = "$sha" ]; then
    git worktree add --detach ".claude/worktrees/review-$n" "$sha"
  else
    echo "fetch failed or wrong commit: do not review" >&2; false
  fi
  ```
  The worktree is created only when the fetch succeeded and returned `$sha`. In a fork, `origin`
  is the fork and its pull request with the same number is a different pull request. Never
  review a commit other than `$sha`.
- Use `.claude/worktrees/` (git-ignored); a worktree under `/tmp` disappears on reboot. Run
  every command there with `git -C` or an absolute path, so a failed `cd` cannot run it in the
  main checkout. Run tests and builds there. A throwaway probe test that confirms a suspected
  bug is allowed there and is never committed. Remove the worktree when done:
  `git worktree remove --force ".claude/worktrees/review-$n"`.
- Read the whole changed files and their callers, not just the hunks. Review the entire pull
  request, every commit, not only the latest push.
- On a re-review, first check each earlier finding against the new commits (fixed, not fixed,
  fixed differently), then review the new changes as fresh code.

## What to judge, in this order

1. **Direction.** Should this be built, now, in this shape? Check it against the principles and
   order in `ROADMAP.md`, the validated envelope in `docs/SCIENTIFIC_LIMITATIONS.md`, the rules
   in `AGENTS.md` and the issue's acceptance. Owner decisions are in scope: when a decision
   recorded in the issue, plan or pull request looks wrong or costly, say so once, plainly, with
   the evidence and the alternative. Do not defer to a decision because the owner made it.
2. **Approach.** Is this the right design for the goal? A root-cause change or a symptom patch;
   reuse of an existing owner (the analytical-method registry, `crate::math`, `api.rs`) or a
   second copy; a simpler alternative; what a new payload field, scenario field or solver option
   commits the project to; for an experiment, whether the oracle can measure the hypothesis at
   all.
3. **Implementation.** Find the bugs. Trace real inputs through the changed code: units (bar,
   m, day, mD, cP); surface against reservoir volumes; both solvers, IMPES and FIM; the legacy
   and the AD twin of a formula; native against wasm (`crate::math`, no `cfg(target_arch)`); a
   value sitting exactly on a clamp; a one-cell and a 1D grid; rate control, BHP control and the
   switch between them; a rejected timestep attempt and its rollback; a checkpoint restore; a
   `$state` proxy sent to the worker; a chart with no run yet; a scenario with
   `analyticalMethod: 'none'`. Apply the area skills for what the diff touches.
4. **Tests and evidence.** Would each new test fail without the change? Is each new Rust test
   selected by a gate filter, or `#[ignore]`d and run nowhere? Is a tolerance loosened, and is
   the written justification in the same commit? Does each stated baseline carry its commit
   hash, command and output, from a committed tree? Is a verdict worded correctly
   (`INCONCLUSIVE` is not `REFUTED`)? Verify the pull request's validation claims by running the
   narrowest relevant gate yourself, and say what you ran.
5. **Docs and tracking.** The owning validation document, the FIM registry, the scenario
   inventory in `README.md`, generated pages (`benchmarks.sh render --check`), the skill that
   states a fact this change moved, and whether `Closes` is honest about the issue's acceptance.
6. **Scope.** Unrelated changes, leftover debugging, commits that mix concerns, a number that
   moved without being mentioned.

## What counts as a finding

- It is verified. You read the code path or ran it. Anything you could not verify is a question,
  labelled as one.
- It states a priority, `file:line`, the concrete failure (this input gives that wrong result),
  why it matters, and the direction of a fix in words or a few lines.
- Priorities match the repository's `priority:` labels:
  - **P0** — a wrong number shown to users as validated; silently wrong physics; a gate that
    reports success without running; the app does not load; a security hole.
  - **P1** — wrong behaviour in a shipped scenario or solver path; a violated `AGENTS.md` rule
    (payload boundary, platform math, solver routing, tolerance change without justification);
    an evidence claim that does not reproduce.
  - **P2** — a real defect with limited reach; a risky path without a test; a test no gate
    runs; a design problem that will cost soon.
  - **P3** — minor: naming, polish, a simplification.
- No padding. No praise, no summary of the diff, no style points `cargo fmt` or eslint own.
  "No findings" is a valid review when you say what you checked.

## Report

In the session, unless the owner asks for it to be posted. A suspected security vulnerability is
never posted, whatever the request says: not in a review, a comment or an issue. It goes to the
owner in the session, who chooses the private route in `SECURITY.md`.

1. **Verdict** — one of *merge*, *merge after fixes* (name the priorities), *rework*,
   *do not build* — with the reason in a sentence or two.
2. **Direction and decisions** — including any owner decision you would change.
3. **Findings** — most severe first.
4. **Questions for the owner** — decisions only the owner can make.
5. **Noticed outside this pull request** — candidate follow-up issues, described, not filed.
6. **What I checked and ran, and what I did not.**
