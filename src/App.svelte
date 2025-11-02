<script lang="ts">
    import { onMount, onDestroy } from 'svelte';
    import init, { ReservoirSimulator } from './lib/ressim/pkg/simulator.js';
    import ThreeDView from './lib/3dview.svelte';

    let wasmReady = false;
    let simulator = null;

    // UI inputs
    let nx = 20;
    let ny = 10;
    let nz = 10;
    let delta_t_days = 1.0;
    let steps = 100;

    // Well inputs
    // let well_i = 0;
    // let well_j = 0;
    // let well_k = 0;
    // let well_rate = 100.0;
    // let is_injector = false;

    // Display data
    let gridStateRaw = null;
    let wellStateRaw = null;
    let simTime = 0;

    // History / replay
    let history = [];
    let currentIndex = -1;
    let playing = false;
    let playSpeed = 2;
    let playTimer = null;

    // Visualization
    let showProperty: 'pressure' | 'saturation_water' | 'saturation_oil' = 'pressure';

    onMount(async () => {
        await init();
        wasmReady = true;
        initSimulator();
        for (let i = 0; i < nz; i++) {
            simulator.add_well(Number(nx-1), Number(0), Number(i), Number(100), Number(200), Boolean(false));
        }
        for (let i = 0; i < nz; i++) {
            simulator.add_well(Number(0), Number(0), Number(i), Number(400), Number(200), Boolean(true));
        }
        runSteps();

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
        history = [];
        // currentIndex = -1;
        // refreshViews();
        // buildInstancedGrid();
        // updateVisualization();
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
        } catch (err) {
            try {
                const grid = JSON.parse(JSON.stringify(simulator.getGridState()));
                const wells = JSON.parse(JSON.stringify(simulator.getWellState()));
                const t = simulator.get_time();
                history.push({ time: t, grid, wells });
                currentIndex = history.length - 1;
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

    function runSteps() {
        if (!simulator) return;
        for (let i = 0; i < Number(steps); i++) {
            simulator.step(Number(delta_t_days));
            refreshViews();
            recordCurrentState();
        }
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
    }

    /* Three.js setup - improved lighting, color encoding and resize handling */
    function refreshViews() {
        if (!simulator) return;
        try {
            gridStateRaw = simulator.getGridState();
            wellStateRaw = simulator.getWellState();
            simTime = simulator.get_time();
        } catch (err) {
            console.error('Failed to read simulator state', err);
        }
    }

        
</script>
<style>
    .controls { display: grid; gap: 0.5rem; grid-template-columns: repeat(2, 1fr); max-width: 1000px; }
    .row { display:flex; gap:0.5rem; align-items:center; }
    pre { background:#f6f8fa; padding:0.75rem; overflow:auto; max-height:240px; }
    button { padding:0.4rem 0.7rem; }
 
</style>
<main>
<h1 class="text-4xl font-bold mb-6">A Simplified Reservoir Simulation Model</h1>
<div class="grid grid-cols-3 gap-4">
  <div class="...">01</div>
  <div class="...">02</div>
  <div class="...">03</div>
  <div class="col-span-2 ...">04</div>
  <div class="...">05</div>
  <div class="...">06</div>
  <div class="col-span-2 ...">07</div>
</div>

<div style="display: grid; grid-template-columns: repeat(4, 1fr); gap: 10px;">
  <div style="background: lightblue;">Column 1</div>
  <div style="background: lightgreen;">Column 2</div>
  <div style="background: lightcoral;">Column 3</div>
  <div style="background: lightgoldenrodyellow;">Column 4</div>
  <div style="background: lightpink; grid-column: span 2;">Colspan (spans 2 columns)</div>
  <div style="background: lightgray;">Column 3</div>
  <div style="background: lightcyan;">Column 4</div>
</div>
<div class="controls">
    <span>{wasmReady ? 'WASM ready' : 'WASM loading...'}</span>
    <!-- <div>
        <label>nx <input type="number" min="1" bind:value={nx} /></label>
        <label>ny <input type="number" min="1" bind:value={ny} /></label>
        <label>nz <input type="number" min="1" bind:value={nz} /></label>
        <div class="row">
            <button on:click={initSimulator}>Init Simulator</button>
            <span>{wasmReady ? 'WASM ready' : 'WASM loading...'}</span>
        </div>
    </div> -->

    <div>
        <label>delta_t_days <input type="number" step="0.1" bind:value={delta_t_days} /></label>
        <label>steps <input type="number" min="1" bind:value={steps} /></label>
        <div class="row">
            <button on:click={stepOnce} disabled={!simulator}>Step & Record</button>
            <button on:click={runSteps} disabled={!simulator}>Run {steps} & Record</button>
        </div>
    </div>

    <div>
        <h4>Replay</h4>
        <div class="row">
            <button on:click={prev} disabled={history.length===0}>Prev</button>
            <button on:click={togglePlay} disabled={history.length===0}>{playing ? 'Stop' : 'Play'}</button>
            <button on:click={next} disabled={history.length===0}>Next</button>
            <label>Speed <input type="number" min="0.1" step="0.1" bind:value={playSpeed} /></label>
        </div>
        <div style="display:flex; gap:0.5rem; align-items:center;">
            <input type="range" min="0" max={Math.max(0, history.length-1)} bind:value={currentIndex} on:input={() => applyHistoryIndex(currentIndex)} style="flex:1;" />
            <span style="min-width:80px;">Step: {currentIndex} / {history.length - 1}</span>
        </div>
        {#if history.length > 0 && currentIndex >= 0 && currentIndex < history.length}
            <div style="color:#666; font-size:12px;">Time: {history[currentIndex].time.toFixed(2)} days</div>
        {/if}
    </div>

    <div>
        <h4>Visualization</h4>
        <label><select bind:value={showProperty}>
            <option value="pressure">Pressure</option>
            <option value="saturation_water">Water Saturation</option>
            <option value="saturation_oil">Oil Saturation</option>
        </select></label>
        <div>time: {simTime}</div>
        <div>recorded steps: {history.length}</div>
    </div>
</div>

<div class="row">
    <div>
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

    <div style="margin-left:1rem; width:340px;">
        <h4>Grid State (current)</h4>
        <pre>{JSON.stringify(gridStateRaw, null, 2)}</pre>

        <h4>Well State (current)</h4>
        <pre>{JSON.stringify(wellStateRaw, null, 2)}</pre>
    </div>
</div>
</main>