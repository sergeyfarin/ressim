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

    import type { GridState, SimulatorSnapshot, WellState, WellStateEntry } from './';

    type HistoryEntry = SimulatorSnapshot;

    type PropertyKey = 'pressure' | 'saturation_water' | 'saturation_oil' | 'permeability_x' | 'permeability_y' | 'permeability_z' | 'porosity';

    export let nx: number = 20;
    export let ny: number = 10;
    export let nz: number = 10;
    export let cellDx: number = 10;
    export let cellDy: number = 10;
    export let cellDz: number = 1;
    export let gridState: GridState | null = null;
    export let showProperty: PropertyKey = 'pressure';
    export let history: HistoryEntry[] = [];
    export let currentIndex: number = -1;
    export let wellState: WellState | null = null;
    export let legendFixedMin: number = 0;
    export let legendFixedMax: number = 1;
    export let s_wc: number = 0.1;
    export let s_or: number = 0.1;
    export let replayTime: number | null = null;
    // export let playing = false;
    // export let playSpeed = 2;
    // export let showDebugState = false;
    export let onApplyHistoryIndex: (index: number) => void = () => {};
    // export let onPrev: () => void = () => {};
    // export let onNext: () => void = () => {};
    // export let onTogglePlay: () => void = () => {};
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
    let activeGrid: GridState | null = null;

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
    const Z_VISUAL_EXAGGERATION = 10;

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

    const showPropertyOptions: Array<{ value: PropertyKey; label: string }> = [
        { value: 'pressure', label: 'Pressure' },
        { value: 'saturation_water', label: 'Water Sat' },
        { value: 'saturation_oil', label: 'Oil Sat' },
        { value: 'permeability_x', label: 'Perm X' },
        { value: 'permeability_y', label: 'Perm Y' },
        { value: 'permeability_z', label: 'Perm Z' },
        { value: 'porosity', label: 'Porosity' },
    ];

    let groupSummary = '';

    $: groupSummary = `${showProperty.replace('_', ' ')} · step ${Math.max(0, currentIndex)}`;

    $: {
        const minValue = Number(legendFixedMin);
        const maxValue = Number(legendFixedMax);

        if (!Number.isFinite(minValue)) {
            legendFixedMin = 0;
        }
        if (!Number.isFinite(maxValue)) {
            legendFixedMax = 1;
        }

        if (Number.isFinite(legendFixedMin) && Number.isFinite(legendFixedMax) && legendFixedMin > legendFixedMax) {
            const tmp = legendFixedMin;
            legendFixedMin = legendFixedMax;
            legendFixedMax = tmp;
        }
    }

    function clamp(value: number, min: number, max: number): number {
        return Math.min(max, Math.max(min, value));
    }

    function onLegendMinInput(event: Event) {
        const input = event.currentTarget as HTMLInputElement;
        legendFixedMin = Number(input.value);
    }

    function onLegendMaxInput(event: Event) {
        const input = event.currentTarget as HTMLInputElement;
        legendFixedMax = Number(input.value);
    }

    function hasTimestepsRun(): boolean {
        return history.length > 0 && currentIndex >= 0 && currentIndex < history.length;
    }

    function getExpectedCellCount(): number {
        return Math.max(0, Number(nx) * Number(ny) * Number(nz));
    }

    function getValidHistoryGrids(): GridState[] {
        const expectedCount = getExpectedCellCount();
        if (expectedCount <= 0 || !Array.isArray(history) || history.length === 0) return [];
        const grids: GridState[] = [];
        for (const entry of history) {
            const grid = entry?.grid;
            if (grid && grid.pressure && grid.pressure.length === expectedCount) {
                grids.push(grid);
            }
        }
        return grids;
    }

    function getStaticReferenceGrid(): GridState | null {
        const historyGrids = getValidHistoryGrids();
        if (historyGrids.length > 0) return historyGrids[0];
        const expectedCount = getExpectedCellCount();
        if (expectedCount > 0 && gridState && gridState.pressure && gridState.pressure.length === expectedCount) {
            return gridState;
        }
        return null;
    }

    function getPropertyValuesFromGrid(grid: GridState | null | undefined, property: PropertyKey): number[] {
        if (!grid || !grid.pressure || grid.pressure.length === 0) return [];
        const values = [];
        for (let i = 0; i < grid.pressure.length; i++) {
            values.push(getCellPropertyValue(grid, i, property));
        }
        return values.filter((value) => Number.isFinite(value));
    }

    function getHistoryPropertyRange(property: PropertyKey): { min: number; max: number } | null {
        const historyGrids = getValidHistoryGrids();
        if (historyGrids.length === 0) return null;

        let min = Number.POSITIVE_INFINITY;
        let max = Number.NEGATIVE_INFINITY;

        for (const grid of historyGrids) {
            if (!grid.pressure) continue;
            for (let i = 0; i < grid.pressure.length; i++) {
                const value = getCellPropertyValue(grid, i, property);
                if (!Number.isFinite(value)) continue;
                if (value < min) min = value;
                if (value > max) max = value;
            }
        }

        if (!Number.isFinite(min) || !Number.isFinite(max) || max <= min) {
            return null;
        }
        return { min, max };
    }

    function getCellPropertyValue(grid: GridState | null | undefined, index: number, property: PropertyKey): number {
        if (!grid) return NaN;
        if (property === 'pressure') return Number(grid.pressure?.[index] ?? NaN);
        if (property === 'saturation_water') return Number(grid.sat_water?.[index] ?? NaN);
        if (property === 'saturation_oil') return Number(grid.sat_oil?.[index] ?? NaN);
        if (property === 'permeability_x') return Number(grid.perm_x?.[index] ?? NaN);
        if (property === 'permeability_y') return Number(grid.perm_y?.[index] ?? NaN);
        if (property === 'permeability_z') return Number(grid.perm_z?.[index] ?? NaN);
        if (property === 'porosity') return Number(grid.porosity?.[index] ?? NaN);
        return NaN;
    }

    function getModelLegendRange(property: PropertyKey): { min: number; max: number } {
        const fixed = fixedRanges[property] ?? { min: 0, max: 1 };

        if (property === 'saturation_water') {
            const swc = clamp(Number(s_wc), 0, 0.95);
            const sor = clamp(Number(s_or), 0, 0.95);
            const min = swc;
            const max = Math.max(min + 1e-6, 1 - sor);
            return { min, max };
        }

        if (property === 'saturation_oil') {
            const swc = clamp(Number(s_wc), 0, 0.95);
            const sor = clamp(Number(s_or), 0, 0.95);
            const min = sor;
            const max = Math.max(min + 1e-6, 1 - swc);
            return { min, max };
        }

        if (property === 'pressure') {
            const historyRange = getHistoryPropertyRange('pressure');
            if (historyRange) {
                return historyRange;
            }
            const values = getPropertyValuesFromGrid(activeGrid, property);
            if (values.length === 0) {
                return fixed;
            }
            const min = Math.min(...values);
            const dataMax = Math.max(...values);
            const max = Number.isFinite(dataMax) ? Math.max(min + 1e-6, dataMax) : Math.max(min + 1e-6, fixed.max);
            return { min, max };
        }

        if (property === 'permeability_x' || property === 'permeability_y' || property === 'permeability_z' || property === 'porosity') {
            const referenceGrid = getStaticReferenceGrid();
            const values = getPropertyValuesFromGrid(referenceGrid, property);
            if (values.length === 0) return fixed;
            const min = Math.min(...values);
            const max = Math.max(...values);
            if (!Number.isFinite(min) || !Number.isFinite(max) || max <= min) {
                return fixed;
            }
            return { min, max };
        }

        const values = getPropertyValuesFromGrid(activeGrid, property);
        if (values.length === 0) {
            return fixed;
        }

        const min = Math.min(...values);
        const max = Math.max(...values);
        if (!Number.isFinite(min) || !Number.isFinite(max) || max <= min) {
            return fixed;
        }
        return { min, max };
    }

    function applyModelLegendMin(): void {
        legendFixedMin = getModelLegendRange(showProperty).min;
    }

    function applyModelLegendMax(): void {
        legendFixedMax = getModelLegendRange(showProperty).max;
    }

    function applyModelLegendRange(): void {
        const range = getModelLegendRange(showProperty);
        legendFixedMin = range.min;
        legendFixedMax = range.max;
    }

    function applySliderValue(event: Event) {
        const input = event.currentTarget as HTMLInputElement;
        currentIndex = Number(input.value);
        onApplyHistoryIndex(currentIndex);
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
        const userMin = Number(legendFixedMin);
        const userMax = Number(legendFixedMax);
        if (Number.isFinite(userMin) && Number.isFinite(userMax) && userMax > userMin) {
            return { min: userMin, max: userMax };
        }
        return fixed;
    }

    function getHue(property: PropertyKey, t: number): number {
        if (property === 'saturation_water') {
            return t * 0.66;
        }
        return (1 - t) * 0.66;
    }

    onMount(() => {
        try {
            const _w = window as typeof window & { __ressim?: unknown };
            _w.__ressim = _w.__ressim || { renderer: null, scene: null, camera: null, instancedMesh: null };
        } catch (e) {
            // ignore debug exposure failures outside of browser envs
        }
        initThree();
        buildInstancedGrid(true);
        lastDimsKey = `${nx}|${ny}|${nz}|${cellDx}|${cellDy}|${cellDz}`;
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
        const dimsKey = `${nx}|${ny}|${nz}|${cellDx}|${cellDy}|${cellDz}`;
        if (renderer && scene && dimsKey !== lastDimsKey) {
            buildInstancedGrid(true);
            lastDimsKey = dimsKey;
        }
    }

    function getVisualCellSizes(): { x: number; y: number; z: number } {
        const x = Math.max(0.001, Number(cellDx) || 1);
        const y = Math.max(0.001, Number(cellDy) || 1);
        const z = Math.max(0.001, Number(cellDz) || 1) * Z_VISUAL_EXAGGERATION;
        return { x, y, z };
    }

    function fitCameraToReservoir(cellSize: { x: number; y: number; z: number }): void {
        if (!camera || !canvasContainer) return;

        const perspectiveCamera = camera as PerspectiveCamera;
        const aspect = Math.max(1e-3, canvasContainer.clientWidth / Math.max(1, canvasContainer.clientHeight));
        perspectiveCamera.aspect = aspect;

        const halfX = (nx * cellSize.x) * 0.5;
        const halfY = (ny * cellSize.y) * 0.5;
        const halfZ = (nz * cellSize.z) * 0.5;
        const radius = Math.max(0.001, Math.sqrt(halfX * halfX + halfY * halfY + halfZ * halfZ));

        const verticalFov = (perspectiveCamera.fov * Math.PI) / 60;
        const horizontalFov = 2 * Math.atan(Math.tan(verticalFov * 0.5) * perspectiveCamera.aspect);
        const fitDistanceV = radius / Math.max(Math.sin(verticalFov * 0.5), 1e-3);
        const fitDistanceH = radius / Math.max(Math.sin(horizontalFov * 0.5), 1e-3);
        const fitDistance = Math.max(fitDistanceV, fitDistanceH) * 1.35;

        const dirX = 1.2;
        const dirY = -1.8;
        const dirZ = 0.8;
        const dirLen = Math.sqrt(dirX * dirX + dirY * dirY + dirZ * dirZ);
        const ux = dirX / dirLen;
        const uy = dirY / dirLen;
        const uz = dirZ / dirLen;

        perspectiveCamera.position.set(ux * fitDistance, uy * fitDistance, uz * fitDistance);
        perspectiveCamera.up.set(0, 0, 1);
        perspectiveCamera.near = Math.max(0.1, fitDistance - radius * 2.5);
        perspectiveCamera.far = Math.max(1000, fitDistance + radius * 4.0);
        perspectiveCamera.lookAt(0, 0, 0);
        perspectiveCamera.updateProjectionMatrix();

        if (controls) {
            controls.target.set(0, 0, 0);
            controls.minDistance = Math.max(0.1, fitDistance * 0.15);
            controls.maxDistance = Math.max(50, fitDistance * 8);
            controls.update();
        }
    }

    let lastModelLegendKey = '';
    let pressureHistoryScanCount = 0;

    function getLegendContextKey(property: PropertyKey): string {
        const dimsKey = `${nx}|${ny}|${nz}|${cellDx}|${cellDy}|${cellDz}`;

        if (property === 'pressure') {
            return `${property}|${dimsKey}`;
        }
        if (property === 'saturation_water' || property === 'saturation_oil') {
            return `${property}|${dimsKey}|${s_wc}|${s_or}`;
        }

        const staticRefLen = getStaticReferenceGrid()?.pressure?.length ?? 0;
        return `${property}|${dimsKey}|${staticRefLen}`;
    }

    function getPressureMaxFromHistorySlice(startIdx: number): number | null {
        const expectedCount = getExpectedCellCount();
        if (expectedCount <= 0 || !Array.isArray(history) || history.length === 0) return null;

        const safeStart = Math.max(0, Math.min(history.length, startIdx));
        let max = Number.NEGATIVE_INFINITY;

        for (let idx = safeStart; idx < history.length; idx++) {
            const grid = history[idx]?.grid;
            if (!Array.isArray(grid) || grid.length !== expectedCount) continue;
            for (const cell of grid) {
                const value = Number(cell?.pressure);
                if (!Number.isFinite(value)) continue;
                if (value > max) max = value;
            }
        }

        return Number.isFinite(max) ? max : null;
    }

    // Auto legend policy:
    // - pressure: initialize min/max from full history once, then only raise max as new history arrives.
    // - saturation: fixed by Swc/Sor model endpoints.
    // - permeability/porosity: recalc only when static context changes.
    $: if (activeGrid) {
        const contextKey = getLegendContextKey(showProperty);
        const contextChanged = contextKey !== lastModelLegendKey;

        if (contextChanged) {
            applyModelLegendRange();
            lastModelLegendKey = contextKey;
            pressureHistoryScanCount = history.length;
        } else if (showProperty === 'pressure') {
            if (history.length < pressureHistoryScanCount) {
                applyModelLegendRange();
                pressureHistoryScanCount = history.length;
            } else if (history.length > pressureHistoryScanCount) {
                const incrementalMax = getPressureMaxFromHistorySlice(pressureHistoryScanCount);
                if (incrementalMax != null) {
                    const currentMin = Number(legendFixedMin);
                    const currentMax = Number(legendFixedMax);
                    const baseMin = Number.isFinite(currentMin) ? currentMin : getModelLegendRange('pressure').min;
                    const baseMax = Number.isFinite(currentMax) ? currentMax : getModelLegendRange('pressure').max;
                    legendFixedMin = baseMin;
                    legendFixedMax = Math.max(baseMax, incrementalMax, baseMin + 1e-6);
                }
                pressureHistoryScanCount = history.length;
            }
        }
    }

    // Trigger visualization on any data / property / legend change
    $: if (instancedMesh && activeGrid) {
        showProperty;
        legendFixedMin;
        legendFixedMax;
        updateVisualization(activeGrid, showProperty);
    }

    $: if (instancedMesh && !activeGrid) {
        clearVisualization(showProperty);
    }

    // Trigger on well state changes
    $: if (scene && wellState) {
        updateWellVisualization(wellState ?? []);
    }

    function getActiveGrid(): GridState | null {
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
        if (!currentGrid || !currentGrid.pressure || currentGrid.pressure.length === 0) {
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
            
            if (instanceId !== undefined && currentGrid.pressure && instanceId < currentGrid.pressure.length) {
                const pressure = Number(currentGrid.pressure?.[instanceId] ?? 0);
                const satWater = Number(currentGrid.sat_water?.[instanceId] ?? 0);
                const satOil = Number(currentGrid.sat_oil?.[instanceId] ?? 0);

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

    function buildInstancedGrid(autoFitCamera = false): void {
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

        const cellSize = getVisualCellSizes();
        const geometry = new BoxGeometry(cellSize.x, cellSize.y, cellSize.z);
        
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
                    tmpMatrix.makeTranslation(
                        (i - xOff) * cellSize.x,
                        (j - yOff) * cellSize.y,
                        (k - zOff) * cellSize.z
                    );
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

        if (autoFitCamera) {
            fitCameraToReservoir(cellSize);
        }

        // Create a single wireframe outline for the whole reservoir volume
        wireframeGroup = new Group();
        const reservoirGeometry = new BoxGeometry(nx * cellSize.x, ny * cellSize.y, nz * cellSize.z);
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

    function updateVisualization(gridArray: GridState, property: PropertyKey): void {
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

    function applyGridToInstances(gridArray: GridState, property: PropertyKey): void {
        if (!instancedMesh) return;

        if (!instancedMesh.instanceColor) return;

        const defaultColor = new Color(0x888888);
        for (let i = 0; i < instancedMesh.count; i++) {
            instancedMesh.setColorAt(i, defaultColor);
        }

        const values: number[] = [];
        const len = gridArray.pressure.length;

        for (let i = 0; i < len; i++) {
            values.push(getCellPropertyValue(gridArray, i, property));
        }

        const range = computeLegendRange(property, values);
        const min = range.min;
        const max = range.max;

        legendMin = min;
        legendMax = max;
        drawLegend(min, max, property);

        for (let i = 0; i < len; i++) {
            const value = values[i];
            if (!Number.isFinite(value)) {
                tmpColor.set(0x888888);
                instancedMesh.setColorAt(i, tmpColor);
                continue;
            }
            let t = (value - min) / (max - min);
            if (!Number.isFinite(t)) t = 0;
            t = Math.max(0, Math.min(1, t));
            const hue = getHue(property, t);
            tmpColor.setHSL(hue, 0.85, 0.55);
            instancedMesh.setColorAt(i, tmpColor);
        }
        instancedMesh.instanceColor.needsUpdate = true;
    }

    function updateWellVisualization(wells: WellState): void {
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
            const w = well as WellStateEntry;
            const i = Number(w.i ?? w.ix ?? 0);
            const j = Number(w.j ?? w.jy ?? 0);
            const k = Number(w.k ?? w.k ?? 0);
            
            const colKey = `${i},${j}`;
            if (!wellColumns.has(colKey)) {
                wellColumns.add(colKey);

                // Find topmost cell with this well (minimum k)
                let topK = k;
                for (const other of wells) {
                    const o = other as WellStateEntry;
                    const oi = Number(o.i ?? o.ix ?? 0);
                    const oj = Number(o.j ?? o.jy ?? 0);
                    const ok = Number(o.k ?? o.k ?? 0);
                    if (oi === i && oj === j && ok < topK) {
                        topK = ok;
                    }
                }

                // Draw cylinder from top face (NZ=0) going up
                const xOff = (nx - 1) * 0.5;
                const yOff = (ny - 1) * 0.5;
                const zOff = (nz - 1) * 0.5;
                const cellSize = getVisualCellSizes();
                const wellRadius = Math.max(0.08 * Math.min(cellSize.x, cellSize.y), 0.05);
                const wellHeight = Math.max(cellSize.z * 2, Math.min(cellSize.x, cellSize.y));

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
                // k=0 corresponds to z = (0 - zOff) * cellSize.z
                const cellCenterX = (i - xOff) * cellSize.x;
                const cellCenterY = (j - yOff) * cellSize.y;
                const topSurfaceZ = (nz - zOff) * cellSize.z - cellSize.z * 0.5;
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
            tmpColor.setHSL(getHue(property, t), 1.0, 0.5);
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
    <div class="w-full">
        <div class="flex items-center gap-3 w-full">
            <input
                type="range"
                class="range range-sm flex-1"
                min="0"
                max={Math.max(0, history.length - 1)}
                bind:value={currentIndex}
                oninput={applySliderValue}
                onchange={applySliderValue}
            />
            <div class="flex flex-col items-end text-right">
                <div class="text-xs opacity-80">Step: {currentIndex} / {Math.max(0, history.length - 1)}</div>
                {#if replayTime !== null}
                    <div class="text-xs opacity-80">Replay Time: {replayTime.toFixed(2)} days</div>
                {/if}
            </div>
        </div>
    </div>
    <label class="form-control mt-3">
        <div class="flex flex-wrap items-center gap-2">
            <div class="label-text text-xs self-center mr-1">Property</div>
            <div class="flex flex-wrap gap-1">
                {#each showPropertyOptions as option}
                    <button
                        type="button"
                        class="btn btn-xs {showProperty === option.value ? 'btn-primary' : 'btn-outline'}"
                        onclick={() => showProperty = option.value}
                    >
                        {option.label}
                    </button>
                {/each}
            </div>
        </div>
    </label>
    <div class="flex items-start gap-4" style="margin-left:4px; align-items:center;">
        <div class="legend" style="margin:0;">
            <canvas bind:this={legendCanvas} width="300" height="18" style="width:200px;height:14px"></canvas>
        </div>
                    <span class="label-text text-xs">Min</span>
                    <div
                        class="flex items-center gap-2 rounded-md border border-base-300 bg-base-100 p-1 transition-colors"
                    >
                        <input
                            type="number"
                            step="any"
                            class="input input-bordered input-sm w-full"
                            value={legendFixedMin}
                            oninput={onLegendMinInput}
                        />
                        <button type="button" class="btn btn-xs btn-outline" onclick={applyModelLegendMin}>Auto</button>
                        
                    </div>
                    <span class="label-text text-xs">Max</span>
                    <div
                        class="flex items-center gap-2 rounded-md border border-base-300 bg-base-100 p-1 transition-colors"
                    >
                        <input
                            type="number"
                            step="any"
                            class="input input-bordered input-sm w-full"
                            value={legendFixedMax}
                            oninput={onLegendMaxInput}
                        />
                        <button type="button" class="btn btn-xs btn-outline" onclick={applyModelLegendMax}>Auto</button>
                        
                    </div>
            
            </div>
            <div class="viz" bind:this={canvasContainer} style="position:relative;">
        {#if tooltipVisible}
            <div style="position:absolute; left:{tooltipX}px; top:{tooltipY}px; background:rgba(0,0,0,0.85); color:#fff; padding:6px 8px; border-radius:4px; font-size:11px; pointer-events:none; white-space:pre-line; line-height:1.4; z-index:1000; border:1px solid #ddd;">
                {tooltipContent}
            </div>
        {/if}
    </div>
</div>
<style>
    /* Revert to non-absolute sizing so parent controls height explicitly */
    .viz { border: 1px solid #ddd; width: 100%; height: clamp(255px, 37vh, 440px); position: relative; background: #fff; }
    .legend { margin-top: 8px; margin-bottom: 8px; color: #222; display:flex; align-items:center; gap:8px; }
    .legend canvas { border: 1px solid #ccc; background: #fff; }
</style>
