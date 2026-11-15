# Agent task playbook

This is the owner's guide to directing coding agents in this repository: which kinds of task
exist, how to start each one, and what comes back. Agents do not need this file. Their rules are
in `AGENTS.md` and the skills under `.claude/skills/`; where this guide and a skill differ, the
skill is what the agent follows.

## Starting a task

Every harness used here loads `AGENTS.md` (Claude Code through the one-line `CLAUDE.md` that
imports it) and the same twelve skills. How to check each harness is in
`.claude/skills/README.md`. Start the prompt with the task skill and the issue or pull request
number:

| Harness | Prefix | Example |
|---|---|---|
| Claude Code | `/` | `/work-issue 104` |
| Codex | `$` | `$work-issue 104` |
| OpenCode | `@` | `@work-issue 104` |

- Your wording sets what the agent may change. "Check", "find", "list" or "assess" gets a
  report and changes nothing. "Draft" gets text. "Fix", "build", "update", "sweep" or "file" —
  or the skill invoked by name — runs the full workflow: branch, commits, pull request, issues.
- Suspected security vulnerabilities come back to you in the session, never as a public issue,
  pull request text, commit message or comment. You choose the private route in `SECURITY.md`.
- One task per session. A session that implements should not also review.
- Add a sentence after the number when the task needs steering: the slice to take, a
  constraint, a decision already made.
- Name an area skill too when the area is not obvious from the issue, for example
  `/work-issue 104 with fim-solver-debug`.
- An agent that was not given a task skill still follows `AGENTS.md` → Task modes, so a plain
  "fix #104" works. The prefix makes it certain, which matters most with smaller models.

## Task skills

These six say how a session runs from start to finish.

| Skill | Start it when | It ends with |
|---|---|---|
| `work-issue` | An issue, bug or feature is ready to build | An open pull request, plus issues for what it found |
| `review-pr` | A pull request needs a second opinion | A verdict and ranked findings; nothing changed |
| `shape-issue` | An idea, bug report or benchmark signal is not yet buildable | A ready GitHub issue and the decisions it needs |
| `update-dependencies` | Updates or security alerts have piled up | One pull request with every update |
| `drift-sweep` | Docs, roadmap, tests or blocked issues may be stale | A small pull request, issues, and a report |
| `sweep-bug-class` | A fixed bug could exist elsewhere | Every instance found, fixed or filed, and a guard |

### `work-issue` — implement

```
/work-issue 104
```

The agent reads the issue, branches from the latest canonical `master`, builds the change, runs
the gates the `ressim-validation` skill names, and opens a ready-for-review pull request without
asking. It never merges.

While it works it looks for other defects in the code it reads. A small, local one that cannot
move a number is fixed in the same pull request as its own commit and listed under *Also fixed*.
Anything larger becomes a GitHub issue listed under *Follow-up issues*. So does anything that
changes a simulated or analytical result, solver convergence, a tolerance or a benchmark record,
even when the fix is one line.

- It stops and asks when the issue has no usable acceptance or depends on a decision you have
  not made. Run `shape-issue` first to avoid that.
- The cross-solver gate and the full benchmark check need OPM Flow on the machine. The pull
  request says plainly when they could not run.
- Review findings go back to the same session. It fixes on the same branch and answers each
  finding in one pull request comment.

### `review-pr` — review only

```
/review-pr 136
```

The agent reviews direction, your own decisions, approach, implementation, tests and evidence,
docs and scope, in that order. It reports a verdict (*merge*, *merge after fixes*, *rework*,
*do not build*) and verified findings ranked `P0`–`P3` with `file:line`. It edits nothing,
commits nothing, and files nothing.

- Use a different model from the one that wrote the pull request, in a fresh session. The value
  is a second opinion with no memory of the implementation.
- Paste the report into the implementing session.
- Ask for a re-review after fixes: `/review-pr 136 re-review`. It checks each earlier finding,
  then reviews the new changes as fresh code.
