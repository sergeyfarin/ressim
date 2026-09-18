#!/usr/bin/env python3
"""Derive a variant of `1D_COMP.DATA`: a finer grid, a connection skin, or both.

C12 asks for a refined *external* solution, not merely a coarse one, so the reference has to be
re-run on finer grids. `--cells` rewrites only the keywords whose length or value depends on the
cell count: the domain, the fluid, the wells, the controls, the report times and the output
requests are all the deck's own.

`--skin` is for a different problem. On the deck as written the reservoir carries about ten times
more flow resistance than either well, so both wells end up within a bar or two of their own BHP
limits and `q = WI * lambda * (BHP - p)` turns a 0.03 bar state agreement into several per cent on
the rate. A skin moves the resistance to the well: at 60 the drawdowns become 10 bar on the
injector and 34 on the producer, and a cumulative becomes a discriminating observable rather than
a small difference of large numbers.

Every substitution asserts on the text it expects to find, so a change to the source deck fails
here rather than silently producing a deck that is no longer the same case.

Usage:  python3 refine_deck.py 1D_COMP.DATA --cells 20 --out 1D_COMP_N020.DATA
        python3 refine_deck.py 1D_COMP.DATA --skin 60 --out 1D_COMP_SKIN.DATA
"""

from __future__ import annotations

import argparse
from pathlib import Path

# The source deck's own discretisation, and the total length it covers. The refined deck keeps the
# length and the well positions (first and last cell); only the number of cells changes.
SOURCE_CELLS = 5
SOURCE_DX_M = 60.0
LENGTH_M = SOURCE_CELLS * SOURCE_DX_M


def substitute(text: str, old: str, new: str) -> str:
    if text.count(old) != 1:
        raise SystemExit(
            f"expected exactly one occurrence of {old!r}, found {text.count(old)}; "
            "the source deck has changed and this script must be updated with it"
        )
    return text.replace(old, new)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("deck", type=Path)
    parser.add_argument("--cells", type=int, default=SOURCE_CELLS)
    parser.add_argument(
        "--skin",
        type=float,
        default=None,
        help="connection skin for both wells (COMPDAT item 11); omitted leaves the deck's default of 0",
    )
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()

    n = args.cells
    if n < SOURCE_CELLS:
        raise SystemExit(f"--cells must be at least {SOURCE_CELLS}")
    dx = LENGTH_M / n

    text = args.deck.read_text()

    # Geometry.
    text = substitute(text, "DIMENS\n5 1 1\n", f"DIMENS\n{n} 1 1\n")
    text = substitute(text, "DXV\n5*60\n", f"DXV\n{n}*{dx!r}\n")
    text = substitute(text, "TOPS\n5*0\n", f"TOPS\n{n}*0\n")

    # Rock. Uniform, so refinement is a pure change of resolution.
    for keyword in ("PERMX", "PERMY", "PERMZ"):
        text = substitute(text, f"{keyword}\n5*100\n", f"{keyword}\n{n}*100\n")
    text = substitute(text, "PORO\n5*0.1\n", f"PORO\n{n}*0.1\n")

    # Initial state. Also uniform.
    text = substitute(text, "PRESSURE\n5*75.\n", f"PRESSURE\n{n}*75.\n")
    text = substitute(text, "SGAS\n5*1.\n", f"SGAS\n{n}*1.\n")
    text = substitute(text, "TEMPI\n5*150\n", f"TEMPI\n{n}*150\n")
    text = substitute(
        text,
        "ZMF\n5*0.1\n5*0.3\n5*0.6\n",
        f"ZMF\n{n}*0.1\n{n}*0.3\n{n}*0.6\n",
    )

    # The producer stays in the last cell; the injector is already in the first.
    text = substitute(text, "PROD FIELD 5 1 1* GAS /", f"PROD FIELD {n} 1 1* GAS /")
    text = substitute(text, "PROD 5 1 1 1 OPEN 2* 0.0151 /", f"PROD {n} 1 1 1 OPEN 2* 0.0151 /")

    if args.skin is not None:
        # COMPDAT item 9 is the wellbore DIAMETER, item 10 the effective Kh (defaulted with `1*`)
        # and item 11 the skin. Getting item 9 wrong was worth 12-15% on the well index; the
        # positions here are spelled out so the next reader does not have to count.
        text = substitute(
            text,
            "INJ  1  1 1 1 OPEN 2* 0.0151 /",
            f"INJ  1  1 1 1 OPEN 2* 0.0151 1* {args.skin!r} /",
        )
        text = substitute(
            text,
            f"PROD {n} 1 1 1 OPEN 2* 0.0151 /",
            f"PROD {n} 1 1 1 OPEN 2* 0.0151 1* {args.skin!r} /",
        )

    args.out.write_text(text)
    skin = "" if args.skin is None else f", skin {args.skin}"
    print(f"wrote {args.out}: {n} cells of {dx} m{skin}", flush=True)


if __name__ == "__main__":
    main()
