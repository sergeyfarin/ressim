# C12 forensics: the producer connection-rate finding, and why it was not one

**Verdict: there is no connection-rate defect.** ResSim and `flowexp_comp` are the same model. At
matched temporal resolution they agree to **0.004 bar** on a single-cell depletion and to
**0.0074 bar / 0.001%** on the 1D displacement's skin variant. The 1.29× reported in
[`COMPOSITIONAL_VALIDATION.md`](COMPOSITIONAL_VALIDATION.md)'s C12 record before 2026-09-18 was an
artifact of two methodological errors in the comparison, both mine, both described below.

The second finding recorded at the same time — a 3.8% surface-metering inconsistency — is also not
a fixed offset. The reference's rate-controlled withdrawal is **timestep-divergent**, and 3.8% is
simply its value at the one timestep the committed fixture happens to use.

This document exists so the ruled-out list is not re-tested. Everything below was measured; nothing
is asserted from reading code alone unless it says so.

## 1. The two methodological errors

### M1 — comparing an instantaneous rate against an implicit step

The finding was built on: *the reference's own molar withdrawal between two of its report steps
follows from its own two states, since the cell holds `PV · c_mix(p, z)`; ask ResSim's connection
law for its rate over the same interval.* The first half is sound. The second was not.

`flowexp_comp` takes **one backward-Euler step per report interval** on every fixture here (§3,
E-TS). A backward-Euler step evaluates its rate at the **end-of-step** state, so over an interval
where the rate decays by a factor of ~0.6 the step's effective rate is the *lowest* rate in the
interval, not the average and not the midpoint value.

I compared it against ResSim's rate at the interval's **midpoint**. For a decay constant of
`k ≈ 0.48/day` that inflates the ratio by `e^{k/2} ≈ 1.27` — essentially the entire 1.29 that was
reported.

**Evaluated where the scheme evaluates it, the ratio is 1.0000–1.0014** (§3, E1).

### M2 — verifying one side's convergence and assuming the other's

C12 established that ResSim's answers are timestep-converged, and then compared them against a
reference whose convergence was never checked. The reference takes one 1-day step over a transient
in which the production rate falls by an order of magnitude; it is nowhere near converged.

That is not a model difference. It is two different points on the same convergence curve, and
**the finer answer disagreeing more with the reference is the expected outcome**, not a defect.

Both simulators converge to the same trajectory when both are refined (§3, E3).

### M3 — a ruled-out that was reasoned wrongly (conclusion right, argument not)

An earlier revision recorded *"the pore volume — `PV` cancels between consecutive steps and the
implied withdrawal is constant"*. `PV` cancels in the **ratio** of consecutive withdrawals; the
inferred withdrawal is directly **proportional** to it. Both findings shared that assumption, so a
wrong `PV` would have moved both together — exactly the compensating-error case.

`PV` turned out to be exactly right, but that was luck, not argument. It is now read from OPM's own
`.INIT` (§2).

## 2. Terms of the rate law, and how each was settled

`q = WI · Σ_P (kr_P/μ_P) · Δp · c_P`. Every term is now checked against **the reference's own code
or output**, not against a re-derivation.

| Term | How it was settled | Result | Re-test? |
| --- | --- | --- | --- |
| `PV` | OPM's own `.INIT` file, `PORV` keyword | `1.0000000E+04` m³, exactly `DX·DY·DZ·PORO` | No |
| `WI` | opm-common's own `Connection::CF()`, via a probe linking `libopmcommon` | `7.87747571538e-13` SI → **6.80614** in ResSim's units against ResSim's 6.806116 — **3e-6** | No |
| `r_w` | same probe | `0.00755` m — independently confirms `COMPDAT` item 9 is a **diameter** | No |
| `r_0` | same probe | `19.7989898732` m, identical to ResSim's Peaceman `r_eq` | No |
| skin | same probe | `0`, as the deck defaults it | No |
| `kr` | opm-common's own parsed `SGOF` table, interpolated | `kro = 0.661285`, `krg = 0.035485` at `Sg = 0.1871` — identical to ResSim's | No |
| `Δp` | the reference's own `WBHP:PROD`; `SGOF`'s `Pcgo` column is all zero | `80.0000` at every step — exact | No |
| `S_g` | the reference's own `SGAS` vs ResSim's flash at the reference's own `(p, z)` | `< 1e-4` | No |
| `μ_L`, `μ_V` | OPM's own PTFlash + LBC (`ternary_bhpdep_*` in the C0 fixture) | `< 2%`; `0.79%` on `μ_L`, and in the direction that *reduces* ResSim's rate | No |
| `c_L`, `c_V`, `c_mix` | OPM's own fixture values | `2e-5` on `c_mix`; **`0.02%` on `Δc_mix`**, which is what the inference rests on | No |
| phase split | produced-stream composition by material balance on the reference's own states | `< 1%` | No |

