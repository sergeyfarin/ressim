"""Build and merge the SPE5 cross-flash fixture (#116). Run with `uv run python -I` or `python3 -I`.

    points  RESSIM_STATES JUTUL_STATES POINTS_OUT
        For cells 32, 33, 39, 40 and every month present in both dumps, emit four points: each
        simulator's own (p, z), and the swapped pairs (ResSim's z at Jutul's p, Jutul's z at
        ResSim's p). The originals and the swaps are labelled so they can be counted separately.

    merge   POINTS RESSIM_FLASH JUTUL_FLASH FIXTURE_OUT [--provenance JSON]
        Join the two flash outputs by label into one fixture with an agreement summary that counts
        the originals and the swaps separately.

    check   FIXTURE
        Re-validate a written fixture's records and recompute its summary and flags from them.

    compare COMMITTED FRESH
        A regenerated fixture against the committed one, provenance excluded (`--check` mode of
        tools/jutul/run-spe5-cross-flash.sh).

Phase labels are compared as L / V / 2P. MultiComponentFlash returns a vapour fraction, so a
`tight` V strictly inside (0, 1) is 2P, and a single phase takes Li's label (0 = L, 1 = V).

A null V is also what a failed MultiComponentFlash solve returns, so `merge` validates every record
before it counts anything and refuses to write a fixture otherwise:

- exactly the expected 192 points (4 cells x 12 reports x 4 kinds, 96 originals and 96 swaps), with
  unique labels, each present exactly once in both flash outputs;
- Jutul `tight`: no error and no warning, a stability report present; a 2P result converged with
  `stability.stable` false; a single-phase result with `stability.stable` true and Li's label 0 or 1;
- ResSim: no error, a phase of L, V or 2P, and beta in [0, 1] (strictly inside for 2P).
"""
import json
import math
import sys

CELLS = (32, 33, 39, 40)
REPORTS = range(205, 217)
EXPECTED_POINTS = len(CELLS) * len(REPORTS) * 4


def build_points(ressim_path, jutul_path, out_path):
    ressim = {s["month"]: s["snapshot"] for s in json.load(open(ressim_path))["states"]}
    jutul = {s["month"]: s for s in json.load(open(jutul_path))["states"]}
    points = []
    for month in sorted(set(ressim) & set(jutul)):
        r, j = ressim[month], jutul[month]
        for c in CELLS:
            rz = [column[c] for column in r["composition"]]
            rp = r["pressure"][c]
            jz, jp = j["z"][c], j["p_bar"][c]
            base = {"month": month, "cell": c}
            points.append({**base, "kind": "original", "side": "ressim",
                           "label": f"m{month} c{c} ressim", "p_bar": rp, "z": rz})
            points.append({**base, "kind": "original", "side": "jutul",
                           "label": f"m{month} c{c} jutul", "p_bar": jp, "z": jz})
            points.append({**base, "kind": "swap", "side": "ressim_z_at_jutul_p",
                           "label": f"m{month} c{c} ressim-z@jutul-p", "p_bar": jp, "z": rz})
            points.append({**base, "kind": "swap", "side": "jutul_z_at_ressim_p",
                           "label": f"m{month} c{c} jutul-z@ressim-p", "p_bar": rp, "z": jz})
    json.dump(points, open(out_path, "w"))
    print(f"{len(points)} points ({len(points) // 2} originals, {len(points) // 2} swaps)")


def jutul_phase(rec):
    """Jutul's phase label for a record that passed `jutul_problems`."""
    if rec["tight"]["V"] is not None:
        return "2P"
    return "V" if rec["li_label"] == 1 else "L"


