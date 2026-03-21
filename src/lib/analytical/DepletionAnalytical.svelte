<script lang="ts">
    import {
        calculateDepletionAnalyticalProduction,
        emptyDepletionAnalyticalResult,
        type DepletionAnalyticalMeta,
        type DepletionAnalyticalPoint,
        type ReservoirGeometry,
    } from "./depletionAnalytical";

    type AnalyticalDataPayload = { production: AnalyticalPoint[] };
    type AnalyticalPoint = DepletionAnalyticalPoint;
    type AnalyticalMetaPayload = DepletionAnalyticalMeta | {
        mode: "waterflood";
        shapeFactor: number | null;
        shapeLabel: string;
        q0?: number;
        tau?: number;
    };

    let {
        enabled = false,
        timeHistory = [],
        reservoir,
        initialSaturation = 0.3,
        // Well & reservoir physics (matched to simulator Peaceman model)
        nz = 1, // Number of layers
        permMode = "uniform", // 'uniform' | 'perLayer' | 'random'
        uniformPermX = 100.0, // Horizontal permeability X [mD]
        uniformPermY = 100.0, // Horizontal permeability Y [mD]
        layerPermsX = [] as number[],
        layerPermsY = [] as number[],
        cellDx = 10.0, // Cell size in X [m]
        cellDy = 10.0, // Cell size in Y [m]
        cellDz = 10.0, // Cell size in Z [m]
        wellRadius = 0.1, // Wellbore radius [m]
        wellSkin = 0.0, // Skin factor [-]
        muO = 1.0, // Oil viscosity [cP]
        sWc = 0.1, // Connate water saturation [-]
        sOr = 0.1, // Residual oil saturation [-]
        nO = 2.0, // Corey exponent for oil [-]
        c_o = 1e-5, // Oil compressibility [1/bar]
        c_w = 3e-6, // Water compressibility [1/bar]
        cRock = 1e-6, // Rock compressibility [1/bar]
        initialPressure = 300.0, // Initial reservoir pressure [bar]
        producerBhp = 100.0, // Producer bottom-hole pressure [bar]
        depletionRateScale = 1.0, // Multiplier on initial rate for manual calibration
        arpsB = 0.0, // Arps decline exponent: 0=exponential, 0<b<1=hyperbolic, 1=harmonic
        onAnalyticalData = () => {},
        onAnalyticalMeta = () => {},
    }: {
        enabled?: boolean;
        timeHistory?: number[];
        reservoir: ReservoirGeometry;
        initialSaturation?: number;
        nz?: number;
        permMode?: string;
        uniformPermX?: number;
        uniformPermY?: number;
        layerPermsX?: number[];
        layerPermsY?: number[];
        cellDx?: number;
        cellDy?: number;
        cellDz?: number;
        wellRadius?: number;
        wellSkin?: number;
        muO?: number;
        sWc?: number;
        sOr?: number;
        nO?: number;
        c_o?: number;
        c_w?: number;
        cRock?: number;
        initialPressure?: number;
        producerBhp?: number;
        depletionRateScale?: number;
        arpsB?: number;
        onAnalyticalData?: (payload: AnalyticalDataPayload) => void;
        onAnalyticalMeta?: (payload: AnalyticalMetaPayload) => void;
    } = $props();

    let analyticalProduction: AnalyticalPoint[] = [];

    function emitEmpty() {
        analyticalProduction = emptyDepletionAnalyticalResult().production;
        onAnalyticalData({ production: analyticalProduction });
    }

    function refreshDepletionAnalyticalProduction() {
        const result = calculateDepletionAnalyticalProduction({
            reservoir,
            timeHistory,
            initialSaturation,
            nz,
            permMode,
            uniformPermX,
            uniformPermY,
            layerPermsX,
            layerPermsY,
            cellDx,
            cellDy,
            cellDz,
            wellRadius,
            wellSkin,
            muO,
            sWc,
            sOr,
            nO,
            c_o,
            c_w,
            cRock,
            initialPressure,
            producerBhp,
            depletionRateScale,
            arpsB,
        });

        onAnalyticalMeta(result.meta);
        analyticalProduction = result.production;
        onAnalyticalData({ production: analyticalProduction });
    }

    $effect(() => {
        if (!enabled) {
            onAnalyticalMeta(emptyDepletionAnalyticalResult().meta);
            emitEmpty();
        }
    });

    $effect(() => {
        if (enabled && timeHistory.length > 0 && reservoir) {
            refreshDepletionAnalyticalProduction();
        }
    });
</script>
