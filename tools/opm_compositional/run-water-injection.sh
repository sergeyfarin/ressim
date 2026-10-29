#!/usr/bin/env bash
# #46: regenerate or verify the JutulDarcy water-injection-into-gas reference fixture. Needs Julia
# 1.12 and tools/jutul's pinned environment, already instantiated as documented there.
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
out="opm/compositional/water_injection"
mkdir -p "$out"
julia +1.12 --project=tools/jutul tools/jutul/water_injection_reference.jl "$work/reference.json"
if "$check"; then
    # Compare parsed numbers, not JSON dictionary ordering. Bounds are regeneration noise, far
    # below the separately declared cross-simulator acceptance bands in the Rust tests.
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
    elif isinstance(a, (float, int)) and not isinstance(a, bool):
        assert math.isclose(a, b, rel_tol=1e-10, abs_tol=1e-12), (path, a, b)
    else:
        assert a == b, (path, a, b)
compare(*(json.load(open(p)) for p in sys.argv[1:]))
PY
else
    cp "$work/reference.json" "$out/reference.json"
    # A dirty generation is explicitly provisional, never represented as a committed baseline.
    uv run --no-project --offline python - "$out" <<'PY'
import hashlib, json, pathlib, subprocess, sys
out = pathlib.Path(sys.argv[1])
paths = [out/"reference.json",
         pathlib.Path("tools/jutul/water_injection_reference.jl"),
         pathlib.Path("tools/jutul/Manifest.toml")]
manifest = {
    "generator": "bash tools/opm_compositional/run-water-injection.sh",
    "verify": "bash tools/opm_compositional/run-water-injection.sh --check",
    "generated_from_commit": subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip(),
    "worktree_clean_at_generation": not subprocess.check_output(["git", "status", "--porcelain"], text=True).strip(),
    "sha256": {str(p): hashlib.sha256(p.read_bytes()).hexdigest() for p in paths if p.exists()},
}
(out/"manifest.json").write_text(json.dumps(manifest, indent=2)+"\n")
PY
fi
echo "water-injection reference: OK"
