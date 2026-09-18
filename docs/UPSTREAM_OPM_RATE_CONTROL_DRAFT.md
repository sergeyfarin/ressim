# Draft issue for OPM/opm-simulators — not filed

**Status: a draft for the maintainer to review and decide whether to post.** Nothing here has been
sent anywhere. It is written to stand alone: an OPM maintainer needs only `flowexp_comp`, the deck
below, and the `PTFlash` header — nothing from this repository.

Background and the full investigation that produced it:
[`COMPOSITIONAL_C12_FORENSICS.md`](COMPOSITIONAL_C12_FORENSICS.md).

---

## Title

`flowexp_comp`: rate-controlled compositional wells do not converge under timestep refinement (BHP-controlled ones do)

## Environment

| | |
| --- | --- |
| `opm-simulators` | `release/2026.04/final`, `b82f21dba405286c4c4446614dd3bf9cdebf7a2c` |
| `opm-common`, `opm-grid`, `opm-material` | 2026.04 Debian packages (`2026.04-1~noble`) |
| Binary | `flowexp_comp`, built with `-DOPM_COMPILE_COMPONENTS="2;3"` |
| Platform | Linux x86-64, GCC, `-O2` |

## Summary

On a single-cell compositional case with one **rate-controlled** well, refining the `TSTEP` ladder
does not converge the solution. The trajectory drifts and then jumps. The same case with a
**BHP-controlled** well converges normally, as does every BHP-controlled case we have tried.

Independently, the simulator's own trajectory is inconsistent with its own `PTFlash` at the deck's
`STCOND`: it delivers roughly 0.73× the moles that flash says the prescribed surface rate
corresponds to.

## Reproducer

One cell, 100 × 100 × 10 m, 10 000 m³ pore volume, initially single-phase liquid at 150 bar and
150 °C with `ZMF = 0.1 / 0.3 / 0.6`. One injector on `WCONINJE ... RATE 2000` delivering a
0.5 CO₂ / 0.3 C₁ / 0.2 C₁₀ stream. The 400 bar limit is never approached (BHP runs 152 → 184), so
the well is on **rate** control throughout. The cell is closed apart from the well.

The `PROPS` section is `opm-tests` `compositional/1D_COMP.DATA`'s verbatim. The schedule is:

```
WELSPECS
INJ FIELD 1 1 1* GAS /
/
COMPDAT
INJ 1 1 1 1 OPEN 2* 0.0151 /
/
WCONINJE
INJ GAS OPEN RATE 2000 1* 400 /
/
WELLSTRE
ISTR 0.5 0.3 0.2 /
/
WINJGAS
INJ STREAM ISTR /
/
TSTEP
    20*1.0
/
END
```

with

```
DIMENS  1 1 1 /     DXV 100 /   DYV 100 /   DZV 10 /   TOPS 1*0 /
PERMX/Y/Z 1*100 /   PORO 1*0.1 /
PRESSURE 1*150. /   SGAS 1*0. /   TEMPI 1*150 /   ZMF 1*0.1 1*0.3 1*0.6 /
ROCK 68.9476 0 /    STCOND 15.0 1.0 /
```

Refine by subdividing `TSTEP` — `20*1.0` → `40*0.5` → `80*0.25` → … — which preserves every
original report time (report `k` becomes report `2k+1`).

### Observed

Cell pressure at t = 20 days:

```
TSTEP      1.0 d     0.5 d     0.25 d    0.125 d   0.0625 d
p(20 d)   183.598   184.160   185.156   185.196   191.935
```

It looks like it is settling near 185.2 and then jumps by 6.7 bar. It is a **threshold** between
0.125 and 0.0625 days rather than a smooth trend, and refining further does not recover it.

The same behaviour appears on a **producer** under `WCONPROD ... ORAT`, so it is not specific to
well direction. There the implied molar withdrawal walks 227 888 → 231 373 → 244 887 → 269 638
mol/day across `TSTEP` 1.0 → 0.05 → 0.01 → 0.0025 d without settling.

