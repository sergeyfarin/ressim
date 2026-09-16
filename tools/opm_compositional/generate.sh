#!/usr/bin/env bash
# Build the OPM PTFlash harness and regenerate the committed compositional fixture.
#
# Everything about this run that could change a number - compiler, OPM package versions, flash
# tolerance, method chain - is captured in the manifest this script writes next to the fixture,
# so a future session can tell whether a diff is a real change or a different environment.
#
# Usage:  bash tools/opm_compositional/generate.sh [--check]
#   (no args)  regenerate opm/compositional/ptflash_fixtures.json and its manifest
#   --check    build and run, but fail if the result differs from what is committed

set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "${here}/../.." && pwd)"
out_dir="${root}/opm/compositional"
fixture="${out_dir}/ptflash_fixtures.json"
manifest="${out_dir}/manifest.json"
bin="$(mktemp -d)/ptflash_harness"

# -std=c++20 is required, not preferred: opm/material/Constants.hpp and PolynomialUtils.hpp use
# std::numbers, and dune/common/std/algorithm.hh hard-errors without three-way comparison.
# NDEBUG is deliberately NOT defined - OPM's EOS asserts are the harness's last line of defence
# against emitting an unphysical molar volume as though it were a reference value.
CXX="${CXX:-g++}"
CXXFLAGS=(-std=c++20 -O2 -I/usr/include -I/usr/include/dune)
LDLIBS=(-ldunecommon -lfmt)

echo "building harness with ${CXX}" >&2
"${CXX}" "${CXXFLAGS[@]}" "${here}/ptflash_harness.cpp" -o "${bin}" "${LDLIBS[@]}"

tmp="$(mktemp)"
"${bin}" > "${tmp}"

if [[ "${1:-}" == "--check" ]]; then
    if diff -q "${tmp}" "${fixture}" > /dev/null; then
        echo "fixture matches ${fixture}" >&2
        exit 0
    fi
    echo "FIXTURE MISMATCH against ${fixture}" >&2
    diff "${fixture}" "${tmp}" | head -40 >&2
    exit 1
fi

mkdir -p "${out_dir}"
mv "${tmp}" "${fixture}"

opm_ver="$(dpkg-query -W -f='${Version}' libopm-common-dev)"
opm_sim_ver="$(dpkg-query -W -f='${Version}' libopm-simulators-dev)"
dune_ver="$(dpkg-query -W -f='${Version}' libdune-common-dev)"
cxx_ver="$("${CXX}" --version | head -1)"

cat > "${manifest}" <<JSON
{
  "fixture": "ptflash_fixtures.json",
  "generator": "tools/opm_compositional/generate.sh",
  "source": "tools/opm_compositional/ptflash_harness.cpp",
  "regenerate": "bash tools/opm_compositional/generate.sh",
  "verify": "bash tools/opm_compositional/generate.sh --check",
  "environment": {
    "compiler": "${cxx_ver}",
    "cxx_standard": "c++20",
    "libopm-common-dev": "${opm_ver}",
    "libopm-simulators-dev": "${opm_sim_ver}",
    "libdune-common-dev": "${dune_ver}"
  },
  "sha256": "$(sha256sum "${fixture}" | cut -d' ' -f1)",
  "generated_from_commit": "$(cd "${root}" && git rev-parse HEAD)",
  "worktree_clean_at_generation": $(cd "${root}" && [[ -z "$(git status --porcelain)" ]] && echo true || echo false)
}
JSON

echo "wrote ${fixture}" >&2
echo "wrote ${manifest}" >&2
