#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

FLOW_BIN="${FLOW_BIN:-flow}"
OUT_DIR="${OUT_DIR:-worklog/opm-ressim-compare/$(date -u +%Y%m%dT%H%M%SZ)}"
CASE_FILTER=""
OPM_VARIANT_FILTER="same"
MODE="both"
DRY_RUN=0
BUILD_WASM=1

usage() {
  cat <<'USAGE'
Usage: scripts/opm-ressim-compare.sh [options]

Runs repeatable ResSim WASM diagnostics and, when the matching deck is present,
the corresponding OPM Flow case. Outputs go under worklog/ by default. OPM deck
inputs are tracked under opm/reference-decks/; generated Flow files are written
to the selected output directory.

Options:
  --case <name>       Run one case. Use --list to see names.
  --opm-variant <v>   OPM deck timestep variant: same | dt4 | dt16 | all.
                      Default: same.
  --ressim-only       Run only ResSim diagnostics.
  --opm-only          Run only OPM Flow decks.
  --out-dir <dir>     Output directory. Default: worklog/opm-ressim-compare/<utc-stamp>
  --flow-bin <path>   OPM Flow executable. Default: FLOW_BIN env or "flow".
  --no-build-wasm     Skip npm run build:wasm before ResSim runs.
  --dry-run           Print commands without running them.
  --list              List configured comparison cases.
  --help              Show this help.

Environment:
  FLOW_BIN            OPM Flow executable path/name.
  OUT_DIR             Output directory override.

Configured cases:
  water-medium-step1      ResSim: water-pressure 20x20x3 dt=0.25 steps=1
  water-medium-6step      ResSim: water-pressure 20x20x3 dt=0.25 steps=6
  gas-rate-10x10x3        ResSim: gas-rate 10x10x3 dt=0.25 steps=6
  heavy-water-12x12x3     ResSim: water-pressure 12x12x3 dt=1 steps=1
  heavy-water-finedt      ResSim: water-pressure 12x12x3 dt=0.0625 steps=16

OPM deck status:
  Phase-0 parity decks live in opm/reference-decks/. The historical heavy-water
  decks still live in worklog/opm-case3/ until they are promoted into the
  tracked reference-deck tree.
USAGE
}

case_names() {
  printf '%s\n' \
    water-medium-step1 \
    water-medium-6step \
    gas-rate-10x10x3 \
    heavy-water-12x12x3 \
    heavy-water-finedt
}

ressim_args_for_case() {
  case "$1" in
    water-medium-step1)
      printf '%s\n' "--preset water-pressure --grid 20x20x3 --dt 0.25 --steps 1 --diagnostic summary"
      ;;
    water-medium-6step)
      printf '%s\n' "--preset water-pressure --grid 20x20x3 --dt 0.25 --steps 6 --diagnostic summary"
      ;;
    gas-rate-10x10x3)
      printf '%s\n' "--preset gas-rate --grid 10x10x3 --dt 0.25 --steps 6 --diagnostic summary"
      ;;
    heavy-water-12x12x3)
      printf '%s\n' "--preset water-pressure --grid 12x12x3 --dt 1 --steps 1 --diagnostic summary"
      ;;
    heavy-water-finedt)
      printf '%s\n' "--preset water-pressure --grid 12x12x3 --dt 0.0625 --steps 16 --diagnostic summary"
      ;;
    *)
      return 1
      ;;
  esac
}

opm_deck_for_case() {
  local case_name="$1"
  local variant="${2:-same}"

  case "$case_name:$variant" in
    heavy-water-12x12x3:same)
      printf '%s\n' "worklog/opm-case3/CASE3.DATA"
      ;;
    heavy-water-finedt:same)
      printf '%s\n' "worklog/opm-case3/CASE3_finedt.DATA"
      ;;
    water-medium-step1:same)
      printf '%s\n' "opm/reference-decks/water-medium-step1/CASE.DATA"
      ;;
    water-medium-step1:dt4)
      printf '%s\n' "opm/reference-decks/water-medium-step1/CASE_DT4.DATA"
      ;;
    water-medium-step1:dt16)
      printf '%s\n' "opm/reference-decks/water-medium-step1/CASE_DT16.DATA"
      ;;
    water-medium-6step:same)
      printf '%s\n' "opm/reference-decks/water-medium-6step/CASE.DATA"
      ;;
    water-medium-6step:dt4)
      printf '%s\n' "opm/reference-decks/water-medium-6step/CASE_DT4.DATA"
      ;;
    water-medium-6step:dt16)
      printf '%s\n' "opm/reference-decks/water-medium-6step/CASE_DT16.DATA"
      ;;
    gas-rate-10x10x3:same)
      printf '%s\n' "opm/reference-decks/gas-rate-10x10x3/CASE.DATA"
      ;;
    gas-rate-10x10x3:dt4)
      printf '%s\n' "opm/reference-decks/gas-rate-10x10x3/CASE_DT4.DATA"
      ;;
    gas-rate-10x10x3:dt16)
      printf '%s\n' "opm/reference-decks/gas-rate-10x10x3/CASE_DT16.DATA"
      ;;
    *)
      return 1
      ;;
  esac
}

opm_variants_for_case() {
  local case_name="$1"
  if [[ "$OPM_VARIANT_FILTER" != "all" ]]; then
    printf '%s\n' "$OPM_VARIANT_FILTER"
    return
  fi

  case "$case_name" in
    water-medium-step1|water-medium-6step|gas-rate-10x10x3)
      printf '%s\n' same dt4 dt16
      ;;
    *)
      printf '%s\n' same
      ;;
  esac
}

