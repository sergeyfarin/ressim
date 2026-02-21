/**
 * export-cases.mjs — Pre-run all scenario cases through the WASM simulator
 * and write JSON snapshots to public/cases/.
 *
 * Usage: npm run cases:export
 *
 * Each case gets a JSON file containing:
 * - params: the full resolved parameters
 * - rateHistory: array of rate-history points
 * - finalGrid: grid state at end of run
 * - finalWells: well state at end of run
 * - simTime: simulated time in days
 * - steps: number of steps run
 */

import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const rootDir = resolve(__dirname, '..');

// Import case catalog
const { caseCatalog, resolveParams } = await import(resolve(rootDir, 'src/lib/caseCatalog.ts'));

// Load WASM synchronously in Node.js
const wasmPath = resolve(rootDir, 'src/lib/ressim/pkg/simulator_bg.wasm');
const wasmBytes = readFileSync(wasmPath);

// Load the JS glue
const simulatorModule = await import(resolve(rootDir, 'src/lib/ressim/pkg/simulator.js'));
const { initSync, ReservoirSimulator, set_panic_hook } = simulatorModule;

// Initialize WASM
initSync({ module: wasmBytes });
set_panic_hook();

const outputDir = resolve(rootDir, 'public/cases');
mkdirSync(outputDir, { recursive: true });

/**
 * Build a create-payload that the configureSimulator function would accept
 * (mirroring sim.worker.js configureSimulator + App.svelte buildCreatePayload)
 */
function buildCreatePayload(caseParams) {
    const p = resolveParams(caseParams);
    const useUniformPerm = p.permMode === 'uniform';
    const nz = p.nz || 1;

    const permsX = useUniformPerm
        ? Array.from({ length: nz }, () => Number(p.uniformPermX || 100))
        : (p.layerPermsX || []).map(Number);
    const permsY = useUniformPerm
        ? Array.from({ length: nz }, () => Number(p.uniformPermY || 100))
        : (p.layerPermsY || []).map(Number);
    const permsZ = useUniformPerm
        ? Array.from({ length: nz }, () => Number(p.uniformPermZ || 10))
        : (p.layerPermsZ || []).map(Number);

    return {
        nx: Number(p.nx), ny: Number(p.ny), nz: Number(p.nz),
        cellDx: Number(p.cellDx), cellDy: Number(p.cellDy), cellDz: Number(p.cellDz),
        initialPressure: Number(p.initialPressure),
        initialSaturation: Number(p.initialSaturation),
        mu_w: Number(p.mu_w), mu_o: Number(p.mu_o),
        c_o: Number(p.c_o), c_w: Number(p.c_w),
        rock_compressibility: Number(p.rock_compressibility),
        depth_reference: Number(p.depth_reference),
        volume_expansion_o: Number(p.volume_expansion_o),
        volume_expansion_w: Number(p.volume_expansion_w),
        rho_w: Number(p.rho_w), rho_o: Number(p.rho_o),
        s_wc: Number(p.s_wc), s_or: Number(p.s_or),
        n_w: Number(p.n_w), n_o: Number(p.n_o),
        max_sat_change_per_step: Number(p.max_sat_change_per_step),
        max_pressure_change_per_step: Number(p.max_pressure_change_per_step),
        max_well_rate_change_fraction: Number(p.max_well_rate_change_fraction),
        capillaryEnabled: Boolean(p.capillaryEnabled),
        capillaryPEntry: Number(p.capillaryPEntry || 0),
        capillaryLambda: Number(p.capillaryLambda || 2),
        gravityEnabled: Boolean(p.gravityEnabled),
        permMode: useUniformPerm ? 'perLayer' : p.permMode,
        minPerm: Number(p.minPerm || 50),
        maxPerm: Number(p.maxPerm || 200),
        useRandomSeed: Boolean(p.useRandomSeed),
        randomSeed: Number(p.randomSeed || 0),
        permsX, permsY, permsZ,
        well_radius: Number(p.well_radius),
        well_skin: Number(p.well_skin),
        injectorBhp: Number(p.injectorBhp),
        producerBhp: Number(p.producerBhp),
        rateControlledWells: Boolean(p.rateControlledWells),
        injectorControlMode: p.injectorControlMode || 'pressure',
        producerControlMode: p.producerControlMode || 'pressure',
        injectorEnabled: p.injectorEnabled !== false,
        targetInjectorRate: Number(p.targetInjectorRate || 0),
        targetProducerRate: Number(p.targetProducerRate || 0),
        injectorI: Number(p.injectorI), injectorJ: Number(p.injectorJ),
        producerI: Number(p.producerI), producerJ: Number(p.producerJ),
    };
}

/**
 * Extract the SoA grid state from the simulator and convert TypedArrays to standard Arrays for JSON serialization.
 */
function getGridStateFromSim(sim) {
    return {
        pressure: Array.from(sim.getPressures()),
        sat_water: Array.from(sim.getSatWater()),
        sat_oil: Array.from(sim.getSatOil()),
        porosity: Array.from(sim.getPorosity()),
        perm_x: Array.from(sim.getPermX()),
        perm_y: Array.from(sim.getPermY()),
        perm_z: Array.from(sim.getPermZ()),
    };
}

/**
 * Configure and run a simulator for given payload (mirrors sim.worker.js configureSimulator)
 */
