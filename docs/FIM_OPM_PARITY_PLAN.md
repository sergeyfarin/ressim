# FIM Bundle Y: OPM Convergence Parity (post-Bundle-X roadmap)

Status: **Y0-Y2a complete through 2026-07-13.** The exact Flow oracle, linear-stack refutation,
injector isolation, and active-bound Jacobian audit are recorded in §§11-14. The original Y1-Y4
checkpoint order below is historical where later evidence supersedes it. The current decision
frontier is Y2b bound/update-policy parity, owned by
`docs/FIM_OPM_CONVERGENCE_EXECUTION_PLAN.md`; follow that document before starting another
solver change.
Prerequisite evidence: `docs/FIM_DIAG_003_PLAN.md` (closed) and `docs/FIM_BUNDLE_X_PLAN.md`
(closed, PROMOTED unconditional). Baselines below re-derived on the committed tree at
`53cae5c` (2026-07-12) — the exact runs are in the worklog note accompanying this plan's commit.

## 1. Where the week's arc landed (review)

The `FIM-DIAG-002` → Bundle W → `FIM-DIAG-003` → Bundle X chain is closed. Its net yield:
the 18k-substep heavy-case catastrophe was a **conjunction** of two well-side defects — a
perforation-rate standoff (fixed by W's nested well solve, kept inert) masking a
producer-fraction fidelity bug (a 3x3-neighborhood mobility window OPM does not have, fixed
unconditionally by X, in two independent duplicate implementations). Along the way:
`FIM-NEWTON-008` (min-iterations off-by-one) promoted; H2 (linear-precision floor) and H3
(MB formula fidelity) refuted with source-cited evidence; the OPM INFOITER differential
trajectory established as a standing method with a tracked deck
(`opm/reference-decks/water-heavy-step1/`).

**Post-X4 baselines (commit `53cae5c`, wasm runner, verbatim substep counts):**

| Case | Legacy (default) | `OpmAligned` (+`nested_well_solve` on heavy) | OPM Flow |
|---|---|---|---|
| heavy `12x12x3 --dt 1` | `25` | `17` (X1 measured `16`; ±1 is this case's known chaos band) | **1 substep, 11 iters, 14 linear iters** |
| `20x20x3 --dt 0.25` | `8` | `16` | — |
| `22x22x1 --dt 0.25` | `4` | `24` (min dt `2.8e-5`) | — |
| `23x23x1 --dt 0.25` | `4` | `12` | — |
| `gas-rate 20x20x3 --dt 0.25` | `2` | **`459`** (`337` linear-bad retries, dt pinned `~1e-4`, `retry_dom=linear-bad:oil@1261`) | — |

The heavy case is no longer pathological on either flavor — for the first time `OpmAligned`
*beats* Legacy on it. But parity with OPM is not "17 substeps"; it is **1 substep at
`dt=1.0` in ~11 Newton iterations with zero cuts**. That residual gap, and the bounded-case
overhead (worst: gas-rate `2 → 459`), is what this plan addresses.

## 2. The remaining gap, decomposed

From the X3 fixed-run `LEDGER` (16-substep run): substeps 0-9 burn ~150 of the 168 total
Newton iterations in an `iters=20` → `growth=0.4` → dt-collapse → recover cycle during the
early transient; substeps 10-15 are healthy (4-12 iters, growing dt). OPM's INFOITER on the
same interval: CNV starts at `~292`, MB at `~0.67`, and converges in 11 iterations flat —
through the same transient, at 12x the dt. The gap therefore decomposes:

- **G1 — Newton robustness at large dt through the transient.** ResSim exhausts a 20-iteration
  budget at `dt=0.0825` where OPM converges `dt=1.0` in 11. Not yet diagnosed — candidates:
  linear-solve quality throttling the Newton direction (see G2), chop/relaxation binding every
  iteration (linear convergence rate instead of quadratic), or the remaining well-structure gap
  (G4). This is measurable, not guessable: the same per-iteration CNV/MB trace vs INFOITER
  differential that closed FIM-DIAG-003, pointed at substep 0 instead of the tail.
- **G2 — Linear stack quality.** The most under-exploited evidence in the registry:
  `FIM-LINEAR-005` measured the current CPR pressure restriction (`row0-schur`) converging
  **0/54** heavy-case systems as full solves offline, while `sum-rows` and `quasi-impes`
  converge ~92% on both corpora — and it has sat OPEN since, with the promotion path already
  written ("Step 9.3"). The gas-rate 459 catastrophe is `linear-bad`-dominated (337 retries),
  the strongest live hint that the linear stack is now the binding constraint under
  `OpmAligned`'s stricter acceptance. OPM uses quasi-IMPES-style CPR weighting (`cprw`).
- **G3 — Timestep controller policy.** Even with a perfect Newton, ResSim cannot reach
  1-substep parity: the heavy run *starts* at trial `dt≈0.0825` and is growth-rate-limited
  (max growth 3.0/`opm-max-growth`, `opm-iter` shrink at budget exhaustion) — covering 1 day
  needs ≥5 substeps by policy alone. OPM takes the report step's full `dt` first and cuts only
  on failure. Controller parity = attempt the full target dt once Newton is robust enough to
  hold it (G1/G2 first — otherwise this just converts substeps into retries).
- **G4 — Well primary-variable structure.** The remaining named architecture divergence:
  ResSim carries per-perforation `q` unknowns with `rate_consistency` rows; OPM's StandardWell
  carries per-well `WQTotal`/fractions/`bhp` with connection rates *derived* at assembly.
  Bundle X's fallback item, never needed for the heavy case — becomes relevant again only if
  Y1 attributes G1's transient stall to the well rows.
- **G5 — Three-phase specifics.** The gas-rate 459 case may add gaps beyond G2 (variable
  substitution / regime switching inside Newton — `docs/FIM_OPM_GAP_ANALYSIS_SPE1.md` gap #5,
  deliberately deferred since Bundle N). Y0 classifies before anything is built.
- **G6 — Per-iteration wall-clock** (preconditioner rebuild dominance, Task #41's 24x factor,
  partially addressed by `FIM-LINEAR-011`). A cost axis, not a convergence axis — explicitly
  out of scope for this plan except where a G2 change moves it incidentally.

## 3. Checkpoints

### Y0 — transient + gas-rate differential diagnostics (~half day, capped-run economics)

1. **Heavy substep 0 vs OPM INFOITER**: windowed per-iteration `CNV-MB`/`WELLJAC` trace on
   substeps 0-2 of the fixed heavy run (`FIM_TRACE_SUBSTEP_START=0`, tiny cap), diffed
   iteration-by-iteration against the D3 deck's `CASE.INFOITER`. Questions: is ResSim's
   per-iteration CNV/MB reduction rate linear where OPM's is quadratic (⇒ chop/damping or
   linear-quality bound)? Which criterion/cell binds at iters 10-20? Does the `PERCELL-CHOP`
   trace show the chop firing every iteration (OPM's chop fires early then releases)?
2. **Gas-rate 459 windowed trace**: one capped `--diagnostic step` run at the dt-collapse
   window. Classify the `linear-bad:oil@1261` failures: CPR restriction quality (⇒ G2, feeds
   Y1), three-phase Jacobian/regime issue (⇒ G5, own bundle), or controller compounding.
3. Verdict: attribute G1 and the gas-rate catastrophe to G2/G4/G5 with evidence. No fixes here.

**DONE — see §6 for the full writeup.** Short version: (1) is a genuine raw-Newton-step
oscillation at the injector-connected cell, not a chop/damping fidelity bug (the damping port
was verified byte-for-byte against pinned OPM source and is faithful); root cause is upstream,
not yet localized to G2 vs G4. (2) is confirmed G2 (linear stack) but via a different mechanism
than guessed: `retry_class=linear-bad` fires on iterations whose nonlinear state is *already*
converged (CNV-MB `would_accept=strict`), because the linear solve's own internal
converged-flag is stricter than what the Newton step actually needed — not a slow/bad search
direction. §6.4 flags a plausible (unconfirmed) unifying hypothesis linking both to the same
under-resourced CPR/GMRES budget.

### Y1 — CPR pressure-restriction promotion (`FIM-LINEAR-005` → live, "Step 9.3")

~~The already-evidenced lever, executed per its own registry retry condition: promote
`sum-rows` or `quasi-impes` (pick by OPM fidelity — OPM's `cprw` uses quasi-IMPES weighting;
`docs/FIM_CPR_IMPROVEMENT_PLAN.md` has the design skeleton) to the live CPR path, flag-gated
through the standard offline-lab-first workflow (the 54+13-system corpora already exist;
re-capture on the post-X4 tree first since trajectories changed). Gates: offline lab
non-regression, then full control matrix + locked smoke + BL benchmarks. Success metric:
gas-rate `OpmAligned` 459 and `22x22x1`'s `linear-bad` retries move materially; heavy
iterations-per-substep drop toward single digits in the transient.~~

**MOOT — this checkpoint's literal action was already executed weeks before this plan was
written.** `quasi-impes` has been the live restriction since commit `77ec900e` (2026-07-05);
the loosened tolerance/budget + block-ILU0 smoother (`FIM-LINEAR-008`) and well-Schur
elimination (`FIM-LINEAR-010`) are also both live defaults. The registry row this checkpoint
cites as its authorization was simply never updated after Step 9.3 landed. See §7 for what Y1
actually found once this was discovered, and for what real next step (if any) replaces it.

### Y2 — evidence-directed structural bundle (shaped by Y0)

Exactly one of, chosen by Y0's attribution — not both speculatively:
- **(a) Well primary-variable restructure (G4)**: per-well `WQTotal`/fractions/`bhp` unknowns,
  connection rates derived at assembly, `rate_consistency` rows deleted. Large
  (`wells_ad.rs`/`assembly_ad.rs` well blocks + W1-style agreement tests); only if Y0 shows
  the transient stall living in the well rows.
- **(b) Three-phase variable substitution (G5)**: OPM's regime-dependent primary-variable
  switching inside Newton; only if Y0 classifies the gas-rate case's failures as
  regime/flash-driven rather than linear-quality-driven.

### Y3 — controller parity (G3, only after Y1/Y2 hold large steps)

Attempt the full target dt as the first trial (OPM semantics), keep the existing retry ladder
as the failure path; re-tune `opm-iter`/PID targets against the now-robust Newton. Gate: heavy
case reaches the `1-3 substeps / ≤~35 total iterations` class without regressing any control
case; fine-dt FOPT physics check (the controller changes trajectory materially).

### Y4 — stack promotion decision (the standing open question)

With Y1-Y3 landed: re-run the original Bundle N §5 gate for `OpmAligned`(+`nested_well_solve`)
as the *default* flavor — heavy class + fine-dt FOPT + full control matrix + **bounded cases
not worse than Legacy** (the criterion that currently fails: `8→16`, `4→24`, `4→12`, `2→459`).
If it passes, flip the default and retire the Legacy compensating-mechanism stack per the
95%-track-OPM policy; if the bounded gap persists, record which G-item still owns it and
re-plan honestly.

## 4. Success criteria (parity, defined measurably)

- Heavy `12x12x3 --dt 1`: **≤3 substeps, ≤35 total Newton iterations**, zero dt cuts on the
  final configuration (OPM: 1/11/0).
- INFOITER-differential shape match on the D3 deck: ResSim's per-iteration MB/CNV trajectory
  shows the same quadratic tail OPM's does (the standing-method check, now cheap).
- Bounded cases under the final default ≤ their current Legacy counts (`8/4/4/2` + gas 6-step).
- No physics regression: fine-dt FOPT within the already-accepted band, BL benchmarks
  unchanged, parity gates 10/10.

## 5. Discipline notes

- Y0 before any Y1/Y2 code: the week's core lesson, twice confirmed (W's ordering hypothesis
  and X0's pivot), is that measured attribution beats plausible mechanism-guessing — every
  unmeasured "obvious" fix this month was wrong about the mechanism.
- One registry row per checkpoint; `FIM-LINEAR-005`'s existing row is Y1's home. Capped-run
  economics for everything except Y3/Y4 full gates.
- The worklog note accompanying this plan's commit records the exact post-X4 baseline
  commands; treat those as the replay set for Y0's before/after.

## 6. Y0 findings (2026-07-12)

Tree: `b2dd34a` (clean) plus one additive, diagnostic-only change not yet committed at the time
of this writeup — a native `repro_gas_rate_20x20x3_opm_aligned_no_trace` test
(`src/lib/ressim/src/fim/timestep.rs`, `mod phase5_repro`), added because the wasm diagnostic
runner cannot host `fim::trace_sink`'s env-gated file trace (every call site is `#[cfg(not(
target_arch = "wasm32"))]`) and no native gas-rate repro driver existed. It was validated
bit-identical to the wasm-recorded baseline before being trusted for tracing:
`accepted_substeps=459 linear_bad=337 nonlinear_bad=0 dt=[7.032e-5,9.539e-4]` — exact match to
the plan's §1 table. **This test should be committed alongside this doc update** so the traces
below are replayable; it changes no production or wasm-exposed path.

### 6.1 Heavy substep 0: not a slow-convergence plateau — a genuine raw-Newton oscillation

Command: `FIM_TRACE_FILE=<f> FIM_TRACE_SUBSTEP_START=0 FIM_NESTED_WELL_SOLVE=1 cargo test
--release --manifest-path src/lib/ressim/Cargo.toml --lib
repro_water_pressure_12x12x3_opm_aligned_no_trace -- --ignored --nocapture` (heavy case, `16`
substeps this run — within the case's recorded ±1 chaos band vs the plan's `17`).

The first trial (`dt=0.25`, a quarter of the report step) **never converges**: after 20
iterations, `cnv[water]` oscillates in the range `90–150` and `mb[water]` in `0.2–0.35` with no
monotonic trend, and the injector's connected cell (cell 0, `(0,0,0)`) has its water saturation
bounce `sw = 0.10 → 0.30 → 0.50 → 0.30 → 0.50 → 0.30 → 0.41 → 0.21 → 0.33 → 0.40 → 0.20 → 0.29 →
0.49 → 0.29 → 0.33 → 0.53 → 0.33 → 0.36 → 0.56 → 0.36 → (post-loop 0.38)` iteration-by-iteration
— a textbook 2-period oscillation, not slow decay. The substep is rejected
(`retry_class=nonlinear-bad dominant=water@0`); the retry at `dt=0.0825` is accepted but *still*
burns the full 20-iteration budget (`accept ... iters=20 ... res=3.04e-7 mb=1.02e-8`), meaning
the oscillation is damped-but-not-eliminated at the smaller dt, resolved only by hitting the
final-iteration relaxed acceptance tier. This directly explains the ledger's `iters=20 →
growth=0.4 (opm-iter) → dt-collapse` cycle the plan's §2 already named — the mechanism is now
observed, not inferred.

**Against OPM's oracle** (`/usr/bin/flow opm/reference-decks/water-heavy-step1/CASE.DATA
--output-extra-convergence-info=steps,iterations`, rerun fresh this session, reproducing the
recorded `11 Newton its / 12 linearizations / 14 linear its / 0 cuts` exactly): OPM's
`CASE.INFOITER` shows `MB_Oil` falling `0.674 → 0.370 → 0.267 → 0.260 → 0.209 → 0.0736 → 0.00741
→ 0.0033 → 8.8e-5 → 3.05e-6 → 6.95e-7 → 1.09e-9` — monotonic-ish with a genuine quadratic tail,
**for the whole 1.0-day step**, 4x ResSim's failing 0.25-day trial. No oscillation signature at
all in OPM's trajectory.

**Why this is not the already-diagnosed damping/chop mechanism** (verified by reading source,
not assumed): the applied saturation step is capped at exactly `OPM_DS_MAX = 0.2`
([newton.rs:2047](../src/lib/ressim/src/fim/newton.rs)) regardless of the OSC-DETECT relaxation
scalar (`relax` drops `1.00 → 0.90 → 0.80 → 0.70 → 0.60 → 0.50` from iter 7 onward as oscillation
is detected) — `raw_dsw` and `step_dS` stay pinned at `±0.2` through iters 11, 14, 15, 17, 18
*despite* `relax ≤ 0.6` at all of them. Read literally this looks like the relaxation is being
masked by the chop. **Checked against OPM's actual pinned source before concluding that**:
- `opm_per_cell_chopped_update` ([newton.rs:2081-2141](../src/lib/ressim/src/fim/newton.rs))
  composes `chopped = update * relaxation` **then** re-clamps any cell still over `dsMax` back
  to exactly `dsMax` — so a cell whose raw (pre-relaxation) delta exceeds `dsMax / relaxation`
  gets reset to the same `±0.2` ceiling every iteration no matter how far `relaxation` has
  decayed. This composition order is a straight port of OPM's own
  `updatePrimaryVariables_` — `blackoilnewtonmethod.hpp:265-268,286-289` computes `satAlpha =
  dsMax_ / maxSatDelta` and applies it to `delta` with **no** prior relaxation multiply inside
  this function.
- The relaxation multiply itself is applied **before** that chop in OPM too, one layer up:
  `NonlinearSystemBlackOilReservoir_impl.hpp:283-301` calls
  `nonlinear_solver.stabilizeNonlinearUpdate(x, dx_old_, current_relaxation_)` (which does
  `dx *= omega` — `NonlinearSolver.cpp:70-88`) immediately before `this->updateSolution(x)`
  (which triggers the dsMax chop). **Same order, same masking-when-raw-delta-is-large-enough
  property, in OPM's real code.**
- Constants match exactly: `relaxMax_=0.5, relaxIncrement_=0.1, relaxRelTol_=0.2`
  ([NonlinearSolver.cpp:167-169](../OPM/opm-simulators/opm/simulators/flow/NonlinearSolver.cpp))
  vs ResSim's `OSCILLATION_MAX_RELAX_FLOOR=0.5, OSCILLATION_RELAX_INCREMENT=0.1,
  OSCILLATION_RELAX_REL_TOL=0.2` ([newton.rs:1271-1274](../src/lib/ressim/src/fim/newton.rs)).
- One real ambiguity chased and closed: OPM also ships a *separate*, well-scoped relaxation
  (`relaxationFactorFraction`/`relaxationFactorFractionsProducer`,
  `StandardWellPrimaryVariables.cpp:62,738`) that damps producer `WFrac`/`GFrac` primary
  variables specifically. This is a **different mechanism** from the global
  `detectOscillations`/`stabilizeNonlinearUpdate` one ResSim's Phase 7 ported (confirmed by
  `grep`: `dampen`/`relaxationFactor`/`oscillat` appear only in `opm/simulators/wells/*`, never
  in `opm/models/nonlinear/*`) — it does **not** invalidate the composition-order check above,
  but is worth knowing about since a future well-structure bundle (G4) will touch that file too.

**Conclusion**: ResSim's oscillation-detection-and-damping port is faithful to OPM's real
composition, order, and constants. The divergence is upstream of it — **OPM's raw (pre-damping)
Newton correction at the injector-connected cell apparently never reaches the magnitude that
needs `dsMax` clamping in the first place**, for the same physical case at 4x the dt, while
ResSim's does, repeatedly, in alternating sign. That points at the Jacobian/flux linearization
or the well-perforation coupling at that cell, not at the damping/chop machinery. Circumstantial
detail worth carrying into the next diagnostic pass: the well-perforation Jacobian entry
`d(res_pf)/dsw` at cell 0 swings `-1.79e4 → -6.51e3 → +1.58e4 → -8.91e3 → +1.72e4 → -9.39e3 →
+5.64e3 → -2.26e4 → ...` — sign-flipping almost every iteration, magnitude `1e3`–`2e4` — but
this is evaluated *at* the oscillating state, so it is as likely a symptom of the oscillation as
a cause. **Not discriminated this session; flagged for Y0.1-continuation, not Y1.**

### 6.2 Gas-rate 459: confirmed G2, but a different mechanism than guessed

Full LEDGER command: `FIM_TRACE_FILE=<f> cargo test --release --manifest-path
src/lib/ressim/Cargo.toml --lib repro_gas_rate_20x20x3_opm_aligned_no_trace -- --ignored
--nocapture` (bit-identical to the recorded baseline: `459/337/0/0`,
`dt=[7.032e-5,9.539e-4]`).

**Not a transient.** The plan's §1/§2 framed this as a dt-collapse "window"; the ledger shows
it is a **steady-state limit cycle spanning nearly the entire run**, from substep ~4 through
substep 458 (the last substep, reaching `t=0.250000` exactly): every accepted substep grows the
next trial dt by `2x`–`3x` (`limiter=opm-restart-growth`/`opm-max-growth`), the grown trial
almost always fails (`retry_class=linear-bad`), retries once at roughly half the failed dt, and
accepts — then immediately grows again into the same failure next step. This grow→fail→retry→
accept cycle alone accounts for the bulk of the 337 linear-bad retries (**zero**
`nonlinear-bad`, all 459 substeps). Every accept and every failing trial alike take only `3`
Newton iterations — this is not a slow grind, it fails fast and retries fast, over and over.

**The dominant retry site drifts, it is not fixed at one cell** — histogram over all 337
retries: `oil@121` (113), `well@3600` (80, the first well-row past the 3600 reservoir rows —
`n_cells=1200 × 3 eqs`), `oil@1201` (66), `oil@7` (55), `oil@1204` (12), `oil@1261` (5, the site
the plan's baseline table happened to record — that field is the *last-observed* dominant site,
not the mode; worth fixing in future baseline tables since it undersells how much the failure
moves with the advancing front). This is consistent with a genuine near-front/well
CPR-preconditioner weakness, not one stuck degree of freedom.

**The key mechanism finding, from a windowed per-iteration trace at substeps 60-62** (`
FIM_TRACE_FILE=<f> FIM_TRACE_SUBSTEP_START=60 FIM_MAX_SUBSTEPS=63 cargo test --release ...`):
iterations classified `linear-bad` are frequently ones whose **nonlinear state is already
converged** — e.g. a retried trial's iteration 1 shows `CNV-MB ... would_accept=strict
binding=[none]` (i.e. the simulator's own strict CNV+MB criteria are already satisfied) with
`res=3.25e-5`, yet the ledger still records that substep's outcome as `retry_class=linear-bad`.
Reading `classify_retry_failure_with_site`
([newton.rs:345-388](../src/lib/ressim/src/fim/newton.rs)): the class is `LinearBad` iff
`!report.converged` for that iteration's linear solve — a flag entirely independent of the
outer CNV-MB/Newton-residual state. The reduction-ratio diagnostics at these same iterations
(`avg_rr` `~1e-4`–`1e-7`, `last_rr` `~1e-7`) sit **far under** the `STRONG_CPR_AVERAGE_
REDUCTION_RATIO=0.25`/`STRONG_CPR_LAST_REDUCTION_RATIO=0.5` thresholds
([newton.rs:38-39](../src/lib/ressim/src/fim/newton.rs)) that would otherwise reclassify a
converged-but-unimpressive solve as `NonlinearBad`/strong-CPR — meaning `report.converged` is
false here for a reason *other* than reduction quality (most likely a small
iteration/application budget: `apps=2`–`3` per Newton iteration throughout this trace, i.e. the
CPR/BiCGSTAB solve is stopping well short of whatever hard convergence gate it checks, even
though the *result* it already has is numerically excellent).

**Conclusion**: the gas-rate 459-substep count is not the reservoir Newton loop struggling
(zero `nonlinear-bad`, fast per-substep convergence) — it is the **linear solve's own internal
convergence gate rejecting solves that were already good enough for the nonlinear iteration**,
compounded by a growth policy that immediately re-attempts a similar dt with no memory of the
retry it just took. This confirms G2 (linear stack quality) as the dominant driver, as the plan
suspected, but via a specific, different mechanism than "the search direction is bad" — it's
"the gate is stricter than the need." This directly matches the FIM-DIAG-003 "H2 — linear-
precision floor" hypothesis shape, in a **different case, different site, different symptom**
than where H2 was refuted (the heavy-case near-`1e-7` MB plateau, refuted in D1 by forced-
direct-linear showing no material change). *That refutation does not transfer here* — it was
scoped to one mechanism on one case; this is a distinct linear-gate-vs-need mismatch on a
three-phase, well-row-dominated case. Treat H2 as live again for this case specifically.

### 6.3 What Y0 did *not* resolve (explicitly, so it isn't silently dropped)

- **G1's true root cause** (why ResSim's raw Newton step at the well-connected cell oscillates
  where OPM's doesn't) is unlocalized — narrowed out of the damping/chop layer, not yet pinned
  on the Jacobian, the well formulation (G4), or the linear solve (see §6.4).
- **G5 (three-phase regime-switching)** was not implicated by the gas-rate trace — every
  observed failure is `linear-bad` with a clean `Saturated` regime in the traced cells, no
  variable-substitution activity visible in the windowed sample. Not ruled out elsewhere in the
  run (only substeps 60-62 were traced in per-iteration detail), but no positive evidence for it
  either. Y2(b) should not be scheduled from this evidence alone.
- **G4 (well primary-variable structure)** is untouched by direct measurement this session; it
  remains a plausible contributor to §6.1's raw-step-magnitude gap by architecture argument
  only (per-perforation `q` + `rate_consistency` rows vs OPM's `WQTotal`/fractions/`bhp`), not
  by trace evidence.

### 6.4 Masking caution: two findings, possibly one cause — not yet discriminated

Per the standing lesson of this program (Bundle W's mechanism fix exposed a second hidden gap;
X0 found the "obvious" ordering hypothesis was the wrong root cause) — **do not read §6.1 and
§6.2 as two independent, fully-explained findings.** Both traces show the `fgmres-cpr`/CPR-
BiCGSTAB linear solve running a small number of applications (`apps=2`–`4`) and few linear
iterations (`linear_iters=1`–`3`) per Newton iteration, on both the heavy case's oscillating
substep 0 and the gas-rate case's rejected-but-converged retries. A single, unconfirmed,
plausible unifying hypothesis: **an under-resourced/low-quality CPR linear solve could
simultaneously (a) produce a poor-quality raw Newton *direction* at well-adjacent cells under
the heavy case's larger dt, feeding §6.1's oscillation, and (b) fail its own internal
convergence gate on the gas-rate case's well/near-front rows despite the resulting *state*
being fine, feeding §6.2** — i.e. one linear-stack deficiency wearing two different masks
depending on dt/regime. This would mean Y1's CPR-restriction-variant promotion
(`FIM-LINEAR-005`) is not just the confirmed fix for §6.2 but a candidate fix for §6.1 too — or
it could resolve only one and leave the other exposed, exactly as Bundle X's fix resolved the
well-side standoff but left the reservoir CNV plateau newly visible. **Y1 must re-measure both
the heavy-case raw-step oscillation (§6.1's iteration-by-iteration `sw`/CNV-MB trace) and the
gas-rate limit cycle (§6.2's retry histogram) before and after the restriction-variant swap** —
do not declare G1 resolved just because Y1's own target metric (gas-rate retries, bounded-case
counts) improves.

### 6.5 Priority update for Y1

Y0's evidence sharpens, rather than changes, the plan's existing sequencing: Y1
(`FIM-LINEAR-005`, CPR restriction-variant promotion) remains the correct next lever — now with
two independent, mechanism-level reasons to expect it to matter (§6.2 directly, §6.4
speculatively for §6.1) instead of one plausibility argument. Y1's gate should explicitly
include the two traces above as before/after checks, not just the summary substep counts.

**§6.5 turned out to be wrong — see §7.** `FIM-LINEAR-005` was not a pending lever; it was
already live. Kept unedited above for provenance (it's what motivated attempting Y1 next); do
not re-read it as current guidance.

## 7. Y1 attempt (2026-07-12): the promotion was already live — what that exposes

### 7.1 What was actually found, in order of discovery

1. **`solve()`'s hardcoded CPR restriction is `CprPressureRestrictionKind::QuasiImpes`**, not
   `Row0Schur`
   ([gmres_block_jacobi.rs:2047](../src/lib/ressim/src/fim/linear/gmres_block_jacobi.rs)). `git
   blame` on that exact line: commit `77ec900e`, **2026-07-05** — a full week before this plan
   was written. The code comment at the same site says outright: "Promoted 2026-07-04 (Phase 9
   Step 9.3, `FIM-LINEAR-005`)."
2. **`FimLinearSolveOptions::default()` already carries the loosened Phase 10 bundle**
   (`FIM-LINEAR-008`): `max_iterations: 20, relative_tolerance: 5e-3, absolute_tolerance: 1e-12`
   ([linear/mod.rs:181-183](../src/lib/ressim/src/fim/linear/mod.rs)), and `solve()`'s fine
   smoother is `CprFineSmootherKind::BlockIlu0` when the backend is `FgmresCpr`
   ([gmres_block_jacobi.rs:2029-2033](../src/lib/ressim/src/fim/linear/gmres_block_jacobi.rs)).
   The comment there: "Re-applied after a first live attempt regressed the heavy case... see
   Phase 10" — confirming the earlier-documented revert (worklog "Step 10.4") was superseded by
   a later, successful re-application this plan's own §1 baseline table never mentioned.
3. **`FIM-LINEAR-010` (well-Schur elimination) is also live by default**
   (`eliminate_wells: true`, [linear/mod.rs:187](../src/lib/ressim/src/fim/linear/mod.rs)) —
   `solve_linearized_system` routes any system with well/perforation rows through
   `well_schur::solve_with_well_elimination` before it ever reaches `gmres_block_jacobi::solve`
   ([linear/mod.rs:242-257](../src/lib/ressim/src/fim/linear/mod.rs)).

**Net: the three linear-stack levers Y1 was written to promote — restriction, tolerance/
smoother bundle, well elimination — are all three already live.** `docs/FIM_EXPERIMENT_
REGISTRY.md`'s `FIM-LINEAR-005` row said `OPEN` with `row0-schur (current, live)`; that was
stale from the moment Phase 10 built on top of the Step 9.3 promotion without anyone circling
back to close the row. Corrected in the registry as part of this checkpoint. **Every baseline
number in this plan's §1 table (heavy `17`, gas-rate `459`, etc.) was already measured with
this entire bundle active** — Y0's findings are not "what happens before Y1," they are "what
`OpmAligned` still does wrong after every linear-stack lever tried so far."

### 7.2 Why the evidence behind that promotion doesn't cover the case this plan targets

Went looking for whether an even-better restriction variant exists unpromoted (the honest
version of "perform Y1" once the literal action turned out to be done) by re-capturing a fresh
corpus on the current tree, under `OpmAligned`, per this plan's own Y1 text ("re-capture on the
post-X4 tree first since trajectories changed"). This is what actually happened:

- **Heavy case** (`FIM_CAPTURE_DIR=<dir> FIM_NESTED_WELL_SOLVE=1 cargo test --release ...
  repro_water_pressure_12x12x3_opm_aligned_no_trace -- --ignored`): **1 file captured.** The
  case now has `linear_bad=0, nonlinear_bad=1` (confirmed by the same run's summary line) — its
  remaining problem (§6.1's raw-Newton oscillation) essentially never trips a linear-solve
  *failure* to capture. This on its own is informative: the linear stack is not what's failing
  on the heavy case anymore.
- **Gas-rate case** (`FIM_CAPTURE_DIR=<dir> cargo test --release ...
  repro_gas_rate_20x20x3_opm_aligned_no_trace -- --ignored`, the case with **337** `linear-bad`
  retries per §6.2): **0 files captured.** Not a capture-harness bug — reading
  `fim/newton.rs` line by line found why: the whole capture-on-linear-failure block (direct-LU
  fallback ladder, dead-state/restart-stagnation bypass bookkeeping, and the `FIM_CAPTURE_DIR`
  write call at line ~3647) lives inside **`if !opm_aligned { ... }`**
  ([newton.rs:3588](../src/lib/ressim/src/fim/newton.rs)). `OpmAligned`'s own linear-failure
  handling is a completely separate branch (`newton.rs:3700-3780`, "Bundle N checkpoint 3 (N5,
  `OpmAligned` only)"): no fallback ladder, no capture call — either the relaxed-reduction
  check (`OPM_RELAXED_LINEAR_SOLVER_REDUCTION = 0.01`,
  [newton.rs:1822](../src/lib/ressim/src/fim/newton.rs)) accepts the near-miss, or the Newton
  iteration aborts immediately via `classify_retry_failure_with_site` and returns.

**This means every restriction/tolerance/smoother/well-elimination decision made in Phase 9-11
was validated exclusively against `Legacy`-flavor captured failures** (confirmed separately:
the original 54+13-system corpora were captured via `repro_water_pressure_12x12x3`/
`repro_water_pressure_23x23x1`, whose driver hardcodes `opm_aligned=false`) — **and the capture
mechanism cannot see an `OpmAligned` failure at all, structurally, as the code stands today.**
Bundle Y exists specifically to close the `OpmAligned`-vs-OPM gap. Every "PROMOTED" linear-stack
verdict in the registry to date has zero direct empirical evidence from the flavor this whole
program targets — it happens to be shared code, so the settings apply to both flavors, but
nobody has ever measured whether they're the *right* settings for `OpmAligned`'s specific
failure shapes (the §6.1 oscillation, the §6.2 well-row/near-front CPR struggle).

A second, smaller gap in the same lab: `solver_lab_compare_restriction_variants`
([solver_lab.rs:270](../src/lib/ressim/src/fim/linear/solver_lab.rs)) calls
`solve_with_restriction_kind`, which calls `gmres_block_jacobi::solve_with_cpr_fine_smoother`
**directly** — bypassing `solve_linearized_system`'s well-Schur-elimination dispatch entirely.
So even a same-flavor rerun of the existing lab today would not exercise the actual live code
path once `FIM-LINEAR-010` is accounted for.

### 7.3 A loose thread noticed in passing, not chased down

While reading the `OpmAligned` no-fallback branch to understand why gas-rate's captures came up
empty, one of the trace lines from §6.2's windowed run reads `LINEAR FAILED (opm-aligned, no
fallback) converged=false finite=true reduction=n/a` — `reduction` is `None`, meaning
`linear_report.failure_diagnostics` was `None` despite `converged=false`. A brief look at
`gmres_block_jacobi.rs` found every `failure_diagnostics: None` site sits on a `converged: true`
return path — none of them explain a `converged: false` result with no diagnostics attached.
The likely explanation is that this system actually went through `well_schur::
solve_with_well_elimination` (§7.2's second gap) rather than `gmres_block_jacobi::solve`
directly, and that wrapper's own failure/recovery path was not read this session. **Not
resolved — flagged for whoever picks this up next; it may be the literal mechanism inside
§6.2's "linear-bad fires on an already-converged nonlinear state."**

### 7.4 What this does and doesn't change about §6

§6.1 and §6.2's factual observations (the oscillation trace, the retry histogram, the
already-converged-nonlinear-state pattern) stand — they were measured directly, not inferred
from docs. What changes is the *interpretation* of §6.2 as "G2, fixable by a restriction swap":
that swap already happened, is already live, and the problem persists. §6.4's "masking"
hypothesis (one under-resourced CPR solve wearing two masks) is neither confirmed nor refuted by
this — it's simply now clear that *further* restriction-variant tuning cannot be trusted without
first fixing the evidence-gathering gap in §7.2, because there is currently no way to measure
whether a candidate change helps or hurts on the actual `OpmAligned` failures.

### 7.5 Options for the real next step (not decided in this session)

1. **Close the evidence gap first.** Add an `OpmAligned`-side capture call (small, additive,
   same no-op-gated pattern as every other diagnostic hook in this codebase — mirror the
   existing `!opm_aligned` block's capture call at the `OpmAligned` abort site,
   `newton.rs:3744-3755`), re-capture the gas-rate case's 337 failures and whatever the heavy
   case's rarer ones look like, and re-run the restriction/tolerance/smoother comparison lab
   *through* `solve_linearized_system` (so well-Schur elimination is included) against those
   real systems. This is the honest version of what this checkpoint set out to do, gated on
   confirming with the user before adding new instrumentation to the Newton hot path.
2. **Chase §7.3's loose thread**: understand exactly why `OpmAligned`'s abort path produces
   `reduction=n/a`, and whether `well_schur::solve_with_well_elimination`'s own failure
   reporting is the missing piece — this might directly explain §6.2's mechanism rather than
   requiring a whole new capture corpus.
3. **Skip the linear stack for now and act on §6.1 instead** — the heavy case's raw-Newton
   oscillation has near-zero linear-solve failures to investigate via this route anyway; G4
   (well primary-variable restructure) or a targeted Jacobian audit at the injector-connected
   cell may be more tractable than re-litigating a linear stack that's already been tuned three
   times this quarter.

No code promoted this checkpoint. `FIM-LINEAR-005`'s registry row corrected to `PROMOTED`
(reflecting reality, not new work). Two fresh capture directories exist in scratch
(`/tmp/.../fim-y1/{heavy,gasrate}-capture`) — not committed, not durable; re-capture if this
work resumes in a future session.

## 8. Y1a executed (2026-07-12): the gas-rate catastrophe is not a linear-solve-quality problem

Registry: `FIM-LINEAR-012` (DIAGNOSTIC). Picked option 1 from §7.5. Two additive, no-op-gated
changes landed (both verified no-op: bit-identical control runs with the env vars unset,
`assembly_ad` parity 10/10, wasm rebuilt green):

- **`fim/newton.rs`**: a capture call mirroring the existing `!opm_aligned` one, added at the
  `OpmAligned` no-fallback abort site (the `else` branch after `LINEAR FAILED (opm-aligned, no
  fallback)`). Gated on `FIM_CAPTURE_DIR`, same as the Legacy-side call.
- **`fim/linear/well_schur.rs`**: an `FIM_WELL_SCHUR_DEBUG=1` diagnostic print at the wrapper's
  existing full-residual safety-net check, reporting the reduced solve's own verdict alongside
  the wrapper's outer verdict.

### 8.1 Fresh corpora, captured on the current tree, under `OpmAligned`

| Case | Command | Systems captured |
|---|---|---|
| Heavy (`12x12x3 --dt 1`, `+nested_well_solve`) | `FIM_CAPTURE_DIR=<dir> FIM_NESTED_WELL_SOLVE=1 cargo test --release ... repro_water_pressure_12x12x3_opm_aligned_no_trace -- --ignored` | **1** |
| Bounded (`23x23x1 --dt 0.25`) | `FIM_CAPTURE_DIR=<dir> cargo test --release ... repro_water_pressure_23x23x1_opm_aligned -- --ignored` | **1** |
| Gas-rate (`20x20x3 --dt 0.25`) | `FIM_CAPTURE_DIR=<dir> cargo test --release ... repro_gas_rate_20x20x3_opm_aligned_no_trace -- --ignored` | **337** |

The heavy/bounded counts (1 each) independently corroborate §6.1: those cases' remaining
problems are essentially not linear-solve failures anymore. Gas-rate's `337` matches its live
`linear_bad` count exactly — the new hook captures everything it should.

### 8.2 Restriction choice: re-confirmed correct on real `OpmAligned` data (not just re-asserted)

`solver_lab_compare_restriction_variants` on the fresh 337-system gas-rate corpus (using
`FimLinearSolveOptions::default()`, i.e. today's live tolerance/budget, but the lab's
restriction-isolation path forces `CprFineSmootherKind::FullIlu0`, not the live `BlockIlu0` —
a known lab limitation, see §7.2):

| Variant | Converged | Wins | Median rel. residual |
|---|---|---|---|
| `row0-schur` | 336/337 | 25/337 | `6.512e-4` |
| `sum-rows` | 337/337 | 54/337 | `2.131e-3` |
| `diag-balanced-sum` | 12/337 | 0/337 | `1.154e0` |
| `dominant-diag-row` | 337/337 | 38/337 | `1.788e-3` |
| `local-schur-balanced` | 336/337 | 0/337 | `6.512e-4` |
| **`quasi-impes`** | **336/337** | **220/337 (65%)** | **`1.133e-4`** |

Unlike the old Legacy-flavor corpora (where `row0-schur` converged `0/54` and `0/13` —
convergence pass/fail was the differentiator), on these real `OpmAligned` systems almost every
variant converges; `quasi-impes` wins on solution *quality* by 5-6x over the next-best. **This
is the first time `FIM-LINEAR-005`'s restriction choice has been checked against the flavor it
actually runs under — and it holds up.** The already-live choice is confirmed, not merely
inherited from stale evidence.

### 8.3 The decisive result: today's exact live configuration converges every one of these systems offline

`solver_lab_compare_backends` (`run_backend`, which calls `solve_linearized_system` — the real
production dispatcher, so well-Schur elimination fires exactly as it does live) on the same
337-system corpus:

```
=== aggregate over 337 systems ===
sparse-lu reference failures: 0
gmres-ilu0 converged: 337
fgmres-cpr converged: 337
fgmres-cpr reproduced live failure offline: 0
```

Every single system that was captured **at the exact moment it was classified as a live
`linear-bad` failure** — same Jacobian, same RHS, same layout, same equation scaling, same
`FimLinearSolveOptions::default()` (quasi-impes, block-ILU0, `5e-3`/`20`, well-Schur
elimination) — converges cleanly when replayed through the real dispatcher in isolation:
`iters=2`, relative residuals in the `2e-5`-`7e-5` range (individual capture lines, e.g.
capture `00334`: `true_res=4.625e-7 rel=1.910e-5`; capture `00336`: `true_res=1.516e-6
rel=4.793e-5`). The lab's own built-in "stop condition 2" assertion — written under the old
Legacy-corpus assumption that a captured failure should still fail offline, else "the capture is
missing state" — panics on this corpus: `current FgmresCpr converged offline on 337/337 systems
that failed live — capture fidelity is suspect`. That assertion firing is itself the finding,
not a broken test: **the gas-rate catastrophe is not caused by the linear solve being unable to
solve these systems.**
It is caused by something in how the live call site interprets or reports an otherwise-good
solve as a failure.

### 8.4 Mechanism, localized to the well-Schur-elimination wrapper (not fully pinned to one line)

Added the `FIM_WELL_SCHUR_DEBUG` print (§ above) and re-ran a live windowed trace
(`FIM_TRACE_SUBSTEP_START=60 FIM_MAX_SUBSTEPS=61`, same window as §6.2). Repeating pattern in
the output:

```
WELL-SCHUR-DEBUG reduced_converged=true reduced_iters=0 full_residual_norm=4.666025e-2 tolerance=2.333004e-4 rhs_norm=4.666007e-2 reduced_final_residual_norm=4.666025e-2
WELL-SCHUR-DEBUG reduced_converged=true reduced_iters=0 full_residual_norm=8.705666e-2 tolerance=4.352834e-4 rhs_norm=8.705667e-2 reduced_final_residual_norm=8.705666e-2
WELL-SCHUR-DEBUG reduced_converged=true reduced_iters=0 full_residual_norm=5.751940e-2 tolerance=2.875970e-4 rhs_norm=5.751940e-2 reduced_final_residual_norm=5.751940e-2
```

(interleaved with many `reduced_iters=1`/`2` lines that converge correctly — this is not every
solve, just a recurring subset). In each of these lines: the **reduced** (well-eliminated)
solve reports `converged=true` after doing **zero iterations** — i.e. the trivial `x_0=0`
starting guess satisfied the reduced system's own tolerance/`family_ok` check immediately — yet
`full_residual_norm` (the recovered full-system residual, `well_schur.rs:257-258`) sits at
essentially the *original* `rhs_norm`, 200x over `tolerance`. The full-system solution this
produces is effectively zero — obviously not converged. `well_schur.rs`'s own safety-net check
(`well_schur.rs:264`, `converged: reduced_report.converged && full_residual_norm <= tolerance`)
correctly catches this and downgrades to `converged=false` overall. But
`failure_diagnostics: reduced_report.failure_diagnostics` (`well_schur.rs:267`) is forwarded
unchanged — and since `reduced_report.converged == true`, every `converged: true` return path in
`gmres_block_jacobi.rs` sets `failure_diagnostics: None` (verified: every such site checked, none
is an exception). So the wrapper's overall report is `converged: false, failure_diagnostics:
None` — precisely the `reduction=n/a` shape from §7.3/§6.2's original trace. Tracing this up to
`newton.rs`'s `OpmAligned` abort branch: `reduction = failure_diagnostics.map(...)` is `None`,
`accept_relaxed` is unconditionally `false` (needs `reduction.is_some_and(...)`), so the Newton
iteration hard-aborts — with no way to ever discover, from inside that branch, that the
underlying linear solve was actually fine.

**What is confirmed**: this exact bug shape (spurious `reduced_iters=0` "convergence" → correct
safety-net rejection → lost `failure_diagnostics` → forced hard abort regardless of solution
quality) reproduces repeatedly on the live gas-rate run, and independently explains §8.3's
offline/live discrepancy without needing any other hypothesis. **What is not yet confirmed**:
direct line-by-line correlation between a specific `FIM_WELL_SCHUR_DEBUG` line and a specific
`LEDGER retry ... retry_class=linear-bad` line in the same run (the two trace streams weren't
cross-referenced by iteration index this session) — so treat "this is *the* mechanism" as
strongly evidenced, not proven exhaustive; there could be a second contributing path not yet
seen. Per the standing masking caution (§6.4): confirm by correlation before treating this as
fully closed.

**Why does the reduced solve return `converged=true` at zero iterations for a state that clearly
isn't converged?** Not resolved this session. Two candidate mechanisms, not yet distinguished:
(a) a `family_ok`/tolerance-basis bug specific to the *reduced* system (its own internal
tolerance check uses `elimination.reduced_rhs`'s norm, not the wrapper's later full-`rhs`-based
`tolerance` — if the reduced RHS is disproportionately small relative to the full RHS for these
particular well-row-dominated states, the reduced solve's own check could pass trivially while
the eliminated well-tail still carries most of the real residual); (b) something in the
elimination/recovery arithmetic (`well_schur.rs:234-252`) producing a degenerate `dx_tail` that
happens to leave the full solution near zero even when the reduced correction is also near zero.
(a) is favored by the numbers (`full_residual_norm ≈ rhs_norm` suggests the *whole* solution
stayed near zero, cause unclear) but not confirmed.

### 8.5 What this means for the plan

This is very likely the primary mechanism behind the plan's gas-rate `2→459` catastrophe and a
strong contributor to `OpmAligned`'s general bounded-case cost disadvantage vs Legacy (every
bounded `OpmAligned` case's `retry_dom` is `linear-bad`, per the plan's original §2 G2 framing —
now reframed: not "the linear stack is weak," but "a subset of results the linear stack
correctly produces are being discarded due to a diagnostics-propagation bug"). It does **not**
explain §6.1 (the heavy case's raw-Newton oscillation) — that case has almost no linear failures
to trigger this path in the first place (§8.1's `1`-system capture). G1 and G2 remain separate
problems; §6.4's "one root cause wearing two masks" hypothesis is **not** supported by this
finding and should be considered weakened, not confirmed.

**Not yet done, deliberately**: no fix implemented. The two candidate fixes named in the
registry row (fix the reduced solve's spurious check, or make the wrapper synthesize proper
`failure_diagnostics` on its safety-net downgrade) are both small and low-risk in isolation, but
per this project's standing discipline, the mechanism should be pinned down further (the §8.4
correlation gap) before writing code that changes live acceptance behavior — a wrong fix here
changes what `OpmAligned` accepts on every bounded/gas case, not just the target one.

### 8.6 Y1d: correlation closed, root cause re-localized (2026-07-12)

(Named Y1d, not Y1c — `Y1c` is already reserved in `TODO.md`/§8.5's own next-steps list for
"skip the linear stack, work G1 directly".)

Closed §8.4's two open items — the correlation gap, and the (a)/(b) discrimination — with two
additive, no-op-verified debug prints (`assembly_ad` parity 10/10 before trusting the run) and
one native run of `repro_gas_rate_20x20x3_opm_aligned_no_trace` (`--release`, `FIM_TRACE_FILE`
+ `FIM_WELL_SCHUR_DEBUG=1` set together so both prints land in the same file, in true call
order, as the existing `LEDGER retry` lines):

1. **`well_schur.rs`'s existing `FIM_WELL_SCHUR_DEBUG` print now also writes through
   `trace_sink::write_line`** (previously `eprintln!`-only, an untimestamped separate stream
   with no ordering guarantee against the trace file).
2. **New `CPR-ACCEPT-DEBUG` print in `gmres_block_jacobi.rs:1651`** (the `beta <= tolerance &&
   family_ok(&residual)` branch — the *preconditioned*-residual accept check, evaluated at
   `iterations == 0` before any Krylov step), gated the same way, firing only when
   `residual_norm > 10 * tolerance` (i.e. only the degenerate case, not every clean 2-iteration
   convergence).

Result: reproduced the exact `459/337` baseline again (`accepted_substeps=459 linear_bad=337`),
confirming the instrumented run is representative. Trace file analysis (3,066 lines):

- **337 `CPR-ACCEPT-DEBUG` lines, 337 `WELL-SCHUR-DEBUG reduced_iters=0` lines, 337 `LEDGER
  retry retry_class=linear-bad` lines — and all three counts are the same 337 events.** Checked
  programmatically, not by inspection: every single one of the 337 `LEDGER retry` lines is
  *immediately* preceded by a `WELL-SCHUR-DEBUG reduced_iters=0` line, itself *immediately*
  preceded by the matching `CPR-ACCEPT-DEBUG iterations=0` line. Zero exceptions, zero
  unexplained `linear-bad` retries, zero `WELL-SCHUR-DEBUG reduced_iters=0` lines that *don't*
  lead to a retry. This closes the §8.4 correlation gap completely — not "strongly evidenced",
  proven exhaustive for this run. The masking-caution question from §6.4 is now further
  weakened for G2 specifically: there is exactly one mechanism producing every gas-rate
  `linear-bad` retry, not several.

- **Discrimination result — neither original candidate is the primary driver:**
  - **(b) degenerate elimination/recovery arithmetic**: refuted by inspection, not just
    measurement. `well_schur.rs:64-252`'s elimination/recovery is exact dense/sparse linear
    algebra (a textbook Schur complement), and it is independently checked by the very safety
    net that catches this bug (`full_residual = rhs - jacobian * solution`, recomputed against
    the *original* system) — if the recovery arithmetic were wrong, that check would report a
    residual inconsistent with a correctly-solved reduced system, not (as observed) a residual
    that exactly equals what an untouched `x=0` guess produces.
  - **(a) tolerance-basis mismatch** (reduced solve's own tolerance uses `reduced_rhs.norm()`,
    not the wrapper's full-`rhs.norm()`): real, but small — measured directly across all 337
    pairs, the relative difference between the two rhs norms is **median 0.00%, max 3.69%**.
    This cannot explain a 200x gap; downgrade from "favored" to "a minor, second-order
    contributor" (§8.4's "(a) is favored by the numbers" is superseded by this measurement).
  - **The actual mechanism**: `gmres_block_jacobi.rs:1651`'s `beta <= tolerance &&
    family_ok(&residual)` check accepts convergence based purely on the *preconditioned*
    residual `beta`, evaluated after exactly one preconditioner application to the untouched
    initial residual (`x=0`, `iterations == 0`, before any Krylov step). Measured across all 337
    events: `tolerance / beta` (how far under tolerance `beta` lands) has **median 12.5x, max
    31.2x**; `residual_norm / tolerance` (how far *over* tolerance the true, unpreconditioned
    residual sits) is **200.0x in every single case** (median = max — this is simply
    `1/relative_tolerance`, confirming it's a fixed structural ratio, not case-specific noise).
    In other words: for these particular well-Schur-*reduced* systems, one preconditioner
    application crushes the preconditioned norm 12-31x past the acceptance threshold while the
    quantity that actually determines full-system solution quality (the raw residual, which is
    what the safety net and the eventual full-system recovery depend on) is still 200x over
    threshold. The preconditioner is not lying about anything invalid — CPR's pressure-block
    inverse genuinely does reduce this particular residual's preconditioned norm sharply — but
    `beta` alone is not a trustworthy proxy for solution quality on these reduced systems, and
    the accept check has no raw-residual floor to catch that.

- **Why "reduced" specifically, not the ordinary full system**: not established this session —
  `run_cpr_iterative_solve` is the same generic function used for both, so this isn't literally
  well-Schur-specific code. Plausible candidates for a follow-up (not investigated): the
  preconditioner is built fresh against the *reduced* Jacobian each call, whose block/pressure
  structure has shifted (well/perforation rows removed) relative to what the restriction
  operator (`quasi-impes`) was tuned against; or these particular states are simply the ones
  where CPR's coarse solve happens to overshoot — the same failure mode could in principle occur
  on non-eliminated systems too, just not observed in this corpus (§8.1: only `1` non-reduced
  capture exists for comparison, too small to check).

**Verdict**: `FIM-LINEAR-012`'s "candidate mechanisms" section is superseded. The bug is not a
tolerance-*basis* mismatch and not elimination arithmetic — it is a missing raw-residual sanity
check on the generic CPR/FGMRES preconditioned-residual accept path, exposed (at least in this
corpus) specifically through well-Schur-reduced systems. No fix implemented this checkpoint —
per standing discipline, a candidate fix (e.g. requiring `residual_norm <= factor * tolerance`
as an additional guard on the `beta`-only accept, or on `iterations == 0` specifically) needs
its own measurement pass before being written, since `gmres_block_jacobi.rs:1651` is shared by
every `FgmresCpr`/`GmresIlu0` solve in the codebase, not just the well-Schur path — a wrong
guard here changes acceptance behavior far beyond `OpmAligned` gas-rate cases.

## 9. Y1e: `FIM-LINEAR-013` — the accept-check fix, measured and promoted (2026-07-12)

(Named Y1e, not Y2 — `Y2` is already reserved in `TODO.md`/§4's own roadmap for "evidence-
directed structural bundle (G4 xor G5)", a later, unstarted phase. This checkpoint is a direct
continuation of Y1d's own "needs its own measurement pass" recommendation.)

Fix: `gmres_block_jacobi.rs:1651`'s condition changed from `beta <= tolerance &&
family_ok(&residual)` to `iterations > 0 && beta <= tolerance && family_ok(&residual)`. At
`iterations == 0`, `solution` is provably still the untouched `x_0 = 0` initial guess (no Krylov
correction has happened yet) — the guard simply stops the code from ever reporting `converged:
true` on that untouched guess based on the preconditioned residual alone. From `iterations >= 1`
onward, `solution` reflects at least one real Krylov step, so the existing preconditioned-residual
test there is unchanged.

Minimal, generic — not well-Schur-specific — and low-risk: any case that previously exited
instantly at `iterations == 0` via this branch now does one more real Krylov step before
converging (correctness-neutral, marginal extra cost only in the rare cases where the fast-path
was actually legitimate).

**Offline validation note**: the 337-system gas-rate capture corpus from Y1a/Y1b/Y1d turned out
*not* to be usable for a before/after offline comparison here — `solver_lab.rs`'s `run_backend`
constructs `FimLinearSolveOptions::default()` rather than the live Newton loop's actual options,
so it never reproduced the live tolerance/`beta` relationship that triggers this bug in the
first place (this likely also explains Y1d's "334/337 offline, `iters=2`" result despite all 337
being live `linear_bad` failures — an open question from Y1d, now resolved). The only reliable
validation is the live re-measurement below, plus a new synthetic unit test
(`beta_only_accept_never_fires_on_the_untouched_initial_guess`,
`fim/linear/gmres_block_jacobi.rs`) that reproduces the same defect *class* — two independent
diagonal blocks, one with an inflated (`1e6`) diagonal — and is confirmed to fail without the
guard and pass with it.

**Live measurement** (native `repro_gas_rate_20x20x3_opm_aligned_no_trace`, `--release`):

| | `linear_bad` | `nonlinear_bad` | `accepted_substeps` |
|---|---|---|---|
| Before (Y1d baseline) | 337 | 0 | 459 |
| After (`FIM-LINEAR-013`) | 1 | 4 | 238 |

`linear_bad` collapses from 337 to 1 — direct confirmation that this accept-check defect was the
dominant driver of the gas-rate `linear-bad` storm, not a coincidental correlation. A small
`nonlinear_bad` count (4) appears where none existed before: expected, not concerning — Newton
iterations that were previously fed a *rejected* (aborted) linear solve are now fed the
*genuinely converged* one, and a handful of those now need slightly more nonlinear massaging
before their own acceptance test passes. Net: `accepted_substeps` `459 → 238` (48% reduction).

**Control matrix, both flavors, wasm rebuilt**: Legacy `8/4/4/2` unchanged (bit-identical);
Legacy heavy (`dt=1`, `25` substeps) unchanged, same `retry_dom`/rung breakdown; `OpmAligned`
bounded cases `16/24/12` unchanged (bit-identical) — consistent with §8.1's finding that these
cases barely exercise the well-Schur-reduced linear-bad path in the first place; `OpmAligned`
gas-rate `459 → 238` (matches the native measurement); `OpmAligned` bounded `12x12x3`+
`nested_well_solve` unchanged at `12`. **Zero regressions across the entire matrix** —
`gmres_block_jacobi.rs:1651` is shared code, so this is the load-bearing check for "did loosening
this accept path break anything relying on the old (buggy) fast exit," and it didn't.

**Gates**: `assembly_ad` parity 10/10; full `fim::linear` module test suite 37/37 (8 offline-lab
tests correctly `ignored` — they need `FIM_CAPTURE_DIR`); new regression test added and
confirmed to fail on the pre-fix code; locked smoke 3/3.

**Verdict: PROMOTED.** Registry: `FIM-LINEAR-013`. This does not close G2 on its own — gas-rate
`OpmAligned` is still `238` substeps vs Legacy's `2` and OPM's presumed low single digits — but
it removes a measured, non-physical source of retries, and the reduction (459→238) is large
enough that the *remaining* gap is now more likely to reflect genuine linear/nonlinear behavior
worth investigating on its own terms, not an artifact of this bug. Next candidates, not decided:
(1) chase the new `nonlinear_bad=4` — is it a real regression risk or a one-time reclassification
artifact; (2) continue toward gas-rate parity now that the accept-check noise is gone; (3) pivot
to G1 (Y1c, heavy-case oscillation), still untouched by this fix (heavy case's linear failure
count was always too small for this bug to matter there).

### 9.1 Y1f: chasing `nonlinear_bad=4` — closed, benign (2026-07-12)

Full replay method: reran the native `repro_gas_rate_20x20x3_opm_aligned_no_trace` test with
`FIM_TRACE_FILE` set (post-`FIM-LINEAR-013`), then diffed its `LEDGER` lines against the
pre-fix trace already captured for Y1d's correlation work (same test, tree at `ccbcf37`, before
`FIM-LINEAR-013`). Reproduced `238/1/4` exactly before analyzing.

**Finding**: all 4 `nonlinear_bad` retries come from a single event — the very first substep of
the run (`substep=0`, `t=0.000000`), across its own retry ladder (`retry_count=1..4`). This is
not 4 scattered failures; it is one already-known "large initial trial `dt` needs shrinking"
transient, now classified differently.

Direct before/after comparison of that exact substep's retry ladder:

| | retries before accept | `dt` at accept | `mb` progression | classification |
|---|---|---|---|---|
| Before (`ccbcf37`, pre-fix) | 7 | `0.000106546` | `inf` on every retry | `linear-bad` (all 7) |
| After (`a88072b`, `FIM-LINEAR-013`) | 4 | `0.002964803` | `7.7e-6 → 8.6e-7 → 1.1e-7 → 1.7e-8` (shrinking, finite) | `nonlinear-bad` (all 4) |

Before the fix, this substep's retry ladder was hitting the exact `FIM-LINEAR-012` bug on
*every* attempt — `mb=inf` on all 7 retries is the signature of a linear solve that never
actually ran (the spurious `reduced_iters=0` accept, immediately downgraded and hard-aborted
with no real Newton work done) — so the ladder had to shrink `dt` almost three orders of
magnitude (`0.25 → 0.000107`) before finally finding a `dt` small enough to sidestep the linear
bug entirely. After the fix, the *same* substep now gets a real, working linear solve on every
attempt; the residual/mb genuinely converge but don't quite clear the strict tolerance within
the `20`-iteration Newton budget at `dt=0.25/0.0825/0.027`, so it correctly retries as
`nonlinear-bad` (OPM's own `solver-restart-factor=0.33` ladder) — and finds an accepted `dt`
`~28x` larger than before (`0.002965` vs `0.000107`) in 3 fewer retries.

**Verdict**: not a regression, not a new failure mode — a strict improvement, correctly
reclassified. The `nonlinear_bad` count exists because the substep still needs *some* dt-shrink
ladder at the very start of the transient (expected: the initial trial `dt=0.25` is aggressive
by policy, `docs/FIM_OPM_PARITY_PLAN.md` §2 G3), not because `FIM-LINEAR-013` introduced new
nonlinear instability. No further action needed on this thread. Confirms (does not newly show)
that G3 (controller policy, initial `dt`/growth caps) remains a real, separate, later-priority
gap — visible now without the linear-bug noise on top of it.

## 10. Y1g: what actually drives `238` substeps — a well-cell Newton stall, not the linear stack (2026-07-12)

Continuing toward gas-rate parity (`238` `OpmAligned` vs Legacy's `2`) now that `FIM-LINEAR-013`
has removed the linear-accept noise. Full method: reran the gas-rate case through the wasm
`--diagnostic step` runner (`--preset gas-rate --grid 20x20x3 --steps 1 --dt 0.25
--opm-aligned`), which captures per-Newton-iteration `CNV-MB`/`STAGNATION-ATTRIB`/`STAG-TREND`
trace lines for every substep — 17,918 lines total, `243` substep attempts (`238` accepted, `5`
retries, matching the native measurement exactly).

### 10.1 The substep count is a limit cycle, not scattered failures

Across all `238` accepted substeps: `129` hit `iters=20` (the full Newton budget) and `109`
converge fast at `iters=3`. This pairing is not incidental — the growth-decision trace shows a
persistent 2-step cycle for the *entire* run, start to finish (checked substeps 0-19 and
223-237, both ends of the run show the identical pattern): an `iters=20` substep triggers OPM's
own iteration-count growth throttle (`growth=0.400 limiter=opm-iter`, shrinking the next trial
`dt`), the resulting smaller-`dt` substep converges fast (`iters=3`, `growth=3.000
limiter=opm-max-growth`, the max-growth ceiling), and the larger `dt` that produces immediately
hits `iters=20` again. Net growth per full cycle ≈ `0.4 × 3 = 1.2x` — this alone, compounded
over the run, is most of why `238` substeps are needed to cover 0.25 days.

**This is not a growth-policy bug.** `opm_iteration_count_dt` (`timestep.rs:483`) is a faithful
port of OPM's `PIDAndIterationCountTimeStepControl`: at `its=20, target=8`, `dt/(1 + (20-8)/8 *
1.0) = dt/2.5 = dt×0.4` — exactly the observed factor. The growth throttle is doing exactly what
OPM's own policy says to do given `20` iterations were used. **The real question is why Newton
needs `20` iterations so often** when the skill's own OPM reference point is "~2.5 Newton
iterations/step."

### 10.2 Oscillation is not the cause (ruling out a G1-style explanation)

`OSC-DETECT osc_phases>0` fires on only `4` of `2,989` per-iteration checks in the whole run,
all from the very first (transient-startup) substep, not from any of the `129` `iters=20`
substeps sampled afterward. G1's heavy-case mechanism (a genuine oscillating Newton trajectory)
does not explain this case's `iters=20` substeps.

### 10.3 The actual mechanism: a genuine, exactly-frozen fixed point at a well-adjacent cell

Sampled `6` `iters=20` substeps at wide intervals (substeps 8, 19, 27, 42, 65, 91, 158 — early,
middle, and late in the run) and inspected their full per-iteration `CNV-MB` trace. All six show
the identical shape: the residual reaches a near-tolerance value within the first 2-3
iterations, then **freezes bit-for-bit identical** for the remaining 14-17 iterations —
`cnv`/`mb` arrays exactly unchanged to the last printed digit, iteration after iteration — until
the final iteration (`19`) accepts via OPM's relaxed final-iteration tier (`would_accept=pv-relaxed`),
not because the strict criterion was ever cleared.

The frozen value is always *just* over the `1e-7` `mb` target — ratios observed from `1.02x`
(substep 65) to `2.6x` (substep 42) — and the binding cell is always well-adjacent: cell `0`/`1`
(the injector's own cell and its immediate neighbor), cell `20` (the injector's other areal
neighbor), or cell `400` (the injector's neighbor directly below it, `(0,0,1)` in a 20×20×3
grid where the injector sits at `(0,0,0)`). Every single sampled substep's stall localizes to a
cell touching the injector — the same structural shape as `FIM-DIAG-003`'s original H1 finding
("displaced well-cell standoff", confirmed there via 100% of frozen-MB iterations sitting at the
producer's own perforation cell or its immediate neighbor) — except here it recurs at the
**injector** under gas-rate `OpmAligned`, not the producer Bundle X's fix targeted.

The per-iteration `res=`/`upd=` detail line for substep 19 makes the freeze mechanical, not just
numerical: from iteration 3 onward, `upd=5.933e-7` is the *exact same floating-point value*
every iteration, and the existing `STAGNATION-ATTRIB`/`STAG-TREND` classifier (already
instrumented in the code, not new) correctly labels every one of these iterations `real-bump` or
`slow-decay` (alternating), accumulating `stagnation_count` up to `16`. This machinery already
exists specifically to detect this pattern — it was built for an earlier stagnation
investigation (`newton.rs:2689-2790`, `stagnation_acceptance_*`) and is firing exactly as
designed.

### 10.4 Why nothing intervenes: the escape mechanism is deliberately Legacy-only

`newton.rs:3275`'s `would_widen` gate (`stagnation_count >= 3 && trend_vs_entry < 0.5 &&
iters_remaining >= 3`) requires the residual to have dropped to *less than half* its
value-at-stagnation-entry before it will relax anything. In substep 19's trace, `trend_vs_entry
= 1.0570` (the residual is slightly *worse* than when stagnation began, not better) — so
`would_widen=false` is the objectively correct evaluation of that condition, not a bug in the
gate itself.

But the containing block never even runs under `OpmAligned` in the first place —
`newton.rs:3299`: `if !opm_aligned && stagnation_count >= 3 { ... }`, with an explicit standing
comment (`newton.rs:3293-3298`) stating this is intentional: *"this residual-trend bailout has
no OPM analog... Under `OpmAligned`, a stagnating trajectory simply keeps iterating (as OPM's
does) until the entry check accepts it or the iteration budget is exhausted and the relaxed
tiers decide."* So ResSim's `OpmAligned` flavor is, by explicit design, faithfully reproducing
what the code's authors believed OPM itself does in this situation: grind through the full
iteration budget with no early trend-based bailout, then rely on the final-iteration relaxed
tier.

### 10.5 What is and isn't established

**Established, with direct evidence**: the `238`-substep count is dominated by a real,
mechanically-understood Newton stall at well-adjacent cells, recurring on `~54%` of accepted
substeps throughout the *entire* run (not a transient), correctly detected by existing
instrumentation but not remedied under `OpmAligned` by design. Not a linear-solve problem (G2,
now largely closed by `FIM-LINEAR-013`) and not an oscillation problem (G1's mechanism, ruled
out directly).

**Not established — the load-bearing open question**: whether OPM itself actually needs a
comparable iteration count for equivalent well-adjacent states in a real gas-rate/injector case,
or whether OPM's own Newton trajectory for this configuration simply never reaches this
near-tolerance-but-frozen fixed point in the first place (e.g. because OPM's well/perforation
Jacobian terms, primary-variable choice, or exact CNV/MB formula differ enough at the
well-connected cell to avoid the stall entirely). The `newton.rs:3293` comment's claim about
OPM's own behavior is asserted, not verified against pinned OPM source or a real run for this
specific well-adjacent-stall scenario — the same "verify doc claims against source" discipline
that caught the stale `FIM-LINEAR-005` registry row (`FIM-LINEAR-012`, Y1) applies here to a
design comment, not a doc, but the standard is the same. Per this project's standing "measure,
don't guess a fix mechanism" discipline, this must be answered with OPM ground truth (an
INFOITER differential trajectory, the established D3/Y0 method) before touching either the
`would_widen` gate's `OpmAligned` exclusion or anything else here — this session did not attempt
that; it requires a gas-rate-comparable OPM reference deck, which does not currently exist in
this working tree (the existing reference decks referenced by the `opm-reference-pipeline` skill
live on a different branch, `origin/fim-opm-continuation-plan`).

**Verdict**: DIAGNOSTIC, not yet registered as its own experiment row (no code touched this
checkpoint). Next concrete step, not started: obtain or author an OPM gas-rate/injector
reference deck and run the INFOITER differential-trajectory comparison at one of these
well-adjacent stall states, to determine whether this is a genuine ResSim-vs-OPM divergence
(actionable) or an already-faithful reproduction of OPM's own behavior (in which case gas-rate
parity has to come from elsewhere — e.g. G3 controller tuning, or accepting `238` as correct
given the underlying physics/discretization).

## 11. Y1h: OPM ground truth obtained — the design comment's premise is refuted (2026-07-12)

Closes §10.5's open question. `origin/fim-opm-continuation-plan` (commit `cacdf767`) already has
`opm/reference-decks/gas-rate-10x10x3/CASE.DATA` — RESV-controlled injector (rate `500`,
`GAS`)/producer (rate `200`, `OIL`), `10x10x3`, `6×0.25`-day steps, `flow 2026.04` validated to
run. Extracted the deck via `git show` into a scratch dir (did not check out the branch, to
avoid disturbing this working tree) and ran it directly, without touching the whole
`opm-ressim-compare.sh` harness (just needed `--output-extra-convergence-info=steps,iterations`
for `CASE.INFOITER`/`CASE.INFOSTEP`, which the harness doesn't add by default).

**Exact replay**:
```
git show origin/fim-opm-continuation-plan@cacdf76701e33bccee6acc127845176be6080858:opm/reference-decks/gas-rate-10x10x3/CASE.DATA > CASE.DATA
flow CASE.DATA --output-extra-convergence-info=steps,iterations --solver-verbosity=3 --time-step-verbosity=3
node scripts/fim-wasm-diagnostic.mjs --preset gas-rate --grid 10x10x3 --dt 0.25 --steps 6 --opm-aligned --diagnostic summary --no-json
```
(ResSim side run at ResSim's own `HEAD`, `5381a1c`.)

**OPM Flow, verbatim (`CASE.INFOSTEP`)**:
```
  Time(day)  TStep(day)  ... WellIt Lins NewtIt LinIt Conv
          0        0.25  ...      0    8      7     8    1
       0.25        0.25  ...      0    6      5     5    1
        0.5        0.25  ...      0    5      4     4    1
       0.75        0.25  ...      0    4      3     3    1
          1        0.25  ...      0    5      4     4    1
       1.25        0.25  ...      0    4      3     3    1
```
**6 report steps, 6 total substeps (exactly 1 per report step), `Conv=1` every time — zero
timestep cuts.** Newton iterations: `7, 5, 4, 3, 4, 3`. `CASE.INFOITER`'s per-iteration `MB_Oil`/
`MB_Gas` for the first substep decay smoothly and monotonically by roughly an order of magnitude
every 1-2 iterations (`1.67e-3 → 1.84e-3 → 2.49e-3 → 3.36e-4 → 5.22e-5 → 1.28e-6 → 1.68e-5 →
6.48e-8`) — no freeze, no plateau, no relaxed-tier acceptance needed anywhere in the run.

**ResSim, verbatim (single cumulative summary line, `--diagnostic summary` over 6 report steps)**:
```
step=  6 | time=1.5000d | ... history+=695 | substeps=695 | accepts=695+0+0 | retries=0/5/0 |
... retry_dom=nonlinear-bad:well@901 ...
```
**695 total accepted substeps to cover the matching 1.5 days** (vs OPM's `6`) — a **~116x**
gap, on a matching hand-authored Flow input rather than a different-grid approximation — this
closes the grid-size caveat from §10's 20x20x3-only evidence. `retries=0/5/0`: even better than
the 20x20x3 case — essentially zero
linear-bad, confirming again that `FIM-LINEAR-013` closed the linear-stack side of this
completely; the entire remaining gap is nonlinear.

### 11.1 Verdict: the `newton.rs:3293-3298` design comment's premise is refuted

That comment claims *"Under `OpmAligned`, a stagnating trajectory simply keeps iterating (as
OPM's does) until the entry check accepts it or the iteration budget is exhausted."* OPM's real
trajectory on this exact case shows **no stagnation of any kind** — every step converges cleanly
in single digits of Newton iterations, no relaxed-tier acceptance, no near-tolerance freeze. The
premise that OpmAligned's lack of a trend-based bailout is *faithfully* modeling OPM is not
supported by this evidence. This is a genuine ResSim-specific defect, not an OPM-faithful
reproduction — §10's open question is closed.

### 11.2 Important prior-art connection: this is the exact code area of a previously-reverted lever

The stagnation-bailout machinery this session has been examining (`newton.rs:2689-2790`'s
`stagnation_acceptance_*` functions, the `would_widen` gate at `newton.rs:3275`, gated `if
!opm_aligned` at `newton.rs:3299`) is **the same file and mechanism family** as
`FIM-LINEAR-009`'s neighbor in the registry, `FIM-NEWTON-004`: *"Loosen or remove
residual-stagnation bailout / above-tolerance stagnation acceptance"* — **REVERTED**, verdict
recorded as *"Bailout still load-bearing; prior widening attempts regressed or no-oped."* Its
own registry-recorded retry condition: *"A new root cause explains the residual plateau and has
a guarded fix."*

This checkpoint (`Y1g`+`Y1h`) is exactly that new root cause, freshly measured against real OPM
output rather than assumed — but the scope is different from what `FIM-NEWTON-004` tried and
reverted. That lever widened the Legacy-side bailout itself (already enabled, already tuned);
what's on the table here is *extending an unchanged, already-existing Legacy mechanism to also
apply under `OpmAligned`*, where it currently doesn't run at all. Related but not identical —
still, the prior revert (and `FIM-NEWTON-005`'s separate, also-reverted post-loop
near-converged-acceptance attempt at this same class of problem, which caused a *live run to not
finish in 8+ minutes* by letting an under-converged state compound forward into later substeps)
means this must not be treated as a quick patch. Any fix here needs its own careful,
narrowly-scoped design and the full measurement discipline this bundle has used throughout
(offline reasoning first, then the bounded control matrix, heavy case, locked smoke, before
promotion) — not attempted this checkpoint. Paused here for explicit direction per standing
practice on this thread (the user was asked; see conversation).

**Verdict**: DIAGNOSTIC. Registry: not yet its own row — will be filed once a concrete fix
design exists to measure (or as a `REFUTED`/`OPEN` cross-reference row if the `FIM-NEWTON-004`
connection is judged to make any `would_widen`-style fix out of scope for now). No code changed.

## 12. Y1i: durable gas-rate oracle + acceptance-gate audit (2026-07-13)

This is a **measurement-infrastructure checkpoint, not a solver change**. No file under
`src/lib/ressim/src/` changed.

### 12.1 The oracle is now a tracked, verified fixture

The `gas-rate-10x10x3` Flow deck is now tracked at
`opm/reference-decks/gas-rate-10x10x3/CASE.DATA`, with a manifest recording the exact ResSim
diagnostic invocation, source commit, deck SHA-256, mapped input invariants, and the Flow
oracle (`6` substeps, Newton `7/5/4/3/4/3`, zero cuts). The manifest checker validates the deck
byte-for-byte before a run and parses `CASE.INFOSTEP` afterward. The side-by-side harness is
`scripts/opm-ressim-compare.sh`; it copies Flow input/output below the requested output
directory so the source tree remains clean.

Fresh Flow replay, using the tracked deck and Flow `2026.04`, reproduces the Y1h oracle exactly:
six accepted `0.25`-day substeps and `26` total Newton iterations. This preserves Y1h's real
result: ResSim and OPM have a large convergence divergence on the matching, hand-authored
reference input. It does **not** establish that their well equations or Newton updates are
identical; the fixture makes that mapping explicit for the next differential experiment.

### 12.2 Correction: `would_widen` is not the live acceptance guard

The post-Y1h source audit found a material distinction that the Y1g/Y1h writeups had conflated.
`newton.rs:3275` computes `would_widen` and emits it only in the `STAG-TREND` trace. The actual
Legacy branch starts at `if !opm_aligned && stagnation_count >= 3` (`newton.rs:3299`), without
requiring `would_widen`; `stagnation_acceptance_allows` then permits residuals up to
`NONLINEAR_HISTORY_RESIDUAL_BAND_FACTOR = 10×` the nominal tolerance when its other checks pass.

Consequently, §10.4's statement that `trend_vs_entry=1.0570` makes the legacy escape guard
inapplicable is false as a description of the executing code. A flip of the `OpmAligned`
exclusion would not be a narrow extension of a trend-protected mechanism. It would broaden the
already-reverted above-tolerance acceptance class to every qualifying stagnation event. This
explains why OPM's better trajectory is evidence of a divergence, but is not the guarded root
cause required by `FIM-NEWTON-004`/`005` to retry acceptance changes.

### 12.3 Next slice: isolate the first ineffective well-adjacent update

Do not change Newton acceptance. Add a native, exact `10x10x3` reproduction that can run both
the live iterative backend and the direct backend, then compare the first report step across a
bounded well/control matrix: no wells, injector only, producer only, both wells; rate and BHP
controls where meaningful. For each variant, record the first iteration where the injector-cell
update repeats, its residual/Jacobian family, and whether direct linear solving changes that
update. Generate matching Flow decks only for variants that isolate a ResSim failure.

Decision rule: a direct-vs-iterative difference owns a linear/update application issue; a
bit-identical ineffective direct update owns the nonlinear well/assembly formulation. Only that
second result authorizes a scoped well/Jacobian investigation (G4/G5). Neither result authorizes
an above-tolerance Newton acceptance change.

## 13. Y1j: injector-update isolation — linear stack ruled out (2026-07-13)

Added the ignored native driver `fim::timestep::phase5_repro::repro_gas_rate_10x10x3_y1j`. It
mirrors the ResSim side of the tracked `gas-rate-10x10x3` oracle for one 0.25-day report step.
`FIM_Y1J_WELLS=both|injector|producer|none`, `FIM_Y1J_CONTROL=rate|pressure`, and
`FIM_FORCE_DIRECT_LINEAR=1` select the matrix. `FIM_MAX_SUBSTEPS=1` deliberately stops after
the first accepted rung; it is a discriminator, **not** a completed 0.25-day result.

The measurements are provisional until replayed from the committed driver revision: the solver
implementation was unchanged, but the driver itself was new in the working tree. The terminal's
Cargo wrapper kept stale invocations waiting on its artifact lock, so commands used the just-built
native release test binary with the same filter and `--ignored --nocapture` arguments.

### 13.1 Direct versus iterative: same plateau and accepted rung

Both-well/rate/OpmAligned runs, with the live FGMRES/CPR stack and with the forced exact direct
backend, each take 5 nonlinear retries and accept exactly `dt=0.000978384825` after 20 Newton
iterations (`res=1.040889e-5`, `mb=9.668795e-10`). Both traces freeze at `res=1.041e-5` over
iterations 13--18, classify the same repeated state as stagnation, and leave `would_widen=false`.
The direct trace differs only at round-off level in BHP (`<1e-6 bar`) and retains the same
perforation rates. An exact linear solve therefore follows the same nonlinear trajectory, retry
ladder, and stalled accepted state: this is not CPR/FGMRES tolerance, preconditioner, or update
application behavior.

### 13.2 Bounded well/control matrix

All rows use the live iterative stack and `FIM_MAX_SUBSTEPS=1`.

| wells | control | retries | first accepted dt (day) | result |
| --- | --- | ---: | ---: | --- |
| both | rate | 5 nonlinear | `0.000978384825` | 20-iteration stall |
| injector only | rate | 6 nonlinear | `0.00032286699225` | stall worsens |
| producer only | rate | 0 | `0.25` | clean full-rung acceptance |
| none | rate | 0 | `0.25` | clean full-rung acceptance |
| both | pressure | 7 nonlinear + 1 linear | `0.00003515970997` | worse; accepted state has `perf@0` hotspot |

The injector is necessary and sufficient for this initial-rung pathology: removing it eliminates
fragmentation, while retaining only it worsens it. Switching both wells from rate to pressure
control does not remove the pathology, so it is not specifically the rate target/constraint row.
The pressure row's perforation hotspot is a separate reason not to treat this bounded test as an
end-to-end pressure-control result.

### 13.3 Verdict and next action

Y1j selects a **G4 injector-well Jacobian/primary-variable audit**, before G5
three-phase-variable-substitution work. The evidence supports an injector well/reservoir coupling
defect: it persists under an exact global linear solve, follows the injector rather than the
producer or no-well state, and survives removal of rate control. It does not yet prove a specific
derivative is wrong or rule out a gas-variable interaction; those are audit questions, not
permission to tune acceptance.

Next bounded slice: at the first stalled injector-only and both-well rate iterations, compare AD
and legacy injector-perforation plus connected-cell Jacobian entries against finite differences
(`d(res_pf)/dq`, `d(res_pf)/dp`, `d(res_pf)/dsw`, and component-row `d/dq`). Map the matching OPM
`StandardWell` primary variables before proposing a structural change. Do not reopen
`would_widen` or the `FIM-NEWTON-004`/`005` acceptance family.

## 14. Y2a: injector Jacobian audit finds an active-bound AD kink (2026-07-13)

Y2a added the test-only, environment-gated `FIM_Y2A_AUDIT=1` trace in `fim/newton.rs`. At the
first three-count stagnation point it reassembles the same state with the independent legacy
assembler and central-differences the legacy residual. For the injector perforation row and its
connected cell's water/oil/gas rows it records residual parity and derivatives against local
`p`, `Sw`, hydrocarbon variable, BHP, and perforation `q`. It is not compiled into production
builds and does not modify an iterate, matrix, or convergence decision.

The bounded injector-only replay was repeated from clean commit `5a600ae` using:

```text
FIM_Y1J_WELLS=injector FIM_Y2A_AUDIT=1 \
FIM_TRACE_FILE=/tmp/y2a-injector-onesided2-20260713.log FIM_TRACE_DT_BELOW=1 \
FIM_MAX_SUBSTEPS=1 target/release/deps/simulator-ddfbb4e26a955ef9 \
  repro_gas_rate_10x10x3_y1j --ignored --nocapture
```

It reproduces Y1j's injector-only result (`6` nonlinear retries; first accepted
`dt=0.00032286699225`; 20 iterations) and, at the final captured stalled rung
(`dt=3.2286699225e-4`, iteration 5), residuals agree between AD and legacy to
`1.137e-13`. The Jacobian does not: the injector cell is exactly at its connate bound
`Sw=Swc=0.15`, and the rate-consistency row has

| derivative | AD | legacy | central FD | forward FD | backward FD |
| --- | ---: | ---: | ---: | ---: | ---: |
| `d(res_pf)/dSw` | `-1428.5586` | `0` | `-714.2793` | `-1428.5585` | `0` |

This is a real one-sided kink, not finite-difference noise. The AD generic relperm path selects
the above-connate branch at equality; the legacy derivative intentionally returns zero at the
clamped boundary. The Newton correction points below `Swc` (`dSw≈-3.13e-5`) and the candidate is
therefore projected back to the same bound, leaving a large active-branch derivative in the
linear system but no admissible saturation movement. The same signature appears in the both-well
case. Several connected oil/gas `p`/`Sw` entries differ as well, so this is broader than a
perforation-row typo; changing only the well Jacobian would leave the reservoir block
inconsistent.

The OPM source audit still establishes the structural difference, but it is not the immediate
Y2a result. `StandardWellPrimaryVariables.hpp` defines per-well `WQTotal` and BHP (with fractions
for the relevant phase sets); `StandardWellPrimaryVariables.cpp` assigns a gas injector's surface
gas rate to `WQTotal`; and `StandardWell_impl.hpp` calculates each perforation connection rate
during assembly before scattering it to the reservoir equations. ResSim instead has BHP plus a
perforation `q` unknown and a rate-consistency row. For this one-perforation deck both systems
happen to have two well-side unknowns, so Y2a does **not** justify a G4 primary-variable
restructure yet.

**Verdict:** a guarded active-bound AD derivative investigation is now prior to G4 restructuring
or any acceptance change. It must establish a single boundary convention across the AD reservoir
and well blocks, with one-sided tests at `Swc` (and the analogous gas/upper-saturation clamps),
then measure the exact deck and control matrix. Do not globally flip generic clamp semantics or
patch only `d(res_pf)/dSw` from this one trace.

## 15. Strategy reconciliation: state-bound policy precedes derivative selection (2026-07-13)

A follow-up source read found a more fundamental divergence that changes Y2b's order. ResSim's
`FimState::enforce_cell_bounds` (`src/lib/ressim/src/fim/state.rs:250`, called at `:390`/`:436`)
clamps water saturation to `Swc` after every frozen Newton update. In OPM's
`OPM/opm-simulators/opm/models/blackoil/blackoilnewtonmethod.hpp:266-267`, the normal saturation
update limits the increment with `dsMax`; `chopAndNormalizeSaturations` is guarded by
`projectSaturations_` at `:456-457`. The default in
`blackoilnewtonmethodparams.hpp:42` is `ProjectSaturations::value = false`.

This is source-confirmed implementation divergence, not yet proof that projection alone causes
the live stall: the exact deck's option path, endpoint extension, normalization, and phase/primary
variable switching must still be traced. It does mean choosing an AD derivative at the `Swc` kink
before deciding whether `Swc` is even a hard Newton-state bound would be premature.

The original Y2b wording is therefore superseded by
`docs/FIM_OPM_CONVERGENCE_EXECUTION_PLAN.md`: first source-complete the state/update lifecycle,
then characterize one-sided derivatives and predicted-vs-realized updates, and only then authorize
a flag-gated behavior probe. G4/G5, controller, AMG, and acceptance changes remain deferred.

### 15.1 Y2b0 result: exact state/update lifecycle (2026-07-13)

**Status: complete, documentation/source audit only; no solver behavior changed.** The audit was
performed at commit `5c29a9d`. The tracked deck has no saturation-projection or `ds-max` override.
The installed Flow binary's help reports `--project-saturations` default `false` and `--ds-max`
default `0.2`; `scripts/opm-ressim-compare.sh --opm-only --no-build-wasm --out-dir
/tmp/ressim-y2b0-opm-audit` revalidated the oracle: six `TStep=0.25` rows, `NewtIt=7,5,4,3,4,3`,
and `Conv=1` throughout.

| Boundary | OPM Flow on this deck | ResSim `OpmAligned` | Audit consequence |
| --- | --- | --- | --- |
| `Sw = Swc = 0.15` | The normal update scales saturation deltas by `dsMax` but writes raw `Sw`; projection/normalization is conditional and disabled. Raw `Sw` is passed to the fluid state and accumulation. The default three-phase material law clamps endpoint *properties* separately: table `krw` is constant below its first point and oil paths use `max(Swl, Sw)`. | `opm_per_cell_chopped_update` applies the same 0.2 maximum implied saturation increment, then `apply_raw_update` calls `enforce_cell_bounds`, replacing `Sw < Swc` with `Swc` before the next residual/Jacobian assembly. | OPM can retain an endpoint-crossing mass state while endpoint relperms stay flat; ResSim erases both the state movement and its accumulation contribution. Plausible cause, not causal verdict. |
| `Sg = 0` | Raw `Sg` is updated without normal projection. `adaptPrimaryVariables` is called after every update and can change `Sg` to `Rs` once free gas becomes negative; the gas relperm table uses its endpoint value. | In the saturated regime, `enforce_cell_bounds` clamps the hydrocarbon variable to `[0, 1-Sw-Sorg]`. The regime remains frozen throughout the Newton loop; classification happens only in accepted-state evaluation. | Boundary treatment and phase switching both differ. Y2b1 must characterize this independently from `Swc`; this does not yet authorize G5. |
| Upper `Sw`/`Sg` / oil complement | With projection off, the normal update does not normalize `Sw+Sg+So`. The deck has no `VAPOIL`, so the oil-disappearance `Rv` route is unavailable; material laws protect properties at endpoints rather than imposing a residual-oil state bound. | `enforce_cell_bounds` caps `Sw` at the configured oil-floor complement and `Sg` at `1-Sw-Sorg`, retaining its imposed residual-oil margin. | A second state-policy divergence. Do not fold it into an `Swc` fix without fixtures that cover it. |

Source chain, in update order:

1. OPM registers and reads `DsMax` and `ProjectSaturations` in
   `OPM/opm-simulators/opm/models/blackoil/blackoilnewtonmethodparams.hpp:37-42` and
   `...params.cpp:35-65`; the actual Flow binary confirms those defaults.
2. `blackoilnewtonmethod.hpp:230-288` computes the implied `Sw`/`So`/`Sg` change and applies
   `dsMax`; `:452-458` adapts primary variables and only then conditionally calls
   `chopAndNormalizeSaturations`.
3. `blackoilintensivequantities.hh:248-290` places raw primary-variable saturations in the fluid
   state used by the residual. `EclMaterialLawManager.cpp:527-546` selects the Default
   three-phase law for this deck (no `STONE1`/`STONE2` keyword). `EclDefaultMaterial.hpp:375-424`
   clamps material-law endpoint use, and `PiecewiseLinearTwoPhaseMaterial.hpp:232-255` returns
   endpoint table values outside the tabulated range.
4. ResSim's OPM-style chop is `fim/newton.rs:2234-2298`; it is selected for `OpmAligned` at
   `:4125-4154`. `fim/state.rs:408-457` applies the raw cell update then always calls
   `enforce_cell_bounds`; `:250-280` defines the hard `Sw`, `Sg`, and residual-oil bounds.
   `fim/newton.rs:2746-2783` is the normal accepted-state regime reclassification path.

**Y2b1 authorization:** add test-only fixtures at `Swc-eps`, `Swc`, `Swc+eps`, `Sg=0±eps`, and
the upper oil-complement boundary. Record raw correction, post-chop candidate, post-bound state,
predicted residual change, and next residual. Keep Legacy and solver behavior unchanged.

### 15.2 Y2b1 result: projection breaks the exact Newton prediction (2026-07-13)

**Status: complete, diagnostic/test-only; no solver behavior changed.** `FimState` now exposes a
test-only unbounded update view and the exact first-rung driver emits `Y2B` raw/bounded candidate
and next-assembly lines under `FIM_Y2B_AUDIT=1`. A controlled one-cell gas-injector fixture covers
`bound-eps/bound/bound+eps` for `Swc`, `Sg=0`, upper `Sw`, and upper `Sg`, recording every
injector perforation and connected component row's residual, AD/legacy Jacobian entries, and
forward/backward/central finite differences.

At `Swc`, the expected discontinuity is explicit: AD selects the active above-bound derivative;
the forward FD agrees, backward FD is the endpoint branch, and central FD lies between them. This
does not authorize derivative averaging. The exact 10x10x3 trace instead supplies the causal
measurement: at `dt=9.78384825e-4`, iteration 5, the chopped correction moves cell 0 from
`Sw=0.15` to raw `0.1499514237572`; ResSim replaces it with `0.15`. The water residual is
`4.8591979e-3` before the step, predicted `-8.53e-11`, and raw next `-8.53e-11`, but bounded next
is `4.8591978e-3`. Oil/gas and rate-consistency rows also receive a large projection effect. A
forced-direct run produces the same correction, raw water closure, and restored bounded residual.

**Decision:** the hard bound is a direct first-order inconsistency in the exact failure path and
OPM leaves this deck unprojected. Authorize the narrow Y2b2 `OpmAligned`-only/default-off
bound-policy probe. Do not alter generic derivative rules, acceptance, or G4/G5 structure.

### 15.3 Y2b2 correction: result is masked by an invalid direct oracle (2026-07-13)

**Status: INCONCLUSIVE; implementation removed pending oracle repair.** The one authorized probe retained raw saturation
primary variables after the existing per-cell `ds-max` chop, only under a native,
`OpmAligned`-only default-off flag; pressure and well bounds remained active. Live first-rung
result improved from `dt=0.000978384825`, five nonlinear retries, to `0.00898425`, three
linear-classified retries. Forced-direct accepted no substep and exhausted 16 linear-classified
retries. Both backends match through the first three cutbacks; at `dt=0.00898425`, live takes a
third iteration and accepts while direct stops at iteration two with a well-row failure. The flag
and all behavior code were deleted.

The original verdict treated that direct/live disagreement as an independent refutation. A
follow-up audit shows the gate compared report semantics, not measured correction quality. At the
decision point both paths have reached the same iteration-1 state. Live CPR reports a finite,
non-strict solve plus `rhs_norm`, allowing Newton to compute reduction `5.299e-3 < 1e-2`, apply
the relaxed-accepted correction, and reach strict nonlinear convergence on iteration 2. Sparse LU
also returns a finite solution, but `sparse_lu_debug.rs` always sets `failure_diagnostics=None`.
`well_schur.rs` forwards that absence; `newton.rs` derives relaxed reduction only through the
optional failure payload, so direct emits `reduction=n/a` and aborts without measuring or applying
its correction. This is the same missing-diagnostics failure shape already documented in §8.4.
At later direct retries, nonlinear convergence is already `would_accept=strict` before the
mandatory solve, yet the same `reduction=n/a` path forces another retry.

Therefore the current evidence does **not** show that the direct correction is poor, and cannot
refute the state policy. The 9.2x live improvement remains provisional positive evidence; it is
not promotable until direct correction quality and full-system residual reduction are measured.

The probe also did not implement the complete OPM lifecycle established in §15.1. It removed the
post-update saturation projection but retained ResSim's frozen Newton regime and flash-side
hydrocarbon clamps, without OPM's per-update `adaptPrimaryVariables`. It is a valid narrow `Swc`
mechanism probe, not a coherent test of raw state plus endpoint properties plus variable
adaptation. Its result cannot refute that larger coupled mechanism.

### 15.4 Y2b2a complete: backend-neutral linear oracle before G4

The next authorized slice is measurement infrastructure, not well restructuring. Y2b2a makes
every linear report expose original full-system RHS norm, recovered full-system residual norm,
and reduction independently of optional backend failure details. It makes the OPM relaxed-linear
check consume that backend-neutral value, adds focused Sparse LU/well-Schur/acceptance contract
tests, and proves the existing forced-direct trace has no finite `reduction=n/a` result. Commit
that oracle change before restoring behavior. This was completed on 2026-07-13: reports carry
original RHS/final-residual norms and derive reduction from them; Sparse LU no longer needs a
failure payload to expose a finite reduction; recovered well-Schur reports use original-system
norms; and relaxed `OpmAligned` acceptance uses this common reduction. The focused contracts,
AD assembly parity, and direct well-elimination test passed. The raw-state probe remained absent
for that checkpoint.

Y2b2b restored that same narrow probe and captured its exact `dt=0.00898425`, iteration-1 system.
CPR replay yields a finite `4.830552e-3` full-system reduction, while explicit Sparse LU returns
an all-zero correction and reduction `1.0`. The forced-direct live case consequently takes 16
linear retries and accepts no substep. This is not itself evidence against raw state and not a
well-Schur result (explicit direct bypasses Schur elimination).

### 15.5 Y2b2c closeout: raw-state capture is rank-deficient (2026-07-14)

The preserved `904x904` live capture was replayed through test-only structural diagnostics.
Conversion to faer's sparse matrix succeeds, but `sp_lu()` factorization fails. The matrix has no
empty rows, duplicates, non-finite entries, or all-zero rows; it has 120 empty columns and the
same 120 missing/zero diagonal candidates. Every one is cell-local primary variable 2 (the gas
component), not a well or perforation unknown. Ordinary dense LU independently rejects the matrix.
A rank-revealing dense SVD is the viable independent direct oracle: rank `784`, relative full
residual `1.101044e-12`, and maximum correction difference from CPR `1.657241e-2`.

Thus CPR's correction is not contradicted by an independent direct replay; the all-zero LU result
is the expected response to a compatible but rank-deficient raw-state system. This is new direct
evidence that retaining raw state without OPM's per-iteration phase-presence/primary-variable
adaptation leaves inactive gas unknowns in ResSim's fixed 3-variable layout. It does not yet prove
the complete OPM lifecycle remedy. Y2b remains inconclusive; G4/G5, acceptance changes, and Y2c
are blocked until that lifecycle is source-scoped as one coupled behavior bundle. Full commands
and artifacts: `FIM_OPM_CONVERGENCE_EXECUTION_PLAN.md` §§5.2–5.3.

### 15.6 Y2b3 source/design closeout: switch meaning, not matrix shape (2026-07-14)

The source-complete lifecycle and ResSim dependency contract are recorded in
`FIM_Y2B3_PRIMARY_VARIABLE_LIFECYCLE_DESIGN.md`. For this `DISGAS`, no-`VAPOIL` deck, OPM retains
one composition-switch slot per cell and changes its meaning after each Newton update: negative
raw `Sg` switches to saturated `Rs`, and over-saturated `Rs` switches to `Sg=0`. The next assembly
uses the adapted meaning, with per-cell hysteresis after a switch. ResSim already has the same
fixed slot plus a `Saturated`/`Undersaturated` tag, so G5-style matrix restructuring is not the
required first move; the missing behavior is the in-loop atomic tag/value adaptation.

The next implementation is constrained by a structural gate: no fixed-layout cell-primary column
may be empty. An empty column must abort the probe with cell/tag/raw/derived/switch diagnostics;
it must never be hidden with a diagonal or tolerated because CPR returns a correction. Transition
and derivative/structure tests precede regeneration of the exact Y2b2 capture. This completes the
requested design prerequisites only; Y2b remains inconclusive and Y2c remains blocked.

### 15.7 Y2b3a result: primary-variable transition state machine (2026-07-14)

The native/default-off Y2b flag now applies the deck-scoped `Sg <-> Rs` state machine after the
meaning-aware Newton chop and before ResSim well post-processing. Switch memory is local to the
Newton solve, matching OPM's ownership: a switch on the preceding candidate supplies `eps=1e-5`,
otherwise `eps=0`. OPM's initialization values are retained exactly within ResSim's available
deck semantics: `Sg -> min(RsMax,RsSat)` and `Rs -> Sg=0`. The previous accepted state is not
retagged or mutated.

Five transition tests pass, but this is not yet a behavior result. The hysteresis test directly
constructs the important remaining Gate B state: a recently switched cell can keep
`Sg=-5e-6` as `Sg`. ResSim's current AD property path floors that value at zero and can discard
the active derivative, unlike OPM's raw accumulation state. Gate B must close that dependency and
prove zero empty columns before the exact first-rung run. Y2b remains inconclusive.

### 15.8 Y2b3b result: raw tagged dependency and structural gate (2026-07-14)

The three-phase `Sg`/`Rs` slot now stays raw through phase-state construction and accumulation;
endpoint extension remains in material-property evaluation. The two-phase and no-PVT paths are
unchanged. Scalar and AD paths were updated together, and within-meaning one-sided finite
differences explicitly cover hysteresis-retained negative `Sg`, newly appeared gas, and
sub-saturated/newly initialized `Rs`.

Both one-cell switch directions retain a live third column after adaptation. A mixed-regime
three-cell gas-injector fixture, including `Sg=-5e-6`, has finite entries, no empty rows or columns,
scalar/AD reservoir-plus-well residual parity, and successful independent Sparse-LU
factorization. This closes the local dependency/structure prerequisite, not the nonlinear
hypothesis: Y2b remains inconclusive. The next bounded step is Gate C's traced exact first-rung
capture and backend-neutral correction/reduction comparison; Y2c remains blocked.

### 15.9 Y2b3c result: exact capture is full-rank; first rung reaches OPM scale (2026-07-14)

The completed lifecycle accepts the full `0.25` day capped first rung on clean commit `1a6460d`
with zero retries and 8 reported residual evaluations (7 applied updates); Flow applies 7
updates and records 8 `INFOITER` evaluations. This supersedes the old
five-retry `0.000978384825` baseline and the incomplete raw probe's three-retry `0.00898425`
result for this scoped behavior comparison.

Because the historical decision rung is no longer visited, a test-only exact-dt selector
regenerated its iteration-1 system directly. The trace maps all 300 fixed third columns to their
adapted meaning (141 `Sg`, 159 `Rs`, with 159 preceding switches); every column is live. The
904-by-904 matrix has 6815 nonzeros and no empty/zero-pivot candidates. Sparse and dense LU both
factor and agree to `7.29e-15`; CPR's correction differs by at most `5.56e-7` and all three expose
comparable full-system norms. The full-rung accepted MB is `1.683337e-8`.

This is the first valid positive Y2 behavior result: Y2b becomes a promotion candidate. It is not
promoted until Y2c reproduces the complete six-step target, Flow oracle, heavy case, controls, and
physics gates on the final committed revision.

### 15.10 Y2c result: gas parity reached, control gate blocks promotion (2026-07-14)

On the complete six-step gas target the lifecycle candidate accepts exactly one substep per report
step with residual-evaluation counts `8,5,4,4,4,4` (applied updates `7,4,3,3,3,3`) and zero
retries. Fresh Flow 2026.04 applies `7,5,4,3,4,3` updates and records one extra `INFOITER`
evaluation per step, also with one substep per report step. Legacy requires 14 accepted substeps. On a
larger `20x20x3` gas first step the candidate needs one substep/8 Newton, versus Legacy's two and
baseline `OpmAligned`'s 238. This validates the lifecycle as a real OPM-alignment mechanism rather
than another favorable single-rung artifact.

The bounded controls prevent promotion:

| water control | Legacy | baseline `OpmAligned` | lifecycle candidate |
| --- | ---: | ---: | ---: |
| `20x20x3` | 8 | 24 | 5 |
| `22x22x1` | **4** | 24 | **11** (8 linear retries) |
| `23x23x1` | 4 | 12 | 3 |

The heavy `12x12x3` first step also remains 7 substeps versus Flow's one. Physics checks do not
show a compensating invalid state: saturation closure is `<=2.22e-16`, all reported values are
finite, and coarse accepted production/inventory stays close to a 4x finer timestep reference
(rates within `0.036%`, gas inventory `0.228%`, injection `0.876%`). The required focused, locked,
Buckley-Leverett, and curated FIM gates pass; shared stops at the unchanged known closed-system
history mismatch.

Verdict: **VALIDATED POSITIVE, DEFAULT-OFF**. The result is closer to Flow on the primary target
but is not a stack promotion. The next bounded question is the first `22x22x1` candidate
`linear-bad` retry, using common full-system norms and an independent direct replay. This directly
tests the promotion blocker while holding the proven lifecycle fixed; it does not authorize G4,
acceptance widening, controller tuning, or a Sparse-LU implementation project.

### 15.11 Y2d0 result: bounded promotion blocker is a real CPR quality gap (2026-07-14)

Clean commit `2030996` reproduced the lifecycle candidate's 11 substeps and eight linear retries
on `22x22x1`. Failure-only capture produced exactly eight artifacts. The first is a 1456-row,
4764-nonzero iteration-0 `max-iters` system (SHA-256
`725cbbc2cc06f1d31ef090c7b7f11e6374ce7b70a3feaf06ccc90f18309e786b`) with no empty,
duplicate, non-finite, all-zero, or zero-pivot-candidate structure; Sparse LU factorizes normally.

The common full-system replay is decisive. At `rhs_norm=2.541597987e3`, production-faithful CPR
reproduces the live 30-iteration failure with a finite correction and reduction
`1.441123105e-2`. Sparse LU returns a finite correction in one iteration with reduction
`5.546336962e-15`. Reported final norms equal independently recomputed `||rhs-Jdx||`; CPR's
`36.62755584` residual is wholly in reservoir rows, while the direct well-row residual is
`2.60e-14`. The corrections materially disagree: maximum pressure delta `181.498 bar`, water
saturation delta `0.351096`, and perforation-rate delta `1550.931 m3/day`. Disabling well Schur
still fails at 30 iterations, so recovery/wrapper semantics alone do not explain the gap.

Verdict: **Y2d0 CONFIRMED**. This first retry is not a lifecycle refutation, singular matrix,
factorization failure, missing report metric, or nonlinear acceptance artifact. It is a genuine
iterative correction-quality failure.

Existing restriction-variant output supplies only a next clue: on all eight frozen failures,
`row0-schur` and `local-schur-balanced` converge 4/8 while current `quasi-impes` converges 0/8.
That helper bypasses production well-Schur elimination and captured equation scaling, and the old
gas corpus previously favored quasi-IMPES 336/337. Therefore no restriction flip is authorized.
Y2d1 must first make the variant replay production-faithful and require the gas corpus as a
counter-control.

### 15.12 Y2d1 result: pressure restriction is corpus-sensitive, not a global fix (2026-07-14)

Y2d1 added test-only restriction injection through the exact production well-Schur reduction and
recovery. The injected quasi-IMPES path matches ordinary production dispatch exactly, including
captured equation scaling, block-ILU0, tolerance, iteration budget, full-system norms, and returned
correction. Sparse LU solves every artifact, and each iterative report's final norm matches an
independent `||rhs-Jdx||` calculation.

On the eight bounded failures, production quasi-IMPES solves `0/8` strictly and `0/8` under the
relaxed `1e-2` gate. Diagonal-balanced summed rows solve `8/8`, improve median reduction from
`1.455e-2` to `1.340e-3`, and reduce the median maximum correction-family disagreement from
`508.9` to `4.23`. This proves genuine pressure-restriction sensitivity.

It is not a promotion result. The historical 337-system gas corpus no longer exists and predates
later capture/report fixes, so clean commit `e143c19` regenerated the current `20x20x3` baseline:
238 accepted substeps, one linear plus four nonlinear retries, and five captured near-miss/failure
systems. On this valid current counter-control, quasi-IMPES is usable on `4/5` with median
reduction `3.646e-5`; diagonal-balanced is usable on only `3/5`, with two severe misses. Summed
rows and dominant-diagonal are gas `5/5`, but resolve only `2/8` and `0/8` bounded systems.

Verdict: **DIAGNOSTIC TRADEOFF; NO UNIVERSAL RESTRICTION AND NO LIVE CHANGE**. Restriction is a
real contributor to the bounded failures, but no existing choice clears both corpora. A
case-adaptive selector would encode the symptom rather than explain it and is not authorized.
Y2d2 holds quasi-IMPES fixed and isolates fine smoother versus Krylov budget using the same
production-faithful oracle.

### 15.13 Y2d2 result: one post-restart correction closes every hard replay (2026-07-14)

Y2d2 held quasi-IMPES restriction, production well Schur elimination/recovery, equation scaling,
tolerance, and restart fixed. At effective budget 30, block-ILU0 remains the best available fine
smoother: it resolves bounded `0/8` and gas `4/5`; full-ILU0 and block Jacobi also resolve `0/8`
and `4/5` but worsen median reductions on both corpora. The latter two are bit-identical here.

With production block-ILU0 fixed, every bounded system stops at iteration 30 with median full
reduction `1.455e-2`, then passes at iteration 31 or 32 with median `2.201e-6`. The sole hard gas
system similarly moves from reduction `4.897e-2` at iteration 30 to `3.440e-5` at iteration 31,
raising gas coverage from `4/5` to `5/5`. Effective budgets 60 and 150 are bit-identical for all
13 systems, proving that the extra budget is not hiding slow unbounded convergence. Residuals and
corrections remain finite; full norms, partitions, and independent residual calculations agree.

Verdict: **OFFLINE KRYLOV TRUNCATION CONFIRMED; NO LIVE OR PRODUCTION CHANGE**. This is closer to
the bounded direct oracle in result, but raising ResSim's cap would move its approach farther from
Flow, whose reference solve stays within 20 iterations. Y2d3 therefore records the exact
iteration-29-to-32 convergence history and audits restart/budget accounting before choosing
between a bookkeeping correction and CPR coarse-stage quality work.

### 15.14 Y2d3 result: the FGMRES label hides a fixed-left recurrence (2026-07-15)

Test-only recording captured every completed Krylov candidate through the production well-Schur
and scaling wrapper. Iteration 30 exactly reproduces the production correction, so the budget is
not off by one. Across all eight bounded failures the true residual remains near its
iteration-one `1e-2` level through the first cycle even as the Givens estimate collapses toward
machine precision; the median estimated-versus-actual preconditioned residual disagreement at
iteration 30 is `1.169223907e19`. Restart two supplies a genuinely new direction and reduces the
bounded true residual by median factor `1.293965443e-4`, converging at iteration 31 or 32. The hard
gas artifact moves from true reduction `4.896722176e-2` at 30 to `3.439884116e-5` at 31.

The coarse-stage controls identify why. Raising the diagnostic dense-pressure threshold from 300
to 512 gives the 484-row bounded systems an exact pressure inverse; all eight then converge in
one iteration, with median true reduction `3.224743915e-14` and median direct-correction delta
`2.601154847e-10`. This proves the captured matrices, restriction, well recovery, scaling, and
block-ILU0 composition can produce the direct answer. Conversely, keeping the iterative path but
tightening its relative stopping tolerance from `1e-6` to `1e-10` leaves every classification
unchanged: bounded is still `0/8` at 30 and the gas corpus `4/5`, with convergence only after the
same restart. Both temporary changes were restored.

The outer method constructs its basis from `M^-1 r`, applies `M^-1 A` to each basis vector, and
combines that basis into the solution. This is standard fixed left-preconditioned GMRES, not a
flexible recurrence. The >300-row CPR pressure application calls a tolerance-terminated iterative
BiCGSTAB solve, so repeated applications are input-dependent rather than one fixed linear map.
The Hessenberg/Givens estimate is therefore not the residual of the recomputed candidate. This
explains the immediate estimate/actual split, false first-cycle collapse, and restart reset
without blaming Newton acceptance or raising the iteration cap.

Verdict: **CLOSER IN APPROACH, PRODUCTION UNCHANGED**. Y2d3 replaces the generic “CPR quality”
diagnosis with a specific algorithm contract. Y2d4 is a test-only true right-preconditioned
FGMRES oracle: Arnoldi on `v_j`, store `z_j=M_j^-1v_j`, form `A z_j`, and update with `Zy`, while
holding every CPR component and the 30-iteration budget fixed. It must clear bounded `8/8` and gas
`5/5` within 30 and preserve the direct comparisons before any OPM source audit or live candidate.

### 15.15 Y2d4 result: true FGMRES closes both replay corpora (2026-07-15)

The Y2d4 oracle implements the actual flexible right-preconditioned recurrence: the Arnoldi basis
stays in raw residual space, every `z_j=M_j^-1v_j` is stored, the operator acts as `A z_j`, and
candidates use `Zy`. A two-dimensional nonlinear-preconditioner control first demonstrates that
the old fixed-left Givens estimate disagrees with an independently reapplied preconditioned
residual, while the flexible estimate agrees with its true raw residual and reaches the direct
solution. A separate fixed-linear-preconditioner control also matches direct.

All production CPR components remained fixed. Results:

| Corpus | Production | True FGMRES | Flexible iterations | Median true reduction | Median direct delta |
| --- | ---: | ---: | --- | ---: | ---: |
| bounded `22x22x1` artifacts | `0/8` | `8/8` | all `2` | `1.204527535e-8` | `1.641232493e-4` |
| current gas artifacts | `4/5` | `5/5` | `1-3` | `6.986957784e-5` | `1.871345444e-6` |

The hard gas artifact changes from production failure at 30/reduction `4.896722176e-2` to a
one-iteration pass at `3.813067988e-3`, direct delta `5.728602178e-6`. No existing production pass
is lost. Maximum relative disagreement between the flexible Givens estimate and independently
recomputed full residual is `1.031351975e-8` bounded and `8.472080369e-11` gas.

The actual Flow 2026.04 reference configuration is importantly different. Its preserved
`CASE.DBG` selects `bicgstab`, `maxiter=20`, `tol=0.005`, and `cprw` with true-IMPES weights,
`paroverilu0`, and a single AMG-backed coarse loop. OPM's generic `FlexibleSolver_impl.hpp`
supports `flexgmres` via Dune's `RestartedFlexibleGMResSolver`, but the exact reference run does
not select it. The result is therefore **a confirmed ResSim algorithm-contract fix, not literal
OPM linear-stack parity**.

Y2d5 integrates only the validated recurrence behind a default-off production option and runs
captured plus live gates while every CPR and nonlinear component remains fixed. Literal OPM
BiCGSTAB/true-IMPES/AMG parity stays separate so a successful FGMRES correction cannot mask those
remaining differences.

### 15.16 Y2d5 result: corrected recurrence unmasks the Y2 lifecycle win (2026-07-15)

The Y2d4 core is production-capable behind an explicit default-off `use_true_fgmres` option and
separate native/WASM diagnostic setter. Default dispatch remains the historical recurrence.
Synthetic dispatch/default controls pass, and promoted dispatch is solution/report-exact with the
Y2d4 oracle on all captures: bounded `8/8` in two iterations and gas `5/5` in one to three.

The live result establishes the masking interaction suspected at Y2c. Holding the complete Y2
primary-variable lifecycle and every CPR/nonlinear component fixed gives:

| Case | Y2 lifecycle before | Y2 + true FGMRES | Flow / control |
| --- | --- | --- | --- |
| water `22x22x1` | 11 substeps, `8L/0N` | **3**, `0L/1N`, Newton `11,5,6` | Legacy 4 |
| water `20x20x3` | 5, `1L/1N` | 5, `0L/2N`, Newton `9,5,5,5,4` | Legacy 8 |
| water `23x23x1` | 3, `1L/0N` | 3, `0L/1N`, Newton `11,5,6` | Legacy 4 |
| heavy water `12x12x3` | 7, `0L/1N` | 7, `0L/1N` | Flow 1/11 Newton |
| gas `10x10x3`, six steps | 6, zero, `8,5,4,4,4,4` | 6, zero, `9,6,5,5,4,4` | Flow 6, `7,5,4,3,4,3` |

Thus the old recurrence really did hide a positive lifecycle result: every Y2 water linear retry
disappears, and the promotion-blocking control becomes better than Legacy in substeps. The stricter
mathematically valid corrections then expose nonlinear work rather than eliminating it. Exact gas
and two water controls require modestly more accepted Newton iterations, so the combined result is
not uniformly closer to Flow and does not authorize a default switch.

Legacy-flavor candidate controls preserve substeps/retries (`20x20x3` water 8/3N,
`22x22x1` 4/2N, `23x23x1` 4/2N, gas `20x20x3` 2/1N, gas `10x10x3` 14 total); heavy improves from
the documented Legacy 25 to 21 substeps. The option is retained as a validated default-off ResSim
correctness path. Y2d6 must now design the actual Flow-selected outer BiCGSTAB, true-IMPES CPRW
weights, paroverilu0, and one-loop AMG application as one source-complete lifecycle. Swapping only
the outer solver while retaining ResSim's input-dependent inner solve would be an invalid oracle,
not a refutation.

### 15.17 Y2d6 result: source-complete Flow lifecycle exposes a well-operator split (2026-07-15)

The design is pinned to the exact `release/2026.04/final` OPM commit
`b82f21dba405286c4c4446614dd3bf9cdebf7a2c` and DUNE-ISTL 2.11.0, matching the installed
Flow 2026.04 package rather than the newer local `master`. Outer BiCGSTAB uses the raw sequential
two-norm, strict `0.005` reduction, twenty complete alpha/omega pairs, and a cleared output before
each right-preconditioner application. The CPR coarse loop applies AMG exactly once; its `0.1`
tolerance does not turn it into a variable tolerance-terminated inner solve.

Two dependencies invalidate a simple outer-method replacement. True-IMPES weights come from
local storage derivatives, which the existing 13 matrix/RHS captures do not retain. More
importantly, Flow's outer operator applies eliminated standard-well effects while its fine
`paroverilu0` factors the reservoir `J_rr` matrix without those effects; CPRW adds well pressure
contributions separately to its coarse matrix. ResSim currently forms the explicit Schur matrix
first and uses it for both fine and coarse CPR construction. That difference can mask either a
positive or negative result even with the same outer recurrence.

The complete component identities, matched/missing map, capture contract, and four-stage gates are
in `docs/FIM_Y2D6_FLOW_LINEAR_LIFECYCLE_DESIGN.md`. The next slice is only Y2d6a: capture raw
storage/true-IMPES inputs and separate reservoir/well blocks, then prove one bounded and one gas
artifact. No outer solver or AMG implementation is yet authorized.

The parallel IMPES audit found no CPR/Newton mechanism to port. IMPES has no coupled Newton
primary-variable or well-tail system, defaults to direct pressure LU, and uses fixed diagonal
Jacobi only in its BiCGSTAB fallback. One genuine reporting defect was found and fixed: convergence
recognized at a loop boundary previously counted the not-yet-executed next iteration. A focused
nonsymmetric regression now reports completed corrections and independently checks the raw
residual; the pressure solution is unchanged.

### 15.18 Y2d6a result: capture payload is sufficient on bounded and gas systems (2026-07-15)

Capture format v3 now records the exact unscaled full Jacobian/RHS/layout/scaling together with
the unscaled per-cell accumulation derivative blocks, normalized Flow true-IMPES weights, and
`J_rr/J_rw/J_wr/J_ww` before well elimination. The native trigger claims only the first system and
does not alter production routing. Its parser fails closed on source/config or weight mismatch and
requires the partitions to reconstruct the full Jacobian bit-for-bit.

The bounded `22x22x1` proof has `1456` rows, 484 cells, four well rows, full nnz `4764`, and
partition nnz `[4752,2,4,6]`. The exact gas `10x10x3` proof has `904` rows, 300 cells, four well
rows, full nnz `5372`, and `[5360,3,2,7]`. Both recompute a normalized maximum absolute weight of
one using the pinned 50-bar pressure scale. This closes the previously identified invalid-oracle
gap; it says nothing yet about convergence. Next is only Y2d6b's seven component identities.

### 15.19 Y2d6b result: separated Flow components satisfy all algebraic gates (2026-07-15)

The test-only lifecycle oracle now applies standard-well elimination matrix-free while retaining
`J_rr` as the fine-smoother matrix. On both v3 artifacts it proves: captured true-IMPES
recomputation; matrix-free/explicit-Schur equality; reservoir plus exactly one well coarse term;
fixed block-ILU0 on `J_rr`; fixed one-application coarse solve; fixed zero-pre/coarse/one-post CPR
order; and an independent raw outer residual norm.

The bounded identity metrics are outer/coarse disagreements `0/0`, well coarse norm
`1.268891369e1`, fine/coarse/CPR linearity `2.10e-16/9.66e-15/1.97e-15`, and residual-norm
disagreement `1.89e-16`. Gas gives `8.16e-19/5.87e-18`, nonzero well norm `1.355152165e-12`,
linearity `1.30e-16/2.03e-15/4.42e-16`, and zero residual-norm disagreement.

This also narrows the AMG scope using DUNE 2.11 source: 484 and 300 pressure rows are below
`coarsenTarget=1200`, so neither proof artifact aggregates. The sequential direct coarse solver is
the entire one-level AMG application and is a fixed map; no general AMG project is justified.
Y2d6b remains algebraic infrastructure, not evidence of fewer Newton iterations. Next is Y2d6c's
identity-gated 8+5 captured comparison under the exact 20-pair BiCGSTAB contract.

### 15.20 Y2d6c step 1: source-complete 8+5 corpus regenerated (2026-07-15)

The failure/near-miss selectors now have a separate capture-v3 output, leaving v2 diagnostics and
the D6a one-shot trigger unchanged. The current bounded run reproduces 11 accepted substeps and
eight linear retries, writing exactly eight `max-iters` systems. The current 20x20x3 gas control
reproduces 238 substeps, one linear and four nonlinear retries, writing exactly four final near
misses and one `max-iters` system. All thirteen parse with the required lifecycle companion,
recomputed weights, and bit-exact full-J partitions. No recurrence result exists yet.

### 15.21 Y2d6c result: coherent Flow lifecycle clears the 8+5 offline gate (2026-07-15)

The test-only outer solve directly transcribes DUNE 2.11 BiCGSTAB: zero initial update, raw
sequential norm, strict `<0.005`, matching breakdown guards, half-step checks, and twenty complete
pairs maximum. Each artifact first passes all seven D6b identities. Recovered full residuals and
reservoir/well partitions are independent; Sparse LU supplies correction deltas but is not trusted
by backend status alone.

| Corpus | Production | true FGMRES | Flow lifecycle | Max complete pairs | Median full reduction | Median direct delta |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| bounded 8 | `0/8` | `8/8` | **`8/8`** | `1` | `1.977026308e-13` | `1.417902240e-9` |
| gas 5 | `4/5` | `5/5` | **`5/5`** | `1` | `3.813935911e-4` | `2.617263521e-6` |

All solutions are finite, no recurrence breaks down, and no production pass is lost. Two late
bounded systems stop validly at Flow's loose criterion with full reductions `1.32e-3` and
`2.25e-4`; their larger direct deltas remain explicit rather than being called direct-equivalent.
This authorizes only D6d's default-off live experiment. It does not yet establish fewer Newton
iterations.

### 15.22 Y2d6d result: offline linear success does not close the live nonlinear gap (2026-07-15)

The complete source-pinned lifecycle is integrated atomically behind native-only
`FIM_FLOW_LIFECYCLE=1`, default false. The live coarse matrix is assembled directly as
`R J_rr P - R J_rw J_ww^-1 J_wr P`; the dense reservoir Schur matrix remains test-only. A coupled
regression proves the live construction and independent explicit-Schur oracle return the same
BiCGSTAB correction and residual.

| Case | Previous Y2 path | Flow lifecycle live | Flow / Legacy control |
| --- | --- | --- | --- |
| exact gas, six steps | 6, evals `8,5,4,4,4,4`; updates `7,4,3,3,3,3` | 6, evals `10,5,5,4,4,4`; updates `9,4,4,3,3,3` | Flow 6, updates `7,5,4,3,4,3` |
| heavy water | 7, `0L/1N` | 7, `0L/0N/1 mixed` | Flow 1/11 Newton |
| Y2 water `22x22x1` | 3 | 3, `11,4,6`, one mixed retry | Legacy 4 |
| Y2 water `23x23x1` | 3 | 3, `10,5,5`, one mixed retry | Legacy 4 |
| Legacy exact gas | 14 total | 14 total | guard unchanged |

The result is neither an invalid-oracle refutation nor a promotion: D6a-c proved the linear
mechanism, while D6d proves that mechanism is not the missing live convergence factor. Y2d7
corrected the accounting: D6d matches Flow's total 26 applied updates, but its per-step L1 distance
from Flow is 4 versus 3 for the previous path. Heavy fragmentation is unchanged. Keep the bounded
native path only as diagnostic infrastructure; do not tune BiCGSTAB/CPR or start Y3 on a gas case
with no cuts.

### 15.23 Y2d7 result: first divergence selects injector source formulation (2026-07-15)

Fresh Flow output at `/tmp/ressim-y2d6e-40f366d/opm` and native traces beside it establish a
source-comparable initial anchor. Flow/ResSim evaluation-0 oil CNV/MB are both
`0.5109/1.667e-3`; gas CNV is `1.2457/1.245`, and gas MB is
`3.5069e-3/3.470e-3`. Flow `INFOSTEP.NewtIt` counts applied updates while ResSim's report counts
the entry residual evaluation that observes convergence. Therefore the comparable update
sequences are Flow `7,5,4,3,4,3`, Y2 `7,4,3,3,3,3`, and D6d `9,4,4,3,3,3`.

The first update is the first material divergence. Flow evaluation 1 keeps the well converged and
has oil MB `1.8375e-3`. Default ResSim changes the satisfied injector rate from `-500` to
`-526.85`, creates well/perforation residuals `5.75e-2/1.70e-2`, and reaches oil MB
`4.311e-3`, binding at injector cell 0. The existing nested-well path holds `q=-500` and removes
the well-row residual, yet oil MB remains `4.311e-3`. Thus the post-update well relaxation is a
real secondary defect, but it does not explain the reservoir component imbalance.

Verdict: **DIAGNOSTIC; G4 AUTHORIZED.** Compare the injector-cell component source and rate/unit
conversion after the first `ds-max` update with Flow `StandardWell`, holding nested well solve and
the Y2 primary lifecycle fixed. Do not alter G5 switching, convergence acceptance, timestep
control, or the closed linear lifecycle. The binding cell is saturated and retains `Sg`, so G5 is
not the first evidenced branch. IMPES has no nonlinear well-tail solve, but any shared component
source/rate conversion defect found in G4 must be audited in its shared well physics.

### 15.24 Y2d8/G4 result: Flow freezes RESV conversion within the report step (2026-07-15)

The traced deck is a gas `RESV` injector at `500 m3/d`, not a surface-rate case. Flow's
`WellAssemble::assembleControlEqInj` calls `RateConverter::calcInjCoeff`; `RateConverter` builds
the hydrocarbon-pore-volume-weighted regional state at `beginReportStep` and refreshes it after
an accepted timestep. Thus report-step-1 `B_g=0.0065` gives the fixed RESV conversion: a
converged `500 m3/d` control maps to `-500/B_g=-76,923.077 Sm3/d`. `StandardWell`'s
perforation assembly then supplies its surface component rate to the reservoir source equations;
Flow's evaluation-1 `WellStatus=CONV` makes this a comparable controlled state.

The new observation-only `WELLSOURCE` trace takes its values from ResSim's live production source
helper. At evaluation 0 it agrees exactly: `q=-500`, `B_g=0.0065`, source `-76,923.077` Sm3/d.
At evaluation 1, cell 0 is `p=242.679`, `B_g=0.005219627`. Nested solve still holds `q=-500`,
but ResSim recomputes source as `q/B_g=-95,792.278` Sm3/d, `24.5%` above Flow (`-4,717.300` in
the 0.25-day residual row). The default relaxation adds its independent q drift and produces
`-100,936.942` Sm3/d.

This **CONFIRMS** the source-formulation mechanism, but is not a license to freeze a single term:
Flow's surface component-rate primary, report-step coefficient, RESV control, connection law,
and reservoir source form one lifecycle. ResSim's q is currently a local reservoir connection
rate. A source-only freeze would deliberately make its three rows inconsistent. Next is G4a's
coherent default-off design/oracle; G5 and solver-policy levers remain held fixed.

### 15.25 G4a result: required single-perf surface-rate lifecycle (2026-07-15)

The prescriptive design is `docs/FIM_G4_INJECTOR_RESV_LIFECYCLE_DESIGN.md`. For the one-perf gas
RESV injector, the Flow-compatible primary is positive surface rate `u`, with
`R_perf=-q_res/B_g(cell)-u`, `R_ctrl=B_g,ref*u-Q_resv`, and source `q_res/B_g(cell)`.
`B_g,ref` is report-step regional; current `B_g(cell)` remains in connection and source
derivatives. At evaluation 1 the converged perforation makes source `-u=-76,923.076923 Sm3/day`
even while local `B_g=0.005219627384`.

The source, control, connection, primary update, diagnostics, AD/legacy parity, and local-solve
coordinates are coupled. q-coordinate nested solve, multi-perf allocation, BHP active switching,
retry lifetime, and IMPES are explicitly outside G4b0. The next commit is context/control
representation plus unit gates only; an assembly or convergence run before those gates is invalid.

### 15.26 G4b0 result: report-step context exists but is intentionally inert (2026-07-15)

G4b0 adds `WellScheduleControl::Resv`, the native-only `FIM_FLOW_RESV_INJECTOR` flag, and
`FlowResvReportStepContext`. It captures Flow-style hydrocarbon-PV-weighted pressure and derives
the one-region `B_g,ref` before a report step; retries retain that immutable value and acceptance
refreshes it. Strict guards reject a q-coordinate nested solve and explicit BHP limit, as well as
unsupported gas/control/topology forms. Neither FIM assembler consumes the context yet, so this
does not alter a primary, source, connection, control row, or IMPES.

The next valid claim is only G4b1's AD/f64 local residual contract. A live result or source trace
under this flag would still be an incomplete lifecycle and is not a parity oracle.

### 15.27 G4b1 result: local current-FVF residual/source contract (2026-07-15)

The generic `flow_resv_injector_residual<S: Scalar>` is evaluated with both plain `f64` and
`Ad<N>` inputs. Its tests at two pressures use distinct current `B_g` values from the frozen
`B_g,ref`, establish the source and perforation pressure derivatives from the same current
connection/FVF expression, prove source's u column is zero, and prove the control u column is
exactly `B_g,ref`; a central finite difference agrees away from any clamp. This helper is inert:
neither legacy nor AD assembly routes through it, and it does not yet represent the complete Flow
well lifecycle.

G4b2 is a readiness audit, not permission to alter one assembler: list and gate every coupled
primary/source/control/connection/update/scaling/diagnostic/reporting route first.

### 15.28 G4b2 result: prevent BHP/q fall-through before atomic routing (2026-07-15)

The readiness audit found a concrete incomplete-lifecycle hazard: recognized `RESV` is not a
`rate` mode in current `physical_well_control`, so it would select the old BHP/q formulation if
allowed into Newton. The native flag is now deliberately non-executable after successful context
capture; a valid fixture proves it returns before time advancement. This is a safety correction,
not a convergence result.

The required complete route is recorded in
`docs/FIM_G4B2_ATOMIC_ROUTE_READINESS_AUDIT.md`: explicit u primary/control metadata, shared
AD+legacy residual/Jacobian/source route, q-relax exclusion, scaling/Schur compatibility, and
source-complete diagnostics/reporting. G4b2a must design those interfaces and gates before the
block can be removed.

### 15.29 G4b2a result: exact typed-primary atomic route and gates (2026-07-15)

`docs/FIM_G4B2A_ATOMIC_ROUTE_IMPLEMENTATION_DESIGN.md` now specifies the complete one-perf
implementation. The selected tail value becomes `FimPerforationPrimary::FlowResvGasSurfaceU`,
not a q-named `f64` carrying a different unit. The selected BHP/u tail remains the same size and
offsets, but initialization solves `q_res=-Q_resv` and sets `u=Q_resv/B_g,ref`. Both assemblers
must call G4b1's shared residual-value helper and scatter the same gas source, perforation, and
control rows; legacy retains independent analytic derivatives for `-q_res/B_g(current)`.

The source has no u column before the perforation equation converges, while the control has no
BHP/cell column. q relaxation, q reporting, FB control, and nested q solve are forbidden for the
selected route. Full AD/legacy/FD, scaling, Schur, and evaluation-0/1 trace gates precede any
live test. This is a design result, not Flow trajectory evidence; the execution block remains.

### 15.30 G4b2b0 result: assembler plumbing confirms scope but not lifecycle completion

The selected route is now available to both FIM assemblers in a focused test fixture, with the
G4b1 residual bundle, u-column FD, and matrix parity. It remains structurally blocked in the
timestep. The temporary state representation still stores u in the historical q-named slot, and
the legacy Jacobian has not yet gained its independent analytic source linearization; therefore
this does not satisfy §15.29 and cannot be used for a Flow comparison. Next is the typed-state
and oracle closeout, not a live run.

### 15.31 G4b2b result: atomic typed RESV lifecycle reaches production Newton

The selected state slot is now physically typed surface `u`; historical q-only consumers cannot
read it. The selected AD route and an independent analytic legacy route scatter the same current-
`B_g` gas source/connection and frozen-`B_g,ref` control, with complete central-FD coverage and
exact direct/Schur correction equality. Selected updates bypass historical FB/q postprocessing,
and all residual-only reassembly, diagnostics, and reporting sites preserve or derive the route's
actual meaning. The former pre-Newton block is removed behind the default-off native flag, and a
valid fixture completes the OpmAligned timestep path.

This closes implementation completeness only. It does not upgrade the earlier Flow comparison:
the exact `gas-rate-10x10x3` first report step must now be replayed from the committed tree with
evaluation-0/1 fields and no retry. A retry, missing trace field, or route fall-through is
`INCONCLUSIVE`, not a refutation of the coherent Flow lifecycle and not authorization to tune
Newton acceptance or timestep control.

### 15.32 G4b2b live oracle: headline parity unchanged; missing u inner solve is exposed

From committed `9cdff9b`, fresh Flow 2026.04 remains six uncut report steps with applied Newton
updates `7,5,4,3,4,3` and 27 total linear iterations. Holding the Y2 raw Sg/Rs lifecycle fixed,
the historical q and typed-u ResSim routes both remain six/zero with applied updates
`7,4,3,3,3,3`; typed u moves total Krylov iterations `61 -> 64`. There is no convergence-count
promotion to claim.

The evaluation-1 differential is more decisive than the headline. Flow reports `WellStatus=CONV`.
Typed ResSim has `u=76,923.077 Sm3/day` but current connection surface rate
`c_s=133,639.380`, leaving `R_perf=56,716.303`; its oil MB improves from historical
`4.311e-3` to `3.493e-3` against Flow `1.8375e-3`, while gas MB worsens from `2.482e-3` to
`4.526e-3` against Flow `2.8814e-3`. Because the connection row is not satisfied, the source
comparison is not a valid complete-lifecycle oracle. Verdict: **INCONCLUSIVE, not refuted**.
G4b3's u-coordinate inner well solve is the next dependency; acceptance, G5, controller, and
linear changes remain unauthorized.

Comparison scope remains narrower than the Flow deck's complete well lifecycle: G4b2b selects
only the injector, has no BHP switching, and leaves ResSim's producer historical. Grid, fluids,
targets, and report timing are mapped; full two-well formulation parity is still open.

### 15.33 G4b3 result: route-aware inner solve restores the comparable source state

Commit `4cdea38` adds the selected frozen-reservoir local system in `(bhp,u)` using the exact
global f64/AD residual evaluations. Local/global row-column agreement, perturbed convergence,
and mixed selected/historical well gates pass. The route-aware Newton update also preserves the
historical producer's Relax/NestedSolve behavior instead of bypassing all well post-processing.

The capped exact first step accepts `0.25 day` with no retry. At evaluation 1,
`u=c_s=76,923.07692`, `R_perf=-1.16e-10`, `R_ctrl=5.68e-14`, and the selected source is
`-76,923.07692`; G4b2c's `c_s/u=1.737` mismatch is gone. Gas MB moves from `4.526e-3` to
`1.737e-3` against Flow `2.8814e-3`, while oil MB remains `3.493e-3` against Flow
`1.8375e-3`. Seven applied updates match both Flow's first step and the prior route, so this is a
mechanism pass, not an iteration-count promotion. G4b4 may now run the six-step comparison with
all solver policy, BHP switching, multi-perf allocation, and G5 held fixed.
