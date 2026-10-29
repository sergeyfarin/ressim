#!/usr/bin/env bash
set -euo pipefail

# Run the 2-D quarter-five-spot compositional reference case and regenerate its fixture.
#
# This is #47's structured-grid trajectory oracle: a 5 x 5 x 1 areal grid in ResSim's pinned
# ternary, with the same CO2 / methane / decane fluid and SGOF table as 1D_COMP.DATA. It exists
# because the 1D displacement cannot see an areal transmissibility, a second cell direction or a
# Peaceman radius that depends on both cell widths, and #47 turns those on.
#
# Two details are load-bearing:
#
# * There is one pre-well TSTEP of 0.001 d. Without a report step before WELSPECS this build of
#   flowexp_comp writes the initial restart state for every report step while its summary advances,
#   so every per-cell array would be the initial state and the fixture would look stationary. The
#   1D deck happens to have the same four pre-well steps for its own reason (see run-1d-comp.sh).
# * The injector is a surface-rate control at 100 000 sm3/day with a 150 bar limit, and stays
#   rate-limited. ResSim's public payload has no surface-rate control for a pinned fluid, so the
#   comparison uses the equivalent molar rate computed from the deck's own `STCOND 15.0 1.0`.
#   Both sides therefore inject the same moles; the cumulative check in
#   `comp_reference_quarter_five_spot_matches_opm` verifies that rather than assuming it.
#
# Usage:  bash tools/opm_compositional/run-quarter-five-spot.sh [--check]

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
case_dir="${repo_root}/opm/compositional/quarter_five_spot"
deck="${case_dir}/QUARTER_FIVE_SPOT.DATA"
fixture="${case_dir}/reference.json"
EXPECTED_REPORT_STEPS=11

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

"${binary}" QUARTER_FIVE_SPOT.DATA > run.log 2>&1 || {
    echo "FAIL: the quarter-five-spot reference did not complete." >&2
    tail -20 run.log >&2
    exit 1
}

tmp_out="${work}/reference.json"
python3 "${repo_root}/tools/opm_compositional/extract_reference.py" \
    "${work}/QUARTER_FIVE_SPOT" --out "${tmp_out}" --cells 25 \
    --summary-vectors FGIT FGPT FOPT FGIR FGPR FOPR

steps="$(python3 -c "import json,sys; print(len(json.load(open(sys.argv[1]))['report_steps']))" "${tmp_out}")"
if [ "${steps}" -ne "${EXPECTED_REPORT_STEPS}" ]; then
    echo "FAIL: the reference produced ${steps} report steps, expected ${EXPECTED_REPORT_STEPS}." >&2
    exit 1
fi

# Guard the restart/summary alignment directly: the pre-well step must be stationary and the first
# post-well step must not be. A regression in the output offset is otherwise easy to mistake for a
# physics change when the committed fixture is regenerated.
python3 - "${tmp_out}" <<'PY'
import json
import sys

with open(sys.argv[1]) as handle:
    reference = json.load(handle)
first, second = reference["report_steps"][:2]
if len(set(first["pressure"])) != 1 or first["pressure"][0] != 75.0:
    raise SystemExit("FAIL: the pre-well report step is not the initial 75 bar state")
if len(set(second["pressure"])) == 1:
    raise SystemExit("FAIL: the first post-well report step did not move")
if not reference["summary"]["FGIT"][-1] == 100000.0:
    raise SystemExit("FAIL: the reference did not inject its expected 100 000 sm3")
PY

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
