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

**47 flashed states** across two fluid systems (C1/C10 binary, CO2/C1/C10 ternary): 29 two-phase,
15 single-liquid, 3 single-vapour, over two isotherms, with bubble and dew crossings, trace
components and analytic derivatives with respect to `[p, z_0 .. z_(N-2)]`.

**21 flash-free EOS states**, evaluating the cubic directly with no stability test. Seven are
genuinely multi-root. They exist because *none* of the 47 flashed states has three real roots, so
without them the root-labelling rule would be untested; they also cover pure components, which
PTFlash cannot flash but the EOS evaluates fine.

Read `tools/opm_compositional/README.md` before using it — in particular the *Oracle domain
limits* section, which records the states OPM's PTFlash cannot resolve and therefore cannot
referee.

This directory holds **thermodynamics only**. Trajectory references for the black-oil path live
in `../reference-decks/`. There is no compositional trajectory reference: no compositional Flow
executable exists on this machine.