opm_output_prefix_for_variant() {
  case "$1" in
    same)
      printf '%s\n' "opm"
      ;;
    *)
      printf '%s\n' "opm-$1"
      ;;
  esac
}

write_status_json() {
  local file="$1"
  local case_name="$2"
  local kind="$3"
  local status="$4"
  local detail="$5"
  if [[ "$DRY_RUN" -eq 1 ]]; then
    printf '+ write %s status=%s detail=%s\n' "$file" "$status" "$detail"
    return
  fi
  printf '{\n  "case": "%s",\n  "kind": "%s",\n  "status": "%s",\n  "detail": "%s"\n}\n' \
    "$case_name" "$kind" "$status" "$detail" > "$file"
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --case)
      CASE_FILTER="${2:?missing case name}"
      shift 2
      ;;
    --opm-variant)
      OPM_VARIANT_FILTER="${2:?missing OPM variant}"
      shift 2
      ;;
    --ressim-only)
      MODE="ressim"
      shift
      ;;
    --opm-only)
      MODE="opm"
      shift
      ;;
    --out-dir)
      OUT_DIR="${2:?missing output directory}"
      shift 2
      ;;
    --flow-bin)
      FLOW_BIN="${2:?missing flow executable}"
      shift 2
      ;;
    --no-build-wasm)
      BUILD_WASM=0
      shift
      ;;
    --dry-run)
      DRY_RUN=1
      shift
      ;;
    --list)
      case_names
      exit 0
      ;;
    --help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if [[ -n "$CASE_FILTER" ]] && ! case_names | grep -qx "$CASE_FILTER"; then
  echo "Unknown case: $CASE_FILTER" >&2
  echo "Known cases:" >&2
  case_names >&2
  exit 2
fi

case "$OPM_VARIANT_FILTER" in
  same|dt4|dt16|all)
    ;;
  *)
    echo "Unknown OPM variant: $OPM_VARIANT_FILTER" >&2
    echo "Known variants: same, dt4, dt16, all" >&2
    exit 2
    ;;
esac

if [[ "$DRY_RUN" -eq 0 ]]; then
  mkdir -p "$OUT_DIR"
fi

if [[ "$FLOW_BIN" == */* && -x "$FLOW_BIN" ]]; then
  FLOW_BIN="$(cd "$(dirname "$FLOW_BIN")" && pwd)/$(basename "$FLOW_BIN")"
fi

if [[ "$MODE" != "opm" && "$BUILD_WASM" -eq 1 ]]; then
  if [[ "$DRY_RUN" -eq 1 ]]; then
    echo "+ npm run build:wasm"
  else
    npm run build:wasm
  fi
fi

for case_name in $(case_names); do
  if [[ -n "$CASE_FILTER" && "$case_name" != "$CASE_FILTER" ]]; then
    continue
  fi

  case_dir="$OUT_DIR/$case_name"
  if [[ "$DRY_RUN" -eq 0 ]]; then
    mkdir -p "$case_dir"
  fi

  if [[ "$MODE" != "opm" ]]; then
    ressim_args="$(ressim_args_for_case "$case_name")"
    echo "== ResSim $case_name =="
    # shellcheck disable=SC2086
    if [[ "$DRY_RUN" -eq 1 ]]; then
      echo "+ node scripts/fim-wasm-diagnostic.mjs $ressim_args --json > $case_dir/ressim.json 2> $case_dir/ressim.log"
    else
      node scripts/fim-wasm-diagnostic.mjs $ressim_args --json > "$case_dir/ressim.json" 2> "$case_dir/ressim.log"
    fi
  fi

  if [[ "$MODE" != "ressim" ]]; then
    for opm_variant in $(opm_variants_for_case "$case_name"); do
      deck="$(opm_deck_for_case "$case_name" "$opm_variant" || true)"
      prefix="$(opm_output_prefix_for_variant "$opm_variant")"
      echo "== OPM $case_name [$opm_variant] =="
      if [[ -z "$deck" || ! -f "$deck" ]]; then
        write_status_json "$case_dir/$prefix-status.json" "$case_name" "$prefix" "missing-deck" "${deck:-unknown}"
        echo "missing OPM deck: ${deck:-unknown}"
        continue
      fi
      if ! command -v "$FLOW_BIN" >/dev/null 2>&1; then
        write_status_json "$case_dir/$prefix-status.json" "$case_name" "$prefix" "missing-flow-bin" "$FLOW_BIN"
        echo "missing OPM Flow executable: $FLOW_BIN"
        continue
      fi
      if [[ "$DRY_RUN" -eq 1 ]]; then
        echo "+ mkdir -p $case_dir/$prefix-work"
        echo "+ cp $deck $case_dir/$prefix-work/$(basename "$deck")"
        echo "+ (cd $case_dir/$prefix-work && $FLOW_BIN $(basename "$deck")) > $case_dir/$prefix.log 2>&1"
      else
        opm_run_dir="$case_dir/$prefix-work"
        mkdir -p "$opm_run_dir"
        cp "$deck" "$opm_run_dir/$(basename "$deck")"
        if (
          cd "$opm_run_dir"
          "$FLOW_BIN" "$(basename "$deck")"
        ) > "$case_dir/$prefix.log" 2>&1; then
          write_status_json "$case_dir/$prefix-status.json" "$case_name" "$prefix" "ran" "$deck"
        else
          write_status_json "$case_dir/$prefix-status.json" "$case_name" "$prefix" "failed" "$deck"
          echo "OPM Flow failed for $case_name [$opm_variant]; see $case_dir/$prefix.log" >&2
          exit 1
        fi
      fi
    done
  fi
done

echo "comparison output: $OUT_DIR"
