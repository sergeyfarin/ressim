# OPM PTFlash oracle for compositional work

Generates the independent thermodynamic reference fixture that
[`docs/COMPOSITIONAL_VALIDATION.md`](../../docs/COMPOSITIONAL_VALIDATION.md) admits C2–C5 against.
This is task **C0** of
[`docs/COMPOSITIONAL_FLUID_EXECUTION_PLAN_2026-09-14.md`](../../docs/COMPOSITIONAL_FLUID_EXECUTION_PLAN_2026-09-14.md).

This is not the same thing as `tools/opm_flow/`. That tool drives the black-oil **Flow simulator**
to produce trajectory references. This one calls OPM's **thermodynamics** directly and produces
scalar flash states. The compositional trajectory oracle the plan's C12 needs does not exist on
this machine and is not provided here — see *What this is not*.

## Regenerate

```bash
bash tools/opm_compositional/generate.sh
```

To verify the committed fixture still reproduces byte-for-byte:

```bash
bash tools/opm_compositional/generate.sh --check
```

Both build the harness into a temporary directory; nothing is installed and no OPM source is
built. `--check` is the form to run in a gate.

## Why this compiles without building OPM

OPM's `opm/material/**` is header-only template code. The installed `-dev` packages carry all of
it, so the harness is an ordinary single translation unit. Only Dune's exception vtable and
libfmt are linked.

```bash
g++ -std=c++20 -O2 -I/usr/include -I/usr/include/dune \
    tools/opm_compositional/ptflash_harness.cpp -o harness -ldunecommon -lfmt
```

`-std=c++20` is **required**, not a preference. `opm/material/Constants.hpp` and
`PolynomialUtils.hpp` use `std::numbers`, and `dune/common/std/algorithm.hh` `#error`s out without
three-way comparison support. C++17 fails to compile.

`-DNDEBUG` is deliberately **not** set. OPM's EOS code asserts on non-finite and non-positive
molar volumes, and an oracle that quietly emits a nonsense reference value is worse than one that
aborts.

## Provenance

| Package | Version | Supplies |
| --- | --- | --- |
| `libopm-common-dev:amd64` | `2026.04-1~noble` | `eos/`, `constraintsolvers/PTFlash.hpp`, `fluidsystems/`, `viscositymodels/LBC.hpp`, `components/` |
| `libopm-simulators-dev:amd64` | `2026.04-1~noble` | `opm/models/ptflash/` (read, not compiled in) |
| `libdune-common-dev:amd64` | `2.11.0-1~noble` | `FieldVector`, `FieldMatrix`, exceptions |

Confirm any header's owner with `dpkg -S /usr/include/opm/<path>`. There are **no** OPM git
checkouts on this machine; the three commit hashes in
[`COMPOSITIONAL_READINESS_ASSESSMENT_2026-09-14.md`](../../docs/COMPOSITIONAL_READINESS_ASSESSMENT_2026-09-14.md)
are upstream citations, not local artifacts.

OPM is GPL-2-or-later. `ptflash_harness.cpp` and `two_component_fluid_system.hh` are ResSim's own
code that includes those headers; the OPM copyright notices remain in the headers themselves. No
OPM source file is vendored into this repository.

## The two fluids

**Ternary — `Opm::ThreeComponentFluidSystem`, used unmodified.** CO2 / C1 / C10, from
`/usr/include/opm/material/fluidsystems/ThreeComponentFluidSystem.hh` (SINTEF, 2022). It supplies
critical temperature, critical pressure, critical volume, acentric factor and molar mass by
delegating to `components/{SimpleCO2,C1,C10}.hpp`, and returns `0.0` from
`interactionCoefficient()` for every pair. The all-zero interaction matrix is that source's
explicit modelling choice, which is what makes it citable; zero BIPs are never a default.

**Binary — `ressim::TwoComponentFluidSystem`.** C1 / C10 only. OPM ships no two-component system,
and the obvious shortcut — run the ternary with `z_CO2 = 0` — does not work (see below). This
class declares OPM's own `C1` and `C10` component classes as a system and delegates every
property to them, reusing OPM's EOS, parameter cache and viscosity model unchanged. It introduces
no fluid data.

Because the ternary's trace-CO2 state and the binary's matching state must agree, the fixture has
a built-in cross-check: `ternary_trace_co2` (z = [1e-6, 0.6, 0.399999]) returns L = 0.730698 and
`binary_p150` (z = [0.6, 0.4]) returns L = 0.730699.

## Oracle domain limits

Measured, not assumed. These are properties of OPM's PTFlash, and the Rust implementation is
**not** required to share them — but where ResSim supports a state OPM cannot resolve, that state
has no external oracle and needs an independent invariant instead.

