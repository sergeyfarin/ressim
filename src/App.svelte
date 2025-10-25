<script>
    import { onMount, onDestroy } from 'svelte';
    import init, { ReservoirSimulator } from './lib/ressim/pkg/simulator.js';
    import * as THREE from 'three';
    import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';

    let wasmReady = false;
    let simulator = null;

    // UI inputs
    let nx = 20;
    let ny = 10;
    let nz = 10;
    let delta_t_days = 1.0;
    let steps = 1;

    // Well inputs
    let well_i = 0;
    let well_j = 0;
    let well_k = 0;
    let well_rate = 100.0;
    let is_injector = false;

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
    let canvasContainer;
    let legendCanvas;
    let renderer, scene, controls;
    /** @type {any} */
    let camera;
    let instancedMesh = null;
    let animationId = null;
    let showProperty = 'pressure';
    let legendMin = 0;
    let legendMax = 1;

    onMount(async () => {
        await init();
        wasmReady = true;
        initThree();
    });

    onDestroy(() => {
        if (wasmReady) {
            stopPlaying();
            if (animationId) cancelAnimationFrame(animationId);
            if (renderer) {
                renderer.dispose();
                renderer.forceContextLoss();
            }
            // remove listener without options to satisfy TypeScript overloads
            window.removeEventListener('resize', onWindowResize);
        }
    });

    function initSimulator() {
        if (!wasmReady) {
            alert('WASM not ready yet');
            return;
        }
        simulator = new ReservoirSimulator(Number(nx), Number(ny), Number(nz));
        history = [];
        currentIndex = -1;
        refreshViews();
        buildInstancedGrid();
        updateVisualization();
    }

    function addWell() {
        if (!simulator) return;
        try {
            simulator.add_well(Number(well_i), Number(well_j), Number(well_k), Number(well_rate), Boolean(is_injector));
            refreshViews();
        } catch (e) {
            console.warn('add_well call failed (check wasm signature):', e);
        }
    }

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
        updateVisualization();
    }

    function runSteps() {
        if (!simulator) return;
        for (let i = 0; i < Number(steps); i++) {
            simulator.step(Number(delta_t_days));
            refreshViews();
            recordCurrentState();
        }
        updateVisualization();
    }

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
        updateVisualization();
    }

    /* Three.js setup - improved lighting, color encoding and resize handling */
    function initThree() {
        const width = canvasContainer?.clientWidth || 800;
        const height = canvasContainer?.clientHeight || 600;

        scene = new THREE.Scene();
        // use a light neutral background so colors and boxes are clearly visible
        scene.background = new THREE.Color(0xf6f6f6);

        camera = new THREE.PerspectiveCamera(45, width / height, 0.1, 10000);
        camera.position.set(0, -Math.max(nx, ny) * 2.2, Math.max(nx, ny) * 2.2);
        camera.up.set(0, 0, 1);
        camera.lookAt(0, 0, Math.max(nx, ny) * 0.5);

        // minimal renderer (no tone mapping / lighting-specific settings required)
        renderer = new THREE.WebGLRenderer({ antialias: true });
        renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
        renderer.setSize(width, height, false);
    // match the scene background with a light clear color
    renderer.setClearColor(0xf6f6f6);

        if (canvasContainer) {
            canvasContainer.innerHTML = '';
            canvasContainer.appendChild(renderer.domElement);
        }

        controls = new OrbitControls(camera, renderer.domElement);
        controls.target.set(0, 0, 0);
        controls.enableDamping = true;
        controls.update();

        window.addEventListener('resize', onWindowResize, { passive: true });

        animate();
    }

    function onWindowResize() {
        if (!canvasContainer || !renderer || !camera) return;
        const w = canvasContainer.clientWidth || 800;
        const h = canvasContainer.clientHeight || 600;
        camera.aspect = w / h;
        camera.updateProjectionMatrix();
        renderer.setSize(w, h, false);
    }

    function buildInstancedGrid() {
        if (!renderer) initThree();
        if (instancedMesh) {
            scene.remove(instancedMesh);
            try { instancedMesh.geometry.dispose(); } catch {}
            try { instancedMesh.material.dispose(); } catch {}
            instancedMesh = null;
        }

        const total = nx * ny * nz;
        if (total === 0) return;

        const boxSize = 1.0;
        const geometry = new THREE.BoxGeometry(boxSize, boxSize, boxSize);
        // use non-lit material so colors are shown uniformly regardless of lights
        const material = new THREE.MeshBasicMaterial({
            vertexColors: true
        });

        instancedMesh = new THREE.InstancedMesh(geometry, material, total);

        const colorArray = new Float32Array(total * 3);
        const attr = new THREE.InstancedBufferAttribute(colorArray, 3, false);
        attr.setUsage(THREE.DynamicDrawUsage);
        instancedMesh.instanceColor = attr;

        let idx = 0;
        const xOff = (nx - 1) * 0.5;
        const yOff = (ny - 1) * 0.5;
        const zOff = (nz - 1) * 0.5;
        const tmpMat = new THREE.Matrix4();

        for (let k = 0; k < nz; k++) {
            for (let j = 0; j < ny; j++) {
                for (let i = 0; i < nx; i++) {
                    tmpMat.makeTranslation((i - xOff) * (boxSize + 0.12), (j - yOff) * (boxSize + 0.12), (k - zOff) * (boxSize + 0.12));
                    instancedMesh.setMatrixAt(idx, tmpMat);
                    // default mid-gray (sRGB)
                    instancedMesh.instanceColor.setXYZ(idx, 0.53, 0.53, 0.53);
                    idx++;
                }
            }
        }

        instancedMesh.instanceMatrix.needsUpdate = true;
        instancedMesh.instanceColor.needsUpdate = true;
        scene.add(instancedMesh);
    }

    /* update / coloring: convert sRGB colors to linear before writing to buffer */
    function updateVisualization() {
        // If history is present and currentIndex points to an entry, use it; otherwise current grid
        if (instancedMesh && history.length > 0 && currentIndex >= 0 && currentIndex < history.length) {
            const entry = history[currentIndex];
            applyGridToInstances(entry.grid);
            return;
        }
        if (instancedMesh && gridStateRaw) {
            applyGridToInstances(gridStateRaw);
        }
    }

    function applyGridToInstances(gridArray) {
        if (!instancedMesh || !gridArray) return;
        const values = [];
        let min = Number.POSITIVE_INFINITY, max = Number.NEGATIVE_INFINITY;
        for (let i = 0; i < gridArray.length; i++) {
            const v = showProperty === 'pressure'
                ? Number(gridArray[i].pressure)
                : Number(gridArray[i].sat_water ?? gridArray[i].satWater ?? 0);
            values.push(v);
            if (v < min) min = v;
            if (v > max) max = v;
        }
        if (!isFinite(min)) min = 0;
        if (!isFinite(max)) max = min + 1;
        if (Math.abs(max - min) < 1e-12) max = min + 1e-6;

        legendMin = min;
        legendMax = max;
        drawLegend(min, max);

        const color = new THREE.Color();
        // write colors in sRGB then convert to linear for correct rendering with sRGBEncoding
        for (let i = 0; i < values.length && i < instancedMesh.count; i++) {
            const t = (values[i] - min) / (max - min);
            color.setHSL((1 - t) * 0.6, 1.0, 0.5); // blue->red
            // convert color from sRGB to linear (renderer expects linear when outputEncoding is sRGB)
            color.convertSRGBToLinear();
            instancedMesh.instanceColor.setXYZ(i, color.r, color.g, color.b);
        }

        instancedMesh.instanceColor.needsUpdate = true;
        instancedMesh.instanceMatrix.needsUpdate = true;
    }

    function animate() {
        animationId = requestAnimationFrame(animate);
        if (controls) controls.update();
        if (renderer && scene && camera) renderer.render(scene, camera);
    }

    /* Legend drawing - use same HSL mapping as the instances */
    function drawLegend(min, max) {
        if (!legendCanvas) return;
        const w = legendCanvas.width;
        const h = legendCanvas.height;
        const ctx = legendCanvas.getContext('2d');
        if (!ctx) return;

        const grad = ctx.createLinearGradient(0, 0, w, 0);
        const tmpCol = new THREE.Color();
        const steps = 64;
        for (let i = 0; i <= steps; i++) {
            const t = i / steps;
            tmpCol.setHSL((1 - t) * 0.6, 1.0, 0.5);
            // canvas expects sRGB strings
            grad.addColorStop(t, tmpCol.getStyle());
        }

        ctx.clearRect(0, 0, w, h);
        ctx.fillStyle = grad;
        ctx.fillRect(0, 0, w, h);

        // draw border
        ctx.strokeStyle = 'rgba(0,0,0,0.6)';
        ctx.strokeRect(0.5, 0.5, w - 1, h - 1);

        // labels (dark text on light legend)
        ctx.font = '11px sans-serif';
        ctx.fillStyle = '#111';
        ctx.textBaseline = 'top';
        ctx.fillText(min.toFixed(3), 2, h + 2);
        const maxText = max.toFixed(3);
        const tw = ctx.measureText(maxText).width;
        ctx.fillText(maxText, w - tw - 2, h + 2);
    }
