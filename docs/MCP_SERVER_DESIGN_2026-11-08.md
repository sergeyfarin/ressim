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

**Revision 3 (same day).** Folds in a second review (of `7d4a13b`). It makes three corrections:
- a twin test is a numerical regression check, not an identity guard, so committed artifacts get a
  recorded input identity checked before scoring (M2);
- the engine hash, build provenance and reproducibility are three separate claims, and the server
  makes only the first two (§4.4);
- a cancelled MCP request cannot carry its partial result, so runs become handles with a separate
  status call (§4.2, §4.3).

It also makes the transcript comparison deterministic, keeps browser-versus-headless integration
tests, corrects the compositional termination status, and names `validate:product` as the M0
gate. §7 records each change.

**Revision 4 (same day).** A third review (of `0c622ec`) accepted the design subject to three
local corrections, all made here:
- the run fingerprint includes the resolved termination policy;
- the dirty-tree identity is a manifest hash that includes untracked inputs;
- resume in the M0 fixture driver waits for the stopped handler to settle.

Remaining feasibility questions belong to P0.

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

**Gate: golden worker transcripts captured *before* the extraction.** After the extraction both
the worker and the session run the same new code, so comparing them cannot show that the
extraction preserved behaviour. That evidence has to come from the old worker. So:

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

**Making "exactly" deterministic.**
- **Timing fields.** Messages carry wall-clock values (`batchMs`, `avgStepMs`, `extractMs` and the
  rest of the run profile). The driver checks each is a finite number ≥ 0 and then removes it
  before comparing.
- **Everything else** — simulation data, step indices, message types and order — is compared
  exactly.
- **Stop at a fixed step.** The worker observes `stop` after a `setTimeout(0)` yield, so a stop sent
  on a timer would land at a variable step. The driver instead delivers `stop` synchronously from
  inside its `post` callback when it receives the *k*-th state message. The flag is then set
  before the worker yields, so the stop step is fixed.
- **Resume waits for the stop to settle.** Resume is *not* sent from inside the callback. The
  worker posts `stopped` before it clears `isRunning`, so a resume sent synchronously from that
  callback would hit "Simulator is already running". The driver waits for the stopped run's
  handler promise to settle, then sends resume.

**Integration tests too.** Browser-versus-headless tests on the same runs are kept as well. They
cannot show the shared loop is right, but they do catch adapter, serialization and lifecycle
differences between the worker's message path and a direct session call. The transcripts and the
integration tests answer different questions.

Before committing, the repository's pre-commit gate applies (`ressim-validation` skill, tier 2):
`pnpm run validate:product` (typecheck, lint, `check:cycles`, full vitest, IMPES bucket, build),
plus the e2e run smoke.

Independent value: the #31 debug scripts and any future Node tooling get a run loop without
copying the worker.

### M1 — Catalog-only first release

Tools:
- `list_scenarios`
- `describe_scenario` (declarations only, see below)
- `start_run`: a scenario key plus declared dimension/variant keys, **no overrides**. Returns a
  `run_id` at once (§4.2).
- `get_run`: status and the summary so far, with an optional bounded wait.
- `cancel_run`: stops a run; its partial summary stays available through `get_run`.
- `get_run_series`

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
tests, all through the real MCP SDK client over stdio, cover:
- `cancel_run` mid-run: a later `get_run` returns a partial summary with outcome `cancelled`;
- MCP request cancellation of a waiting `get_run`: the wait ends, the run keeps going, and a fresh
  `get_run` still finds it;
