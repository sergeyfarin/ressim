<script lang="ts">
    import { onMount, onDestroy } from 'svelte';
    import FractionalFlow from './lib/FractionalFlow.svelte';

    let wasmReady = false;
    let simWorker: Worker | null = null;
    let runCompleted = false;
    let workerRunning = false;

    // UI inputs
    let nx = 15;
    let ny = 10;
    let nz = 10;
    let delta_t_days = 0.25;
    let steps = 5;

    // --- NEW STATE VARIABLES ---

    // Initial Conditions
    let initialPressure = 300.0;
    let initialSaturation = 0.3;

    // Relative Permeability
    let s_wc = 0.1;
    let s_or = 0.1;
    let n_w = 2.0;
    let n_o = 2.0;

    // Permeability
    let permMode = 'default'; // 'default', 'random', 'perLayer'
    let minPerm = 50.0;
    let maxPerm = 200.0;
    let useRandomSeed = true;
    let randomSeed = 12345;
    let layerPermsXStr = "100, 150, 50, 200, 120, 1000, 90, 110, 130, 70";
    let layerPermsYStr = "100, 150, 50, 200, 120, 1000, 90, 110, 130, 70";
    let layerPermsZStr = "10, 15, 5, 20, 12, 8, 9, 11, 13, 7";
    let scenarioPreset = 'custom';

    // Well inputs
    let well_radius = 0.1;
    let well_skin = 0.0;

    // Stability
    let max_sat_change_per_step = 0.1;
    let gravityEnabled = false;
    let capillaryEnabled = true;
    let capillaryPEntry = 5.0;
    let capillaryLambda = 2.0;

    // Display data
    let gridStateRaw = null;
    let wellStateRaw = null;
    let simTime = 0;
    let rateHistory = [];
    let analyticalProductionData = [];

    type BenchmarkRow = {
        name: string;
        pvBtSim: number;
        pvBtRef: number;
        relError: number;
        tolerance: number;
    };

    type BenchmarkMode = 'baseline' | 'refined';
    type BenchmarkModes = Record<BenchmarkMode, BenchmarkRow[]>;

    type WorkerProfile = {
        batchMs: number;
        avgStepMs: number;
        simStepMs: number;
        extractMs: number;
        snapshotsSent: number;
    };

    type ProfileStats = {
        batchMs: number;
        avgStepMs: number;
        extractMs: number;
        renderApplyMs: number;
        snapshotsSent: number;
    };

    type BenchmarkArtifact = {
        generatedAt?: string;
        source?: string;
        command?: string;
        defaultMode?: BenchmarkMode;
        modes?: Partial<BenchmarkModes>;
        cases?: BenchmarkRow[];
    };

    const fallbackBenchmarkModes: BenchmarkModes = {
        baseline: [
            {
                name: 'BL-Case-A',
                pvBtSim: 0.5239,
                pvBtRef: 0.5860,
                relError: 0.106,
                tolerance: 0.25,
            },
            {
                name: 'BL-Case-B',
                pvBtSim: 0.4768,
                pvBtRef: 0.5074,
                relError: 0.060,
                tolerance: 0.30,
            },
        ],
        refined: [
            {
                name: 'BL-Case-A-Refined',
                pvBtSim: 0.5649,
                pvBtRef: 0.5860,
                relError: 0.036,
                tolerance: 0.25,
            },
            {
                name: 'BL-Case-B-Refined',
                pvBtSim: 0.4907,
                pvBtRef: 0.5074,
                relError: 0.033,
                tolerance: 0.30,
            },
        ],
    };

    let benchmarkModes: BenchmarkModes = {
        baseline: [...fallbackBenchmarkModes.baseline],
        refined: [...fallbackBenchmarkModes.refined],
    };
    let selectedBenchmarkMode: BenchmarkMode = 'baseline';
    let benchmarkSource = 'fallback';
    let benchmarkGeneratedAt = '';

    const benchmarkModeLabel: Record<BenchmarkMode, string> = {
        baseline: 'Baseline (nx=24, dt=0.5 day)',
        refined: 'Refined (nx=96, dt=0.125 day)',
    };

    $: benchmarkRows = benchmarkModes[selectedBenchmarkMode] ?? [];

    $: baselineRelErrByCase = new Map(
        (benchmarkModes.baseline ?? []).map((row) => [row.name.replace('-Refined', ''), row.relError])
    );

    $: benchmarkRowsWithStatus = benchmarkRows.map((row) => ({
        ...row,
        passes: row.relError <= row.tolerance,
        improvementVsBaselinePp: (() => {
            const baseKey = row.name.replace('-Refined', '');
            const baselineRelErr = baselineRelErrByCase.get(baseKey);
            if (!Number.isFinite(baselineRelErr)) return null;
            return (baselineRelErr - row.relError) * 100.0;
        })(),
    }));

    $: allBenchmarksPass = benchmarkRowsWithStatus.every((row) => row.passes);

    // History / replay
    let history = [];
    let currentIndex = -1;
    let playing = false;
    let playSpeed = 2;
    let playTimer = null;
    const HISTORY_RECORD_INTERVAL = 2;
    const MAX_HISTORY_ENTRIES = 300;
    let showDebugState = false;
    let profileStats: ProfileStats = {
        batchMs: 0,
        avgStepMs: 0,
        extractMs: 0,
        renderApplyMs: 0,
        snapshotsSent: 0,
    };
    let ThreeDViewComponent = null;
    let RateChartComponent = null;

    // Visualization
    let showProperty: 'pressure' | 'saturation_water' | 'saturation_oil' | 'permeability_x' | 'permeability_y' | 'permeability_z' | 'porosity' = 'pressure';
    let legendRangeMode: 'fixed' | 'percentile' = 'fixed';
    let legendPercentileLow = 5;
    let legendPercentileHigh = 95;

    const scenarioPresets = {
        custom: null,
        baseline_waterflood: {
            initialPressure: 300,
            initialSaturation: 0.3,
            s_wc: 0.1,
            s_or: 0.1,
            n_w: 2.0,
            n_o: 2.0,
            capillaryEnabled: true,
            capillaryPEntry: 5.0,
            capillaryLambda: 2.0,
            permMode: 'random',
            minPerm: 50,
            maxPerm: 200,
            useRandomSeed: true,
            randomSeed: 12345,
        },
        high_contrast_layers: {
            initialPressure: 320,
            initialSaturation: 0.25,
            s_wc: 0.12,
            s_or: 0.12,
            n_w: 2.2,
            n_o: 2.2,
            capillaryEnabled: true,
            capillaryPEntry: 8.0,
            capillaryLambda: 2.5,
            permMode: 'perLayer',
            layerPermsXStr: '30, 40, 60, 90, 150, 400, 150, 90, 60, 40',
            layerPermsYStr: '30, 40, 60, 90, 150, 400, 150, 90, 60, 40',
            layerPermsZStr: '3, 4, 6, 9, 15, 40, 15, 9, 6, 4',
        },
        viscous_fingering_risk: {
            initialPressure: 280,
            initialSaturation: 0.2,
            s_wc: 0.08,
            s_or: 0.15,
            n_w: 1.6,
            n_o: 2.4,
            capillaryEnabled: true,
            capillaryPEntry: 3.0,
            capillaryLambda: 1.6,
            permMode: 'random',
            minPerm: 20,
            maxPerm: 500,
            useRandomSeed: true,
            randomSeed: 987654,
        },
    };

    function applyScenarioPreset() {
        const preset = scenarioPresets[scenarioPreset];
        if (!preset) return;

        if (preset.initialPressure !== undefined) initialPressure = preset.initialPressure;
        if (preset.initialSaturation !== undefined) initialSaturation = preset.initialSaturation;
        if (preset.s_wc !== undefined) s_wc = preset.s_wc;
        if (preset.s_or !== undefined) s_or = preset.s_or;
        if (preset.n_w !== undefined) n_w = preset.n_w;
        if (preset.n_o !== undefined) n_o = preset.n_o;
        if (preset.capillaryEnabled !== undefined) capillaryEnabled = preset.capillaryEnabled;
        if (preset.capillaryPEntry !== undefined) capillaryPEntry = preset.capillaryPEntry;
        if (preset.capillaryLambda !== undefined) capillaryLambda = preset.capillaryLambda;
        if (preset.permMode !== undefined) permMode = preset.permMode;
        if (preset.minPerm !== undefined) minPerm = preset.minPerm;
        if (preset.maxPerm !== undefined) maxPerm = preset.maxPerm;
        if (preset.useRandomSeed !== undefined) useRandomSeed = preset.useRandomSeed;
        if (preset.randomSeed !== undefined) randomSeed = preset.randomSeed;
        if (preset.layerPermsXStr !== undefined) layerPermsXStr = preset.layerPermsXStr;
        if (preset.layerPermsYStr !== undefined) layerPermsYStr = preset.layerPermsYStr;
        if (preset.layerPermsZStr !== undefined) layerPermsZStr = preset.layerPermsZStr;
    }

    function pushHistoryEntry(entry) {
        history = [...history, entry];
        if (history.length > MAX_HISTORY_ENTRIES) {
            const overflow = history.length - MAX_HISTORY_ENTRIES;
            history = history.slice(overflow);
            currentIndex = Math.max(0, currentIndex - overflow);
        }
        currentIndex = history.length - 1;
    }

    function updateProfileStats(profile: Partial<WorkerProfile> = {}, renderApplyMs = 0) {
        profileStats = {
            batchMs: Number(profile.batchMs ?? profileStats.batchMs ?? 0),
            avgStepMs: Number(profile.avgStepMs ?? profile.simStepMs ?? profileStats.avgStepMs ?? 0),
            extractMs: Number(profile.extractMs ?? profileStats.extractMs ?? 0),
            renderApplyMs,
            snapshotsSent: Number(profile.snapshotsSent ?? profileStats.snapshotsSent ?? 0),
        };
    }

    function applyWorkerState(message) {
        const renderStart = performance.now();
        gridStateRaw = message.grid;
        wellStateRaw = message.wells;
        simTime = message.time;
        rateHistory = message.rateHistory;

        if (message.recordHistory) {
            pushHistoryEntry({
                time: message.time,
                grid: message.grid,
                wells: message.wells,
            });
        }

        updateProfileStats(message.profile, performance.now() - renderStart);
    }

    function handleWorkerMessage(event) {
        const { type, ...message } = event.data ?? {};
        if (type === 'ready') {
            wasmReady = true;
            initSimulator();
            return;
        }

        if (type === 'state') {
            applyWorkerState(message);
            return;
        }

        if (type === 'batchComplete') {
            workerRunning = false;
            runCompleted = true;
            updateProfileStats(message.profile, profileStats.renderApplyMs);
            applyHistoryIndex(history.length - 1);
            return;
        }

        if (type === 'error') {
            workerRunning = false;
            console.error('Simulation worker error:', message.message);
            alert(`Simulation error: ${message.message}`);
        }
    }

    function setupWorker() {
        simWorker = new Worker(new URL('./lib/sim.worker.js', import.meta.url), { type: 'module' });
        simWorker.onmessage = handleWorkerMessage;
        simWorker.postMessage({ type: 'init' });
    }

    async function loadUiModules() {
        try {
            const [threeDModule, rateChartModule] = await Promise.all([
                import('./lib/3dview.svelte'),
                import('./lib/RateChart.svelte'),
            ]);
            ThreeDViewComponent = threeDModule.default;
            RateChartComponent = rateChartModule.default;
        } catch (error) {
            console.error('Failed to load UI modules:', error);
        }
    }

    onMount(() => {
        setupWorker();
        loadUiModules();
        loadBenchmarkResults();
    });

    onDestroy(() => {
        stopPlaying();
        if (simWorker) {
            simWorker.postMessage({ type: 'dispose' });
            simWorker.terminate();
            simWorker = null;
        }
    });

    function initSimulator() {
        if (!wasmReady || !simWorker) {
            alert('WASM not ready yet');
            return;
        }

        history = [];
        currentIndex = -1;
        runCompleted = false;

        simWorker.postMessage({
            type: 'create',
            payload: {
                nx: Number(nx),
                ny: Number(ny),
                nz: Number(nz),
                initialPressure: Number(initialPressure),
                initialSaturation: Number(initialSaturation),
                s_wc: Number(s_wc),
                s_or: Number(s_or),
                n_w: Number(n_w),
                n_o: Number(n_o),
                max_sat_change_per_step: Number(max_sat_change_per_step),
                capillaryEnabled: Boolean(capillaryEnabled),
                capillaryPEntry: Number(capillaryPEntry),
                capillaryLambda: Number(capillaryLambda),
                gravityEnabled: Boolean(gravityEnabled),
                permMode,
                minPerm: Number(minPerm),
                maxPerm: Number(maxPerm),
                useRandomSeed: Boolean(useRandomSeed),
                randomSeed: Number(randomSeed),
                permsX: layerPermsXStr.split(',').map(Number),
                permsY: layerPermsYStr.split(',').map(Number),
                permsZ: layerPermsZStr.split(',').map(Number),
                well_radius: Number(well_radius),
                well_skin: Number(well_skin),
            }
        });
        runSteps();
    }

    async function loadBenchmarkResults() {
        try {
            const response = await fetch('/benchmark-results.json', { cache: 'no-store' });
            if (!response.ok) return;

            const artifact = (await response.json()) as BenchmarkArtifact;
            const normalizeRows = (rows: BenchmarkRow[] | undefined): BenchmarkRow[] => (rows ?? [])
                .map((row) => ({
                    name: String(row.name),
                    pvBtSim: Number(row.pvBtSim),
                    pvBtRef: Number(row.pvBtRef),
                    relError: Number(row.relError),
                    tolerance: Number(row.tolerance),
                }))
                .filter((row) =>
                    row.name.length > 0 &&
                    Number.isFinite(row.pvBtSim) &&
                    Number.isFinite(row.pvBtRef) &&
                    Number.isFinite(row.relError) &&
                    Number.isFinite(row.tolerance)
                );

            const normalizedBaseline = normalizeRows(artifact.modes?.baseline ?? artifact.cases);
            const normalizedRefined = normalizeRows(artifact.modes?.refined);

            if (normalizedBaseline.length > 0) {
                benchmarkModes = {
                    baseline: normalizedBaseline,
                    refined: normalizedRefined.length > 0 ? normalizedRefined : benchmarkModes.refined,
                };
                selectedBenchmarkMode = artifact.defaultMode ?? 'baseline';
                benchmarkSource = artifact.source ? String(artifact.source) : 'artifact';
                benchmarkGeneratedAt = artifact.generatedAt ? String(artifact.generatedAt) : '';
            }
        } catch (error) {
            console.warn('Failed to load benchmark-results artifact, using fallback values.', error);
        }
    }

    // function addWell() {
    //     if (!simulator) return;
    //     try {
    //         simulator.add_well(Number(well_i), Number(well_j), Number(well_k), Number(well_rate), Boolean(is_injector));
    //         refreshViews();
    //     } catch (e) {
    //         console.warn('add_well call failed (check wasm signature):', e);
    //     }
    // }

    function stepOnce() {
        if (!simWorker || workerRunning) return;
        workerRunning = true;
        simWorker.postMessage({
            type: 'run',
            payload: {
                steps: 1,
                deltaTDays: Number(delta_t_days),
                historyInterval: 1,
            }
        });
    }

    function runSteps() {
        if (!simWorker || workerRunning) return;
        workerRunning = true;
        simWorker.postMessage({
            type: 'run',
            payload: {
                steps: Number(steps),
                deltaTDays: Number(delta_t_days),
                historyInterval: HISTORY_RECORD_INTERVAL,
            }
        });
    }



    /* Playback controls */
    function play() {
        if (history.length === 0) return;
        playing = true;
        stopPlaying();
        playTimer = setInterval(() => {
            next();
            if (currentIndex >= history.length - 1) {
                stopPlaying();
            }
        }, 1000 / playSpeed);
    }

    function stopPlaying() {
        playing = false;
        if (playTimer) {
            clearInterval(playTimer);
            playTimer = null;
        }
    }

    function togglePlay() {
        if (playing) stopPlaying(); else play();
    }

    function next() {
        if (history.length === 0) return;
        currentIndex = Math.min(history.length - 1, currentIndex + 1);
        applyHistoryIndex(currentIndex);
    }

    function prev() {
        if (history.length === 0) return;
        currentIndex = Math.max(0, currentIndex - 1);
        applyHistoryIndex(currentIndex);
    }

    function applyHistoryIndex(idx) {
        if (idx < 0 || idx >= history.length) return;
        const entry = history[idx];
        gridStateRaw = entry.grid;
        wellStateRaw = entry.wells;
        simTime = entry.time;
        // We don't update rateHistory here, as it's cumulative
    }

    
