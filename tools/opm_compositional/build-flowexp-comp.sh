#!/usr/bin/env bash
set -euo pipefail

# Build OPM's experimental compositional simulator, `flowexp_comp`.
#
# This is the **trajectory oracle** the compositional plan's C12 requires, and it is not packaged:
# `libopm-simulators-bin` ships only the black-oil `flow`. The source lives in
# `opm-simulators/flowexperimental/comp/`, which Debian does not ship either.
#
# Everything that could change a result is pinned below. Build once; the binary is reused.
#
# Usage:  bash tools/opm_compositional/build-flowexp-comp.sh [work_dir]
#   work_dir defaults to ../ressim-opm-build relative to the repository root, i.e. OUTSIDE the
#   repository — this produces ~1 GB of build tree and a 10 MB binary, none of which belongs in git.

# The 2026.04 release, matching the installed libopm-common-dev / libopm-simulators-dev packages.
# Building against a different upstream revision than the installed headers is the mismatch the
# readiness assessment warns about, so the tag is pinned rather than tracking a branch.
OPM_REF="release/2026.04/final"
OPM_COMMIT="b82f21dba405286c4c4446614dd3bf9cdebf7a2c"

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
work_dir="${1:-$(cd "${repo_root}/.." && pwd)/ressim-opm-build}"
src="${work_dir}/opm-simulators"

mkdir -p "${work_dir}"

if [ ! -d "${src}/.git" ]; then
    echo "cloning opm-simulators at ${OPM_REF}" >&2
    git clone --depth 1 --branch "${OPM_REF}" https://github.com/OPM/opm-simulators.git "${src}"
fi

actual="$(cd "${src}" && git rev-parse HEAD)"
if [ "${actual}" != "${OPM_COMMIT}" ]; then
    echo "FAIL: ${src} is at ${actual}, expected ${OPM_COMMIT} (${OPM_REF})." >&2
    echo "      Refusing to build against an unpinned revision." >&2
    exit 1
fi

# --- Three build-tree patches. None changes OPM's behaviour; all three exist because this is a
# --- targeted build of one executable in an environment that lacks some optional test dependencies.
#
#   1. `target_sources(test_* ...)` is unconditional, and the test targets do not exist when
#      BUILD_TESTING is off.
#   2. `install(TARGETS flow ...)` runs even when the `flow` target was not created.
#   3. `test_tuning_trgmbe` and `test_tuning_tsinit_nextstep` are added unconditionally and link
#      Boost::unit_test_framework, which `libboost-test-dev` provides and which is not installed
#      here. Guarding them avoids installing a system package for two tests unrelated to
#      `flowexp_comp`.
#
# Applied idempotently so a re-run is safe.
python3 - "${src}/CMakeLists.txt" <<'PY'
import re, sys
path = sys.argv[1]
s = open(path).read()
if "ResSim build-tree patch" in s:
    print("patches already applied", file=sys.stderr)
    raise SystemExit(0)

s = re.sub(
    r"^(\s*)target_sources\((test_[A-Za-z0-9_]+) PRIVATE \$<TARGET_OBJECTS:moduleVersion>\)$",
    r"\1if(TARGET \2)\n\1  target_sources(\2 PRIVATE $<TARGET_OBJECTS:moduleVersion>)\n\1endif()",
    s, flags=re.M)

s = s.replace("""  if (BUILD_FLOW)
    install(TARGETS flow DESTINATION bin)""",
"""  if (BUILD_FLOW AND TARGET flow)
    install(TARGETS flow DESTINATION bin)""")

old = """  opm_add_test(test_tuning_trgmbe
    ONLY_COMPILE
    SOURCES
      tests/test_tuning_TRGMBE.cpp
    LIBRARIES
      Boost::unit_test_framework
  )

  opm_add_test(test_tuning_tsinit_nextstep
    ONLY_COMPILE
    SOURCES
      tests/test_tuning_TSINIT_NEXTSTEP.cpp
     LIBRARIES
      Boost::unit_test_framework opmcommon
  )"""
new = """  # ResSim build-tree patch: these link Boost::unit_test_framework, which is not installed here
  # and is unrelated to flowexp_comp.
  if(TARGET Boost::unit_test_framework)
""" + "\n".join("  " + line if line.strip() else line for line in old.splitlines()) + """
  endif()"""
assert old in s, "the Boost test block moved; re-check the patch against this revision"
s = s.replace(old, new, 1)
open(path, "w").write(s)
print("patches applied", file=sys.stderr)
PY

cd "${src}"

# OPM_COMPILE_COMPONENTS restricts the instantiated component counts. ResSim's V1 is N=2 and N=3,
# and every extra count is a substantial compile.
cmake -B build \
    -DCMAKE_BUILD_TYPE=Release \
    -DOPM_COMPILE_COMPONENTS="2;3" \
    -DOPM_ENABLE_PYTHON=OFF \
    -DBUILD_TESTING=OFF

cmake --build build --target flowexp_comp -j "$(nproc)"

binary="${src}/build/bin/flowexp_comp"
if [ ! -x "${binary}" ]; then
    echo "FAIL: ${binary} was not produced." >&2
    exit 1
fi

echo
echo "built: ${binary}"
"${binary}" --help > /dev/null && echo "runs: yes"
