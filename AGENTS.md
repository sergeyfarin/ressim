# Agent Instructions — ResSim

Browser reservoir simulator: a Rust/WASM engine (IMPES and fully implicit FIM, three-phase
black-oil, plus a compositional engine) under a Svelte 5 UI, with every result drawn against an
analytical or OPM Flow reference. `README.md` has the feature list, scenario inventory, project
layout and validate commands; `docs/DOCUMENTATION_INDEX.md` says which docs are authoritative.

This file is the only repo-wide instruction file, loaded by every harness used here (Claude
Code, Codex, OpenCode). `CLAUDE.md` is the single line `@AGENTS.md`, so Claude Code loads this
file whatever its version (`.claude/skills/README.md` names the one setting that skips it). Do
not add harness-specific copies. Keep it to rules that apply to most tasks and are costly to
miss. Task detail belongs in a skill; product facts belong in `README.md` or `docs/`.

## Task modes

Most requests are one of two modes. Decide which before you start, and do not mix them.

**Work an issue** (implement an issue, bug fix, feature or experiment) — `work-issue` skill:

- Branch from the latest canonical `master` (`upstream/master` when an `upstream` remote exists,
  otherwise `origin/master`), build the change, validate it.
- Finish with a pushed branch and an open, ready-for-review pull request. Do this without asking;
  it overrides any machine-wide default to wait for a request. Never merge.
- While in the code, look for bugs, weak tests, code smells and improvement opportunities beyond
  the task. A finding that is small, local and cannot move a number is fixed in the same pull
  request as its own commit. Anything else becomes a GitHub issue, and so does anything that
  changes a simulated or analytical result, solver convergence, a tolerance or a benchmark
  record, however short the fix. Nothing is left only in the conversation.

**Review a pull request** (review, re-review, assess) — `review-pr` skill:

- Review everything: direction, owner decisions, approach, implementation, bugs, tests and
  evidence, docs. An owner decision that looks wrong is a finding like any other.
- Review only. No edits, commits, pushes, pull request comments or new issues unless the owner
  asks for that specific thing. Describe a fix; do not make it.
- Report a verdict, then verified findings ranked `P0`–`P3` with `file:line`.

Four recurring tasks have their own skill: `shape-issue` (no code, no branch), and
`update-dependencies`, `drift-sweep` and `sweep-bug-class`, which run as *work an issue*. A
request that fits none of these follows the owner's wording; it still ends in a pull request
when it changes the repository. Rules: `docs/DEVELOPER_WORKFLOW.md`. Owner's guide:
`docs/AGENT_TASK_PLAYBOOK.md`.

Two rules bound every task:

- **The requested outcome sets the side effects.** A request to check, find, list or assess
  returns findings in the session and changes nothing: no branch, no edit to the checkout,
  nothing written to GitHub. A request to draft or propose returns text. Only a request to fix,
  build, update, sweep or file — or a task skill invoked by name — runs that skill's write
  workflow (branch, commits, pull request, issues, labels, comments). A probe that needs code
  changed runs in a scratch worktree under `.claude/worktrees/` and is never committed. When
  the wording is unclear, report first and ask.
- **A suspected security vulnerability is never made public before it is fixed.** Do not
  describe it in an issue, a pull request, a commit message or a comment. Report it to the owner
  in the session and follow `SECURITY.md` (GitHub private vulnerability reporting). An advisory
  GitHub has already published may be tracked publicly by its ID.

## Skills — load the ones matching your task before working

Twelve skills live in `.claude/skills/<name>/SKILL.md`. Claude Code and OpenCode find them there;
Codex finds the same files through `.agents/skills`, a symlink. If your harness did not list
them, open the file. A `SKILL.md` names the reference files beside it; read the ones for the area
you are changing, because they are not loaded automatically.

| Work | Skill |
|---|---|
| Implementing an issue, bug fix, feature or experiment | `work-issue` |
| Reviewing a pull request, branch or diff | `review-pr` |
| Turning an idea or finding into a ready issue | `shape-issue` |
| Dependencies, Dependabot, security alerts | `update-dependencies` |
| Stale docs, roadmap, tests, blocked issues | `drift-sweep` |
| Finding every instance of one bug pattern | `sweep-bug-class` |
| Which gate to run for which change. **Every task.** | `ressim-validation` |
| Any change under `src/lib/ressim/src/`, including the compositional engine | `engine-physics-change` |
| FIM convergence, Newton, timestep or linear-solver work | `fim-solver-debug` |
| Any Svelte/TS change: stores, workers, charts, 3D view | `frontend-architecture` |
| Catalog cases, sensitivities, analytical overlays | `add-scenario` |
| OPM Flow decks, artifacts and comparisons | `opm-reference-pipeline` |

A typical task uses one task skill, `ressim-validation` and one area skill.

## Tooling

- **pnpm** only (never npm/yarn). **uv** for all Python.
- **Svelte 5 runes** only (`$state`, `$derived`, `$derived.by`, `$effect`, `$props`, `$bindable`);
  no `export let`, no legacy stores. TypeScript strict. Tailwind utilities, no custom CSS unless
  unavoidable.
