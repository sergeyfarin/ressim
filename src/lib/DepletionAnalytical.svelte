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

        // TODO: Unit-conversion coupling placeholder.
        // This scale currently calibrates depletion timescale against simulation output.
        const tauDays = Math.max(1e-6, (poreVolume / Math.max(dietzShapeFactor, 1e-9)) * Math.max(1e-9, depletionTauScale));

        // TODO: Unit-conversion coupling placeholder.
        // This scale currently calibrates analytical rate magnitude against simulation output.
        const nominalRate = (initialOilInPlace / tauDays) * Math.max(0, depletionRateScale);

        let cumulativeOil = 0;
        analyticalProduction = timeHistory.map((t, idx) => {
            const boundedTime = Math.max(0, Number(t) || 0);
            const oilRate = Math.max(0, nominalRate * Math.exp(-boundedTime / tauDays));
            const dt = idx > 0 ? Math.max(0, boundedTime - Math.max(0, Number(timeHistory[idx - 1]) || 0)) : boundedTime;
            cumulativeOil += oilRate * dt;

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