**None of these is the discrepancy, and none needs re-testing.** The `WI`, `r_w`, `r_0`, skin and
`kr` rows supersede the earlier Peaceman re-derivation: they come from opm-common itself.

## 3. Experiment register

Each row is an experiment that was actually run. The last column is the point of the table — what
the result does **not** establish.

| ID | Question | Result | Does **not** rule out |
| --- | --- | --- | --- |
| **E-TS** | How many timesteps does the reference take? | **One per report interval** on every fixture: BHP depletion 20/20, 1D skin 28/28, 1D plain 31/28, ORAT 19 (aborts at 7) | — |
| **E1** | Does ResSim's rate match if evaluated where backward Euler evaluates it? | At the **end-of-step** state: **1.0000, 1.0000, 0.9999, 1.0001, 1.0001, 1.0001** across steps 2–8 (at the midpoint: 1.33–1.29) | That the two *converged* solutions agree — see E3 |
| **E2** | Does ResSim at the reference's timestep reproduce the reference? | BHP depletion `p(1)` **99.2391 vs 99.2390**. 1D skin: worst **0.0074 bar** over the trajectory, cumulative injection **0.001%**. 1D plain: worst 1.681 bar, cumulative 0.042% — the residual is the deck's sub-day `TSTEP`s, where OPM cut 3 extra steps | That either is *correct*; matched discretisation tests model equivalence, not accuracy |
| **E3** | Does the reference converge toward ResSim when forced to sub-step? | BHP depletion `p(1)`: **99.2390** (1 d) → **95.3159** (0.05 d) → **95.0018** (0.01 d). ResSim: 95.3200 (0.05 d) → 95.0278 (0.0125 d) → 94.9493 (0.003125 d). Same limit, ~94.95 | — |
| **E4** | Is the reference's **rate-controlled** withdrawal timestep-independent? | **No, and it does not converge.** Implied withdrawal: **227 888** (1 d) → **231 373** (0.05 d) → **244 887** (0.01 d) → **269 638** (0.0025 d) mol/day. ResSim: **236 926**, timestep-independent | The mechanism. The signature — growing without bound as `dt → 0` — is that of a term divided by `dt`, and `CompWell::assembleSourceTerm` has one: `(new_component_masses − component_masses)/dt` over a hard-coded 0.0216 m³ wellbore. Not proven |
| **E5** | Can the reference's own summary give `PV`, `PI` or a reservoir-volume rate? | **No.** `FVPR`, `FVPT`, `WPI:PROD`, `FOIP`, `FGIP`, `FPR` are all written as identically zero | — |

## 4. What this leaves standing

The other C12 findings are **unaffected**, because none of them rests on a trajectory comparison:

* **Wells had to become implicit.** A source held fixed over a step has no pressure feedback; the
  observed 238 bar against a 150 bar injector is a property of the scheme, not of a resolution
  mismatch.
* **`COMPDAT` item 9 is a diameter.** Now confirmed directly by opm-common's `Connection::rw()`.
* **The injector takes the perforation cell's total mobility.** This is a **code-reading** fact
  about `CompWell::calculateSingleConnectionRate`'s injecting branch, not an inference from a
  measurement. The 27% figure quoted alongside it was measured across mismatched timesteps and
  should not be relied on; the law change stands on the source.
* **The well-opening tolerance** had to be sized to the reference's single-precision `TIME`.
* **The surface flash** agrees with OPM's own PTFlash: `1.4e-8` on the gas/oil ratio and `0.15%` on
  the absolute volume per mole.

## 5. What is now open

1. **The reference's rate control does not converge** (E4). ResSim's value is the one consistent
   with OPM's *own* flash, so the ORAT fixture cannot referee a withdrawal at any timestep. This
   supersedes the "3.8% surface metering" finding, which was that divergence sampled at one step
   size.
2. **`flowexp_comp` cannot cross a saturation pressure** on the ORAT deck — unchanged, and now
   joined by the observation that its ORAT run also fails at fine timesteps (`orat400` aborted at
   2163 of 2400 steps).

Neither is ResSim's to fix. Neither blocks a model-equivalence claim, because E2 and E3 establish
that independently of the rate control.

## 5b. The guard: a convergence census

