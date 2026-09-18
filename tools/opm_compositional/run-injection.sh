#!/usr/bin/env bash
set -euo pipefail

# Run the single-cell mixture-injection reference case and regenerate its fixture.
#
# This is the only fixture in C12 that exercises a SURFACE-RATE CONTROL ON A MIXTURE. Everywhere
# else a rate control is exercised the stream is pure CO2, or the reference does not converge.
#
# A caveat that belongs with it: this reference does not converge either. Its final pressure walks
# 183.60 (1 d) -> 184.16 (0.5 d) -> 185.16 (0.25 d) -> 185.20 (0.125 d) -> 191.94 (0.0625 d), the
# same 1/dt signature the ORAT depletion has, which tells us the behaviour is not specific to
# producers. The comparison against it is therefore made at MATCHED resolution — one step per
# report interval, which is what the reference takes and where the spurious term is smallest. See
# docs/COMPOSITIONAL_C12_FORENSICS.md.
#
# Usage:  bash tools/opm_compositional/run-injection.sh [--check]

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
case_dir="${repo_root}/opm/compositional/injection"
deck="${case_dir}/INJECTION.DATA"
fixture="${case_dir}/reference.json"
EXPECTED_REPORT_STEPS=20

binary="${FLOWEXP_COMP:-$(cd "${repo_root}/.." && pwd)/ressim-opm-build/opm-simulators/build/bin/flowexp_comp}"
if [ ! -x "${binary}" ]; then
    echo "FAIL: ${binary} not found." >&2
    echo "      Build it first: bash tools/opm_compositional/build-flowexp-comp.sh" >&2
    exit 1
fi

work="$(mktemp -d)"
trap 'rm -rf "${work}"' EXIT
cp "${deck}" "${work}/"
cd "${work}"

"${binary}" INJECTION.DATA > run.log 2>&1 || {
    echo "FAIL: the injection reference did not complete." >&2
    tail -20 run.log >&2
    exit 1
}

tmp_out="${work}/reference.json"
python3 "${repo_root}/tools/opm_compositional/extract_reference.py" \
    "${work}/INJECTION" --out "${tmp_out}" --cells 1 \
    --summary-vectors WBHP:INJ FGIT FGIR

steps="$(python3 -c "import json,sys; print(len(json.load(open(sys.argv[1]))['report_steps']))" "${tmp_out}")"
if [ "${steps}" -ne "${EXPECTED_REPORT_STEPS}" ]; then
    echo "FAIL: the reference produced ${steps} report steps, expected ${EXPECTED_REPORT_STEPS}." >&2
    exit 1
fi

if [[ "${1:-}" == "--check" ]]; then
    if diff -q "${tmp_out}" "${fixture}" > /dev/null 2>&1; then
        echo "fixture matches ${fixture}" >&2
        exit 0
    fi
    echo "FIXTURE MISMATCH against ${fixture}" >&2
    diff "${fixture}" "${tmp_out}" 2>&1 | head -20 >&2
    exit 1
fi

mkdir -p "$(dirname "${fixture}")"
cp "${tmp_out}" "${fixture}"
echo "wrote ${fixture}" >&2
