import fs from 'fs';
import path from 'path';
import zlib from 'zlib';
import { fileURLToPath } from 'url';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Read catalog
const catalogPath = path.join(__dirname, '../public/cases/catalog.json');
const catalog = JSON.parse(fs.readFileSync(catalogPath, 'utf-8'));

const outDir = path.join(__dirname, '../public/cases/prerun');
if (!fs.existsSync(outDir)) fs.mkdirSync(outDir, { recursive: true });

function formatTimeToken(days) {
    if (days >= 365) return `${(days / 365).toPrecision(3)} years`;
    if (days >= 30) return `${(days / 30).toPrecision(3)} months`;
    return `${days} days`;
}

function evaluateDisabilityRules(toggles, rules) {
    for (const rule of rules) {
        let conditionMet = true;
        for (const [key, value] of Object.entries(rule.when)) {
            if (Array.isArray(value)) {
                if (!value.includes(toggles[key])) conditionMet = false;
            } else {
                if (toggles[key] !== value) conditionMet = false;
            }
        }
        if (conditionMet) {
            for (const [key, disabledOptions] of Object.entries(rule.disable)) {
                if (disabledOptions.length === 0 || disabledOptions.includes(toggles[key])) {
                    return true; // Is disabled
                }
            }
        }
    }
    return false;
}

function computeWellPositions(params, geo, well) {
    const nx = params.nx || 1;
    const ny = params.ny || 1;

    let injI = 0, injJ = 0;
    let prodI = nx - 1, prodJ = ny - 1;

    if (geo === '1d') {
        injI = 0; injJ = 0;
        prodI = nx - 1; prodJ = 0;
    } else if (geo === '2dxz') {
        if (well === 'e2e') {
            injI = 0; injJ = 0;
            prodI = nx - 1; prodJ = 0;
        }
    } else {
        // 2D/3D Areal
        if (well === 'e2e' || well === 'corner') {
            injI = 0; injJ = 0;
            prodI = nx - 1; prodJ = ny - 1;
        } else if (well === 'center') {
            injI = 0; injJ = 0; // if injector present
            prodI = Math.floor(nx / 2); prodJ = Math.floor(ny / 2);
        } else if (well === 'offctr') {
            injI = 0; injJ = 0;
            prodI = Math.floor(nx / 4); prodJ = Math.floor(ny / 2);
        }
    }

    return { injectorI: injI, injectorJ: injJ, producerI: prodI, producerJ: prodJ };
}

function validatePhysicalCase(history, toggles, steps) {
    const warnings = [];
    if (!history || history.length === 0) return warnings;

    if (toggles.mode === 'wf') {
        const btStep = history.findIndex(h => h.water_cut > 0.01);
        if (btStep < 0) warnings.push('No water breakthrough detected by end of run');
        else if (btStep < steps * 0.1) warnings.push(`Very early breakthrough at step ${btStep}/${steps} (${(btStep / steps * 100).toFixed(0)}%)`);
        else if (btStep > steps * 0.9) warnings.push(`Very late breakthrough at step ${btStep}/${steps} (${(btStep / steps * 100).toFixed(0)}%)`);
    }

    if (toggles.mode === 'dep') {
        const p0 = history[0]?.avg_reservoir_pressure;
        const p1 = history[1]?.avg_reservoir_pressure;
        if (p0 != null && p1 != null && p0 - p1 > p0 * 0.20) {
            warnings.push('Pressure drops >20% in first step (too fast)');
        }

        const step20pct = Math.floor(steps * 0.2);
        const p20 = history[step20pct]?.avg_reservoir_pressure;
        if (p0 != null && p20 != null && p0 - p20 > p0 * 0.80) {
            warnings.push('Extensive depletion in first 20% of timesteps (too fast)');
        }
    }

    const last = history[history.length - 1];
    if (last) {
        const err = last.material_balance_error_m3;
        if (err !== undefined && Math.abs(err) > 1.0) {
            warnings.push(`High material balance error: ${err.toFixed(3)} m³`);
        }
    }

    return warnings;
}

