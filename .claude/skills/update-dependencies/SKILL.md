---
name: update-dependencies
description: Maintenance procedure for ResSim's dependencies - survey Dependabot pull requests and GitHub security alerts, update pnpm packages, Rust crates, the Python and Julia tool environments and GitHub Actions in one branch, adopt what the new versions make possible, validate, and open one pull request. A check request returns the survey only and updates nothing. Use when asked to update, upgrade or bump dependencies, check security or vulnerability warnings, handle Dependabot, or modernize after an upgrade.
---

# Update Dependencies

One branch, one pull request, everything current. This task runs as a `work-issue` task: branch
from the canonical `master`, finish with an open pull request, fix small things, file issues for
the rest.

When the request is only to check (security warnings, what is outdated, what Dependabot has
open), run section 1 and report in the session; update nothing, create no branch and write
nothing to GitHub (`AGENTS.md` → Task modes). Section 1 only reads.

A dependency update here is not automatically routine: several crates sit inside the numerical
path, and a changed last bit is a changed answer. Section 3 lists them. Read it before updating.

## 1. Survey first

```sh
(
set -euo pipefail
repo=$(bash scripts/canonical-repo.sh)
survey() {  # survey <label> <command...>: print the rows, then "<label>: <count>" or UNCHECKED
  local label=$1 out; shift
  if out=$("$@"); then
    printf '%s\n' "$out" | awk -v l="$label" 'NF { print; n++ } END { print l ": " n + 0 }'
  else
    echo "$label: UNCHECKED"
  fi
}
survey "dependabot pull requests" gh pr list -R "$repo" --author "app/dependabot" --state open --limit 200 --json number,title --jq '.[] | "#\(.number) \(.title)"'
survey "dependabot alerts" gh api --paginate "repos/$repo/dependabot/alerts?state=open&per_page=100" --jq '.[] | "\(.security_advisory.severity) \(.dependency.package.ecosystem) \(.dependency.package.name) \(.security_advisory.ghsa_id)"'
survey "code-scanning alerts" gh api --paginate "repos/$repo/code-scanning/alerts?state=open&per_page=100" --jq '.[] | "\(.rule.security_severity_level // "-") \(.rule.id) \(.most_recent_instance.location.path)"'
survey "secret-scanning alerts" gh api --paginate "repos/$repo/secret-scanning/alerts?state=open&per_page=100" --jq '.[].number'
survey "blocked issues" gh issue list -R "$repo" --label blocked --state open --limit 500 --json number,title --jq '.[] | "#\(.number) \(.title)"'
)
```

- `repo` is the canonical repository from `scripts/canonical-repo.sh` (`upstream` when that
  remote exists, otherwise `origin`). In a fork clone `gh`'s default repository is the fork,
  whose alert feeds are disabled or empty; a survey of it says nothing about this project. The
  block runs in a strict subshell, so a missing remote stops it before any survey.
- Every `gh ... list` passes `--limit` and every `gh api` survey passes `--paginate`: a `gh` list
  defaults to 30 results and an unpaginated call returns the first page only, and either way the
  list silently looks complete. Compare each count with its limit.
- A feed that answers with an error (403 or 404: not enabled, or the token lacks access), or
  fails part-way through the pages, is **unchecked**, not empty. Say so in the report.
- Judge a feed by the exit status of `gh` itself, as `survey` does. Never pipe a survey into
  `wc` or `grep` and read the count: the pipeline reports the last command's status, and an
  error body counts as lines. Count with `awk`, which exits 0 on an empty result; `grep -c`
  exits 1 on zero matches and stops a `set -e` shell before the next survey.
- The code-scanning and secret-scanning rows are for the session only (section 5); they are
  never copied into a pull request, an issue or a commit message.

There is no `.github/dependabot.yml`: Dependabot opens security updates only, so version
updates happen only through this task. `pnpm audit` reads the same public advisories for the
lockfile and is a useful cross-check; `cargo audit` does the same for `Cargo.lock` where it is
installed. Note which `blocked` issues and which holds from the last dependency pull request are
upgrades waiting on a peer range; this run is when they get rechecked.

## 2. Update — the routine ecosystems

- **pnpm** (root and the `src/lib/*` workspace packages): `pnpm -r update --latest`, holding a
  package back with `'!<name>'`. Recheck every entry under `overrides` in
  `pnpm-workspace.yaml`; each carries the condition for dropping it. After a Playwright bump run
  `pnpm exec playwright install chromium`. A pnpm bump through either `update --latest` or
  `self-update` writes `packageManager` without its `+sha512.` hash. Put it back from what this
  prints, in hex. It prints the hash only when the lookup returns `sha512-` and the rest decodes
  completely to exactly 64 bytes; anything else prints nothing to stdout and exits non-zero:

  ```sh
  (
    set -euo pipefail
    i=$(pnpm view pnpm@<v> dist.integrity)
    case $i in sha512-?*) ;; *) echo "not a sha512 integrity" >&2; exit 1 ;; esac
    h=$(printf '%s' "${i#sha512-}" | base64 -d | od -An -v -tx1 | tr -d ' \n')
    [ "${#h}" -eq 128 ] || { echo "digest is not 64 bytes" >&2; exit 1; }
    printf '%s\n' "$h"
  )
  ```