function runCase(payload, steps, deltaTDays) {
    const sim = new ReservoirSimulator(payload.nx, payload.ny, payload.nz);

    sim.setCellDimensions(payload.cellDx, payload.cellDy, payload.cellDz);
    sim.setFluidProperties(payload.mu_o, payload.mu_w);
    sim.setFluidCompressibilities(payload.c_o, payload.c_w);
    sim.setRockProperties(
        payload.rock_compressibility,
        payload.depth_reference,
        payload.volume_expansion_o,
        payload.volume_expansion_w
    );
    sim.setFluidDensities(payload.rho_o, payload.rho_w);
    sim.setInitialPressure(payload.initialPressure);
    sim.setInitialSaturation(payload.initialSaturation);

    const pEntry = payload.capillaryEnabled ? payload.capillaryPEntry : 0;
    sim.setCapillaryParams(pEntry, payload.capillaryLambda);
    sim.setGravityEnabled(payload.gravityEnabled);
    sim.setRelPermProps(payload.s_wc, payload.s_or, payload.n_w, payload.n_o);
    sim.setStabilityParams(
        payload.max_sat_change_per_step,
        payload.max_pressure_change_per_step,
        payload.max_well_rate_change_fraction
    );

    sim.setWellControlModes(
        payload.injectorControlMode,
        payload.producerControlMode
    );
    sim.setTargetWellRates(payload.targetInjectorRate, payload.targetProducerRate);

    const bhpMin = Math.min(payload.producerBhp, payload.injectorBhp);
    const bhpMax = Math.max(payload.producerBhp, payload.injectorBhp);
    sim.setWellBhpLimits(bhpMin, bhpMax);

    if (payload.permMode === 'random') {
        if (payload.useRandomSeed) {
            sim.setPermeabilityRandomSeeded(payload.minPerm, payload.maxPerm, BigInt(payload.randomSeed));
        } else {
            sim.setPermeabilityRandom(payload.minPerm, payload.maxPerm);
        }
    } else if (payload.permMode === 'perLayer') {
        sim.setPermeabilityPerLayer(
            new Float64Array(payload.permsX),
            new Float64Array(payload.permsY),
            new Float64Array(payload.permsZ)
        );
    }

    // Add wells
    const clamp = (v, max) => Math.max(0, Math.min(max - 1, Number(v)));
    const prodI = clamp(payload.producerI, payload.nx);
    const prodJ = clamp(payload.producerJ, payload.ny);
    const injI = clamp(payload.injectorI, payload.nx);
    const injJ = clamp(payload.injectorJ, payload.ny);

    for (let k = 0; k < payload.nz; k++) {
        sim.add_well(prodI, prodJ, k, payload.producerBhp, payload.well_radius, payload.well_skin, false);
    }
    if (payload.injectorEnabled) {
        for (let k = 0; k < payload.nz; k++) {
            sim.add_well(injI, injJ, k, payload.injectorBhp, payload.well_radius, payload.well_skin, true);
        }
    }

    // Collect history snapshots every N steps
    // Target ~4 steps of history visually so the scrub slider isn't overloaded + final state
    const historyInterval = Math.max(1, Math.floor(steps / 4));
    const history = [];

    for (let i = 0; i < steps; i++) {
        sim.step(deltaTDays);
        if (i % historyInterval === 0 || i === steps - 1) {
            history.push({
                time: sim.get_time(),
                grid: getGridStateFromSim(sim),
                wells: sim.getWellState(),
            });
        }
    }

    return {
        rateHistory: sim.getRateHistory(),
        finalGrid: getGridStateFromSim(sim),
        finalWells: sim.getWellState(),
        simTime: sim.get_time(),
        history,
    };
}

// Process all cases
const indexEntries = [];
let total = 0;
let success = 0;

for (const [catKey, category] of Object.entries(caseCatalog)) {
    for (const caseEntry of category.cases) {
        total++;
        const key = caseEntry.key;
        console.log(`  Running ${catKey}/${key}...`);

        try {
            const payload = buildCreatePayload(caseEntry.params);
            const steps = caseEntry.params.steps || 20;
            const dt = caseEntry.params.delta_t_days || 0.25;

            const result = runCase(payload, steps, dt);

            const caseData = {
                key,
                category: catKey,
                label: caseEntry.label,
                description: caseEntry.description,
                params: resolveParams(caseEntry.params),
                generatedAt: new Date().toISOString(),
                simTime: result.simTime,
                steps,
                rateHistory: result.rateHistory,
                finalGrid: result.finalGrid,
                finalWells: result.finalWells,
                history: result.history,
            };

            const outPath = resolve(outputDir, `${key}.json`);
            writeFileSync(outPath, JSON.stringify(caseData) + '\n', 'utf8');
            console.log(`    ✓ ${key}: ${result.simTime.toFixed(1)} days, ${result.rateHistory.length} rate points`);

            indexEntries.push({
                key,
                category: catKey,
                label: caseEntry.label,
                description: caseEntry.description,
                file: `${key}.json`,
            });
            success++;
        } catch (err) {
            console.error(`    ✗ ${key}: ${err.message}`);
        }
    }
}

// Write index
const index = {
    generatedAt: new Date().toISOString(),
    cases: indexEntries,
};
writeFileSync(resolve(outputDir, 'index.json'), JSON.stringify(index, null, 2) + '\n', 'utf8');

console.log(`\nDone: ${success}/${total} cases exported to ${outputDir}`);
if (success < total) process.exit(1);
