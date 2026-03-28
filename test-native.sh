#!/usr/bin/env bash
set -euo pipefail

cd /home/reken/Repos/ressim/src/lib/ressim

SCENARIOS=(
  # 1D waterflood (pressure)
  "wf_p_24              fim_debug_wf_p_24"
  "wf_p_48              fim_debug_wf_p_48"
  "wf_p_100             fim_debug_wf_p_100"
  # 2D waterflood (pressure)
  "wf_p_12x12           fim_debug_wf_p_12x12"
  "wf_p_24x24           fim_debug_wf_p_24x24"
  # 3D waterflood (pressure)
  "wf_p_12x12x3         fim_debug_wf_p_12x12x3"
  "wf_p_24x24x2         fim_debug_wf_p_24x24x2"
  # Waterflood (rate-controlled)
  "wf_r_24               fim_debug_wf_r_24"
  "wf_r_12x12            fim_debug_wf_r_12x12"
  "wf_r_12x12x3          fim_debug_wf_r_12x12x3"
  # Waterflood breakthrough
  "wf_bt_24              fim_debug_wf_bt_24"
  "wf_bt_48              fim_debug_wf_bt_48"
  "wf_bt_12x12           fim_debug_wf_bt_12x12"
  "wf_bt_12x12x3         fim_debug_wf_bt_12x12x3"
  "sweep_areal          fim_debug_sweep_areal"
  # SPE1 depletion (three-phase)
  "spe1                  fim_debug_spe1_depletion"
  # Gas injection
  "gas_24                fim_debug_gas_24"
  "gas_12x12             fim_debug_gas_12x12"
  "gas_12x12x3           fim_debug_gas_12x12x3"
  "gas_10x10x3           fim_debug_gas_10x10x3"
  # Legacy probe
  "probe_24              native_single_step_fim_probe_case_a_24_cells"
)

usage() {
  echo "Usage: $0 [scenario|all]"
  echo ""
  echo "Runs native FIM debug tests with per-iteration diagnostics."
  echo "Diagnostic output goes to stderr; redirect with 2>file to capture."
  echo ""
  echo "Available scenarios:"
  printf "  %-20s %s\n" "SCENARIO" "DESCRIPTION"
  printf "  %-20s %s\n" "--------" "-----------"
  printf "  %-20s %s\n" "wf_p_24"      "1D waterflood pressure 24×1×1"
  printf "  %-20s %s\n" "wf_p_48"      "1D waterflood pressure 48×1×1"
  printf "  %-20s %s\n" "wf_p_100"     "1D waterflood pressure 100×1×1"
  printf "  %-20s %s\n" "wf_p_12x12"   "2D waterflood pressure 12×12×1"
  printf "  %-20s %s\n" "wf_p_24x24"   "2D waterflood pressure 24×24×1"
  printf "  %-20s %s\n" "wf_p_12x12x3" "3D waterflood pressure 12×12×3"
  printf "  %-20s %s\n" "wf_p_24x24x2" "3D waterflood pressure 24×24×2"
  printf "  %-20s %s\n" "wf_r_24"      "1D waterflood rate 24×1×1"
  printf "  %-20s %s\n" "wf_r_12x12"   "2D waterflood rate 12×12×1"
  printf "  %-20s %s\n" "wf_r_12x12x3" "3D waterflood rate 12×12×3"
  printf "  %-20s %s\n" "wf_bt_24"     "1D breakthrough 24×1×1"
  printf "  %-20s %s\n" "wf_bt_48"     "1D breakthrough 48×1×1"
  printf "  %-20s %s\n" "wf_bt_12x12"  "2D breakthrough 12×12×1"
  printf "  %-20s %s\n" "wf_bt_12x12x3""3D breakthrough 12×12×3"
  printf "  %-20s %s\n" "sweep_areal"  "Frontend areal sweep baseline 21×21×1"
  printf "  %-20s %s\n" "spe1"         "SPE1 depletion 10×10×3 (three-phase)"
  printf "  %-20s %s\n" "gas_24"       "1D gas injection 24×1×1"
  printf "  %-20s %s\n" "gas_12x12"    "2D gas injection 12×12×1"
  printf "  %-20s %s\n" "gas_12x12x3"  "3D gas injection 12×12×3"
  printf "  %-20s %s\n" "gas_10x10x3"  "3D gas injection 10×10×3"
  printf "  %-20s %s\n" "probe_24"     "Legacy single-step probe 24 cells"
  printf "  %-20s %s\n" "all"          "Run all scenarios sequentially"
  echo ""
  echo "Examples:"
  echo "  $0 wf_p_24"
  echo "  $0 gas_12x12x3 2>debug.log"
  echo "  $0 all"
}

if [[ $# -eq 0 ]]; then
  usage
  exit 0
fi

TARGET="$1"

run_scenario() {
  local name="$1"
  local test_fn="$2"
  local exact_test_fn="$test_fn"
  if [[ "$test_fn" == fim_debug_* ]]; then
    exact_test_fn="tests::fim_debug::$test_fn"
  fi
  echo "▶ Running scenario: $name ($test_fn)"
  cargo test --release "$exact_test_fn" -- --ignored --exact --nocapture
  echo ""
}

if [[ "$TARGET" == "all" ]]; then
  for entry in "${SCENARIOS[@]}"; do
    read -r name test_fn <<< "$entry"
    run_scenario "$name" "$test_fn"
  done
else
  found=false
  for entry in "${SCENARIOS[@]}"; do
    read -r name test_fn <<< "$entry"
    if [[ "$name" == "$TARGET" ]]; then
      run_scenario "$name" "$test_fn"
      found=true
      break
    fi
  done
  if [[ "$found" == "false" ]]; then
    echo "Unknown scenario: $TARGET"
    echo ""
    usage
    exit 1
  fi
fi
