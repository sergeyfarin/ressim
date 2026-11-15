# What CI runs

Reference for the `ressim-validation` skill. The list of gates CI does **not** run is in
`SKILL.md`, because every engine task needs it.

`.github/workflows/pr-tests.yml` is the source of truth. As of 2026-09-25 it runs, in order:
`pnpm install`, an explicit `scripts/build-wasm.sh`, lint, `check:cycles`, typecheck,
`validate-solver-coverage.sh all`, the Buckley-Leverett benchmarks,
`validate-compositional.sh thermo`, `validate-native-binding.sh`,
`benchmarks.sh check --tier fast` (benchmark records and the generated page), the OPM artifact pipeline
pytest (`tools/opm_flow`), the full vitest suite via `pnpm run test:coverage`, `pnpm run build`,
and a Playwright smoke test of the built bundle (`test:deployed` against `vite preview`).

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