- To have the review posted on GitHub, say so in the prompt. The default is a report in the
  session.

### `shape-issue` — make work buildable

```
/shape-issue the gas-rate deck stays 2.7 % above Flow after refinement
/shape-issue 107
```

The agent searches for duplicates, checks the roadmap, the open-items list and (for FIM) the
experiment registry, gathers evidence, bounds the work to one reviewable pull request or a
parent with slices, and files the issue with measurable acceptance and a named validation gate.
Ask for a *draft* to get the text back without anything being filed or commented on.
A question whose answer depends on a hypothesis is filed as a scientific investigation with its
oracle. It writes no code, sets no milestone and does not touch roadmap order; both are yours.

- The report lists **decisions for the owner**, each with a recommendation. Answer them on the
  issue before running `work-issue`.
- Give it an existing issue number to turn a brief issue into a ready one.

### `update-dependencies` — upgrades and security

```
/update-dependencies
```

The agent surveys open Dependabot pull requests and GitHub's Dependabot, code-scanning and
secret-scanning alerts, updates pnpm packages, Rust crates, the `uv` tool environment and GitHub
Actions in one branch, runs the gates and opens one pull request.

- Anything it cannot move is held with the reason and tracked on an issue.
- Every published advisory ends as fixed, not fixable yet (with an issue naming the advisory ID)
  or believed not applicable. Code-scanning findings on our own code, and whether a vulnerable
  dependency is reachable here, come to you in the session, not into the pull request. It does
  not dismiss alerts; that is yours.
- It links the Dependabot pull requests it covers (`Covers #n`) and leaves them open;
  Dependabot closes each once its update is on `master`.
- "Check the security warnings" gets the survey only, with no updates.
- A feed it could not read is reported as unchecked, never as empty.
- Not routine here, and treated accordingly:
  - `three` is pinned and stays pinned unless you ask.
  - `libm`, `faer`, `nalgebra`, `sprs` and `rand` sit in the numerical path. Moving one is an
    engine change with the cross-solver gate and the benchmark check.
  - `wasm-bindgen` moves together with its CLI; `pyo3` moves together with `pythonize`.
  - JutulDarcy, Julia and OPM Flow are reference simulators. Upgrading one is a re-baselining
    task of its own, never a side effect.

### `drift-sweep` — find what went stale

```
/drift-sweep
/drift-sweep blocked
/drift-sweep docs tests
```

Four scopes; name the ones you want or run all four. "Check for drift" or "list what is stale"
gets the report only: no branch, no label change, no comment.

| Scope | Checks |
|---|---|
| `blocked` | Each `blocked` issue's named prerequisite. Verifiably unblocked: it comments with the evidence, removes the label and tells you. It does not start the work. |
| `roadmap` | `ROADMAP.md`, the README's near-term focus, the open-items list and issue status against what actually merged. |
| `docs` | Commands, paths, test names and counts in the docs, `AGENTS.md` and the skills against the code. |
| `tests` | Rust tests no gate selects, ignored or skipped tests whose reason is gone, tests that cannot fail, flaky tests. |

It corrects facts and never decides: it will not reorder the roadmap, change scope, replace a
measured number, touch a tolerance, or choose between a document and the code when they
disagree about behaviour. Those come back to you. Dated plans, reviews and the FIM worklog are
records and are left alone.

### `sweep-bug-class` — find every copy

```
/sweep-bug-class the bug fixed in #136
/sweep-bug-class platform math in the engine
check whether the bug fixed in #136 exists anywhere else
```

The first two run the full sweep. The third is a check: it reports every hit with a verdict and
changes nothing; a probe test it needs is written in a scratch worktree and removed.

Give it a pull request that just fixed something, a class from the table in
`.claude/skills/sweep-bug-class/SKILL.md`, or a pattern in words. It searches the whole tree for
the pattern, counts and classifies every hit, proves the real ones with a failing test, fixes
the small ones that cannot move a number, files the rest, and adds a guard where the pattern can
be detected mechanically.

