#!/usr/bin/env bash
# Install the WASM build tools CI needs, so `scripts/build-wasm.sh` never downloads them itself.
#
# wasm-pack looks for `wasm-bindgen` (of the version in Cargo.lock) and `wasm-opt` on PATH and
# only downloads them when they are missing, anonymously, from github.com release pages. During a
# GitHub incident those downloads time out, and a `wasm-opt` download failure has no fallback, so
# the build fails before any of the change under test runs (2026-11-09). This script puts pinned
# versions of all three tools in one directory, which the workflows cache with actions/cache; a
# cache hit makes no network request at all. On a miss each tool is downloaded with retries, and
# if github.com still refuses, built from crates.io instead.
#
#   bash scripts/ci-install-wasm-tools.sh        # installs into $WASM_TOOLS_DIR (default below)
#
# On GitHub Actions the directory is appended to $GITHUB_PATH; elsewhere, add `$dir/bin` to PATH.
set -euo pipefail

WASM_PACK_VERSION=0.13.1
# One wasm-opt version on both paths: binaryen 116 is the newest release that the crates.io
# fallback (the `wasm-opt` crate, which bundles binaryen's source) also provides, so a fallback
# install optimizes exactly as a normal one does. (wasm-pack 0.13.1 would itself fetch 117; with
# wasm-opt on PATH it never does.)
BINARYEN_VERSION=116
WASM_OPT_CRATE_VERSION=0.116.1

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOCK="$ROOT/Cargo.lock"
WASM_BINDGEN_VERSION="$(awk '$0 == "name = \"wasm-bindgen\"" { getline; gsub(/version = |"/, ""); print; exit }' "$LOCK")"
if [ -z "$WASM_BINDGEN_VERSION" ]; then
  echo "[wasm-tools] could not read the wasm-bindgen version from $LOCK" >&2
  exit 1
fi

DIR="${WASM_TOOLS_DIR:-$HOME/.ressim-wasm-tools}"
BIN="$DIR/bin"
mkdir -p "$BIN"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

# Every attempt is bounded, not only its connection: `--max-time` caps one attempt, and a
# transfer slower than 10 kB/s for 30 s is abandoned, so a response that stalls after its headers
# cannot hold the job until the workflow times out. `--retry-max-time` stops curl from starting a
# new attempt after 900 s; an attempt already running keeps its own 300 s limit, so one tool's
# downloads end within about 900 + 300 s, after which the caller falls back to crates.io.
fetch() {
  curl -sSfL --connect-timeout 20 --max-time 300 --speed-limit 10000 --speed-time 30 \
    --retry 6 --retry-all-errors --retry-delay 15 --retry-max-time 900 \
    -A ressim-ci -o "$2" "$1"
}

# `cargo install` a crate into a temporary root and copy the named binaries into $BIN.
from_crates_io() {
  local crate="$1" version="$2"
  shift 2
  echo "[wasm-tools] github.com download failed; building $crate $version from crates.io" >&2
  cargo install "$crate" --version "$version" --locked --root "$WORK/cargo-$crate"
  for binary in "$@"; do
    install -m 755 "$WORK/cargo-$crate/bin/$binary" "$BIN/$binary"
  done
}

reports() {
  [ -x "$BIN/$1" ] && "$BIN/$1" --version 2>/dev/null | grep -qF "$2"
}

if reports wasm-pack "wasm-pack $WASM_PACK_VERSION"; then
  echo "[wasm-tools] wasm-pack $WASM_PACK_VERSION cached"
else
  name="wasm-pack-v$WASM_PACK_VERSION-x86_64-unknown-linux-musl"
  if fetch "https://github.com/rustwasm/wasm-pack/releases/download/v$WASM_PACK_VERSION/$name.tar.gz" "$WORK/wp.tgz"; then
    tar -xzf "$WORK/wp.tgz" -C "$WORK"
    install -m 755 "$WORK/$name/wasm-pack" "$BIN/wasm-pack"
  else
    from_crates_io wasm-pack "$WASM_PACK_VERSION" wasm-pack
  fi
fi

if reports wasm-bindgen "wasm-bindgen $WASM_BINDGEN_VERSION"; then
  echo "[wasm-tools] wasm-bindgen $WASM_BINDGEN_VERSION cached"
else
  name="wasm-bindgen-$WASM_BINDGEN_VERSION-x86_64-unknown-linux-musl"
  if fetch "https://github.com/rustwasm/wasm-bindgen/releases/download/$WASM_BINDGEN_VERSION/$name.tar.gz" "$WORK/wb.tgz"; then
    tar -xzf "$WORK/wb.tgz" -C "$WORK"
    for binary in wasm-bindgen wasm-bindgen-test-runner wasm2es6js; do
      install -m 755 "$WORK/$name/$binary" "$BIN/$binary"
    done
  else
    from_crates_io wasm-bindgen-cli "$WASM_BINDGEN_VERSION" wasm-bindgen wasm-bindgen-test-runner wasm2es6js
  fi
fi

if reports wasm-opt "version $BINARYEN_VERSION"; then
  echo "[wasm-tools] wasm-opt cached ($("$BIN/wasm-opt" --version))"
else
  name="binaryen-version_$BINARYEN_VERSION"
  if fetch "https://github.com/WebAssembly/binaryen/releases/download/version_$BINARYEN_VERSION/$name-x86_64-linux.tar.gz" "$WORK/by.tgz"; then
    tar -xzf "$WORK/by.tgz" -C "$WORK"
    install -m 755 "$WORK/$name/bin/wasm-opt" "$BIN/wasm-opt"
  else
    from_crates_io wasm-opt "$WASM_OPT_CRATE_VERSION" wasm-opt
  fi
fi

for tool in wasm-pack wasm-bindgen wasm-opt; do
  echo "[wasm-tools] $("$BIN/$tool" --version)"
done
if [ -n "${GITHUB_PATH:-}" ]; then
  echo "$BIN" >> "$GITHUB_PATH"
fi
