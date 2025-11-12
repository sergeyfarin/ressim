<script lang="ts">
    import { onMount, onDestroy, tick } from 'svelte';
    import init, { ReservoirSimulator } from './lib/ressim/pkg/simulator.js';
    import ThreeDView from './lib/3dview.svelte';
    import RateChart from './lib/RateChart.svelte';
    // import FractionalFlow from './lib/FractionalFlow.svelte';

    let wasmReady = false;
    let simulator = null;
    let runCompleted = false;

    // UI inputs
    let nx = 15;
    let ny = 10;
    let nz = 10;
    let delta_t_days = 0.25;
    let steps = 400;

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
    let layerPermsXStr = "100, 150, 50, 200, 120, 80, 90, 110, 130, 70";
    let layerPermsYStr = "100, 150, 50, 200, 120, 80, 90, 110, 130, 70";
    let layerPermsZStr = "10, 15, 5, 20, 12, 8, 9, 11, 13, 7";

    // Well inputs
    let well_radius = 0.1;
    let well_skin = 0.0;

    // Stability
    let max_sat_change_per_step = 0.1;

    // Display data
    let gridStateRaw = null;
    let wellStateRaw = null;
    let simTime = 0;
    let rateHistory = [];
    let analyticalProductionData = [];

    // History / replay
    let history = [];
    let currentIndex = -1;
    let playing = false;
    let playSpeed = 2;
    let playTimer = null;

    // Visualization
    let showProperty: 'pressure' | 'saturation_water' | 'saturation_oil' | 'permeability_x' | 'permeability_y' | 'permeability_z' | 'porosity' = 'pressure';

    onMount(async () => {
        await init();
        wasmReady = true;
        initSimulator();
        

    });

    onDestroy(() => {
        if (wasmReady) {
            stopPlaying();
        }
    });

    function initSimulator() {
        if (!wasmReady) {
            alert('WASM not ready yet');
            return;
        }
        simulator = new ReservoirSimulator(Number(nx), Number(ny), Number(nz));

        // Set properties from UI
        simulator.setInitialPressure(Number(initialPressure));
        simulator.setInitialSaturation(Number(initialSaturation));
        simulator.setRelPermProps(Number(s_wc), Number(s_or), Number(n_w), Number(n_o));
        simulator.setStabilityParams(Number(max_sat_change_per_step));

        if (permMode === 'random') {
            simulator.setPermeabilityRandom(Number(minPerm), Number(maxPerm));
        } else if (permMode === 'perLayer') {
            try {
                const permsX = layerPermsXStr.split(',').map(Number);
                const permsY = layerPermsYStr.split(',').map(Number);
                const permsZ = layerPermsZStr.split(',').map(Number);
                simulator.setPermeabilityPerLayer(permsX, permsY, permsZ);
            } catch (e) {
                console.error("Failed to set layer permeability:", e);
                alert("Invalid format for layer permeability. Please use comma-separated numbers.");
            }
        }
        // If permMode is 'default', we just use the default permeability from the simulator's constructor.

        history = [];
        currentIndex = -1;
        
        refreshViews();
        for (let i = 0; i < nz; i++) {
            simulator.add_well(Number(nx-1), Number(0), Number(i), Number(100), Number(well_radius), Number(well_skin), Boolean(false));
        }
        for (let i = 0; i < nz; i++) {
            simulator.add_well(Number(0), Number(0), Number(i), Number(400), Number(well_radius), Number(well_skin), Boolean(true));
        }
        runSteps();
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

    function recordCurrentState() {
        if (!simulator) return;
        try {
            const grid = structuredClone(simulator.getGridState());
            const wells = structuredClone(simulator.getWellState());
            const t = simulator.get_time();
            history.push({ time: t, grid, wells });
            currentIndex = history.length - 1;
            rateHistory = simulator.getRateHistory();
            // Reassign to trigger Svelte reactivity for arrays
            history = [...history];
        } catch (err) {
            try {
                const grid = JSON.parse(JSON.stringify(simulator.getGridState()));
                const wells = JSON.parse(JSON.stringify(simulator.getWellState()));
                const t = simulator.get_time();
                history.push({ time: t, grid, wells });
                currentIndex = history.length - 1;
                rateHistory = simulator.getRateHistory();
                // Reassign to trigger Svelte reactivity for arrays
                history = [...history];
            } catch (e) {
                console.error('Failed to record state:', e);
            }
        }
    }

    function stepOnce() {
        if (!simulator) return;
        simulator.step(Number(delta_t_days));
        refreshViews();
        recordCurrentState();
    }

    async function runSteps() {
        if (!simulator) return;
        const total = Number(steps);
        for (let i = 0; i < total; i++) {
            simulator.step(Number(delta_t_days));
            refreshViews();
            recordCurrentState();
            // Yield occasionally so UI (slider, 3D) can update during long runs
            if (i % 10 === 0) {
                await tick();
            }
        }
        // Ensure the latest snapshot is applied to the view
        applyHistoryIndex(history.length - 1);
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
        gridStateRaw = structuredClone(entry.grid);
        wellStateRaw = structuredClone(entry.wells);
        simTime = entry.time;
        // We don't update rateHistory here, as it's cumulative
    }

    /* Three.js setup - improved lighting, color encoding and resize handling */
    function refreshViews() {
        if (!simulator) return;
        try {
            // Always clone so ThreeDView sees a new reference and re-renders colors
            // (wasm may mutate the same underlying array/object in place)
            const g = simulator.getGridState();
            const w = simulator.getWellState();
            try {
                gridStateRaw = structuredClone(g);
            } catch (_) {
                gridStateRaw = JSON.parse(JSON.stringify(g));
            }
            try {
                wellStateRaw = structuredClone(w);
            } catch (_) {
                wellStateRaw = JSON.parse(JSON.stringify(w));
            }
            simTime = simulator.get_time();
            rateHistory = simulator.getRateHistory();
        } catch (err) {
            console.error('Failed to read simulator state', err);
        }
    }

        
</script>
<main class="min-h-screen bg-base-200">
<!-- <FractionalFlow
    rockProps={{ s_wc, s_or, n_w, n_o }}
    fluidProps={{ mu_w: 0.5, mu_o: 1.0 }}
    timeHistory={history.map(h => h.time)}
    injectionRate={rateHistory.find(r => r.total_injection > 0)?.total_injection ?? 0}
    reservoir={{ length: nx * 10, area: ny * 10 * nz * 1, porosity: 0.2 }}
    on:analyticalData={(e) => analyticalProductionData = e.detail.production}
/> -->
<h1 class="text-4xl font-bold mb-6">A Simplified Reservoir Simulation Model</h1>

    <div class="grid grid-cols-2 gap-4">
        <div class="grid grid-cols-2 gap-4">
            <div class="bg-blue-200">
                <h3>Reservoir Properties</h3>
                
                <div>
                    
                    <!-- <label class="form-control w-full"> -->
                        <div class="label-text">Pressure</div>
                        <div class=""> (psi) </div>
                        <input type="number" step="10" class="input input-bordered w-1/3" bind:value={initialPressure} />
                    <!-- </label> -->
                </div>
                <div>
                    <br />
                    <label class="form-control">
                        <span class="label-text w-1/2">Water Saturation</span>
                        <input type="number" step="0.05" class="input input-bordered w-1/2" bind:value={initialSaturation} />
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
            <div>
                <h4>Stability</h4>
                <label class="form-control w-full">
                    <span class="label-text">Max Saturation Change</span>
                    <input type="number" step="0.01" class="input input-bordered" bind:value={max_sat_change_per_step} />
                </label>
            </div>
        </div>
        <div class="controls">
            <span>{wasmReady ? 'WASM ready' : 'WASM loading...'}</span>
            <div>
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
            </div>

            <div>
                <label class="form-control w-full">
                    <span class="label-text">delta_t_days</span>
                    <input type="number" step="0.1" class="input input-bordered" bind:value={delta_t_days} />
                </label>
                <label class="form-control w-full">
                    <span class="label-text">steps</span>
                    <input type="number" min="1" class="input input-bordered" bind:value={steps} />
                </label>
                <div class="row">
                    <button class="btn btn-secondary" on:click={stepOnce} disabled={!simulator}>Step & Record</button>
                    <button class="btn btn-secondary" on:click={runSteps} disabled={!simulator}>Run {steps} & Record</button>
                </div>
            </div>

            <div>
                <h4>Replay</h4>
                <div class="row">
                    <button class="btn btn-outline" on:click={prev} disabled={history.length===0}>Prev</button>
                    <button class="btn btn-outline" on:click={togglePlay} disabled={history.length===0}>{playing ? 'Stop' : 'Play'}</button>
                    <button class="btn btn-outline" on:click={next} disabled={history.length===0}>Next</button>
                    <label class="form-control">
                        <span class="label-text">Speed</span>
                        <input type="number" min="0.1" step="0.1" class="input input-bordered" bind:value={playSpeed} />
                    </label>
                </div>
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
                <div>time: {simTime}</div>
                <div>recorded steps: {history.length}</div>
            </div>
        </div>
        <div class="row" style="margin-top: 1rem;">
            <RateChart {rateHistory} {analyticalProductionData} />
        </div>
    </div>
    <div class="viz-wrapper">
        <ThreeDView
            nx={nx}
            ny={ny}
            nz={nz}
            gridState={gridStateRaw}
            showProperty={showProperty}
            history={history}
            currentIndex={currentIndex}
            wellState={wellStateRaw}
        />
    </div>
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
</main>