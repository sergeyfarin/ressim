# Physics Review (Archived)

## Status

This document is retained only as historical context.

It captured an earlier physics audit before several major implementation slices landed. It should not be used as the current implementation reference.

## Use These Docs Instead

- `README.md` for the current product and physics summary
- `docs/P4_TWO_PHASE_BENCHMARKS.md` for validated Buckley-Leverett benchmark behavior
- `docs/UNIT_SYSTEM.md` and `docs/TRANSMISSIBILITY_FACTOR.md` for current unit documentation
- `TODO.md` for remaining physics and correctness work

## Why This Note Was Archived

The original review included findings that are no longer true in the current repository state, including:

- capillary pressure being absent
- relative-permeability endpoint scaling being absent
- outdated unit-system guidance centered on `0.001127`
- older well/state-loading issues that were later fixed or moved to tracked TODO items

Keeping the old review in active form was misleading because later changes resolved or superseded a meaningful portion of it.

## Historical Value That Still Remains

The archived review was still useful for two reasons:

1. it identified several real physics gaps early enough to drive later implementation work, and
2. it showed which areas deserve recurring scrutiny: unit consistency, well control semantics, saturation bounds, and material balance.

If a new physics audit is needed, start from the live Rust sources under `src/lib/ressim/src/` and record current findings in a new document rather than reviving the outdated content that used to live here.