- a run hitting each cap: a partial summary naming the cap;
- two simultaneous `start_run` calls: the second queues and does not overrun memory;
- worker failure: outcome `failed`, and the server keeps serving;
- a dirty-tree run (labelled `experimental`) and a stale-wasm start (refused or flagged).

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
2. **Artifact compatibility is exact-run identity, recorded and checked.** A committed OPM Flow
   or JutulDarcy curve is applicable only to the unmodified declared run it was generated for:
   same scenario, same dimension and variant, no overrides. Each artifact gets an identity record:
   - the declared run (scenario, dimension, variant keys);
   - an **input fingerprint**: a hash of the run's resolved experiment input. That means grid
     size, porosity, the ordered setter calls `configureReservoirSimulator` makes, the full report
     schedule (step count and step length), and the resolved termination policy. The policy
     lives in the run spec, not in the setter calls, yet changing one of its thresholds changes
     when the declared run ends, so it has to be in the identity;
   - the artifact file hash and the source deck hash.

   The fingerprint reuses the recorder and argument encoding `catalogCases.ts` already has. They
   move into a shared module under `src/lib/` that both the parity gate and the server import.
   Two checks use the record:
   - A unit test recomputes every recorded fingerprint from the current catalog. Any input change
     fails CI at once, however small, and re-recording it is a reviewed act. A second test checks
     the fingerprint's coverage: changing only a termination threshold, or only one setter
     argument, must change it.
   - The server checks the fingerprint of the run being scored before scoring it, and refuses on a
     mismatch.

   **What this does and does not establish.** The record proves the run's inputs have not changed
   since the artifact was admitted. It does not prove that the deck was a faithful translation of
   those inputs. That rests on the admission review and on the scenario's twin test
   (`catalog/opmFlowTwin.ts`), a *numerical regression check* within tolerance bands: `wf_bl1d`
   allows 4 % worst cumulative-oil and 2 % injection error. A small input change can stay inside
   those bands, which is why the twin test is not the identity guard.

   At `2ef5027`, 7 of the 19 committed artifacts have twin tests (`wf_bl1d`, `wf_capillary`,
   `wf_gravity_stability` and the four `sweep_*` cases); none has an identity record yet. M2 offers
   an artifact comparison only for artifacts with both. Generating the deck from the recorded call
   list, which would make the deck binding hold by construction, is a possible later
   strengthening and is out of scope here.

   Changing a viscosity can leave Buckley–Leverett applicable while making the base-case Flow
   curve inapplicable, and the two checks report that independently.

Then `compare_runs` is added under the semantics in §4.5.

Gate: predicate unit tests on every declared variant of the scenarios M2 covers. Client tests for
`compare_runs`:
- an applicable comparison;
- an analytical "not applicable" with reasons;
- a windowed comparison;
- a refused artifact comparison on a non-declared run, and on a declared run whose fingerprint
  no longer matches the record;
- the same verdict at two different `max_points`.

### M3 — Validated parameter overrides

`start_run` accepts overrides within the bounds `validateInputs.ts` and the warning policy
already enforce for Custom Mode. An override the app would reject is rejected with the app's
message. Every comparison on an overridden run goes through the M2 predicates, and artifact
comparisons are refused by construction. Gate: one client test per rejection path, and one where
an override flips an analytical method to "not applicable".

### M4 — Sweeps, then compositional

These two are separately gated and either can land first.

- **Sweeps.** `start_run` accepts a short list of variants or override sets and returns one
  `run_id` per run at once. The supervisor queues them under the global limits.
