# Single-cell mixture injection reference case

The only fixture in C12 that exercises a **surface-rate control on a mixture**. Everywhere else a
rate control appears, the stream is pure CO₂ (the 1D deck's injector, for one report interval) or
the reference does not converge.

| | |
| --- | --- |
| Deck | `INJECTION.DATA`, derived from the depletion pair — the PROPS section is `1D_COMP.DATA`'s verbatim, © 2024 SINTEF Digital / TNO, Open Database License |
| Simulator | `flowexp_comp`, the same build as the other cases |
| Regenerate | `bash tools/opm_compositional/run-injection.sh` |
| Verify unchanged | `bash tools/opm_compositional/run-injection.sh --check` |
| Consumed by | `comp_injection_*` in `src/lib/ressim/src/compositional/reference_tests.rs` |

## The case

One cell, 100 m × 100 m × 10 m, 10 000 m³ pore volume, initially single-phase liquid at 150 bar
and 150 °C with `z = [0.1, 0.3, 0.6]`. One injector on `WCONINJE ... RATE 2000` delivering
**0.5 CO₂ / 0.3 methane / 0.2 decane** — a stream that genuinely splits at the deck's `STCOND`, so
how many moles "2000 sm³/day" means depends on the surface flash. The 400 bar limit is never
approached (BHP runs 152 → 184), so the well stays on rate control throughout.

**The cell is closed apart from the well**, which is what makes the reference's own answer
readable: the change in what it holds *is* the injected amount, in moles, with no surface
conversion anywhere in the inference.

## What it establishes

```text
2000 sm3/day of 0.5 CO2 / 0.3 C1 / 0.2 C10 is:
  105 025.5 mol/day  by ResSim's surface separation
  105 024.7 mol/day  by OPM's own PTFlash at STCOND    <- 8e-6 from ResSim
   76 583   mol/day  by flowexp_comp's own trajectory  <- 0.73x its own flash
```

**ResSim's surface conversion for a mixture is validated to 8e-6** against OPM's own flash — the
tightest such agreement in C12, and the gap it was built to close.

## What it opens, and what it generalises

`flowexp_comp`'s own trajectory injects 27% fewer moles than its own flash says 2000 sm³/day is,
and refining its `TSTEP` moves it *toward* ResSim without settling:

```text
final pressure   183.60 (1 d)  184.16 (0.5 d)  185.16 (0.25 d)  185.20 (0.125 d)  191.94 (0.0625 d)
```

This **generalises the ORAT depletion finding from producers to injectors**. Both fixtures whose
reference fails the convergence census are rate-controlled; neither BHP-controlled one does. The
pattern is clean enough to state: in this build of `flowexp_comp`, a **rate-controlled** well does
not converge in the timestep, and a **BHP-controlled** one does.

So no rate-controlled fixture here can referee a rate at any step size. ResSim's *conversion* is
validated against OPM's flash; ResSim's rate *control* end to end on a mixture is not validated
against a simulator, and that residual is recorded rather than papered over. It is narrow: what
remains untested is the algebra that turns a validated conversion into a BHP, and that is
unit-tested.

See [`docs/COMPOSITIONAL_C12_FORENSICS.md`](../../../docs/COMPOSITIONAL_C12_FORENSICS.md).
