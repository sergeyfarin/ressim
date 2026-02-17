<script lang="ts">
    import { onDestroy, onMount } from 'svelte';
    import {
        AmbientLight,
        BoxGeometry,
        Color,
        CylinderGeometry,
        DirectionalLight,
        EdgesGeometry,
        Group,
        InstancedMesh,
        LineBasicMaterial,
        LineSegments,
        Matrix4,
        Mesh,
        MeshStandardMaterial,
        PerspectiveCamera,
        Raycaster,
        Scene,
        Vector2,
        WebGLRenderer,
    } from 'three';
    import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
    import type { Material } from 'three';

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

    type PropertyKey = 'pressure' | 'saturation_water' | 'saturation_oil' | 'permeability_x' | 'permeability_y' | 'permeability_z' | 'porosity';

    export let nx = 20;
    export let ny = 10;
    export let nz = 10;
    export let gridState: GridCell[] | null = null;
    export let showProperty: PropertyKey = 'pressure';
    export let history: HistoryEntry[] = [];
    export let currentIndex = -1;
    export let wellState: unknown = null;
    export let legendRangeMode: 'fixed' | 'percentile' = 'percentile';
    export let legendPercentileLow = 5;
    export let legendPercentileHigh = 95;
    export let legendFixedMin = 0;
    export let legendFixedMax = 1;
    export let theme: 'dark' | 'light' = 'dark';

    let renderer: WebGLRenderer | null = null;
    let scene: Scene | null = null;
    let controls: OrbitControls | null = null;
    let camera: PerspectiveCamera | null = null;
    let instancedMesh: InstancedMesh | null = null;
    let wireframeGroup: Group | null = null;
    let wellsGroup: Group | null = null;
    let animationId: number | null = null;
    let legendCanvas: HTMLCanvasElement | null = null;
    let canvasContainer: HTMLElement | null = null;
    let legendMin = 0;
    let legendMax = 1;
    let lastDimsKey = '';
    
    // Reactive grid reference
    let activeGrid: GridCell[] | null = null;

    // Tooltip state
    let tooltipVisible = false;
    let tooltipX = 0;
    let tooltipY = 0;
    let tooltipContent = '';
    let raycaster = new Raycaster();
    let mouse = new Vector2();
    let tooltipRafId: number | null = null;
    let latestMouseEvent: MouseEvent | null = null;

    // Helpers used in instancing and color mapping
    const tmpMatrix = new Matrix4();
    const tmpColor = new Color();

    // Fixed color ranges per property to keep legend stable
    // Pressure is intentionally auto-scaled from current values for better contrast.
    const fixedRanges: Record<PropertyKey, { min: number; max: number }> = {
        pressure: { min: 0, max: 1000 },
        saturation_water: { min: 0, max: 1 },
        saturation_oil: { min: 0, max: 1 },
        permeability_x: { min: 0, max: 1000 },
        permeability_y: { min: 0, max: 1000 },
        permeability_z: { min: 0, max: 1000 },
        porosity: { min: 0, max: 0.4 },
    };

    const propertyDisplay: Record<PropertyKey, { label: string; unit: string; decimals: number }> = {
        pressure: { label: 'Pressure', unit: 'bar', decimals: 2 },
        saturation_water: { label: 'Water Saturation', unit: 'fraction', decimals: 3 },
        saturation_oil: { label: 'Oil Saturation', unit: 'fraction', decimals: 3 },
        permeability_x: { label: 'Permeability X', unit: 'mD', decimals: 1 },
        permeability_y: { label: 'Permeability Y', unit: 'mD', decimals: 1 },
        permeability_z: { label: 'Permeability Z', unit: 'mD', decimals: 1 },
        porosity: { label: 'Porosity', unit: 'fraction', decimals: 3 },
    };

    function clamp(value: number, min: number, max: number): number {
        return Math.min(max, Math.max(min, value));
    }

    function percentile(sortedValues: number[], p: number): number {
        if (sortedValues.length === 0) return NaN;
        const bounded = clamp(p, 0, 100);
        const idx = (bounded / 100) * (sortedValues.length - 1);
        const lo = Math.floor(idx);
        const hi = Math.ceil(idx);
        if (lo === hi) return sortedValues[lo];
        const t = idx - lo;
        return sortedValues[lo] * (1 - t) + sortedValues[hi] * t;
    }

    function formatLegendValue(property: PropertyKey, value: number): string {
        const decimals = propertyDisplay[property]?.decimals ?? 3;
        return Number.isFinite(value) ? value.toFixed(decimals) : 'n/a';
    }

    function getPropertyDisplay(property: PropertyKey): { label: string; unit: string; decimals: number } {
        return propertyDisplay[property] ?? { label: 'Property', unit: '-', decimals: 3 };
    }

    function computeLegendRange(property: PropertyKey, values: number[]): { min: number; max: number } {
        const fixed = fixedRanges[property] ?? { min: 0, max: 1 };
        const finiteValues = values.filter((value) => Number.isFinite(value));

        if (legendRangeMode === 'fixed') {
            const userMin = Number(legendFixedMin);
            const userMax = Number(legendFixedMax);
            if (Number.isFinite(userMin) && Number.isFinite(userMax) && userMax > userMin) {
                return { min: userMin, max: userMax };
            }
            return fixed;
        }

        if (property === 'pressure') {
            if (finiteValues.length < 2) return fixed;
            const minValue = Math.min(...finiteValues);
            const maxValue = Math.max(...finiteValues);
            if (!Number.isFinite(minValue) || !Number.isFinite(maxValue) || maxValue <= minValue) {
                return fixed;
            }
            const span = maxValue - minValue;
            const padding = Math.max(span * 0.03, 1e-6);
            return { min: minValue - padding, max: maxValue + padding };
        }

        if (finiteValues.length < 2) {
            return fixed;
        }

        const sorted = [...finiteValues].sort((a, b) => a - b);
        const lowP = clamp(legendPercentileLow, 0, 99);
        const highP = clamp(Math.max(legendPercentileHigh, lowP + 1), 1, 100);
        const pLow = percentile(sorted, lowP);
        const pHigh = percentile(sorted, highP);

        if (!Number.isFinite(pLow) || !Number.isFinite(pHigh) || pHigh <= pLow) {
            return fixed;
        }

        return { min: pLow, max: pHigh };
    }

    onMount(() => {
        try {
            const _w = window as typeof window & { __ressim?: unknown };
            _w.__ressim = _w.__ressim || { renderer: null, scene: null, camera: null, instancedMesh: null };
        } catch (e) {
            // ignore debug exposure failures outside of browser envs
        }
        initThree();
        buildInstancedGrid();
        // Initial resize after a short delay to allow layout to settle
        setTimeout(resize, 50);
    });

    onDestroy(() => {
        if (animationId !== null) {
            cancelAnimationFrame(animationId);
            animationId = null;
        }
        controls?.dispose();
        if (renderer?.domElement) {
            renderer.domElement.removeEventListener('mousemove', onCanvasMouseMove);
        }
        if (tooltipRafId !== null) {
            cancelAnimationFrame(tooltipRafId);
            tooltipRafId = null;
        }
        renderer?.dispose();
        renderer?.forceContextLoss?.();
        window.removeEventListener('resize', onWindowResize);
    });

    // Compute activeGrid reactively — explicitly reference every dependency
    // so Svelte's compiler can track them (function-body refs are invisible).
    $: {
        gridState;
        history;
        history.length;
        currentIndex;
        nx; ny; nz;
        activeGrid = getActiveGrid();
    }

    $: {
        const dimsKey = `${nx}|${ny}|${nz}`;
        if (renderer && scene && dimsKey !== lastDimsKey) {
            buildInstancedGrid();
        }
        lastDimsKey = dimsKey;
    }

    // Trigger visualization on any data / property / legend change
    $: if (instancedMesh && activeGrid) {
        // Touch legend + property deps so edits re-trigger.
        showProperty;
        legendRangeMode;
        legendPercentileLow;
        legendPercentileHigh;
        legendFixedMin;
        legendFixedMax;
        updateVisualization(activeGrid, showProperty);
    }

    $: if (instancedMesh && !activeGrid) {
        clearVisualization(showProperty);
    }

    // Trigger on well state changes
    $: if (scene && wellState) {
        updateWellVisualization(wellState as unknown as unknown[]);
    }

    function getActiveGrid(): GridCell[] | null {
        const expectedCount = nx * ny * nz;
        if (expectedCount <= 0) return null;

        if (history.length > 0 && currentIndex >= 0 && currentIndex < history.length) {
            const entry = history[currentIndex];
            const historyGrid = entry?.grid ?? null;
            if (Array.isArray(historyGrid) && historyGrid.length === expectedCount) {
                return historyGrid;
            }
        }

        if (Array.isArray(gridState) && gridState.length === expectedCount) {
            return gridState;
        }

        return null;
    }

    function initThree(): void {
        const width = canvasContainer?.clientWidth ?? 800;
        const height = canvasContainer?.clientHeight ?? 600;

        scene = new Scene();
        const backgroundHex = theme === 'dark' ? 0x000000 : 0xf6f6f6;
        scene.background = new Color(backgroundHex);

        // Add lights for MeshStandardMaterial to show colors
        const ambientLight = new AmbientLight(0xffffff, 0.8);
        scene.add(ambientLight);
        
        const directionalLight = new DirectionalLight(0xffffff, 0.6);
        directionalLight.position.set(5, 10, 7);
        scene.add(directionalLight);    
        
        const gridSize = Math.max(nx, ny, nz)*4.5;
        const newCamera = new PerspectiveCamera(
            7, 
            1, 
            10, 
            1000
        );
        
        // Position camera at an angle to see 3D depth
        newCamera.position.set(gridSize * 1.2, -gridSize * 1.8, gridSize * 0.8);
        newCamera.up.set(0, 0, 1);
        newCamera.lookAt(0, 0, 0);
    
        camera = newCamera;

        renderer = new WebGLRenderer({ antialias: true });
        renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
        renderer.setSize(width, height, true);
        renderer.setClearColor(backgroundHex);

        if (canvasContainer) {
            // Clear existing children
            while (canvasContainer.firstChild) {
                canvasContainer.removeChild(canvasContainer.firstChild);
            }
            canvasContainer.appendChild(renderer.domElement);
            try {
                const _w = window as typeof window & { __ressim?: unknown };
                _w.__ressim = { renderer, scene, camera, instancedMesh };
            } catch (e) {
                // ignore debug exposure failures outside of browser envs
            }
        }

        controls = new OrbitControls(camera, renderer.domElement);
        controls.enableDamping = true;

        window.addEventListener('resize', onWindowResize);

        renderer.domElement.addEventListener('mousemove', onCanvasMouseMove, { passive: true });

        animate();
    }

    $: if (scene && renderer) {
        const backgroundHex = theme === 'dark' ? 0x000000 : 0xf6f6f6;
        scene.background = new Color(backgroundHex);
        renderer.setClearColor(backgroundHex);
    }

    function resize(): void {
        if (!canvasContainer || !renderer || !camera) return;
        const w = canvasContainer.clientWidth;
        const h = canvasContainer.clientHeight;

        if (w === 0 || h === 0) return; // Avoid resizing to zero
        
        const perspectiveCamera = camera as PerspectiveCamera;
        perspectiveCamera.aspect = w / h;
        perspectiveCamera.updateProjectionMatrix();
        renderer.setSize(w, h, true);
    }

    function onWindowResize(): void {
        resize();
    }

    function onCanvasMouseMove(event: MouseEvent): void {
        latestMouseEvent = event;
        if (tooltipRafId !== null) return;

        tooltipRafId = requestAnimationFrame(() => {
            tooltipRafId = null;
            if (!latestMouseEvent) return;
            performTooltipHitTest(latestMouseEvent);
        });
    }

    function performTooltipHitTest(event: MouseEvent): void {
        if (!renderer || !scene || !camera || !instancedMesh || !canvasContainer) {
            tooltipVisible = false;
            return;
        }

        // Get the active grid - use current gridState or history entry
        const currentGrid = getActiveGrid();
        if (!currentGrid || currentGrid.length === 0) {
            tooltipVisible = false;
            return;
        }

        const canvas = renderer.domElement;
        const rect = canvas.getBoundingClientRect();
        const x = event.clientX - rect.left;
        const y = event.clientY - rect.top;

        // Convert mouse position to normalized device coordinates
        mouse.x = (x / rect.width) * 2 - 1;
        mouse.y = -(y / rect.height) * 2 + 1;

        // Update the picking ray
        raycaster.setFromCamera(mouse, camera);

        // Check intersections with instanced mesh
        const intersects = raycaster.intersectObject(instancedMesh, false);

        if (intersects.length > 0) {
            const intersection = intersects[0];
            // Get the instance ID from the intersection
            const instanceId = intersection.instanceId;
            
            if (instanceId !== undefined && instanceId < currentGrid.length) {
                const cell = currentGrid[instanceId];
                const pressure = Number(cell.pressure ?? 0);
                const satWater = Number(
                    (cell as Record<string, unknown>).sat_water ?? 
                    (cell as Record<string, unknown>).satWater ?? 
                    (cell as Record<string, unknown>).sw ?? 
                    0
                );
                const satOil = Number(
                    (cell as Record<string, unknown>).sat_oil ?? 
                    (cell as Record<string, unknown>).satOil ?? 
                    (cell as Record<string, unknown>).so ?? 
                    0
                );

                tooltipContent = `Pressure: ${pressure.toFixed(2)}\nWater Sat: ${satWater.toFixed(3)}\nOil Sat: ${satOil.toFixed(3)}`;
                tooltipX = x + 10;
                tooltipY = y + 10;
                tooltipVisible = true;
            } else {
                tooltipVisible = false;
            }
        } else {
            tooltipVisible = false;
        }
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
        
        if (wireframeGroup) {
            scene.remove(wireframeGroup);
            wireframeGroup.traverse((child) => {
                if (child instanceof LineSegments) {
                    child.geometry.dispose();
                    (child.material as Material).dispose();
                }
            });
            wireframeGroup = null;
        }

        // Initialize wells group if not already done
        if (!wellsGroup) {
            wellsGroup = new Group();
            scene.add(wellsGroup);
        } else {
            wellsGroup.clear();
        }

        const total = nx * ny * nz;
        if (total === 0) return;

        const boxSize = 1.0;
        const geometry = new BoxGeometry(boxSize, boxSize, boxSize);
        
        // Use per-instance colors
        const material = new MeshStandardMaterial({ 
            color: 0xffffff,
            roughness: 0.8,
            metalness: 0.0,
            wireframe: false
        });

        const mesh = new InstancedMesh(geometry, material, total);
        instancedMesh = mesh;

        // Assign a default per-instance color (medium gray)
        const defaultColor = new Color(0x888888);

        let idx = 0;
        const xOff = (nx - 1) * 0.5;
        const yOff = (ny - 1) * 0.5;
        const zOff = (nz - 1) * 0.5;

        for (let k = 0; k < nz; k++) {
            for (let j = 0; j < ny; j++) {
                for (let i = 0; i < nx; i++) {
                    tmpMatrix.makeTranslation((i - xOff) * boxSize, (j - yOff) * boxSize, (k - zOff) * boxSize);
                    mesh.setMatrixAt(idx, tmpMatrix);
                    // Default color; will be overwritten by updateVisualization
                    mesh.setColorAt(idx, defaultColor);
                    idx++;
                }
            }
        }

        mesh.instanceMatrix.needsUpdate = true;
        if (mesh.instanceColor) {
            mesh.instanceColor.needsUpdate = true;
        }
        scene.add(mesh);

        // Create a single wireframe outline for the whole reservoir volume
        wireframeGroup = new Group();
        const reservoirGeometry = new BoxGeometry(nx * boxSize, ny * boxSize, nz * boxSize);
        const edgesGeometry = new EdgesGeometry(reservoirGeometry);
        reservoirGeometry.dispose();
        const lineMaterial = new LineBasicMaterial({ 
            color: 0x000000,
            transparent: true,
            opacity: 0.6,
            depthTest: true,
            depthWrite: false
        });
        const reservoirEdges = new LineSegments(edgesGeometry, lineMaterial);
        reservoirEdges.position.set(0, 0, 0);
        wireframeGroup.add(reservoirEdges);
        scene.add(wireframeGroup);

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

    function clearVisualization(property: PropertyKey): void {
        if (!instancedMesh || !instancedMesh.instanceColor) return;

        const defaultColor = new Color(0x888888);
        for (let i = 0; i < instancedMesh.count; i++) {
            instancedMesh.setColorAt(i, defaultColor);
        }
        instancedMesh.instanceColor.needsUpdate = true;

        const fixed = fixedRanges[property] ?? { min: 0, max: 1 };
        legendMin = fixed.min;
        legendMax = fixed.max;
        drawLegend(legendMin, legendMax, property);
    }

    function applyGridToInstances(gridArray: GridCell[], property: PropertyKey): void {
        if (!instancedMesh) return;

        if (!instancedMesh.instanceColor) return;

        const defaultColor = new Color(0x888888);
        for (let i = 0; i < instancedMesh.count; i++) {
            instancedMesh.setColorAt(i, defaultColor);
        }

        const values: number[] = [];

        for (let i = 0; i < gridArray.length; i++) {
            const cell = gridArray[i];
            let rawValue: number;
            
            if (property === 'pressure') {
                rawValue = Number(cell.pressure);
            } else if (property === 'saturation_water') {
                rawValue = Number(
                    (cell as Record<string, unknown>).sat_water ?? 
                    (cell as Record<string, unknown>).satWater ?? 
                    (cell as Record<string, unknown>).sw ?? 
                    NaN
                );
            } else if (property === 'saturation_oil') {
                rawValue = Number(
                    (cell as Record<string, unknown>).sat_oil ?? 
                    (cell as Record<string, unknown>).satOil ?? 
                    (cell as Record<string, unknown>).so ?? 
                    NaN
                );
            } else if (property === 'permeability_x') {
                rawValue = Number((cell as Record<string, unknown>).perm_x ?? NaN);
            } else if (property === 'permeability_y') {
                rawValue = Number((cell as Record<string, unknown>).perm_y ?? NaN);
            } else if (property === 'permeability_z') {
                rawValue = Number((cell as Record<string, unknown>).perm_z ?? NaN);
            } else if (property === 'porosity') {
                rawValue = Number((cell as Record<string, unknown>).porosity ?? NaN);
            } else {
                rawValue = NaN;
            }
            
            values.push(rawValue);
        }

        const range = computeLegendRange(property, values);
        const min = range.min;
        const max = range.max;

        legendMin = min;
        legendMax = max;
        drawLegend(min, max, property);

        for (let i = 0; i < values.length && i < instancedMesh.count; i++) {
            const value = values[i];
            if (!Number.isFinite(value)) {
                tmpColor.set(0x888888);
                instancedMesh.setColorAt(i, tmpColor);
                continue;
            }
            let t = (value - min) / (max - min);
            if (!Number.isFinite(t)) t = 0;
            t = Math.max(0, Math.min(1, t));
            const hue = (1 - t) * 0.66; // blue (high) to red (low)
            tmpColor.setHSL(hue, 0.85, 0.55);
            instancedMesh.setColorAt(i, tmpColor);
        }
        instancedMesh.instanceColor.needsUpdate = true;
    }

    function updateWellVisualization(wells: unknown[]): void {
        if (!scene || !wellsGroup) return;

        // Remove old wells
        scene.remove(wellsGroup);
        wellsGroup.traverse((child) => {
            if (child instanceof Mesh) {
                child.geometry.dispose();
                (child.material as Material).dispose();
            }
        });
        wellsGroup.clear();

        if (!Array.isArray(wells) || wells.length === 0) return;

        // Track which (i, j) columns have wells to avoid duplicates
        const wellColumns = new Set<string>();

        // Find the topmost cell for each well column
        for (const well of wells) {
            const w = well as Record<string, unknown>;
            const i = Number(w.i ?? w.x ?? 0);
            const j = Number(w.j ?? w.y ?? 0);
            const k = Number(w.k ?? w.z ?? 0);
            
            const colKey = `${i},${j}`;
            if (!wellColumns.has(colKey)) {
                wellColumns.add(colKey);

                // Find topmost cell with this well (minimum k)
                let topK = k;
                for (const other of wells) {
                    const o = other as Record<string, unknown>;
                    const oi = Number(o.i ?? o.x ?? 0);
                    const oj = Number(o.j ?? o.y ?? 0);
                    const ok = Number(o.k ?? o.z ?? 0);
                    if (oi === i && oj === j && ok < topK) {
                        topK = ok;
                    }
                }

                // Draw cylinder from top face (NZ=0) going up
                const xOff = (nx - 1) * 0.5;
                const yOff = (ny - 1) * 0.5;
                const zOff = (nz - 1) * 0.5;
                const boxSize = 1.0;
                const wellRadius = 0.15;
                const wellHeight = 10.0;

                const wellCylinder = new Mesh(
                    new CylinderGeometry(wellRadius, wellRadius, wellHeight, 16),
                    new MeshStandardMaterial({
                        color: 0x8B4513,
                        roughness: 0.6,
                        metalness: 0.3,
                        emissive: 0x3d2817
                    })
                );

                // Rotate cylinder to point along Z-axis instead of Y-axis
                wellCylinder.rotation.x = Math.PI / 2;

                // Position at top of reservoir (k=0 is at top, so z is maximum)
                // k=0 corresponds to z = (0 - zOff) * boxSize = -(nz-1)/2 * boxSize
                const cellCenterX = (i - xOff) * boxSize;
                const cellCenterY = (j - yOff) * boxSize;
                const topSurfaceZ = (nz - zOff) * boxSize - boxSize * 0.5;
                const cellTopZ = topSurfaceZ + wellHeight * 0.5;

                wellCylinder.position.set(cellCenterX, cellCenterY, cellTopZ);
                wellsGroup.add(wellCylinder);
            }
        }

        scene.add(wellsGroup);
    }

    function animate(): void {
        animationId = requestAnimationFrame(animate);
        controls?.update();
        if (renderer && scene && camera) {
            renderer.render(scene, camera);
        }
    }

    function drawLegend(min: number, max: number, property: PropertyKey): void {
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
        ctx.fillText(formatLegendValue(property, min), 2, h + 2);
        const maxText = formatLegendValue(property, max);
        const textWidth = ctx.measureText(maxText).width;
        ctx.fillText(maxText, w - textWidth - 2, h + 2);
        
    }
</script>
<div style="display:flex; flex-direction:column;">
    <div class="viz" bind:this={canvasContainer} style="position:relative;">
        {#if tooltipVisible}
            <div style="position:absolute; left:{tooltipX}px; top:{tooltipY}px; background:rgba(0,0,0,0.85); color:#fff; padding:6px 8px; border-radius:4px; font-size:11px; pointer-events:none; white-space:pre-line; line-height:1.4; z-index:1000; border:1px solid #ddd;">
                {tooltipContent}
            </div>
        {/if}
    </div>
    <div class="legend" style="margin-left:4px;">
        <canvas bind:this={legendCanvas} width="200" height="18" style="width:200px;height:14px"></canvas>
        <div style="display:flex; flex-direction:column; margin-left:8px;">
            <div style="color:#222; font-size:12px">
                {getPropertyDisplay(showProperty).label} ({getPropertyDisplay(showProperty).unit})
            </div>
            <div style="color:#444; font-size:11px">
                {legendRangeMode === 'percentile'
                    ? `Percentile P${clamp(legendPercentileLow, 0, 99)}–P${clamp(Math.max(legendPercentileHigh, legendPercentileLow + 1), 1, 100)}`
                    : (showProperty === 'pressure' ? 'Auto range' : 'Fixed range')}
            </div>
            <div style="color:#444; font-size:11px">
                min {formatLegendValue(showProperty, legendMin)} — max {formatLegendValue(showProperty, legendMax)}
            </div>
        </div>
    </div>
</div>
<style>
    /* Revert to non-absolute sizing so parent controls height explicitly */
    .viz { border: 1px solid #ddd; width: 100%; height: clamp(255px, 37vh, 440px); position: relative; background: #fff; }
    .legend { margin-top: 8px; color: #222; display:flex; align-items:center; gap:8px; }
    .legend canvas { border: 1px solid #ccc; background: #fff; }
</style>
