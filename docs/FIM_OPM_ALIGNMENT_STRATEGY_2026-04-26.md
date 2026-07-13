# FIM vs OPM Alignment Strategy (2026-04-26 session decision record)

> Provenance: renamed 2026-07-05 from the orphaned `docs/20260426.md`. This is the session record
> that set the "95% track OPM" policy and the Bundle A/B/C sequencing. It proved load-bearing on
> 2026-07-05: its `FIM-DAMP-002`/`003` context (inflection chop removal already refuted; k-sweep
> methodology) stopped a repeat experiment and led directly to `FIM-DAMP-004` (`k=1.25`).

> **Current addendum (2026-07-13):** the 95%-track-OPM policy still governs, but the Bundle C/AMG
> ordering below is superseded for convergence work. Exact-deck direct/live equivalence and the
> Y2a injector audit point first to state-bound/update semantics at `Swc`. Follow
> `FIM_OPM_CONVERGENCE_EXECUTION_PLAN.md`; AMG is a deferred scale-up item until linear error is
> measured as the limiting cause on a relevant case.

## Bundle status as of 2026-07-05 (added at rescue time)

- **Bundle A** (per-cell damping + well-row scaling): the row-scaling half was tried and REVERTED
  (`FIM-LINEAR-001`, blanket per-row scaling regressed heavy case 31→241). Per-cell damping was
  never promoted — still gated on the linear-solver direction being tamed first (i.e. Bundle C).
- **Bundle B** (summed-IMPES restriction): DONE, in improved form — the offline solver lab
  (Phase 9) compared `sum-rows` vs `quasi-impes` on real captured failure corpora and promoted
  `quasi-impes` (OPM's own construction), `FIM-LINEAR-005`/Step 9.3.
- **Bundle C** (full CPR completion with AMG coarse solver): **STILL OPEN** — the last major
  OPM architecture gap (`FIM-LINEAR-006`). After it lands, this doc's own plan says to revisit
  per-cell damping and dropping the inflection chop "in a clean state". Note the constraint
  recorded in the Phase 10 plan: no mature wasm32-compatible pure-Rust AMG crate exists;
  hand-rolling is ~1500-2000 LOC. `docs/FIM_CPR_IMPROVEMENT_PLAN.md` Phase 3 has the design
  skeleton.

On "track OPM closely vs. preserve ressim originality"
My honest take: 95% track OPM, 5% keep one ressim-specific piece.

OPM Flow is the de facto open-source reference for black-oil simulation; it's been validated on hundreds of public datasets across decades. Almost every ressim deviation we've measured has been a net negative or, at best, a workaround for an upstream gap that OPM avoided more elegantly. Specifically:

Things ressim has that OPM doesn't, that have been refuted as deviations from best practice in this session:

Wang-Tchelepi inflection chop (was: any crossing fires; OPM has nothing equivalent). Option B refutation showed it does real work — but ONLY because ressim's linear-solver direction is wild. With a proper CPR + AMG (matching OPM), the iter-0 dsw would be small and the chop would rarely fire. Option A k=1.2 widening got us partway; full removal would be possible after CPR completion. Long-term: drop it.
Proportional Appleyard damping (single global scalar). OPM's dsMax=0.2 is per-cell. Same story: ressim's wild iter-0 dsw=36 makes proportional necessary; with a better linear solver the per-cell version would work. Long-term: drop it.
Wang-Tchelepi STAGNATION (count=3) bailout (Fix A2). OPM has no per-iter bailout; just iterates to its max=12. Fix A2 Stage 1 showed widening doesn't help on case 2 (residual really diverges), but the gate itself is a ressim-specific choice that an OPM-style "let it iterate" would not have. Long-term: drop it.
HOTSPOT effective-move floor that turns into zero-move iters (Fix A1 fixed the worst symptom but the floor itself is ressim-specific machinery).
Things ressim has that OPM doesn't, that ARE worth keeping (defensible):

The Stage 1 probes themselves (STAG-TREND, DAMP-BREAKDOWN, COMPONENT-CLIP, UPWIND-SUMMARY). These aren't physics — they're diagnostics. OPM has DBG output but it's much less granular per Newton iter. Keep these forever; they're how we steered every fix this session.
The dt-cut growth machinery. OPM has its own (TimeStepControl=pid+newtoniteration, growth=1.25, decay=0.75 — which we already adopted). Ressim's implementation is ours, but the policy matches. Keep the implementation, since it's working and OPM-aligned.
The FIM/IMPES dual-mode capability. OPM is FIM-only. We can validate FIM against IMPES on simple cases as an internal correctness check — that's a ressim-specific advantage.
Things OPM has that ressim doesn't, that are BLOCKING us right now:

Full CPR with AMG coarse solver (Fix 2 + Fix 4). The audit's #1 missing piece. Until we have this, every Newton-mechanic experiment will keep hitting the "ressim's linear-solver direction is wild" wall. This should be the strategic priority after we exhaust the smaller bundling experiments.
Summed-IMPES restriction in CPR (the pressure equation; instead of "row 0 = water"). Audit's Fix 2.
Adaptive nonlinear solver tolerances per timestep. OPM tightens/loosens based on convergence history. Smaller win but real.
So your framing — bundle 2-3-4 changes if they bring us closer to OPM — is exactly right. The bundles I'd target:

Bundle A (this session, achievable): Per-cell damping + well-row scaling. Tests whether the "wild linear-solver direction" can be tamed enough at the source for OPM-style per-cell damping to work. If yes, we close the case-2 substep gap. If no, we've sharpened the case for tackling CPR completion next.

Bundle B (next session, 2-4 days): Summed-IMPES restriction (Fix 2). Self-contained, replaces restriction[0]=1.0; ... with restriction = [1/Bw, 1/Bo, 1/Bg]. Independent of solver chain.

Bundle C (multi-week): Full CPR completion = Fix 2 + Fix 4 (AMG coarse solver). Brings ressim's linear solver chain in line with OPM's. After this lands, revisit Option B (drop inflection chop) and per-cell damping in a clean state.

OK, proceeding with Bundle A now. Plan: keep the per-cell damping on this branch, add well/perforation row scaling, run the 4-case shortlist + case-3 fine-dt.

Update Todos

Locate row scaling sites: where Jacobian is handed to linear solver

Identify magnitude of well/perforation rows vs cell rows on case 2 step 1 iter 0

Design Bundle A: scale well/perforation rows by 1/100 (or similar) before linear solve, leaving cell rows untouched

Implement: scale RHS row + Jacobian row + recover x normally (rows are scaled, cols are not)

Quick scale-factor sweep on case 2 + case 3 to find the right factor

Validate: 4-case shortlist + case-3 fine-dt physics

Decide promote/refine/escalate to CPR completion
