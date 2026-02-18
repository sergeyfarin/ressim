<script lang="ts">
    import { createEventDispatcher } from 'svelte';

    export let enabled = false;
    export let timeHistory: number[] = [];
    export let reservoir: { length: number; area: number; porosity: number };
    export let initialSaturation = 0.3;
    export let dietzShapeFactor = 21.2;
    export let depletionTauScale = 0.25;
    export let depletionRateScale = 1.0;

    const dispatch = createEventDispatcher();

    let analyticalProduction: { time: number; oilRate: number; waterRate: number; cumulativeOil: number }[] = [];

    function emitEmpty() {
        analyticalProduction = [];
        dispatch('analyticalData', { production: analyticalProduction });
    }

    function calculateDepletionAnalyticalProduction() {
        dispatch('analyticalMeta', {
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

        dispatch('analyticalData', { production: analyticalProduction });
    }

    $: if (!enabled) {
        dispatch('analyticalMeta', {
            mode: 'waterflood',
            shapeFactor: null,
            shapeLabel: '',
        });
        emitEmpty();
    }

    $: if (enabled && timeHistory.length > 0 && reservoir) {
        calculateDepletionAnalyticalProduction();
    }
</script>