</script>
<main class="min-h-screen bg-base-200">
<FractionalFlow
    rockProps={{ s_wc, s_or, n_w, n_o }}
    fluidProps={{ mu_w: 0.5, mu_o: 1.0 }}
    timeHistory={rateHistory.map((point) => point.time)}
    injectionRate={rateHistory.find(r => r.total_injection > 0)?.total_injection ?? 0}
    reservoir={{ length: nx * 10, area: ny * 10 * nz * 1, porosity: 0.2 }}
    on:analyticalData={(e) => analyticalProductionData = e.detail.production}
/>
<h1 class="text-4xl font-bold mb-6">A Simplified Reservoir Simulation Model</h1>

    <div class="grid grid-cols-2 gap-4">
        <div class="grid grid-cols-2 gap-4">
            <div class="bg-blue-200">
                <h3>Reservoir Properties</h3>                
                <div>
                    <br />
                    <label class="form-control">
                        <span class="label-text">Scenario Preset</span>
                        <select class="select select-bordered w-1/2" bind:value={scenarioPreset} on:change={applyScenarioPreset}>
                            <option value="custom">Custom</option>
                            <option value="baseline_waterflood">Baseline Waterflood</option>
                            <option value="high_contrast_layers">High Contrast Layers</option>
                            <option value="viscous_fingering_risk">Viscous Fingering Risk</option>
                        </select>
                    </label>
                </div>
                <div>
                    <br />
                    <label class="form-control">
                        <span class="label-text">Pressure (bar)</span>
                        <input type="number" step="10" class="input input-bordered w-1/4" bind:value={initialPressure} />
                    </label>
                </div>
                <div>
                    <br />
                    <label class="form-control">
                        <span class="label-text">Water Saturation</span>
                        <input type="number" step="0.05" class="input input-bordered w-1/4" bind:value={initialSaturation} />
                    </label>
                </div>
                <div>
                    <br />
                    <label class="label cursor-pointer justify-start gap-3">
                        <input type="checkbox" class="checkbox checkbox-sm" bind:checked={gravityEnabled} />
                        <span class="label-text">Enable gravity terms</span>
                    </label>
                </div>

            </div>
            
            <div>
                <h4>Rel. Permeability</h4>
                <label class="form-control w-full">
                    <span class="label-text">S_wc</span>
                    <input type="number" step="0.05" class="input input-bordered" bind:value={s_wc} />
                </label>
                <label class="form-control w-full">
                    <span class="label-text">S_or</span>
                    <input type="number" step="0.05" class="input input-bordered" bind:value={s_or} />
                </label>
                <label class="form-control w-full">
                    <span class="label-text">n_w</span>
                    <input type="number" step="0.1" class="input input-bordered" bind:value={n_w} />
                </label>
                <label class="form-control w-full">
                    <span class="label-text">n_o</span>
                    <input type="number" step="0.1" class="input input-bordered" bind:value={n_o} />
                </label>

                <h4 class="mt-4">Capillary Pressure</h4>
                <label class="label cursor-pointer justify-start gap-3">
                    <input type="checkbox" class="checkbox checkbox-sm" bind:checked={capillaryEnabled} />
                    <span class="label-text">Enable capillary pressure</span>
                </label>
                <label class="form-control w-full">
                    <span class="label-text">P_entry (bar)</span>
                    <input type="number" min="0" step="0.1" class="input input-bordered" bind:value={capillaryPEntry} disabled={!capillaryEnabled} />
                </label>
                <label class="form-control w-full">
                    <span class="label-text">Lambda</span>
                    <input type="number" min="0.1" step="0.1" class="input input-bordered" bind:value={capillaryLambda} disabled={!capillaryEnabled} />
                </label>
            </div>
            <div class="col-span-2">
                <h4>Permeability</h4>
                <label class="form-control w-full">
                    <span class="label-text">Mode</span>
                    <select class="select select-bordered" bind:value={permMode}>
                        <option value="default">Default</option>
                        <option value="random">Random</option>
                        <option value="perLayer">Per Layer</option>
                    </select>
                </label>
                {#if permMode === 'random'}
                    <div>
                        <label class="form-control w-full">
                            <span class="label-text">Use Seeded Randomness</span>
                            <input type="checkbox" class="checkbox" bind:checked={useRandomSeed} />
                        </label>
                        {#if useRandomSeed}
                            <label class="form-control w-full">
                                <span class="label-text">Random Seed</span>
                                <input type="number" step="1" class="input input-bordered" bind:value={randomSeed} />
                            </label>
                        {/if}
                        <label class="form-control w-full">
                            <span class="label-text">Min Perm</span>
                            <input type="number" class="input input-bordered" bind:value={minPerm} />
                        </label>
                        <label class="form-control w-full">
                            <span class="label-text">Max Perm</span>
                            <input type="number" class="input input-bordered" bind:value={maxPerm} />
                        </label>
                    </div>
                {:else if permMode === 'perLayer'}
                    <div>
                        <label class="form-control w-full">
                            <span class="label-text">Perm X (by layer, csv)</span>
                            <input type="text" class="input input-bordered" bind:value={layerPermsXStr} />
                        </label>
                        <label class="form-control w-full">
                            <span class="label-text">Perm Y (by layer, csv)</span>
                            <input type="text" class="input input-bordered" bind:value={layerPermsYStr} />
                        </label>
                        <label class="form-control w-full">
                            <span class="label-text">Perm Z (by layer, csv)</span>
                            <input type="text" class="input input-bordered" bind:value={layerPermsZStr} />
                        </label>
                    </div>
                {/if}
            </div>
            <div>
                <h4>Well Properties</h4>
                <label class="form-control w-full">
                    <span class="label-text">Well Radius (m)</span>
                    <input type="number" step="0.01" class="input input-bordered" bind:value={well_radius} />
                </label>
                <label class="form-control w-full">
                    <span class="label-text">Skin</span>
                    <input type="number" step="0.1" class="input input-bordered" bind:value={well_skin} />
                </label>
            </div>
            <!-- <div>
                <h4>Stability</h4>
                <label class="form-control w-full">
                    <span class="label-text">Max Saturation Change</span>
                    <input type="number" step="0.01" class="input input-bordered" bind:value={max_sat_change_per_step} />
                </label>
            </div> -->
            <div class="controls">
            <span>{wasmReady ? 'WASM ready' : 'WASM loading...'}</span>
            <!-- <div>
                <label class="form-control w-full">
                    <span class="label-text">nx</span>
                    <input type="number" min="1" class="input input-bordered" bind:value={nx} />
                </label>
                <label class="form-control w-full">
                    <span class="label-text">ny</span>
                    <input type="number" min="1" class="input input-bordered" bind:value={ny} />
                </label>
                <label class="form-control w-full">
                    <span class="label-text">nz</span>
                    <input type="number" min="1" class="input input-bordered" bind:value={nz} />
                </label>
                <div class="row">
                    <button class="btn btn-primary" on:click={initSimulator}>Init Simulator</button>
                </div>
            </div> -->

            <div>
                <label class="form-control w-full">
                    <span class="label-text">delta_t_days</span>
                    <input type="number" step="0.1" class="input input-bordered" bind:value={delta_t_days} />
                </label>
                <label class="form-control w-full">
                    <span class="label-text">steps</span>
                    <input type="number" min="1" class="input input-bordered" bind:value={steps} />
                </label>
                <!-- <div class="row">
                    <button class="btn btn-secondary" on:click={stepOnce} disabled={!simulator}>Step & Record</button>
                    <button class="btn btn-secondary" on:click={runSteps} disabled={!simulator}>Run {steps} & Record</button>
                </div> -->
            </div>

            <div>
                <label class="label cursor-pointer justify-start gap-3 mb-2">
                    <input type="checkbox" class="checkbox checkbox-sm" bind:checked={showDebugState} />
                    <span class="label-text">Show raw debug state</span>
                </label>
                <!-- <h4>Replay</h4>
                <div class="row">
                    <button class="btn btn-outline" on:click={prev} disabled={history.length===0}>Prev</button>
                    <button class="btn btn-outline" on:click={togglePlay} disabled={history.length===0}>{playing ? 'Stop' : 'Play'}</button>
                    <button class="btn btn-outline" on:click={next} disabled={history.length===0}>Next</button>
                    <label class="form-control">
                        <span class="label-text">Speed</span>
                        <input type="number" min="0.1" step="0.1" class="input input-bordered" bind:value={playSpeed} />
                    </label>
                </div> -->
                <div style="display:flex; gap:0.5rem; align-items:center;">
                    <input type="range" class="range" min="0" max={Math.max(0, history.length-1)} bind:value={currentIndex} on:input={() => applyHistoryIndex(currentIndex)} style="flex:1;" />
                    <span style="min-width:80px;">Step: {currentIndex} / {history.length - 1}</span>
                </div>
                {#if history.length > 0 && currentIndex >= 0 && currentIndex < history.length}
                    <div style="color:#666; font-size:12px;">Time: {history[currentIndex].time.toFixed(2)} days</div>
                {/if}
            </div>
            <div>
                <h4>Visualization</h4>
                <label class="form-control w-full">
                    <span class="label-text">Property</span>
                    <select class="select select-bordered" bind:value={showProperty}>
                        <option value="pressure">Pressure</option>
                        <option value="saturation_water">Water Saturation</option>
                        <option value="saturation_oil">Oil Saturation</option>
                        <option value="permeability_x">Permeability X</option>
                        <option value="permeability_y">Permeability Y</option>
                        <option value="permeability_z">Permeability Z</option>
                        <option value="porosity">Porosity</option>
                    </select>
                </label>
                <label class="form-control w-full mt-2">
                    <span class="label-text">Legend Range Mode</span>
                    <select class="select select-bordered" bind:value={legendRangeMode}>
                        <option value="fixed">Fixed</option>
                        <option value="percentile">Percentile (adaptive)</option>
                    </select>
                </label>
                {#if legendRangeMode === 'percentile'}
                    <div class="grid grid-cols-2 gap-2 mt-2">
                        <label class="form-control w-full">
                            <span class="label-text">Low Percentile (%)</span>
                            <input type="number" min="0" max="99" step="1" class="input input-bordered" bind:value={legendPercentileLow} />
                        </label>
                        <label class="form-control w-full">
                            <span class="label-text">High Percentile (%)</span>
                            <input type="number" min="1" max="100" step="1" class="input input-bordered" bind:value={legendPercentileHigh} />
                        </label>
                    </div>
                {/if}
                <div>time: {simTime}</div>
                <div>recorded steps: {history.length}</div>
                <div>worker: {workerRunning ? 'running' : 'idle'}</div>
                <div>run completed: {runCompleted ? 'yes' : 'no'}</div>
                <div style="font-size:12px; color:#666; margin-top:4px;">
                    avg step: {profileStats.avgStepMs.toFixed(3)} ms · batch: {profileStats.batchMs.toFixed(1)} ms
                </div>
                <div style="font-size:12px; color:#666;">
                    state extract: {profileStats.extractMs.toFixed(3)} ms · apply: {profileStats.renderApplyMs.toFixed(3)} ms · snapshots: {profileStats.snapshotsSent}
                </div>
            </div>
        </div>
        

            
        </div>
        <div class="row" style="margin-top: 1rem;">
            {#if RateChartComponent}
                <svelte:component this={RateChartComponent} {rateHistory} {analyticalProductionData} />
            {:else}
                <div style="padding:0.75rem; color:#666; font-size:12px;">Loading rate chart…</div>
            {/if}
        </div>
    </div>
    <div class="viz-wrapper">
        {#if ThreeDViewComponent}
            <svelte:component
                this={ThreeDViewComponent}
                nx={nx}
                ny={ny}
                nz={nz}
                gridState={gridStateRaw}
                showProperty={showProperty}
                legendRangeMode={legendRangeMode}
                legendPercentileLow={legendPercentileLow}
                legendPercentileHigh={legendPercentileHigh}
                history={history}
                currentIndex={currentIndex}
                wellState={wellStateRaw}
            />
        {:else}
            <div style="height:600px; border:1px solid #ddd; background:#fff; display:flex; align-items:center; justify-content:center; color:#666; font-size:12px;">
                Loading 3D view…
            </div>
        {/if}
    </div>
    {#if showDebugState}
        <div class="grid-well-wrapper">
            <div>
                <h4>Grid State (current)</h4>
                <pre>{JSON.stringify(gridStateRaw, null, 2)}</pre>
            </div>
            <div>
                <h4>Well State (current)</h4>
                <pre>{JSON.stringify(wellStateRaw, null, 2)}</pre>
            </div>
        </div>
    {/if}

    <div class="bg-base-100 p-4 mt-4 rounded-lg shadow-sm">
        <h3 class="text-xl font-semibold mb-2">P4-1 Benchmark Results</h3>
        <p class="text-sm mb-3">Published-reference Buckley-Leverett comparison (analytical vs simulation).</p>
        <div class="text-xs mb-3">Data source: {benchmarkSource}{benchmarkGeneratedAt ? `, generated: ${benchmarkGeneratedAt}` : ''}</div>
        <div class="join mb-3">
            <button
                class="btn btn-sm join-item"
                class:btn-active={selectedBenchmarkMode === 'baseline'}
                on:click={() => selectedBenchmarkMode = 'baseline'}
            >
                Baseline
            </button>
            <button
                class="btn btn-sm join-item"
                class:btn-active={selectedBenchmarkMode === 'refined'}
                on:click={() => selectedBenchmarkMode = 'refined'}
            >
                Refined
            </button>
        </div>
        <div class="text-xs mb-3">Showing: {benchmarkModeLabel[selectedBenchmarkMode]}</div>
        <div class="overflow-x-auto">
            <table class="table table-sm w-full">
                <thead>
                    <tr>
                        <th>Case</th>
                        <th>PV_BT_sim</th>
                        <th>PV_BT_ref</th>
                        <th>Relative Error</th>
                        <th>Improvement vs baseline</th>
                        <th>Tolerance</th>
                        <th>Status</th>
                    </tr>
                </thead>
                <tbody>
                    {#each benchmarkRowsWithStatus as row}
                        <tr>
                            <td>{row.name}</td>
                            <td>{row.pvBtSim.toFixed(4)}</td>
                            <td>{row.pvBtRef.toFixed(4)}</td>
                            <td>{(row.relError * 100).toFixed(1)}%</td>
                            <td>{row.improvementVsBaselinePp === null ? '-' : `${row.improvementVsBaselinePp.toFixed(1)} pp`}</td>
                            <td>{(row.tolerance * 100).toFixed(1)}%</td>
                            <td>{row.passes ? 'PASS' : 'FAIL'}</td>
                        </tr>
                    {/each}
                </tbody>
            </table>
        </div>
        <div class="text-sm mt-2">
            Overall status: {allBenchmarksPass ? 'PASS' : 'FAIL'}
        </div>
    </div>
</main>