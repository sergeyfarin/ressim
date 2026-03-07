<script lang="ts">
    import { onMount, onDestroy, tick, untrack } from "svelte";
    import { Chart, registerables, type ChartDataset } from "chart.js";
    import {
        getLineDataset,
        getDatasetLabel,
        safeSetDatasetData,
        applyThemeToChart,
        externalTooltipHandler,
    } from "./chart-helpers";

    type XYPoint = { x: number; y: number | null };
    type LineDataset = ChartDataset<"line", Array<number | null | XYPoint>>;

    /** Panel configuration passed from parent */
    export interface CurveConfig {
        label: string;
        color: string;
        borderWidth?: number;
        borderDash?: number[];
        yAxisID: string;
        defaultVisible?: boolean;
        disabled?: boolean;
    }

    let {
        panelId = "",
        title = "",
        expanded = $bindable(true),
        curves = [],
        seriesData = [],
        scaleConfigs = {},
        theme = "dark",
        logScale = $bindable(false),
        allowLogToggle = false,
        chartHeight = "min(36vh, 300px)",
        targetLeftGutter = 0,
        targetRightGutter = 0,
        onGutterMeasure,
    }: {
        panelId?: string;
        title?: string;
        expanded?: boolean;
        curves?: CurveConfig[];
        seriesData?: XYPoint[][];
        scaleConfigs?: Record<string, any>;
        theme?: "dark" | "light";
        logScale?: boolean;
        allowLogToggle?: boolean;
        chartHeight?: string;
        targetLeftGutter?: number;
        targetRightGutter?: number;
        onGutterMeasure?: (left: number, right: number) => void;
    } = $props();

    let chartCanvas = $state<HTMLCanvasElement | null>(null);
    let chart = $state<Chart<"line", XYPoint[], number> | null>(null);
    let visibleCurves = $state<boolean[]>([]);
    let curveSignature = $state("");
    let chartSchemaSignature = $derived.by(() => JSON.stringify({
        curves: curves.map((curve) => ({
            label: curve.label,
            yAxisID: curve.yAxisID,
            borderWidth: curve.borderWidth ?? 2,
            borderDash: curve.borderDash ?? [],
        })),
        scales: Object.entries(cleanScaleConfigs).map(([axisId, cfg]) => ({
            axisId,
            title: cfg?.title?.text ?? "",
        })),
    }));
    let mountedChartSchemaSignature = $state("");

    // Extract custom metadata (_dynamicTitle, _fraction, _auto) from scale configs
    // so Chart.js doesn't try to resolve them as scriptable options
    type ScaleMeta = {
        _dynamicTitle?: (labels: string[]) => string;
        _fraction?: boolean;
        _auto?: boolean;
    };
    let scaleDerived = $derived.by(() => {
        let meta: Record<string, ScaleMeta> = {};
        let clean: Record<string, any> = {};
        for (const [key, cfg] of Object.entries(scaleConfigs)) {
            const { _dynamicTitle, _fraction, _auto, ...rest } = cfg;
            meta[key] = { _dynamicTitle, _fraction, _auto };
            clean[key] = structuredClone(rest);
        }
        return { meta, clean };
    });
    let scaleMeta = $derived(scaleDerived.meta);
    let cleanScaleConfigs = $derived(scaleDerived.clean);

    // Initialize visibility from curve defaults
    $effect(() => {
        const nextSignature = JSON.stringify(
            curves.map((curve) => [
                curve.label,
                curve.defaultVisible !== false,
                curve.disabled ?? false,
            ]),
        );
        if (
            curves.length > 0 &&
            (visibleCurves.length !== curves.length || curveSignature !== nextSignature)
        ) {
            curveSignature = nextSignature;
            visibleCurves = curves.map((c) => c.defaultVisible !== false);
        }
    });

    $effect(() => {
        // Explicitly track dependencies that should trigger a chart update
        const _data = seriesData;
        const _visible = visibleCurves;
        const _log = logScale;
        const _scale = scaleConfigs;
        if (chart && seriesData) {
            untrack(() => updateChart());
        }
    });

    $effect(() => {
        const _theme = theme;
        if (chart && theme) {
            untrack(() => applyThemeToChart(chart, theme));
        }
    });

    // Recreate the Chart.js instance when the dataset/axis schema changes.
    $effect(() => {
        const schemaSignature = chartSchemaSignature;
        if (expanded) {
            tick().then(() => {
                if (!expanded || !chartCanvas) return;
                if (!chart || mountedChartSchemaSignature !== schemaSignature) {
                    destroyChart();
                    createChart();
                    mountedChartSchemaSignature = schemaSignature;
                }
            });
        }
    });

    function toggleCurve(idx: number) {
        visibleCurves = visibleCurves.map((v, i) => (i === idx ? !v : v));
        updateChart();
    }

    // Smart decimal formatting for tooltips (3 significant digits)
    function formatValueTooltip(value: number): string {
        if (!Number.isFinite(value)) return "";
        const abs = Math.abs(value);
        if (abs >= 100) return value.toFixed(0);
        if (abs >= 10) return value.toFixed(1);
        if (abs >= 1) return value.toFixed(2);
        return value.toFixed(3);
    }

    function updateChart() {
        if (!chart || !seriesData) return;

        const datasetSchemaMismatch =
            chart.data.datasets.length !== curves.length ||
            curves.some((curve, idx) => {
                const dataset = getLineDataset(chart, idx);
                return (
                    !dataset
                    || getDatasetLabel(chart, idx) !== curve.label
                    || dataset.yAxisID !== curve.yAxisID
                );
            });
        if (datasetSchemaMismatch) {
            destroyChart();
            createChart();
            return;
        }

        for (let idx = 0; idx < curves.length; idx++) {
            // Unwrap array and inner points so Chart.js can safely append internal tracking without triggering Proxies
            let rawData = seriesData[idx] ?? [];
            if (Array.isArray(rawData)) {
                rawData = rawData.map((pt) =>
                    pt && typeof pt === "object" ? { ...pt } : pt,
                ) as any;
            }
            safeSetDatasetData(chart, idx, rawData);

            const lineDataset = getLineDataset(chart, idx);
            if (lineDataset) {
                lineDataset.pointRadius = 0;
                lineDataset.pointHoverRadius = 0;
                lineDataset.pointHitRadius = 8;
            }

            const visible = visibleCurves[idx] ?? true;
            chart.setDatasetVisibility(idx, visible);
        }

        // Update axis visibility based on visible curves
        const activeAxisIds = new Set(
            visibleCurves
                .map((visible, idx) => (visible ? curves[idx]?.yAxisID : null))
                .filter((id): id is string => Boolean(id)),
        );

        const scales = (chart.options.scales ?? {}) as Record<string, any>;
        for (const [axisId, config] of Object.entries(scales)) {
            if (axisId === "x") continue;
            if (config) config.display = activeAxisIds.has(axisId);
        }

        // Handle log scale for rate axis
        if (scales.y) {
            scales.y.type = logScale ? "logarithmic" : "linear";
            if (logScale) {
                const yValues = collectAxisValues("y").filter((v) => v > 0);
                if (yValues.length > 0) {
                    const minY = Math.min(...yValues);
                    const maxY = Math.max(...yValues);
                    scales.y.min = Math.pow(
                        10,
                        Math.floor(Math.log10(Math.max(1e-6, minY))),
                    );
                    scales.y.max = Math.pow(
                        10,
                        Math.ceil(Math.log10(Math.max(1e-6, maxY * 1.1))),
                    );
                } else {
                    delete scales.y.min;
                }
            } else {
                scales.y.min = 0;
                applyPositiveAxisBounds(scales.y, collectAxisValues("y"));
            }
        }

        // Auto-bound other axes
        for (const [axisId, config] of Object.entries(scales)) {
            if (axisId === "x" || axisId === "y") continue;
            if (!config || !config.display) continue;
            if ((scaleMeta[axisId] as any)?._fraction) {
                // Remove hard cap of 1 so WOR can exceed 1, but default to 0-1 bounds
                applyPositiveAxisBounds(config, collectAxisValues(axisId));
            } else if ((scaleMeta[axisId] as any)?._auto) {
                applyAutoAxisBounds(config, collectAxisValues(axisId));
            } else {
                applyPositiveAxisBounds(config, collectAxisValues(axisId));
            }
        }

        // Dynamic axis title based on which curves are visible
        for (const [axisId, config] of Object.entries(scales)) {
            if (!config || axisId === "x") continue;
            const meta = scaleMeta[axisId];
            if (meta && typeof meta._dynamicTitle === "function") {
                const visibleLabels = curves
                    .filter(
                        (c, idx) => visibleCurves[idx] && c.yAxisID === axisId,
                    )
                    .map((c) => c.label);
                const newTitle = meta._dynamicTitle(visibleLabels);
                if (config.title && newTitle) config.title.text = newTitle;
            }
        }

        chart.update();

        // After update, measure the native gutters (excluding any manual padding we added)
        if (onGutterMeasure && chart.chartArea) {
            const currentPaddingLeft =
                (chart.options.layout?.padding as any)?.left ?? 0;
            const currentPaddingRight =
                (chart.options.layout?.padding as any)?.right ?? 0;
            const nativeLeft = chart.chartArea.left - currentPaddingLeft;
            const nativeRight =
                chart.width - chart.chartArea.right - currentPaddingRight;
            onGutterMeasure(nativeLeft, nativeRight);
        }
    }

    // Reactively apply padding to match the target gutters from parent
    $effect(() => {
        const _left = targetLeftGutter;
        const _right = targetRightGutter;
        if (
            chart &&
            chartCanvas &&
            (targetLeftGutter > 0 || targetRightGutter > 0)
        ) {
            untrack(() => applyTargetGutters());
        }
    });

    function applyTargetGutters() {
        if (!chart || !chart.chartArea) return;
        const currentPaddingLeft =
            (chart.options.layout?.padding as any)?.left ?? 0;
        const currentPaddingRight =
            (chart.options.layout?.padding as any)?.right ?? 0;

        const myNativeLeft = chart.chartArea.left - currentPaddingLeft;
        const myNativeRight =
            chart.width - chart.chartArea.right - currentPaddingRight;

        const padLeft = Math.max(0, targetLeftGutter - myNativeLeft);
        const padRight = Math.max(0, targetRightGutter - myNativeRight);

        if (
            Math.abs(padLeft - currentPaddingLeft) > 1 ||
            Math.abs(padRight - currentPaddingRight) > 1
        ) {
            chart.options.layout = {
                padding: { left: padLeft, right: padRight },
            };
            chart.update("none"); // Update without animation to prevent UI jitter
        }
    }

    function collectAxisValues(axisId: string): number[] {
        const values: number[] = [];
        for (let idx = 0; idx < curves.length; idx++) {
            if (!visibleCurves[idx] || curves[idx]?.yAxisID !== axisId)
                continue;
            const data = seriesData[idx] ?? [];
            for (const point of data) {
                if (
                    point &&
                    typeof point === "object" &&
                    Number.isFinite(point.y)
                ) {
                    values.push(point.y as number);
                }
            }
        }
        return values;
    }

    function applyPositiveAxisBounds(axis: any, values: number[]) {
        if (!axis || !values.length) return;
        axis.min = 0;
        // Let Chart.js automatically calculate the most aesthetic max bound natively!
    }

    function applyAutoAxisBounds(axis: any, values: number[]) {
        if (!axis || !values.length) return;
        const minValue = Math.min(...values);
        const maxValue = Math.max(...values);
        // Explicitly pad completely flat lines to prevent Chart.js scaling glitches
        if (Math.abs(maxValue - minValue) < 1e-9) {
            const pad = Math.max(Math.abs(maxValue) * 0.05, 1);
            axis.min = minValue - pad;
            axis.max = maxValue + pad;
            return;
        }
        // Otherwise, allow Chart.js to natively auto-pad both max and min ceilings!
    }

    function getCurveColor(idx: number): string {
        return curves[idx]?.color ?? "#888";
    }

    function getCurveDash(idx: number): string {
        const dash = curves[idx]?.borderDash;
        return Array.isArray(dash) ? dash.join(", ") : "";
    }

    function getCurveBorderWidth(idx: number): number {
        return curves[idx]?.borderWidth ?? 2;
    }

    function createChart() {
        if (!chartCanvas || chart) return;
        Chart.register(...registerables);
        const ctx = chartCanvas.getContext("2d");
        if (!ctx) return;

        const datasets = curves.map((curve) => ({
            label: curve.label,
            data: [] as XYPoint[],
            borderColor: curve.color,
            borderWidth: curve.borderWidth ?? 2,
            borderDash: curve.borderDash ? [...curve.borderDash] : undefined,
            yAxisID: curve.yAxisID,
            pointRadius: 0,
            pointHoverRadius: 0,
            pointHitRadius: 8,
        }));

        chart = new Chart(ctx, {
            type: "line",
            data: { labels: [], datasets },
            options: {
                animation: false,
                responsive: true,
                maintainAspectRatio: false,
                plugins: {
                    legend: { display: false },
                    tooltip: {
                        enabled: false,
                        external: externalTooltipHandler,
                        callbacks: {
                            label: (context) => {
                                const label = context.dataset.label ?? "";
                                const rawValue = context.parsed?.y;
                                if (!Number.isFinite(rawValue)) return label;
                                return `${label}: ${formatValueTooltip(Number(rawValue))}`;
                            },
                        },
                    },
                },
                scales: {
                    x: {
                        type: "linear",
                        display: true,
                        title: {
                            font: {
                                family: "'JetBrains Mono', monospace",
                                size: 11,
                            },
                        },
                        ticks: {
                            font: {
                                family: "'JetBrains Mono', monospace",
                                size: 10,
                            },
                        },
                    },
                    ...Object.fromEntries(
                        Object.entries(cleanScaleConfigs).map(([key, cfg]) => [
                            key,
                            {
                                ...cfg,
                                title: {
                                    ...(cfg.title ?? {}),
                                    font: {
                                        family: "'JetBrains Mono', monospace",
                                        size: 11,
                                    },
                                },
                                ticks: {
                                    ...(cfg.ticks ?? {}),
                                    font: {
                                        family: "'JetBrains Mono', monospace",
                                        size: 10,
                                    },
                                },
                            },
                        ]),
                    ),
                },
            },
        });

        applyThemeToChart(chart, theme);
        updateChart();
    }

    function destroyChart() {
        chart?.destroy();
        chart = null;
    }

    onMount(() => {
        if (expanded) createChart();
    });

    onDestroy(() => {
        destroyChart();
    });
