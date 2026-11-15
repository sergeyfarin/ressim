---
name: update-dependencies
description: Maintenance procedure for ResSim's dependencies - survey Dependabot pull requests and GitHub security alerts, update pnpm packages, Rust crates, the Python and Julia tool environments and GitHub Actions in one branch, adopt what the new versions make possible, validate, and open one pull request. Use when asked to update, upgrade or bump dependencies, check security or vulnerability warnings, handle Dependabot, or modernize after an upgrade.
---

# Update Dependencies

One branch, one pull request, everything current. This task runs as a `work-issue` task: branch
from the canonical `master`, finish with an open pull request, fix small things, file issues for
the rest.

A dependency update here is not automatically routine: several crates sit inside the numerical
path, and a changed last bit is a changed answer. Section 3 lists them. Read it before updating.

## 1. Survey first

```sh
gh pr list --author "app/dependabot" --state open
gh api "repos/{owner}/{repo}/dependabot/alerts?state=open" --jq '.[] | "\(.security_advisory.severity) \(.dependency.package.ecosystem) \(.dependency.package.name) \(.html_url)"'
gh api "repos/{owner}/{repo}/code-scanning/alerts?state=open&per_page=100" --jq '.[] | "\(.rule.severity) \(.rule.id) \(.most_recent_instance.location.path)"'
gh api "repos/{owner}/{repo}/secret-scanning/alerts?state=open" --jq 'length'
gh issue list --label blocked --state open
```

There is no `.github/dependabot.yml`: Dependabot opens security updates only, so version
updates happen only through this task. Note which `blocked` issues and which holds from the last
dependency pull request are upgrades waiting on a peer range; this run is when they get
rechecked.

## 2. Update — the routine ecosystems

- **pnpm** (root and the `src/lib/*` workspace packages): `pnpm -r update --latest`, holding a
  package back with `'!<name>'`. Recheck every entry under `overrides` in
  `pnpm-workspace.yaml`; each carries the condition for dropping it. After a Playwright bump run
  `pnpm exec playwright install chromium`.
- **Rust**: raise direct versions in `src/lib/ressim/Cargo.toml` and
  `crates/ressim-py/Cargo.toml`, then `cargo update` (one `Cargo.lock` at the root). CI uses the
  floating `stable` toolchain, so run `cargo clippy --workspace --all-targets` with the current
  stable: a new release can add a deny-by-default lint.
- **Python**: `uv lock --upgrade` in `tools/opm_flow` (always `uv`, never `pip`).
- **GitHub Actions**: bump action versions in both files under `.github/workflows/`. The Node
  version is stated in both workflows; change it everywhere or nowhere. Pushing a workflow file
  needs the `workflow` scope on the `gh` token.

## 3. Not routine — decide each one deliberately

| Dependency | Why | What to do |
|---|---|---|
| `three`, `@types/three` | Pinned to an exact version by `AGENTS.md`; the 3D view is version-sensitive | Hold, every run, unless the owner asks for the upgrade |
| `libm` | Every `pow`/`exp`/`ln` in the engine goes through it so native and wasm agree bit for bit (#62) | Treat as an engine change: all gates in section 6 including the cross-solver gate |
| `faer`, `nalgebra`, `sprs` | Linear algebra inside the solvers; a different pivot or rounding moves Newton paths | Same. If a banded number moves, back the crate out and file an issue with the diff |
| `rand`, `getrandom` | Seeded permeability fields; a changed generator stream changes those scenarios | Same |
| `wasm-bindgen`, `js-sys`, `serde-wasm-bindgen` | The CLI version must equal the crate version; `scripts/ci-install-wasm-tools.sh` reads it from `Cargo.lock` and pins binaryen | Move together, then `RESSIM_FORCE_WASM_BUILD=1 bash scripts/build-wasm.sh` |
| `pyo3`, `pythonize` | `pythonize` requires a matching `pyo3` minor | Move together or not at all |
| `JutulDarcy`, Julia (`tools/jutul/Project.toml`) | The second reference simulator, pinned exactly; its output is recorded in the benchmark records | Hold. Upgrading it is a re-baselining task with its own issue |
| OPM Flow | Installed on the machine, not in the repo; committed artifacts name the version that made them | Not part of this task. Never regenerate artifacts as a side effect |
| `typescript` | Bounded by the `typescript-eslint` peer range | Move only when the peer range allows; never force a peer range |
| `tailwindcss`, `vite`, `svelte` majors | A content-glob or build change produces an unstyled page with no error | Allowed, with the packaged-build browser smoke in section 6 |

## 4. Hold what cannot move

A version that a peer dependency rejects, that section 3 holds, or that fails validation for a
reason too large to fix here is held. Say which and why in the commit and in the pull request
under *Held back on purpose*, and record it on the issue that tracks it (create one when none
exists, labelled `blocked` with the prerequisite named).

## 5. Modernize, and close the alerts

- Read the release notes of every major and notable minor bump. Look for an API that replaces a
  local helper or workaround, a deprecation the code now triggers, a default that changed, and a
  new lint. Small and local: do it in this pull request as its own commit. Anything else: an
  issue naming the new option and the code it would simplify.
- Every open alert from step 1 ends in one of three states, listed in the pull request:
  **fixed** by an update in this branch; **not fixable yet** (no patched release, or the patch is
  held), with an issue carrying the advisory link, the affected path and whether the vulnerable
  code is reachable in this app; or **believed not applicable**, with the reason. Dismissing an
  alert on GitHub is the owner's call; never dismiss one yourself.
- A secret-scanning alert is reported to the owner at once and never quoted.

## 6. Validate

```sh
RESSIM_FORCE_WASM_BUILD=1 pnpm run validate:full
cargo clippy --workspace --all-targets
bash scripts/validate-compositional.sh thermo
bash scripts/validate-native-binding.sh
bash scripts/benchmarks.sh check --tier fast
(cd tools/opm_flow && uv run pytest -q)
pnpm run package:dist && bash scripts/smoke-preview.sh
```

When any crate the engine links moved by more than a lockfile patch bump, or any crate in
section 3 moved at all, also run `bash scripts/validate-cross-solver.sh` and
`bash scripts/benchmarks.sh check` (they need OPM Flow). When they could not run, say so in the
pull request and name the crates that moved. No tolerance, band or benchmark record changes in a
dependency pull request.

## 7. Ship

One pull request following `work-issue` step 6. Its body lists: what moved (majors called out),
what is held and why, each alert's state, what was modernized, the gates run and not run, the
follow-up issues, and the Dependabot pull requests it supersedes. Close those with a comment
linking this pull request.
