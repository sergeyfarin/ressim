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

**83 flashed states** across three fluid systems (C1/C10 binary, CO2/C1/C10 ternary, and the
six-component SPE5 fluid from #45): 49 two-phase, 30 single-liquid, 4 single-vapour, over four
isotherms, with bubble and dew crossings, trace
components and analytic derivatives with respect to `[p, z_0 .. z_(N-2)]`.

Two of those are **surface** states — `ternary_stcond_*`, at 1 bar and 288.15 K, which is the
decks' `STCOND 15.0 1.0`, and four are `ternary_bhpdep_*`, two-phase decane-rich states at 82–87
bar taken from C12's BHP depletion. The latter are there for the **viscosity**: `flowexp_comp`
writes `OIL_VISC` and `GAS_VISC` as identically zero, so a trajectory comparison cannot see them at
all, and a producer's rate is `WI · (kr/μ) · Δp`. They are two orders of magnitude below any other pressure here, because a
surface separation is a flash at 1 bar and nothing else in this fixture is. C5's separation had no
external reference until C12's depletion case needed one; both states passed every existing
comparison unchanged, with no widened tolerance.

**36 flash-free EOS states** (28 binary/ternary, 8 SPE5), evaluating the cubic directly with no stability test. Seven are
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

`quarter_five_spot/` is #47's **structured-grid trajectory** fixture: the same ternary flood on a
5 × 5 × 1 areal grid, so the x- and y-faces and a two-direction Peaceman radius are part of the
comparison. It is derived from `1d_comp/`'s deck and uses ResSim's pinned EOS data, so the reference
and the case solve one fluid. See [`quarter_five_spot/README.md`](quarter_five_spot/README.md).

Trajectory references for the black-oil path live in `../reference-decks/` and are a different
simulator (`flow`) entirely.

## Shared run record and scorecard (#22)

`bash scripts/validate-compositional.sh reference` runs `comp_reference_run_records` in release,
which writes each committed trajectory fixture's ResSim counterpart as `<case>.comp.json` in the
same `<case>.<run>.json` shape as the black-oil runs (`run: "comp"`). The cells carry `p`, `sw`,
`sg` and component-major `z`; the comparator grades `p`, `sg` and `z` because no compositional
fixture carries water. `sw` is extra in the record, not compared.

The case list is [`trajectory_cases.json`](trajectory_cases.json): `1d_comp`, `1d_comp_skin`,
`depletion`, `depletion_bhp`, `injection`, and `quarter_five_spot`. The refinement ladders under
`1d_comp/refined/` are a study, not scorecard cases. `tools/opm_flow/compare_small_direct.py`
reads the manifest with `--comp-reference-dir`, compares against the fixtures, and checks the
shared [`scorecard.json`](../reference-decks/small-direct/scorecard.json). The fixture stays the
oracle; the scorecard is a regression ratchet, and these fixtures are not timestep-converged
(see [`reference_convergence.json`](reference_convergence.json)), so a band is not an accuracy
claim.

The compositional half needs neither `flowexp_comp` nor the OPM Python bindings: the fixture is
committed and `opm.io` is imported only for live Flow output. Re-baseline deliberately, on a
committed tree:

```bash
bash scripts/validate-compositional.sh update-scorecard
```

A write merges the six compositional cases into the shared file and preserves the black-oil
cases the current invocation did not measure. The top-level `provenance.cases` names what that
write measured; each newly measured case also carries its own `measured` block.

## Build environment

Every fixture in this directory that comes out of OPM — the PTFlash fixture, `spe3/ptflash.json`
and the `flowexp_comp` trajectories — was last regenerated or re-verified in this environment:

| | |
| --- | --- |
| OS | Ubuntu 26.04.1 LTS (`resolute`) |
| Compiler | `g++ (Ubuntu 15.2.0-16ubuntu1) 15.2.0`, `-std=c++20` |
| OPM packages | `libopm-common-dev`, `libopm-simulators-dev`, `libopm-grid-dev` `2026.04-1~resolute`, from the OPM PPA |
| Dune | `libdune-common-dev` `2.11.0-1` (Ubuntu's own; no `~noble` suffix any more) |
| `flowexp_comp` | rebuilt by `build-flowexp-comp.sh` from the same pinned `release/2026.04/final` (`b82f21d`), with the compiler above |

The OPM release did not change: `2026.04` on both sides. The packaging (`~noble` → `~resolute`),
Dune (`2.11.0-1~noble` → `2.11.0-1`) and the compiler (g++ 13.3 → 15.2) did. `manifest.json` and
`spe3/manifest.json` record the same versions per generation.

### Re-baseline of 2026-10-29

Moved from Ubuntu 24.04 / g++ 13.3 / `2026.04-1~noble`. `bash scripts/validate-compositional.sh
all` re-derives every fixture below from the new build and passes, and these are all the files that
changed:

- `ptflash_fixtures.json` — values move in the last digits, worst 3e-8 relative outside the
  derivatives (`L` at `spe5_dew_stcond_t3e-4`, an absolute 3e-11). Twelve states changed how OPM
  reached its answer, all recorded per state: `binary_p80` now converges on `ssi+newton`;
  `ternary_p110`, `spe5_oil_p20` and three dew-sweep states fall back to plain `ssi`; and six
  dew-sweep states moved between the 1e-12 and 1e-9 tolerance rungs. That moves the dew sweep's
  derivative split from 26 OPM-checked / 7 difference-checked states to **24 / 9**
  (`comp_domain_spe5_dew_sweep_matches_the_oracle`). The worst gaps did not move: 1.9e-9 against
  OPM, 4.2e-9 against the difference, |ΔL| 3.7e-11 and |Δx|, |Δy| 1.4e-12. No bound changed.
- `spe3/ptflash.json` — roundoff, worst 6e-13 relative.
- `1d_comp/refined/n020/reference.json` — 32 saturations and mole fractions in the eighth
  significant digit (worst 6e-10 absolute); pressure is unchanged. Its only consumer, the release
  refinement study (`validate-compositional.sh refinement`), reproduces C12's recorded ladder to
  every printed digit.

Everything else `flowexp_comp` writes here (`1d_comp/`, `refined/n010`, `n040`, `skin/`, both
depletions, `injection/`, `quarter_five_spot/`) and the convergence census reproduced byte for
byte. `water_injection/` is a JutulDarcy reference and was not affected.

Not re-run: the black-oil `flow` references under `../reference-decks/`. `libopm-simulators-bin`
(which ships `/usr/bin/flow`) was not reinstalled by the upgrade, so their manifests still name
`flow 2026.04 (Ubuntu 2026.04-1~noble)`, which is what produced them.