- Run it after any bug whose cause was a copyable shape: a formula with a twin, a clamp, a
  duplicated helper, a test module no gate selects.
- In the engine most instances change a number, so expect a proved list and issues more often
  than a large diff.
- The report includes the exact search commands, so the sweep can be rerun.

## Area skills

These six hold the rules for a part of the codebase. Agents load them from the routing table in
`AGENTS.md`; you rarely need to name them.

| Skill | Covers |
|---|---|
| `ressim-validation` | Which gate to run for which change; the Rust buckets, the cross-solver gate, benchmark records and CI in reference files |
| `engine-physics-change` | Units, module map, the dual-implementation trap, test placement, the payload boundary; `compositional.md` beside it for the compositional engine |
| `fim-solver-debug` | Oracle and verdict rules, the diagnostic runner, the control matrix, the checklist of levers already tried |
| `frontend-architecture` | Stores, worker, the chart stack and its registry, the 3D view, Svelte rules |
| `add-scenario` | The scenario checklist, sensitivities, analytical wiring, the quality bar |
| `opm-reference-pipeline` | Decks, artifacts, the vector contract, deck pitfalls |

## Tasks worth practising that have no skill

These are prompts, not procedures. Each is one session.

- **Challenge a decision before it is built.** "Here is my plan for X. Argue against it, name
  the cheapest alternative, and say what evidence would change your mind. Do not implement." Use
  a different model from the one that will build it.
- **Audit an oracle before an experiment.** "For this hypothesis, say what the oracle measures,
  whether both sides report comparable quantities, and what result would be `INCONCLUSIVE`
  rather than `REFUTED`. Do not run the experiment." This is the cheapest way to avoid a
  retracted finding.
- **Act on a benchmark signal.** "Read the Signals block of `docs/BENCHMARKS.md` and the pinned
  nightly issue. For each flagged gap, say whether it is explained, tracked, or new, and shape an
  issue for each new one." It pairs with `shape-issue`.
- **Verify in the running app.** "Build, run `bash scripts/smoke-preview.sh`, then open
  `<scenario>` in a browser test and report what the charts show." Tests passing and the
  feature working are different claims.
- **Ask which tests cannot fail.** "For `<module>`, list the tests that would still pass if the
  behaviour they name were broken. Prove each one by breaking it. File issues." This is the
  `tests` scope of `drift-sweep` aimed at one module.
- **Hold a retrospective after a hard review.** "This pull request took several review rounds.
  What class of bug was each finding? Add any new class to the table in `sweep-bug-class`."

## A workable rhythm

A suggestion, not a rule.

- **Per feature:** `shape-issue` → answer its decisions → `work-issue` → `review-pr` with
  another model → fixes → merge.
- **Per experiment:** an oracle audit first, then `work-issue` on a `research` issue; the
  verdict goes in the registry or the owning validation document whatever it is.
- **After a bug fix with a copyable cause:** `sweep-bug-class`.
- **Monthly:** `update-dependencies` and `drift-sweep blocked`.
- **Before a release or a re-baseline:** a full `drift-sweep`.

## Merging

You merge. Use a merge commit: benchmark stamps and the cross-solver scorecard name the branch
commit that produced them, and a squash or rebase would leave them naming a commit that is not
on `master`.

## When the project changes phase

Agent guidance follows decisions; it does not anticipate them.

- **Planned work that will make some guidance wrong** (the MCP server, SPE5 and SPE3 going live,
  black-oil schedules, saved-state versioning, dropping the legacy build layout): the exact lines
  are listed on [#137](https://github.com/sergeyfarin/ressim/issues/137) and are changed in the
  pull request that delivers each one.

- **A new harness or model is adopted.** Check that it loads `AGENTS.md` and one of
  `.claude/skills/` or `.agents/skills/`, and add it to the table in
  `.claude/skills/README.md` with how to verify it. Do not add a harness-specific instruction
  file.
