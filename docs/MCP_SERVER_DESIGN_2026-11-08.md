# MCP server design — ResSim as an instrument for agents (2026-11-08)

Pre-implementation note. Evaluates a proposal to expose ResSim through the Model Context Protocol
(MCP) so an AI agent can run catalog experiments, sweep parameters and compare the results against
references. It also fixes the architecture and splits the work into gated phases. Tracking issue:
[#126](https://github.com/sergeyfarin/ressim/issues/126).

**Priority.** This work comes after the scientific-validation track (`ROADMAP.md` §1, #52/#56). It
adds no physics and no reference, so it can wait until they are done.

## 1. The proposal and the verdict

The proposal: an optional MCP server, separate from the browser application, with a few
experiment-level tools (`list_scenarios`, `describe_scenario`, `run_scenario`, `compare_runs`,
`get_run_series`), compact results, enforced resource limits, honest reference validity, and docs
and benchmark results exposed as MCP resources. It suggested Python on the `ressim-py` PyO3
binding, fed by a scenario manifest generated from the TypeScript catalog.

**Verdict: valid, but the server should be TypeScript on the wasm engine rather than Python.**

The goal and the guardrails are sound and are adopted unchanged (§4). The language is the one
decision this note reverses. The proposal itself states the deciding principle: one authoritative
definition, many consumers. Every input an experiment needs, apart from the engine, is TypeScript
today:

| What an experiment needs | Where it lives | Python route | Node route |
|---|---|---|---|
| Scenario definitions, sensitivities, variants | `src/lib/catalog/scenarios/` | generated manifest | import |
| Run specs and create payloads | `scenario/runModel.ts` (`buildScenarioRunSpecs`, `buildCreatePayloadForRun`) | manifest | import |
| Payload → engine setter calls | `workers/configureSimulator.ts` | recorded call list (exists: `catalogCases.ts`) | import |
| Early-stop rules | `workers/terminationPolicy.ts` | **port** | import |
| Analytical references (BL, depletion, material balance, …) | `@ressim/analytical`, `charts/analyticalMethodRegistry.ts` | **port** | import |
| OPM Flow / JutulDarcy reference curves | `catalog/opmFlowArtifacts.ts` + committed artifacts | re-parse | import |
| Input-validity and runtime warnings | `validateInputs.ts`, `warningPolicy.ts` | **port** | import |
| Compositional engine (`comp_*` scenarios) | wasm `CompositionalSimulator`, `workers/compositionalSession.ts` | **not bound** (`CONFIGURATION_COVERAGE.md`) | import |

A Python server needs a manifest for the first three rows. That half exists already: `catalogCases.ts`
records the exact setter calls for every black-oil catalog run so the parity gate can replay them.
The other rows would have to be ported, which creates second copies of the analytical methods and
the termination and warning policies. Those are the copies this repository's rules forbid. A
Python server also could not run the compositional scenarios at all.

A Node server imports all of it. It also has precedent: `crates/ressim-py/parity/run_wasm.mjs` already
loads the shipped wasm bundle under Node and drives the catalog through Vite's `runnerImport`.

The engine choice does not change the answer. The native/wasm parity gate
(`scripts/validate-native-binding.sh`) holds every black-oil catalog setup bit-identical across the
two targets (#62, #64). The only open cost is speed: wasm under Node is single-threaded and slower
than native. M1 measures the gap before anything depends on it.

**When Python earns its place.** It becomes worth adding as an optional *backend* behind the same
tool schemas (M4) if M1 shows wasm too slow for sweeps an agent would actually request, or if
someone needs the server inside a Python analysis stack. It is not a second front door with its
own catalog.

## 2. Architecture

```
MCP client (Claude Code, Codex, …)
        │  stdio
        ▼
tools/ressim-mcp            TypeScript; MCP SDK; tool schemas, limits, provenance
        │  imports by package name / app modules
        ▼
headless session layer      BlackOilSession (new, M0) · CompositionalSession (exists)
        │  catalog → runModel → configureSimulator → step → terminationPolicy
        ▼
wasm engine (src/lib/ressim/pkg, built by scripts/build-wasm.sh)
```

- **Placement.** The server lives in its own directory, `tools/ressim-mcp/`, with its own
  `package.json` and the MCP SDK as its only new dependency. The app bundle and the `@ressim/*`
  packages never import it. Whether it becomes a pnpm workspace member is decided in M1; if it
  does, `pnpm-workspace.yaml` gains one entry and `packageBoundaries.test.ts` gains its rule.
- **Module resolution.** App modules do not load under the plain Node ESM resolver (#31). The
  server is bundled with Vite (library/SSR build) into one Node entry plus the `.wasm` file, the
  same mechanism `run_wasm.mjs` relies on.
- **Scenario-agnostic.** The server never branches on a scenario key. Everything a tool reports
  about a case comes from the scenario's own declarations (`docs/SCENARIO_CATALOG_ARCHITECTURE.md`).
- **The browser app is unchanged** apart from the M0 refactor, which it shares.

## 3. Phases

Each phase ends in something usable or measurable. Each has an exit gate and none depends on a
later one.

### M0 — Headless black-oil session (prerequisite, useful on its own)

The black-oil run loop lives inline in `src/lib/workers/sim.worker.ts`: step, snapshot, rate
history, `evaluateTerminationPolicy`, stop handling. The compositional path already factored the
same loop into `workers/compositionalSession.ts`. M0 extracts a `BlackOilSession` with the same
shape (create from a payload, `run({ steps, dt, onState, shouldStop })`, snapshot, stop reason),
and the worker becomes a thin message adapter over both sessions.

- No behaviour change: the worker posts the same messages in the same order.
- Gate: `pnpm run check:cycles`, frontend unit tests, the termination-policy tests run against the
  session, and the e2e run smoke (`ressim-validation` frontend row).
- Independent value: the debug scripts in #31 and any future Node tooling get a run loop without
  copying the worker.

### M1 — Proof of concept: three tools, black-oil only

Tools: `list_scenarios`, `run_scenario` and `get_run_series`, for two or three curated
black-oil cases (one waterflood with a Buckley–Leverett reference, one depletion case).

- `run_scenario` takes a scenario key plus optional sensitivity dimension/variant keys. M1 has
  **no free-form parameter overrides**; a run is something the app can already start.
- Results are summaries (§4.2). Series come only from `get_run_series`, and its size is bounded.
- **Measure:** wall time of a few catalog runs, wasm under Node against `ressim-py` natively, on
  one committed revision. Record the numbers with the commit hash and exact commands in this
  document, marked provisional, per AGENTS.md evidence rules. This decides whether M4 is needed.
- Gate: a scripted MCP client test (SDK client over stdio) that lists, runs and fetches a series,
  and checks the run's final summary against the app's own `buildRunResult` for the same spec.
  The same spec must give the same numbers.

### M2 — Experiment interface

- `describe_scenario`: physics, solver policy and its rationale, sensitivity dimensions,
  termination policy, analytical method plus its validity summary, and declared reference
  sources. Everything comes from the scenario definition, nothing is restated.
- `compare_runs`: two or more runs, plus the analytical overlay or committed reference curve
  wherever the scenario declares one, through `analyticalMethodRegistry` and `opmFlowArtifacts`.
- **Parameter overrides** within the bounds `validateInputs.ts` and the warning policy already
  enforce for Custom Mode. An override the app would reject is rejected with the app's message.
  An override that leaves an analytical method's assumptions makes the comparison mark that
  reference as not applicable instead of reporting a deviation from it.
- **Sweeps:** `run_scenario` accepts a short list of variants or override sets, runs them
  sequentially under the §4.3 limits, and returns one summary row per run.
- Compositional scenarios join through `CompositionalSession`. The two withheld cases
  (`comp_spe5_wag`, `comp_spe3_gas_cycling`) stay hidden exactly as they are in the picker.
- Gate: client tests for each tool; one test where an out-of-validity override must yield
  "reference not applicable".

### M3 — Resources and provenance

- MCP resources: the scenario catalog, `docs/BENCHMARKS.md`, the validation documents indexed in
  `docs/DOCUMENTATION_INDEX.md`, and `docs/SCIENTIFIC_LIMITATIONS.md`, served read-only from the
  repository as committed, never regenerated by the server.
- Every run result carries provenance: git commit and dirty flag, engine target (`wasm32`), solver
  (IMPES/FIM/compositional), the resolved run spec, and wall time.
- Gate: client test that a run on a dirty tree is labelled dirty.

### M4 — Optional native backend (only if M1's measurement asks for it)

A backend interface behind the same tools: the wasm session by default, `ressim-py` as an opt-in
for black-oil runs, fed by the recorded call list `catalogCases.ts` already produces. Analytical
comparison and termination stay in TypeScript. The native side returns raw series and nothing
else. The parity gate is the correctness argument, and the run's provenance names which backend
ran it.

### Out of scope

A hosted or remote server. GitHub Pages cannot run one, and it would need hosting, authentication
and resource policing that a local stdio tool does not. Revisit it only if a real external user
needs it, under a separate issue.

## 4. Contracts the server keeps

### 4.1 Tools are experiment-level

There is no tool per engine setter. An agent reaches the engine only through a scenario, its
declared variants, and validated overrides (M2).

### 4.2 Compact results

- `run_scenario` returns a summary: final time, stop reason (completed / termination policy /
  solver failure / limit), recovery factor, cumulative phase volumes, breakthrough time where the
  scenario defines one, solver warnings, step and Newton counts, and a `run_id`.
- `get_run_series(run_id, series[], max_points)` returns named time series, downsampled to
  `max_points` (hard cap) with the downsampling stated in the response.
- Full 3D grids are never returned. An end-of-run field is available only as named per-cell
  arrays, under a cell-count cap.

### 4.3 Limits are enforced, not advised

There are server-side caps on cells per run, runs per call, report steps, and wall time per run.
A run that hits a cap stops and says which cap, with the partial summary. Runs are kept in memory
by `run_id` with a small LRU bound, and nothing is written outside a declared output directory.

### 4.4 Reference honesty

- An analytical comparison is reported only inside the method's validity summary. Outside it, the
  response says "not applicable" and why.
- OPM Flow and JutulDarcy curves are **committed artifacts**. The response names them as such,
  with the artifact path and the deck or commit it came from. The server never runs Flow, and
  never presents a committed curve as a fresh external run.
- Benchmark numbers come from `docs/BENCHMARKS.md` as committed. The server never computes or
  restates a "current baseline" (AGENTS.md evidence discipline).

## 5. Alternatives considered

| Alternative | Why not (now) |
|---|---|
| Python server on `ressim-py` + generated manifest (the original proposal) | Ports the analytical, termination and warning logic. Cannot run compositional. Kept as M4's optional backend, not the front door. |
| MCP inside the browser app or the wasm engine | Couples the product to an agent protocol and violates the engine's target-agnostic boundary (`ENGINE_PAYLOAD_BOUNDARY_DESIGN_2026-09-21.md`). |
| One tool per engine setter | Lets an agent build a model the catalog never validated, with no reference and no limits. |
| Rust-native MCP server | Would reach the engine directly, but the catalog, analytical methods and policies would still be TypeScript, so it has the Python problem in another language. |
| Do nothing; agents use the repo and skills | Already covers *developing* ResSim. It does not let an agent *use* the simulator as an instrument, which is what this adds. |

## 6. Open questions, decided in the named phase

- Workspace membership and the exact SDK version pin (M1). The MCP SDKs move quickly; pin a
  release that is at least two weeks old and record it here.
- Default values of the §4.3 caps (M1, from the measured run times).
- Whether `run_id` results survive a server restart. Default: no (M1).
