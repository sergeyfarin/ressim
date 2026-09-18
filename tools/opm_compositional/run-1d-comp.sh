#!/usr/bin/env bash
set -euo pipefail

# Run the 1D compositional reference case and its skin variant, and regenerate their fixtures.
#
# The skin variant exists because on the deck as written the reservoir carries about ten times more
# flow resistance than either well, so both wells sit within a bar or two of their own BHP limits.
# `q = WI * lambda * (BHP - p)` then turns a 0.03 bar state agreement into several per cent on the
# rate, which is what keeps the plain deck's cumulative from being a usable acceptance observable.
# A skin of 60 moves the resistance to the well — drawdowns of 10 bar on the injector and 34 on the
# producer — and makes a cumulative discriminating. `refine_deck.py` writes the variant and asserts
# on every keyword it rewrites.
#
# Needs `flowexp_comp`, which is built by build-flowexp-comp.sh and is not packaged. The runs
# themselves take well under a second; the build is the expensive part and is done once.
#
# Usage:  bash tools/opm_compositional/run-1d-comp.sh [--check]

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
case_dir="${repo_root}/opm/compositional/1d_comp"
deck="${case_dir}/1D_COMP.DATA"
fixture="${case_dir}/reference.json"

binary="${FLOWEXP_COMP:-$(cd "${repo_root}/.." && pwd)/ressim-opm-build/opm-simulators/build/bin/flowexp_comp}"
if [ ! -x "${binary}" ]; then
    echo "FAIL: ${binary} not found." >&2
    echo "      Build it first: bash tools/opm_compositional/build-flowexp-comp.sh" >&2
    echo "      Or point FLOWEXP_COMP at an existing one." >&2
    exit 1
fi

work="$(mktemp -d)"
trap 'rm -rf "${work}"' EXIT
cp "${deck}" "${work}/"
cd "${work}"

"${binary}" 1D_COMP.DATA > run.log 2>&1 || {
    echo "FAIL: the reference run did not complete." >&2
    tail -20 run.log >&2
    exit 1
}

tmp_out="${work}/reference.json"
python3 "${repo_root}/tools/opm_compositional/extract_reference.py" \
    "${work}/1D_COMP" --out "${tmp_out}" --cells 5

# The skin variant, in its own directory beside the base case.
skin_dir="${work}/skin"
mkdir -p "${skin_dir}"
python3 "${repo_root}/tools/opm_compositional/refine_deck.py" \
    "${deck}" --skin "${SKIN:-60}" --out "${skin_dir}/1D_COMP.DATA" >&2
(cd "${skin_dir}" && "${binary}" 1D_COMP.DATA > run.log 2>&1) || {
    echo "FAIL: the skin variant did not complete." >&2
    tail -20 "${skin_dir}/run.log" >&2
    exit 1
}
python3 "${repo_root}/tools/opm_compositional/extract_reference.py" \
    "${skin_dir}/1D_COMP" --out "${skin_dir}/reference.json" --cells 5
skin_fixture="${case_dir}/skin/reference.json"

if [[ "${1:-}" == "--check" ]]; then
    status=0
    for pair in "${tmp_out}:${fixture}" "${skin_dir}/reference.json:${skin_fixture}"; do
        produced="${pair%%:*}"
        committed="${pair##*:}"
        if diff -q "${produced}" "${committed}" > /dev/null 2>&1; then
            echo "fixture matches ${committed}" >&2
        else
            echo "FIXTURE MISMATCH against ${committed}" >&2
            diff "${committed}" "${produced}" 2>&1 | head -20 >&2
            status=1
        fi
    done
    exit "${status}"
fi

cp "${tmp_out}" "${fixture}"
echo "wrote ${fixture}" >&2
mkdir -p "$(dirname "${skin_fixture}")"
cp "${skin_dir}/reference.json" "${skin_fixture}"
echo "wrote ${skin_fixture}" >&2
