# What CI runs

Reference for the `ressim-validation` skill. The list of gates CI does **not** run is in
`SKILL.md`, because every engine task needs it.

`.github/workflows/pr-tests.yml` is the source of truth; it runs on pull requests and on manual
dispatch only, so a commit pushed straight to `master` is covered by none of it. Its steps, in
order (read the file when the exact order matters):

1. `scripts/ci-install-wasm-tools.sh` (the WASM build tools, from a cache), `pnpm install`, and an
   explicit `scripts/build-wasm.sh`.
2. `pnpm run lint`, `pnpm run check:cycles`, `pnpm run typecheck`.
3. `validate-solver-coverage.sh all`, then the Buckley-Leverett benchmarks.
4. `validate-compositional.sh thermo`, `validate-native-binding.sh`,
   `benchmarks.sh check --tier fast` (benchmark records and the generated page).
5. The OPM artifact pipeline pytest (`tools/opm_flow`) and the SPE5 tool tests
   (`python3 -I -m unittest discover -s tools/jutul/tests`).
6. The full vitest suite via `pnpm run test:coverage`, then `pnpm run build`.
7. Two packaged-build browser smokes, each `pnpm run package:dist` then
   `bash scripts/smoke-preview.sh` (`test:deployed` against `vite preview`): the legacy layout
   (`RESIM_BASE_PATH=/ressim/`) and the root candidate (`RESIM_BASE_PATH=/`).

`publish-dist.yml` runs on a push to `master`: `pnpm run validate:product`, package, deploy to
GitHub Pages, then `test:deployed` against the deployed revision. That is narrower than PR CI: no
FIM bucket, no native-binding parity, no benchmark check.

The explicit WASM build matters: `src/lib/ressim/pkg/` is **generated, not committed** (#30), so
that step is what produces the bindings every frontend simulation test loads. It is belt and
braces — `pretypecheck` / `pretest*` / `prebuild` in `package.json` run `scripts/build-wasm.sh`
too — but keeping it explicit puts a Rust build failure in its own CI step.

Locally the same hooks mean you never have to remember the rebuild: `pnpm run typecheck`, any
`pnpm run test*`, `pnpm run dev` and `pnpm run build` all build the bindings first. The script
skips the work (~25 s of wasm-bindgen + wasm-opt that `wasm-pack` re-runs even when cargo has
nothing to recompile) when `pkg/simulator_bg.wasm` is already newer than `src/lib/ressim/src/`,
both `Cargo.toml`s, `Cargo.lock` and the script itself. Force a rebuild with
`RESSIM_FORCE_WASM_BUILD=1 bash scripts/build-wasm.sh`.

Before 2026-09-14 (#13) CI ran only the IMPES bucket, no lint, no cycle check, no build and no
Buckley-Leverett gate. If you are reading an older run, do not assume it covered FIM.
