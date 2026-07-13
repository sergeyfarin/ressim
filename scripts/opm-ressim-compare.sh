#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

CASE_KEY="gas-rate-10x10x3"
FLOW_BIN="${FLOW_BIN:-flow}"
OUT_DIR="${OUT_DIR:-/tmp/ressim-opm-compare/${CASE_KEY}}"
MODE="both"
BUILD_WASM=1
DRY_RUN=0

usage() {
  cat <<'USAGE'
Usage: scripts/opm-ressim-compare.sh [options]

Runs the tracked gas-rate FIM/OPM convergence baseline. Flow artifacts are
always written below --out-dir so the source tree remains clean.

Options:
  --case <key>       Only gas-rate-10x10x3 is currently tracked.
  --ressim-only      Run only the ResSim diagnostic.
  --opm-only         Run only OPM Flow and verify CASE.INFOSTEP.
  --out-dir <dir>    Output directory (default /tmp/ressim-opm-compare/gas-rate-10x10x3).
  --flow-bin <path>  Flow executable (default FLOW_BIN or flow).
  --no-build-wasm    Do not rebuild the committed WASM package.
  --dry-run          Print commands without running simulators.
  --help             Show this help.
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --case) CASE_KEY="${2:?missing case key}"; shift 2 ;;
    --ressim-only) MODE="ressim"; shift ;;
    --opm-only) MODE="opm"; shift ;;
    --out-dir) OUT_DIR="${2:?missing output directory}"; shift 2 ;;
    --flow-bin) FLOW_BIN="${2:?missing Flow binary}"; shift 2 ;;
    --no-build-wasm) BUILD_WASM=0; shift ;;
    --dry-run) DRY_RUN=1; shift ;;
    --help) usage; exit 0 ;;
    *) echo "Unknown option: $1" >&2; usage >&2; exit 2 ;;
  esac
done

if [[ "$CASE_KEY" != "gas-rate-10x10x3" ]]; then
  echo "Unsupported tracked case: $CASE_KEY" >&2
  exit 2
fi

DECK="opm/reference-decks/$CASE_KEY/CASE.DATA"
node scripts/opm-reference-fixture-check.mjs --case "$CASE_KEY"

if [[ "$DRY_RUN" -eq 1 ]]; then
  echo "+ mkdir -p $OUT_DIR"
  if [[ "$MODE" != "opm" && "$BUILD_WASM" -eq 1 ]]; then
    echo "+ pnpm run build:wasm"
  fi
  if [[ "$MODE" != "opm" ]]; then
    echo "+ node scripts/fim-wasm-diagnostic.mjs --preset gas-rate --grid 10x10x3 --dt 0.25 --steps 6 --opm-aligned --diagnostic summary > $OUT_DIR/ressim.json"
  fi
  if [[ "$MODE" != "ressim" ]]; then
    echo "+ cp $DECK $OUT_DIR/opm/CASE.DATA"
    echo "+ (cd $OUT_DIR/opm && $FLOW_BIN CASE.DATA --output-extra-convergence-info=steps,iterations --solver-verbosity=3 --time-step-verbosity=3)"
    echo "+ node scripts/opm-reference-fixture-check.mjs --case $CASE_KEY --infostep $OUT_DIR/opm/CASE.INFOSTEP"
  fi
  exit 0
fi

mkdir -p "$OUT_DIR"

if [[ "$MODE" != "opm" ]]; then
  if [[ "$BUILD_WASM" -eq 1 ]]; then
    pnpm run build:wasm
  fi
  node scripts/fim-wasm-diagnostic.mjs \
    --preset gas-rate --grid 10x10x3 --dt 0.25 --steps 6 \
    --opm-aligned --diagnostic summary > "$OUT_DIR/ressim.json" 2> "$OUT_DIR/ressim.log"
  if [[ ! -s "$OUT_DIR/ressim.json" ]]; then
    echo "ResSim diagnostic emitted no JSON; see $OUT_DIR/ressim.log" >&2
    exit 1
  fi
  node -e "JSON.parse(require('node:fs').readFileSync(process.argv[1], 'utf8'))" "$OUT_DIR/ressim.json"
fi

if [[ "$MODE" != "ressim" ]]; then
  OPM_DIR="$OUT_DIR/opm"
  mkdir -p "$OPM_DIR"
  cp "$DECK" "$OPM_DIR/CASE.DATA"
  (
    cd "$OPM_DIR"
    "$FLOW_BIN" CASE.DATA --output-extra-convergence-info=steps,iterations \
      --solver-verbosity=3 --time-step-verbosity=3
  ) > "$OUT_DIR/opm.log" 2>&1
  node scripts/opm-reference-fixture-check.mjs --case "$CASE_KEY" --infostep "$OPM_DIR/CASE.INFOSTEP"
fi

printf 'comparison output: %s\n' "$OUT_DIR"
