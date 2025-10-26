<script lang="ts">
    import { onDestroy, onMount } from 'svelte';
    import * as THREE from 'three';
    import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
    import type { InstancedMesh as ThreeInstancedMesh, PerspectiveCamera as ThreePerspectiveCamera } from 'three';

    type GridCell = {
        pressure?: number;
        sat_water?: number;
        satWater?: number;
        [key: string]: unknown;
    };

    type HistoryEntry = {
        time: number;
        grid: GridCell[];
        wells: unknown;
    };

    type PropertyKey = 'pressure' | 'saturation';

    export let nx = 20;
    export let ny = 10;
    export let nz = 10;
    export let gridState: GridCell[] | null = null;
    export let showProperty: PropertyKey = 'pressure';
    export let history: HistoryEntry[] = [];
    export let currentIndex = -1;

    let renderer: THREE.WebGLRenderer | null = null;
    let scene: THREE.Scene | null = null;
    let controls: OrbitControls | null = null;
    let camera: ThreePerspectiveCamera | null = null;
    let instancedMesh: ThreeInstancedMesh | null = null;
    let animationId: number | null = null;
    let legendCanvas: HTMLCanvasElement | null = null;
    let canvasContainer: HTMLElement | null = null;
    let legendMin = 0;
    let legendMax = 1;
    let lastDimsKey = '';

    let activeGrid: GridCell[] | null = null;

    const tmpColor = new THREE.Color();
    const tmpMatrix = new THREE.Matrix4();

    onMount(() => {
        try {
            const _w = window as typeof window & { __ressim?: unknown };
            _w.__ressim = _w.__ressim || { renderer: null, scene: null, camera: null, instancedMesh: null };
        } catch (e) {
            // ignore debug exposure failures outside of browser envs
        }
        initThree();
        buildInstancedGrid();
    });

    onDestroy(() => {
        if (animationId !== null) {
            cancelAnimationFrame(animationId);
            animationId = null;
        }
        controls?.dispose();
        renderer?.dispose();
        renderer?.forceContextLoss?.();
        window.removeEventListener('resize', onWindowResize);
    });

    $: activeGrid = getActiveGrid();

    $: {
        const dimsKey = `${nx}|${ny}|${nz}`;
        if (renderer && scene && dimsKey !== lastDimsKey) {
            buildInstancedGrid();
        }
        lastDimsKey = dimsKey;
    }

    // Trigger on gridState changes
    $: if (instancedMesh && gridState) {
        updateVisualization(gridState, showProperty);
    }

    // Trigger on history/index changes
    $: currentIndex, history.length, (() => {
        if (instancedMesh && history.length > currentIndex && currentIndex >= 0) {
            const grid = history[currentIndex]?.grid;
            if (grid) {
                updateVisualization(grid, showProperty);
            }
        }
    })();

    // Trigger on property changes
    $: if (instancedMesh && activeGrid && showProperty) {
        updateVisualization(activeGrid, showProperty);
    }

    function getActiveGrid(): GridCell[] | null {
        if (history.length > 0 && currentIndex >= 0 && currentIndex < history.length) {
            const entry = history[currentIndex];
            return entry?.grid ?? null;
        }
        return gridState;
    }

    function initThree(): void {
        const width = canvasContainer?.clientWidth ?? 800;
        const height = canvasContainer?.clientHeight ?? 600;

        scene = new THREE.Scene();
        scene.background = new THREE.Color(0xf6f6f6);

        // Add lights for MeshStandardMaterial to show colors
        const ambientLight = new THREE.AmbientLight(0xffffff, 0.8);
        scene.add(ambientLight);
        
        const directionalLight = new THREE.DirectionalLight(0xffffff, 0.6);
        directionalLight.position.set(5, 10, 7);
        scene.add(directionalLight);    const newCamera = new THREE.PerspectiveCamera(45, width / height, 0.1, 10000) as ThreePerspectiveCamera;
    newCamera.position.set(0, -Math.max(nx, ny) * 2.2, Math.max(nx, ny) * 2.2);
    newCamera.up.set(0, 0, 1);
    newCamera.lookAt(0, 0, Math.max(nx, ny) * 0.5);
    camera = newCamera;

        renderer = new THREE.WebGLRenderer({ antialias: true });
        renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
        renderer.setSize(width, height, false);
        renderer.setClearColor(0xf6f6f6);

        if (canvasContainer) {
            canvasContainer.innerHTML = '';
            canvasContainer.appendChild(renderer.domElement);
            try {
                const _w = window as typeof window & { __ressim?: unknown };
                _w.__ressim = { renderer, scene, camera, instancedMesh };
            } catch (e) {
                // ignore debug exposure failures outside of browser envs
            }
        }

    controls = new OrbitControls(newCamera, renderer.domElement);
        controls.target.set(0, 0, 0);
        controls.enableDamping = true;
        controls.update();

        window.addEventListener('resize', onWindowResize, { passive: true });

        animate();
    }

    function onWindowResize(): void {
        if (!canvasContainer || !renderer || !camera) return;
        const w = canvasContainer.clientWidth || 800;
        const h = canvasContainer.clientHeight || 600;
        camera.aspect = w / h;
        camera.updateProjectionMatrix();
        renderer.setSize(w, h, false);
    }

    function buildInstancedGrid(): void {
        if (!renderer || !scene || !camera) {
            initThree();
        }
        if (!renderer || !scene || !camera) return;

        if (instancedMesh) {
            scene.remove(instancedMesh);
            instancedMesh.geometry.dispose();
            const material = instancedMesh.material;
            if (Array.isArray(material)) {
                material.forEach((mat) => mat.dispose?.());
            } else {
                material.dispose();
            }
            instancedMesh = null;
        }

        const total = nx * ny * nz;
        if (total === 0) return;

        const boxSize = 1.0;
        const geometry = new THREE.BoxGeometry(boxSize, boxSize, boxSize);
        // MeshStandardMaterial with vertexColors works well with instance colors
        const material = new THREE.MeshStandardMaterial({ 
            vertexColors: true,
            roughness: 0.8,
            metalness: 0.0
        });

        const mesh = new THREE.InstancedMesh(geometry, material, total) as ThreeInstancedMesh;
        instancedMesh = mesh;

        const colorArray = new Float32Array(total * 3);
        const attr = new THREE.InstancedBufferAttribute(colorArray, 3, false);
        attr.setUsage(THREE.DynamicDrawUsage);
        // Set instanceColor which Three.js uses for InstancedMesh
        mesh.instanceColor = attr;
        // Also add it as a geometry attribute so the material can access it
        geometry.setAttribute('color', attr);

        let idx = 0;
        const xOff = (nx - 1) * 0.5;
        const yOff = (ny - 1) * 0.5;
        const zOff = (nz - 1) * 0.5;

        for (let k = 0; k < nz; k++) {
            for (let j = 0; j < ny; j++) {
                for (let i = 0; i < nx; i++) {
                    tmpMatrix.makeTranslation((i - xOff) * (boxSize + 0.12), (j - yOff) * (boxSize + 0.12), (k - zOff) * (boxSize + 0.12));
                    mesh.setMatrixAt(idx, tmpMatrix);
                    // Default gray - will be updated when actual data arrives
                    attr.setXYZ(idx, 0.53, 0.53, 0.53);
                    idx++;
                }
            }
        }

        mesh.instanceMatrix.needsUpdate = true;
        if (mesh.instanceColor) {
            mesh.instanceColor.needsUpdate = true;
        }
        scene.add(mesh);

        try {
            const _w = window as typeof window & { __ressim?: unknown };
            _w.__ressim = { renderer, scene, camera, instancedMesh };
        } catch (e) {
            // ignore debug exposure failures
        }

        const grid = getActiveGrid();
        if (grid && instancedMesh) {
            updateVisualization(grid, showProperty);
        }
    }

    function updateVisualization(gridArray: GridCell[], property: PropertyKey): void {
        applyGridToInstances(gridArray, property);
    }

    function applyGridToInstances(gridArray: GridCell[], property: PropertyKey): void {
        if (!instancedMesh) return;

        const instAttr = instancedMesh.instanceColor;
        if (!instAttr) return;

        const values: number[] = [];
        let min = Number.POSITIVE_INFINITY;
        let max = Number.NEGATIVE_INFINITY;

        for (let i = 0; i < gridArray.length; i++) {
            const cell = gridArray[i];
            let rawValue: number;
            
            if (property === 'pressure') {
                rawValue = Number(cell.pressure);
            } else {
                rawValue = Number(
                    (cell as Record<string, unknown>).sat_water ?? 
                    (cell as Record<string, unknown>).satWater ?? 
                    (cell as Record<string, unknown>).sw ?? 
                    NaN
                );
            }
            
            values.push(rawValue);
            
            if (Number.isFinite(rawValue)) {
                if (rawValue < min) min = rawValue;
                if (rawValue > max) max = rawValue;
            }
        }

        if (property === 'saturation') {
            if (!Number.isFinite(min)) min = 0;
            if (!Number.isFinite(max)) max = 1;
            min = Math.max(0, Math.min(1, min));
            max = Math.max(0, Math.min(1, max));
            if (Math.abs(max - min) < 1e-12) {
                min = 0;
                max = 1;
            }
        } else {
            if (!Number.isFinite(min)) min = 0;
            if (!Number.isFinite(max)) max = min + 1;
            if (Math.abs(max - min) < 1e-12) {
                max = min + 1e-6;
            }
        }

        legendMin = min;
        legendMax = max;
        drawLegend(min, max);

        for (let i = 0; i < values.length && i < instancedMesh.count; i++) {
            const value = values[i];
            if (!Number.isFinite(value)) {
                instAttr.setXYZ(i, 0.7, 0.7, 0.7);
                continue;
            }
            let t = (value - min) / (max - min);
            if (!Number.isFinite(t)) t = 0;
            t = Math.max(0, Math.min(1, t));

            const hue = (1 - t) * 0.66;
            const saturation = 0.85;
            const lightness = 0.55;
            tmpColor.setHSL(hue, saturation, lightness);
            instAttr.setXYZ(i, tmpColor.r, tmpColor.g, tmpColor.b);
        }

        instAttr.needsUpdate = true;
        if (instancedMesh.geometry) {
            const colorAttr = instancedMesh.geometry.getAttribute('color');
            if (colorAttr) (colorAttr as THREE.BufferAttribute).needsUpdate = true;
        }
    }

    function animate(): void {
        animationId = requestAnimationFrame(animate);
        controls?.update();
        if (renderer && scene && camera) {
            renderer.render(scene, camera);
        }
    }

    function drawLegend(min: number, max: number): void {
        if (!legendCanvas) return;
        const ctx = legendCanvas.getContext('2d');
        if (!ctx) return;

        const w = legendCanvas.width;
        const h = legendCanvas.height;
        const gradient = ctx.createLinearGradient(0, 0, w, 0);
        const steps = 64;
        for (let i = 0; i <= steps; i++) {
            const t = i / steps;
            tmpColor.setHSL((1 - t) * 0.6, 1.0, 0.5);
            gradient.addColorStop(t, tmpColor.getStyle());
        }

        ctx.clearRect(0, 0, w, h);
        ctx.fillStyle = gradient;
        ctx.fillRect(0, 0, w, h);

        ctx.strokeStyle = 'rgba(0,0,0,0.6)';
        ctx.strokeRect(0.5, 0.5, w - 1, h - 1);

        ctx.font = '11px sans-serif';
        ctx.fillStyle = '#111';
        ctx.textBaseline = 'top';
        ctx.fillText(min.toFixed(3), 2, h + 2);
        const maxText = max.toFixed(3);
        const textWidth = ctx.measureText(maxText).width;
        ctx.fillText(maxText, w - textWidth - 2, h + 2);
    }
</script>
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
<style>
    .viz { border: 1px solid #ddd; width: 800px; height: 600px; position: relative; background: #fff; }
    .legend { margin-top: 8px; color: #222; display:flex; align-items:center; gap:8px; }
    .legend canvas { border: 1px solid #ccc; background: #fff; }
</style>