def jutul_problems(rec):
    t = rec["tight"]
    problems = []
    if "error" in t:
        problems.append(f"error: {t['error']}")
    if t.get("warnings"):
        problems.append(f"warnings: {t['warnings']}")
    stability = t.get("stability")
    if not isinstance(stability, dict) or not isinstance(stability.get("stable"), bool):
        problems.append("no stability report")
        return problems
    v = t.get("V")
    if v is not None:
        if not (isinstance(v, (int, float)) and 0 < v < 1):
            problems.append(f"V={v!r} outside (0, 1)")
        if t.get("converged") is not True:
            problems.append("two-phase solve not converged")
        if stability["stable"]:
            problems.append("two-phase result with a stable stability verdict")
    else:
        if not stability["stable"]:
            problems.append("no split although the stability test found the feed unstable")
        if rec.get("li_label") not in (0, 1):
            problems.append(f"li_label={rec.get('li_label')!r}")
    return problems


def ressim_problems(rec):
    problems = []
    if "error" in rec:
        problems.append(f"error: {rec['error']}")
    phase, beta = rec.get("phase"), rec.get("beta")
    if phase not in ("L", "V", "2P"):
        problems.append(f"phase={phase!r}")
    if not isinstance(beta, (int, float)) or not 0 <= beta <= 1:
        problems.append(f"beta={beta!r}")
    elif phase == "2P" and not 0 < beta < 1:
        problems.append(f"two-phase beta={beta!r}")
    return problems


def by_label(records, what):
    out = {}
    for r in records:
        if r["label"] in out:
            sys.exit(f"{what}: duplicate label {r['label']!r}")
        out[r["label"]] = r
    return out


def validate(points, ressim, jutul):
    """Every reason the inputs cannot support an agreement count; empty when they can."""
    labels = by_label(points, "points")
    kinds = [pt["kind"] for pt in points]
    problems = []
    if len(points) != EXPECTED_POINTS or kinds.count("original") != kinds.count("swap"):
        problems.append(f"{len(points)} points ({kinds.count('original')} originals, "
                        f"{kinds.count('swap')} swaps); expected {EXPECTED_POINTS}, half each")
    if sorted({pt["month"] for pt in points}) != list(REPORTS):
        problems.append(f"reports {sorted({pt['month'] for pt in points})}, expected 205-216")
    for name, side in (("ResSim", ressim), ("Jutul", jutul)):
        if set(side) != set(labels):
            problems.append(f"{name} flash labels differ from the points: "
                            f"missing {sorted(set(labels) - set(side))[:5]}, "
                            f"extra {sorted(set(side) - set(labels))[:5]}")
    for label in labels:
        if label in ressim:
            problems += [f"{label} ResSim: {p}" for p in ressim_problems(ressim[label])]
        if label in jutul:
            problems += [f"{label} Jutul: {p}" for p in jutul_problems(jutul[label])]
    return problems


def require_valid(points, ressim, jutul):
    problems = validate(points, ressim, jutul)
    if problems:
        print("\n".join(problems[:50]), file=sys.stderr)
        sys.exit(f"refusing to report agreement: {len(problems)} invalid or missing results")


def agreement(points, ressim, jutul):
    merged, summary = [], {}
    for pt in points:
        r, j = ressim[pt["label"]], jutul[pt["label"]]
        jp = jutul_phase(j)
        jv = j["tight"]["V"] if jp == "2P" else (0.0 if jp == "L" else 1.0)
        agree_phase = r["phase"] == jp
        agree_beta = agree_phase and abs(r["beta"] - jv) <= 1e-6
        merged.append({**pt, "ressim": r, "jutul": j, "jutul_phase": jp,
                       "agree_phase": agree_phase, "agree_beta_1e-6": agree_beta})
        s = summary.setdefault(pt["kind"], {"points": 0, "agree_phase": 0, "agree_beta_1e-6": 0,
                                            "jutul_tight_unconverged": 0, "jutul_warned": 0})
        s["points"] += 1
        s["agree_phase"] += agree_phase
        s["agree_beta_1e-6"] += agree_beta
        s["jutul_tight_unconverged"] += jp == "2P" and not j["tight"]["converged"]
        s["jutul_warned"] += bool(j["tight"]["warnings"] or j["default"]["warnings"])
    disagreements = [m["label"] for m in merged if not m["agree_beta_1e-6"]]
    return merged, summary, disagreements


