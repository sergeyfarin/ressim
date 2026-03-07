<script lang="ts">
    import { onMount, onDestroy, tick } from "svelte";
    import FractionalFlow from "./lib/analytical/FractionalFlow.svelte";
    import DepletionAnalytical from "./lib/analytical/DepletionAnalytical.svelte";
    import RunControls from "./lib/ui/cards/RunControls.svelte";
    import WarningPolicyPanel from "./lib/ui/feedback/WarningPolicyPanel.svelte";
    import ModePanel from "./lib/ui/modes/ModePanel.svelte";
    import SwProfileChart from "./lib/charts/SwProfileChart.svelte";
    import { getBenchmarkRateChartLayoutConfig } from "./lib/charts/benchmarkChartConfig";
    import Button from "./lib/ui/controls/Button.svelte";
    import Card from "./lib/ui/controls/Card.svelte";
    import { getBenchmarkFamily, getPresetEntry } from "./lib/catalog/caseCatalog";
    import { createSimulationStore } from "./lib/stores/simulationStore.svelte";

    // ---------- Store ----------
    const store = createSimulationStore();
    const scenario = store.scenarioSelection;
    const params = store.parameterState;
    const runtime = store.runtimeState;

    function handleCloneBenchmarkToCustom() {
        scenario.cloneActiveBenchmarkToCustom();
    }

    // ---------- UI-only state ----------
    let theme: "dark" | "light" = $state("dark");
    let showDebugState = $state(false);
    let showProperty: "pressure" | "saturation_water" | "saturation_oil" =
        $state("pressure");
    let legendFixedMin = $state(0);
    let legendFixedMax = $state(1);

    type ThreeDViewComponentType = typeof import("./lib/visualization/3dview.svelte").default;
    type RateChartComponentType =
        typeof import("./lib/charts/RateChart.svelte").default;
    let ThreeDViewComponent = $state<ThreeDViewComponentType | null>(null);
    let RateChartComponent = $state<RateChartComponentType | null>(null);
    let loadingThreeDView = $state(false);
    const activeBenchmarkBaseResult = $derived.by(() => {
        if (scenario.activeMode !== "benchmark") return null;
        const benchmarkId = scenario.toggles.benchmarkId ?? null;
        if (!benchmarkId) return null;
        return runtime.benchmarkRunResults.find((result) => (
            result.familyKey === benchmarkId && result.variantKey === null
        )) ?? null;
    });
    const activeRateChartLayoutConfig = $derived.by(() => {
        if (scenario.activeMode === "benchmark") {
            return getBenchmarkRateChartLayoutConfig({
                family: getBenchmarkFamily(scenario.toggles.benchmarkId),
                referencePolicy: activeBenchmarkBaseResult?.referencePolicy ?? null,
            });
        }

        return getPresetEntry(scenario.activeCase)?.layoutConfig ?? {};
    });

    // ---------- Config diff $effect ----------
    $effect(() => {
        runtime.checkConfigDiff();
    });

    // ---------- Theme ----------
    function toggleTheme() {
        theme = theme === "dark" ? "light" : "dark";
    }

    $effect(() => {
        if (typeof document === "undefined") return;
        document.documentElement.setAttribute("data-theme", theme);
    });
    $effect(() => {
        if (typeof localStorage === "undefined") return;
        localStorage.setItem("ressim-theme", theme);
    });

    // ---------- Lazy module loading ----------
    async function loadRateChartModule() {
        try {
            const rateChartModule = await import("./lib/charts/RateChart.svelte");
            RateChartComponent = rateChartModule.default;
        } catch (error) {
            console.error("Failed to load rate chart module:", error);
        }
    }

    async function loadThreeDViewModule() {
        if (ThreeDViewComponent || loadingThreeDView) return;
        loadingThreeDView = true;
        try {
            const threeDModule = await import("./lib/visualization/3dview.svelte");
            ThreeDViewComponent = threeDModule.default;
        } catch (error) {
            console.error("Failed to load 3D view module:", error);
        } finally {
            loadingThreeDView = false;
        }
    }

    // ---------- Lifecycle ----------
    onMount(async () => {
        const savedTheme = localStorage.getItem("ressim-theme");
        if (savedTheme === "light" || savedTheme === "dark") theme = savedTheme;
        document.documentElement.setAttribute("data-theme", theme);
        runtime.setupWorker();

        await loadRateChartModule();
        await loadThreeDViewModule();
        await tick();

        scenario.handleModeChange("dep");
    });

    onDestroy(() => {
        runtime.dispose();
    });