- **Three.js is pinned** to the exact version in `package.json`; do not upgrade it.
- The five `@ressim/*` workspace packages under `src/lib/` (`analytical`, `charts`, `primitives`,
  `quantities`, `presets`) are imported **by name** from outside; relative imports only inside a
  package. `src/lib/packageBoundaries.test.ts` enforces it.
- `src/lib/ressim/pkg/` is generated by `scripts/build-wasm.sh` and never committed (#30).
- Rust: no `unwrap()` in library code, `///` on public API, `cargo fmt`.

## Rules that are easy to break

- **Full `cargo test` is not a gate** — FIM/SPE1 tests can hang or dominate runtime. Use
  `bash scripts/validate-solver-coverage.sh {shared|fim|impes|all}` and the `ressim-validation`
  decision table. Any change that can move an answer also runs
  `bash scripts/validate-cross-solver.sh` (local only; needs OPM Flow) and
  `bash scripts/benchmarks.sh check`.
- **Benchmark tolerances** do not change without written justification in the same commit.
- **The engine never names a target type.** `src/lib/ressim/src/api.rs` is the payload boundary in
  plain Rust; `frontend.rs` (wasm) and `crates/ressim-py` (Python) are conversion shims holding no
  decisions. No `JsValue` or `PyObject` in the engine
  (`docs/ENGINE_PAYLOAD_BOUNDARY_DESIGN_2026-09-21.md`).
- **No `cfg(target_arch)` in solver routing, and no platform math in the engine.** Native and
  wasm agree bit for bit on the parity matrix. Engine code calls `crate::math`, never
  `f64::powf`/`exp`/`ln`/trig/`hypot` (glibc and wasm's libm differ in the last bit; #62), and
  `src/lib/ressim/clippy.toml` plus `validate-native-binding.sh` enforce it. See the `fim-solver-debug` skill
  before touching the linear route.
- **Worker messages are structured-cloneable.** Svelte `$state` values are Proxies and fail to
  clone; all sends go through `RuntimeStoreImpl.#post()` (applies `$state.snapshot()`), never
  `simWorker.postMessage` directly.
- **No runtime import cycles under `src/`** — they blank the app with a TDZ error at load.
  Break one with `import type` or a leaf module, not by reordering. Gated by
  `pnpm run check:cycles`.
- **No scenario-key branching** (`if (scenario.key === …)`) outside `src/lib/catalog/scenarios/`.
- Minimal, root-cause changes. No new dependencies unless essential. No speculative abstraction.

## Validation and commits

- Frontend: `pnpm run validate` while iterating, `pnpm run validate:product` before committing.
- Rust engine: `bash scripts/validate-solver-coverage.sh {shared|fim|impes|all}`, plus
  `cargo test --manifest-path src/lib/ressim/Cargo.toml benchmark_buckley` for shared physics.
  Compositional: `bash scripts/validate-compositional.sh thermo`.
- Payload boundary or bindings: `bash scripts/validate-native-binding.sh`.
- Anything that can move an answer: `bash scripts/validate-cross-solver.sh` and
  `bash scripts/benchmarks.sh check`.
- The full decision table is the `ressim-validation` skill. Report the command output, not
  "tests pass", and say which gates you did not run.
- Conventional Commits with the smallest honest scope and the issue number, for example
  `fix(compositional): reject partial checkpoint surface reporting (#88)`. Unrelated changes go
  in separate commits. Implementation work lands through a branch and a pull request; a direct
  commit to `master` needs the owner's say-so.

## Evidence discipline

- **Baselines.** Never call a solver, runtime or convergence number "the current baseline" unless
  it was reproduced on a committed revision, with the commit hash, the exact command and the key
  output recorded beside it. Dirty-tree or intermediate-commit results are provisional; promote
  only after rerunning the gates on the final cleaned-up tree, or mark the number provisional. When
  a baseline replaces an older one, say which and why. Never hand-copy benchmark numbers into
  docs: `docs/BENCHMARKS.md` is generated by `bash scripts/benchmarks.sh update` (committed tree
  only), and other docs link to it or use `validate-cross-solver.sh --markdown`.
- **Verdicts.** A green suite proves its own contracts, not a physics or convergence hypothesis.
  FIM experiments follow the oracle and verdict rules (`INCONCLUSIVE` ≠ `REFUTED`) in the
  `fim-solver-debug` skill.

## Tracking

- GitHub Issues is the source of truth for actionable work; the rules are in
  `docs/DEVELOPER_WORKFLOW.md`. Update the issue with in-scope discoveries, open a new one for
  anything separate, and close only with the gate green and the commit or PR linked.
- A deliberate "not doing this" goes in `docs/OPEN_ITEMS_2026-09-21.md` with the reason.
- Stable scientific evidence goes in its owning validation doc; FIM experiments go in
  `docs/FIM_EXPERIMENT_REGISTRY.md` and `docs/FIM_CONVERGENCE_WORKLOG.md`.
