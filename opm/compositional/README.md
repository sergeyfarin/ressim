# Compositional reference fixtures

`ptflash_fixtures.json` is the independent thermodynamic oracle for the compositional plan's
C2–C5 tasks. It is **generated**, never hand-edited.

| | |
| --- | --- |
| Produced by | [`tools/opm_compositional/`](../../tools/opm_compositional/README.md) |
| Regenerate | `bash tools/opm_compositional/generate.sh` |
| Verify unchanged | `bash tools/opm_compositional/generate.sh --check` |
| Environment, checksum, source commit | `manifest.json` |
| Admission targets and results | [`docs/COMPOSITIONAL_VALIDATION.md`](../../docs/COMPOSITIONAL_VALIDATION.md) |

**49 flashed states** across two fluid systems (C1/C10 binary, CO2/C1/C10 ternary): 31 two-phase,
15 single-liquid, 3 single-vapour, over three isotherms, with bubble and dew crossings, trace
components and analytic derivatives with respect to `[p, z_0 .. z_(N-2)]`.

Two of those are **surface** states — `ternary_stcond_*`, at 1 bar and 288.15 K, which is the
decks' `STCOND 15.0 1.0`. They are two orders of magnitude below any other pressure here, because a
surface separation is a flash at 1 bar and nothing else in this fixture is. C5's separation had no
external reference until C12's depletion case needed one; both states passed every existing
comparison unchanged, with no widened tolerance.

**28 flash-free EOS states**, evaluating the cubic directly with no stability test. Seven are
genuinely multi-root. They exist because *none* of the flashed states has three real roots, so
without them the root-labelling rule would be untested; they also cover pure components, which
PTFlash cannot flash but the EOS evaluates fine.

Seven of them — `eos_depletion_*` — are there for a different reason: they carry OPM's own molar
volume at each pressure the depletion reference reported, so C12 can compare the **difference**
between consecutive states rather than the states themselves. An accumulation term uses `Δc`, and
`Δc` is 0.4% of `c` along that path, so agreeing on `c` to 0.1% would say almost nothing about it.

Read `tools/opm_compositional/README.md` before using it — in particular the *Oracle domain
limits* section, which records the states OPM's PTFlash cannot resolve and therefore cannot
referee.

`1d_comp/` holds the **trajectory** reference: OPM's `flowexp_comp` running a five-cell 1D CO2
flood in ResSim's own V1 fluid, plus the same deck refined to 10, 20 and 40 cells for C12's
refinement study. See [`1d_comp/README.md`](1d_comp/README.md), including the three places that
deck's data deliberately differs from the pinned specification.

`depletion/` holds C12's **second** fixture: a single cell taken down through its own saturation
pressure at a fixed surface oil rate. Its reference run aborts when the oracle's own Rachford-Rice
gives out one step after gas appears, which is kept rather than worked around. See
[`depletion/README.md`](depletion/README.md).

Trajectory references for the black-oil path live in `../reference-decks/` and are a different
simulator (`flow`) entirely.