</script>

<!-- FIX #6: Load JetBrains Mono font for digits -->
<svelte:head>
    <link rel="preconnect" href="https://fonts.googleapis.com" />
    <link
        rel="preconnect"
        href="https://fonts.gstatic.com"
        crossorigin="anonymous"
    />
    <link
        href="https://fonts.googleapis.com/css2?family=JetBrains+Mono:wght@400;500&display=swap"
        rel="stylesheet"
    />
</svelte:head>

<div
    class="border-t border-border overflow-hidden {expanded
        ? 'bg-card'
        : 'bg-muted/50'} first:border-t-0"
    id="panel-{panelId}"
>
    <!-- Collapsible header -->
    <button
        type="button"
        class="w-full flex items-center justify-start px-4 py-2 bg-muted/40 text-xs font-semibold
            hover:bg-muted/60 transition-colors cursor-pointer select-none"
        onclick={() => {
            if (expanded) {
                expanded = false;
                destroyChart();
            } else {
                expanded = true;
            }
        }}
    >
        {title}
        <svg
            class="w-3.5 h-3.5 transition-transform ml-2 {expanded
                ? 'rotate-90'
                : ''}"
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
        >
            <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M9 5l7 7-7 7"
            ></path>
        </svg>
    </button>

    {#if expanded}
        <div class="pb-2 flex flex-col">
            {#if allowLogToggle || curves.length > 0}
                <div
                    class="flex flex-wrap items-center gap-1.5 px-4 md:px-5 py-2"
                >
                    {#if allowLogToggle}
                        <!-- Linear/Log radio-style toggle pair -->
                        <div
                            class="inline-flex rounded-md border border-border shadow-sm overflow-hidden shrink-0 mr-2"
                        >
                            <button
                                type="button"
                                class="px-2.5 py-0.5 text-[11px] font-medium transition-colors
                                {!logScale
                                    ? 'bg-primary text-primary-foreground'
                                    : 'bg-transparent text-muted-foreground hover:bg-muted/50 hover:text-foreground'}"
                                onclick={(e) => {
                                    e.stopPropagation();
                                    logScale = false;
                                }}
                                title="Linear Scale"
                            >
                                Lin
                            </button>
                            <button
                                type="button"
                                class="px-2.5 py-0.5 text-[11px] font-medium transition-colors border-l border-border
                                {logScale
                                    ? 'bg-primary text-primary-foreground'
                                    : 'bg-transparent text-muted-foreground hover:bg-muted/50 hover:text-foreground'}"
                                onclick={() => {
                                    logScale = true;
                                }}
                            >
                                Log
                            </button>
                        </div>
                    {/if}

                    <!-- Curve toggles with visible border and ✕/+ indicators -->
                    {#each curves as curve, idx}
                        <button
                            type="button"
                            disabled={curve.disabled}
                            class="flex items-center gap-1.5 px-2 py-0.5 rounded text-[11px] font-medium transition-all
                        {visibleCurves[idx]
                                ? 'bg-muted border-2 border-primary/30 opacity-100 shadow-sm'
                                : 'bg-transparent border border-dashed border-border opacity-50'}
                        {!curve.disabled
                                ? 'cursor-pointer hover:opacity-75'
                                : 'disabled:opacity-30 disabled:cursor-not-allowed disabled:grayscale'}"
                            onclick={() => toggleCurve(idx)}
                            title={curve.disabled
                                ? "Curve disabled for this case"
                                : visibleCurves[idx]
                                  ? `Hide ${curve.label}`
                                  : `Show ${curve.label}`}
                        >
                            <svg
                                width="14"
                                height="3"
                                class="overflow-visible shrink-0"
                                viewBox="0 0 14 3"
                            >
                                <line
                                    x1="0"
                                    y1="1.5"
                                    x2="14"
                                    y2="1.5"
                                    stroke={getCurveColor(idx)}
                                    stroke-width={getCurveBorderWidth(idx)}
                                    stroke-dasharray={getCurveDash(idx)}
                                />
                            </svg>
                            <span>{curve.label}</span>
                            <span
                                class="opacity-60 ml-0.5 {visibleCurves[idx]
                                    ? 'text-[9px]'
                                    : 'text-[14px]'}"
                                >{visibleCurves[idx] ? "✕" : "+"}</span
                            >
                        </button>
                    {/each}
                </div>
            {/if}

            <!-- FIX #2: Chart canvas with fixed left padding for y-axis alignment -->
            <div
                style="position: relative; height: {chartHeight}; width: 100%;"
                class="pb-2"
            >
                <canvas bind:this={chartCanvas}></canvas>
            </div>
        </div>
    {/if}
</div>
