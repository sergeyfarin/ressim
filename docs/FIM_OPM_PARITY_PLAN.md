# FIM Bundle Y: OPM Convergence Parity (post-Bundle-X roadmap)

Status: Y0 diagnostics complete (2026-07-12, see §6). **Y1 attempted 2026-07-12, redirected —
see §7: the literal action ("promote quasi-impes to live") was already done weeks ago and the
registry row describing it as OPEN was stale.** The real gap Y1 surfaced is that the offline
solver lab which justified that (and every other live linear-stack default) has never captured
an `OpmAligned` failure — it structurally cannot, today. Next step is not yet executed; see §7.5
for the options. Registry: rows to be opened per checkpoint.
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
