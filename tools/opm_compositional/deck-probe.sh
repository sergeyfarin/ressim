#!/usr/bin/env bash
set -euo pipefail

# Print what opm-common makes of a deck: the connection transmissibility factors (with the well
# index they imply in ResSim's units), the wellbore radius and equivalent radius, and the SGOF
# table evaluated wherever you ask.
#
# This is the tool that settled C12's connection-rate forensics — see
# docs/COMPOSITIONAL_C12_FORENSICS.md §2. It exists because comparing against a reimplementation
# of Peaceman verifies the formula in the plan, not the number the reference used.
#
# Usage:  bash tools/opm_compositional/deck-probe.sh <deck> [Sg ...]
#   e.g.  bash tools/opm_compositional/deck-probe.sh \
#             opm/compositional/depletion/bhp/DEPLETION.DATA 0.1871 0.22794

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
bin="$(mktemp -d)/deck_probe"
trap 'rm -rf "$(dirname "${bin}")"' EXIT

if [ ! -f /usr/include/opm/input/eclipse/Parser/Parser.hpp ]; then
    echo "FAIL: opm-common headers not installed (libopm-common-dev)." >&2
    exit 1
fi

"${CXX:-g++}" -std=c++20 -O2 -I/usr/include "${here}/deck_probe.cpp" -o "${bin}" -lopmcommon
"${bin}" "$@"
