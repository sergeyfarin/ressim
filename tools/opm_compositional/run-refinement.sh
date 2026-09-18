#!/usr/bin/env bash
set -euo pipefail

# Re-run the 1D compositional case on the refined grids C12's refinement study uses.
#
# The plan asks for "a refined external solution, not merely a coarse Flow output", so the
# reference itself is re-run at 10, 20 and 40 cells over the same 300 m. Only the discretisation
# changes: refine_deck.py asserts on every keyword it rewrites, so a change to the source deck
# fails there rather than quietly producing a different case.
#
# Usage:  bash tools/opm_compositional/run-refinement.sh [--check]

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
case_dir="${repo_root}/opm/compositional/1d_comp"
deck="${case_dir}/1D_COMP.DATA"
refined_dir="${case_dir}/refined"

# The ladder. Each step doubles the resolution, which is what makes a convergence rate readable.
CELL_COUNTS=(10 20 40)

binary="${FLOWEXP_COMP:-$(cd "${repo_root}/.." && pwd)/ressim-opm-build/opm-simulators/build/bin/flowexp_comp}"
if [ ! -x "${binary}" ]; then
    echo "FAIL: ${binary} not found." >&2
    echo "      Build it first: bash tools/opm_compositional/build-flowexp-comp.sh" >&2
    exit 1
fi

work="$(mktemp -d)"
trap 'rm -rf "${work}"' EXIT

status=0
for cells in "${CELL_COUNTS[@]}"; do
    name="$(printf 'n%03d' "${cells}")"
    run_dir="${work}/${name}"
    mkdir -p "${run_dir}"
    python3 "${repo_root}/tools/opm_compositional/refine_deck.py" \
        "${deck}" --cells "${cells}" --out "${run_dir}/1D_COMP.DATA" >&2

    (cd "${run_dir}" && "${binary}" 1D_COMP.DATA > run.log 2>&1) || {
        echo "FAIL: the ${cells}-cell run did not complete." >&2
        tail -20 "${run_dir}/run.log" >&2
        exit 1
    }

    python3 "${repo_root}/tools/opm_compositional/extract_reference.py" \
        "${run_dir}/1D_COMP" --out "${run_dir}/reference.json" --cells "${cells}"

    fixture="${refined_dir}/${name}/reference.json"
    if [[ "${1:-}" == "--check" ]]; then
        if diff -q "${run_dir}/reference.json" "${fixture}" > /dev/null 2>&1; then
            echo "fixture matches ${fixture}" >&2
        else
            echo "FIXTURE MISMATCH against ${fixture}" >&2
            diff "${fixture}" "${run_dir}/reference.json" 2>&1 | head -20 >&2
            status=1
        fi
    else
        mkdir -p "$(dirname "${fixture}")"
        cp "${run_dir}/reference.json" "${fixture}"
        echo "wrote ${fixture}" >&2
    fi
done

exit "${status}"
