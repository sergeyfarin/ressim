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

    type PropertyKey = 'pressure' | 'saturation_water' | 'saturation_oil';

    export let nx = 20;
    export let ny = 10;
    export let nz = 10;
    export let gridState: GridCell[] | null = null;
    export let showProperty: PropertyKey = 'pressure';
    export let history: HistoryEntry[] = [];
    export let currentIndex = -1;
    export let wellState: unknown = null;

    let renderer: THREE.WebGLRenderer | null = null;
    let scene: THREE.Scene | null = null;
    let controls: OrbitControls | null = null;
    let camera: ThreePerspectiveCamera | null = null;
    let instancedMesh: ThreeInstancedMesh | null = null;
    let wireframeGroup: THREE.Group | null = null;
    let wellsGroup: THREE.Group | null = null;
    let animationId: number | null = null;
    let legendCanvas: HTMLCanvasElement | null = null;
    let topRightLegendCanvas: HTMLCanvasElement | null = null;
    let canvasContainer: HTMLElement | null = null;
    let legendMin = 0;
    let legendMax = 1;
    let lastDimsKey = '';
    
    // Fixed ranges for constant color mapping across timesteps
    let fixedRanges: Record<string, { min: number; max: number }> = {
        'pressure': { min: 0, max: 500 },
        'saturation_water': { min: 0.2, max: 0.8 },
        'saturation_oil': { min: 0.2, max: 0.8 }
    };

    let activeGrid: GridCell[] | null = null;

    const tmpColor = new THREE.Color();
    const tmpMatrix = new THREE.Matrix4();
    
    // Tooltip state
    let tooltipVisible = false;
    let tooltipX = 0;
    let tooltipY = 0;
    let tooltipContent = '';
    let raycaster = new THREE.Raycaster();
    let mouse = new THREE.Vector2();

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

    // Trigger on well state changes
    $: if (scene && wellState) {
        updateWellVisualization(wellState as unknown as unknown[]);
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
        scene.add(directionalLight);    
        
        const gridSize = Math.max(nx, ny, nz)*4.5;
        const newCamera = new THREE.PerspectiveCamera(
            7, 
            1, 
            10, 
            1000
        ) as ThreePerspectiveCamera;
        
        // Position camera at an angle to see 3D depth
        newCamera.position.set(gridSize * 1.2, -gridSize * 1.8, gridSize * 0.8);
        newCamera.up.set(0, 0, 1);
        newCamera.lookAt(0, 0, 0);
    
        camera = newCamera;

        renderer = new THREE.WebGLRenderer({ antialias: true });
        renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
        renderer.setSize(width, height, false);
        renderer.setClearColor(0xf6f6f6);

        if (canvasContainer) {
            // Clear existing children except for the legend canvas
            while (canvasContainer.firstChild) {
                if (canvasContainer.firstChild !== topRightLegendCanvas) {
                    canvasContainer.removeChild(canvasContainer.firstChild);
                } else {
                    break;
                }
            }
            canvasContainer.appendChild(renderer.domElement);
            if (topRightLegendCanvas && topRightLegendCanvas.parentNode !== canvasContainer) {
                canvasContainer.appendChild(topRightLegendCanvas);
            }
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
        renderer.domElement.addEventListener('mousemove', onCanvasMouseMove, { passive: true });

        animate();
    }

    function onWindowResize(): void {
        if (!canvasContainer || !renderer || !camera) return;
        const w = canvasContainer.clientWidth || 800;
        const h = canvasContainer.clientHeight || 600;
        
        const perspectiveCamera = camera as THREE.PerspectiveCamera;
        perspectiveCamera.aspect = w / h;
        perspectiveCamera.updateProjectionMatrix();
        renderer.setSize(w, h, false);
    }

    function onCanvasMouseMove(event: MouseEvent): void {
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
                if (child instanceof THREE.LineSegments) {
                    child.geometry.dispose();
                    (child.material as THREE.Material).dispose();
                }
            });
            wireframeGroup = null;
        }

        // Initialize wells group if not already done
        if (!wellsGroup) {
            wellsGroup = new THREE.Group();
            scene.add(wellsGroup);
        } else {
            wellsGroup.clear();
        }

        const total = nx * ny * nz;
        if (total === 0) return;

        const boxSize = 1.0;
        const geometry = new THREE.BoxGeometry(boxSize, boxSize, boxSize);
        
        // MeshStandardMaterial with vertexColors works well with instance colors
        const material = new THREE.MeshStandardMaterial({ 
            vertexColors: true,
            roughness: 0.8,
            metalness: 0.0,
            wireframe: false
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
                    tmpMatrix.makeTranslation((i - xOff) * boxSize, (j - yOff) * boxSize, (k - zOff) * boxSize);
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

        // Create wireframe edges for each cell using LineSegments (only edges, no diagonals)
        wireframeGroup = new THREE.Group();
        const edgesGeometry = new THREE.EdgesGeometry(geometry);
        const lineMaterial = new THREE.LineBasicMaterial({ 
            color: 0x000000,
            transparent: true,
            opacity: 0.8,
            depthTest: true,
            depthWrite: false
        });
        
        idx = 0;
        for (let k = 0; k < nz; k++) {
            for (let j = 0; j < ny; j++) {
                for (let i = 0; i < nx; i++) {
                    const lineSegments = new THREE.LineSegments(edgesGeometry, lineMaterial);
                    tmpMatrix.makeTranslation((i - xOff) * boxSize, (j - yOff) * boxSize, (k - zOff) * boxSize);
                    lineSegments.applyMatrix4(tmpMatrix);
                    wireframeGroup.add(lineSegments);
                    idx++;
                }
            }
        }
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

    function applyGridToInstances(gridArray: GridCell[], property: PropertyKey): void {
        if (!instancedMesh) return;

        const instAttr = instancedMesh.instanceColor;
        if (!instAttr) return;

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
            } else {
                rawValue = NaN;
            }
            
            values.push(rawValue);
        }

        // Use fixed ranges for consistent color mapping
        const range = fixedRanges[property] ?? { min: 0, max: 1 };
        let min = range.min;
        let max = range.max;

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

    function updateWellVisualization(wells: unknown[]): void {
        if (!scene || !wellsGroup) return;

        // Remove old wells
        scene.remove(wellsGroup);
        wellsGroup.traverse((child) => {
            if (child instanceof THREE.Mesh) {
                child.geometry.dispose();
                (child.material as THREE.Material).dispose();
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

                const wellCylinder = new THREE.Mesh(
                    new THREE.CylinderGeometry(wellRadius, wellRadius, wellHeight, 16),
                    new THREE.MeshStandardMaterial({
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
        
        // Draw top-right legend
        if (topRightLegendCanvas) {
            const ctx2 = topRightLegendCanvas.getContext('2d');
            if (ctx2) {
                const w2 = topRightLegendCanvas.width;
                const h2 = topRightLegendCanvas.height;
                const gradientH = ctx2.createLinearGradient(0, 0, w2, 0);
                for (let i = 0; i <= steps; i++) {
                    const t = i / steps;
                    tmpColor.setHSL((1 - t) * 0.6, 1.0, 0.5);
                    gradientH.addColorStop(t, tmpColor.getStyle());
                }
                
                ctx2.clearRect(0, 0, w2, h2);
                ctx2.fillStyle = gradientH;
                ctx2.fillRect(0, 0, w2, h2);
                
                ctx2.strokeStyle = 'rgba(0,0,0,0.8)';
                ctx2.lineWidth = 1;
                ctx2.strokeRect(0.5, 0.5, w2 - 1, h2 - 1);
                
                ctx2.font = 'bold 10px sans-serif';
                ctx2.fillStyle = '#000';
                ctx2.textBaseline = 'bottom';
                ctx2.textAlign = 'left';
                ctx2.fillText(min.toFixed(2), 4, h2 - 4);
                ctx2.textAlign = 'right';
                ctx2.fillText(max.toFixed(2), w2 - 4, h2 - 4);
                
                ctx2.font = 'bold 9px sans-serif';
                ctx2.fillStyle = '#333';
                ctx2.textAlign = 'center';
                let propLabel = 'Pressure';
                if (showProperty === 'saturation_water') propLabel = 'Water Sat.';
                else if (showProperty === 'saturation_oil') propLabel = 'Oil Sat.';
                ctx2.fillText(propLabel, w2 / 2, 10);
            }
        }
    }
</script>
<div style="display:flex; flex-direction:column;">
    <div class="viz" bind:this={canvasContainer} style="position:relative;">
        <canvas 
            bind:this={topRightLegendCanvas} 
            width="160" 
            height="70" 
            style="position:absolute; top:10px; right:10px; width:160px; height:70px; background:rgba(255,255,255,0.9); border:1px solid #999; border-radius:4px; padding:8px; box-sizing:border-box; cursor:default; pointer-events:none;">
        </canvas>
        {#if tooltipVisible}
            <div style="position:absolute; left:{tooltipX}px; top:{tooltipY}px; background:rgba(0,0,0,0.85); color:#fff; padding:6px 8px; border-radius:4px; font-size:11px; pointer-events:none; white-space:pre-line; line-height:1.4; z-index:1000; border:1px solid #ddd;">
                {tooltipContent}
            </div>
        {/if}
    </div>
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