def merge(points_path, ressim_path, jutul_path, out_path, provenance):
    points = json.load(open(points_path))
    ressim = by_label(json.load(open(ressim_path))["points"], "ResSim flash")
    jutul = by_label(json.load(open(jutul_path))["points"], "Jutul flash")
    require_valid(points, ressim, jutul)
    merged, summary, disagreements = agreement(points, ressim, jutul)
    fixture = {"schema": "ressim-spe5-cross-flash-fixture/1", "provenance": provenance,
               "summary": summary, "disagreements": disagreements, "points": merged}
    json.dump(fixture, open(out_path, "w"), indent=1)
    print(json.dumps(summary, indent=1))
    print("disagreements:", disagreements)


DERIVED = ("ressim", "jutul", "jutul_phase", "agree_phase", "agree_beta_1e-6")


def split_fixture(fixture):
    points = [{k: v for k, v in m.items() if k not in DERIVED} for m in fixture["points"]]
    ressim = by_label([m["ressim"] for m in fixture["points"]], "ResSim flash")
    jutul = by_label([m["jutul"] for m in fixture["points"]], "Jutul flash")
    return points, ressim, jutul


def check(fixture_path):
    """Re-validate a written fixture and recompute its summary from the records it holds."""
    fixture = json.load(open(fixture_path))
    points, ressim, jutul = split_fixture(fixture)
    require_valid(points, ressim, jutul)
    merged, summary, disagreements = agreement(points, ressim, jutul)
    if summary != fixture["summary"] or disagreements != fixture["disagreements"]:
        sys.exit(f"stored summary does not match the records: {summary} {disagreements}")
    if merged != fixture["points"]:
        sys.exit("stored per-point agreement flags do not match the records")
    total = sum(s["points"] for s in summary.values())
    agree = sum(s["agree_beta_1e-6"] for s in summary.values())
    print(f"{fixture_path}: {total} valid points, {agree}/{total} agree; "
          f"disagreements {disagreements}")


def close(a, b, path):
    if isinstance(a, dict) and isinstance(b, dict):
        if a.keys() != b.keys():
            return [f"{path}: keys {sorted(a.keys() ^ b.keys())}"]
        return [d for k in a for d in close(a[k], b[k], f"{path}.{k}")]
    if isinstance(a, list) and isinstance(b, list):
        if len(a) != len(b):
            return [f"{path}: length {len(a)} vs {len(b)}"]
        return [d for i, (x, y) in enumerate(zip(a, b)) for d in close(x, y, f"{path}[{i}]")]
    if isinstance(a, float) and isinstance(b, float):
        return [] if math.isclose(a, b, rel_tol=1e-10, abs_tol=1e-14) else [f"{path}: {a} vs {b}"]
    return [] if a == b else [f"{path}: {a!r} vs {b!r}"]


def compare(committed_path, fresh_path):
    """A regenerated fixture against the committed one, provenance excluded."""
    a, b = (json.load(open(p)) for p in (committed_path, fresh_path))
    diffs = close({k: v for k, v in a.items() if k != "provenance"},
                  {k: v for k, v in b.items() if k != "provenance"}, "fixture")
    if diffs:
        print("\n".join(diffs[:50]), file=sys.stderr)
        sys.exit(f"regenerated fixture differs from {committed_path} in {len(diffs)} values")
    print(f"regenerated fixture matches {committed_path} (provenance excluded)")


if __name__ == "__main__":
    cmd = sys.argv[1]
    if cmd == "points":
        build_points(*sys.argv[2:5])
    elif cmd == "merge":
        prov = {}
        if "--provenance" in sys.argv:
            prov = json.loads(sys.argv[sys.argv.index("--provenance") + 1])
        merge(*sys.argv[2:6], prov)
    elif cmd == "check":
        check(sys.argv[2])
    elif cmd == "compare":
        compare(*sys.argv[2:4])
    else:
        sys.exit(f"unknown command {cmd}")