</script>
<style>
    .controls { display: grid; gap: 0.5rem; grid-template-columns: repeat(2, 1fr); max-width: 1000px; }
    .row { display:flex; gap:0.5rem; align-items:center; }
    pre { background:#f6f8fa; padding:0.75rem; overflow:auto; max-height:240px; }
    button { padding:0.4rem 0.7rem; }
    .viz { border: 1px solid #ddd; width: 800px; height: 600px; position: relative; background: #fff; }
    .legend { margin-top: 8px; color: #222; display:flex; align-items:center; gap:8px; }
    .legend canvas { border: 1px solid #ccc; background: #fff; }
    
</style>
<main>
<h3 class="text-4xl font-bold mb-6">Reservoir Simulator (with Replay + 3D)</h3>

<div class="controls">
    <div>
        <label>nx <input type="number" min="1" bind:value={nx} /></label>
        <label>ny <input type="number" min="1" bind:value={ny} /></label>
        <label>nz <input type="number" min="1" bind:value={nz} /></label>
        <div class="row">
            <button on:click={initSimulator}>Init Simulator</button>
            <span>{wasmReady ? 'WASM ready' : 'WASM loading...'}</span>
        </div>
    </div>

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
        <input type="range" min="0" max={Math.max(0, history.length-1)} bind:value={currentIndex} on:input={() => applyHistoryIndex(currentIndex)} />
        <div>Step: {currentIndex} / {history.length - 1}</div>
    </div>

    <div>
        <h4>Visualization</h4>
        <label><select bind:value={showProperty} on:change={updateVisualization}>
            <option value="pressure">Pressure</option>
            <option value="saturation">Saturation (water)</option>
        </select></label>
        <div>time: {simTime}</div>
        <div>recorded steps: {history.length}</div>
    </div>
</div>

<div class="row">
    <div style="display:flex; flex-direction:column;">
        <div class="viz" bind:this={canvasContainer}></div>
        <div class="legend" style="margin-left:4px;">
            <canvas bind:this={legendCanvas} width="200" height="18" style="width:200px;height:14px"></canvas>
            <div style="display:flex; flex-direction:column; margin-left:8px;">
                <div style="color:#222; font-size:12px">{showProperty === 'pressure' ? 'Pressure' : 'Saturation'}</div>
                <div style="color:#444; font-size:11px">min {legendMin.toFixed(3)} — max {legendMax.toFixed(3)}</div>
            </div>
        </div>
    </div>

    <div style="margin-left:1rem; width:340px;">
        <h4>Grid State (current)</h4>
        <pre>{JSON.stringify(gridStateRaw, null, 2)}</pre>

        <h4>Well State (current)</h4>
        <pre>{JSON.stringify(wellStateRaw, null, 2)}</pre>
    </div>
</div>
</main>