</script>

<main
    class="min-h-screen text-foreground bg-background relative"
    data-theme={theme}
>
    <!-- Geological layers — styled by .geo-layers in app.css (opacity adapts via [data-theme]) -->
    <div class="geo-layers">
        <svg
            viewBox="0 0 100 100"
            preserveAspectRatio="none"
            class="w-full h-full"
        >
            <!-- ORDER MATTERS: shallowest (top of page) rendered FIRST, deepest LAST -->
            <!-- Each path fills from its wavy top edge down to Y=100. Later paths paint over earlier ones. -->
            <!-- So you see each layer's color only in the band between its top edge and the next deeper layer's top edge. -->

            {#if theme === "dark"}
                <!-- Dark theme: muted desaturated earth tones -->
                <path
                    fill="#3a3228"
                    d="M0,5 C24,11 76,-1 100,8 L100,100 L0,100 Z"
                />
                <path
                    fill="#4a3d2e"
                    d="M0,15 C26,20 70,8 100,17 L100,100 L0,100 Z"
                />
                <path
                    fill="#3e3326"
                    d="M0,24 C30,29 65,16 100,26 L100,100 L0,100 Z"
                />
                <path
                    fill="#554838"
                    d="M0,30 C28,36 72,21 100,32 L100,100 L0,100 Z"
                />
                <path
                    fill="#4a3f30"
                    d="M0,38 C35,43 65,29 100,41 L100,100 L0,100 Z"
                />
                <path
                    fill="#362c22"
                    d="M0,45 C30,49 60,37 100,47 L100,100 L0,100 Z"
                />
                <path
                    fill="#5a4f40"
                    d="M0,49 C25,54 75,41 100,51 L100,100 L0,100 Z"
                />
                <path
                    fill="#483e30"
                    d="M0,56 C32,62 66,50 100,58 L100,100 L0,100 Z"
                />
                <path
                    fill="#3a3228"
                    d="M0,64 C26,69 70,58 100,66 L100,100 L0,100 Z"
                />
                <path
                    fill="#5e5345"
                    d="M0,69 C28,74 68,62 100,71 L100,100 L0,100 Z"
                />
                <path
                    fill="#4a3d2e"
                    d="M0,75 C32,79 68,69 100,76 L100,100 L0,100 Z"
                />
                <path
                    fill="#362c22"
                    d="M0,79 C32,83 68,74 100,80 L100,100 L0,100 Z"
                />
                <path
                    fill="#554838"
                    d="M0,84 C30,89 70,78 100,85 L100,100 L0,100 Z"
                />
                <path
                    fill="#3e3326"
                    d="M0,91 C35,94 65,86 100,93 L100,100 L0,100 Z"
                />
                <path
                    fill="#2e2620"
                    d="M0,95 C30,98 60,93 100,97 L100,100 L0,100 Z"
                />
            {:else}
                <!-- Light theme: warm saturated earth tones -->
                <path
                    fill="#F2DFB8"
                    d="M0,5 C24,11 76,-1 100,8 L100,100 L0,100 Z"
                />
                <path
                    fill="#D9B78D"
                    d="M0,15 C26,20 70,8 100,17 L100,100 L0,100 Z"
                />
                <path
                    fill="#F2DFB8"
                    d="M0,24 C30,29 65,16 100,26 L100,100 L0,100 Z"
                />
                <path
                    fill="#7A4B29"
                    d="M0,30 C28,36 72,21 100,32 L100,100 L0,100 Z"
                />
                <path
                    fill="#BA8E68"
                    d="M0,38 C35,43 65,29 100,41 L100,100 L0,100 Z"
                />
                <path
                    fill="#4A2E1B"
                    d="M0,45 C30,49 60,37 100,47 L100,100 L0,100 Z"
                />
                <path
                    fill="#7A4B29"
                    d="M0,49 C25,54 75,41 100,51 L100,100 L0,100 Z"
                />
                <path
                    fill="#D9B78D"
                    d="M0,56 C32,62 66,50 100,58 L100,100 L0,100 Z"
                />
                <path
                    fill="#9C6B46"
                    d="M0,64 C26,69 70,58 100,66 L100,100 L0,100 Z"
                />
                <path
                    fill="#e6d5b9"
                    d="M0,69 C28,74 68,62 100,71 L100,100 L0,100 Z"
                />
                <path
                    fill="#BA8E68"
                    d="M0,75 C32,79 68,69 100,76 L100,100 L0,100 Z"
                />
                <path
                    fill="#4A2E1B"
                    d="M0,79 C32,83 68,74 100,80 L100,100 L0,100 Z"
                />
                <path
                    fill="#9C6B46"
                    d="M0,84 C30,89 70,78 100,85 L100,100 L0,100 Z"
                />
                <path
                    fill="#7A4B29"
                    d="M0,91 C35,94 65,86 100,93 L100,100 L0,100 Z"
                />
                <path
                    fill="#4A2E1B"
                    d="M0,95 C30,98 60,93 100,97 L100,100 L0,100 Z"
                />
            {/if}
        </svg>
    </div>

    <!-- Gradient overlay — tints the geo-layers area and continues down the page -->
    <div class="geo-gradient-overlay"></div>

    <!-- Main Content — z-[2] ensures it renders above both layers and gradient overlay -->
    <div class="mx-auto w-full space-y-4 p-4 lg:p-6 2xl:px-8 relative z-2">
        <!-- Hidden component for analytical calculations -->
        <FractionalFlow
            rockProps={{
                s_wc: params.s_wc,
                s_or: params.s_or,
                n_w: params.n_w,
                n_o: params.n_o,
                k_rw_max: params.k_rw_max,
                k_ro_max: params.k_ro_max,
            }}
            fluidProps={{ mu_w: params.mu_w, mu_o: params.mu_o }}
            initialSaturation={params.initialSaturation}
            timeHistory={runtime.rateHistory.map((point) => point.time)}
            injectionRateSeries={runtime.rateHistory.map((point) =>
                Number(point.total_injection ?? 0),
            )}
            reservoir={{
                length: params.nx * params.cellDx,
                area: params.ny * params.cellDy * params.nz * params.cellDz,
                porosity: params.reservoirPorosity,
            }}
            scenarioMode={params.analyticalSolutionMode}
            onAnalyticalData={(detail) => {
                if (params.analyticalSolutionMode === "waterflood") {
                    runtime.analyticalProductionData = detail.production;
                }
            }}
            onAnalyticalMeta={(detail) => {
                if (params.analyticalSolutionMode === "waterflood") {
                    runtime.analyticalMeta = detail;
                }
            }}
        />

        <DepletionAnalytical
            enabled={params.analyticalSolutionMode === "depletion"}
            timeHistory={runtime.rateHistory.map((point) => point.time)}
            reservoir={{
                length: params.nx * params.cellDx,
                area: params.ny * params.cellDy * params.nz * params.cellDz,
                porosity: params.reservoirPorosity,
            }}
            initialSaturation={params.initialSaturation}
            nz={params.nz}
            permMode={params.permMode}
            uniformPermX={params.uniformPermX}
            uniformPermY={params.uniformPermY}
            layerPermsX={params.layerPermsX}
            layerPermsY={params.layerPermsY}
            cellDx={params.cellDx}
            cellDy={params.cellDy}
            cellDz={params.cellDz}
            wellRadius={params.well_radius}
            wellSkin={params.well_skin}
            muO={params.mu_o}
            sWc={params.s_wc}
            sOr={params.s_or}
            nO={params.n_o}
            c_o={params.c_o}
            c_w={params.c_w}
            cRock={params.rock_compressibility}
            initialPressure={params.initialPressure}
            producerBhp={params.producerBhp}
            depletionRateScale={params.analyticalDepletionRateScale}
            onAnalyticalData={(detail) => {
                if (params.analyticalSolutionMode === "depletion") {
                    runtime.analyticalProductionData = detail.production;
                }
            }}
            onAnalyticalMeta={(detail) => {
                if (params.analyticalSolutionMode === "depletion") {
                    runtime.analyticalMeta = detail;
                }
            }}
        />

        <!-- Header -->
        <header
            class="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between"
        >
            <div>
                <h1 class="text-2xl font-bold lg:text-3xl">
                    Simplified Reservoir Simulation Model
                </h1>
                <p class="text-sm opacity-80">
                    Interactive two-phase simulation with 3D visualisation fully
                    in browser.
                </p>
            </div>
            <Button size="sm" variant="outline" onclick={toggleTheme}>
                {theme === "dark" ? "☀ Light" : "🌙 Dark"}
            </Button>
        </header>

        <!-- Mode Panel: mode tabs + grouped dimension selectors + inline parameter panels -->
        <ModePanel
            activeMode={scenario.activeMode}
            isModified={scenario.isModified}
            toggles={scenario.toggles}
            disabledOptions={scenario.disabledOptions}
            onModeChange={scenario.handleModeChange}
            onParamEdit={scenario.handleParamEdit}
            onToggleChange={scenario.handleToggleChange}
            basePreset={scenario.basePreset}
            onCloneBenchmarkToCustom={handleCloneBenchmarkToCustom}
            benchmarkProvenance={scenario.benchmarkProvenance}
            benchmarkSweepRunning={runtime.benchmarkSweepRunning}
            benchmarkSweepProgressLabel={runtime.benchmarkSweepProgressLabel}
            benchmarkSweepError={runtime.benchmarkSweepError}
            benchmarkRunResults={runtime.benchmarkRunResults}
            onRunBenchmarkBase={runtime.runActiveBenchmarkBase}
            onRunBenchmarkSensitivityAxis={runtime.runActiveBenchmarkSensitivityAxis}
            onStopBenchmarkSweep={runtime.stopRun}
            {params}
            validationErrors={params.validationErrors}
            warningPolicy={runtime.warningPolicy}
        />

        <!-- Run Controls -->
        <RunControls
            wasmReady={runtime.wasmReady}
            workerRunning={runtime.workerRunning}
            runCompleted={runtime.runCompleted}
            simTime={runtime.simTime}
            historyLength={runtime.history.length}
            estimatedRunSeconds={runtime.estimatedRunSeconds}
            longRunEstimate={runtime.longRunEstimate}
            hasValidationErrors={params.hasValidationErrors}
            canStop={runtime.workerRunning}
            runProgress={runtime.workerRunning && runtime.currentRunTotalSteps > 0
                ? `${runtime.currentRunStepsCompleted} / ${runtime.currentRunTotalSteps}`
                : ""}
            inputsAnchorHref=""
            bind:steps={params.steps}
            bind:historyInterval={params.historyInterval}
            onRunSteps={runtime.runSteps}
            onStepOnce={runtime.stepOnce}
            onInitSimulator={runtime.initSimulator}
            onStopRun={runtime.stopRun}
            fieldErrors={params.validationErrors}
            warningPolicy={runtime.warningPolicy}
        />

        <!-- Error / Warning banners -->
        {#if runtime.runtimeError}
            <div
                class="rounded-md border border-destructive bg-card text-destructive p-3 text-xs font-medium"
            >
                {runtime.runtimeError}
            </div>
        {/if}

        {#if runtime.warningPolicy.referenceCaveat.items.length > 0}
            <WarningPolicyPanel
                policy={runtime.warningPolicy}
                groups={["referenceCaveat"]}
                groupSources={{ referenceCaveat: ["analytical"] }}
            />
        {/if}

        <div class="grid grid-cols-1 gap-4 xl:grid-cols-2 xl:items-start mt-2">
            <div class="space-y-4">
                <Card class="overflow-hidden">
                    {#if RateChartComponent}
                        <RateChartComponent
                            rateHistory={runtime.rateHistory}
                            analyticalProductionData={runtime.analyticalProductionData}
                            avgReservoirPressureSeries={runtime.avgReservoirPressureSeries}
                            avgWaterSaturationSeries={runtime.avgWaterSaturationSeries}
                            ooipM3={params.ooipM3}
                            poreVolumeM3={params.poreVolumeM3}
                            activeMode={scenario.activeMode}
                            activeCase={scenario.activeCase}
                            {theme}
                            analyticalMeta={runtime.analyticalMeta}
                            layoutConfig={activeRateChartLayoutConfig}
                        />
                    {:else}
                        <div
                            class="p-4 md:p-5 text-sm text-muted-foreground w-full text-center"
                        >
                            Loading rate chart…
                        </div>
                    {/if}
                </Card>

                <Card>
                    <div class="p-4 md:p-5">
                        <SwProfileChart
                            gridState={runtime.gridStateRaw ?? null}
                            nx={params.nx}
                            ny={params.ny}
                            nz={params.nz}
                            cellDx={params.cellDx}
                            cellDy={params.cellDy}
                            cellDz={params.cellDz}
                            simTime={runtime.simTime}
                            producerJ={params.producerJ}
                            initialSaturation={params.initialSaturation}
                            injectionRate={runtime.latestInjectionRate}
                            scenarioMode={params.analyticalSolutionMode}
                            rockProps={{
                                s_wc: params.s_wc,
                                s_or: params.s_or,
                                n_w: params.n_w,
                                n_o: params.n_o,
                            }}
                            fluidProps={{ mu_w: params.mu_w, mu_o: params.mu_o }}
                        />
                    </div>
                </Card>

                {#if params.analyticalSolutionMode === "depletion"}
                    <div
                        class="rounded-md border border-border bg-card p-3 text-xs shadow-sm"
                    >
                        <div class="font-semibold text-foreground">
                            Depletion Analytical Mode
                        </div>
                        <div class="text-muted-foreground mt-1">
                            Model: {runtime.analyticalMeta.shapeLabel || "PSS"} — q(t)&nbsp;=&nbsp;J·ΔP·e<sup
                                >−t/τ</sup
                            >, τ&nbsp;=&nbsp;V<sub>p</sub>·c<sub>t</sub>/J
                        </div>
                    </div>
                {:else if params.analyticalSolutionMode === "waterflood"}
                    <div
                        class="rounded-md border border-border bg-card p-3 text-xs shadow-sm"
                    >
                        <div class="font-semibold text-foreground">
                            Waterflood Analytical Mode
                        </div>
                        <div class="text-muted-foreground mt-1">
                            Model: Buckley-Leverett Fractional Flow
                        </div>
                    </div>
                {/if}
            </div>

            <div class="space-y-4">
                <Card>
                    <div class="p-4 md:p-5">
                        {#if ThreeDViewComponent}
                            {#key `${params.nx}-${params.ny}-${params.nz}-${runtime.vizRevision}`}
                                <ThreeDViewComponent
                                    nx={params.nx}
                                    ny={params.ny}
                                    nz={params.nz}
                                    cellDx={params.cellDx}
                                    cellDy={params.cellDy}
                                    cellDz={params.cellDz}
                                    {theme}
                                    gridState={runtime.gridStateRaw}
                                    bind:showProperty
                                    bind:legendFixedMin
                                    bind:legendFixedMax
                                    s_wc={params.s_wc}
                                    s_or={params.s_or}
                                    bind:currentIndex={runtime.currentIndex}
                                    replayTime={runtime.replayTime}
                                    onApplyHistoryIndex={runtime.applyHistoryIndex}
                                    history={runtime.history}
                                    wellState={runtime.wellStateRaw}
                                />
                            {/key}
                        {:else}
                            <div
                                class="flex items-center justify-center rounded border border-border bg-muted/20"
                                style="height: clamp(240px, 35vh, 420px);"
                            >
                                {#if loadingThreeDView}
                                    <div class="flex items-center space-x-2">
                                        <div
                                            class="h-4 w-4 animate-spin rounded-full border-b-2 border-primary"
                                        ></div>
                                        <span class="text-sm font-medium"
                                            >Loading...</span
                                        >
                                    </div>
                                {:else}
                                    <Button
                                        size="sm"
                                        variant="default"
                                        onclick={loadThreeDViewModule}
                                        >Load 3D view</Button
                                    >
                                {/if}
                            </div>
                        {/if}
                    </div>
                </Card>
            </div>
        </div>

        <!-- Debug State -->
        {#if showDebugState}
            <Card class="mt-4">
                <div class="grid gap-4 p-4 lg:grid-cols-2">
                    <div>
                        <h4 class="mb-2 text-sm font-semibold">
                            Grid State (current)
                        </h4>
                        <pre
                            class="max-h-100 overflow-auto rounded border border-border bg-muted/20 p-2 text-xs">{JSON.stringify(
                                runtime.gridStateRaw,
                                null,
                                2,
                            )}</pre>
                    </div>
                    <div>
                        <h4 class="mb-2 text-sm font-semibold">
                            Well State (current)
                        </h4>
                        <pre
                            class="max-h-105 overflow-auto rounded border border-border bg-muted p-2 text-xs">{JSON.stringify(
                                runtime.wellStateRaw,
                                null,
                                2,
                            )}</pre>
                    </div>
                </div>
            </Card>
        {/if}
    </div>
</main>