Rules are only worth what enforces them, so the distance of each reference from
timestep-convergence is now **measured and gated** rather than reasoned about.

`bash tools/opm_compositional/check-reference-convergence.sh` re-runs each fixture's deck with its
`TSTEP` ladder halved, and halved again, and records how far the reference's own trajectory moves.
`opm/compositional/reference_convergence.json` holds the result; the `reference` gate verifies it.

```text
fixture           halved      quartered   growth   trend
1d_comp           2.8212 bar  4.1041 bar    1.46   converging
1d_comp_skin      1.5422 bar  2.3732 bar    1.54   converging
depletion_bhp     1.6030 bar  2.6887 bar    1.68   converging
depletion_orat    0.0107 bar  0.1236 bar   11.61   diverging
```

**Two refinements, not one.** A single halving moves the ORAT depletion by 0.0107 bar — reassuring,
and wrong. What separates a converging reference from a diverging one is whether successive
movements shrink, and that needs two.

The census also carries the movement at the **final** report step, because settled-state bands are
compared against that rather than against the trajectory worst. It is what shows that the skin
variant's 2.09 bar settled-state "disagreement" sits inside its reference's own 0.998 bar temporal
uncertainty, and is therefore not a disagreement.

Three tests enforce it (`comp_oracle_*`):

* the census is what it was, so an upstream rebuild that changes a reference's behaviour fails
  rather than silently shifting a number;
* **no acceptance band that compares ResSim's converged answer against a reference may sit below
  that reference's own recorded movement.** The band table is written out in the test, so the rule
  is auditable rather than assumed. Matched-resolution tests are deliberately exempt — they do not
  inherit the gap, which is why they exist;
* the skin variant's settled-state gap is explicitly recorded as being within the reference's
  uncertainty, because it reads like a result otherwise.

## 6. Rules these errors imply

1. **Never compare a converged solution against an unconverged one.** Either match the
   discretisation on both sides, or refine both. Matching the reference's timestep *removes* its
   controller from the comparison; it does not "compare two timestep controllers", which was the
   reasoning that kept C12 from doing it.
2. **Evaluate a rate where the scheme evaluates it.** Comparing an instantaneous rate against a
   `Δinventory/Δt` from an implicit step compares two different quantities.
3. **Check the reference's own convergence before concluding the model differs.** It is cheap —
   re-run it with a subdivided `TSTEP` ladder — and it was the whole answer here. This is now
   automated: §5b. Refine **twice**; once cannot distinguish converging from diverging.
4. **When an inference rests on a difference of nearly equal quantities, test the difference.**
   `Δc_mix` is 0.4% of `c_mix`, so 0.1% agreement on `c` permits 25% on `Δc`.
5. **A term-by-term agreement plus a product disagreement means the comparison is wrong**, not that
   the terms are subtly wrong. That is the point at which this investigation should have started.

## 7. Reproducing this

```bash
# E-TS: the reference's timestep count
grep -c 'Time step .* done' <run>.log

# E1, E2: matched-resolution agreement
cargo test --manifest-path src/lib/ressim/Cargo.toml --lib -- comp_depletion_bhp_ --nocapture

# E3, E4: force the reference to sub-step, by subdividing the deck's TSTEP ladder
python3 tools/opm_compositional/halve_tstep.py <deck> --out <halved>

# The census that now gates all of this
bash tools/opm_compositional/check-reference-convergence.sh          # remeasure
bash tools/opm_compositional/check-reference-convergence.sh --check  # verify (in the gate)

# The opm-common probe that settled the WI, r_w, r_0, skin and kr rows of §2
bash tools/opm_compositional/deck-probe.sh \
    opm/compositional/depletion/bhp/DEPLETION.DATA 0.1871 0.22794

# PV, straight from the reference's own INIT file
convertECL <case>.INIT && grep -A1 "'PORV" <case>.FINIT
```

Verbatim output of the probe on the BHP depletion deck, which is §2's `WI`, `r_w`, `r_0`, skin and
`kr` rows:

```text
grid 1x1x1
well PROD  ref depth 5 m
  (1,1,1)  CF = 7.87747571538e-13 m3  -> well index 6.80613901809 m3.cP/(day.bar)
      Kh = 9.86923266716e-13  rw = 0.00755  r0 = 19.7989898732  skin = 0  depth = 5  dir = Z
SGOF: 21 rows, Sg from 0 to 1
  at Sg = 0.1871:  krg = 0.035485  kro = 0.661285
  at Sg = 0.22794:  krg = 0.052573  kro = 0.596693
```

ResSim's well index for the same connection is **6.806116**; the probe says **6.806139**.
