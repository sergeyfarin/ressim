#!/usr/bin/env bash
set -euo pipefail

# Run both single-cell depletion reference cases and regenerate their fixtures.
#
# Two controls, for two different reasons. The rate-controlled deck brackets the phase appearance
# and is where the surface separation is compared head to head; the BHP-controlled one in `bhp/`
# goes nowhere near a surface volume and is where the composition evolution is compared, because
# the reference's surface metering turned out to be self-inconsistent by 3.8% on a mixture.
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
# The BHP-controlled variant runs to completion.
EXPECTED_BHP_REPORT_STEPS=20

binary="${FLOWEXP_COMP:-$(cd "${repo_root}/.." && pwd)/ressim-opm-build/opm-simulators/build/bin/flowexp_comp}"
if [ ! -x "${binary}" ]; then
    echo "FAIL: ${binary} not found." >&2
    echo "      Build it first: bash tools/opm_compositional/build-flowexp-comp.sh" >&2
    exit 1
fi

work="$(mktemp -d)"
trap 'rm -rf "${work}"' EXIT

# name : deck : committed fixture : expected report steps
cases=(
    "rate:${deck}:${fixture}:${EXPECTED_REPORT_STEPS}"
    "bhp:${case_dir}/bhp/DEPLETION.DATA:${case_dir}/bhp/reference.json:${EXPECTED_BHP_REPORT_STEPS}"
)

status=0
for entry in "${cases[@]}"; do
    IFS=: read -r name case_deck committed expected <<< "${entry}"
    run_dir="${work}/${name}"
    mkdir -p "${run_dir}"
    cp "${case_deck}" "${run_dir}/DEPLETION.DATA"

    # The rate-controlled run's abort is expected; `|| true` keeps `set -e` from treating it as a
    # failure. What is checked is the fixture that comes out.
    (cd "${run_dir}" && "${binary}" DEPLETION.DATA > run.log 2>&1) || true

    produced="${run_dir}/reference.json"
    python3 "${repo_root}/tools/opm_compositional/extract_reference.py" \
        "${run_dir}/DEPLETION" --out "${produced}" --cells 1 \
        --summary-vectors WBHP:PROD FOPT FOPR FGPT FGPR

    steps="$(python3 -c "import json,sys; print(len(json.load(open(sys.argv[1]))['report_steps']))" "${produced}")"
    if [ "${steps}" -ne "${expected}" ]; then
        echo "FAIL: the ${name} reference produced ${steps} report steps, expected ${expected}." >&2
        echo "      The oracle's behaviour on this case has changed. See the deck's header." >&2
        tail -5 "${run_dir}/run.log" >&2
        exit 1
    fi

    if [[ "${1:-}" == "--check" ]]; then
        if diff -q "${produced}" "${committed}" > /dev/null 2>&1; then
            echo "fixture matches ${committed}" >&2
        else
            echo "FIXTURE MISMATCH against ${committed}" >&2
            diff "${committed}" "${produced}" 2>&1 | head -20 >&2
            status=1
        fi
    else
        mkdir -p "$(dirname "${committed}")"
        cp "${produced}" "${committed}"
        echo "wrote ${committed}" >&2
    fi
done

exit "${status}"
