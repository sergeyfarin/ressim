<script lang="ts">
    import { onDestroy, onMount, tick } from "svelte";
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
    } from "three";
    import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
    import type { Material } from "three";

    import type {
        GridState,
        SimulatorSnapshot,
        WellState,
        WellStateEntry,
    } from "../simulator-types";
    import ToggleGroup from "../ui/controls/ToggleGroup.svelte";

    type HistoryEntry = SimulatorSnapshot;

    type PropertyKey =
        | "pressure"
        | "saturation_water"
        | "saturation_oil"
        | "saturation_gas"
        | "saturation_ternary";

    type RgbTriplet = [number, number, number];
    type OklabTriplet = [number, number, number];

    const TERNARY_WATER_COLOR = new Color("#38bdf8");
    const TERNARY_OIL_COLOR = new Color("#4ade80");
    const TERNARY_GAS_COLOR = new Color("#fb7185");
    const TERNARY_WATER_SRGB: RgbTriplet = [0.2196, 0.7412, 0.9725];
    const TERNARY_OIL_SRGB: RgbTriplet = [0.2902, 0.8706, 0.502];
    const TERNARY_GAS_SRGB: RgbTriplet = [0.9843, 0.4431, 0.5216];
    const TERNARY_WATER_OKLAB = srgbToOklab(TERNARY_WATER_SRGB);
    const TERNARY_OIL_OKLAB = srgbToOklab(TERNARY_OIL_SRGB);
    const TERNARY_GAS_OKLAB = srgbToOklab(TERNARY_GAS_SRGB);

    export let nx: number = 20;
    export let ny: number = 10;
    export let nz: number = 10;
    export let cellDx: number = 10;
    export let cellDy: number = 10;
    export let cellDz: number = 1;
    export let gridState: GridState | null = null;
    export let cellDzPerLayer: number[] = [];
    export let showProperty: PropertyKey = "pressure";
    export let history: HistoryEntry[] = [];
    export let currentIndex: number = -1;
    export let wellState: WellState | null = null;
    export let sourceLabel: string = "Live runtime";
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
    export let theme: "dark" | "light" = "dark";

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
    let lastDimsKey = "";
    let lastLegendModeKey = "";

    // Reactive grid reference
    let activeGrid: GridState | null = null;

    // Tooltip state
    let tooltipVisible = false;
    let tooltipX = 0;
    let tooltipY = 0;
    let tooltipContent = "";
    let raycaster = new Raycaster();
    let mouse = new Vector2();
    let tooltipRafId: number | null = null;

    let visibleCellIndices: number[] = [];
    let latestMouseEvent: MouseEvent | null = null;
    let modelRadius = 100; // Tracks the bounding radius, updated by fitCamera

    // Helpers used in instancing and color mapping
    const tmpMatrix = new Matrix4();
    const tmpColor = new Color();
    const Z_VISUAL_EXAGGERATION = 10;

    type VisualReservoirMetrics = {
        cellSizeX: number;
        cellSizeY: number;
        layerThicknesses: number[];
        layerCenters: number[];
        totalThickness: number;
        maxLayerThickness: number;
        topSurfaceZ: number;
    };

    // Fixed color ranges per property to keep legend stable
    // Pressure is intentionally auto-scaled from current values for better contrast.
    const fixedRanges: Record<PropertyKey, { min: number; max: number }> = {
        pressure: { min: 0, max: 1000 },
        saturation_water: { min: 0, max: 1 },
        saturation_oil: { min: 0, max: 1 },
        saturation_gas: { min: 0, max: 1 },
        saturation_ternary: { min: 0, max: 1 },
    };

    const propertyDisplay: Record<
        PropertyKey,
        { label: string; unit: string; decimals: number }
    > = {
        pressure: { label: "Pressure", unit: "bar", decimals: 2 },
        saturation_water: {
            label: "Water Saturation",
            unit: "fraction",
            decimals: 3,
        },
        saturation_oil: {
            label: "Oil Saturation",
            unit: "fraction",
            decimals: 3,
        },
        saturation_gas: {
            label: "Gas Saturation",
            unit: "fraction",
            decimals: 3,
        },
        saturation_ternary: {
            label: "Ternary Saturation",
            unit: "blend",
            decimals: 3,
        },
    };

    const showPropertyOptions: Array<{ value: PropertyKey; label: string }> = [
        { value: "pressure", label: "Pressure" },
        { value: "saturation_water", label: "Water Sat" },
        { value: "saturation_oil", label: "Oil Sat" },
        { value: "saturation_gas", label: "Gas Sat" },
        { value: "saturation_ternary", label: "Ternary Sat" },
    ];

    let groupSummary = "";

    $: groupSummary = `${showProperty.replace("_", " ")} · snapshot ${Math.max(0, currentIndex)}`;

    $: {
        const minValue = Number(legendFixedMin);
        const maxValue = Number(legendFixedMax);

        if (!Number.isFinite(minValue)) {
            legendFixedMin = 0;
        }
        if (!Number.isFinite(maxValue)) {
            legendFixedMax = 1;
        }

        if (
            Number.isFinite(legendFixedMin) &&
            Number.isFinite(legendFixedMax) &&
            legendFixedMin > legendFixedMax
        ) {
            const tmp = legendFixedMin;
            legendFixedMin = legendFixedMax;
            type VisualReservoirMetrics = {
                cellSizeX: number;
                cellSizeY: number;
                layerThicknesses: number[];
                layerCenters: number[];
                totalThickness: number;
                maxLayerThickness: number;
                topSurfaceZ: number;
            };
            legendFixedMax = tmp;
        }
    }

    function clamp(value: number, min: number, max: number): number {
        return Math.min(max, Math.max(min, value));
    }

    function srgbChannelToLinear(value: number): number {
        if (value <= 0.04045) return value / 12.92;
        return Math.pow((value + 0.055) / 1.055, 2.4);
    }

    function linearChannelToSrgb(value: number): number {
        const clamped = clamp(value, 0, 1);
        if (clamped <= 0.0031308) return clamped * 12.92;
        return 1.055 * Math.pow(clamped, 1 / 2.4) - 0.055;
    }

    function srgbToOklab([red, green, blue]: RgbTriplet): OklabTriplet {
        const linearRed = srgbChannelToLinear(red);
        const linearGreen = srgbChannelToLinear(green);
        const linearBlue = srgbChannelToLinear(blue);

        const l =
            0.4122214708 * linearRed +
            0.5363325363 * linearGreen +
            0.0514459929 * linearBlue;
        const m =
            0.2119034982 * linearRed +
            0.6806995451 * linearGreen +
            0.1073969566 * linearBlue;
        const s =
            0.0883024619 * linearRed +
            0.2817188376 * linearGreen +
            0.6299787005 * linearBlue;

        const lRoot = Math.cbrt(l);
        const mRoot = Math.cbrt(m);
        const sRoot = Math.cbrt(s);

        return [
            0.2104542553 * lRoot +
                0.793617785 * mRoot -
                0.0040720468 * sRoot,
            1.9779984951 * lRoot -
                2.428592205 * mRoot +
                0.4505937099 * sRoot,
            0.0259040371 * lRoot +
                0.7827717662 * mRoot -
                0.808675766 * sRoot,
        ];
    }

    function oklabToSrgb([lightness, aAxis, bAxis]: OklabTriplet): RgbTriplet {
        const lRoot =
            lightness + 0.3963377774 * aAxis + 0.2158037573 * bAxis;
        const mRoot =
            lightness - 0.1055613458 * aAxis - 0.0638541728 * bAxis;
        const sRoot =
            lightness - 0.0894841775 * aAxis - 1.291485548 * bAxis;

        const l = lRoot * lRoot * lRoot;
        const m = mRoot * mRoot * mRoot;
        const s = sRoot * sRoot * sRoot;

        const linearRed =
            4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s;
        const linearGreen =
            -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s;
        const linearBlue =
            -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s;

        return [
            linearChannelToSrgb(linearRed),
            linearChannelToSrgb(linearGreen),
            linearChannelToSrgb(linearBlue),
        ];
    }

    function getTernaryBlendSrgb(
        waterWeight: number,
        oilWeight: number,
        gasWeight: number,
    ): RgbTriplet {
        const total = waterWeight + oilWeight + gasWeight;
        if (!Number.isFinite(total) || total <= 1e-9) {
            return [0.5, 0.5, 0.5];
        }

        const normalizedWater = waterWeight / total;
        const normalizedOil = oilWeight / total;
        const normalizedGas = gasWeight / total;

        return oklabToSrgb([
            TERNARY_WATER_OKLAB[0] * normalizedWater +
                TERNARY_OIL_OKLAB[0] * normalizedOil +
                TERNARY_GAS_OKLAB[0] * normalizedGas,
            TERNARY_WATER_OKLAB[1] * normalizedWater +
                TERNARY_OIL_OKLAB[1] * normalizedOil +
                TERNARY_GAS_OKLAB[1] * normalizedGas,
            TERNARY_WATER_OKLAB[2] * normalizedWater +
                TERNARY_OIL_OKLAB[2] * normalizedOil +
                TERNARY_GAS_OKLAB[2] * normalizedGas,
        ]);
    }

    function isTernaryBlend(property: PropertyKey): boolean {
        return property === "saturation_ternary";
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
        return (
            history.length > 0 &&
            currentIndex >= 0 &&
            currentIndex < history.length
        );
    }

    function getExpectedCellCount(): number {
        return Math.max(0, Number(nx) * Number(ny) * Number(nz));
    }

    function getValidHistoryGrids(): GridState[] {
        const expectedCount = getExpectedCellCount();
        if (
            expectedCount <= 0 ||
            !Array.isArray(history) ||
            history.length === 0
        )
            return [];
        const grids: GridState[] = [];
        for (const entry of history) {
            const grid = entry?.grid;
            if (
                grid &&
                grid.pressure &&
                grid.pressure.length === expectedCount
            ) {
                grids.push(grid);
            }
        }
        return grids;
    }

    function getStaticReferenceGrid(): GridState | null {
        const historyGrids = getValidHistoryGrids();
        if (historyGrids.length > 0) return historyGrids[0];
        const expectedCount = getExpectedCellCount();
        if (
            expectedCount > 0 &&
            gridState &&
            gridState.pressure &&
            gridState.pressure.length === expectedCount
        ) {
            return gridState;
        }
        return null;
    }

    function getPropertyValuesFromGrid(
        grid: GridState | null | undefined,
        property: PropertyKey,
    ): number[] {
        if (!grid || !grid.pressure || grid.pressure.length === 0) return [];
        const values = [];
        for (let i = 0; i < grid.pressure.length; i++) {
            values.push(getCellPropertyValue(grid, i, property));
        }
        return values.filter((value) => Number.isFinite(value));
    }

    function getHistoryPropertyRange(
        property: PropertyKey,
    ): { min: number; max: number } | null {
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

    function getCellPropertyValue(
        grid: GridState | null | undefined,
        index: number,
        property: PropertyKey,
    ): number {
        if (!grid) return NaN;
        if (property === "pressure")
            return Number(grid.pressure?.[index] ?? NaN);
        if (property === "saturation_water")
            return Number(grid.sat_water?.[index] ?? NaN);
        if (property === "saturation_oil")
            return Number(grid.sat_oil?.[index] ?? NaN);
        if (property === "saturation_gas")
            return Number(grid.sat_gas?.[index] ?? NaN);
        if (property === "saturation_ternary") return NaN;
        return NaN;
    }

    function getPhaseSaturations(
        grid: GridState | null | undefined,
        index: number,
    ): { water: number; oil: number; gas: number } | null {
        if (!grid) return null;

        const water = Number(grid.sat_water?.[index] ?? NaN);
        const oil = Number(grid.sat_oil?.[index] ?? NaN);
        const gas = Number(grid.sat_gas?.[index] ?? NaN);

        if (
            !Number.isFinite(water) ||
            !Number.isFinite(oil) ||
            !Number.isFinite(gas)
        ) {
            return null;
        }

        return {
            water: Math.max(0, water),
            oil: Math.max(0, oil),
            gas: Math.max(0, gas),
        };
    }

    function getTernaryBlendColor(
        grid: GridState | null | undefined,
        index: number,
    ): Color | null {
        const phases = getPhaseSaturations(grid, index);
        if (!phases) return null;

        const total = phases.water + phases.oil + phases.gas;
        if (!Number.isFinite(total) || total <= 1e-9) return null;

        const waterWeight = phases.water / total;
        const oilWeight = phases.oil / total;
        const gasWeight = phases.gas / total;
        const [red, green, blue] = getTernaryBlendSrgb(
            waterWeight,
            oilWeight,
            gasWeight,
        );

        tmpColor.setRGB(red, green, blue);
        tmpColor.convertSRGBToLinear();

        return tmpColor;
    }

    function getModelLegendRange(property: PropertyKey): {
        min: number;
        max: number;
    } {
        const fixed = fixedRanges[property] ?? { min: 0, max: 1 };

        function roundLegendBound(val: number, isMax: boolean): number {
            if (!Number.isFinite(val) || val === 0) return val;
            const absVal = Math.abs(val);
            const sign = Math.sign(val);

            // If the value has 2 or more digits before decimal point (>= 10)
            if (absVal >= 10) {
                // Determine magnitude for 2 significant digits
                const mag = Math.pow(10, Math.floor(Math.log10(absVal)) - 1);
                // For max, round up. For min, round down. Account for sign.
                if ((isMax && sign > 0) || (!isMax && sign < 0)) {
                    return Math.ceil(absVal / mag) * mag * sign;
                } else {
                    return Math.floor(absVal / mag) * mag * sign;
                }
            } else if (absVal >= 1) {
                // If value is between 1 and 10, keep 1 decimal place max (2 sig figs)
                if ((isMax && sign > 0) || (!isMax && sign < 0)) {
                    return (Math.ceil(absVal * 10) / 10) * sign;
                } else {
                    return (Math.floor(absVal * 10) / 10) * sign;
                }
            } else {
                // Less than 1, keep 2 significant digits
                const mag = Math.pow(10, Math.floor(Math.log10(absVal)) - 1);
                if ((isMax && sign > 0) || (!isMax && sign < 0)) {
                    return Math.ceil(absVal / mag) * mag * sign;
                } else {
                    return Math.floor(absVal / mag) * mag * sign;
                }
            }
        }

        if (property === "saturation_water") {
            const swc = clamp(Number(s_wc), 0, 0.95);
            const sor = clamp(Number(s_or), 0, 0.95);
            const min = swc;
            const max = Math.max(min + 1e-6, 1 - sor);
            return { min, max };
        }

        if (property === "saturation_oil") {
            const swc = clamp(Number(s_wc), 0, 0.95);
            const sor = clamp(Number(s_or), 0, 0.95);
            const min = sor;
            const max = Math.max(min + 1e-6, 1 - swc);
            return { min, max };
        }

        if (property === "saturation_gas") {
            const swc = clamp(Number(s_wc), 0, 0.95);
            const min = swc;
            const max = Math.max(min + 1e-6, 1 - swc);
            return { min, max };
        }

        if (property === "saturation_ternary") {
            return fixed;
        }

        if (property === "pressure") {
            const historyRange = getHistoryPropertyRange("pressure");
            if (historyRange) {
                return {
                    min: roundLegendBound(historyRange.min, false),
                    max: roundLegendBound(historyRange.max, true),
                };
            }
            const values = getPropertyValuesFromGrid(activeGrid, property);
            if (values.length === 0) {
                return fixed;
            }
            const min = Math.min(...values);
            const dataMax = Math.max(...values);
            const max = Number.isFinite(dataMax)
                ? Math.max(min + 1e-6, dataMax)
                : Math.max(min + 1e-6, fixed.max);
            return {
                min: roundLegendBound(min, false),
                max: roundLegendBound(max, true),
            };
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
        return {
            min: roundLegendBound(min, false),
            max: roundLegendBound(max, true),
        };
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
        return Number.isFinite(value) ? value.toFixed(decimals) : "n/a";
    }

    function getPropertyDisplay(property: PropertyKey): {
        label: string;
        unit: string;
        decimals: number;
    } {
        return (
            propertyDisplay[property] ?? {
                label: "Property",
                unit: "-",
                decimals: 3,
            }
        );
    }

    function computeLegendRange(
        property: PropertyKey,
        values: number[],
    ): { min: number; max: number } {
        const fixed = fixedRanges[property] ?? { min: 0, max: 1 };
        const userMin = Number(legendFixedMin);
        const userMax = Number(legendFixedMax);
        if (
            Number.isFinite(userMin) &&
            Number.isFinite(userMax) &&
            userMax > userMin
        ) {
            return { min: userMin, max: userMax };
        }
        return fixed;
    }

    function getHue(property: PropertyKey, t: number): number {
        if (property === "saturation_water") {
            return t * 0.66;
        }
        return (1 - t) * 0.66;
    }

    onMount(() => {
        try {
            const _w = window as typeof window & { __ressim?: unknown };
            _w.__ressim = _w.__ressim || {
                renderer: null,
                scene: null,
                camera: null,
                instancedMesh: null,
            };
        } catch (e) {
            // ignore debug exposure failures outside of browser envs
        }
        initThree();
        buildInstancedGrid(true);
        lastDimsKey = getDimensionsKey();
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
            renderer.domElement.removeEventListener(
                "mousemove",
                onCanvasMouseMove,
            );
        }
        if (tooltipRafId !== null) {
            cancelAnimationFrame(tooltipRafId);
            tooltipRafId = null;
        }
        renderer?.dispose();
        renderer?.forceContextLoss?.();
        window.removeEventListener("resize", onWindowResize);
    });

    // Compute activeGrid reactively — explicitly reference every dependency
    // so Svelte's compiler can track them (function-body refs are invisible).
    $: {
        gridState;
        history;
        history.length;
        currentIndex;
        nx;
        ny;
        nz;
        activeGrid = getActiveGrid();
    }

    $: {
        const dimsKey = getDimensionsKey();
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

    function getDimensionsKey(): string {
        const layerSignature = Array.isArray(cellDzPerLayer)
            ? cellDzPerLayer
                  .slice(0, Math.max(0, nz))
                  .map((value) => String(Number(value)))
                  .join(",")
            : "";
        return `${nx}|${ny}|${nz}|${cellDx}|${cellDy}|${cellDz}|${layerSignature}`;
    }

    function getVisualReservoirMetrics(): VisualReservoirMetrics {
        const { x: cellSizeX, y: cellSizeY } = getVisualCellSizes();
        const fallbackThickness = Math.max(0.001, Number(cellDz) || 1);
        const layerThicknesses = Array.from(
            { length: Math.max(0, nz) },
            (_, k) => {
                const rawThickness = Array.isArray(cellDzPerLayer)
                    ? Number(cellDzPerLayer[k])
                    : Number.NaN;
                const thickness =
                    Number.isFinite(rawThickness) && rawThickness > 0
                        ? rawThickness
                        : fallbackThickness;
                return thickness * Z_VISUAL_EXAGGERATION;
            },
        );
        const totalThickness = Math.max(
            0.001,
            layerThicknesses.reduce((sum, thickness) => sum + thickness, 0),
        );
        const maxLayerThickness = Math.max(
            0.001,
            ...layerThicknesses,
            fallbackThickness * Z_VISUAL_EXAGGERATION,
        );
        const layerCenters: number[] = [];
        let currentTop = totalThickness * 0.5;
        for (const thickness of layerThicknesses) {
            layerCenters.push(currentTop - thickness * 0.5);
            currentTop -= thickness;
        }

        return {
            cellSizeX,
            cellSizeY,
            layerThicknesses,
            layerCenters,
            totalThickness,
            maxLayerThickness,
            topSurfaceZ: totalThickness * 0.5,
        };
    }

    function fitCameraToReservoir(metrics: VisualReservoirMetrics): void {
        if (!camera || !canvasContainer) return;

        const perspectiveCamera = camera as PerspectiveCamera;
        const aspect = Math.max(
            1e-3,
            canvasContainer.clientWidth /
                Math.max(1, canvasContainer.clientHeight),
        );
        perspectiveCamera.aspect = aspect;

        const halfX = nx * metrics.cellSizeX * 0.5;
        const halfY = ny * metrics.cellSizeY * 0.5;
        const halfZ = metrics.totalThickness * 0.5;
        const radius = Math.max(
            0.001,
            Math.sqrt(halfX * halfX + halfY * halfY + halfZ * halfZ),
        );

        // Store radius for dynamic clipping updates
        modelRadius = radius;

        const verticalFov = (perspectiveCamera.fov * Math.PI) / 60;
        const horizontalFov =
            2 *
            Math.atan(Math.tan(verticalFov * 0.5) * perspectiveCamera.aspect);
        const fitDistanceV =
            radius / Math.max(Math.sin(verticalFov * 0.5), 1e-3);
        const fitDistanceH =
            radius / Math.max(Math.sin(horizontalFov * 0.5), 1e-3);
        const fitDistance = Math.max(fitDistanceV, fitDistanceH) * 2.1;

        const dirX = 1.2;
        const dirY = -1.8;
        const dirZ = 0.8;
        const dirLen = Math.sqrt(dirX * dirX + dirY * dirY + dirZ * dirZ);
        const ux = dirX / dirLen;
        const uy = dirY / dirLen;
        const uz = dirZ / dirLen;

        perspectiveCamera.position.set(
            ux * fitDistance,
            uy * fitDistance,
            uz * fitDistance,
        );
        perspectiveCamera.up.set(0, 0, 1);
        // Use generous near/far based on radius
        perspectiveCamera.near = Math.max(0.01, radius * 0.001);
        perspectiveCamera.far = Math.max(1000, radius * 100);
        perspectiveCamera.lookAt(0, 0, 0);
        perspectiveCamera.updateProjectionMatrix();

        if (controls) {
            controls.target.set(0, 0, 0);
            controls.minDistance = Math.max(0.1, radius * 0.3);
            controls.maxDistance = Math.max(50, radius * 20);
            controls.update();
        }
    }

    let lastModelLegendKey = "";
    let pressureHistoryScanCount = 0;

    function getLegendContextKey(property: PropertyKey): string {
        const dimsKey = getDimensionsKey();

        if (property === "pressure") {
            return `${property}|${dimsKey}`;
        }
        if (
            property === "saturation_water" ||
            property === "saturation_oil" ||
            property === "saturation_gas"
        ) {
            return `${property}|${dimsKey}|${s_wc}|${s_or}`;
        }

        if (property === "saturation_ternary") {
            return `${property}|${dimsKey}`;
        }

        const staticRefLen = getStaticReferenceGrid()?.pressure?.length ?? 0;
        return `${property}|${dimsKey}|${staticRefLen}`;
    }

    function getPressureMaxFromHistorySlice(startIdx: number): number | null {
        const expectedCount = getExpectedCellCount();
        if (
            expectedCount <= 0 ||
            !Array.isArray(history) ||
            history.length === 0
        )
            return null;

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
        } else if (showProperty === "pressure") {
            if (history.length < pressureHistoryScanCount) {
                applyModelLegendRange();
                pressureHistoryScanCount = history.length;
            } else if (history.length > pressureHistoryScanCount) {
                const incrementalMax = getPressureMaxFromHistorySlice(
                    pressureHistoryScanCount,
                );
                if (incrementalMax != null) {
                    const currentMin = Number(legendFixedMin);
                    const currentMax = Number(legendFixedMax);
                    const baseMin = Number.isFinite(currentMin)
                        ? currentMin
                        : getModelLegendRange("pressure").min;
                    const baseMax = Number.isFinite(currentMax)
                        ? currentMax
                        : getModelLegendRange("pressure").max;
                    legendFixedMin = baseMin;
                    legendFixedMax = Math.max(
                        baseMax,
                        incrementalMax,
                        baseMin + 1e-6,
                    );
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

    $: if (legendCanvas) {
        const legendModeKey = `${showProperty}|${isTernaryBlend(showProperty) ? "ternary" : "scalar"}`;
        if (legendModeKey !== lastLegendModeKey) {
            lastLegendModeKey = legendModeKey;
            const propertyAtSchedule = showProperty;
            tick().then(() => {
                if (!legendCanvas || showProperty !== propertyAtSchedule) return;
                if (activeGrid) {
                    updateVisualization(activeGrid, propertyAtSchedule);
                } else {
                    clearVisualization(propertyAtSchedule);
                }
            });
        }
    }

    $: if (instancedMesh && !activeGrid) {
        clearVisualization(showProperty);
    }

    // Trigger on well state changes
    $: if (scene) {
        history;
        history.length;
        currentIndex;
        wellState;
        updateWellVisualization(getActiveWellState());
    }

    function getActiveGrid(): GridState | null {
        const expectedCount = nx * ny * nz;
        if (expectedCount <= 0) return null;

        if (
            history.length > 0 &&
            currentIndex >= 0 &&
            currentIndex < history.length
        ) {
            const entry = history[currentIndex];
            const historyGrid = entry?.grid ?? null;
            if (
                historyGrid &&
                historyGrid.pressure &&
                historyGrid.pressure.length === expectedCount
            ) {
                return historyGrid;
            }
        }

        if (
            gridState &&
            gridState.pressure &&
            gridState.pressure.length === expectedCount
        ) {
            return gridState;
        }

        return null;
    }

    function getActiveWellState(): WellState {
        if (
            history.length > 0 &&
            currentIndex >= 0 &&
            currentIndex < history.length &&
            Array.isArray(history[currentIndex]?.wells)
        ) {
            return history[currentIndex]?.wells ?? [];
        }

        return Array.isArray(wellState) ? wellState : [];
    }

    function initThree(): void {
        const width = canvasContainer?.clientWidth ?? 800;
        const height = canvasContainer?.clientHeight ?? 600;

        scene = new Scene();
        const backgroundHex = theme === "dark" ? 0x000000 : 0xf6f6f6;
        scene.background = new Color(backgroundHex);

        // Add lights for MeshStandardMaterial to show colors
        const ambientLight = new AmbientLight(0xffffff, 0.8);
        scene.add(ambientLight);

        const directionalLight = new DirectionalLight(0xffffff, 0.6);
        directionalLight.position.set(5, 10, 7);
        scene.add(directionalLight);

        const gridSize = Math.max(nx, ny, nz) * 4.5;
        const newCamera = new PerspectiveCamera(7, 1, 0.01, 100000);

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

        // Dynamically update near/far clipping planes as user orbits/zooms
        controls.addEventListener("change", () => {
            if (!camera) return;
            const dist = camera.position.length();
            const perspectiveCamera = camera as PerspectiveCamera;
            perspectiveCamera.near = Math.max(0.01, dist * 0.001);
            perspectiveCamera.far = Math.max(
                dist + modelRadius * 20,
                modelRadius * 100,
            );
            perspectiveCamera.updateProjectionMatrix();
        });

        window.addEventListener("resize", onWindowResize);

        renderer.domElement.addEventListener("mousemove", onCanvasMouseMove, {
            passive: true,
        });

        animate();
    }

    $: if (scene && renderer) {
        const backgroundHex = theme === "dark" ? 0x000000 : 0xf6f6f6;
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
        if (
            !renderer ||
            !scene ||
            !camera ||
            !instancedMesh ||
            !canvasContainer
        ) {
            tooltipVisible = false;
            return;
        }

        // Get the active grid - use current gridState or history entry
        const currentGrid = getActiveGrid();
        if (
            !currentGrid ||
            !currentGrid.pressure ||
            currentGrid.pressure.length === 0
        ) {
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
            const instanceId = intersection.instanceId;

            if (
                instanceId !== undefined &&
                instanceId < visibleCellIndices.length
            ) {
                const cellIndex = visibleCellIndices[instanceId];
                if (
                    currentGrid.pressure &&
                    cellIndex < currentGrid.pressure.length
                ) {
                    const i = cellIndex % nx;
                    const j = Math.floor(cellIndex / nx) % ny;
                    const k = Math.floor(cellIndex / (nx * ny));
                    const pressure = Number(
                        currentGrid.pressure?.[cellIndex] ?? 0,
                    );
                    const satWater = Number(
                        currentGrid.sat_water?.[cellIndex] ?? 0,
                    );
                    const satOil = Number(
                        currentGrid.sat_oil?.[cellIndex] ?? 0,
                    );
                    const satGas = Number(
                        currentGrid.sat_gas?.[cellIndex] ?? 0,
                    );

                    const pLabel = `P: ${pressure.toFixed(2)} bar`;
                    const swLabel = `Sw: ${satWater.toFixed(3)}`;
                    const soLabel = `So: ${satOil.toFixed(3)}`;
                    const sgLabel = `Sg: ${satGas.toFixed(3)}`;
                    const ijkLabel = `Cell: i=${i}, j=${j}, k=${k}`;
                    const deckLabel = `Deck: I=${i + 1}, J=${j + 1}, K=${k + 1}`;
                    const rawIndexLabel = `Index: ${cellIndex}`;

                    const bold = (s: string) => `<b>${s}</b>`;
                    const highlightAllPhases = showProperty === "saturation_ternary";
                    tooltipContent =
                        bold(ijkLabel) +
                        "<br>" +
                        deckLabel +
                        "<br>" +
                        rawIndexLabel +
                        "<br>" +
                        (showProperty === "pressure" ? bold(pLabel) : pLabel) +
                        "<br>" +
                        (showProperty === "saturation_water" || highlightAllPhases
                            ? bold(swLabel)
                            : swLabel) +
                        "<br>" +
                        (showProperty === "saturation_oil" || highlightAllPhases
                            ? bold(soLabel)
                            : soLabel) +
                        "<br>" +
                        (showProperty === "saturation_gas" || highlightAllPhases
                            ? bold(sgLabel)
                            : sgLabel);
                    tooltipX = x + 10;
                    tooltipY = y + 10;
                    tooltipVisible = true;
                } else {
                    tooltipVisible = false;
                }
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

        visibleCellIndices = [];
        for (let k = 0; k < nz; k++) {
            for (let j = 0; j < ny; j++) {
                for (let i = 0; i < nx; i++) {
                    const isBoundary =
                        i === 0 ||
                        i === nx - 1 ||
                        j === 0 ||
                        j === ny - 1 ||
                        k === 0 ||
                        k === nz - 1;
                    if (isBoundary) {
                        const cellIndex = i + j * nx + k * nx * ny;
                        visibleCellIndices.push(cellIndex);
                    }
                }
            }
        }

        const meshCount = visibleCellIndices.length;
        if (meshCount === 0) return;

        const metrics = getVisualReservoirMetrics();
        const geometry = new BoxGeometry(1, 1, 1);

        const material = new MeshStandardMaterial({
            color: 0xffffff,
            roughness: 0.8,
            metalness: 0.0,
            wireframe: false,
        });

        const mesh = new InstancedMesh(geometry, material, meshCount);
        instancedMesh = mesh;

        const defaultColor = new Color(0x888888);
        const xOff = (nx - 1) * 0.5;
        const yOff = (ny - 1) * 0.5;

        for (let idx = 0; idx < visibleCellIndices.length; idx++) {
            const cellIndex = visibleCellIndices[idx];
            const i = cellIndex % nx;
            const j = Math.floor(cellIndex / nx) % ny;
            const k = Math.floor(cellIndex / (nx * ny));

            tmpMatrix.makeScale(
                metrics.cellSizeX,
                metrics.cellSizeY,
                metrics.layerThicknesses[k] ?? metrics.maxLayerThickness,
            );
            tmpMatrix.setPosition(
                (i - xOff) * metrics.cellSizeX,
                (j - yOff) * metrics.cellSizeY,
                // Simulator/deck indexing uses k=0 as the top layer.
                // Render with the same vertical ordering and actual layer
                // thicknesses so the 3D view matches solver geometry.
                metrics.layerCenters[k] ?? 0,
            );
            mesh.setMatrixAt(idx, tmpMatrix);
            mesh.setColorAt(idx, defaultColor);
        }

        mesh.instanceMatrix.needsUpdate = true;
        if (mesh.instanceColor) {
            mesh.instanceColor.needsUpdate = true;
        }
        scene.add(mesh);

        if (autoFitCamera) {
            fitCameraToReservoir(metrics);
        }

        // Create a single wireframe outline for the whole reservoir volume
        wireframeGroup = new Group();
        const reservoirGeometry = new BoxGeometry(
            nx * metrics.cellSizeX,
            ny * metrics.cellSizeY,
            metrics.totalThickness,
        );
        const edgesGeometry = new EdgesGeometry(reservoirGeometry);
        reservoirGeometry.dispose();
        const lineMaterial = new LineBasicMaterial({
            color: 0x000000,
            transparent: true,
            opacity: 0.6,
            depthTest: true,
            depthWrite: false,
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

    function updateVisualization(
        gridArray: GridState,
        property: PropertyKey,
    ): void {
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
        if (isTernaryBlend(property)) {
            drawTernaryLegend();
        } else {
            drawLegend(legendMin, legendMax, property);
        }
    }

    function applyGridToInstances(
        gridArray: GridState,
        property: PropertyKey,
    ): void {
        if (!instancedMesh) return;

        if (!instancedMesh.instanceColor) return;

        const defaultColor = new Color(0x888888);
        for (let i = 0; i < instancedMesh.count; i++) {
            instancedMesh.setColorAt(i, defaultColor);
        }

        if (!gridArray || !gridArray.pressure) return;

        if (isTernaryBlend(property)) {
            legendMin = 0;
            legendMax = 1;
            drawTernaryLegend();

            for (let idx = 0; idx < visibleCellIndices.length; idx++) {
                const cellIndex = visibleCellIndices[idx];
                const color = getTernaryBlendColor(gridArray, cellIndex);
                if (!color) {
                    tmpColor.set(0x888888);
                    instancedMesh.setColorAt(idx, tmpColor);
                    continue;
                }
                instancedMesh.setColorAt(idx, color);
            }

            instancedMesh.instanceColor.needsUpdate = true;
            return;
        }

        const values: number[] = [];
        const numInstances = visibleCellIndices.length;

        for (let idx = 0; idx < numInstances; idx++) {
            const cellIndex = visibleCellIndices[idx];
            if (cellIndex < gridArray.pressure.length) {
                values.push(
                    getCellPropertyValue(gridArray, cellIndex, property),
                );
            } else {
                values.push(NaN);
            }
        }

        const range = computeLegendRange(
            property,
            values.filter((v) => Number.isFinite(v)),
        );
        const min = range.min;
        const max = range.max;

        legendMin = min;
        legendMax = max;
        drawLegend(min, max, property);

        for (let idx = 0; idx < numInstances; idx++) {
            const value = values[idx];
            if (!Number.isFinite(value)) {
                tmpColor.set(0x888888);
                instancedMesh.setColorAt(idx, tmpColor);
                continue;
            }
            let t = (value - min) / (max - min);
            if (!Number.isFinite(t)) t = 0;
            t = Math.max(0, Math.min(1, t));
            const hue = getHue(property, t);
            tmpColor.setHSL(hue, 0.85, 0.55);
            instancedMesh.setColorAt(idx, tmpColor);
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
                const metrics = getVisualReservoirMetrics();
                const wellRadius = Math.max(
                    0.08 * Math.min(metrics.cellSizeX, metrics.cellSizeY),
                    0.05,
                );
                const wellHeight = Math.max(
                    metrics.maxLayerThickness * 2,
                    Math.min(metrics.cellSizeX, metrics.cellSizeY),
                );

                const wellCylinder = new Mesh(
                    new CylinderGeometry(
                        wellRadius,
                        wellRadius,
                        wellHeight,
                        16,
                    ),
                    new MeshStandardMaterial({
                        color: 0x8b4513,
                        roughness: 0.6,
                        metalness: 0.3,
                        emissive: 0x3d2817,
                    }),
                );

                // Rotate cylinder to point along Z-axis instead of Y-axis
                wellCylinder.rotation.x = Math.PI / 2;

                const cellCenterX = (i - xOff) * metrics.cellSizeX;
                const cellCenterY = (j - yOff) * metrics.cellSizeY;
                const cellTopZ = metrics.topSurfaceZ + wellHeight * 0.5;

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
        const ctx = legendCanvas.getContext("2d");
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

        ctx.strokeStyle = "rgba(0,0,0,0.6)";
        ctx.strokeRect(0.5, 0.5, w - 1, h - 1);

        ctx.font = "11px sans-serif";
        ctx.fillStyle = "#111";
        ctx.textBaseline = "top";
        ctx.fillText(formatLegendValue(property, min), 2, h + 2);
        const maxText = formatLegendValue(property, max);
        const textWidth = ctx.measureText(maxText).width;
        ctx.fillText(maxText, w - textWidth - 2, h + 2);
    }

    function drawTernaryLegend(): void {
        if (!legendCanvas) return;
        const ctx = legendCanvas.getContext("2d");
        if (!ctx) return;

        const w = legendCanvas.width;
        const h = legendCanvas.height;

        const paddingX = 10;
        const paddingTop = 6;
        const paddingBottom = 8;
        const vertexWater = { x: paddingX, y: h - paddingBottom };
        const vertexOil = { x: w - paddingX, y: h - paddingBottom };
        const vertexGas = { x: w * 0.5, y: paddingTop };

        ctx.clearRect(0, 0, w, h);

        const image = ctx.createImageData(w, h);
        const data = image.data;
        const denom =
            (vertexOil.y - vertexGas.y) * (vertexWater.x - vertexGas.x) +
            (vertexGas.x - vertexOil.x) * (vertexWater.y - vertexGas.y);

        for (let py = 0; py < h; py++) {
            for (let px = 0; px < w; px++) {
                const lWater =
                    ((vertexOil.y - vertexGas.y) * (px - vertexGas.x) +
                        (vertexGas.x - vertexOil.x) * (py - vertexGas.y)) /
                    denom;
                const lOil =
                    ((vertexGas.y - vertexWater.y) * (px - vertexGas.x) +
                        (vertexWater.x - vertexGas.x) * (py - vertexGas.y)) /
                    denom;
                const lGas = 1 - lWater - lOil;

                if (lWater < 0 || lOil < 0 || lGas < 0) continue;

                const [red, green, blue] = getTernaryBlendSrgb(
                    lWater,
                    lOil,
                    lGas,
                );
                const index = (py * w + px) * 4;
                data[index] = Math.round(red * 255);
                data[index + 1] = Math.round(green * 255);
                data[index + 2] = Math.round(blue * 255);
                data[index + 3] = 255;
            }
        }

        ctx.putImageData(image, 0, 0);
    }
</script>

<div style="display:flex; flex-direction:column;">
    <div class="mb-2 px-1">
        <div class="ui-section-kicker">
            Spatial View
        </div>
        <p class="text-xs text-muted-foreground">
            {sourceLabel} spatial snapshot and well placement.
        </p>
    </div>
    <div class="flex items-center gap-4 w-full px-1">
        <input
            type="range"
            class="time-slider flex-1"
            min="0"
            max={Math.max(0, history.length - 1)}
            bind:value={currentIndex}
            oninput={applySliderValue}
            onchange={applySliderValue}
        />
        <div
            class="flex flex-col items-end text-right min-w-35 select-none"
        >
            <div class="text-[12px] font-mono font-medium text-foreground">
                Snapshot <span class="text-primary">{currentIndex}</span><span
                    class="text-muted-foreground"
                >
                    / {Math.max(0, history.length - 1)}</span
                >
                {#if replayTime !== null}
                    {@const hrs = replayTime * 24}
                    {@const yrs = replayTime / 365.25}
                    <span class="text-muted-foreground ml-1">
                        ({replayTime < 1
                            ? `${hrs.toFixed(1)} hrs`
                            : replayTime > 365
                              ? `${yrs.toFixed(1)} yrs`
                              : `${replayTime.toFixed(1)} days`})
                    </span>
                {/if}
            </div>
        </div>
    </div>
    <div
        class="flex flex-wrap items-center gap-4 mt-2 mb-2 w-full justify-between"
    >
        <ToggleGroup options={showPropertyOptions} bind:value={showProperty} />

        <div class="flex items-center gap-3">
            <div
                class={`legend ${isTernaryBlend(showProperty) ? "legend--ternary" : ""}`}
                style="margin:0;"
            >
                <canvas
                    bind:this={legendCanvas}
                    class:legend-canvas--ternary={isTernaryBlend(showProperty)}
                    width={isTernaryBlend(showProperty) ? 140 : 200}
                    height={isTernaryBlend(showProperty) ? 78 : 18}
                    style={isTernaryBlend(showProperty)
                        ? "width:116px;height:64px"
                        : "width:140px;height:12px"}
                ></canvas>
            </div>

            {#if !isTernaryBlend(showProperty)}
                <div class="flex items-center gap-1.5">
                    <span
                        class="label-text ui-panel-kicker"
                        >Min</span
                    >
                    <div
                        class="flex items-center gap-1 rounded-md border border-border bg-card p-0.5 transition-colors"
                    >
                        <input
                            type="number"
                            step="any"
                            class="flex h-6 w-14 rounded-md border-0 bg-transparent px-1.5 py-1 text-[11px] font-mono shadow-sm transition-colors focus:ring-1 focus:ring-ring"
                            value={legendFixedMin}
                            oninput={onLegendMinInput}
                        />
                        <button
                            type="button"
                            class="ui-chip inline-flex h-6 w-8 items-center justify-center whitespace-nowrap rounded-sm bg-muted px-0 transition-colors hover:bg-accent hover:text-accent-foreground"
                            onclick={applyModelLegendMin}>Auto</button
                        >
                    </div>
                </div>

                <div class="flex items-center gap-1.5">
                    <span
                        class="label-text ui-panel-kicker"
                        >Max</span
                    >
                    <div
                        class="flex items-center gap-1 rounded-md border border-border bg-card p-0.5 transition-colors"
                    >
                        <input
                            type="number"
                            step="any"
                            class="flex h-6 w-14 rounded-md border-0 bg-transparent px-1.5 py-1 text-[11px] font-mono shadow-sm transition-colors focus:ring-1 focus:ring-ring"
                            value={legendFixedMax}
                            oninput={onLegendMaxInput}
                        />
                        <button
                            type="button"
                            class="ui-chip inline-flex h-6 w-8 items-center justify-center whitespace-nowrap rounded-sm bg-muted px-0 transition-colors hover:bg-accent hover:text-accent-foreground"
                            onclick={applyModelLegendMax}>Auto</button
                        >
                    </div>
                </div>
            {/if}
        </div>
    </div>
    <div style="position:relative;">
        <div class="viz" bind:this={canvasContainer}></div>
        {#if tooltipVisible}
            <div
                style="position:absolute; left:{tooltipX}px; top:{tooltipY}px; background:rgba(0,0,0,0.85); color:#fff; padding:6px 8px; border-radius:4px; font-size:11px; pointer-events:none; white-space:nowrap; line-height:1.5; z-index:1000; border:1px solid rgba(255,255,255,0.15);"
            >
                {@html tooltipContent}
            </div>
        {/if}
    </div>
</div>

<style>
    /* Revert to non-absolute sizing so parent controls height explicitly */
    .viz {
        border: 1px solid #ddd;
        width: 100%;
        height: clamp(383px, 56vh, 660px);
        position: relative;
        background: #fff;
    }
    .legend {
        margin-top: 8px;
        margin-bottom: 8px;
        color: #222;
        display: flex;
        align-items: center;
        gap: 8px;
    }
    .legend--ternary {
        margin-top: 0;
        margin-bottom: 0;
        gap: 0;
    }
    .legend canvas {
        border: 1px solid #ccc;
        background: #fff;
    }
    .legend canvas.legend-canvas--ternary {
        border: 0;
        background: transparent;
        padding: 0;
    }
</style>
