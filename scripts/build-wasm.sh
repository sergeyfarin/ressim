#!/bin/sh
set -eu

WASM_PKG_DIR="src/lib/ressim/pkg"
WASM_STAMP="$WASM_PKG_DIR/simulator_bg.wasm"

# Inputs that invalidate the generated package. `scripts/build-wasm.sh` is in the
# list so that editing this file forces one rebuild.
WASM_INPUTS="src/lib/ressim/src src/lib/ressim/Cargo.toml Cargo.toml Cargo.lock scripts/build-wasm.sh"

# `pkg/` is a build product, not a source artifact (#30). wasm-bindgen does not emit
# a stable declaration order across toolchains and the release wasm embeds
# host-absolute rustc/registry paths, so a committed copy produces meaningless diffs
# and silently goes stale against `src/lib/ressim/src/`. Fail loudly if it comes back.
assert_pkg_untracked() {
  command -v git >/dev/null 2>&1 || return 0
  git rev-parse --git-dir >/dev/null 2>&1 || return 0
  if [ -n "$(git ls-files -- "$WASM_PKG_DIR")" ]; then
    echo "Error: $WASM_PKG_DIR is tracked by git, but it is generated output (#30)." >&2
    echo "       Run: git rm -r --cached $WASM_PKG_DIR" >&2
    exit 1
  fi
}

# wasm-pack re-runs wasm-bindgen and wasm-opt (~25 s) even when cargo has nothing to
# recompile. Every entry point that consumes the bindings now builds them first, so
# `pnpm run validate:product` would pay that three times without this check.
pkg_is_fresh() {
  if [ "${RESSIM_FORCE_WASM_BUILD:-0}" = "1" ]; then
    return 1
  fi
  if [ ! -f "$WASM_STAMP" ] || [ ! -f "$WASM_PKG_DIR/simulator.js" ] ||
    [ ! -f "$WASM_PKG_DIR/simulator.d.ts" ]; then
    return 1
  fi
  # shellcheck disable=SC2086 # WASM_INPUTS is a deliberate list of paths.
  if [ -n "$(find $WASM_INPUTS -newer "$WASM_STAMP" -print 2>/dev/null | head -n 1)" ]; then
    return 1
  fi
  return 0
}

build_wasm() {
  if command -v rustup >/dev/null 2>&1; then
    rustup target add wasm32-unknown-unknown
  fi

  cd src/lib/ressim
  wasm-pack build --target web --out-dir ./pkg
  rm -f pkg/.gitignore
}

has_prebuilt_pkg() {
  [ -f "$WASM_PKG_DIR/simulator.js" ] && [ -f "$WASM_STAMP" ]
}

assert_pkg_untracked

if pkg_is_fresh; then
  echo "[build:wasm] $WASM_PKG_DIR is newer than its inputs; skipping rebuild (RESSIM_FORCE_WASM_BUILD=1 to force)"
  exit 0
fi

if command -v wasm-pack >/dev/null 2>&1; then
  build_wasm
  exit 0
fi

if ! command -v cargo >/dev/null 2>&1; then
  if command -v curl >/dev/null 2>&1; then
    echo "[build:wasm] cargo not found, installing Rust toolchain via rustup..."
    curl https://sh.rustup.rs -sSf | sh -s -- -y --profile minimal --default-toolchain stable
    export PATH="$HOME/.cargo/bin:$PATH"
  fi
fi

if ! command -v wasm-pack >/dev/null 2>&1 && command -v cargo >/dev/null 2>&1; then
  echo "[build:wasm] installing wasm-pack via cargo..."
  cargo install wasm-pack --locked
  export PATH="$HOME/.cargo/bin:$PATH"
fi

if command -v wasm-pack >/dev/null 2>&1; then
  build_wasm
  exit 0
fi

# `pkg/` is no longer committed, so this can only be a package left over from an
# earlier local build. Using it is better than failing, but it may be stale.
if has_prebuilt_pkg; then
  echo "[build:wasm] warning: wasm-pack unavailable; reusing the stale local WASM package in $WASM_PKG_DIR" >&2
  exit 0
fi

echo "Error: wasm-pack/cargo unavailable and no WASM package found in $WASM_PKG_DIR." >&2
echo "       $WASM_PKG_DIR is generated (#30) — install wasm-pack and rerun." >&2
exit 1
