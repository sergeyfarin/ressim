#!/usr/bin/env bash
set -euo pipefail

# Run the 1D compositional reference case and regenerate its fixture.
#
# Needs `flowexp_comp`, which is built by build-flowexp-comp.sh and is not packaged. The run itself
# takes well under a second; the build is the expensive part and is done once.
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

if [[ "${1:-}" == "--check" ]]; then
    if diff -q "${tmp_out}" "${fixture}" > /dev/null; then
        echo "fixture matches ${fixture}" >&2
        exit 0
    fi
    echo "FIXTURE MISMATCH against ${fixture}" >&2
    diff "${fixture}" "${tmp_out}" | head -30 >&2
    exit 1
fi

cp "${tmp_out}" "${fixture}"
echo "wrote ${fixture}" >&2
