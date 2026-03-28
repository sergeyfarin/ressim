#!/usr/bin/env bash
set -euo pipefail

cd /home/reken/Repos/ressim/src/lib/ressim

echo "Running native Rust release FIM probe with the same 24-cell one-step case as test-wasm.sh..."
echo "Watch for: ms, history, and warning. Healthy runs keep history low, not in the thousands."

cargo test --release \
  native_single_step_fim_probe_case_a_24_cells \
  -- --ignored --nocapture