- **Rust**: raise direct versions in `src/lib/ressim/Cargo.toml` and
  `crates/ressim-py/Cargo.toml`, then `cargo update` (one `Cargo.lock` at the root). CI uses the
  floating `stable` toolchain, so run `cargo clippy --workspace --all-targets` with the current
  stable: a new release can add a deny-by-default lint.
- **Python**: `uv lock --upgrade` in `tools/opm_flow` (always `uv`, never `pip`).
- **GitHub Actions**: review action versions in both files under `.github/workflows/`. Use floating
  major tags so patch and minor fixes arrive between manual dependency reviews. Update the tag
  only when moving to a new major, and keep the Rust toolchain action on `@stable`. The Node
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

## 5. Modernize, and settle the alerts

- Read the release notes of every major and notable minor bump. Look for an API that replaces a
  local helper or workaround, a deprecation the code now triggers, a default that changed, and a
  new lint. Small and local: do it in this pull request as its own commit. Anything else: an
  issue naming the new option and the code it would simplify.

What may be said in public about an alert depends on whether the vulnerability is already
public. `SECURITY.md` governs: nothing unpublished is disclosed before a fix.

- **Published advisories** (Dependabot alerts, `pnpm audit` and `cargo audit` findings, which
  come from public GHSA/CVE records) are tracked in the open **by advisory ID and dependency
  only**. Each ends in one of three states, listed in the pull request: **fixed** by an update in
  this branch; **not fixable yet** (no patched release, or the patch is held), with an issue
  naming the advisory ID and the dependency; or **believed not applicable**, stated without the
  reasoning.
- Whether the vulnerable code is **reachable in this app**, and through which path, is not
  published while the alert is open: it turns a public advisory into a map of how to attack
  this deployment. Give that analysis to the owner in the session.
- **First-party findings** (code-scanning alerts on this repository's own code, and anything you
  notice yourself) are unpublished. Do not name, describe or link them in the pull request, a
  commit message, an issue or a comment. Report them to the owner in the session, ranked, with
  your recommendation; the owner chooses the private route in `SECURITY.md`. A code-scanning
  rule that is about code quality and not security is ordinary work.
- **Secret-scanning alerts** are reported to the owner at once and never quoted.
- Dismissing any alert on GitHub is the owner's call; never dismiss one yourself.

## 6. Validate

Commit the updates first: the packaged-build smoke checks that the bundle was built from the
commit it is packaged as. Then run, as one command, the local sequence from `docs/DEPLOYMENT.md`
with the dependency gates added. The strict subshell stops at the first failure in any shell:

```sh
(
  set -euo pipefail
  RESIM_BUILD_SHA=$(git rev-parse HEAD); export RESIM_BUILD_SHA
  RESIM_BUILD_REF=$(git branch --show-current); export RESIM_BUILD_REF
  export RESIM_EXPECTED_SHA=$RESIM_BUILD_SHA RESIM_BASE_PATH=/
  RESSIM_FORCE_WASM_BUILD=1 pnpm run validate:full
  cargo clippy --workspace --all-targets
  bash scripts/validate-compositional.sh thermo
  bash scripts/validate-native-binding.sh
  bash scripts/benchmarks.sh check --tier fast
  (cd tools/opm_flow && uv run pytest -q)
  pnpm run package:dist
  bash scripts/smoke-preview.sh
)
```

`validate:full` ends with the production build, which records `RESIM_BUILD_SHA` and the base
path; `package:dist` refuses a bundle whose commit or base path differs, and fails outright when
no SHA is set.

**Can it move an answer?** Treat any change to the Rust dependency configuration as able to,
whatever the version increment and whether the crate is direct, transitive or a build
dependency: `faer` reaches the engine through transitive crates (`faer-traits`, `gemm-f64`) and
through build dependencies that generate its floating-point kernels (`nano-gemm-codegen`), so
no dependency-tree comparison is a safe filter. Check against the commit this branch was cut
from:

```sh
(
  set -euo pipefail
  base=$(bash scripts/canonical-repo.sh --remote)
  mb=$(git merge-base "$base/master" HEAD)
  if git diff --quiet "$mb" -- Cargo.lock '*Cargo.toml' '.cargo/*' 'rust-toolchain*'; then
    echo "Rust dependency configuration unchanged"
  else
    echo "Rust dependency configuration changed: run the cross-solver gate and benchmarks.sh check"
  fi
)
```

A `git diff` that fails is reported as "changed", so an error runs the gates rather than
skipping them. When the configuration changed, also run `bash scripts/validate-cross-solver.sh`
and `bash scripts/benchmarks.sh check` (they need OPM Flow). When they could not run, say so in
the pull request and name the crates that moved. No tolerance, band or benchmark record changes
in a dependency pull request.

## 7. Ship

One pull request following `work-issue` step 6. Its body lists: what moved (majors called out),
what is held and why, the state of each published advisory by ID, what was modernized, the gates
run and not run, the follow-up issues, and the Dependabot pull requests it covers
(`Covers #73, #74`).

Do not close those Dependabot pull requests. The owner may reject this pull request or drop an
upgrade in review, and a closed Dependabot pull request would then claim an update that never
landed. Dependabot closes its own pull request once `master` has the same or a newer version;
after this one merges, list any that are still open for the owner.
