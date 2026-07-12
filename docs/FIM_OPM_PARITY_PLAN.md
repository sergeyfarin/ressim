# FIM Bundle Y: OPM Convergence Parity (post-Bundle-X roadmap)

Status: PLANNED (2026-07-12). Registry: rows to be opened per checkpoint (`FIM-LINEAR-005` is
the standing OPEN row Y2 executes).
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

### Y1 — CPR pressure-restriction promotion (`FIM-LINEAR-005` → live, "Step 9.3")

The already-evidenced lever, executed per its own registry retry condition: promote
`sum-rows` or `quasi-impes` (pick by OPM fidelity — OPM's `cprw` uses quasi-IMPES weighting;
`docs/FIM_CPR_IMPROVEMENT_PLAN.md` has the design skeleton) to the live CPR path, flag-gated
through the standard offline-lab-first workflow (the 54+13-system corpora already exist;
re-capture on the post-X4 tree first since trajectories changed). Gates: offline lab
non-regression, then full control matrix + locked smoke + BL benchmarks. Success metric:
gas-rate `OpmAligned` 459 and `22x22x1`'s `linear-bad` retries move materially; heavy
iterations-per-substep drop toward single digits in the transient.

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
