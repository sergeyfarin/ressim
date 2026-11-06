#!/usr/bin/env bash
# #52/#116: regenerate or verify the SPE5 Case 1 JutulDarcy compositional reference.
# The full 20-year schedule takes hours; --check replays it and compares parsed numbers.
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"
check=false
for arg in "$@"; do
    case "$arg" in
        --check) check=true ;;
        *) echo "usage: $0 [--check]" >&2; exit 2 ;;
    esac
done
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
out="opm/compositional/spe5"

# A caught exception or an incomplete simulate! call still writes a JSON file. Never let that
# replace the committed fixture: require the full 240-report, 7305 d horizon first.
validate_reference() {
    uv run --no-project --offline python - "$1" <<'PY'
import json, math, sys

path = sys.argv[1]
with open(path) as f:
    result = json.load(f)
assert result.get("schema") == "ressim-spe5-reference/1", result.get("schema")
assert result.get("completed") is True, f"completed={result.get('completed')!r}"
assert result.get("failed_at") is None, f"failed_at={result.get('failed_at')!r}"
reports = result.get("reports")
assert isinstance(reports, list), "reports is not a list"
assert len(reports) == 240, f"expected 240 reports, got {len(reports)}"
for i, report in enumerate(reports, start=1):
    expected = 30.4375*i
    assert math.isclose(report["day"], expected, rel_tol=0.0, abs_tol=1e-9), (
        f"report {i} is at {report['day']} d, expected {expected} d")
assert math.isclose(float(result.get("reached_day")), 7305.0, rel_tol=0.0, abs_tol=1e-9), (
    f"reached_day={result.get('reached_day')!r}")
PY
}

julia +1.12 --project=tools/jutul tools/jutul/spe5_reference.jl "$work/reference.json"
validate_reference "$work/reference.json"
if "$check"; then
    # Compare parsed numbers, not JSON dictionary ordering. The same Julia and pinned JutulDarcy
    # replay bit-for-bit; the tolerance only absorbs JSON float formatting noise.
    uv run --no-project --offline python - "$out/reference.json" "$work/reference.json" <<'PY'
import json, math, sys
def compare(a, b, path="root"):
    if isinstance(a, dict):
        assert a.keys() == b.keys(), path
        for key in a:
            compare(a[key], b[key], f"{path}.{key}")
    elif isinstance(a, list):
        assert len(a) == len(b), path
        for i, (x, y) in enumerate(zip(a, b)):
            compare(x, y, f"{path}[{i}]")
    elif isinstance(a, (float, int)):
        assert math.isclose(a, b, rel_tol=1e-10, abs_tol=1e-12), (path, a, b)
    else:
        assert a == b, (path, a, b)
compare(*(json.load(open(p)) for p in sys.argv[1:]))
PY
else
    # Build the manifest from the validated temporary reference, then publish both artifacts.
    # A dirty generation is explicitly provisional, never represented as a committed baseline.
    uv run --no-project --offline python - "$out" "$work/reference.json" "$work/manifest.json" <<'PY'
import hashlib, json, pathlib, subprocess, sys
out = pathlib.Path(sys.argv[1])
reference = pathlib.Path(sys.argv[2])
manifest_out = pathlib.Path(sys.argv[3])
paths = {
    str(out/"reference.json"): reference,
    str(out/"case.json"): out/"case.json",
    "tools/jutul/spe5_reference.jl": pathlib.Path("tools/jutul/spe5_reference.jl"),
    "tools/jutul/Project.toml": pathlib.Path("tools/jutul/Project.toml"),
    "tools/jutul/Manifest.toml": pathlib.Path("tools/jutul/Manifest.toml"),
}
manifest = {
    "generator": "bash tools/opm_compositional/run-spe5.sh",
    "verify": "bash tools/opm_compositional/run-spe5.sh --check",
    "generated_from_commit": subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip(),
    "worktree_clean_at_generation": not subprocess.check_output(["git", "status", "--porcelain"], text=True).strip(),
    "sha256": {name: hashlib.sha256(path.read_bytes()).hexdigest()
               for name, path in paths.items() if path.exists()},
}
manifest_out.write_text(json.dumps(manifest, indent=2)+"\n")
PY
    cp "$work/reference.json" "$out/reference.json"
    cp "$work/manifest.json" "$out/manifest.json"
fi
echo "SPE5 reference: OK"