**BHP-controlled wells on the same grid and fluid converge normally**: halving and quartering the
ladder moves them by 1.60 and 2.69 bar respectively, a ratio of 1.68, which is ordinary first-order
behaviour.

### Second, possibly related observation

The cell is closed apart from the well, so the change in what it holds is the injected amount in
moles. Evaluating the deck's own `PROPS` with `Opm::PTFlash` at `STCOND` (1 bar, 288.15 K) for the
injected stream gives `L = 0.2032`, a liquid molar volume of 2.0765e-4 and a vapour molar volume of
2.3847e-2 m³/mol, so one mole of feed occupies 1.9043e-2 m³ at surface, and 2000 sm³/day is
**105 025 mol/day**.

The run's own trajectory injects **76 583 mol/day** — 0.73× that. `FGIT` reads exactly 2000 × t
throughout, so the control equation is satisfied in its own terms.

## What has been ruled out

| Hypothesis | Test | Result |
| --- | --- | --- |
| Wellbore storage term `(new_component_masses − component_masses)/dt` | `wellbore_volume_` reduced 1e-6× in a local build | **Refuted.** `p(20 d)` moves to 183.574 / 184.111 / 184.998 / 184.993 / 191.460 — the jump survives |
| Wellbore flash tolerance (`flashFluidState_` uses `ssi` at `1.e-6`) | tightened to `1.e-9` and `1.e-11` | **Cannot be tightened.** At 1e-9 the run aborts after 8 of 20 report steps, earlier still under refinement; at 1e-11 after 3 |
| Pore volume | read from the run's own `.INIT` | `PORV = 1.0000000E+04` m³, exactly `DX·DY·DZ·PORO` |
| Connection transmissibility factor | `opm-common`'s own `Connection::CF()` | `7.87747571538e-13` m³, with `rw = 0.00755`, `r0 = 19.79899`, `skin = 0` — all as expected |
| Relative permeability | `opm-common`'s own parsed `SGOF`, interpolated | as expected |

## Notes that may help

* The failure is a **threshold**, not a rate — it appears abruptly between 0.125 and 0.0625 days.
* It affects **rate-controlled wells in both directions** and leaves BHP-controlled wells alone,
  which points at the control equation or the primary-variable update rather than at the connection
  or the accumulation.
* `flowexp_comp` writes `FVPR`, `FVPT`, `WPI:PROD`, `FOIP`, `FGIP`, `FPR`, `OIL_DEN`, `GAS_DEN`,
  `OIL_VISC` and `GAS_VISC` as identically zero, which made this harder to localise than it needed
  to be. Populating the well PI and a reservoir-volume rate would have shortened it considerably.
* Separately: `flowexp_comp` cannot follow a cell through its own saturation pressure on a
  rate-controlled depletion — it aborts one report step after gas appears with
  `Rachford-Rice did not converge`, on `ssi`, `newton` and `ssi-newton` alike. Possibly the same
  underlying fragility; possibly not. Happy to open that separately if it is more useful.

---

## For the maintainer, before posting

* Both diagnostic patches were local to `../ressim-opm-build` and are **reverted**; the binary was
  rebuilt and all eight committed fixtures plus the convergence census reproduce
  (`bash scripts/validate-compositional.sh reference`).
* The full deck is `opm/compositional/injection/INJECTION.DATA` in this repository, and the
  producer variant is `opm/compositional/depletion/DEPLETION.DATA`. Both carry the ODbL notice from
  the `opm-tests` deck they derive from, so they can be attached to an upstream issue as they are.
* Reproduce the ladder with `python3 tools/opm_compositional/halve_tstep.py <deck> --out <halved>`,
  or the whole census with `bash tools/opm_compositional/check-reference-convergence.sh`.
* The `CF` and `SGOF` numbers come from `bash tools/opm_compositional/deck-probe.sh`.