// Generate all permutations
function generateAllCombinations(dimensions) {
    const results = [];
    const keys = dimensions.map(d => d.key);

    function helper(depth, current) {
        if (depth === dimensions.length) {
            results.push({ ...current });
            return;
        }
        const dim = dimensions[depth];
        for (const opt of dim.options) {
            current[dim.key] = opt.value;
            helper(depth + 1, current);
        }
    }

    helper(0, {});
    return results;
}

async function main() {
    console.log("Initializing WASM...");
    const simulatorModule = await import('../src/lib/ressim/pkg/simulator.js');
    const { default: init, set_panic_hook, ReservoirSimulator } = simulatorModule;

    const wasmPath = path.join(__dirname, '../src/lib/ressim/pkg/simulator_bg.wasm');
    const wasmBuffer = fs.readFileSync(wasmPath);
    await init(wasmBuffer);
    set_panic_hook();

    console.log("Generating valid combinations...");
    const theoreticalCombos = generateAllCombinations(catalog.dimensions);
    const validCombos = theoreticalCombos.filter(combo => !evaluateDisabilityRules(combo, catalog.disabilityRules));

    console.log(`Theoretical combinations: ${theoreticalCombos.length}`);
    console.log(`Valid combinations (after rules): ${validCombos.length}`);

    const casesToRun = [];

    // Map regular combos
    for (const combo of validCombos) {
        let params = { ...catalog.defaults };
        for (const dim of catalog.dimensions) {
            const opt = dim.options.find(o => o.value === combo[dim.key]);
            if (opt && opt.params) {
                params = { ...params, ...opt.params };
            }
        }
        const wells = computeWellPositions(params, combo.geo, combo.well);
        params = { ...params, ...wells };

        const keyParts = catalog.dimensions.map(d => `${d.key}-${combo[d.key]}`);
        const caseKey = keyParts.join('_');
        casesToRun.push({ key: caseKey, isBenchmark: false, toggles: combo, params });
    }

    // Add benchmarks
    for (const b of catalog.benchmarks) {
        const params = { ...catalog.defaults, ...b.params };
        casesToRun.push({ key: `bench_${b.key.replace(/_/g, '-')}`, isBenchmark: true, toggles: { mode: 'benchmark' }, params });
    }

    console.log(`Executing ${casesToRun.length} cases...`);
    let completed = 0;
    let failed = 0;

    for (const run of casesToRun) {
        const { key, params } = run;
        try {
            const nx = Number(params.nx || 1), ny = Number(params.ny || 1), nz = Number(params.nz || 1);
            const cellCount = nx * ny * nz;

            const runtime = new ReservoirSimulator(nx, ny, nz, Number(params.reservoirPorosity || 0.2));
            runtime.setCellDimensions(Number(params.cellDx || 10), Number(params.cellDy || 10), Number(params.cellDz || 10));
            runtime.setFluidProperties(Number(params.mu_o || 1.0), Number(params.mu_w || 0.5));
            runtime.setFluidCompressibilities(Number(params.c_o || 1e-5), Number(params.c_w || 3e-6));
            runtime.setFluidDensities(Number(params.rho_o || 800), Number(params.rho_w || 1000));

            const p_entry = Boolean(params.capillaryEnabled) ? Number(params.capillaryPEntry || 0) : 0;
            runtime.setCapillaryParams(p_entry, Number(params.capillaryLambda || 2.5));

            runtime.setGravityEnabled(Boolean(params.gravityEnabled));

            runtime.setRockProperties(Number(params.c_r || 1e-6), Number(params.depth_reference || 0), 1.0, 1.0); // b_o, b_w = 1.0 standard

            runtime.setRelPermProps(
                Number(params.s_wc || 0.1), Number(params.s_or || 0.1),
                Number(params.n_w || 2), Number(params.n_o || 2),
                Number(params.k_rw_max || 1.0), Number(params.k_ro_max || 1.0)
            );

            runtime.setStabilityParams(
                Number(params.max_sat_change_per_step || 0.1),
                50.0,
                0.2
            );

            runtime.setWellControlModes(String(params.injectorControlMode || 'pressure'), String(params.producerControlMode || 'pressure'));

            const targetInjectorRate = Number(params.targetInjectorRate || 0);
            runtime.setTargetWellRates(targetInjectorRate, targetInjectorRate);

            runtime.setWellBhpLimits(0, Math.max(Number(params.producerBhp || 100), Number(params.injectorBhp || 500)));

            if (params.permMode === 'random') {
                runtime.setPermeabilityRandomSeeded(Number(params.minPerm || 10), Number(params.maxPerm || 500), BigInt(42));
            } else if (params.permMode === 'perLayer') {
                const px = params.layerPermsX || Array(nz).fill(200);
                const py = params.layerPermsY || Array(nz).fill(200);
                const pz = params.layerPermsZ || Array(nz).fill(20);
                while (px.length < nz) px.push(px[px.length - 1]);
                while (py.length < nz) py.push(py[py.length - 1]);
                while (pz.length < nz) pz.push(pz[pz.length - 1]);
                runtime.setPermeabilityPerLayer(new Float64Array(px.slice(0, nz)), new Float64Array(py.slice(0, nz)), new Float64Array(pz.slice(0, nz)));
            } else {
                const kx = Number(params.uniformPermX || 200);
                const ky = Number(params.uniformPermY || kx);
                const kz = Number(params.uniformPermZ || kx * 0.1);
                runtime.setPermeabilityPerLayer(
                    new Float64Array(Array(nz).fill(kx)),
                    new Float64Array(Array(nz).fill(ky)),
                    new Float64Array(Array(nz).fill(kz))
                );
            }

            runtime.setInitialPressure(Number(params.p_initial || 300));
            runtime.setInitialSaturation(Number(params.s_wc || 0.0));

            for (let i = 0; i < nz; i++) {
                runtime.add_well(Number(params.producerI || 0), Number(params.producerJ || 0), i, Number(params.producerBhp || 100), Number(params.well_radius || 0.1), Number(params.skin_factor || 0), false);
            }
            if (params.injectorEnabled !== false) {
                for (let i = 0; i < nz; i++) {
                    runtime.add_well(Number(params.injectorI || 0), Number(params.injectorJ || 0), i, Number(params.injectorBhp || 500), Number(params.well_radius || 0.1), Number(params.skin_factor || 0), true);
                }
            }

            const history = [];

            // Generate sequence
            history.push({ time: Number(runtime.get_time()), grid: null, wells: runtime.getWellState() });

            const steps = Number(params.steps || 30);
            for (let i = 0; i < steps; i++) {
                runtime.step(Number(params.delta_t_days || 1.0));

                let gridState = undefined;
                if (cellCount <= 100 || i === steps - 1 || i % 10 === 0) {
                    gridState = { pressure: Array.from(runtime.getPressures()), sat_water: Array.from(runtime.getSatWater()) };
                }

                const rh = runtime.getRateHistory();
                const lastRh = Array.isArray(rh) ? rh[rh.length - 1] : null;
                const water_cut = lastRh && lastRh.producer_water_rate !== undefined && lastRh.producer_oil_rate !== undefined ?
                    (lastRh.producer_water_rate / (lastRh.producer_oil_rate + lastRh.producer_water_rate || 1)) : 0;

                history.push({
                    time: Number(runtime.get_time()),
                    grid: gridState,
                    wells: runtime.getWellState(),
                    avg_reservoir_pressure,
                    water_cut,
                    material_balance_error_m3: runtime.cumulative_mb_error_m3
                });
            }

            const finalGrid = { pressure: Array.from(runtime.getPressures()), sat_water: Array.from(runtime.getSatWater()) };
            const warnings = validatePhysicalCase(history, run.toggles, steps);

            const outData = {
                simTime: Number(runtime.get_time()),
                finalGrid,
                history,
                rateHistory: runtime.getRateHistory(),
                warnings
            };

            const outPath = path.join(prerunDir, `${key}.json.gz`);
            const compressedContext = zlib.gzipSync(JSON.stringify(outData));
            fs.writeFileSync(outPath, compressedContext);

            completed++;
            if (completed % 50 === 0) console.log(`  Completed ${completed}/${casesToRun.length}`);
        } catch (e) {
            console.error(`Failed to run case ${key}: ${e}`);
            failed++;
        }
    }

    console.log(`\nFinished! \n  Completed: ${completed}\n  Failed: ${failed}`);
}

main().catch(console.error);
