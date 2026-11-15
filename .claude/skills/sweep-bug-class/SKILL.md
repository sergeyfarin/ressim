---
name: sweep-bug-class
description: Hunt for every remaining instance of one known bug class across the whole ResSim tree - search for the pattern rather than the copies a ticket named, count and classify every hit, prove the real ones with a failing test, and add a guard so it cannot return. Use when asked to sweep, hunt, audit or grep for a bug class or pattern, or to check whether a bug that was just fixed exists elsewhere.
---

# Sweep One Bug Class

A fix is only as good as its sweep. Two FIM defects in a row here had the same signature (a
primary sitting exactly on a clamp), and Rust tests in modules that no gate selected hid
failures more than once (#13). This task takes one class and
finds all of it.

It runs as a `work-issue` task: branch from the canonical `master`, finish with an open pull
request, fix small instances, file issues for the rest.

## 1. Pick the class and learn its shape

The class comes from the prompt: a bug that was just fixed, a class from the list below, or a
pattern described in words. Read the fix that closed it. Write down, before searching:

- the code shape that causes it;
- the shape of a correct version;
- how it shows up in tests, fixtures, decks, scripts and docs, not only in source.

## 2. Search for the pattern, not the copies

- Search the whole tree: `src/lib/ressim/src/` (both solvers and the compositional engine),
  `crates/`, the TypeScript under `src/`, `scripts/`, `tools/`, decks under `opm/`. A class
  found in Rust may also live in the TypeScript mirror of the same formula.
- Use several spellings of the pattern. A helper that was copied was usually also renamed,
  reordered or inlined; a formula has a legacy and an AD twin.
- Search for the absence as well as the presence: a call site that should use the shared owner
  and does not.
- Keep the exact commands. They go in the report, so the sweep can be rerun.

## 3. Count and classify every hit

List every hit with `file:line` and give each one a verdict:

- **Instance** — the bug is there.
- **Safe** — with the reason: a guard, a different input domain, a test-only path.
- **Unsure** — say what you would need to know.

State the total. A sweep that reports "a few" has not counted.

## 4. Prove each instance

A named test that fails, or a reproduction with the input and the wrong result. An instance you
cannot make fail is "unsure", not "instance".

## 5. Fix or file

The `work-issue` rule applies. A small, local instance that cannot move a number is fixed in this
pull request with its named test. A larger one, and **any instance whose fix changes a simulated
or analytical result, convergence behaviour, a tolerance or a benchmark record**, becomes an
issue with the evidence from step 4. In the engine that is most instances: the usual output of an
engine sweep is a proved list and issues, not a large diff.

When several copies of a helper are the cause, move them onto one owner. Then ask what the old
copies rejected by accident that the shared one accepts, and make that rejection explicit and
tested.

## 6. Make it stay fixed

When the pattern can be detected mechanically, add a guard that fails on a new copy, with an
allow-list that gives a reason per entry rather than a loose pattern. When it cannot be detected
mechanically, say so. Add the class and its guard to the list below in the same pull request.

## Classes this repo has shipped, and their guards

| Class | Guard today |
|---|---|
| Platform math in the engine (`f64::powf`, `exp`, `ln`, trig) instead of `crate::math` | `src/lib/ressim/clippy.toml` disallowed methods, run by `scripts/validate-native-binding.sh` |
| `cfg(target_arch)` or a target type (`JsValue`, `PyObject`) inside the engine | `scripts/validate-native-binding.sh` (builds the engine without its wasm bindings and compares native with browser results) |
| A primary whose derivative depends on which side of a clamp its roundoff lands | `src/lib/ressim/src/fim/jacobian_audit.rs` (finite-difference audit, test-only); the `does_not_fragment` tests |
| A Rust test or module that no gate filter selects | `scripts/validate-solver-coverage.sh` fails on a filter that runs nothing; a new module still has to be added by hand |
| A formula changed in one twin only (legacy against AD assembly, IMPES against FIM) | The `assembly_ad` bit-parity tests and the `*_on_both_solvers` tests |
| Scenario-key or analytical-method branching outside the catalog and the registry | `src/lib/catalog/scenarioAgnosticArchitecture.test.ts`, `src/lib/charts/chartAgnosticArchitecture.test.ts`, `src/lib/charts/analyticalMethodRegistry.test.ts` |
| A relative import across a workspace package, or a runtime import cycle | `src/lib/packageBoundaries.test.ts`, `pnpm run check:cycles`, `scripts/import-gates.test.ts` |
| A summary vector drawn with the wrong unit, or surface and reservoir volumes on one axis | `tools/opm_flow/opm_flow_tool/vectors.py` and the `tools/opm_flow/tests` pytest; `src/lib/catalog/opmReferenceWiring.test.ts` |
| A benchmark number typed into a document by hand | `bash scripts/benchmarks.sh render --check` for the generated page; nothing for other documents |
| A TypeScript analytical formula that drifted from its Rust twin | None. Known gap (`engine-physics-change` skill) |

## 7. Report

- The pattern and the exact search commands.
- The count: hits, instances, safe, unsure.
- A table of every hit with its verdict and outcome (fixed, issue number, left with reason).
- The guard added, or why none is possible.
