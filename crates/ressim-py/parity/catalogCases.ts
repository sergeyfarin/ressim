/**
 * Derives the catalog half of the parity matrix from the scenario catalog itself (#64).
 *
 * Every black-oil run the app can start — each scenario's base run, or every variant of every
 * sensitivity dimension — is turned into its create payload by the app's own path
 * (`buildScenarioRunSpecs` then `buildCreatePayloadForRun`) and configured through the worker's
 * own `configureReservoirSimulator`. That call is made against a recorder rather than a simulator,
 * so what comes out is the exact ordered list of setter calls the browser makes. Both runners then
 * replay that one list: the browser half through the wasm bindings, the native half through
 * `ressim-py`. Identical physical inputs are a property of the construction, not of two
 * hand-written setups being kept in step.
 *
 * A setter the native shim does not bind fails the native replay by name, so the catalog — not a
 * list somebody remembered to update — says what `ressim-py` must be able to express.
 *
 * Loaded by `run_wasm.mjs` through Vite's `runnerImport`; Node only, never imported by the app.
 */
import { listScenarios } from '../../../src/lib/catalog/scenarios';
import { isCompositionalCreate } from '../../../src/lib/compositional/createPayload';
import { buildCreatePayloadForRun, buildScenarioRunSpecs } from '../../../src/lib/scenario/runModel';
import { configureReservoirSimulator } from '../../../src/lib/workers/configureSimulator';
import type { ReservoirSimulator } from '../../../src/lib/ressim/pkg/simulator.js';

/**
 * Report steps each catalog run is replayed for. Parity is a property of the binding, so a
 * prefix of the run is enough to show both targets configured and stepped the same engine; the
 * full horizon is the scenario tests' business. Two steps rather than one so the second step
 * starts from a state the engine produced, not one the payload supplied.
 *
 * The prefix is the only thing the replay changes about a run: configuration is the recorded
 * call list, unaltered.
 */
export const CATALOG_STEP_CAP = 2;

/**
 * Longest step, in days, a catalog run is replayed with. The IMPES sweep cases report every five
 * days and substep to their CFL limit inside each report step, so two of their report steps cost
 * minutes of wall time while proving no more about the binding than two short ones. A run whose
 * report step is shorter keeps its own.
 */
export const CATALOG_DT_CAP_DAYS = 0.1;

/**
 * Runs left out of the replay matrix, by scenario key, with the reason. Everything else in the
 * catalog is in it.
 */
export const CATALOG_EXCLUSIONS: Record<string, string> = {
    // Decided by `isCompositionalCreate`, the worker's own routing; listed so the exclusion is
    // visible in the gate's output rather than implied by a filter.
    comp_co2_1d:
        'Compositional engine (CompositionalSimulator). ressim-py binds only ReservoirSimulator; '
        + 'compositional parity is validate-compositional.sh, not this gate.',
};

/** One recorded call: the browser method name and its arguments, JSON-safe (see `encodeArg`). */
export type RecordedCall = [method: string, args: unknown[]];

export type CatalogCase = {
    id: string;
    scenario: string;
    /** Every run spec key that configures to this exact call list and step plan. */
    runs: string[];
    nx: number;
    ny: number;
    nz: number;
    porosity: number;
    calls: RecordedCall[];
    steps: number;
    dtDays: number;
    fim: boolean;
};

/**
 * Make an argument survive JSON on its way to Python unchanged. Non-finite numbers (the worker
 * passes `NaN` for "no rate target"), bigints and typed arrays, which JSON cannot carry, become
 * tagged objects both replayers decode back to the same value and type.
 */
function encodeArg(value: unknown): unknown {
    if (typeof value === 'bigint') return { $bigint: value.toString() };
    if (typeof value === 'number' && !Number.isFinite(value)) return { $num: String(value) };
    if (value instanceof Float64Array) return { $f64: Array.from(value, encodeArg) };
    if (ArrayBuffer.isView(value)) throw new Error(`unsupported typed array ${value.constructor.name}`);
    if (Array.isArray(value)) return value.map(encodeArg);
    if (value && typeof value === 'object') {
        return Object.fromEntries(Object.entries(value).map(([k, v]) => [k, encodeArg(v)]));
    }
    return value;
}

/** A stand-in simulator that records every method call made on it, in order. */
function recorder(calls: RecordedCall[]): ReservoirSimulator {
    return new Proxy({}, {
        get: (_target, prop) => (...args: unknown[]) => {
            calls.push([String(prop), args.map(encodeArg)]);
        },
    }) as unknown as ReservoirSimulator;
}

/** Every black-oil run in the catalog as a replayable case, identical setups merged. */
export function buildCatalogCases(): { cases: CatalogCase[]; excluded: { run: string; reason: string }[] } {
    const byFingerprint = new Map<string, CatalogCase>();
    const excluded: { run: string; reason: string }[] = [];

    for (const scenario of listScenarios()) {
        const dimensions = scenario.sensitivities.length > 0 ? scenario.sensitivities : [null];
        for (const dimension of dimensions) {
            const specs = buildScenarioRunSpecs({
                scenarioKey: scenario.key,
                dimensionKey: dimension?.key ?? null,
                variantKeys: dimension ? dimension.variants.map((variant) => variant.key) : [],
            });
            for (const spec of specs) {
                const payload = buildCreatePayloadForRun(spec);
                if (isCompositionalCreate(payload) || CATALOG_EXCLUSIONS[scenario.key]) {
                    excluded.push({
                        run: spec.key,
                        reason: CATALOG_EXCLUSIONS[scenario.key] ?? 'compositional create payload',
                    });
                    continue;
                }
                const calls: RecordedCall[] = [];
                configureReservoirSimulator(recorder(calls), payload);
                const candidate: CatalogCase = {
                    id: spec.key,
                    scenario: scenario.key,
                    runs: [spec.key],
                    nx: payload.nx,
                    ny: payload.ny,
                    nz: payload.nz,
                    porosity: Number(payload.porosity),
                    calls,
                    steps: Math.min(spec.steps, CATALOG_STEP_CAP),
                    dtDays: Math.min(spec.deltaTDays, CATALOG_DT_CAP_DAYS),
                    fim: payload.fimEnabled === true,
                };
                // Many variants differ only in what the chart or the analytical overlay does with
                // the result; replaying the same engine input twice proves nothing new.
                const fingerprint = JSON.stringify([
                    candidate.nx, candidate.ny, candidate.nz, candidate.porosity,
                    candidate.calls, candidate.steps, candidate.dtDays,
                ]);
                const existing = byFingerprint.get(fingerprint);
                if (existing) existing.runs.push(spec.key);
                else byFingerprint.set(fingerprint, candidate);
            }
        }
    }
    return { cases: [...byFingerprint.values()], excluded };
}
