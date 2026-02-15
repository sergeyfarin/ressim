<script lang="ts">
    import { createEventDispatcher } from 'svelte';

    export let rockProps: { s_wc: number; s_or: number; n_w: number; n_o: number };
    export let fluidProps: { mu_w: number; mu_o: number };
    export let timeHistory: number[] = [];
    export let injectionRate: number; // Total injection rate m³/day
    export let reservoir: { length: number; area: number; porosity: number };
    export let initialSaturation = 0.3;

    const dispatch = createEventDispatcher();

    let analyticalProduction: { time: number; oilRate: number; waterRate: number; cumulativeOil: number }[] = [];

    $: if (timeHistory.length > 0 && rockProps && fluidProps && reservoir && injectionRate > 0) {
        calculateAnalyticalProduction();
        dispatch('analyticalData', { production: analyticalProduction });
    }

    function k_rw(s_w: number) {
        const { s_wc, s_or, n_w } = rockProps;
        const s_eff = Math.max(0, Math.min(1, (s_w - s_wc) / (1 - s_wc - s_or)));
        return Math.pow(s_eff, n_w);
    }

    function k_ro(s_w: number) {
        const { s_wc, s_or, n_o } = rockProps;
        const s_eff = Math.max(0, Math.min(1, (1 - s_w - s_or) / (1 - s_wc - s_or)));
        return Math.pow(s_eff, n_o);
    }

    function fractionalFlow(s_w: number) {
        const { mu_w, mu_o } = fluidProps;
        const krw = k_rw(s_w);
        const kro = k_ro(s_w);
        const numerator = krw / mu_w;
        const denominator = numerator + (kro / mu_o);
        if (denominator === 0) return 0;
        return numerator / denominator;
    }

    function dfw_dSw(s_w: number, ds: number = 1e-6) {
        const sMin = rockProps.s_wc;
        const sMax = 1 - rockProps.s_or;
        if (s_w < sMin || s_w > sMax) return 0;
        const fw_plus = fractionalFlow(Math.min(sMax, s_w + ds));
        const fw_minus = fractionalFlow(Math.max(sMin, s_w - ds));
        return (fw_plus - fw_minus) / (2 * ds);
    }

    function calculateAnalyticalProduction() {
        const { s_wc, s_or } = rockProps;
        const initial_sw = Math.max(s_wc, Math.min(1 - s_or, initialSaturation));

        // Find shock front saturation (Sw_f) using graphical tangent method
        let sw_f = initial_sw;
        let max_slope = 0;
        for (let s = initial_sw + 5e-4; s <= 1 - s_or; s += 5e-4) {
            const fw = fractionalFlow(s);
            const slope = fw / (s - initial_sw);
            if (slope > max_slope) {
                max_slope = slope;
                sw_f = s;
            }
        }

        const fw_at_shock = fractionalFlow(sw_f);
        const dfw_at_shock = fw_at_shock / (sw_f - initial_sw);

        const poreVolume = reservoir.length * reservoir.area * reservoir.porosity;
        const v_shock = (injectionRate / (reservoir.area * reservoir.porosity)) * dfw_at_shock;
        const breakthroughTime = reservoir.length / v_shock;

        const newProduction: { time: number; oilRate: number; waterRate: number; cumulativeOil: number }[] = [];
        let cumulativeOil = 0;

        for (let i = 0; i < timeHistory.length; i++) {
            const t = timeHistory[i];
            let oilRate = 0;
            if (t <= breakthroughTime) {
                // Before breakthrough, production is pure oil (at injection rate)
                oilRate = injectionRate;
            } else {
                // After breakthrough, find saturation at the outlet (x=L)
                const v_t = injectionRate / (reservoir.area * reservoir.porosity);
                
                let s_w_at_outlet = sw_f;
                // Find Sw at x=L by solving x/t = v_t * dfw/dSw for Sw
                // L/t = v_t * dfw/dSw  => dfw/dSw = (L/t) / v_t
                const target_dfw = (reservoir.length / t) / v_t;
                let bestDelta = Number.POSITIVE_INFINITY;

                // Search Sw that minimizes derivative mismatch.
                for (let s = sw_f; s <= 1 - s_or; s += 5e-4) {
                    const derivative = dfw_dSw(s, 1e-4);
                    const delta = Math.abs(derivative - target_dfw);
                    if (delta < bestDelta) {
                        bestDelta = delta;
                        s_w_at_outlet = s;
                    }
                }
                
                const fw_at_outlet = fractionalFlow(s_w_at_outlet);
                const waterCut = fw_at_outlet;
                oilRate = injectionRate * (1 - waterCut);
            }
            const boundedOilRate = Math.max(0, oilRate);
            const waterRate = Math.max(0, injectionRate - boundedOilRate);
            const dt = i > 0 ? Math.max(0, t - timeHistory[i - 1]) : Math.max(0, t);
            cumulativeOil += boundedOilRate * dt;

            newProduction.push({
                time: t,
                oilRate: boundedOilRate,
                waterRate,
                cumulativeOil,
            });
        }
        analyticalProduction = newProduction;
    }
</script>

<!-- This component has no UI, it only performs calculations -->
