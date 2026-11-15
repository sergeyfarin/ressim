# ResSim Skill Library

Twelve skills that let any agent work on this project at the standard it was built to:

- Six **task skills** say how a session runs from start to finish. The two everyday modes are
  `work-issue` (implement, then open a pull request) and `review-pr` (review only, change
  nothing). Four cover recurring work: `shape-issue`, `update-dependencies`, `drift-sweep`,
  `sweep-bug-class`.
- Six **area skills** hold the rules for a part of the codebase: `ressim-validation`,
  `engine-physics-change`, `fim-solver-debug`, `frontend-architecture`, `add-scenario`,
  `opm-reference-pipeline`.

Which skill covers which work is in `AGENTS.md` → Task modes and Skills, because every harness
loads that file. `docs/AGENT_TASK_PLAYBOOK.md` is the owner's guide to starting each one. This
file is for maintaining the library.

## Layout

```
.claude/skills/<name>/SKILL.md      rules for every use of the skill
.claude/skills/<name>/<topic>.md    reference file, read when SKILL.md says so
.agents/skills -> ../.claude/skills symlink, so Codex finds the same files
```

Reference files today:

| Skill | Reference file | Read it for |
|---|---|---|
| `ressim-validation` | `rust-gates.md` | Changing a bucket filter or a test one selects |
| `ressim-validation` | `benchmarks.md` | Re-baselining, adding a benchmark, a failing cross-solver or benchmark check |
| `ressim-validation` | `ci.md` | Editing a workflow or `scripts/build-wasm.sh` |
| `engine-physics-change` | `compositional.md` | Anything under `compositional/`, `fluid/` or `tools/jutul/` |
| `fim-solver-debug` | `lever-history.md` | The full entry behind each lever in the checklist |
| `opm-reference-pipeline` | `internals.md` | Editing the pipeline's Python, a curve or an x-axis mapping |
| `opm-reference-pipeline` | `deck-pitfalls.md` | The full entry behind each deck pitfall |

## How each harness finds them

| Harness | Instructions | Skills | Force one | Verify |
|---|---|---|---|---|
| Claude Code | `CLAUDE.md`, which imports `AGENTS.md` | `.claude/skills/` | `/skill-name` | `/memory` lists `AGENTS.md`; `/skills` lists the skills |
| Codex | `AGENTS.md` | `.agents/skills/` (the symlink) | `$skill-name` | `codex debug prompt-input "hello"` shows both |
| OpenCode | `AGENTS.md` (it prefers it over `CLAUDE.md`) | `.claude/skills/` | `@skill-name` | not yet verified here |

Claude Code reads `AGENTS.md` by itself only from v2.1.277, and only while no `CLAUDE.md` or
`CLAUDE.local.md` exists in the directory or above it. The one-line `CLAUDE.md` (`@AGENTS.md`)
makes it load in every version, under the default *Project instructions* setting, under
`claude-md` and `claude-md-and-agents-md`, and when someone adds a personal `CLAUDE.local.md`.
The `managed-only` setting skips every project instruction file, this import included; a
session under it has no repository guidance.

All three load only each skill's `name` and `description` up front, the `SKILL.md` body when the
skill is used, and a reference file only when the body tells the agent to read it. Three
consequences:

- The description decides whether a skill is used at all. It says what the skill covers and when
  to use it, in words a task prompt would contain. Codex gives the whole list of descriptions a
  small budget, so keep each one to two or three sentences.
- A reference file nobody is told to read is never read. `SKILL.md` names each one and the work
  that needs it.
- When a session should not depend on automatic selection, start the prompt with the task skill:
  `/work-issue 104`, `$work-issue 104` or `@work-issue 104`. Invoking a task skill by name runs
  its write workflow unless the request carries an explicit limit ("check", "list", "draft",
  "read-only"); the limit always wins (`AGENTS.md` → Task modes).

Check discovery after adding or renaming a skill: `codex debug prompt-input "hello"` prints the
prompt Codex would send, including the skills it lists.

With smaller models, or for long tasks:

1. One skill-sized task per session. Do not mix an engine change and a frontend change.
2. Ask for the validation command output verbatim at the end, not "tests pass".
3. For convergence experiments, require the agent to name its oracle and to classify missing or
   incomparable diagnostics as `INCONCLUSIVE`, never `REFUTED`.

## Where things go

