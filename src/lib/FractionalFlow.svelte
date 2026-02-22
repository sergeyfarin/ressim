<script lang="ts">
    import { onMount, onDestroy } from 'svelte';
    import { Chart, registerables } from 'chart.js';
    import { safeSetDatasetData } from './chart-helpers';

    type RockProps = { s_wc: number; s_or: number; n_w: number; n_o: number };
    type FluidProps = { mu_w: number; mu_o: number };
    type Reservoir = { length: number; area: number; porosity: number };
    type AnalyticalPoint = { time: number; oilRate: number; waterRate: number; cumulativeOil: number };
    type AnalyticalDataPayload = { production: AnalyticalPoint[] };
    type AnalyticalMetaPayload = {
        mode: 'waterflood' | 'depletion';
        shapeFactor: number | null;
        shapeLabel: string;
    };

    let {
        rockProps,
        fluidProps,
        timeHistory = [],
        injectionRateSeries = [],
        reservoir,
        initialSaturation = 0.3,
        scenarioMode = 'waterflood',
        onAnalyticalData = () => {},
        onAnalyticalMeta = () => {},
    }: {
        rockProps: RockProps;
        fluidProps: FluidProps;
        timeHistory?: number[];
        injectionRateSeries?: number[];
        reservoir: Reservoir;
        initialSaturation?: number;
        scenarioMode?: 'waterflood' | 'depletion';
        onAnalyticalData?: (payload: AnalyticalDataPayload) => void;
        onAnalyticalMeta?: (payload: AnalyticalMetaPayload) => void;
    } = $props();

    type WelgeMetrics = {
        shockSw: number;
        breakthroughPvi: number;
        waterCutAtBreakthrough: number;
        initialSw: number;
    };

    let welgeCanvas: HTMLCanvasElement | undefined;
    let welgeChart: Chart<'line', Array<{ x: number; y: number }>, number> | null = null;
    let welgeMetrics = $state<WelgeMetrics>({
        shockSw: 0,
        breakthroughPvi: 0,
        waterCutAtBreakthrough: 0,
        initialSw: 0,
    });

    let analyticalProduction: AnalyticalPoint[] = [];

    $effect(() => {
        if (rockProps && fluidProps) {
            welgeMetrics = computeWelgeMetrics();
            updateWelgeChart();
        }
    });

    $effect(() => {
        if (
            scenarioMode === 'waterflood' &&
            timeHistory.length > 0 &&
            rockProps &&
            fluidProps &&
            reservoir &&
            injectionRateSeries.length > 0
        ) {
            calculateAnalyticalProduction();
            onAnalyticalData({ production: analyticalProduction });
        }
    });

    onMount(() => {
        Chart.register(...registerables);
        const ctx = welgeCanvas?.getContext('2d');
        if (!ctx) return;

        welgeChart = new Chart(ctx, {
            type: 'line',
            data: {
                labels: [],
                datasets: [
                    {
                        label: 'Fractional Flow f_w(Sw)',
                        data: [],
                        borderColor: '#2563eb',
                        borderWidth: 2.2,
                        pointRadius: 0,
                        parsing: false,
                    },
                    {
                        label: 'Welge Tangent from Swi',
                        data: [],
                        borderColor: '#16a34a',
                        borderWidth: 2,
                        borderDash: [6, 4],
                        pointRadius: 0,
                        parsing: false,
                    },
                ],
            },
            options: {
                responsive: true,
                maintainAspectRatio: false,
                plugins: {
                    legend: {
                        display: true,
                        labels: { boxWidth: 14, usePointStyle: false },
                    },
                },
                scales: {
                    x: {
                        type: 'linear',
                        title: {
                            display: true,
                            text: 'Water Saturation Sw',
                        },
                        min: 0,
                        max: 1,
                    },
                    y: {
                        type: 'linear',
                        title: {
                            display: true,
                            text: 'Fractional Flow f_w',
                        },
                        min: 0,
                        max: 1,
                    },
                },
            },
        });

        updateWelgeChart();
    });

    onDestroy(() => {
        welgeChart?.destroy();
        welgeChart = null;
    });



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

    function computeWelgeMetrics(): WelgeMetrics {
        const { s_wc, s_or } = rockProps;
        const sMin = s_wc;
        const sMax = 1 - s_or;
        const initialSwClamped = Math.max(sMin, Math.min(sMax, initialSaturation));

        const fwInitial = fractionalFlow(initialSwClamped);
        let swShock = initialSwClamped;
        let maxSlope = 0;
        for (let s = initialSwClamped + 5e-4; s <= sMax; s += 5e-4) {
            const fw = fractionalFlow(s);
            const slope = (fw - fwInitial) / Math.max(1e-12, s - initialSwClamped);
            if (slope > maxSlope && Number.isFinite(slope)) {
                maxSlope = slope;
                swShock = s;
            }
        }

        const fwShock = fractionalFlow(swShock);
        const dfwAtShock = (fwShock - fwInitial) / Math.max(1e-12, swShock - initialSwClamped);
        const breakthroughPvi = dfwAtShock > 1e-12 ? 1.0 / dfwAtShock : 0;

        return {
            shockSw: swShock,
            breakthroughPvi,
            waterCutAtBreakthrough: fwShock,
            initialSw: initialSwClamped,
        };
    }

    function updateWelgeChart() {
        if (!welgeChart || !rockProps) return;

        const sMin = rockProps.s_wc;
        const sMax = 1 - rockProps.s_or;
        const fwCurve = [];
        const tangentCurve = [];
        const fwInitial = fractionalFlow(welgeMetrics.initialSw);
        const slope = (fractionalFlow(welgeMetrics.shockSw) - fwInitial)
            / Math.max(1e-12, welgeMetrics.shockSw - welgeMetrics.initialSw);

        for (let s = sMin; s <= sMax; s += 0.005) {
            fwCurve.push({ x: s, y: fractionalFlow(s) });
            tangentCurve.push({ x: s, y: Math.max(0, Math.min(1, fwInitial + slope * (s - welgeMetrics.initialSw))) });
        }

        safeSetDatasetData(welgeChart, 0, fwCurve);
        safeSetDatasetData(welgeChart, 1, tangentCurve);
        welgeChart.update();
    }

    function calculateAnalyticalProduction() {
        onAnalyticalMeta({
            mode: scenarioMode,
            shapeFactor: null,
            shapeLabel: '',
        });

        const { s_wc, s_or } = rockProps;
        const initial_sw = Math.max(s_wc, Math.min(1 - s_or, initialSaturation));

        // Find shock front saturation (Sw_f) using Welge tangent method
        // Tangent is drawn from (Sw_init, fw(Sw_init)) to the fw curve
        const fw_initial = fractionalFlow(initial_sw);
        let sw_f = initial_sw;
        let max_slope = 0;
        for (let s = initial_sw + 5e-4; s <= 1 - s_or; s += 5e-4) {
            const fw = fractionalFlow(s);
            const slope = (fw - fw_initial) / Math.max(1e-12, s - initial_sw);
            if (slope > max_slope) {
                max_slope = slope;
                sw_f = s;
            }
        }

        const fw_at_shock = fractionalFlow(sw_f);
        const dfw_at_shock = (fw_at_shock - fw_initial) / Math.max(1e-12, sw_f - initial_sw);

        const poreVolume = reservoir.length * reservoir.area * reservoir.porosity;
        const q0 = injectionRateSeries.find((rate) => Number.isFinite(rate) && rate > 0) ?? 0;
        if (q0 <= 0) {
            analyticalProduction = timeHistory.map((t) => ({
                time: t,
                oilRate: 0,
                waterRate: 0,
                cumulativeOil: 0,
            }));
            return;
        }

        // Breakthrough PVI from Welge tangent: PVI_bt = 1 / dfw_at_shock
        const breakthroughPVI = dfw_at_shock > 1e-12 ? 1.0 / dfw_at_shock : Number.POSITIVE_INFINITY;

        // Helper: find outlet Sw after breakthrough using bisection on dfw/dSw
        // In the rarefaction zone [sw_f, 1-s_or], dfw/dSw is monotonically decreasing
        // so we can use bisection to find Sw where dfw/dSw = target_dfw
        function findOutletSw(target_dfw: number): number {
            let lo = sw_f;
            let hi = 1 - s_or;

            // Check if target is within the range of dfw/dSw
            const dfw_lo = dfw_dSw(lo, 1e-4);
            const dfw_hi = dfw_dSw(hi, 1e-4);

            if (target_dfw >= dfw_lo) return lo;   // before shock front
            if (target_dfw <= dfw_hi) return hi;   // fully swept

            // Bisection: dfw/dSw is decreasing, so we look for where dfw/dSw crosses target_dfw
            for (let iter = 0; iter < 50; iter++) {
                const mid = 0.5 * (lo + hi);
                const dfw_mid = dfw_dSw(mid, 1e-4);
                if (dfw_mid > target_dfw) {
                    lo = mid;  // need higher Sw (lower dfw/dSw)
                } else {
                    hi = mid;  // need lower Sw (higher dfw/dSw)
                }
                if (hi - lo < 1e-6) break;
            }
            return 0.5 * (lo + hi);
        }

        const newProduction: AnalyticalPoint[] = [];
        let cumulativeOil = 0;
        let cumulativePVI = 0;  // track cumulative pore volumes injected

        for (let i = 0; i < timeHistory.length; i++) {
            const t = timeHistory[i];
            const q = Number.isFinite(injectionRateSeries[i]) && injectionRateSeries[i] > 0
                ? injectionRateSeries[i]
                : q0;

            // Accumulate PVI using the actual injection rate (I5 fix)
            const dt = i > 0 ? Math.max(0, t - timeHistory[i - 1]) : Math.max(0, t);
            if (poreVolume > 0) {
                cumulativePVI += (q * dt) / poreVolume;
            }

            let oilRate = 0;
            if (cumulativePVI <= breakthroughPVI) {
                // Before breakthrough, outlet is at initial saturation
                oilRate = q * (1 - fw_initial);
            } else {
                // After breakthrough, find Sw at outlet via method of characteristics
                // x = v_t * dfw/dSw * t  →  at outlet x=L: dfw/dSw = L / (v_t * t)
                // Using PVI: dfw/dSw = 1/PVI (dimensionless form)
                const target_dfw = cumulativePVI > 1e-12 ? 1.0 / cumulativePVI : dfw_at_shock;
                const s_w_at_outlet = findOutletSw(target_dfw);

                const fw_at_outlet = fractionalFlow(s_w_at_outlet);
                oilRate = q * (1 - fw_at_outlet);
            }
            const boundedOilRate = Math.max(0, oilRate);
            const waterRate = Math.max(0, q - boundedOilRate);
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

<!-- <div class="card border border-base-300 bg-base-100 shadow-sm">
    <div class="card-body p-3 sm:p-4">
        <h3 class="text-sm font-semibold">Welge f(Sw) Diagram</h3>
        <div class="grid grid-cols-1 gap-3 lg:grid-cols-3">
            <div class="lg:col-span-2" style="height: 180px;">
                <canvas bind:this={welgeCanvas}></canvas>
            </div>
            <div class="space-y-1 text-xs">
                <div><span class="font-semibold">Shock Sw:</span> {welgeMetrics.shockSw.toFixed(4)}</div>
                <div><span class="font-semibold">Breakthrough PVI:</span> {welgeMetrics.breakthroughPvi.toFixed(4)}</div>
                <div><span class="font-semibold">Water Cut @ Breakthrough:</span> {welgeMetrics.waterCutAtBreakthrough.toFixed(4)}</div>
            </div>
        </div>
    </div>
</div> -->
