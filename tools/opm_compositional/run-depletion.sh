#!/usr/bin/env bash
set -euo pipefail

# Run the single-cell depletion reference case and regenerate its fixture.
#
# **This run is expected to abort**, and that is the finding rather than a fault to work around.
# flowexp_comp depletes the cell cleanly to 111.55 bar single phase, produces one report step at
# 109.10 bar with gas just appeared, and then its own Rachford-Rice stops converging. Every
# two-phase flash method it offers fails the same way and halving the rate only moves where. The
# seven report steps it does produce are the fixture; the deck's header records the abort.
#
# So this script checks the number of report steps rather than the exit status, which would
# otherwise make an expected abort indistinguishable from a real failure.
#
# Usage:  bash tools/opm_compositional/run-depletion.sh [--check]

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
case_dir="${repo_root}/opm/compositional/depletion"
deck="${case_dir}/DEPLETION.DATA"
fixture="${case_dir}/reference.json"

# How far the oracle gets before its flash gives out. A change here is a change in the oracle, not
# in the case, and it must be noticed rather than absorbed.
EXPECTED_REPORT_STEPS=7

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

# The abort is expected; `|| true` keeps `set -e` from treating it as a failure. What is checked
# is the fixture that comes out.
"${binary}" DEPLETION.DATA > run.log 2>&1 || true

tmp_out="${work}/reference.json"
python3 "${repo_root}/tools/opm_compositional/extract_reference.py" \
    "${work}/DEPLETION" --out "${tmp_out}" --cells 1 \
    --summary-vectors WBHP:PROD FOPT FOPR FGPT FGPR

steps="$(python3 -c "import json,sys; print(len(json.load(open(sys.argv[1]))['report_steps']))" "${tmp_out}")"
if [ "${steps}" -ne "${EXPECTED_REPORT_STEPS}" ]; then
    echo "FAIL: the reference produced ${steps} report steps, expected ${EXPECTED_REPORT_STEPS}." >&2
    echo "      The oracle's behaviour on this case has changed. See the deck's header." >&2
    tail -5 run.log >&2
    exit 1
fi

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