- **Compositional.** Scenarios on `CompositionalSession` join `list_scenarios` and `start_run`.
  The withheld cases (`comp_spe5_wag`, `comp_spe3_gas_cycling`) stay hidden exactly as in the
  picker. `CompositionalSession.run()` already evaluates termination policies (#111). #98 stays
  open only until a real SPE5 run stops on its GOR or WOR criterion and records the stopping
  report, and that case is withheld anyway.

### M5 — Optional native backend, only if measured workloads justify it

**Trigger (provisional cost bar).** M5 starts only when both hold, measured end to end on one
committed revision:
- a recorded agent workload (the M1 acceptance task, or a later real one) takes over 5 minutes on
  wasm;
- native including IPC and serialization is at least 2× faster on that same workload.

Both sides use the same workload, release builds and hardware. The figure is the median of at
least five repetitions, with the spread reported.

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

- **Runs are handles.** `start_run` validates the request against the caps, queues it, and
  returns a `run_id` before any simulation starts. The handle is known before cancellation can
  happen.

  This matters because of MCP's cancellation rule: a receiver should not respond to a cancelled
  request, and a client ignores a response that arrives afterwards. A long blocking
  `run_scenario` that is cancelled could therefore deliver neither its partial summary nor its
  id.
- `get_run(run_id, wait_seconds?)` returns the status and the summary so far. Status is `queued`,
  `running`, or a final outcome (§4.5). The summary holds:
  - final (or latest) time and outcome;
  - recovery factor and cumulative phase volumes;
  - breakthrough time where the scenario defines one;
  - solver warnings, and step and Newton counts.

  The optional wait is capped (tens of seconds), so no request blocks for a whole run. Cancelling a
  waiting `get_run` cancels only the wait.
- If the pinned MCP spec version offers a standard mechanism for long-running requests, M1
  evaluates it against these handle tools; it replaces them only if partial results stay
  retrievable after cancellation.
- `get_run_series(run_id, quantities[], max_points)` returns series named by
  `contracts/run-quantities.json` ids, with that contract's units. It is downsampled for display
  to `max_points` (hard cap), and the response states the downsampling.
- Full 3D grids are never returned. An end-of-run field is available only as named per-cell
  arrays, under a cell-count cap.

### 4.3 Limits and execution

- **One simulation at a time** (default; configurable to a small N). Each run executes in its own
  `worker_threads` worker. Further `start_run` calls queue, with a bounded queue length, and a
  call beyond it is rejected immediately with the reason.
- **Per-run caps:** cells, report steps, wall time. Caps are checked before the worker starts
  where possible (cells, steps), and wall time is supervised by the parent.
- **Deadline.** At the wall-time cap the supervisor sends a stop request, which the session
  honours between report steps. If the worker has not acknowledged within a grace period (P0's
  measured bound plus margin), the supervisor terminates it. The result is then
  `outcome: 'limit-wall-time'`. It carries only the last accepted report-step state the
  supervisor *received*, and says so: anything computed after that is lost.
- **Cancellation.** `cancel_run(run_id)` follows the same stop-then-terminate path and ends in
  `outcome: 'cancelled'`. The partial summary is retrieved with `get_run`. A queued run is removed
  without starting. MCP request cancellation applies only to the request it names: it ends a
  `get_run` wait, and never stops a run.
- **Failure and recovery.** A worker that throws or exits unexpectedly yields
  `outcome: 'failed'` with the error. The supervisor discards that worker and the next run gets a
  fresh one. A crashed run never takes the server down.
- **Retained runs** are bounded by both entry count and retained bytes (full-resolution series
  plus any retained per-cell arrays), with LRU eviction. A `run_id` evicted or unknown returns a
  clear error, never a stale result. Nothing is written outside a declared output directory, and
  results do not survive a restart.

### 4.4 Provenance, from M1

The server makes three separate claims, and claims them separately.

1. **Identity of the executed bytes.** The SHA-256 of the loaded `.wasm` names exactly the binary
   that ran. A copied binary keeps the same hash anywhere, so the claim travels with the bytes.
2. **Build provenance**: how those bytes were produced.
   - **Source content.** Git HEAD alone does not say, because `pkg/` is generated and untracked
     (#30). `build-wasm.sh` also judges freshness by modification time, and with no `wasm-pack`
     it reuses a stale local package with only a warning. So M1 makes `build-wasm.sh` write a
     build record into `pkg/`: a content hash of its declared inputs (`WASM_INPUTS`) and the
     toolchain versions. At start-up the server recomputes the input hash from the working tree. A
     mismatch, or a missing record, means stale. A stale package is refused by default, or flagged
     on every result if that policy is chosen in M1. The record is what the build script asserted,
     not a proof that these sources produced these bytes. It closes the timestamp gap, not the
     trust gap.
   - **Tree state.** Every result carries the source revision and a dirty flag. A dirty tree's
     changes are not preserved, so its runs are labelled `experimental`.
   - **Dirty-tree hash.** Each dirty run also carries a hash of a defined working-tree manifest.
     The manifest takes every modified, added or untracked (not ignored) file under the source
     roots: `src/`, `crates/`, `contracts/`, `scripts/`, `tools/ressim-mcp/`, the root `Cargo.*`,
     `package.json` and `pnpm-lock.yaml`. It sorts them by path and hashes each path, its status
     and the full content of each file, binary or not. Two runs on the same dirty state can then
     be recognised as such.
   - **Why not `git diff HEAD`.** Hashing that output would miss untracked files, and a binary
     diff does not encode full contents, so different trees could share a hash. Ignored files,
     including the generated `pkg/`, are outside the manifest; the wasm hash covers `pkg/`.
3. **Reproducibility is not claimed.** Rebuilding the same bytes would need recoverable inputs
   (no uncommitted changes) and a specified build environment. Host paths embedded in the release
   wasm (see the comment in `build-wasm.sh`) also prevent a byte-identical rebuild elsewhere. The
   server records enough to *investigate* a result, not to promise it can be regenerated.

Every run result carries:
- the resolved run spec (scenario, dimension, variant, overrides) and its input fingerprint (M2);
- the wasm hash and the build record's input hash and freshness verdict;
- source revision, dirty flag, the dirty-tree manifest hash, and `experimental` when dirty;
- backend (`wasm32`) and solver (IMPES / FIM / compositional);
- server version and wall time.

Comparisons also carry each reference's identity: the artifact's identity record (M2), or the
analytical method id.

### 4.5 Comparison semantics, from M2

- **Quantities and basis.** Series and metrics use `contracts/run-quantities.json` ids and units.
  In that contract today, every volume quantity (cumulatives and rates) is on surface basis
  (`Sm³`, `Sm³/day`); the others are pressures, ratios and fractions. A reservoir-volume quantity,
  if needed, is added to that registry under its own id, never as a unit flag on an existing one.
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
  their identity record (path, artifact and deck hashes, declared run, input fingerprint). The server never runs Flow, and never presents a committed curve as
  a fresh run. Benchmark numbers come from `docs/BENCHMARKS.md` as committed, and the server never
  computes or restates a "current baseline" (AGENTS.md evidence discipline).

## 5. Alternatives considered

| Alternative | Why not (now) |
|---|---|
| Python server on `ressim-py` + generated manifest (the original proposal) | Ports the analytical, termination and warning logic. Cannot run compositional. Kept as M5's optional backend under a measured trigger, not as the front door. |
| Simulation on the supervisor's event loop | `step()` is synchronous, so deadlines and cancellation could not interrupt a run. |
| General model-compatibility rules for committed artifacts | An artifact belongs to one declared run. A recorded input fingerprint checks that identity exactly, while a rule language would be a speculative abstraction. |
| Twin test as the identity guard | It compares outputs within tolerance bands (4 % / 2 % for `wf_bl1d`), so a small input change can pass. It stays as the numerical regression check. |
| One blocking `run_scenario` call with MCP request cancellation | A cancelled request should get no response, so neither the partial summary nor the run id could be delivered. |
| MCP inside the browser app or the wasm engine | Couples the product to an agent protocol and violates the engine's target-agnostic boundary (`ENGINE_PAYLOAD_BOUNDARY_DESIGN_2026-09-21.md`). |
| One tool per engine setter | Lets an agent build a model the catalog never validated, with no reference and no limits. |
| Rust-native MCP server | The catalog, analytical methods and policies would still be TypeScript, so it has the Python problem in another language. |
| Do nothing; agents use the repo and skills | Already covers *developing* ResSim. It does not let an agent *use* the simulator as an instrument, which is what this adds. |

## 6. Open questions, decided in the named phase

- SDK version pin (M1). The MCP SDKs move quickly; pin a release that is at least two weeks old
  and record it here.
- Default values of the §4.3 caps and the grace period (M1, from P0 and M1 measurements).
- Stale-wasm policy: refuse or flag (M1, default refuse).
- `get_run` maximum wait, and whether a standard long-running-request mechanism in the pinned
  MCP spec replaces the handle tools (M1).
- Where artifact identity records live, and the script that writes them at admission (M2).
- Workspace membership of `tools/ressim-mcp` (M1).

## 7. Review record

### Third review (rev. 3 at `0c622ec`)

Design accepted subject to three corrections, all made in revision 4.

| Review point | Disposition |
|---|---|
| 1. Termination policy is outside the run fingerprint | Accepted. The resolved termination policy is part of the input fingerprint, and a test checks that changing only a threshold changes it. |
| 2. A `git diff HEAD` hash is not a complete dirty-tree identity | Accepted. Replaced by a hash of a defined working-tree manifest that includes untracked files and full contents. |
| 3. Resume cannot be sent synchronously from the `stopped` callback | Accepted. Stop stays synchronous; resume waits for the stopped handler to settle. Confirmed in `sim.worker.ts`: `postStopped` runs before `isRunning = false`. |

### Second review (rev. 2 at `7d4a13b`)

| Review point | Disposition |
|---|---|
| 1. Twin tests do not establish exact setup identity | Accepted. Each artifact gets an identity record (declared run, input fingerprint, artifact and deck hashes), checked in CI and before scoring. Twin tests remain the numerical regression check. The "cannot be wrong in a subtle way" claim is withdrawn. |
| 2. Reproducibility overclaimed | Accepted. Three separate claims: byte identity, build provenance (now with a content-hash build record instead of timestamps), and reproducibility, which is not claimed. Dirty runs are `experimental`. The "machine-local hash" wording is corrected. |
| 3. Cancelled requests cannot return partial results | Accepted. `start_run` returns a handle at once; `get_run` (bounded wait) and `cancel_run` are separate; tests use the real SDK client. |
| Deterministic transcript comparison | Accepted. Timing fields are checked and removed; stop is delivered synchronously at the *k*-th state message (resume scheduling corrected in revision 4). |
| Keep browser-versus-headless integration tests | Accepted. They catch adapter and lifecycle differences, while the transcripts check behaviour preservation. |
| Compositional termination status | Corrected. `CompositionalSession.run()` evaluates policies; #98 waits only on the real SPE5 stopping report. |
| M0 gate | Corrected to `pnpm run validate:product`. |
| §4.5 basis wording | Corrected: volume quantities are surface basis; the contract also has pressures and ratios. |
| Native trigger | Kept as provisional, with same-workload, release-build, same-hardware, repeated measurement. |

### First review (rev. 1 at `d900000`)

| Review point | Disposition |
|---|---|
| 1. Reference applicability needs an executable contract | Accepted. M2 prerequisites: per-method predicates with status, reasons and window. Artifact compatibility is narrowed to exact-run identity (§8), recorded and checked since revision 3. |
| 2. Wall-time limits cannot interrupt a synchronous `step()` | Accepted. Worker-per-run, stop-then-terminate deadline, cancellation (run handles since revision 3), failure recovery, global concurrency and retained-byte limits (§2, §4.3). P0 measures terminate latency. |
| 3. Provenance belongs in M1, including the engine artifact | Accepted (§4.4); claims separated in revision 3. |
| 4. Parity and extraction evidence too weak | Accepted. M0 gate is golden transcripts captured before extraction. The two-step parity prefix is stated in §1, and M5 adds a full-horizon check. |
| 5. M4 leaves termination ownership unresolved | Accepted. Now M5: incremental stepping is required, there is a measured trigger, and speed is a hypothesis. |
| 6. Comparison semantics undefined | Accepted (§4.5), reusing `contracts/run-quantities.json` for ids, units and basis. |
| Delivery sequence and a concrete agent task as acceptance | Accepted. P0 → M0 → M1 (catalog-only, accepted on the mobility-variant task) → M2 → M3 → M4 → M5. |

## 8. Where this note refines the review

- **Artifacts need identity, not compatibility rules.** The first review asked for
  "model/variant compatibility rules for artifacts". A committed curve is generated from one
  declared run, so exact-run identity is the whole applicability rule. Rules that let a curve stand
  in for a *different* run are the risk the review warns about. Revision 3 makes the identity
  itself exact (an input fingerprint) rather than inferring it from a tolerance-banded twin test.
- **Browser-versus-headless comparison cannot show behaviour was preserved.** Once the worker
  delegates to `BlackOilSession`, both paths run the same code. Evidence that the extraction
  changed nothing has to come from the pre-extraction worker, hence the golden transcripts. The
  integration tests stay for what they can catch (§3 M0).
- **A worker thread before a child process.** A worker is lighter and shares the existing bundle.
  A child process is the fallback only if P0 shows terminate does not interrupt wasm promptly.
