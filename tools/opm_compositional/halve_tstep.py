#!/usr/bin/env python3
"""Halve every `TSTEP` in a deck, so the same case is reported at the same times but solved with
twice as many steps.

This exists for the convergence census (`check-reference-convergence.sh`). C12 spent several
commits comparing ResSim's timestep-converged answers against a reference that takes one
backward-Euler step per report interval, and read the difference as a model defect. The census
measures how far each reference is from converged so that no acceptance band can be set below its
own temporal uncertainty again. See `docs/COMPOSITIONAL_C12_FORENSICS.md`.

Every original report time survives: a `TSTEP` of `n*v` becomes `2n*(v/2)`, so report `k` of the
original is report `2k+1` of the halved deck.

Usage:  python3 halve_tstep.py DECK --out HALVED
"""

from __future__ import annotations

import argparse
import re
from pathlib import Path

# A TSTEP block runs from the keyword to the terminating slash. Entries are `value` or `count*value`.
TSTEP_BLOCK = re.compile(r"(^[ \t]*TSTEP[ \t]*$)(.*?)(^[ \t]*/[ \t]*$)", re.MULTILINE | re.DOTALL)
ENTRY = re.compile(r"(?:(\d+)\*)?([0-9]*\.?[0-9]+(?:[eEdD][-+]?\d+)?)")


def halve_block(body: str) -> str:
    out_lines = []
    for line in body.split("\n"):
        stripped = line.strip()
        if not stripped or stripped.startswith("--"):
            out_lines.append(line)
            continue
        indent = line[: len(line) - len(line.lstrip())]
        entries = []
        for count, value in ENTRY.findall(stripped):
            n = int(count) if count else 1
            entries.append(f"{2 * n}*{float(value) / 2.0!r}")
        if not entries:
            out_lines.append(line)
            continue
        out_lines.append(indent + " ".join(entries))
    return "\n".join(out_lines)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("deck", type=Path)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()

    text = args.deck.read_text()
    blocks = TSTEP_BLOCK.findall(text)
    if not blocks:
        raise SystemExit(f"{args.deck} has no TSTEP block; nothing to halve")

    halved = TSTEP_BLOCK.sub(lambda m: m.group(1) + halve_block(m.group(2)) + m.group(3), text)
    args.out.write_text(halved)
    print(f"wrote {args.out}: {len(blocks)} TSTEP block(s) halved", flush=True)


if __name__ == "__main__":
    main()
