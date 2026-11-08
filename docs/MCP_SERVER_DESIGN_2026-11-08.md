# MCP server design — ResSim as an instrument for agents (2026-11-08)

Pre-implementation note. It evaluates a proposal to expose ResSim through the Model Context Protocol
(MCP), so an AI agent can run catalog experiments and compare them against references. It fixes
the architecture and splits the work into gated phases. Tracking issue:
[#126](https://github.com/sergeyfarin/ressim/issues/126).

**Revision 2 (same day).** Folds in a design review of revision 1 (`d900000`). Revision 1 left
several things out:
- an executable reference-applicability contract;
- an execution model that can actually interrupt a run;
- provenance from the first release;
- a session-extraction gate stronger than "the same function gives the same summary";
- termination ownership for the native backend;
- comparison semantics.

All six points were checked against the code and accepted. §8 records where this note refines
the review instead of taking its wording.

**Priority.** This work comes after the scientific-validation track (`ROADMAP.md` §1, #52/#56). It
adds no physics and no reference, so it can wait until they are done.

## 1. The proposal and the verdict

The proposal: an optional MCP server, separate from the browser application, with a few
experiment-level tools, compact results, enforced resource limits, honest reference validity, and
docs exposed as MCP resources. It suggested Python on the `ressim-py` PyO3 binding, fed by a
scenario manifest generated from the TypeScript catalog.

**Verdict: valid, but the server should be TypeScript on the wasm engine rather than Python.** The
proposal's own principle decides it: one authoritative definition, many consumers. Every input an
experiment needs, apart from the engine, is TypeScript today:

| What an experiment needs | Where it lives | Python route | Node route |
|---|---|---|---|
| Scenario definitions, sensitivities, variants | `src/lib/catalog/scenarios/` | generated manifest | import |
| Run specs and create payloads | `scenario/runModel.ts` | manifest | import |
| Payload → engine setter calls | `workers/configureSimulator.ts` | recorded call list (exists: `catalogCases.ts`) | import |
| Early-stop rules | `workers/terminationPolicy.ts` | **port** | import |
| Analytical references | `@ressim/analytical`, `charts/analyticalMethodRegistry.ts` | **port** | import |
| OPM Flow / JutulDarcy reference curves | `catalog/opmFlowArtifacts.ts` + committed artifacts | re-parse | import |
| Quantity ids, labels, units | `contracts/run-quantities.json` (generated from the TS registry) | read | import |
| Input-validity and runtime warnings | `validateInputs.ts`, `warningPolicy.ts` | **port** | import |
| Compositional engine (`comp_*` scenarios) | wasm `CompositionalSimulator`, `workers/compositionalSession.ts` | **not bound** | import |

A Python server needs a manifest for the first rows. That half exists already: `catalogCases.ts`
records the exact setter calls for every black-oil catalog run so the parity gate can replay them.
The rows marked "port" would become second copies of the analytical methods and the termination
and warning policies, which this repository's rules forbid. And a Python server could not run the
compositional scenarios at all. A Node server imports all of it.
`crates/ressim-py/parity/run_wasm.mjs` and `catalog/opmFlowTwin.ts` already drive catalog runs
through the shipped wasm under Node. That shows the modules load under Vite's `runnerImport` and
vitest. It does **not** show that a packaged server works outside the repository; P0 tests that.

**Engine choice and answers.** The native/wasm parity gate (`scripts/validate-native-binding.sh`)
holds every black-oil catalog setup bit-identical across the two targets (#62, #64). It covers
configuration and a short prefix only: two steps of at most 0.1 day each (`CATALOG_STEP_CAP`,
`CATALOG_DT_CAP_DAYS`). Full-horizon cross-target agreement is a separate claim and matters only if
M5 adds a native backend (§3).

**Speed is a hypothesis.** wasm under Node is *expected* to be slower than native, but nobody has
measured it here. §3 M5 states what measurement would justify a native backend.

## 2. Architecture

```
MCP client (Claude Code, Codex, …)
        │  stdio
        ▼
tools/ressim-mcp — supervisor            tool schemas, limits, queue, cancellation,
        │                                provenance, retained runs, comparisons
        │  worker_threads message channel (one run per worker)
        ▼
simulation worker                        BlackOilSession (new, M0) · CompositionalSession
        │  catalog → runModel → configureSimulator → step → terminationPolicy
        ▼
wasm engine (src/lib/ressim/pkg, built by scripts/build-wasm.sh)
```

- **Placement.** The server lives in its own directory, `tools/ressim-mcp/`, with its own
  `package.json` and the MCP SDK as its only new dependency. The app bundle and the `@ressim/*`
  packages never import it. If it becomes a pnpm workspace member, `pnpm-workspace.yaml` gains one
  entry and `packageBoundaries.test.ts` gains its rule.
- **Module resolution and packaging.** App modules do not load under the plain Node ESM resolver
  (#31). The server is bundled with Vite (library/SSR build) into a supervisor entry, a worker
  entry and the `.wasm` file. P0 proves that bundle starts from an arbitrary working directory.
- **Execution is out of the supervisor's event loop.** `step()` is a synchronous wasm call. A timer
  or stop flag on the same event loop cannot interrupt it, so every run executes in a
  `worker_threads` worker that the supervisor can terminate (§4.3). A child process is the
  fallback if P0 shows `Worker.terminate()` does not interrupt a long wasm call promptly.
- **Scenario-agnostic.** The server never branches on a scenario key. Everything a tool reports
  about a case comes from the scenario's own declarations (`docs/SCENARIO_CATALOG_ARCHITECTURE.md`).
- **The browser app is unchanged** apart from the M0 refactor, which it shares.

## 3. Phases

Each phase ends in something usable or measurable, with its own exit gate. None depends on a
later one. Overrides, sweeps and compositional support are separate phases on purpose: each one
widens what an agent can claim, so each is gated on its own.

### P0 — Packaging spike

A throwaway proof, before any server code:

- A Vite-built Node entry, run from a temporary directory outside the repository, loads the wasm,
  configures one catalog run through `buildCreatePayloadForRun` and `configureReservoirSimulator`,
  and steps it.
- The same from inside a `worker_threads` worker. `Worker.terminate()` during a deliberately long
  step must return control within a stated bound and free the worker's memory. Record the
  observed bound.
- Output: a short section added to this document with the commit, commands and results. If
  terminate does not interrupt wasm, §2 switches to a child process before M1.

### M0 — Headless black-oil session, behaviour-preserving

The black-oil run loop lives inline in `src/lib/workers/sim.worker.ts`: initial state, step,
rate-history deltas, `evaluateTerminationPolicy`, stop and failure handling. The compositional
path already factored the same loop into `workers/compositionalSession.ts`. M0 extracts a
`BlackOilSession` with the same shape, and the worker becomes a message adapter over both
sessions.

**Gate: golden worker transcripts captured *before* the extraction.** Comparing the worker with
the session after the extraction would be circular, because both would run the same new code. So:

1. On the pre-extraction commit, drive the existing worker module under vitest over several
   representative catalog runs, chosen to cover:
   - a waterflood to full horizon;
   - a depletion case that stops early on its termination policy;
   - a run stopped by the client mid-way;
   - a resume after stop;
   - a run that fails (an engine error surfaced as a worker error).

   Record the full ordered message sequence for each: types, step indices, initial-state emission,
   rate-history deltas, cumulative volumes, final time, warnings, termination summary.
2. Commit those transcripts as fixtures.
3. After the extraction, the same driver must reproduce every transcript exactly.

The repository's frontend gate also applies (`ressim-validation` skill, frontend row):
`pnpm run validate` (typecheck, lint, `check:cycles`, fast tests, build) plus the e2e run smoke.

Independent value: the #31 debug scripts and any future Node tooling get a run loop without
copying the worker.

### M1 — Catalog-only first release

Tools:
- `list_scenarios`
- `describe_scenario` (declarations only, see below)
- `run_scenario` (a scenario key plus declared dimension/variant keys, **no overrides**)
- `get_run_series`
- `cancel` through MCP's request-cancellation notification, not a custom tool

This phase also delivers bounded execution, retained runs and the limits in §4.3, provenance on
every result (§4.4), and read-only MCP resources: `contracts/run-quantities.json`,
`docs/BENCHMARKS.md`, `docs/SCIENTIFIC_LIMITATIONS.md` and the validation documents indexed in
`docs/DOCUMENTATION_INDEX.md`, served as committed.

`describe_scenario` in M1 reports what the scenario declares: physics summary, solver policy and
rationale, dimensions and variants, termination policy, and the analytical method with its
**prose** validity summary, labelled as prose. It does not claim a reference applies to a run;
that waits for M2.

**Acceptance is one real agent task, not protocol plumbing.** A scripted MCP client session asks:

> Compare the declared mobility-ratio variants of the 1D waterflood, report how water
> breakthrough time and recovery at breakthrough change, and explain the trend.

It passes when the session completes inside the limits, using only M1 tools, and its reported
breakthrough times equal those in the golden worker transcripts for the same runs. Separate
client tests cover:
- cancellation mid-run: a partial summary marked `cancelled`;
- a run hitting each cap: a partial summary naming the cap;
- two simultaneous `run_scenario` calls: the second queues and does not overrun memory;
- worker failure: an error result, and the server keeps serving;
- a dirty-tree and a stale-wasm run: provenance flags them.

**Measure here:** wall time of the acceptance task's runs on wasm, recorded with commit hash and
exact commands, marked provisional, per AGENTS.md evidence rules. This is the input to M5.

### M2 — Reference comparisons with executable applicability

M2 has two prerequisites, both TypeScript, both shared with the app:

1. **Analytical applicability predicates.** Each analytical method in `analyticalMethodRegistry`
   gains a function from the resolved create payload (and, where needed, the run's own series) to
   `{ status: 'applicable' | 'not-applicable' | 'applicable-in-window', reasons: string[],
   window?: { from, to, axis } }`. Examples:
   - Buckley–Leverett is not applicable with capillary pressure or gravity that the method ignores.
   - Pseudo-steady-state depletion is applicable only after the transient, so it returns a window.

   Today `analyticalMethodSummary` is prose, and the per-variant `affectsAnalytical` flag only says
   whether the overlay must be recomputed, not whether it is still valid. Predicates are written
   only for the methods M2 exposes, each with tests on the catalog's own variants.
2. **Artifact compatibility is exact-run identity.** A committed OPM Flow or JutulDarcy curve is
   applicable only to the unmodified declared run it was generated for: same scenario, same
   dimension and variant, no overrides. The guard that this declared run still matches the deck
   is a scenario twin test (`catalog/opmFlowTwin.ts`), which fails when the setup drifts from Flow.
   At `2ef5027` only 7 of the 19 committed artifacts have one: `wf_bl1d`, `wf_capillary`,
   `wf_gravity_stability` and the four `sweep_*` cases. M2 offers an artifact comparison only for
   artifacts with a twin test, and adds twins for the others it needs. The server needs the run's
   identity and the guard, not general model-compatibility rules, and refuses the comparison
   otherwise.
   Changing a viscosity can leave Buckley–Leverett applicable while making the base-case Flow
   curve inapplicable, and the two checks report that independently.

Then `compare_runs` is added under the semantics in §4.5.

Gate: predicate unit tests on every declared variant of the scenarios M2 covers. Client tests for
`compare_runs`:
- an applicable comparison;
- an analytical "not applicable" with reasons;
- a windowed comparison;
- a refused artifact comparison on a non-declared run;
- the same verdict at two different `max_points`.

### M3 — Validated parameter overrides

`run_scenario` accepts overrides within the bounds `validateInputs.ts` and the warning policy
already enforce for Custom Mode. An override the app would reject is rejected with the app's
message. Every comparison on an overridden run goes through the M2 predicates, and artifact
comparisons are refused by construction. Gate: one client test per rejection path, and one where
an override flips an analytical method to "not applicable".

### M4 — Sweeps, then compositional

These two are separately gated and either can land first.

- **Sweeps.** `run_scenario` accepts a short list of variants or override sets. The supervisor
  queues them under the global limits and returns one summary row per run plus each `run_id`.
- **Compositional.** Scenarios on `CompositionalSession` join `list_scenarios` and `run_scenario`.
  The withheld cases (`comp_spe5_wag`, `comp_spe3_gas_cycling`) stay hidden exactly as in the
  picker. Compositional termination policies are not enforced yet (#98), and
  `describe_scenario` says so for affected cases.

### M5 — Optional native backend, only if measured workloads justify it

**Trigger.** M5 starts only when both hold, measured end to end on one committed revision:
- a recorded agent workload (the M1 acceptance task, or a later real one) takes over 5 minutes on
  wasm;
- native including IPC and serialization is at least 2× faster on that same workload.

Below that, the cost of a second backend, a second parity claim and a Python toolchain in the
server's path is not worth paying. If the trigger never fires, record that in
`docs/OPEN_ITEMS_2026-09-21.md`.

**Termination ownership.** If M5 starts, termination stays in TypeScript, so the native side must
step incrementally:
- the supervisor requests one report step at a time;
- the native process answers with that step's state delta;
- the supervisor evaluates `terminationPolicy` and decides whether to continue.

A native backend that runs the whole schedule before returning cannot honour early termination or
cancellation, and is not acceptable. Black-oil only (`ressim-py` does not bind compositional).
The run's provenance names the backend. A full-horizon native-vs-wasm agreement check on the
acceptance runs joins the gate, because the existing parity gate covers only a two-step prefix.

### Out of scope

A hosted or remote server. GitHub Pages cannot run one, and it would need hosting, authentication
and resource policing that a local stdio tool does not. Revisit it only if a real external user
needs it, under a separate issue.

## 4. Contracts the server keeps

### 4.1 Tools are experiment-level

There is no tool per engine setter. An agent reaches the engine only through a scenario, its
declared variants, and (from M3) validated overrides.

### 4.2 Compact results

- `run_scenario` returns a summary and a `run_id`:
  - final time and outcome (§4.5);
  - recovery factor and cumulative phase volumes;
  - breakthrough time where the scenario defines one;
  - solver warnings, and step and Newton counts.
- `get_run_series(run_id, quantities[], max_points)` returns series named by
  `contracts/run-quantities.json` ids, with that contract's units. It is downsampled for display
  to `max_points` (hard cap), and the response states the downsampling.
- Full 3D grids are never returned. An end-of-run field is available only as named per-cell
  arrays, under a cell-count cap.

### 4.3 Limits and execution

- **One simulation at a time** (default; configurable to a small N). Each run executes in its own
  `worker_threads` worker. Further `run_scenario` calls queue, with a bounded queue length, and a
  call beyond it is rejected immediately with the reason.
- **Per-run caps:** cells, report steps, wall time. Caps are checked before the worker starts
  where possible (cells, steps), and wall time is supervised by the parent.
- **Deadline.** At the wall-time cap the supervisor sends a stop request, which the session
  honours between report steps. If the worker has not acknowledged within a grace period (P0's
  measured bound plus margin), the supervisor terminates it. The result is then
  `outcome: 'limit-wall-time'`. It carries only the last accepted report-step state the
  supervisor *received*, and says so: anything computed after that is lost.
- **Cancellation.** An MCP cancellation notification for an in-flight `run_scenario` follows the
  same path, ending in `outcome: 'cancelled'`.
- **Failure and recovery.** A worker that throws or exits unexpectedly yields
  `outcome: 'failed'` with the error. The supervisor discards that worker and the next run gets a
  fresh one. A crashed run never takes the server down.
- **Retained runs** are bounded by both entry count and retained bytes (full-resolution series
  plus any retained per-cell arrays), with LRU eviction. A `run_id` evicted or unknown returns a
  clear error, never a stale result. Nothing is written outside a declared output directory, and
  results do not survive a restart.

### 4.4 Provenance, from M1

Every run result carries:
- the resolved run spec (scenario, dimension, variant, overrides);
- source revision and dirty flag;
- engine artifact identity: SHA-256 of the loaded `.wasm` bytes, and whether `pkg/` was fresh
  against its inputs at server start;
- backend (`wasm32`) and solver (IMPES / FIM / compositional);
- server version and wall time.

Comparisons also carry each reference's identity: artifact path and content hash, or the
analytical method id.

Git HEAD alone does not identify the engine. `pkg/` is generated and untracked (#30).
`build-wasm.sh` decides freshness by modification time, and with no `wasm-pack` it reuses a stale
local package with only a warning. The server applies the same freshness rule at start-up. A stale
package is either refused or flagged on every result, decided in M1, defaulting to refuse. The
wasm hash identifies what ran on this machine; it is not reproducible across machines, because the
release wasm embeds host paths (see the comment in `build-wasm.sh`).

### 4.5 Comparison semantics, from M2

- **Quantities and basis.** Series and metrics use `contracts/run-quantities.json` ids and units.
  Today they are all surface volumes (`Sm³`) or dimensionless. A reservoir-volume quantity, if
  needed, is added to that registry under its own id, never as a unit flag on an existing one.
- **Axis.** Every comparison states its axis (time in days, or a declared dimensionless axis such
  as pore volumes injected). Runs on different axes are not compared.
- **Horizon.** Metrics cover the overlapping interval of the compared series only, and the
  response states that interval.
- **Interpolation.** Linear in the stated axis onto the union of report times inside the overlap.
  It never extrapolates.
- **Incomplete runs.** A run with outcome other than `completed` or `terminated-by-policy` is
  compared only over the interval it reached, and is flagged.
- **Distinct outcomes for events.** For an event metric such as breakthrough, "not reached before
  the run ended" (with the end time and the outcome that ended it) is different from "not
  applicable to this scenario". Neither is reported as a number.
- **Metrics come from retained full-resolution data**, never from the `max_points` downsample, so
  the display resolution cannot change a verdict.
- **Reference honesty.** OPM Flow and JutulDarcy curves are named as committed artifacts with
  path, hash and source deck. The server never runs Flow, and never presents a committed curve as
  a fresh run. Benchmark numbers come from `docs/BENCHMARKS.md` as committed, and the server never
  computes or restates a "current baseline" (AGENTS.md evidence discipline).

## 5. Alternatives considered

| Alternative | Why not (now) |
|---|---|
| Python server on `ressim-py` + generated manifest (the original proposal) | Ports the analytical, termination and warning logic. Cannot run compositional. Kept as M5's optional backend under a measured trigger, not as the front door. |
| Simulation on the supervisor's event loop | `step()` is synchronous, so deadlines and cancellation could not interrupt a run. |
| General model-compatibility rules for committed artifacts | An artifact belongs to one declared run, and a twin test can guard that identity. Exact-run identity plus a twin test is executable today, while a rule language would be a speculative abstraction. |
| MCP inside the browser app or the wasm engine | Couples the product to an agent protocol and violates the engine's target-agnostic boundary (`ENGINE_PAYLOAD_BOUNDARY_DESIGN_2026-09-21.md`). |
| One tool per engine setter | Lets an agent build a model the catalog never validated, with no reference and no limits. |
| Rust-native MCP server | The catalog, analytical methods and policies would still be TypeScript, so it has the Python problem in another language. |
| Do nothing; agents use the repo and skills | Already covers *developing* ResSim. It does not let an agent *use* the simulator as an instrument, which is what this adds. |

## 6. Open questions, decided in the named phase

- SDK version pin (M1). The MCP SDKs move quickly; pin a release that is at least two weeks old
  and record it here.
- Default values of the §4.3 caps and the grace period (M1, from P0 and M1 measurements).
- Stale-wasm policy: refuse or flag (M1, default refuse).
- Workspace membership of `tools/ressim-mcp` (M1).

## 7. Review record

| Review point (rev. 1 at `d900000`) | Disposition |
|---|---|
| 1. Reference applicability needs an executable contract | Accepted. M2 prerequisites: per-method predicates with status, reasons and window. Artifact compatibility is narrowed to exact-run identity (§8). |
| 2. Wall-time limits cannot interrupt a synchronous `step()` | Accepted. Worker-per-run, stop-then-terminate deadline, MCP cancellation, failure recovery, global concurrency and retained-byte limits (§2, §4.3). P0 measures terminate latency. |
| 3. Provenance belongs in M1, including the engine artifact | Accepted (§4.4). The wasm hash is machine-local (§8). |
| 4. Parity and extraction evidence too weak | Accepted. M0 gate is golden transcripts captured before extraction. The two-step parity prefix is stated in §1, and M5 adds a full-horizon check. |
| 5. M4 leaves termination ownership unresolved | Accepted. Now M5: incremental stepping is required, there is a measured trigger, and speed is a hypothesis. |
| 6. Comparison semantics undefined | Accepted (§4.5), reusing `contracts/run-quantities.json` for ids, units and basis. |
| Delivery sequence and a concrete agent task as acceptance | Accepted. P0 → M0 → M1 (catalog-only, accepted on the mobility-variant task) → M2 → M3 → M4 → M5. |

## 8. Where this note refines the review

- **Artifacts need identity, not compatibility rules.** The review asks for "model/variant
  compatibility rules for artifacts". A committed curve is generated from one declared run.
  Exact-run identity, guarded by an `opmFlowTwin.ts` test that fails when that run's setup drifts
  from its deck, is therefore the whole rule: it is executable now and cannot be wrong in a subtle
  way. The work it implies is concrete: twin tests for the 12 artifacts that lack one, where M2
  needs them. Rules that let a curve stand in for a *different* run are the risk the review warns
  about.
- **Browser-versus-headless comparison is circular after M0.** Once the worker delegates to
  `BlackOilSession`, both paths run the same code, so comparing them proves nothing. The evidence
  has to be captured from the pre-extraction worker, hence the golden transcripts.
- **Engine hash is identity, not reproducibility.** Recording the hash answers "which binary ran".
  It does not let someone else rebuild that binary, because the release wasm embeds host paths.
  Source revision plus dirty flag plus freshness is the reproducibility claim; the hash is the
  audit claim.
- **A worker thread before a child process.** A worker is lighter and shares the existing bundle.
  A child process is the fallback only if P0 shows terminate does not interrupt wasm promptly.
