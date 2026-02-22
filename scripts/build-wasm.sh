#!/bin/sh
set -eu

WASM_PKG_DIR="src/lib/ressim/pkg"

build_wasm() {
  if command -v rustup >/dev/null 2>&1; then
    rustup target add wasm32-unknown-unknown
  fi

  cd src/lib/ressim
  wasm-pack build --target web --out-dir ./pkg
  rm -f pkg/.gitignore
}

has_prebuilt_pkg() {
  [ -f "$WASM_PKG_DIR/ressim.js" ] || [ -f "$WASM_PKG_DIR/package.json" ]
}

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

if has_prebuilt_pkg; then
  echo "[build:wasm] warning: wasm-pack unavailable; using prebuilt WASM package in $WASM_PKG_DIR"
  exit 0
fi

echo "Error: wasm-pack/cargo unavailable and no prebuilt WASM package found in $WASM_PKG_DIR"
exit 1
