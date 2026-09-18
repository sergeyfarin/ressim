#!/usr/bin/env bash
set -euo pipefail

# Measure how far each compositional reference is from being timestep-converged, and check that
# against what is recorded.
#
# WHY THIS EXISTS. C12 compared ResSim's timestep-converged answers against a reference that takes
# one backward-Euler step per report interval, and read the difference as a model defect. It was
# not. The retraction is in docs/COMPOSITIONAL_C12_FORENSICS.md; this script is the guard.
#
# It does NOT assert that the references are converged — none of them is. It records HOW FAR each
# one moves when its own TSTEP ladder is halved, so that:
#
#   * no acceptance band can be set below a reference's own temporal uncertainty without that
#     being visible, and
#   * if a reference's behaviour changes upstream, the gate fails instead of the number quietly
#     drifting.
#
# A fixture whose movement is recorded as `diverges` is one whose reference gets WORSE under
# refinement. That is a property of the oracle, not of ResSim, and it is recorded so it cannot be
# mistaken for either.
#
# Usage:  bash tools/opm_compositional/check-reference-convergence.sh [--check]

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
census="${repo_root}/opm/compositional/reference_convergence.json"

binary="${FLOWEXP_COMP:-$(cd "${repo_root}/.." && pwd)/ressim-opm-build/opm-simulators/build/bin/flowexp_comp}"
if [ ! -x "${binary}" ]; then
    echo "SKIP: flowexp_comp not built; the census cannot be remeasured." >&2
    echo "      Build it with: bash tools/opm_compositional/build-flowexp-comp.sh" >&2
    exit 0
fi

# name : deck : cells
cases=(
    "1d_comp:${repo_root}/opm/compositional/1d_comp/1D_COMP.DATA:5"
    "1d_comp_skin:SKIN:5"
    "depletion_orat:${repo_root}/opm/compositional/depletion/DEPLETION.DATA:1"
    "depletion_bhp:${repo_root}/opm/compositional/depletion/bhp/DEPLETION.DATA:1"
)

work="$(mktemp -d)"
trap 'rm -rf "${work}"' EXIT

python3 - "${work}" "${binary}" "${census}" "${1:-}" "${cases[@]}" <<'PY'
import json
import subprocess
import sys
from pathlib import Path

work = Path(sys.argv[1])
binary = sys.argv[2]
census_path = Path(sys.argv[3])
mode = sys.argv[4]
cases = sys.argv[5:]

repo = Path(__file__).resolve().parent if False else Path(census_path).parents[2]
tools = repo / "tools" / "opm_compositional"
sys.path.insert(0, str(tools))
import extract_reference as ex  # noqa: E402


def run(deck: Path, out_dir: Path) -> list[dict]:
    """Run the deck and return its per-report cell arrays. Tolerates an expected abort."""
    out_dir.mkdir(parents=True, exist_ok=True)
    local = out_dir / deck.name
    local.write_text(deck.read_text())
    subprocess.run([binary, local.name], cwd=out_dir, capture_output=True)
    stem = local.stem
    for candidate in (stem, stem.upper()):
        unrst = out_dir / f"{candidate}.UNRST"
        if unrst.exists():
            funrst = out_dir / f"{candidate}.FUNRST"
            if not funrst.exists():
                subprocess.run(["convertECL", unrst.name], cwd=out_dir, check=True,
                               capture_output=True)
            steps = ex.parse_formatted_restart(funrst)
            return [s for s in steps if "PRESSURE" in s]
    raise SystemExit(f"no restart produced for {deck}")


measured = {}
for entry in cases:
    name, deck_spec, cells = entry.split(":")
    cells = int(cells)
    case_dir = work / name
    case_dir.mkdir(parents=True, exist_ok=True)

    if deck_spec == "SKIN":
        deck = case_dir / "1D_COMP.DATA"
        subprocess.run(
            ["python3", str(tools / "refine_deck.py"),
             str(repo / "opm/compositional/1d_comp/1D_COMP.DATA"), "--skin", "60",
             "--out", str(deck)],
            check=True, capture_output=True)
    else:
        deck = Path(deck_spec)

    # Two refinements, not one. A single halving is not enough to tell a converging reference from
    # a diverging one, and the ORAT depletion is exactly that case: it moves 0.011 bar on the first
    # halving and then accelerates. What distinguishes them is whether successive movements shrink.
    decks = [deck]
    for level in (1, 2):
        finer = case_dir / f"refine{level}_{deck.name}"
        subprocess.run(
            ["python3", str(tools / "halve_tstep.py"), str(decks[-1]), "--out", str(finer)],
            check=True, capture_output=True)
        decks.append(finer)

    runs = [run(d, case_dir / f"level{i}") for i, d in enumerate(decks)]

    def movement(coarse, fine, stride):
        """Worst and final pressure movement at the coarse run's own report times."""
        shared = 0
        worst = 0.0
        final = 0.0
        for k in range(len(coarse)):
            j = stride * (k + 1) - 1
            if j >= len(fine):
                break
            step = max(abs(coarse[k]["PRESSURE"][c] - fine[j]["PRESSURE"][c])
                       for c in range(cells))
            worst = max(worst, step)
            final = step
            shared = k + 1
        return worst, final, shared

    first_worst, first_final, shared = movement(runs[0], runs[1], 2)
    second_worst, second_final, _ = movement(runs[0], runs[2], 4)

    # The trend is the point. A converging reference moves less the second time; a diverging one
    # moves more. `second` is measured against the SAME coarse run as `first`, so if the reference
    # converges it must be the larger of the two only by the amount still left to converge.
    growth = (second_worst / first_worst) if first_worst > 0 else float("inf")
    trend = "diverging" if growth > 2.5 else "converging"

    measured[name] = {
        "report_steps": len(runs[0]),
        "compared": shared,
        "halved_worst_bar": round(first_worst, 4),
        "halved_final_bar": round(first_final, 4),
        "quartered_worst_bar": round(second_worst, 4),
        "quartered_final_bar": round(second_final, 4),
        "growth": round(growth, 3),
        "trend": trend,
    }

document = {
    "schema": "ressim-compositional-reference-convergence/1",
    "note": (
        "How far each compositional reference moves when its own TSTEP ladder is halved, and then "
        "halved again. None of them is converged; this records how far from it each one is, so "
        "that no acceptance band is set below a reference's own temporal uncertainty. `growth` is "
        "the second movement over the first, both measured against the same coarse run: a "
        "converging reference is well under 2, a diverging one is unbounded. See "
        "docs/COMPOSITIONAL_C12_FORENSICS.md."
    ),
    "units": {"pressure": "bar"},
    "fixtures": measured,
}

text = json.dumps(document, indent=1, sort_keys=False) + "\n"
if mode == "--check":
    if not census_path.exists():
        raise SystemExit(f"FAIL: {census_path} does not exist; run without --check first")
    committed = census_path.read_text()
    if committed == text:
        print(f"census matches {census_path}", file=sys.stderr)
        raise SystemExit(0)
    print(f"CENSUS MISMATCH against {census_path}", file=sys.stderr)
    print("committed:", file=sys.stderr)
    print(committed, file=sys.stderr)
    print("measured:", file=sys.stderr)
    print(text, file=sys.stderr)
    raise SystemExit(1)

census_path.write_text(text)
print(f"wrote {census_path}", file=sys.stderr)
for name, m in measured.items():
    print(
        f"  {name:16s} halved {m['halved_worst_bar']:8.4f} bar -> quartered "
        f"{m['quartered_worst_bar']:8.4f} bar  growth {m['growth']:6.2f}  {m['trend']}",
        file=sys.stderr,
    )
PY
