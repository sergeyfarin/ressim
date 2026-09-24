from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from .artifacts import DEFAULT_ARTIFACT_DIR, DEFAULT_RUN_ROOT, build_artifact, run_flow, write_deck
from .cases import CASES


def _case_keys(value: str) -> list[str]:
    if value == "all":
        return sorted(CASES)
    if value not in CASES:
        known = ", ".join(sorted(CASES))
        raise SystemExit(f"Unknown case '{value}'. Known cases: {known}, all")
    return [value]


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="ResSim OPM Flow deck/artifact tooling")
    sub = parser.add_subparsers(dest="command", required=True)

    deck_parser = sub.add_parser("generate-deck", help="write an Eclipse-style DATA deck")
    deck_parser.add_argument("case", choices=sorted(CASES))
    deck_parser.add_argument("--output", type=Path)

    run_parser = sub.add_parser("run-flow", help="run installed OPM Flow for a generated deck")
    run_parser.add_argument("case", choices=sorted(CASES))
    run_parser.add_argument("--run-root", type=Path, default=DEFAULT_RUN_ROOT)

    artifact_parser = sub.add_parser("build-artifacts", help="write frontend JSON artifact metadata")
    artifact_parser.add_argument("case", choices=[*sorted(CASES), "all"])
    artifact_parser.add_argument("--artifact-dir", type=Path, default=DEFAULT_ARTIFACT_DIR)
    artifact_parser.add_argument("--generated-at", default="1970-01-01T00:00:00+00:00")
    artifact_parser.add_argument("--run-root", type=Path, default=DEFAULT_RUN_ROOT)

    args = parser.parse_args(argv)

    if args.command == "generate-deck":
        path = write_deck(CASES[args.case], args.output)
        print(path)
        return 0

    if args.command == "run-flow":
        path = run_flow(CASES[args.case], args.run_root)
        print(path)
        return 0

    if args.command == "build-artifacts":
        failed = []
        for key in _case_keys(args.case):
            path = build_artifact(CASES[key], args.artifact_dir, args.generated_at, args.run_root)
            artifact = json.loads(path.read_text(encoding="utf-8"))
            print(f"{path} [{artifact['status']}]")
            if artifact["status"] == "error":
                failed.append(f"{key}: {artifact['notes']}")
        # A unit mismatch or an unparseable summary fails generation outright;
        # the error artifact is left on disk only so the reason can be read.
        for reason in failed:
            print(f"error: {reason}", file=sys.stderr)
        return 1 if failed else 0

    parser.error(f"Unhandled command {args.command}")
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
