<script lang="ts">
    type Reservoir = { length: number; area: number; porosity: number };
    type AnalyticalPoint = { time: number; oilRate: number; waterRate: number; cumulativeOil: number };
    type AnalyticalDataPayload = { production: AnalyticalPoint[] };
    type AnalyticalMetaPayload = {
        mode: 'waterflood' | 'depletion';
        shapeFactor: number | null;
        shapeLabel: string;
    };

    let {
        enabled = false,
        timeHistory = [],
        reservoir,
        initialSaturation = 0.3,
        dietzShapeFactor = 21.2,
        depletionTauScale = 0.25,
        depletionRateScale = 1.0,
        onAnalyticalData = () => {},
        onAnalyticalMeta = () => {},
    }: {
        enabled?: boolean;
        timeHistory?: number[];
        reservoir: Reservoir;
        initialSaturation?: number;
        dietzShapeFactor?: number;
        depletionTauScale?: number;
        depletionRateScale?: number;
        onAnalyticalData?: (payload: AnalyticalDataPayload) => void;
        onAnalyticalMeta?: (payload: AnalyticalMetaPayload) => void;
    } = $props();

    let analyticalProduction: AnalyticalPoint[] = [];

    function emitEmpty() {
        analyticalProduction = [];
        onAnalyticalData({ production: analyticalProduction });
    }

    function calculateDepletionAnalyticalProduction() {
        onAnalyticalMeta({
            mode: 'depletion',
            shapeFactor: dietzShapeFactor,
            shapeLabel: 'user-defined',
        });

        if (!reservoir || timeHistory.length === 0) {
            emitEmpty();
            return;
        }

        const poreVolume = Math.max(1e-9, reservoir.length * reservoir.area * reservoir.porosity);
        const initialOilInPlace = poreVolume * Math.max(0, 1 - initialSaturation);

        // Material-balance + semi-steady-state decline:
        // q = J * (p_avg - p_wf),  dN/dt = -q,  p_avg ∝ N  => q(t) = N(t) / tau.
        // We represent J with Dietz shape factor and user scale, and retain depletionTauScale
        // as the storage/compressibility lumped coefficient.
        const productivityScale = Math.max(1e-9, Math.max(dietzShapeFactor, 1e-9) * Math.max(1e-9, depletionRateScale));
        const tauDays = Math.max(1e-6, (poreVolume * Math.max(1e-9, depletionTauScale)) / productivityScale);

        analyticalProduction = timeHistory.map((t) => {
            const boundedTime = Math.max(0, Number(t) || 0);
            const boundedExponent = Math.min(700, boundedTime / tauDays);
            const cumulativeOil = initialOilInPlace * (1 - Math.exp(-boundedExponent));
            const remainingOil = Math.max(0, initialOilInPlace - cumulativeOil);
            const oilRate = remainingOil / tauDays;

            return {
                time: boundedTime,
                oilRate,
                waterRate: 0,
                cumulativeOil,
            };
        });

        onAnalyticalData({ production: analyticalProduction });
    }

    $effect(() => {
        if (!enabled) {
        onAnalyticalMeta({
            mode: 'waterflood',
            shapeFactor: null,
            shapeLabel: '',
        });
        emitEmpty();
        }
    });

    $effect(() => {
        if (enabled && timeHistory.length > 0 && reservoir) {
            calculateDepletionAnalyticalProduction();
        }
    });
</script>