1. **An exactly zero component is unsupported.** `checkStability_` forms `z_i/K_i` on its liquid
   branch and `K_i*z_i` on its vapour branch, so a zero mole fraction makes Michelsen's test
   degenerate and every such state throws `Stability test did not converge`. OPM's own source
   carries the matching TODO ("make sure that no mole fraction is smaller than 1e-8?"). This is
   why the binary exists as its own system and why the composition sweeps stop at 0.001 rather
   than 0. **Consequence for C1/C3:** the active-component policy for true zeros cannot be
   validated against this oracle. Pure-component states likewise have no external reference here;
   for those, a single-component PR root is its own analytic oracle, since `A` and `B` reduce to
   closed forms in reduced temperature and pressure.
2. **`K` must be seeded by the caller.** PTFlash never applies its own `wilsonK_`; passing `K = 0`
   fails every state. The harness seeds Wilson explicitly, reproducing `wilsonK_` from the same
   header. A "cold start" in this fixture therefore means Wilson, not zero.
3. **The flash tolerance floor is about 1e-9.** At `--tolerance 1e-11` or tighter, 27 of 47 states
   fail with `Newton composition update did not converge`; at `1e-9`, all resolve. The pinned
   value is `1e-9`. This is a floor on the *reference*, and it is what caps the achievable
   equilibrium residual recorded in `COMPOSITIONAL_VALIDATION.md`.
4. **Some states need the SSI-only path.** The default method chain is `ssi+newton`, falling back
   to `ssi`. Six states resolve only on the fallback. The method that actually succeeded is
   recorded per state in `method_used`, so no fixture value is anonymous about how it was reached.
5. **`Z` must be written to the fluid state before asking for viscosity.** LBC reads
   `compressFactor` and nothing in PTFlash sets it; without that write every viscosity is `NaN`.
6. **OPM clamps fugacity coefficients** into `[1e-10, 1e10]` in `CubicEOS.hpp`. No fixture state
   reaches that clamp. The Rust port must not copy it — plan rule 6 forbids clamping an invalid
   result — and if a future fixture state ever did clamp, it would have to be excluded rather
   than matched.

## What this is not

It is **not** the compositional trajectory oracle. `libopm-simulators-bin` installs `/usr/bin/flow`
and nothing else, and `flow 2026.04` is the black-oil simulator; `flow_comp` lives in
`opm-simulators/flowexperimental/`, which Debian does not package. C12 stays **BLOCKED** until
someone builds that application. A scalar flash oracle cannot validate a flow trajectory, and this
tool must never be cited as though it had.

It is also not a validation of ResSim. It produces reference numbers. The tests that consume them
live in the Rust crate and are listed in `COMPOSITIONAL_VALIDATION.md`.

## Two kinds of state

`states` are **flashed**: PTFlash decides the phase state and the harness reports the equilibrium.

`eos_states` are **not flashed**. The harness evaluates OPM's cubic EOS directly at a given
`(p, T, x)` and reports both extreme roots, with no stability test and therefore no claim about
which phase is present. They were added after the first fixture revealed that **none of the 47
flashed states has three real roots** — at 150 °C every one of them is monotonic, so a port could
get the root-labelling rule completely wrong and still reproduce the entire fixture. The plan's C2
requires "at least one state where two algebraic roots must not be treated as two coexisting
stable phases"; these supply it, and they also cover pure components, which PTFlash cannot flash
at all but the EOS evaluates without difficulty.

Both roots come out of OPM unchanged: `PTFlashParameterCache::updateMolarVolume_` calls
`computeMolarVolume` with `isGasPhase=true` for the gas index and `false` for the oil index,
selecting the largest and smallest root of the same cubic. Setting both phase compositions to the
same `x` reads both off one composition. When the cubic has a single real root the two are
identical — itself a contract worth testing.

Seven of the 21 flash-free states are genuinely multi-root: pure n-decane below its vapour
pressure at 423 K (`T_r = 0.685`), and C10-rich binary and ternary mixtures in the same region.
Pure methane (`T_c = 190.6 K`) is supercritical there and gives one root at every pressure, which
is the contrast that makes the test meaningful.

## Output

One JSON object with two `systems` entries (binary first, then ternary), each carrying its
component table, interaction matrix, a list of flash-free `eos_states`, and a list of flashed
`states`. Units are SI throughout —
Pa, K, mol, m³, kg, Pa·s — and stated in the `units` field. Critical volume is m³/kmol, matching
how `LBC.hpp` consumes `criticalVolume()` (it divides by 1000).

Two conventions the plan specifically warns about are resolved in the data itself:

- `L_liquid` is OPM's **liquid** mole fraction. `beta_vapour` is `1 - L`, the vapour fraction this
  plan uses. Both are written out so nothing downstream has to infer which is which.
- Derivatives are `d/du` for `u = [p_Pa, z_0 .. z_(N-2)]`, ResSim's own primary variables. The
  dependent `z_(N-1)` is seeded with `dz_(N-1)/dz_k = -1`, so the invariant C4 must verify is
  enforced by the oracle rather than assumed by its consumer.
