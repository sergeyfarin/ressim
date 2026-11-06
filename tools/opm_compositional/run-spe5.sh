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
julia +1.12 --project=tools/jutul tools/jutul/spe5_reference.jl "$work/reference.json"
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
    cp "$work/reference.json" "$out/reference.json"
    # A dirty generation is explicitly provisional, never represented as a committed baseline.
    uv run --no-project --offline python - "$out" <<'PY'
import hashlib, json, pathlib, subprocess, sys
out = pathlib.Path(sys.argv[1])
paths = [out/"reference.json", out/"case.json",
         pathlib.Path("tools/jutul/spe5_reference.jl"),
         pathlib.Path("tools/jutul/Project.toml"),
         pathlib.Path("tools/jutul/Manifest.toml")]
manifest = {
    "generator": "bash tools/opm_compositional/run-spe5.sh",
    "verify": "bash tools/opm_compositional/run-spe5.sh --check",
    "generated_from_commit": subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip(),
    "worktree_clean_at_generation": not subprocess.check_output(["git", "status", "--porcelain"], text=True).strip(),
    "sha256": {str(p): hashlib.sha256(p.read_bytes()).hexdigest() for p in paths if p.exists()},
}
(out/"manifest.json").write_text(json.dumps(manifest, indent=2)+"\n")
PY
fi
echo "SPE5 reference: OK"
