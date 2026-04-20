# FIM Bypass Trigger Audit (2026-04-19)

This doc captures the direct-bypass trigger audit follow-up from the
2026-04-18 cost profile and the 2026-04-19 Jacobian reuse revert. Its
purpose is to identify which `lin_ms` path actually costs what on the
medium-water and related shelves, and to lay out a two-slice narrowing
plan (Plan B: Lever 1 + Lever 3) for reducing that cost.

See also:
- `docs/FIM_JACOBIAN_REUSE_INVESTIGATION.md` — Stage 2 REVERTED (prior lever)
- `docs/FIM_CONVERGENCE_WORKLOG.md` — 2026-04-19 entry
- `project_fim_cost_profile_2026-04-18` memory — cost profile that
  motivated this audit (now updated with the audit's reframing)

## Probe design

A `BYPASS-AUDIT` per-Newton-iter trace line was added to `newton.rs`
(at the end of each Newton iter, after the fallback block):

```
BYPASS-AUDIT iter={n} cat={category} first_backend={kind} \
  first_ms={f64} fallback_ms={f64} used_fallback={bool} rows={n_rows}
```

Categorisation (priority order, matching the existing bypass-label
selector at newton.rs:2668-2677):
1. `dead-state` — `dead_state_direct_bypass` flag set pre-solve
2. `restart-stag` — `restart_stagnation_direct_bypass` flag set pre-solve
3. `repeated-zm` — `repeated_zero_move_direct_bypass` flag set pre-solve
4. `zm-fallback` — `zero_move_fallback_direct_bypass` flag set pre-solve
5. `post-fail-fallback` — no pre-bypass flag; first-solve iterative
   failed and fallback sparse-lu fired
6. `near-converged-accept` — no pre-bypass flag; first-solve iterative
   failed but `should_accept_near_converged_iterative_step` accepted it
   (no sparse-lu rescue)
7. `clean` — no pre-bypass flag; first-solve iterative converged

Per-iter `first_ms` captures the first-solve wall time; `fallback_ms`
captures the post-failure sparse-lu rescue cost (0 when no fallback).
Aggregation verified: probe total_ms == step-reported lin_ms on every
case (no blind spots).

The probe was reverted via `git checkout -- src/lib/ressim/src/fim/newton.rs`
after measurement.

## Shortlist results — 2026-04-19

Commands (current-head wasm):
```
node scripts/fim-wasm-diagnostic.mjs --preset water-pressure \
    --grid 20x20x3 --steps 1 --dt 0.25 --diagnostic step --no-json
node scripts/fim-wasm-diagnostic.mjs --preset water-pressure \
    --grid 20x20x3 --steps 6 --dt 0.25 --diagnostic step --no-json
node scripts/fim-wasm-diagnostic.mjs --preset water-pressure \
    --grid 12x12x3 --steps 1 --dt 1 --diagnostic step --no-json
node scripts/fim-wasm-diagnostic.mjs --preset gas-rate \
    --grid 10x10x3 --steps 6 --dt 0.25 --diagnostic step --no-json
```

### Case 1 — medium-water 20x20x3 dt=0.25 step=1 (profiled case)
Probe total = 17,945 ms (matches step-reported `lin_ms`).

| category               | iters | first_ms | fallback_ms | total_ms | % lin_ms |
|------------------------|------:|---------:|------------:|---------:|---------:|
| post-fail-fallback     |    54 | 11,710.0 |       640.0 | 12,350.0 |   68.8% |
| clean                  |    29 |  3,606.0 |         0.0 |  3,606.0 |   20.1% |
| near-converged-accept  |     7 |  1,569.0 |         0.0 |  1,569.0 |    8.7% |
| repeated-zm            |    41 |    420.0 |         0.0 |    420.0 |    2.3% |
| dead-state             |     0 |      0.0 |         0.0 |      0.0 |    0.0% |
| restart-stag           |     0 |      0.0 |         0.0 |      0.0 |    0.0% |
| zm-fallback            |     0 |      0.0 |         0.0 |      0.0 |    0.0% |

### Case 2 — medium-water 20x20x3 dt=0.25 steps=6
Probe total = 125,843 ms (matches sum of per-step `lin_ms`).

| category               | iters | first_ms | fallback_ms | total_ms | % lin_ms |
|------------------------|------:|---------:|------------:|---------:|---------:|
| post-fail-fallback     |   320 | 90,170.0 |     3,556.0 | 93,726.0 |   74.5% |
| clean                  |   105 | 15,820.0 |         0.0 | 15,820.0 |   12.6% |
| near-converged-accept  |    53 | 12,996.0 |         0.0 | 12,996.0 |   10.3% |
| repeated-zm            |   246 |  2,631.0 |         0.0 |  2,631.0 |    2.1% |
| dead-state             |    51 |    670.0 |         0.0 |    670.0 |    0.5% |
| restart-stag           |     0 |      0.0 |         0.0 |      0.0 |    0.0% |
| zm-fallback            |     0 |      0.0 |         0.0 |      0.0 |    0.0% |

### Case 3 — heavy-water 12x12x3 dt=1
Probe total = 1,709 ms (matches step-reported `lin_ms`).

| category               | iters | first_ms | fallback_ms | total_ms | % lin_ms |
|------------------------|------:|---------:|------------:|---------:|---------:|
| post-fail-fallback     |    24 |  1,183.0 |        73.0 |  1,256.0 |   73.5% |
| dead-state             |   117 |    453.0 |         0.0 |    453.0 |   26.5% |
| clean                  |     0 |      0.0 |         0.0 |      0.0 |    0.0% |
| near-converged-accept  |     0 |      0.0 |         0.0 |      0.0 |    0.0% |
| repeated-zm            |     0 |      0.0 |         0.0 |      0.0 |    0.0% |
| restart-stag           |     0 |      0.0 |         0.0 |      0.0 |    0.0% |
| zm-fallback            |     0 |      0.0 |         0.0 |      0.0 |    0.0% |

### Case 4 — gas-rate 10x10x3 dt=0.25 steps=6
Probe total = 1,662 ms (matches sum of per-step `lin_ms`).

| category               | iters | first_ms | fallback_ms | total_ms | % lin_ms |
|------------------------|------:|---------:|------------:|---------:|---------:|
| clean                  |    33 |    593.0 |         0.0 |    593.0 |   35.7% |
| post-fail-fallback     |    32 |    521.0 |        52.0 |    573.0 |   34.5% |
| dead-state             |   380 |    496.0 |         0.0 |    496.0 |   29.8% |
| near-converged-accept  |     0 |      0.0 |         0.0 |      0.0 |    0.0% |
| repeated-zm            |     0 |      0.0 |         0.0 |      0.0 |    0.0% |
| restart-stag           |     0 |      0.0 |         0.0 |      0.0 |    0.0% |
| zm-fallback            |     0 |      0.0 |         0.0 |      0.0 |    0.0% |

## Reframing (vs the 2026-04-18 profile's hypothesis)

The 2026-04-18 cost profile identified "88% per-Newton-iter sparse-LU
on bypass paths" as the dominant cost and listed **four direct-bypass
triggers** (zero-move hotspot / restart-stagnation / dead-state /
post-failure fallback) as the candidates for narrowing. The audit
reframes this:

- **`post-fail-fallback` is 68.8% (case 1) / 74.5% (case 2) / 73.5%
  (case 3) / 34.5% (case 4)** of all `lin_ms`. This is the path where
  `fgmres-cpr` is tried, hits `max-iters`, and falls through to sparse-lu.
- The four **pre-solve direct-bypass triggers combined** (dead-state +
  restart-stag + repeated-zm + zm-fallback) are:
  - Case 1: **2.3%** (all `repeated-zm`)
  - Case 2: **2.6%** (repeated-zm 2.1% + dead-state 0.5%)
  - Case 3: **26.5%** (all `dead-state`)
  - Case 4: **29.8%** (all `dead-state`)
- `near-converged-accept` is another **8.7% / 10.3%** on medium-water —
  iterative didn't formally converge but was accepted via the existing
  gate; zero sparse-lu rescue cost.

**Conclusion:** on medium-water (the profiled shelf), the real cost
concentration is not the direct-bypass triggers — it's **`fgmres-cpr`
failing to converge and triggering sparse-lu rescue**. Together,
`post-fail-fallback` + `near-converged-accept` account for **77.5% of
case 1 lin_ms and 84.8% of case 2 lin_ms**. These are the two failure
outcomes of the iterative attempt and they share the same underlying
driver: `fgmres-cpr` hitting `max-iters` with non-zero residual.

Heavy-water (case 3) shares the same dominant category (`post-fail-fallback`
73.5%) but its ~26.5% `dead-state` slice is meaningful at that scale.
Gas-rate (case 4) has a roughly-even three-way split, with `dead-state`
contributing ~30% — but total cost is only 1,662 ms across 6 steps so
absolute savings on gas are small.

## Plan B — Two-slice narrowing (Lever 1 + Lever 3)

Both levers target the `fgmres-cpr fails → sparse-lu rescues` cost
concentration, attacking it from two independent angles. Each slice is
independently reversible and can be promoted or reverted without
blocking the other.

### Lever 1 — Widen `should_accept_near_converged_iterative_step`

**Current gate** (newton.rs:272-306):
```rust
// accept iff:
//   backend == FgmresCpr
//   failure.reason in { RestartStagnation, MaxIterations }
//   outer_residual_norm <= tolerance * 16.0         // OUTER_FACTOR
//   candidate_residual_norm <= outer_res * 8.0      // CANDIDATE_WORSENING_FACTOR
//   at least one restart showed solution_improved
```

**Observation from trace:** many `max-iters` failures have
`outer_res / tol` in the range 70-1800. A few sample failures from
case 1 (substep 0, retry 0):

| iter | outer_res | tol     | ratio   | current-gate result |
|-----:|----------:|--------:|--------:|:--------------------|
|    2 |   8.53e0  | 2.10e-4 |  40,000 | reject (right) |
|    3 |   3.04e-2 | 2.09e-4 |     145 | reject — **candidate** |
|    4 |   3.64e-1 | 2.04e-4 |   1,800 | reject (right) |
|    6 |   1.42e-2 | 2.06e-4 |      70 | reject — **candidate** |

At those ratios the iterative result is still "close" relative to the
Newton residual scale (Newton residuals are `O(1)` on water breakthrough).
Accepting a candidate at 70-150× tol vs paying a ~150 ms sparse-lu
refactor is likely a net win when damp + Appleyard chop the update anyway.

**Proposed change:**
- Raise `NEAR_CONVERGED_ITERATIVE_OUTER_FACTOR` from `16.0` to `200.0`.
- Keep `NEAR_CONVERGED_ITERATIVE_CANDIDATE_WORSENING_FACTOR = 8.0` (this
  is the safety rail against "candidate diverged inside fgmres"; do not
  relax).
- Keep the `at least one restart improved` requirement — it's the
  "some progress was made" guard.
- Also require `damping * ||update||_inf < 1e-1` at the iter level
  (evaluated after Appleyard) as an extra safety rail against accepting
  a too-loose iterative step that Newton will then damp hard and waste
  anyway. This condition is checked AFTER the accept decision is
  already committed; to add it cleanly means returning a conditional
  from the gate + adding a post-damp recheck — skip on first pass, add
  if first pass shows bad trajectories.

**Expected impact (estimated from audit):** the 8.7% / 10.3% current
`near-converged-accept` share grows by absorbing some fraction of the
68.8% / 74.5% `post-fail-fallback` share. If the gate absorbs 30-50%
of `post-fail-fallback` cost (the "close enough" subset), case 1 saves
20-34% of its `lin_ms`, case 2 saves 22-37%.

**Risks:**
- **Trajectory divergence** like the permissive-gate variant of the
  reverted Jacobian reuse Stage 2: accepting too-loose steps can
  add Newton iters (or retries) that cost more than the saved
  sparse-lu rescue. The existing `candidate_residual_norm` guard
  (`<= outer_res * 8.0`) is the primary defense.
- **Locked smoke tests** (drsdt0, spe1_first_steps, spe1_gas_injection)
  are sensitive to exact Newton trajectories and must stay green.
- **Heavy-water case 3** has zero `near-converged-accept` coverage
  already — heavy-water failures are at high ratios. Lever 1 is mostly
  a medium-water win.

### Lever 3 — Post-fail short-circuit

**Observation:** in the trace, a characteristic pattern is a run of
consecutive Newton iters where `fgmres-cpr` fails at `max-iters` and
sparse-lu rescues each time (case 1 substep 0: iters 2, 3, 4, 6 all
post-fail-fallback). The `fgmres-cpr` attempt on iter N+1 after iter N
failed costs ~300-450 ms (150 bicgstab inner iters × 5 restarts) and
effectively always fails the same way — the Jacobian structure hasn't
changed enough between iters to make the iterative method suddenly
converge.

**Proposed change:**
- Add a `iterative_failed_last_iter: bool` flag in the Newton loop.
- Set it when the current iter's first-solve was `fgmres-cpr` and
  failed (regardless of whether the fallback or the near-converged
  accept path was taken).
- On iter N+1, if the flag is set and no bypass is already active,
  skip the iterative attempt and call `direct_fallback_kind_for_rows`
  directly. This replaces `iterative_ms (~300-450) + sparse_lu_ms
  (~150)` with just `sparse_lu_ms (~150)`.
- **Clear the flag** whenever any of these happen:
  - a clean iterative converge (would never have set the flag, but
    safe to clear)
  - a successful near-converged-accept
  - a converged iter where some hotspot recovery fires
  - at the start of each new substep (flag lifetime = Newton loop only)
- Do **not** set the flag inside bypass paths — a dead-state or
  repeated-zm iter is already direct, and we don't want the next clean
  iter to inherit the short-circuit.

**Expected impact (estimated from audit):** removes the `first_ms` of
consecutive `post-fail-fallback` iters after the first one. In case 1
the 54 `post-fail-fallback` iters account for 11,710 `first_ms`; if
80% of those iters are "consecutive failures" (the `fgmres-cpr` attempt
is wasted), saving 80% × 11,710 ≈ 9,370 ms, i.e. **~52% of case 1
lin_ms**. Even a conservative 50% "consecutive" estimate gives
**~33% savings on case 1**. Case 2 projection: **~35-50% savings**.

**Risks:**
- **Too eager short-circuit:** sometimes a failed iterative iter is
  immediately followed by a cleanly-converged one (state moved
  significantly after Appleyard damping). Short-circuiting that iter
  would lose a free clean iterative convergence. Cheap to measure: add
  the flag, emit a probe line noting "short-circuit would have applied"
  before actually gating on it; audit for how often the next-iter
  iterative would have worked. Do that in a Stage 1 measurement pass
  before Stage 2 promotion — same promotion discipline used for
  Jacobian reuse.
- **Bypass interaction:** we must NOT short-circuit while a
  `dead-state` / `repeated-zm` flag is already active — those are
  already direct, and the short-circuit would be a no-op but would
  mask the bypass trigger category in the probe.
- **Backend-switch hysteresis:** if the iterative method *could* have
  converged on iter N+1 (we don't know without trying), we're leaving
  the clean-iter path permanently. This is fine as long as the flag
  clears on the next clean opportunity. The clear-conditions above
  ensure that.

### Lever 3 Stage 1 (measurement-only probe, no behavioral change)

Before implementing the gate, measure how often the short-circuit
would have helped vs. how often the next-iter iterative would have
converged. Instrument with the existing `BYPASS-AUDIT` line extended
by a field `prev_iter_failed={true|false}`. Then:

- Count iters where `prev_iter_failed = true`.
- Of those, partition by `cat`:
  - `cat == post-fail-fallback` → **would benefit** (short-circuit
    saves ~300-450 ms per iter)
  - `cat == near-converged-accept` → would benefit less (no sparse-lu
    rescue happens today)
  - `cat == clean` → would be **harmed** (we'd have short-circuited a
    clean iter)

If benefit:harm iter ratio is >5:1 on case 1 and case 2, Lever 3 is
safe to promote. If the ratio is <2:1, the iterative method recovers
often enough that short-circuiting is a net negative.

### Lever 1 Stage 1 (measurement-only dry-run)

Instrument the existing `should_accept_near_converged_iterative_step`
gate to emit **both** the current decision and the hypothetical
decision under the proposed `200.0` outer factor. Run on the four
cases; count:

- Iters where current rejects but proposed accepts (the "newly accepted"
  set).
- For those iters, the distribution of `outer_res / tol` ratios and
  `cand_res / outer_res` ratios.
- Projected `lin_ms` savings: sum of `first_ms + fallback_ms` for
  newly-accepted iters, minus an estimate of trajectory-divergence
  retries (conservatively, 10% of saved ms as "might come back as
  retries").

If case 1 projected savings is ≥15% of `lin_ms` and the outer-ratio
distribution has a clean "close-to-tol" cluster (not a long tail),
Lever 1 is safe to promote. If the distribution has no clean elbow
between "close enough" and "genuinely far", the proposed factor needs
tuning.

### Staging & ordering

Ordering by independence, risk, and cost:

1. **Lever 3 Stage 1 probe** (measure consecutive-failure share) —
   lowest implementation cost, sharpest signal.
2. **Lever 1 Stage 1 probe** (measure widened-gate absorption) —
   low cost, independent of Lever 3.
3. Decide: promote neither, one, or both based on the two probes.
4. **Lever 3 Stage 2** if Stage 1 passes — simpler of the two,
   behavior change is local (a single `if` branch in the iter loop).
5. **Lever 1 Stage 2** if Stage 1 passes — change is a single-constant
   edit; safety relies on the `cand_res / outer_res` guard already in
   the gate.
6. Validate on locked smoke tests + full shortlist + same-wasm A/B
   baseline swap, per the promotion discipline in `CLAUDE.md`.

## What this audit closes out

- **Dead-state / restart-stag / zm-fallback direct-bypass narrowing is
  dead** on medium-water. Combined share is 2-3% of lin_ms. Not worth
  touching.
- **`repeated-zm` direct-bypass narrowing is dead** on medium-water
  (2.3% case 1, 2.1% case 2). Leave the existing direct-bypass
  logic alone.
- **`dead-state` is worth a look on heavy-water (26.5%) and
  gas-rate (29.8%)** but those shelves have small absolute cost
  (1,709 ms and 1,662 ms total across all probed work), so surgery
  there saves small ms in absolute terms even if the percentage is
  high. Defer.
- The real lever is **iterative-backend failure handling**, not
  direct-bypass trigger policies.

## Probe revert

Probe revert via `git checkout -- src/lib/ressim/src/fim/newton.rs`.
Raw logs archived at `/tmp/bypass-audit/case{1-4}-*.log`. Aggregator
script at `/tmp/bypass-audit/aggregate.mjs`.

## Lever 3 Stage 1 probe — 2026-04-19

Probe: extended the `BYPASS-AUDIT` trace line with a `prev_iter_failed`
field. The flag is set iff the *previous* Newton iter's category was
`post-fail-fallback` (i.e. the iterative backend failed AND sparse-lu
rescued). It clears after any other category (clean, near-converged-
accept, bypass paths, zero-move-Appleyard-accept). No behavioral
change — measurement only.

The probe was reverted via `git checkout -- src/lib/ressim/src/fim/newton.rs`
and WASM rebuilt clean after the sweep.

### Decision rule

Per the Stage 1 design: categorize iters where `prev_iter_failed=true`
by their own `cat`:
- `cat == post-fail-fallback` → **benefit** (short-circuit saves the
  wasted `first_ms` of the fgmres-cpr attempt that would again fail)
- `cat == near-converged-accept` → **neutral** (no sparse-lu rescue
  today, so short-circuit replaces a cheap-ish iterative with a
  sparse-lu call — cost-shift, not savings)
- `cat == clean` → **harm** (short-circuit would have foregone a
  genuine clean iterative convergence)
- `cat == repeated-zm / dead-state / restart-stag / zm-fallback` →
  **n/a** (bypass already active; short-circuit gated on "no bypass
  active" wouldn't fire)

Promotion threshold: benefit:harm ≥ 5:1 on case 1 and case 2.

### Shortlist results

| case                                   | total iters | total_ms | benefit (pff) | harm (clean) | neutral (nca) | ratio   | proj. savings |
|----------------------------------------|------------:|---------:|--------------:|-------------:|--------------:|:--------|--------------:|
| Case 1: medium-water 20x20x3 step-1    |         131 |   17,089 |            19 |            2 |             0 | **9.5:1**  | **21.0% lin_ms** |
| Case 2: medium-water 20x20x3 6-step    |         775 |  122,099 |            76 |            3 |             6 | **25.3:1** | **17.1% lin_ms** |
| Case 3: heavy-water 12x12x3 dt=1       |         141 |    1,618 |             1 |            0 |             0 | 1:0     | 2.6% lin_ms   |
| Case 4: gas-rate 10x10x3 6-step        |         445 |    1,562 |             0 |            0 |             0 | 0:0     | 0% lin_ms     |

`pff` = post-fail-fallback; `nca` = near-converged-accept.

"proj. savings" is the sum of `first_ms` over benefit iters / total
`lin_ms`. This is a **best-case** figure that ignores:
- harm iters (foregone clean iterative; each loses a ~100-200 ms
  sparse-lu but avoids a similar-cost clean fgmres-cpr, roughly a wash)
- neutral iters (replace iterative with sparse-lu — cost-shift,
  direction depends on sparse-lu vs fgmres-cpr cost at that row count)

### Interpretation

**Case 1 and Case 2 pass the promotion threshold cleanly.** On
medium-water (the shelf where `lin_ms` concentrates — case 2 alone
is 122 s across 6 steps), ~17-21% of lin_ms is a wasted fgmres-cpr
attempt that immediately follows another failed fgmres-cpr attempt.
The benefit:harm ratios (9.5:1 and 25.3:1) are well above the 5:1
threshold.

**Case 3 and Case 4 see no Lever-3 benefit.** Both cases are
already fully covered by the `dead-state` direct-bypass: after the
first failure they go straight to sparse-lu for every subsequent
iter, so `prev_iter_failed=true` iters are already in `cat==dead-state`
rather than `cat==post-fail-fallback`. This is not a Lever 3 failure
— it's that the existing bypass logic already catches the pattern on
those shelves. Lever 3 is purely a medium-water win, which matches
where the cost actually is.

**Neutral cases are small enough to ignore at Stage 1.** 6 iters in
case 2 (~1.6 s total), 0 elsewhere. Not enough to move the verdict.

**Raw logs:** `/tmp/bypass-audit/case{1-4}.log`. Aggregator:
`/tmp/bypass-audit/aggregate.mjs`.

### Verdict

**PROMOTE to Lever 3 Stage 2 implementation.** The gate is:
- Track `iterative_failed_last_iter: bool` in the Newton loop.
- Set it iff the current iter's category was `post-fail-fallback`.
- Clear it on any other category.
- On iter N+1, if the flag is set AND no pre-solve bypass is already
  active, swap `linear_options.kind` to `direct_fallback_kind_for_rows`
  before the solve (skip the iterative attempt).
- Emit a fim_trace line noting the short-circuit so retro-analysis
  can distinguish it from the pre-existing bypass triggers.

**Expected case 1 savings (conservative, accounting for harm):**
- Benefit: save 3,590 ms of wasted first_ms (21% of 17,089).
- Harm: 2 iters will no longer converge cleanly via iterative; they
  now spend a sparse-lu (~180 ms each) instead of a fgmres-cpr
  (~150-250 ms). Net: roughly a wash, possibly small loss (~60 ms).
- **Expected net: 19-21% lin_ms reduction on case 1**.

**Expected case 2 savings:** benefit 20,882 ms / harm 3 iters ~540 ms.
Net **~16-17% lin_ms reduction** (~20 s off a 122 s run).

**No regression expected on case 3 / case 4:** Lever 3 fires only in
medium-water-like regimes; on shelves where `dead-state` bypass
dominates, the Lever 3 gate will rarely trigger (because `dead-state`
pre-empts it) and when it does will behave identically to the existing
bypass.

### Follow-up before Stage 2

Two risks deferred to Stage 2 validation, not blockers:

1. **Cross-substep hysteresis.** Flag lifetime should be Newton-loop
   only (clears at each substep boundary). Implementation: `let mut
   iterative_failed_last_iter = false;` inside `run_fim_timestep` and
   scoped to a single Newton loop — matches the existing bypass-flag
   pattern.
2. **Locked smoke tests** (drsdt0, spe1_first_steps, spe1_gas_injection)
   must stay green on Stage 2. These tests gate subtle Newton trajectory
   differences that could be perturbed if Lever 3 changes the exact
   step at which `dead_state_direct_bypass` latches.

Stage 2 should land as a minimal diff: one bool, one if-branch at
newton.rs:2658 (right before the existing bypass check), and a
fim_trace line. Promote per `CLAUDE.md` discipline: run locked smoke
tests + 4-case shortlist A/B on same-wasm baseline swap.

## Lever 3 Stage 2 — implementation + A/B result — 2026-04-20

Landed as a minimal additive diff in `src/lib/ressim/src/fim/newton.rs`
(+14 lines, 2 modifications around lines 2254, 2658, 2754):

```rust
// state declaration inside run_fim_timestep
let mut iterative_failed_last_iter = false;

// at the bypass-check site, hoist the or-chain and add a second branch:
let any_preexisting_bypass = dead_state_direct_bypass
    || restart_stagnation_direct_bypass
    || zero_move_fallback_direct_bypass
    || repeated_zero_move_direct_bypass;
if any_preexisting_bypass {
    linear_options.kind = direct_fallback_kind_for_rows(assembly.jacobian.rows());
    fim_trace!(sim, options.verbose,
        "    iter {:>2}: bypassing iterative backend after {}; using {}",
        iteration, /* label */, linear_options.kind.label());
} else if iterative_failed_last_iter {
    linear_options.kind = direct_fallback_kind_for_rows(assembly.jacobian.rows());
    fim_trace!(sim, options.verbose,
        "    iter {:>2}: iterative-failure short-circuit (prev iter fell back); using {}",
        iteration, linear_options.kind.label());
}

// after the linear-solve block:
iterative_failed_last_iter = used_fallback && !any_preexisting_bypass;
```

**Invariants:**
- Flag lifetime is Newton-loop-local (created inside `run_fim_timestep`,
  naturally clears at substep boundary).
- Flag sets iff `used_fallback && !any_preexisting_bypass` — i.e. this
  iter's category was `post-fail-fallback`, matching the Stage 1
  definition exactly.
- Flag clears on every other outcome (clean, near-converged-accept,
  any pre-existing bypass iter, and implicitly on return paths since
  the Newton loop exits). We deliberately do NOT set the flag from
  bypass iters — those don't touch the iterative backend, so "prev
  iter's iterative failed" is the wrong summary.

### Locked smoke tests

`cargo test --release --lib` on clean master: 297 passed / 7 failed.
`cargo test --release --lib` with Stage 2: 297 passed / 7 failed.
Exact parity — Stage 2 introduces no new test failures. (The 7 pre-
existing failures are tracked separately; 5 of them are FIM-scope,
unrelated to this work. Not blocking.)

### A/B sweep (same-wasm, 4 shortlist cases, 2026-04-20)

Commit state: [726d2a4](https://github.com/..) + Stage 2 diff. Both
baseline and stage2 WASM rebuilt from the same toolchain.

Per-step summary (values from the diagnostic `step=N |...` line):

#### Case 1 — medium-water 20x20x3 dt=0.25 step-1
| step | base lin_ms | stg lin_ms | Δ       | substeps | accepts        | oil     |
|-----:|------------:|-----------:|:--------|---------:|:---------------|--------:|
| 1    |      17,602 |     14,336 | **−18.6%** |    12=12 | 12+0+0=12+0+0 | 3337.62 |

#### Case 2 — medium-water 20x20x3 dt=0.25 6-step
| step | base lin_ms | stg lin_ms | Δ       | substeps | accepts        | oil     |
|-----:|------------:|-----------:|:--------|---------:|:---------------|--------:|
| 1    |      17,531 |     14,317 | −18.3%  |    12=12 | 12+0+0=12+0+0 | 3337.62 |
| 2    |      19,653 |     18,033 |  −8.2%  |    20=20 | 20+0+0=20+0+0 | 3458.48 |
| 3    |      19,746 |     15,292 | −22.6%  |    16=16 | 16+0+0=16+0+0 | 3517.50 |
| 4    |      24,022 |     20,802 | −13.4%  |    23=23 | 23+0+0=23+0+0 | 3556.99 |
| 5    |      20,172 |     15,843 | −21.5%  |    13=13 | 13+0+0=13+0+0 | 3587.28 |
| 6    |      20,642 |     18,047 | −12.6%  |    18=18 | 18+0+0=18+0+0 | 3610.27 |
| **Σ**|     121,766 |    102,334 | **−16.0%** |  102=102 | exact match   | exact   |

#### Case 3 — heavy-water 12x12x3 dt=1
| step | base lin_ms | stg lin_ms | Δ     | substeps | accepts                  | oil     |
|-----:|------------:|-----------:|:------|---------:|:-------------------------|--------:|
| 1    |       1,649 |      1,640 | −0.5% |    16=16 | 15+4+8354=15+4+8354      | 3808.44 |

#### Case 4 — gas-rate 10x10x3 6-step
| step | base lin_ms | stg lin_ms | Δ     | substeps | accepts        | oil    |
|-----:|------------:|-----------:|:------|---------:|:---------------|-------:|
| 1    |         533 |        532 | −0.2% |     8=8  | 8+0+0=8+0+0   | 160.88 |
| 2    |         299 |        300 | +0.3% |     4=4  | 4+0+0=4+0+0   | 161.10 |
| 3    |         203 |        193 | −4.9% |     4=4  | 4+0+0=4+0+0   | 161.31 |
| 4    |         195 |        191 | −2.1% |     4=4  | 4+0+0=4+0+0   | 161.50 |
| 5    |         177 |        179 | +1.1% |     4=4  | 4+0+0=4+0+0   | 161.72 |
| 6    |         171 |        173 | +1.2% |     4=4  | 4+0+0=4+0+0   | 161.92 |
| **Σ**|       1,578 |      1,568 | −0.6% |    28=28 | exact match   | exact  |

#### Overall

| metric           | baseline  | stage 2   | delta                |
|------------------|----------:|----------:|:---------------------|
| total lin_ms     |   142,595 |   119,878 | **−22,717 (−15.9%)** |
| max oil div      | —         | —         | 0.0 (bit-exact)      |
| trajectory       | —         | —         | **EQUIVALENT on all 4 cases** |

### Short-circuit activation count (probe trace)

| case | fired | Stage 1 prediction (benefit iters) |
|------|------:|-----------------------------------:|
| 1    |    15 | 19                                 |
| 2    |    78 | 76                                 |
| 3    |     1 |  1                                 |
| 4    |     0 |  0                                 |

Close match. Case 1's small drift (15 vs 19) is within noise — Newton
trajectory is trajectory-equivalent but not strictly iter-by-iter
identical because the short-circuit replaces a ~150-250 ms fgmres-cpr
with a ~100-180 ms sparse-lu, and the slightly different wall-clock
and solution numeric noise can shift when exactly `dead_state_direct_
bypass` flips on/off — which in turn can shift whether a given iter
lands as `post-fail-fallback` or `dead-state`. The final oil/substeps
are bit-exact so this is functionally equivalent.

### Verdict

**PROMOTED.** Lever 3 Stage 2 is the new baseline for medium-water
FIM convergence cost. Net **−15.9% across the 4-case shortlist** and
**−16.0% on the 6-step medium-water case** (the shelf where lin_ms
actually concentrates — 85% of the absolute cost savings). Cases 3/4
are neutral (as predicted). Trajectories bit-exact.

### Baseline update (per `CLAUDE.md` promotion discipline)

- **Commit:** Stage 2 diff landed on top of `726d2a4` (bypass audit doc).
- **Replay commands:** same as the Stage 1 probe (see "Shortlist
  results" above).
- **Baseline numbers (new, post-Stage-2, to be referenced by future
  convergence work):** case 1 lin_ms = 14,336; case 2 Σ lin_ms =
  102,334; case 3 = 1,640; case 4 Σ = 1,568.
- **Superseded baseline:** case 1 = 17,602; case 2 Σ = 121,766;
  cases 3/4 unchanged (within noise).
- **Raw logs (not committed, reproducible):** `/tmp/lever3-stage2/{baseline,stage2}-case{1-4}.log`.
  Comparator: `/tmp/lever3-stage2/compare.mjs`.

### Next direction

- **Lever 1 Stage 1 probe** is the natural next step. Lever 1 targets
  the `near-converged-accept` gate widening (raise `NEAR_CONVERGED_
  ITERATIVE_OUTER_FACTOR` from 16.0 to 200.0). Expected to compose
  additively with Lever 3 because the two levers catch different
  failure subsets:
  - Lever 3 shorts-out consecutive iterative failures (already landed).
  - Lever 1 would convert more single-failure iters into near-converged-
    accepts, saving the sparse-lu rescue cost entirely for those.
  - The two do not overlap: once Lever 3 fires on iter N, iter N+1
    uses sparse-lu from the start — Lever 1's gate never applies.
    Lever 1 would help on iters where the previous iter was `clean`
    but this one is `post-fail-fallback`.
- Stage 1 probe for Lever 1: instrument `should_accept_near_converged_
  iterative_step` to emit the current+hypothetical decisions under the
  proposed widened factor; run on the 4-case shortlist; project
  savings.
