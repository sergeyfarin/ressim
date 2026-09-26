#!/usr/bin/env bash
set -euo pipefail

# Native (non-WASM) consumer gate — S5 of docs/ARCHITECTURE_SPLIT_PLAN_2026-09-19.md.
#
# Checks four things, in order of what they would cost to get wrong:
#
#   0. Engine library code calls no `f64` transcendental (`powf`, `exp`, `ln`, trig, `hypot`, ...):
#      those call the platform libm, which is not bit-identical across targets (#62). The engine
#      uses `crate::math` instead. The rule lives in src/lib/ressim/clippy.toml; this runs only
#      that lint, so unrelated clippy findings cannot fail the gate.
#   1. `ressim-py` links the engine with `default-features = false`, so no wasm-bindgen, js-sys or
#      serde-wasm-bindgen appears in its dependency graph. This is what makes S4's feature gate a
#      fact the compiler enforces rather than a convention.
#   2. The extension module builds and imports.
#   3. The native bindings and the *browser* bindings produce the same pressures and saturations
#      for the committed Buckley case A fixture. Both are the same Rust compiled for two targets,
#      so the bar is 1e-9 — a floating-point courtesy, not a tolerance budget. This checks that a
#      binding forwards faithfully; `benchmark_buckley` in the Rust suite owns the physics.
#
# Deliberately NOT a physics gate, and deliberately not in `validate:product`: it needs a Python
# interpreter and the generated wasm bundle. Run it when touching crates/ressim-py, the engine's
# `wasm` feature, anything in frontend.rs, or engine math (a new transcendental call site).

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
parity="$repo_root/crates/ressim-py/parity"

echo "== 0. target-independent math"
# Both feature sets: the browser build (default features) and the native one ressim-py links.
for features in "" "--no-default-features"; do
    ( cd "$repo_root/src/lib/ressim" && cargo clippy --quiet --lib $features -- \
        -A clippy::all -A warnings -D clippy::disallowed_methods )
done
echo "gate ok: engine library code calls no platform transcendental (crate::math only)"

echo "== 1. dependency isolation"
graph="$(cargo tree --manifest-path "$repo_root/Cargo.toml" -p ressim-py -e normal 2>/dev/null || true)"
leaked="$(printf '%s\n' "$graph" | grep -cE 'wasm-bindgen|js-sys|serde-wasm-bindgen' || true)"
if [ "$leaked" -ne 0 ]; then
    echo "FAIL: ressim-py links $leaked wasm crate(s); the engine's default features leaked in." >&2
    printf '%s\n' "$graph" | grep -E 'wasm-bindgen|js-sys|serde-wasm-bindgen' >&2
    exit 1
fi
echo "gate ok: ressim-py's dependency graph contains no wasm binding crates"

echo "== 2. build and install the extension module"
cargo build --manifest-path "$repo_root/Cargo.toml" -p ressim-py --quiet
# The cdylib is libressim.so; Python imports a module named ressim.
# Honour CARGO_TARGET_DIR: copying from a hard-coded target/ silently tests a stale build.
cp "${CARGO_TARGET_DIR:-$repo_root/target}/debug/libressim.so" "$parity/ressim.so"
python3 -c "import sys; sys.path.insert(0, '$parity'); import ressim; ressim.Simulator(1,1,1,0.2)"
echo "gate ok: the extension module builds and imports"

echo "== 3. native/wasm parity on the committed fixture"
# Rebuild rather than trust an existing bundle: a stale one compares the native engine against
# an older wasm engine. build-wasm.sh no-ops when the bundle is newer than its sources.
# build-wasm.sh works on the current directory's tree; build this one's.
( cd "$repo_root" && bash scripts/build-wasm.sh )
node "$parity/run_wasm.mjs"
( cd "$parity" && python3 compare_native.py )

echo
echo "native binding gate: OK"