- `AGENTS.md` — rules that apply to most tasks and are costly to miss. Always loaded, so keep it
  short.
- A skill — the procedure and the traps for one kind of task.
- `README.md` / `docs/` — product facts, measurements and history. A skill links to them rather
  than restating them.

## Rules for maintaining this library

- Keep it small. A new skill must earn its place: a recurring task shape, real failure modes,
  not derivable from a quick read of the code. Do not add one for a one-off task.
- A task skill is a procedure and an area skill is a rule set. Keep them apart: a task skill
  points at area skills and restates no area rule. The three maintenance skills reuse
  `work-issue` for branching, triage and the pull request instead of repeating it.
- Adding, renaming or removing a skill updates the routing table in `AGENTS.md`, the list in
  `docs/AGENT_TASK_PLAYBOOK.md` and the tables above in the same change.
- Keep `SKILL.md` lean, about 250 lines at most, and watch its size in bytes as well: long lines
  cost a small-context model as much as many lines. It holds what applies to every use of the
  skill. Detail for one area or one family goes in a reference file.
- A skill that grows by one appended paragraph per issue is turning into a changelog. Write the
  rule, then the named test or gate that guards it. The issue number points at the story; the
  story itself belongs in the issue, the registry or the owning validation document.
- When a skill holds a list of things this repo got wrong (the FIM lever checklist, the deck
  pitfalls, the bug classes in `sweep-bug-class`), `SKILL.md` keeps one line per entry and the
  full entry goes in a reference file. FIM lever IDs are the registry's and are never
  renumbered.
- **Adding a bug class:** in the same pull request as the fix, add a row to the table in
  `sweep-bug-class/SKILL.md` naming the pattern and the test or gate that now guards it. If no
  guard is possible, say "None" and why.
- Move text verbatim when splitting a file, and check that nothing was lost by comparing the
  tokens of the old file with the new files.
- Frontmatter is `name` and `description` only, and `name` equals the directory name. That
  subset is what all three harnesses read.
- Refer to files in another skill by their path from the repo root.
- Guidance follows decisions and does not anticipate them. Lines that planned work will make
  wrong are listed on issue #137 and are changed in the pull request that delivers that work.
- Skills state current facts (counts, live or legacy status, known gaps). When one drifts from
  reality, fix it in the same pull request that moved reality. The `docs` scope of `drift-sweep`
  checks every cited path and name mechanically.
- `AGENTS.md` is the only repo-wide instruction file. `CLAUDE.md` stays the single line
  `@AGENTS.md` (see the note under the harness table for exactly when Claude Code loads it). Do
  not add harness-specific copies or content, in `CLAUDE.md` or in Copilot, Cursor or OpenCode
  instruction files; they drift from it.
- **Every command is run before it goes into a skill**, extracted from the file exactly as
  written, under `bash -euo pipefail`, in three cases: success, an empty result, and failure (a
  nonexistent endpoint, a wrong SHA). A command that only ran on the happy path is not verified.
- Every `gh ... list` in a skill passes `--limit` and every `gh api` survey passes `--paginate`.
  Every `--json` field must exist in the oldest `gh` in use (2.46 here; it has no
  `closingIssuesReferences` on `gh pr view`). In that version a plain `gh issue view <n>` or
  `gh pr view <n>` fails outright (a retired Projects field); always pass `--json` with the
  fields you need.
- Every `gh` call names its repository (`-R "$repo"`, `repos/$repo/…`, or a pull request URL),
  with `repo` resolved from `upstream` when it exists, otherwise `origin`. Test a new command in
  a fork clone whose `gh` default is the fork as well as in a direct clone.
- A command's result is judged by that command's own exit status. Never pipe a survey into `wc`
  or `grep` and read the count; count with `awk`. A guard written as `test … || echo "stop"`
  does not stop the next line: put what must not run inside the `if`.
- Each task skill that can write states its check-only or draft form, and that form is
  write-free in every step it runs, not only in its summary. A probe that needs code changed
  goes in a scratch worktree under `.claude/worktrees/` (git-ignored; never `/tmp`, which a
  reboot clears), is never committed, and is removed afterwards.
- No skill tells an agent to describe a suspected security vulnerability in an issue, a pull
  request, a commit message or a comment. `SECURITY.md` governs.
- After fixing a defect in one skill, search every skill, `AGENTS.md` and the docs for the same
  pattern before calling it done.
